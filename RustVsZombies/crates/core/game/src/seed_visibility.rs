//! Source-owned whole seed-bank visibility, rebound to each ready native world.
use crate::{control::OwnedRule, runtime::RuntimeResult};
use rsvz_backend_api::{BoardReadinessBackend, GameUiBackend, SeedBankVisibilityBackend, WorldIdentityBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{GameUi, WorldId};
use rsvz_schedule::tick::{TickControl, TickHandle, TickLifetime, TickOptions, TickTaskState, with_scheduler};
use std::cell::{Cell, RefCell};

std::thread_local! {
    static RULE: RefCell<OwnedRule<Option<bool>>> = const { RefCell::new(OwnedRule::new()) };
    static WORLD: Cell<Option<WorldId>> = const { Cell::new(None) };
}
std::thread_local! { static TASK: Cell<Option<TickHandle>> = const { Cell::new(None) }; }

pub(crate) fn ready_world() -> RuntimeResult<Option<WorldId>>
where
    CurrentBackend: WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    crate::access::with_backend(|backend| {
        let ready = match backend.game_ui().map_err(crate::access::operation_error)? {
            GameUi::Playing => backend.playing_board_ready(),
            GameUi::LevelIntro => backend.level_intro_board_ready(),
            _ => return Ok(None),
        }
        .map_err(crate::access::operation_error)?;
        if ready {
            backend.world_identity().map_err(crate::access::operation_error)
        } else {
            Ok(None)
        }
    })
}
fn read() -> RuntimeResult<Option<bool>>
where
    CurrentBackend: SeedBankVisibilityBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    let world = ready_world()?;
    let value = if world.is_some() {
        Some(
            crate::access::with_backend(|backend| backend.seed_bank_visible())
                .map_err(crate::access::operation_error)?,
        )
    } else {
        None
    };
    WORLD.set(world);
    Ok(value)
}
fn apply(value: Option<bool>) -> RuntimeResult<()>
where
    CurrentBackend: SeedBankVisibilityBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    if let Some(value) = value {
        // If cleanup races a world replacement, this feature has not written
        // the new bank yet and must not restore the previous bank's baseline.
        if WORLD.get().is_some() && ready_world()? == WORLD.get() {
            crate::access::with_backend(|backend| backend.set_seed_bank_visible(value))
                .map_err(crate::access::operation_error)?;
        }
    }
    Ok(())
}
fn maintain() -> RuntimeResult<()>
where
    CurrentBackend: SeedBankVisibilityBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    let desired = RULE.with_borrow(|rule| rule.effective_declaration());
    if desired.is_none() {
        return Ok(());
    }
    if ready_world()? != WORLD.get() {
        let baseline = read()?;
        RULE.with_borrow_mut(|rule| rule.replace_baseline(baseline));
    }
    apply(desired.flatten())
}

pub fn set_visible(visible: bool) -> RuntimeResult<()>
where
    CurrentBackend: SeedBankVisibilityBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    if CurrentBackend::SEED_BANK_VISIBILITY_IS_NOOP {
        return Ok(());
    }
    maintain()?;
    crate::control::declare(&RULE, Some(visible), read, apply)?;
    with_scheduler(|scheduler| {
        if TASK
            .get()
            .is_some_and(|handle| scheduler.state(handle) != TickTaskState::Stopped)
        {
            return;
        }
        TASK.set(Some(
            scheduler.spawn(
                TickOptions::active_phase()
                    .lifetime(TickLifetime::Session)
                    .idle_neutral()
                    .name("seed_bank_visibility"),
                |_| {
                    let active = RULE.with_borrow(|rule| rule.declaration().is_some());
                    if active {
                        maintain()?;
                    }
                    Ok(TickControl::Continue)
                },
            ),
        ));
    });
    Ok(())
}

pub fn release() -> RuntimeResult<()>
where
    CurrentBackend: SeedBankVisibilityBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    if CurrentBackend::SEED_BANK_VISIBILITY_IS_NOOP {
        return Ok(());
    }
    maintain()?;
    crate::control::release(&RULE, read, apply)
}
