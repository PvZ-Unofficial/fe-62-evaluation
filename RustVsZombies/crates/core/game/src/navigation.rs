//! Saved-game navigation, advanced only at the outer dispatch boundary.
//! The caller chooses when to navigate and owns the plan; no script is stopped,
//! restarted, or assigned a different reload policy here.
use crate::{
    access::{operation_error, with_backend, with_exclusive_backend},
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::{
    AdvancedPauseBackend, BoardReadinessBackend, GameUiBackend, MainMenuBackend, NativeGameModeBackend,
    NativeNavigationBackend, WorldIdentityBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{GameUi, WorldId};
use rsvz_schedule::{
    module::{ModuleSlot, current_module},
    state_hook::StateHookHandle,
};
use std::{
    cell::Cell,
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

thread_local! {
    static ACTIVE: std::cell::RefCell<Weak<Cell<bool>>> = const { std::cell::RefCell::new(Weak::new()) };
    static OWNER: Cell<Option<ModuleSlot>> = const { Cell::new(None) };
}
pub(crate) fn owned_by_current() -> bool {
    opening_blocked() && OWNER.get() == Some(current_module())
}

/// An explicit native navigation temporarily excludes automatic Opening. This
/// does not change either source's saved auto-enter preference or reload mode.
pub(crate) fn opening_blocked() -> bool {
    ACTIVE.with_borrow(|active| active.upgrade().is_some_and(|active| active.get()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Save,
    Back,
    Enter(u8),
    Continue(u8),
    Ready,
    Done,
}

/// Save, return to the selector, re-enter the same native mode, and continue its
/// save if present. Drive `advance` once per outer dispatch (including menus),
/// as a service; ordinary battle-only callbacks cannot drive menu transitions.
/// Dropping/cancelling leaves the game at its current native screen.
pub struct SavedGameReentry {
    original_world: WorldId,
    mode: i32,
    source: ModuleSlot,
    phase: Phase,
    started: Option<Instant>,
    active: Rc<Cell<bool>>,
    cleanup: Option<StateHookHandle>,
}

impl SavedGameReentry {
    pub fn prepare() -> RuntimeResult<Self>
    where
        CurrentBackend: GameUiBackend + NativeGameModeBackend + WorldIdentityBackend,
    {
        if opening_blocked() {
            return Err(RuntimeError::new("another native navigation is pending"));
        }
        let (original_world, mode) = with_backend(|backend| {
            if backend.game_ui().map_err(operation_error)? != GameUi::Playing {
                return Err(RuntimeError::new("saved-game reentry requires a battle"));
            }
            let mode = backend.native_game_mode().map_err(operation_error)?;
            if !(0..=73).contains(&mode) {
                return Err(RuntimeError::new("native game mode is outside 0..=73"));
            }
            let world = backend
                .world_identity()
                .map_err(operation_error)?
                .ok_or_else(|| RuntimeError::new("no current world"))?;
            Ok((world, mode))
        })?;
        let active = Rc::new(Cell::new(true));
        let cleanup_active = Rc::clone(&active);
        let cleanup = crate::state_hook::register_native_cleanup(i32::MAX, move || {
            cleanup_active.set(false);
            Ok(())
        });
        ACTIVE.with_borrow_mut(|current| *current = Rc::downgrade(&active));
        OWNER.set(Some(current_module()));
        Ok(Self {
            original_world,
            mode,
            source: current_module(),
            phase: Phase::Save,
            started: None,
            active,
            cleanup: Some(cleanup),
        })
    }

    pub fn world(&self) -> WorldId {
        self.original_world
    }
    pub fn started(&self) -> bool {
        self.started.is_some()
    }

    pub fn cancel(&mut self) {
        self.active.set(false);
        if let Some(cleanup) = self.cleanup.take() {
            rsvz_schedule::module::with_module(self.source, || crate::state_hook::remove_state_hook(cleanup));
        }
    }

    pub fn advance(&mut self) -> RuntimeResult<bool>
    where
        CurrentBackend: NativeNavigationBackend
            + GameUiBackend
            + NativeGameModeBackend
            + WorldIdentityBackend
            + BoardReadinessBackend
            + AdvancedPauseBackend,
    {
        if current_module() != self.source {
            return Err(RuntimeError::new("navigation belongs to another execution source"));
        }
        if self.phase == Phase::Done {
            return Ok(true);
        }
        if !self.active.get() {
            return Err(RuntimeError::new("navigation was cancelled"));
        }
        if self
            .started
            .is_some_and(|time| time.elapsed() > Duration::from_secs(10))
        {
            self.cancel();
            return Err(RuntimeError::new(
                "native navigation timed out; current screen retained",
            ));
        }
        let ui = with_backend(|backend| backend.game_ui()).map_err(operation_error)?;
        match self.phase {
            Phase::Save | Phase::Back => {
                let world = with_backend(|backend| backend.world_identity()).map_err(operation_error)?;
                if ui != GameUi::Playing || world != Some(self.original_world) {
                    return Err(RuntimeError::new("world changed before returning to the menu"));
                }
                if with_backend(|backend| backend.advanced_pause_active()) {
                    return Err(RuntimeError::new("release advanced pause before navigation"));
                }
                if self.phase == Phase::Save {
                    self.started = Some(Instant::now());
                    with_exclusive_backend(|backend| backend.save_current_game()).map_err(operation_error)?;
                    self.phase = Phase::Back;
                } else {
                    with_exclusive_backend(|backend| backend.back_to_main_menu()).map_err(operation_error)?;
                    self.phase = Phase::Enter(5);
                }
            }
            Phase::Enter(remaining) => {
                if !matches!(ui, GameUi::Menu | GameUi::Challenge | GameUi::Loading) {
                    return Err(RuntimeError::new("native screen changed while waiting to enter"));
                }
                if remaining > 0 {
                    self.phase = Phase::Enter(remaining - 1);
                } else {
                    with_exclusive_backend(|backend| backend.enter_native_mode(self.mode)).map_err(operation_error)?;
                    self.phase = Phase::Continue(3);
                }
            }
            Phase::Continue(remaining) => {
                if remaining > 0 {
                    self.phase = Phase::Continue(remaining - 1);
                } else {
                    with_exclusive_backend(|backend| backend.continue_saved_game_if_present())
                        .map_err(operation_error)?;
                    self.phase = Phase::Ready;
                }
            }
            Phase::Ready => {
                let ready = with_backend(|backend| {
                    let ready = match ui {
                        GameUi::Playing => backend.playing_board_ready(),
                        GameUi::LevelIntro => backend.level_intro_board_ready(),
                        _ => return Ok(false),
                    }
                    .map_err(operation_error)?;
                    if !ready {
                        return Ok(false);
                    }
                    if backend.native_game_mode().map_err(operation_error)? != self.mode {
                        return Err(RuntimeError::new("native navigation entered a different game mode"));
                    }
                    Ok(backend
                        .world_identity()
                        .map_err(operation_error)?
                        .is_some_and(|world| world != self.original_world))
                })?;
                if ready {
                    self.phase = Phase::Done;
                    self.cancel();
                    return Ok(true);
                }
            }
            Phase::Done => unreachable!(),
        }
        Ok(false)
    }
}

impl Drop for SavedGameReentry {
    fn drop(&mut self) {
        self.cancel();
    }
}
