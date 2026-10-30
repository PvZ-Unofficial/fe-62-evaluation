//! Source-owned assistance controls and explicit board operations.
use crate::{
    access::{operation_error, with_backend},
    control::{self, OwnedRule},
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::*;
use rsvz_current::CurrentBackend;
use rsvz_model::*;
use rsvz_schedule::{
    module::current_module,
    tick::{TickControl, TickHandle, TickLifetime, TickOptions, TickTaskState, with_scheduler},
};
use std::cell::{Cell, RefCell};

pub fn request_animation_skip(value: Option<bool>) -> RuntimeResult<()>
where
    CurrentBackend: AnimationSkipBackend,
{
    control::request_native_rule!(
        bool,
        value,
        animation_skip_enabled,
        set_animation_skip_enabled,
        CONTROL_ANIMATION_SKIP_ENABLED
    )
}
pub fn move_view(delta: PixelPos) -> RuntimeResult<()>
where
    CurrentBackend: BoardViewBackend,
{
    with_backend(|b| {
        let current = b.board_view_position().map_err(operation_error)?;
        let x = current
            .x
            .checked_add(delta.x)
            .ok_or_else(|| RuntimeError::new("view x overflow"))?;
        let y = current
            .y
            .checked_add(delta.y)
            .ok_or_else(|| RuntimeError::new("view y overflow"))?;
        b.set_board_view_position(PixelPos { x, y }).map_err(operation_error)
    })
}
pub fn reset_view() -> RuntimeResult<()>
where
    CurrentBackend: BoardViewBackend,
{
    with_backend(|b| b.set_board_view_position(PixelPos { x: 0, y: 0 })).map_err(operation_error)
}
pub fn restore_scene_music() -> RuntimeResult<()>
where
    CurrentBackend: GameMusicBackend + SceneBackend,
{
    with_backend(|b| {
        let scene = b.scene().map_err(operation_error)?;
        b.ensure_music_tune(scene.code() + 1).map_err(operation_error)
    })
}
thread_local! {static HOUSE:RefCell<OwnedRule<bool>>=const{RefCell::new(OwnedRule::new())};}
std::thread_local! {
    static HOUSE_TASK:Cell<Option<TickHandle>>=const{Cell::new(None)};
}
fn house_read() -> RuntimeResult<bool>
where
    CurrentBackend: HouseEntryPauseBackend,
{
    with_backend(|b| b.house_entry_pause_enabled()).map_err(operation_error)
}
fn house_write(v: bool) -> RuntimeResult<()>
where
    CurrentBackend: HouseEntryPauseBackend,
{
    with_backend(|b| b.set_house_entry_pause_enabled(v)).map_err(operation_error)
}
pub fn request_house_pause(value: Option<bool>) -> RuntimeResult<()>
where
    CurrentBackend: HouseEntryPauseBackend
        + AdvancedPauseBackend
        + WorldIdentityBackend
        + GameUiBackend
        + ClockBackend
        + HintCaptionBackend,
{
    match value {
        Some(v) => control::declare(&HOUSE, v, house_read, house_write)?,
        None => control::release(&HOUSE, house_read, house_write)?,
    }
    if value == Some(true)
        && !HOUSE_TASK
            .get()
            .is_some_and(|h| with_scheduler(|s| s.state(h) != TickTaskState::Stopped))
    {
        let mut observed_world = crate::board::world_id()?;
        HOUSE_TASK.set(Some(crate::tick::spawn(
            TickOptions::any_dispatch()
                .lifetime(TickLifetime::Session)
                .name("house_entry_pause"),
            move |_| {
                let world = crate::board::world_id()?;
                let changed = world != observed_world;
                observed_world = world;
                if HOUSE.with_borrow(|rule| rule.effective_declaration() == Some(true))
                    && with_backend(|b| b.take_house_entry()).map_err(operation_error)?
                    && !changed
                {
                    let pause = || {
                        crate::game_pause::set_with_options(
                            true,
                            AdvancedPauseOptions {
                                draw_mask: false,
                                ..AdvancedPauseOptions::default()
                            },
                        )
                    };
                    pause()?;
                    crate::caption::hint("The Zombies Ate Your Brains!", 1000)?;
                }
                Ok(TickControl::Continue)
            },
        )));
    }
    Ok(())
}

/// One explicit sequential skip. It cannot acquire a physical fast-forward loop
/// already owned by another operation and cleans up only the loop it started.
pub struct SkipWave {
    world: WorldId,
    source: rsvz_schedule::module::ModuleSlot,
    execution: Option<crate::modules::ExecutionId>,
    request: crate::logic::fast_forward::FastForwardRequest,
    running: std::rc::Rc<Cell<bool>>,
    cleanup: rsvz_schedule::state_hook::StateHookHandle,
    done: bool,
}
impl SkipWave {
    pub fn prepare(wave: i32) -> RuntimeResult<Self>
    where
        CurrentBackend:
            FastForwardBackend + WaveTimingBackend + WorldIdentityBackend + SpawnScheduleBackend + GameUiBackend,
    {
        let world = with_backend(|b| {
            if b.game_ui().map_err(operation_error)? != GameUi::Playing
                || wave < 0
                || wave as usize > b.spawn_wave_count().map_err(operation_error)?
            {
                return Err(RuntimeError::new(
                    "skip wave is outside this battle's actual wave count",
                ));
            }
            if b.fast_forward_active() {
                return Err(RuntimeError::new("another fast-forward operation is active"));
            }
            b.world_identity()
                .map_err(operation_error)?
                .ok_or_else(|| RuntimeError::new("no skip world"))
        })?;
        let running = std::rc::Rc::new(Cell::new(false));
        let cleanup_running = std::rc::Rc::clone(&running);
        let cleanup = crate::state_hook::register_native_cleanup(i32::MAX, move || {
            if cleanup_running.get() {
                with_backend(|b| -> RuntimeResult<()> {
                    if b.world_identity().map_err(operation_error)? == Some(world) {
                        b.stop_fast_forward(FastForwardStopReason::RuntimeExit)
                            .map_err(operation_error)?;
                    }
                    Ok(())
                })?;
                cleanup_running.set(false);
            }
            Ok(())
        });
        Ok(Self {
            world,
            source: current_module(),
            execution: crate::modules::current_execution(current_module()),
            request: FastForwardRequest::until(
                RelativeTime::new(Wave(wave), 0),
                FastForwardOptions::basic(),
                crate::timeline::runtime_wave_clocks(),
            ),
            running,
            cleanup,
            done: false,
        })
    }
    pub fn cancel(&mut self) -> RuntimeResult<()>
    where
        CurrentBackend: FastForwardBackend + WorldIdentityBackend,
    {
        if current_module() != self.source {
            return Err(RuntimeError::new("skip belongs to another source"));
        }
        if self.running.get() {
            with_backend(|b| -> RuntimeResult<()> {
                if b.world_identity().map_err(operation_error)? == Some(self.world) {
                    b.stop_fast_forward(FastForwardStopReason::ConditionMet)
                        .map_err(operation_error)?;
                }
                Ok(())
            })?;
        }
        self.running.set(false);
        self.done = true;
        crate::state_hook::remove_state_hook(self.cleanup);
        Ok(())
    }
    pub fn advance(&mut self) -> RuntimeResult<bool>
    where
        CurrentBackend: FastForwardBackend + WaveTimingBackend + WorldIdentityBackend + GameUiBackend,
    {
        use crate::logic::fast_forward::FastForwardRequestExt;
        if self.done {
            return Ok(true);
        }
        if current_module() != self.source || crate::modules::current_execution(self.source) != self.execution {
            return Err(RuntimeError::new("skip belongs to another source"));
        }
        let valid = with_backend(|b| -> RuntimeResult<bool> {
            Ok(b.world_identity().map_err(operation_error)? == Some(self.world)
                && b.game_ui().map_err(operation_error)? == GameUi::Playing)
        })?;
        if !valid {
            self.running.set(false);
            self.done = true;
            crate::state_hook::remove_state_hook(self.cleanup);
            return Err(RuntimeError::new("skip world changed"));
        }
        if !self.request.should_continue(Some(crate::timing::wave_timing())) {
            if self.running.get() {
                with_backend(|b| b.stop_fast_forward(FastForwardStopReason::ConditionMet)).map_err(operation_error)?;
            }
            self.running.set(false);
            self.done = true;
            crate::state_hook::remove_state_hook(self.cleanup);
            return Ok(true);
        }
        if self.running.get() {
            if !with_backend(|b| b.fast_forward_active()) {
                self.running.set(false);
                self.done = true;
                crate::state_hook::remove_state_hook(self.cleanup);
                return Err(RuntimeError::new("skip interrupted by pause or native control"));
            }
        } else {
            with_backend(|b| b.start_fast_forward(FastForwardOptions::basic())).map_err(operation_error)?;
            self.running.set(true);
        }
        Ok(false)
    }
}

crate::control::rule_ledgers! {
    CONTROL_ANIMATION_SKIP_ENABLED: bool,
}
