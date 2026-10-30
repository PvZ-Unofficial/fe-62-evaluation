use rsvz_backend_api::backend::{
    CardAppendSelectionBackend, CardSelectionReadBackend, NativeGameModeBackend, SeedChooserControlBackend,
};
use rsvz_model::model::CheckedCardSelection;

use crate::error::Result;
use crate::runtime::Pvz1051Backend;
use crate::{
    error::Pvz1051Error,
    raw::{abi, layout as ptrs},
};

impl SeedChooserControlBackend for Pvz1051Backend {
    fn chooser_readiness(&self) -> Result<rsvz_backend_api::opening::SeedChooserReadiness> {
        self.seed_chooser_readiness()
    }
    fn chooser_card_available(&self, selection: CheckedCardSelection) -> Result<bool> {
        self.seed_chooser()?;
        let profile = unsafe { ptrs::LawnApp::player_info(self.app().as_ptr()) };
        if profile.is_null() {
            return Err(Pvz1051Error::NullUserData);
        }
        let mode = self.native_game_mode()?;
        let available = |kind: i32| {
            // Native chooser's HasSeedType callsite handles Gatling's purchase
            // separately (0x40220B..0x402224). All calls use validated 0..48 kinds.
            unsafe {
                let owned = if kind == 40 {
                    ptrs::PlayerInfo::purchase(profile, 0) > 0
                } else {
                    abi::lawn_app_has_seed_type(kind) != 0
                };
                owned
                    && !(mode == 31 && matches!(kind, 1 | 9 | 41 | 24 | 8))
                    && abi::seed_not_allowed_during_trial(kind) == 0
            }
        };
        Ok(available(selection.packet_kind().code())
            && selection.imitator_target().is_none_or(|kind| available(kind.code())))
    }
    fn chooser_card_locked(&self, index: usize) -> Result<bool> {
        let selection = self.selected_card(index)?;
        let chooser = self.seed_chooser()?;
        // SAFETY: selected_card validates index and fixed-array seed kind.
        Ok(unsafe {
            ptrs::ChosenSeed::picked_by_dave(ptrs::SeedChooserScreen::chosen_seed(
                chooser.as_ptr(),
                selection.packet_kind().code() as usize,
            ))
        })
    }
    fn deselect_card(&self, index: usize) -> Result<()> {
        self.ensure_seed_chooser_ready_for_auto_selection()?;
        let selection = self.selected_card(index)?;
        let chooser = self.seed_chooser()?;
        // SAFETY: native selected-card lookup validated the fixed-array entry;
        // removal never replaces Board or frees the chooser.
        unsafe {
            let chosen =
                ptrs::SeedChooserScreen::chosen_seed(chooser.as_ptr(), selection.packet_kind().code() as usize);
            if ptrs::ChosenSeed::seed_state(chosen) != 1 || ptrs::ChosenSeed::picked_by_dave(chosen) {
                return Err(Pvz1051Error::OperationRejected(
                    "selected card is moving or locked by Crazy Dave",
                ));
            }
            abi::seed_chooser_clicked_seed_in_bank(chosen);
        }
        Ok(())
    }
    fn stop_seed_chooser_fast_forward(&self) -> Result<()> {
        crate::runtime::hook::stop_seed_chooser_fast_forward();
        Ok(())
    }
}

impl CardAppendSelectionBackend for Pvz1051Backend {
    fn select_card(&self, selection: CheckedCardSelection) -> Result<()> {
        self.ensure_level_intro_ready()?;
        let seed_chooser = self.seed_chooser()?;

        match selection.imitator_target() {
            None => {
                // SAFETY: UI state and seed chooser readiness were checked above; the wrapper
                // receives a typed PvZ 1051 seed enum.
                unsafe { crate::ops::seed::choose_card(seed_chooser, selection.packet_kind()) };
            }
            Some(kind) => {
                // SAFETY: UI state and seed chooser readiness were checked above; the wrapper
                // receives a typed PvZ 1051 imitator target enum.
                unsafe { crate::ops::seed::choose_imitator_card(seed_chooser, kind) };
            }
        }
        Ok(())
    }
}
