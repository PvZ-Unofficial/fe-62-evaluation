use rsvz_backend_api::backend::{
    AdvancedPauseBackend, BattleEntryBackend, FastForwardBackend, GameSpeedHintBackend, GameUiBackend, MainMenuBackend,
    SeedChooserFastForwardBackend,
};
use rsvz_model::model::{BattleConfig, GameMode, GameUi, PositiveFiniteF32, SceneKind};
use rsvz_model::{
    AdvancedPauseOptions, FastForwardOptions, FastForwardStopReason, RandomStreamKind, SeedChooserFastForwardOptions,
};

use crate::error::{Pvz1051Error, Result};
use crate::raw::{abi as asm, layout as ptrs};
use crate::runtime::Pvz1051Backend;

const SURVIVAL_ENDLESS_SCENES: &[(SceneKind, i32)] = &[
    (SceneKind::Day, 11),
    (SceneKind::Night, 12),
    (SceneKind::Pool, 13),
    (SceneKind::Fog, 14),
    (SceneKind::Roof, 15),
];

impl rsvz_backend_api::HintCaptionBackend for Pvz1051Backend {
    fn show_hint_caption(&mut self, caption: rsvz_model::HintCaption<'_>) -> Result<()> {
        // SAFETY: current live Board and its native message widget field.
        let widget = unsafe { ptrs::Board::message_widget(self.board()?.as_ptr()) };
        if widget.is_null() {
            return Err(Pvz1051Error::OperationRejected("native message widget is unavailable"));
        }
        let text = caption.text().as_bytes();
        let mut buffer = [0_u8; 128];
        buffer[..text.len()].copy_from_slice(text);
        // A-TAS's borrowed MSVC string view: allocator word, pointer/inline
        // union, byte length, capacity >=16. Native SetLabel only reads this
        // argument and copies it; it must not own or destroy the Rust buffer.
        let mut label = [0_u32; 7];
        label[1] = buffer.as_ptr() as u32;
        label[5] = text.len() as u32;
        label[6] = 128;
        // SAFETY: checked literal length/counter; all borrowed storage stays live
        // for the verified native call. Exclusive access covers animation changes.
        unsafe {
            asm::message_widget_set_label(widget, label.as_ptr(), 7);
            ptrs::MessageWidget::set_duration(widget, caption.duration());
        }
        Ok(())
    }
}

impl rsvz_backend_api::NativeCaptionReadBackend for Pvz1051Backend {
    fn caption_bytes(&self, output: &mut [u8; 128]) -> Result<usize> {
        // SAFETY: scoped Board access; the message has a fixed 128-byte label.
        let widget = unsafe { ptrs::Board::message_widget(self.board()?.as_ptr()) };
        if widget.is_null() {
            return Err(Pvz1051Error::OperationRejected("no native message widget"));
        }
        for (index, byte) in output.iter_mut().enumerate() {
            *byte = unsafe { ptrs::MessageWidget::label_byte(widget, index) };
            if *byte == 0 {
                return Ok(index);
            }
        }
        Err(Pvz1051Error::InvariantViolated("unterminated native caption"))
    }
    fn caption_duration(&self) -> Result<i32> {
        // SAFETY: current non-null message widget and verified scalar offset.
        let widget = unsafe { ptrs::Board::message_widget(self.board()?.as_ptr()) };
        if widget.is_null() {
            return Err(Pvz1051Error::OperationRejected("no native message widget"));
        }
        Ok(unsafe { ptrs::MessageWidget::duration(widget) })
    }
    fn caption_style(&self) -> Result<i32> {
        // SAFETY: current non-null message widget and verified scalar offset.
        let widget = unsafe { ptrs::Board::message_widget(self.board()?.as_ptr()) };
        if widget.is_null() {
            return Err(Pvz1051Error::OperationRejected("no native message widget"));
        }
        Ok(unsafe { ptrs::MessageWidget::style(widget) })
    }
}

impl BattleEntryBackend for Pvz1051Backend {
    fn enter_game(&mut self, config: BattleConfig) -> Result<()> {
        let mode = game_mode_for_battle_config(config)?;
        rsvz_backend_api::NativeNavigationBackend::enter_native_mode(self, mode.0)
    }

