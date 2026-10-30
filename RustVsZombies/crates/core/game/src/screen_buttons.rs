//! Source-owned screen controls; actions and product defaults belong to callers.
use crate::{
    access::{operation_error, with_backend},
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::ScreenButtonBackend;
use rsvz_current::CurrentBackend;
pub use rsvz_model::ScreenButton;
use rsvz_schedule::{
    module::{ModuleSlot, current_module},
    state_hook::StateHookHandle,
};
use std::cell::Cell;
std::thread_local! { static CLEANUP: Cell<Option<StateHookHandle>> = const {Cell::new(None)}; }

fn source() -> u8 {
    u8::from(current_module() == ModuleSlot::Modifier)
}

// Keep capability-gated bodies at their call sites: the 1051 shared SDK must
// not export uninstantiable functions for a capability it does not implement.
#[inline]
pub fn replace(buttons: &[ScreenButton]) -> RuntimeResult<()>
where
    CurrentBackend: ScreenButtonBackend,
{
    validate(buttons)?;
    with_backend(|backend| backend.replace_screen_buttons(source(), buttons)).map_err(operation_error)?;
    if CLEANUP.get().is_none() {
        CLEANUP.set(Some(crate::state_hook::register_native_cleanup(0, clear)));
    }
    Ok(())
}
#[inline]
pub fn clear() -> RuntimeResult<()>
where
    CurrentBackend: ScreenButtonBackend,
{
    with_backend(|backend| backend.replace_screen_buttons(source(), &[])).map_err(operation_error)?;
    if let Some(hook) = CLEANUP.take() {
        crate::state_hook::remove_state_hook(hook);
    }
    Ok(())
}
#[inline]
pub fn take() -> RuntimeResult<Option<u32>>
where
    CurrentBackend: ScreenButtonBackend,
{
    with_backend(|backend| backend.take_screen_button(source())).map_err(operation_error)
}
#[inline]
pub fn available() -> RuntimeResult<bool>
where
    CurrentBackend: ScreenButtonBackend,
{
    with_backend(|backend| backend.screen_buttons_available(source())).map_err(operation_error)
}
fn validate(buttons: &[ScreenButton]) -> RuntimeResult<()> {
    let mut used = [0u8; 2];
    if buttons.len() > 12 {
        return Err(RuntimeError::new("too many screen buttons"));
    }
    for (i, button) in buttons.iter().enumerate() {
        if button.id == 0
            || button.id == u32::MAX
            || button.first >= 6
            || button.span == 0
            || button.span > 6 - button.first
            || button.label.is_empty()
            || button.label.chars().count() > 32
            || buttons[..i].iter().any(|other| other.id == button.id)
        {
            return Err(RuntimeError::new("invalid screen button"));
        }
        let mask = ((1u8 << button.span) - 1) << button.first;
        let side = usize::from(button.right);
        if used[side] & mask != 0 {
            return Err(RuntimeError::new("overlapping screen buttons"));
        }
        used[side] |= mask;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_rejects_overlap_and_keeps_sides_independent() {
        let pause = ScreenButton {
            id: 1,
            right: true,
            first: 2,
            span: 2,
            label: "暂停".into(),
        };
        assert!(validate(std::slice::from_ref(&pause)).is_ok());
        let mut next = ScreenButton { id: 2, ..pause.clone() };
        assert!(validate(&[pause.clone(), next.clone()]).is_err());
        next.right = false;
        assert!(validate(&[pause, next]).is_ok());
    }
}
