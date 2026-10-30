//! Explicit source-owned capture of lawn clicks; no editor or pause policy.
use crate::{
    access::{operation_error, with_backend},
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::{BoardClickBackend, WorldIdentityBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{PixelPos, WorldId};
use rsvz_schedule::{
    module::{ModuleSlot, current_module},
    state_hook::StateHookHandle,
};
use std::cell::Cell;
thread_local! { static OWNER: Cell<Option<(ModuleSlot,WorldId,StateHookHandle)>> = const {Cell::new(None)}; }

pub fn start() -> RuntimeResult<()>
where
    CurrentBackend: BoardClickBackend + WorldIdentityBackend,
{
    if OWNER.get().is_some() {
        return Err(RuntimeError::new("lawn input is already captured"));
    }
    let world = crate::board::world_id()?.ok_or_else(|| RuntimeError::new("no current world"))?;
    with_backend(|b| b.set_board_click_capture(true)).map_err(operation_error)?;
    let hook = crate::state_hook::register_native_cleanup(0, stop);
    OWNER.set(Some((current_module(), world, hook)));
    Ok(())
}
pub fn stop() -> RuntimeResult<()>
where
    CurrentBackend: BoardClickBackend,
{
    if let Some((owner, _, hook)) = OWNER.get() {
        if owner != current_module() {
            return Err(RuntimeError::new("lawn input belongs to another source"));
        }
        with_backend(|b| b.set_board_click_capture(false)).map_err(operation_error)?;
        OWNER.set(None);
        crate::state_hook::remove_state_hook(hook);
    }
    Ok(())
}
pub fn take() -> RuntimeResult<Option<(PixelPos, i32)>>
where
    CurrentBackend: BoardClickBackend + WorldIdentityBackend,
{
    let Some((owner, world, _)) = OWNER.get() else {
        return Ok(None);
    };
    if owner != current_module() {
        return Err(RuntimeError::new("lawn input belongs to another source"));
    }
    if crate::board::world_id()? != Some(world) {
        stop()?;
        return Ok(None);
    }
    with_backend(|b| b.take_board_click())
        .map_err(operation_error)
        .map(|value| value.map(|(x, y, button)| (PixelPos { x, y }, button)))
}

/// Keep native card/tool use available. Opt-in; ordinary capture is unchanged.
pub fn prefer_native_cursor() -> RuntimeResult<()>
where
    CurrentBackend: BoardClickBackend,
{
    ensure_owner()?;
    with_backend(|b| b.set_board_click_native_priority(true)).map_err(operation_error)
}

pub fn preview(value: Option<rsvz_model::LawnPreview>) -> RuntimeResult<()>
where
    CurrentBackend: BoardClickBackend,
{
    ensure_owner()?;
    with_backend(|b| b.set_lawn_preview(value)).map_err(operation_error)
}

/// False after native input took over; queued earlier clicks can still be drained.
pub fn active() -> RuntimeResult<bool>
where
    CurrentBackend: BoardClickBackend,
{
    ensure_owner()?;
    with_backend(|b| b.board_click_capture_active()).map_err(operation_error)
}

fn ensure_owner() -> RuntimeResult<()> {
    if OWNER.get().is_some_and(|(owner, _, _)| owner == current_module()) {
        Ok(())
    } else {
        Err(RuntimeError::new("lawn input is not owned by this source"))
    }
}
