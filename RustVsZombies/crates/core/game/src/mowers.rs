//! Explicit native mower actions. No module is stopped by these operations.
use crate::{
    access::operation_error,
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::{
    BoardReadinessBackend, DeadObjectReclaimBackend, GameUiBackend, LawnMowerEditBackend, LawnMowerReadBackend,
    LawnMowerRestoreBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::GameUi;

pub fn validate() -> RuntimeResult<()>
where
    CurrentBackend: BoardReadinessBackend + GameUiBackend,
{
    crate::access::with_backend(|backend| {
        if backend.game_ui().map_err(operation_error)? != GameUi::Playing
            || !backend.playing_board_ready().map_err(operation_error)?
        {
            return Err(RuntimeError::new("mower actions require a ready battle"));
        }
        Ok(())
    })
}

pub fn start_all() -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend + GameUiBackend + LawnMowerEditBackend,
{
    validate()?;
    crate::access::with_backend(|backend| {
        let mut count = 0;
        for mower in backend.lawn_mowers().map_err(operation_error)? {
            backend.start_lawn_mower(mower).map_err(operation_error)?;
            count += 1;
        }
        Ok(count)
    })
}

/// Apply native Die to exactly the initial live set. Native bonus replacements
/// remain real game side effects and are not silently suppressed or re-killed.
pub fn clear_all() -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend + GameUiBackend + LawnMowerEditBackend,
{
    validate()?;
    crate::access::with_backend(clear_native)
}

fn clear_native(backend: &CurrentBackend) -> RuntimeResult<usize>
where
    CurrentBackend: LawnMowerEditBackend,
{
    let mowers: Vec<_> = backend.lawn_mowers().map_err(operation_error)?.collect();
    let count = mowers.len();
    for mower in mowers {
        backend.kill_lawn_mower(mower).map_err(operation_error)?;
    }
    Ok(count)
}

/// Clear the initial set, reclaim dead native resources without advancing time,
/// then run ready-position initialization with explicit mode-guard bypass.
pub fn restore() -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + LawnMowerEditBackend
        + LawnMowerRestoreBackend
        + DeadObjectReclaimBackend,
{
    validate()?;
    crate::access::with_exclusive_backend(|backend| {
        clear_native(backend)?;
        backend.reclaim_dead_objects().map_err(operation_error)?;
        backend.initialize_lawn_mowers_ready().map_err(operation_error)?;
        Ok(backend.lawn_mowers().map_err(operation_error)?.count())
    })
}