    fn start_battle(&mut self) -> Result<()> {
        if let Some(host) = crate::module_link::services() {
            return crate::module_link::checked((host.battle_start)());
        }
        self.ensure_level_intro_ready()?;
        let seed_chooser = self.seed_chooser()?;
        let seed_bank = self.seed_bank()?;
        // SAFETY: `seed_chooser` is non-null and points to the active level-intro seed chooser.
        let selected = unsafe { ptrs::SeedChooserScreen::seeds_in_bank(seed_chooser.as_ptr()) };
        // SAFETY: `seed_bank` is non-null and points to the active Board seed bank.
        let slots = unsafe { ptrs::SeedBank::packet_count(seed_bank.as_ptr()) };
        // SAFETY: `seed_chooser` is non-null and points to the active level-intro seed chooser.
        let flying = unsafe { ptrs::SeedChooserScreen::seeds_in_flight(seed_chooser.as_ptr()) };
        validate_seed_chooser_fill(
            selected,
            slots,
            flying,
            crate::patches::random_locked(RandomStreamKind::Battle),
        )?;

        // Reset keeps a fresh chooser at stage 0 while it is being prepared. Restore the requested
        // stage now so native SetPacketType applies the correct survival initial-cooldown policy.
        crate::impls::reset::restore_pending_world_stage(self)?;

        // SAFETY: The game UI and seed chooser readiness were checked as LevelIntro above.
        // PickRandomSeeds fills missing slots without replacement, lands any flying cards, and
        // closes the chooser. Locked partial selection was rejected above because its rejection
        // sampling loop may otherwise never progress.
        unsafe { asm::seed_chooser_pick_random_seeds() };
        crate::impls::reset::apply_pending_card_cooldowns(seed_bank);

        Ok(())
    }
}

fn validate_seed_chooser_fill(selected: i32, slots: i32, flying: i32, locked: bool) -> Result<()> {
    if selected > slots {
        return Err(Pvz1051Error::SeedChooserCardsNotReady {
            selected,
            slots,
            flying,
        });
    }
    if selected < slots && locked {
        return Err(Pvz1051Error::KindUnavailable(
            "partial card selection with locked randomness",
        ));
    }
    Ok(())
}

fn game_mode_for_battle_config(config: BattleConfig) -> Result<GameMode> {
    match config {
        BattleConfig::Endless(config) => survival_endless_game_mode(config.scene),
    }
}

fn survival_endless_game_mode(scene: SceneKind) -> Result<GameMode> {
    SURVIVAL_ENDLESS_SCENES
        .iter()
        .find_map(|(survival_scene, mode)| (*survival_scene == scene).then_some(GameMode(*mode)))
        .ok_or(Pvz1051Error::KindUnavailable(
            "survival endless is only available on adventure battle scenes",
        ))
}

impl MainMenuBackend for Pvz1051Backend {
    fn back_to_main_menu(&mut self) -> Result<()> {
        self.ensure_playing()?;
        // SAFETY: The game UI and board readiness were checked as Playing above.
        unsafe { crate::raw::abi::lawn_app_do_back_to_main() };
        Ok(())
    }
}

impl GameSpeedHintBackend for Pvz1051Backend {
    fn game_speed_multiplier(&self) -> Result<f64> {
        // SAFETY: current token owns App lifetime; +0x490 is the native double multiplier.
        Ok(unsafe { ptrs::LawnApp::update_multiplier(self.app().as_ptr()) })
    }
    fn set_game_speed_multiplier(&self, multiplier: f64) -> Result<()> {
        if !multiplier.is_finite() || multiplier < 0.0 {
            return Err(Pvz1051Error::InvalidGameSpeed(multiplier as f32));
        }
        crate::runtime::hook::set_game_speed_multiplier(multiplier)
    }
    fn set_game_speed(&self, speed: PositiveFiniteF32) -> Result<()> {
        let speed = speed.get();
        if !(0.05..=100.0).contains(&speed) {
            return Err(Pvz1051Error::InvalidGameSpeed(speed));
        }
        crate::runtime::hook::set_game_speed(speed)
    }
}

impl FastForwardBackend for Pvz1051Backend {
    fn start_fast_forward(&self, options: FastForwardOptions) -> Result<()> {
        crate::runtime::hook::start_fast_forward(options)
    }

    fn stop_fast_forward(&self, reason: FastForwardStopReason) -> Result<()> {
        crate::runtime::hook::stop_fast_forward(reason)
    }

    fn fast_forward_active(&self) -> bool {
        crate::runtime::hook::fast_forward_active()
    }
}

impl SeedChooserFastForwardBackend for Pvz1051Backend {
    fn request_seed_chooser_fast_forward(&self, options: SeedChooserFastForwardOptions) -> Result<()> {
        crate::runtime::hook::request_seed_chooser_fast_forward(options)
    }
}

impl AdvancedPauseBackend for Pvz1051Backend {
    fn set_advanced_pause_with_options(&self, enabled: bool, options: AdvancedPauseOptions) -> Result<()> {
        crate::runtime::hook::set_advanced_pause(self, enabled, options)
    }

    fn advanced_pause_options(&self) -> AdvancedPauseOptions {
        crate::runtime::hook::advanced_pause_options()
    }

