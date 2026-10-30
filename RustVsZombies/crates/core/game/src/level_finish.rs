//! Non-blocking PT survival-round completion sequence. Defaults, stopping other
//! scripts and choosing pause authority belong to the caller.
use crate::{
    access::{operation_error, with_backend, with_exclusive_backend},
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::{
    AdvancedPauseBackend, ClockBackend, FrameTimingBackend, GameUiBackend, NativeGameModeBackend,
    NativeLevelFlowBackend, SunWriteBackend, WorldIdentityBackend, ZombiePresenceBackend, ZombieStatusWriteBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{GameUi, WorldId, ZombieDeathState};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

// Native difficulty computes (stage*20 + wave + 10)*2 before dividing.
pub const MAX_TARGET_FLAGS: u32 = (((i32::MAX / 2 - 110) / 20) * 2) as u32;

#[derive(Clone, Copy, Debug)]
enum Phase {
    Prepared,
    Align(i32),
    Fade(Instant),
    Kill(Instant),
    Resume(Instant),
    Values(i32),
    Boundary,
    Done,
    Cancelled,
}
struct State {
    world: WorldId,
    target_stage: Option<i32>,
    mode: i32,
    initial_stage: i32,
    deadline: Option<Instant>,
    sun: u32,
    duration: Duration,
    phase: Phase,
    started: bool,
    opened_pause: bool,
}
pub struct EndlessRoundFinish {
    state: Rc<RefCell<State>>,
    cleanup: Option<rsvz_schedule::state_hook::StateHookHandle>,
    source: rsvz_schedule::module::ModuleSlot,
}

impl EndlessRoundFinish {
    /// Validate everything before the caller stops any other execution. Final
    /// flags describe the next chooser, after the native survival-stage increment.
    pub fn prepare(flags: u32, sun: u32) -> RuntimeResult<Self>
    where
        CurrentBackend: NativeLevelFlowBackend + NativeGameModeBackend + GameUiBackend + WorldIdentityBackend,
    {
        if flags % 2 != 0 || flags > MAX_TARGET_FLAGS || sun > i32::MAX as u32 {
            return Err(RuntimeError::new(
                "invalid final flags or sun for native survival arithmetic",
            ));
        }
        let (world, duration) = with_backend(|backend| {
            if backend.game_ui().map_err(operation_error)? != GameUi::Playing
                || !(11..=15).contains(&backend.native_game_mode().map_err(operation_error)?)
            {
                return Err(RuntimeError::new(
                    "target survival flags require an endless survival battle",
                ));
            }
            if backend.next_survival_stage_countdown().map_err(operation_error)? != 0 {
                return Err(RuntimeError::new("native round completion is already pending"));
            }
            backend.validate_level_completion().map_err(operation_error)?;
            Ok((
                backend
                    .world_identity()
                    .map_err(operation_error)?
                    .ok_or_else(|| RuntimeError::new("no current world"))?,
                Duration::from_millis(u64::from(backend.native_frame_duration_ms().map_err(operation_error)?)),
            ))
        })?;
        Ok(Self {
            cleanup: None,
            source: rsvz_schedule::module::current_module(),
            state: Rc::new(RefCell::new(State {
                world,
                target_stage: Some((flags / 2) as i32),
                mode: with_backend(|backend| backend.native_game_mode()).map_err(operation_error)?,
                initial_stage: with_backend(|backend| backend.survival_stage()).map_err(operation_error)?,
                deadline: None,
                sun,
                duration,
                phase: Phase::Prepared,
                started: false,
                opened_pause: false,
            })),
        })
    }
    pub fn world(&self) -> WorldId {
        self.state.borrow().world
    }
    pub fn started(&self) -> bool {
        self.state.borrow().started
    }
    pub fn targets_flags(&self) -> bool {
        self.state.borrow().target_stage.is_some()
    }

    /// PT's same native completion sequence for ordinary battle modes. Target
    /// flags are an endless-survival concept and are deliberately not written
    /// into an unrelated challenge's stage field.
    pub fn prepare_native(sun: u32) -> RuntimeResult<Self>
    where
        CurrentBackend: NativeLevelFlowBackend + NativeGameModeBackend + GameUiBackend + WorldIdentityBackend,
    {
        if sun > i32::MAX as u32 {
            return Err(RuntimeError::new("sun exceeds native range"));
        }
        let (world, mode, initial_stage, duration) = with_backend(|backend| {
            let mode = backend.native_game_mode().map_err(operation_error)?;
            if backend.game_ui().map_err(operation_error)? != GameUi::Playing
                || !(0..=70).contains(&mode)
                || matches!(mode, 43 | 50)
            {
                return Err(RuntimeError::new("this native mode has no supported battle completion"));
            }
            backend.validate_level_completion().map_err(operation_error)?;
            if backend.next_survival_stage_countdown().map_err(operation_error)? != 0 {
                return Err(RuntimeError::new("native completion is already pending"));
            }
            Ok((
                backend
                    .world_identity()
                    .map_err(operation_error)?
                    .ok_or_else(|| RuntimeError::new("no world"))?,
                mode,
                backend.survival_stage().map_err(operation_error)?,
                Duration::from_millis(u64::from(backend.native_frame_duration_ms().map_err(operation_error)?)),
            ))
        })?;
        Ok(Self {
            state: Rc::new(RefCell::new(State {
                world,
                mode,
                initial_stage,
                target_stage: None,
                sun,
                duration,
                deadline: None,
                phase: Phase::Prepared,
                started: false,
                opened_pause: false,
            })),
            cleanup: None,
            source: rsvz_schedule::module::current_module(),
        })
    }

    /// Call at the existing outer dispatch boundary, including while the ordinary
    /// pause dialog is open. Returns true only after the native round transition.
    pub fn advance(&mut self) -> RuntimeResult<bool>
    where
        CurrentBackend: NativeLevelFlowBackend
            + NativeGameModeBackend
            + GameUiBackend
            + WorldIdentityBackend
            + ClockBackend
            + AdvancedPauseBackend
            + SunWriteBackend
            + ZombiePresenceBackend
            + ZombieStatusWriteBackend,
    {
        if rsvz_schedule::module::current_module() != self.source {
            return Err(RuntimeError::new("level completion belongs to another source"));
        }
        let mut state = self.state.borrow_mut();
        let (world, ui) = with_backend(|backend| -> RuntimeResult<_> {
            Ok((
                backend.world_identity().map_err(operation_error)?,
                backend.game_ui().map_err(operation_error)?,
            ))
        })?;
        if with_backend(|backend| backend.native_game_mode()).map_err(operation_error)? != state.mode {
            return Err(RuntimeError::new("native game mode changed during level completion"));
        }
        let native_boundary = if state.target_stage.is_none() && matches!(state.phase, Phase::Boundary) {
            if matches!(ui, GameUi::Award | GameUi::Credit) {
                true
            } else if world != Some(state.world)
                && world.is_some()
                && matches!(ui, GameUi::Playing | GameUi::LevelIntro)
            {
                with_backend(|backend| {
                    Ok::<_, RuntimeError>(
                        state.initial_stage.checked_add(1) == Some(backend.survival_stage().map_err(operation_error)?)
                            && backend.next_survival_stage_countdown().map_err(operation_error)? == 0,
                    )
                })?
            } else {
                false
            }
        } else {
            false
        };
        if state.target_stage.is_none() && matches!(state.phase, Phase::Boundary) && native_boundary {
            state.phase = Phase::Done;
            if let Some(hook) = self.cleanup.take() {
                crate::state_hook::remove_state_hook(hook);
            }
            return Ok(true);
        }
        if world != Some(state.world) {
            return Err(RuntimeError::new("world changed during level completion"));
        }
        if state.deadline.is_some_and(|deadline| Instant::now() > deadline) {
            return Err(RuntimeError::new(
                "level completion timed out; committed native effects are retained",
            ));
        }
        let clock = with_backend(|backend| backend.clock()).map_err(operation_error)?;
        match state.phase {
            Phase::Done => return Ok(true),
            Phase::Cancelled => {
                return Err(RuntimeError::new(
                    "level completion was cancelled; native effects are not rolled back",
                ));
            }
            Phase::Boundary => {
                let complete = with_backend(|backend| -> RuntimeResult<bool> {
                    let stage = backend.survival_stage().map_err(operation_error)?;
                    Ok(match state.target_stage {
                        Some(target) => {
                            stage == target
                                && backend.next_survival_stage_countdown().map_err(operation_error)? == 0
                                && backend.survival_flags_completed().map_err(operation_error)? == target * 2
                        }
                        None => {
                            state.initial_stage.checked_add(1) == Some(stage)
                                && backend.next_survival_stage_countdown().map_err(operation_error)? == 0
                        }
                    })
                })?;
                if complete && matches!(ui, GameUi::Playing | GameUi::LevelIntro) {
                    state.phase = Phase::Done;
                    if let Some(hook) = self.cleanup.take() {
                        crate::state_hook::remove_state_hook(hook);
                    }
                    return Ok(true);
                }
            }
            _ if ui != GameUi::Playing => {
                return Err(RuntimeError::new(
                    "battle ended before level-completion sequence finished",
                ));
            }
            Phase::Prepared => {
                if with_backend(|backend| backend.advanced_pause_active()) {
                    return Err(RuntimeError::new("release advanced pause before completing a round"));
                }
                // Retain unfinished cleanup even if the caller drops the plan.
                // Successful completion/cancellation removes this hook promptly.
                let cleanup_state = Rc::clone(&self.state);
                self.cleanup = Some(crate::state_hook::register_native_cleanup(i32::MAX, move || {
                    cancel(&mut cleanup_state.borrow_mut())
                }));
                state.started = true;
                state.deadline = Some(Instant::now() + Duration::from_secs(600));
                with_exclusive_backend(|backend| backend.set_pause_dialog(false)).map_err(operation_error)?;
                if with_backend(|backend| backend.ordinary_paused()).map_err(operation_error)? {
                    return Err(RuntimeError::new("another native dialog prevents resuming the battle"));
                }
                state.phase = Phase::Align(clock.wrapping_add(75 - ((i64::from(clock) + 500).rem_euclid(75) as i32)));
            }
            Phase::Align(target) if clock.wrapping_sub(target) >= 0 => {
                with_exclusive_backend(|backend| backend.set_pause_dialog(true)).map_err(operation_error)?;
                state.opened_pause = with_backend(|backend| backend.ordinary_paused()).map_err(operation_error)?;
                state.phase = Phase::Fade(Instant::now() + state.duration);
            }
            Phase::Fade(deadline) if Instant::now() >= deadline => {
                with_exclusive_backend(|backend| backend.begin_level_completion()).map_err(operation_error)?;
                state.phase = Phase::Kill(Instant::now() + state.duration);
            }
            Phase::Kill(deadline) if Instant::now() >= deadline => {
                with_backend(|backend| -> RuntimeResult<()> {
                    for zombie in backend.present_zombies().map_err(operation_error)? {
                        backend
                            .set_zombie_death_state(zombie, ZombieDeathState::Mowered)
                            .map_err(operation_error)?;
                    }
                    Ok(())
                })?;
                state.phase = Phase::Resume(Instant::now() + state.duration);
            }
            Phase::Resume(deadline) if Instant::now() >= deadline => {
                with_exclusive_backend(|backend| backend.set_pause_dialog(false)).map_err(operation_error)?;
                state.opened_pause = false;
                if with_backend(|backend| backend.ordinary_paused()).map_err(operation_error)? {
                    return Err(RuntimeError::new("native pause remained active"));
                }
                state.phase = Phase::Values(clock.wrapping_add(10));
            }
            Phase::Values(target) if clock.wrapping_sub(target) >= 0 => {
                // FadeOutLevel schedules the native 500-frame transition. Its
                // final increment supplies the last stage, including target zero.
                with_backend(|backend| {
                    if let Some(target) = state.target_stage {
                        backend.set_survival_stage(target - 1).map_err(operation_error)?;
                    }
                    backend.set_sun(state.sun).map_err(operation_error)
                })?;
                state.phase = Phase::Boundary;
            }
            _ => {}
        }
        Ok(false)
    }
    pub fn cancel(&mut self) -> RuntimeResult<()>
    where
        CurrentBackend: NativeLevelFlowBackend + WorldIdentityBackend,
    {
        if rsvz_schedule::module::current_module() != self.source {
            return Err(RuntimeError::new("level completion belongs to another source"));
        }
        cancel(&mut self.state.borrow_mut())?;
        if let Some(hook) = self.cleanup.take() {
            crate::state_hook::remove_state_hook(hook);
        }
        Ok(())
    }
}

fn cancel(state: &mut State) -> RuntimeResult<()>
where
    CurrentBackend: NativeLevelFlowBackend + WorldIdentityBackend,
{
    if matches!(state.phase, Phase::Done | Phase::Cancelled) {
        return Ok(());
    }
    if state.opened_pause
        && with_backend(|backend| backend.world_identity()).map_err(operation_error)? == Some(state.world)
    {
        with_exclusive_backend(|backend| backend.set_pause_dialog(false)).map_err(operation_error)?;
        state.opened_pause = false;
    }
    state.phase = Phase::Cancelled;
    Ok(())
}
