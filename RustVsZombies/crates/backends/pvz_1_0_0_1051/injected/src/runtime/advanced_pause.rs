//! 1051 advanced-pause runtime.

use rsvz_model::AdvancedPauseOptions;

use super::Pvz1051Backend;
use super::backend::{current_app, current_board};
use crate::error::{Pvz1051Error, Result};
use crate::patches::{PatchSet, advanced_pause_patch};
use crate::raw::{abi as asm, layout as ptrs};

#[derive(Default)]
pub(crate) struct AdvancedPauseRuntime {
    active: Option<ActiveAdvancedPause>,
}

struct ActiveAdvancedPause {
    options: AdvancedPauseOptions,
    patch: PatchSet,
}

impl AdvancedPauseRuntime {
    pub(crate) const fn new() -> Self {
        Self { active: None }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn options(&self) -> AdvancedPauseOptions {
        self.active.as_ref().map(|active| active.options).unwrap_or_default()
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool, options: AdvancedPauseOptions) -> Result<()> {
        match (enabled, self.active.as_mut()) {
            (true, Some(active)) => {
                active.options = options;
                Ok(())
            }
            (true, None) => {
                let patch = advanced_pause_patch()?;
                self.active = Some(ActiveAdvancedPause { options, patch });
                Ok(())
            }
            (false, Some(_active)) => self.restore(),
            (false, None) => Ok(()),
        }
    }

    pub(crate) fn restore(&mut self) -> Result<()> {
        let Some(active) = self.active.as_mut() else {
            return Ok(());
        };
        active.patch.restore()?;
        self.active = None;
        Ok(())
    }

    pub(crate) fn update_frame(&mut self) -> Result<()> {
        let Some(active) = self.active.as_ref() else {
            return Ok(());
        };
        update_advanced_pause_frame(active.options)
    }
}

pub(crate) fn update_advanced_pause_frame(options: AdvancedPauseOptions) -> Result<()> {
    let app = current_app()?;
    // Native input can leave the battle before the next outer Rust dispatch
    // releases its world-bound pause request. Never adjust counters in a menu/chooser.
    if unsafe { ptrs::LawnApp::game_ui(app.as_ptr()) } != 3 {
        return Ok(());
    }
    let board = current_board()?;

    if super::seed_chooser::game_is_paused_or_modal_current()? {
        return Ok(());
    }

    if options.refresh_cursor_preview {
        // SAFETY: `board` is non-null and points to the active Board. Cursor pointers may be null
        // when no card/tool is held; the ABI wrappers are called only when the target object exists.
        if unsafe { !ptrs::Board::cursor_object(board.as_ptr()).is_null() } {
            // SAFETY: cursor object exists on the active board.
            unsafe { asm::cursor_object_update() };
        }
        // SAFETY: same active-board precondition as cursor object.
        if unsafe { !ptrs::Board::cursor_preview(board.as_ptr()).is_null() } {
            // SAFETY: cursor preview exists on the active board.
            unsafe { asm::cursor_preview_update() };
        }
    }

    // SAFETY: Board::Update increments +0x556c at 0x415f82 before the pause jump,
    // and LawnApp advances its counter. Only these two need compensation (AvZ UpdateAdvancedPause).
    // The jump skips the +0x5568 game-clock increment at 0x415b29; decrementing it would rewind time.
    unsafe {
        decrement(ptrs::Board::effect_counter_mut(board.as_ptr()));
        decrement(ptrs::LawnApp::app_counter_mut(app.as_ptr()));
    }

    Ok(())
}

unsafe fn decrement(value: *mut i32) {
    // SAFETY: Caller guarantees `value` points to a live copied PvZ i32 counter.
    unsafe { *value = (*value).saturating_sub(1) };
}

pub(crate) fn ensure_can_enable(backend: &Pvz1051Backend) -> Result<()> {
    backend.ensure_playing().map_err(|error| match error {
        Pvz1051Error::WrongGameUi { .. } | Pvz1051Error::NullBoard => {
            Pvz1051Error::AbiPreconditionFailed("advanced pause requires an active fight board")
        }
        error => error,
    })
}