    fn advanced_pause_active(&self) -> bool {
        crate::runtime::hook::advanced_pause_active()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn partial_seed_fill_rejects_locked_rng_but_accepts_seeded_rng() {
        assert!(validate_seed_chooser_fill(1, 10, 1, false).is_ok());
        assert!(matches!(
            validate_seed_chooser_fill(1, 10, 1, true),
            Err(Pvz1051Error::KindUnavailable(
                "partial card selection with locked randomness"
            ))
        ));
        assert!(validate_seed_chooser_fill(10, 10, 1, true).is_ok());
    }
}

impl rsvz_backend_api::WindowRulesBackend for Pvz1051Backend {
    fn window_left_aligned(&self) -> Result<bool> {
        Ok(crate::patches::window_view::left_aligned())
    }
    fn set_window_left_aligned(&self, enabled: bool) -> Result<()> {
        crate::patches::window_view::set_left_aligned(enabled)
    }
    fn window_expanded(&self) -> Result<bool> {
        Ok(crate::patches::window_view::enabled())
    }
    fn set_window_expanded(&self, enabled: bool) -> Result<()> {
        crate::patches::window_view::set(enabled)
    }
    fn window_scale(&self) -> Result<f64> {
        crate::patches::window_view::scale()
    }
    fn set_window_scale(&self, scale: f64) -> Result<()> {
        crate::patches::window_view::set_scale(scale)
    }
    fn window_view_width(&self) -> Result<i32> {
        Ok(crate::patches::window_view::width())
    }
    fn window_resizable(&self) -> Result<bool> {
        Ok(crate::patches::window_resize::enabled())
    }
    fn set_window_resizable(&self, enabled: bool) -> Result<()> {
        crate::patches::window_resize::set(enabled)
    }
    fn background_run_rule(&self) -> Result<rsvz_model::BackgroundRunRule> {
        use rsvz_model::BackgroundRunRule::{Disabled, Enabled};
        Ok(if crate::patches::leases::BoolPatchId::BackgroundRunning.enabled()? {
            Enabled
        } else {
            Disabled
        })
    }
    fn set_background_run_rule(&self, rule: rsvz_model::BackgroundRunRule) -> Result<()> {
        crate::patches::leases::set_bool_patch(
            crate::patches::leases::BoolPatchId::BackgroundRunning,
            rule == rsvz_model::BackgroundRunRule::Enabled,
        )
    }
    fn pause_dialog_disabled(&self) -> Result<bool> {
        crate::patches::leases::BoolPatchId::PauseDialogDisabled.enabled()
    }
    fn set_pause_dialog_disabled(&self, enabled: bool) -> Result<()> {
        crate::patches::leases::set_bool_patch(crate::patches::leases::BoolPatchId::PauseDialogDisabled, enabled)
    }
}

impl rsvz_backend_api::NativeNavigationBackend for Pvz1051Backend {
    fn save_current_game(&mut self) -> Result<()> {
        self.ensure_playing()?;
        let board = self.board()?;
        // SAFETY: exclusive current battle; verified native SaveData entry and stack ABI.
        unsafe { asm::board_try_save_game(board.as_ptr()) };
        Ok(())
    }
    fn enter_native_mode(&mut self, mode: i32) -> Result<()> {
        if !(0..=73).contains(&mode) {
            return Err(Pvz1051Error::OperationRejected("invalid native game mode"));
        }
        let actual = self.game_ui()?;
        match actual {
            GameUi::Loading => {
                // SAFETY: the decoded UI is Loading, matching PvZ's loading-complete path before
                // the game selector is discarded.
                unsafe { crate::raw::abi::lawn_app_loading_completed() };
                // SAFETY: the decoded UI started in a menu-like startup state; removing the game
                // selector mirrors the prior enter-game recipe before opening the requested mode.
                unsafe { crate::raw::abi::lawn_app_kill_game_selector() };
            }
            GameUi::Menu => {
                // SAFETY: the decoded UI is Menu, so the selector exists on the normal menu path.
                unsafe { crate::raw::abi::lawn_app_kill_game_selector() };
            }
            GameUi::Challenge => {
                // SAFETY: the decoded UI is Challenge, so PvZ's challenge-screen cleanup is the
                // matching transition before opening the requested mode.
                unsafe { crate::raw::abi::lawn_app_kill_challenge_screen() };
            }
            _ => {
                return Err(Pvz1051Error::WrongGameUi {
                    expected: GameUi::Menu,
                    actual,
                });
            }
        }
        // SAFETY: active level UIs were rejected above; this operation resumes a saved
        // battle when one exists, matching the native saved-game flag of 1.
        unsafe { crate::raw::abi::lawn_app_pre_new_game(1, mode) };
        Ok(())
    }
    fn continue_saved_game_if_present(&mut self) -> Result<bool> {
        self.click_continue_dialog_if_present()
    }
}
