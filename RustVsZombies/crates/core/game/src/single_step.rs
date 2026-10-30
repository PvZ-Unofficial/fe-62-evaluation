//! One actual native battle update, observed at the next shared dispatch boundary.
//! Native hosts already dispatch between individual updates, including accelerated
//! batches. No timer, empty dispatch, worker, or extra gameplay loop is introduced.
use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::{
    AdvancedPauseBackend, BoardReadinessBackend, ClockBackend, GameUiBackend, WorldIdentityBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{AdvancedPauseOptions, GameUi, WorldId};
use rsvz_schedule::state_hook::{StateHookHandle, with_state_hooks_ref};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepProgress {
    Pending,
    Completed,
    Cancelled,
    WorldChanged,
    InvalidAdvance,
}
#[derive(Clone, Copy)]
struct Step {
    id: u64,
    world: WorldId,
    clock: i32,
    progress: StepProgress,
}
std::thread_local! {
    static SERIAL: Cell<u64> = const { Cell::new(0) };
    static STEP: Cell<Option<Step>> = const { Cell::new(None) };
}
std::thread_local! {
    static CLEANUP: Cell<Option<StateHookHandle>> = const { Cell::new(None) };
}

pub(crate) fn pending() -> bool {
    STEP.get().is_some_and(|step| step.progress == StepProgress::Pending)
}

pub fn progress(id: u64) -> Option<StepProgress> {
    STEP.get().filter(|step| step.id == id).map(|step| step.progress)
}

pub fn request() -> RuntimeResult<u64>
where
    CurrentBackend: AdvancedPauseBackend + BoardReadinessBackend + ClockBackend + GameUiBackend + WorldIdentityBackend,
{
    request_with_options(AdvancedPauseOptions::default())
}

/// Complete one update and pause with the caller's presentation options.
pub fn request_with_options(options: AdvancedPauseOptions) -> RuntimeResult<u64>
where
    CurrentBackend: AdvancedPauseBackend + BoardReadinessBackend + ClockBackend + GameUiBackend + WorldIdentityBackend,
{
    if STEP.get().is_some_and(|step| step.progress == StepProgress::Pending) {
        return Err(RuntimeError::new("a single step is already pending"));
    }
    let (world, clock) = crate::access::with_backend(|backend| {
        if backend.game_ui().map_err(crate::access::operation_error)? != GameUi::Playing
            || !backend.playing_board_ready().map_err(crate::access::operation_error)?
        {
            return Err(RuntimeError::new(
                "single step requires a ready battle without a native modal pause",
            ));
        }
        Ok((
            backend
                .world_identity()
                .map_err(crate::access::operation_error)?
                .ok_or_else(|| RuntimeError::new("no step world"))?,
            backend.clock().map_err(crate::access::operation_error)?,
        ))
    })?;
    let id = SERIAL
        .get()
        .checked_add(1)
        .ok_or_else(|| RuntimeError::new("single-step identity exhausted"))?;
    // Establish post-step pause and cleanup before allowing one native update.
    crate::game_pause::set_with_options(true, options)?;
    let installed = CLEANUP
        .get()
        .is_some_and(|handle| with_state_hooks_ref(|hooks| hooks.is_registered(handle)));
    if !installed {
        CLEANUP.set(Some(crate::state_hook::register_native_cleanup(i32::MAX, move || {
            cancel_for_declaration()
        })));
    }
    crate::access::with_backend(|backend| backend.release_advanced_pause_for_step(options))
        .map_err(crate::access::operation_error)?;
    SERIAL.set(id);
    STEP.set(Some(Step {
        id,
        world,
        clock,
        progress: StepProgress::Pending,
    }));
    Ok(id)
}

fn finish(mut step: Step, progress: StepProgress) -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend + GameUiBackend + WorldIdentityBackend,
{
    // Keep Pending if restoring the native pause fails; do not claim completion.
    crate::game_pause::apply_effective()?;
    step.progress = progress;
    STEP.set(Some(step));
    Ok(())
}

pub(crate) fn poll() -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend + ClockBackend + GameUiBackend + WorldIdentityBackend,
{
    let Some(step) = STEP.get().filter(|step| step.progress == StepProgress::Pending) else {
        return Ok(());
    };
    let progress = crate::access::with_backend(|backend| -> RuntimeResult<StepProgress> {
        if backend.world_identity().map_err(crate::access::operation_error)? != Some(step.world)
            || backend.game_ui().map_err(crate::access::operation_error)? != GameUi::Playing
        {
            return Ok(StepProgress::WorldChanged);
        }
        let delta = backend
            .clock()
            .map_err(crate::access::operation_error)?
            .wrapping_sub(step.clock);
        Ok(match delta {
            0 => StepProgress::Pending,
            1 => StepProgress::Completed,
            _ => StepProgress::InvalidAdvance,
        })
    })?;
    if progress != StepProgress::Pending {
        finish(step, progress)?;
    }
    Ok(())
}

pub(crate) fn cancel_for_declaration() -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend + GameUiBackend + WorldIdentityBackend,
{
    if let Some(step) = STEP.get().filter(|step| step.progress == StepProgress::Pending) {
        finish(step, StepProgress::Cancelled)?;
    }
    Ok(())
}
