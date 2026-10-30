//! Source-owned advanced game pause. This does not pause a script's tasks.
use crate::{
    control::OwnedRule,
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::{AdvancedPauseBackend, ClockBackend, GameUiBackend, WorldIdentityBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{AdvancedPauseOptions, GameUi, WorldId};
use rsvz_schedule::state_hook::{StateEvent, StateHookHandle, with_state_hooks_ref};
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy)]
struct Request {
    world: Option<WorldId>,
    enabled: bool,
    options: AdvancedPauseOptions,
}
std::thread_local! {
    static PAUSE: RefCell<OwnedRule<Request>> = const { RefCell::new(OwnedRule::new()) };
}
std::thread_local! {
    static WORLD_HOOK: Cell<Option<StateHookHandle>> = const { Cell::new(None) };
}

fn read_native() -> RuntimeResult<Request>
where
    CurrentBackend: AdvancedPauseBackend + WorldIdentityBackend + GameUiBackend,
{
    crate::access::with_backend(|backend| {
        Ok(Request {
            world: backend.world_identity().map_err(crate::access::operation_error)?,
            enabled: backend.advanced_pause_active(),
            options: backend.advanced_pause_options(),
        })
    })
}
fn apply(request: Request) -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend + WorldIdentityBackend + GameUiBackend,
{
    crate::access::with_backend(|backend| {
        let world = backend.world_identity().map_err(crate::access::operation_error)?;
        // A previous Board's pause must never freeze its successor during cleanup.
        let enabled = request.enabled
            && request.world.is_some()
            && request.world == world
            && backend.game_ui().map_err(crate::access::operation_error)? == GameUi::Playing;
        backend
            .set_advanced_pause_with_options(enabled, request.options)
            .map_err(crate::access::operation_error)
    })
}

pub fn active() -> bool
where
    CurrentBackend: AdvancedPauseBackend,
{
    crate::access::with_backend(|backend| backend.advanced_pause_active())
}

/// Declare pause using the library's existing presentation defaults.
pub fn set(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend + WorldIdentityBackend + GameUiBackend + ClockBackend,
{
    set_with_options(enabled, AdvancedPauseOptions::default())
}

/// Presentation belongs to the caller; source ownership and world cleanup are
/// identical for every choice.
pub fn set_with_options(enabled: bool, options: AdvancedPauseOptions) -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend + WorldIdentityBackend + GameUiBackend + ClockBackend,
{
    maintain_world()?;
    crate::single_step::cancel_for_declaration()?;
    let world = crate::access::with_backend(|backend| {
        if enabled && backend.game_ui().map_err(crate::access::operation_error)? != GameUi::Playing {
            return Err(RuntimeError::new("advanced pause requires a battle"));
        }
        backend.world_identity().map_err(crate::access::operation_error)
    })?;
    if enabled && world.is_none() {
        return Err(RuntimeError::new("advanced pause requires a current world"));
    }
    crate::control::declare(
        &PAUSE,
        Request {
            world,
            enabled,
            options,
        },
        read_native,
        apply,
    )?;
    let installed = WORLD_HOOK
        .get()
        .is_some_and(|handle| with_state_hooks_ref(|hooks| hooks.is_registered(handle)));
    if !installed {
        WORLD_HOOK.set(Some(crate::state_hook::register_fallible(
            StateEvent::BeforeTick,
            i32::MIN,
            maintain_world,
        )));
    }
    Ok(())
}

pub fn release() -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend + WorldIdentityBackend + GameUiBackend,
{
    crate::single_step::cancel_for_declaration()?;
    crate::control::release(&PAUSE, read_native, apply)
}

pub fn declaration() -> Option<bool> {
    PAUSE.with_borrow(|pause| pause.declaration().map(|request| request.enabled))
}

pub(crate) fn replay_world_restored(before: WorldId, after: WorldId) {
    PAUSE.with_borrow_mut(|pause| {
        pause.map_values(|mut request| {
            if request.world == Some(before) {
                request.world = Some(after);
            }
            request
        })
    });
}

/// Called by the requesting module's own lifecycle hook and the modifier service
/// while no battle tick can run. No tasks or entity scans are installed.
#[doc(hidden)]
pub fn maintain_world() -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend + WorldIdentityBackend + GameUiBackend + ClockBackend,
{
    crate::single_step::poll()?;
    let world =
        crate::access::with_backend(|backend| backend.world_identity()).map_err(crate::access::operation_error)?;
    let playing = crate::access::with_backend(|backend| backend.game_ui()).map_err(crate::access::operation_error)?
        == GameUi::Playing;
    let stale = PAUSE.with_borrow(|pause| {
        pause
            .declaration()
            .is_some_and(|request| request.world != world || !playing)
    });
    if stale {
        release()?;
    }
    Ok(())
}

pub(crate) fn apply_effective() -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend + WorldIdentityBackend + GameUiBackend,
{
    if let Some(request) = PAUSE.with_borrow(|pause| pause.effective_declaration()) {
        apply(request)?;
    }
    Ok(())
}
