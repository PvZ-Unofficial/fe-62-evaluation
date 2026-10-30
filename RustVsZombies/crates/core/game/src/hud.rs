//! Source-owned native HUD visibility; exact fade values are restored per world.
use crate::control::OwnedRule;
use crate::runtime::RuntimeResult;
use rsvz_backend_api::{
    BoardReadinessBackend, GameHudBackend, GameUiBackend, HudField, SeedBankVisibilityBackend, WorldIdentityBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::WorldId;
use rsvz_schedule::{TickControl, TickHandle, TickLifetime, TickOptions, TickTaskState, tick::with_scheduler};
use std::cell::{Cell, RefCell};
const FIELDS: [HudField; 3] = [
    HudField::ShovelVisible,
    HudField::CoinBankFadeCountdown,
    HudField::MenuButtonHidden,
];
thread_local! {
    static RULE:RefCell<OwnedRule<Option<[i32;3]>>>=const{RefCell::new(OwnedRule::new())};
    static WORLD:Cell<Option<WorldId>>=const{Cell::new(None)};
}
std::thread_local! {static TASK:Cell<Option<TickHandle>>=const{Cell::new(None)};}
fn read() -> RuntimeResult<Option<[i32; 3]>>
where
    CurrentBackend:
        GameHudBackend + SeedBankVisibilityBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    let world = crate::seed_visibility::ready_world()?;
    let value = if world.is_some() {
        Some(crate::access::with_backend(|b| -> RuntimeResult<_> {
            Ok([
                b.hud_field(FIELDS[0])?,
                b.hud_field(FIELDS[1])?,
                b.hud_field(FIELDS[2])?,
            ])
        })?)
    } else {
        None
    };
    WORLD.set(world);
    Ok(value)
}
fn apply(value: Option<[i32; 3]>) -> RuntimeResult<()>
where
    CurrentBackend:
        GameHudBackend + SeedBankVisibilityBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    if let Some(value) = value
        && WORLD.get().is_some()
        && crate::seed_visibility::ready_world()? == WORLD.get()
    {
        crate::access::with_backend(|b| -> RuntimeResult<()> {
            for (field, value) in FIELDS.into_iter().zip(value) {
                b.set_hud_field(field, value)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}
fn maintain() -> RuntimeResult<()>
where
    CurrentBackend:
        GameHudBackend + SeedBankVisibilityBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    let desired = RULE.with_borrow(|rule| rule.effective_declaration());
    if desired.is_some() && crate::seed_visibility::ready_world()? != WORLD.get() {
        let baseline = read()?;
        RULE.with_borrow_mut(|rule| rule.replace_baseline(baseline));
        apply(desired.flatten())?;
    }
    // In particular, do not rewrite the coin-bank fade counter every frame.
    Ok(())
}
/// Applies the original hide-UI combination. Headless display configuration is a no-op.
pub fn set_hidden(hidden: bool) -> RuntimeResult<()>
where
    CurrentBackend:
        GameHudBackend + SeedBankVisibilityBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    if CurrentBackend::HUD_IS_NOOP {
        return Ok(());
    }
    maintain()?;
    let previous = RULE.with_borrow(|rule| rule.declaration());
    crate::control::declare(
        &RULE,
        Some([i32::from(!hidden), i32::from(hidden), i32::from(hidden)]),
        read,
        apply,
    )?;
    if let Err(error) = crate::seed_visibility::set_visible(!hidden) {
        match previous {
            Some(value) => crate::control::declare(&RULE, value, read, apply)?,
            None => crate::control::release(&RULE, read, apply)?,
        }
        return Err(error);
    }
    with_scheduler(|scheduler| {
        if TASK
            .get()
            .is_some_and(|task| scheduler.state(task) != TickTaskState::Stopped)
        {
            return;
        }
        TASK.set(Some(
            scheduler.spawn(
                TickOptions::active_phase()
                    .lifetime(TickLifetime::Session)
                    .idle_neutral()
                    .name("native_hud"),
                |_| {
                    maintain()?;
                    Ok(TickControl::Continue)
                },
            ),
        ));
    });
    Ok(())
}
pub fn release() -> RuntimeResult<()>
where
    CurrentBackend:
        GameHudBackend + SeedBankVisibilityBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    if CurrentBackend::HUD_IS_NOOP {
        return Ok(());
    }
    maintain()?;
    crate::control::release(&RULE, read, apply)?;
    crate::seed_visibility::release()
}
