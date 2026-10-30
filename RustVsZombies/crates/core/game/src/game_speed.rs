//! Configured native update multiplier for this execution module.
use crate::{
    control::OwnedRule,
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::GameSpeedHintBackend;
use rsvz_current::CurrentBackend;
use rsvz_model::PositiveFiniteF32;
use rsvz_schedule::state_hook::{StateEvent, StateHookHandle, with_state_hooks_ref};
use std::cell::{Cell, RefCell};

std::thread_local! {
    static SPEED: RefCell<OwnedRule<f64>> = const { RefCell::new(OwnedRule::new()) };
}
std::thread_local! {
    static FIGHT_CLEANUP: Cell<Option<StateHookHandle>> = const { Cell::new(None) };
}

/// Scope is explicit and independent of module identity. The existing `set`
/// entry retains its ordinary battle-lifetime behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedLifetime {
    Battle,
    Session,
}

fn native_value() -> RuntimeResult<f64>
where
    CurrentBackend: GameSpeedHintBackend,
{
    let value = crate::access::with_backend(|backend| backend.game_speed_multiplier())
        .map_err(crate::access::operation_error)?;
    if !value.is_finite() || value < 0.0 {
        return Err(RuntimeError::new("native update multiplier is invalid"));
    }
    Ok(value)
}
fn apply(value: f64) -> RuntimeResult<()>
where
    CurrentBackend: GameSpeedHintBackend,
{
    crate::access::with_backend(|backend| backend.set_game_speed_multiplier(value))
        .map_err(crate::access::operation_error)
}

pub fn read() -> RuntimeResult<Option<f64>>
where
    CurrentBackend: GameSpeedHintBackend,
{
    if CurrentBackend::GAME_SPEED_IS_NOOP {
        Ok(None)
    } else {
        native_value().map(Some)
    }
}

pub fn set(value: f32) -> RuntimeResult<()>
where
    CurrentBackend: GameSpeedHintBackend,
{
    set_with_lifetime(value, SpeedLifetime::Battle)
}

pub fn set_with_lifetime(value: f32, lifetime: SpeedLifetime) -> RuntimeResult<()>
where
    CurrentBackend: GameSpeedHintBackend,
{
    let value = PositiveFiniteF32::new(value).map_err(|error| RuntimeError::new(error.to_string()))?;
    if CurrentBackend::GAME_SPEED_IS_NOOP {
        return crate::access::with_backend(|backend| backend.set_game_speed(value))
            .map_err(crate::access::operation_error);
    }
    if !(0.05..=100.0).contains(&value.get()) {
        return Err(RuntimeError::new("game speed must be in 0.05..=100"));
    }
    crate::control::declare_persistent(&SPEED, f64::from(value.get()), native_value, apply)?;
    if lifetime == SpeedLifetime::Battle {
        let installed = FIGHT_CLEANUP
            .get()
            .is_some_and(|handle| with_state_hooks_ref(|hooks| hooks.is_registered(handle)));
        if !installed {
            FIGHT_CLEANUP.set(Some(crate::state_hook::register_fallible(
                StateEvent::ExitFight,
                i32::MAX,
                release,
            )));
        }
    } else if let Some(handle) = FIGHT_CLEANUP.take() {
        rsvz_schedule::state_hook::with_state_hooks(|hooks| {
            hooks.remove(handle);
        });
    }
    Ok(())
}

pub fn release() -> RuntimeResult<()>
where
    CurrentBackend: GameSpeedHintBackend,
{
    if CurrentBackend::GAME_SPEED_IS_NOOP {
        return Ok(());
    }
    crate::control::release(&SPEED, native_value, apply)
}

/// Requests are distinct from the actual native value returned by `read`.
pub fn declaration() -> Option<f64> {
    SPEED.with_borrow(OwnedRule::declaration)
}

/// Reapply this source's effective request after an external/native timing reset.
/// The caller decides whether and when to maintain a persistent preference.
pub fn reapply() -> RuntimeResult<()>
where
    CurrentBackend: GameSpeedHintBackend,
{
    if let Some(value) = SPEED.with_borrow(OwnedRule::declaration) {
        if native_value()?.to_bits() != value.to_bits() {
            apply(value)?;
        }
    }
    Ok(())
}
