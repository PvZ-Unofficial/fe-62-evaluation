//! A-TAS repeated native Dance operations, separate from SEML gait settings.
use crate::{control::OwnedRule, runtime::RuntimeResult};
use rsvz_backend_api::DanceModeBackend;
use rsvz_current::CurrentBackend;
use rsvz_schedule::tick::{TickControl, TickHandle, TickLifetime, TickOptions, TickTaskState, with_scheduler};
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DanceRepeatMode {
    Off,
    Fast,
    Slow,
}

#[derive(Clone, Copy)]
enum Declaration {
    Native(bool),
    Repeat(DanceRepeatMode),
}

std::thread_local! {
    static RULE: RefCell<OwnedRule<Declaration>> = const { RefCell::new(OwnedRule::new()) };
}
std::thread_local! {
    static TASK: Cell<Option<TickHandle>> = const { Cell::new(None) };
}

fn native() -> RuntimeResult<Declaration>
where
    CurrentBackend: DanceModeBackend,
{
    crate::access::with_backend(|backend| backend.dance_mode_enabled())
        .map(Declaration::Native)
        .map_err(crate::access::operation_error)
}
fn reset(value: Declaration) -> RuntimeResult<()>
where
    CurrentBackend: DanceModeBackend,
{
    // A-TAS resets Dance(false) immediately on Fast/Slow/Off changes. Repeated
    // per-frame calls below must not be replaced by flag-value deduplication.
    let enabled = match value {
        Declaration::Native(enabled) => enabled,
        Declaration::Repeat(_) => false,
    };
    crate::access::with_backend(|backend| backend.restore_dance_mode(enabled)).map_err(crate::access::operation_error)
}

pub fn effective_mode() -> DanceRepeatMode {
    RULE.with_borrow(|rule| match rule.effective_declaration() {
        Some(Declaration::Repeat(mode)) => mode,
        _ => DanceRepeatMode::Off,
    })
}

pub fn set_repeat(mode: DanceRepeatMode) -> RuntimeResult<()>
where
    CurrentBackend: DanceModeBackend,
{
    crate::control::declare(&RULE, Declaration::Repeat(mode), native, reset)?;
    with_scheduler(|scheduler| {
        if TASK
            .get()
            .is_some_and(|handle| scheduler.state(handle) != TickTaskState::Stopped)
        {
            return;
        }
        TASK.set(Some(
            scheduler.spawn(
                TickOptions::playing_frame()
                    .lifetime(TickLifetime::Session)
                    .idle_neutral()
                    .name("dance_repeat"),
                |_| {
                    let selected = RULE.with_borrow(|rule| {
                        if rule.declaration().is_some() {
                            rule.effective_declaration()
                        } else {
                            None
                        }
                    });
                    if let Some(Declaration::Repeat(mode @ (DanceRepeatMode::Fast | DanceRepeatMode::Slow))) = selected
                    {
                        crate::access::with_backend(|backend| backend.set_dance_mode(mode == DanceRepeatMode::Slow))
                            .map_err(crate::access::operation_error)?;
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
    CurrentBackend: DanceModeBackend,
{
    crate::control::release(&RULE, native, reset)
}
