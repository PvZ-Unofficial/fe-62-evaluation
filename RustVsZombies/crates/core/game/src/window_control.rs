//! Source-owned native background and ordinary pause-dialog settings.
use crate::{control::OwnedRule, runtime::RuntimeResult};
use rsvz_backend_api::WindowRulesBackend;
use rsvz_current::CurrentBackend;
use rsvz_model::BackgroundRunRule;
use std::cell::RefCell;

std::thread_local! {
    static BACKGROUND: RefCell<OwnedRule<BackgroundRunRule>> = const { RefCell::new(OwnedRule::new()) };
}
fn read_background() -> RuntimeResult<BackgroundRunRule>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::access::with_backend(|backend| backend.background_run_rule()).map_err(crate::access::operation_error)
}
fn apply_background(rule: BackgroundRunRule) -> RuntimeResult<()>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::access::with_backend(|backend| backend.set_background_run_rule(rule)).map_err(crate::access::operation_error)
}

pub fn set_background_rule(rule: BackgroundRunRule) -> RuntimeResult<()>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::control::declare_persistent(&BACKGROUND, rule, read_background, apply_background)
}

/// Supply a source-owned fallback, used only while no explicit request exists.
pub fn set_background_fallback(rule: BackgroundRunRule) -> RuntimeResult<()>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::control::declare_fallback(&BACKGROUND, rule, read_background, apply_background)
}

pub fn set_pause_dialog_disabled(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::control::declare_native_rule!(
        bool,
        enabled,
        pause_dialog_disabled,
        set_pause_dialog_disabled,
        CONTROL_PAUSE_DIALOG_DISABLED
    )
}

crate::control::rule_ledgers! {
    CONTROL_WINDOW_RESIZABLE: bool,
    CONTROL_WINDOW_LEFT_ALIGNED: bool,
    CONTROL_WINDOW_EXPANDED: bool,
    CONTROL_WINDOW_SCALE: f64,
    CONTROL_PAUSE_DIALOG_DISABLED: bool,
}

/// Allow dragging the native game window border. The logical game canvas stays unchanged.
/// The request is source-owned and restored when that source stops.
pub fn set_resizable(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::control::declare_native_rule!(
        bool,
        enabled,
        window_resizable,
        set_window_resizable,
        CONTROL_WINDOW_RESIZABLE
    )
}

/// Show more of the existing background without changing the logical game layout.
pub fn set_expanded(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::control::declare_native_rule!(
        bool,
        enabled,
        window_expanded,
        set_window_expanded,
        CONTROL_WINDOW_EXPANDED
    )
}

/// Supply a resize default while no source has made an explicit declaration.
pub fn set_resizable_fallback(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::control::declare_fallback(
        &CONTROL_WINDOW_RESIZABLE,
        enabled,
        || crate::access::with_backend(|b| b.window_resizable()).map_err(crate::access::operation_error),
        |value| crate::access::with_backend(|b| b.set_window_resizable(value)).map_err(crate::access::operation_error),
    )
}

/// Supply an expansion default while preserving another source's explicit choice.
pub fn set_expanded_fallback(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::control::declare_fallback(
        &CONTROL_WINDOW_EXPANDED,
        enabled,
        || crate::access::with_backend(|b| b.window_expanded()).map_err(crate::access::operation_error),
        |value| crate::access::with_backend(|b| b.set_window_expanded(value)).map_err(crate::access::operation_error),
    )
}
/// Scale the game window and picture together, preserving the visible region.
pub fn set_scale(scale: f64) -> RuntimeResult<()>
where
    CurrentBackend: WindowRulesBackend,
{
    if !scale.is_finite() || scale <= 0.0 {
        return Err(crate::runtime::RuntimeError::new(
            "game window scale must be finite and positive",
        ));
    }
    crate::control::declare_native_rule!(f64, scale, window_scale, set_window_scale, CONTROL_WINDOW_SCALE)
}
pub fn scale() -> RuntimeResult<f64>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::access::with_backend(|backend| backend.window_scale()).map_err(crate::access::operation_error)
}
pub fn view_width() -> RuntimeResult<i32>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::access::with_backend(|backend| backend.window_view_width()).map_err(crate::access::operation_error)
}

/// Prefer the original left edge, shifting only when the right resource edge is reached.
/// False retains the default lawn-centered presentation. Does not enable expansion.
pub fn set_left_aligned(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: WindowRulesBackend,
{
    crate::control::declare_native_rule!(
        bool,
        enabled,
        window_left_aligned,
        set_window_left_aligned,
        CONTROL_WINDOW_LEFT_ALIGNED
    )
}
