use crate::{
    error::{Pvz1051Error, Result},
    raw::{abi, layout as ptrs},
    runtime::Pvz1051Backend,
};
use rsvz_backend_api::{GameUiBackend, NativeLevelFlowBackend, SeedBankEditBackend};
use rsvz_model::GameUi;

impl NativeLevelFlowBackend for Pvz1051Backend {
    fn ordinary_paused(&self) -> Result<bool> {
        // SAFETY: validated live Board and scalar field.
        Ok(unsafe { ptrs::Board::paused(self.board()?.as_ptr()) })
    }
    fn set_pause_dialog(&mut self, enabled: bool) -> Result<()> {
        if self.game_ui()? != GameUi::Playing {
            return Err(Pvz1051Error::OperationRejected("pause dialog requires a battle"));
        }
        if self.ordinary_paused()? == enabled {
            return Ok(());
        }
        // SAFETY: exclusive live battle access. Native KeyDown uses the same
        // pause sample at 0x6A71A4; 2019 is Resume for DIALOG_PAUSED (19).
        unsafe {
            if enabled {
                abi::lawn_app_play_sample(std::ptr::read_unaligned(0x6a71a4 as *const i32));
                abi::lawn_app_do_pause_dialog();
            } else {
                abi::lawn_app_button_depress(2019);
            }
        }
        Ok(())
    }
    fn begin_level_completion(&mut self) -> Result<()> {
        self.validate_level_completion()?;
        let board = self.board()?;
        // SAFETY: exclusive current Board; verified ECX ABI and native call site.
        unsafe { abi::board_fade_out_level(board.as_ptr()) };
        Ok(())
    }
    fn validate_level_completion(&self) -> Result<()> {
        if self.game_ui()? != GameUi::Playing {
            return Err(Pvz1051Error::OperationRejected("level completion requires a battle"));
        }
        // Native SeedChooser construction indexes ChosenSeeds by every active
        // packet type during repick. -1 corrupts its buttons at offsets 0x94/0x98.
        if (1..=15).contains(&self.raw_game_mode()?) {
            for index in 0..usize::from(self.seed_packet_count()?) {
                let slot = rsvz_model::SeedSlot::new(index).expect("native packet count is bounded");
                let kind = self.seed_packet_kind(slot)?;
                if !(0..=52).contains(&kind) || kind == 48 && !(0..=52).contains(&self.seed_packet_imitater_kind(slot)?)
                {
                    return Err(Pvz1051Error::OperationRejected(
                        "active seed packet is invalid for survival repick",
                    ));
                }
            }
        }
        Ok(())
    }
    fn survival_stage(&self) -> Result<i32> {
        // SAFETY: a live Board owns a live Challenge; reject an absent one.
        let challenge = unsafe { ptrs::Board::challenge(self.board()?.as_ptr()) };
        if challenge.is_null() {
            return Err(Pvz1051Error::OperationRejected("missing challenge"));
        }
        // SAFETY: Challenge validated above, verified +0x6C scalar.
        Ok(unsafe { ptrs::Challenge::endless_rounds(challenge) })
    }
    fn survival_flags_completed(&self) -> Result<i32> {
        let board = self.board()?;
        // Adventure's native divisor can equal NumWaves while constructing Board.
        if unsafe { ptrs::Board::num_waves(board.as_ptr()) } <= 0 {
            return Err(Pvz1051Error::OperationRejected("flag count requires initialized waves"));
        }
        // SAFETY: live Board; ESI ABI and returned EAX verified at native call sites.
        Ok(unsafe { abi::board_survival_flags_completed(board.as_ptr()) })
    }
    fn set_survival_stage(&self, stage: i32) -> Result<()> {
        // -1 is used by native transition setup. Higher values must leave native
        // stage*20 difficulty/flag arithmetic representable.
        if !(-1..=(i32::MAX / 2 - 110) / 20).contains(&stage) {
            return Err(Pvz1051Error::OperationRejected(
                "survival stage exceeds native arithmetic range",
            ));
        }
        // SAFETY: current Board owns this Challenge; scalar write does not reclaim objects.
        let challenge = unsafe { ptrs::Board::challenge(self.board()?.as_ptr()) };
        if challenge.is_null() {
            return Err(Pvz1051Error::OperationRejected("missing challenge"));
        }
        unsafe { std::ptr::write_unaligned(ptrs::Challenge::endless_rounds_mut(challenge), stage) };
        Ok(())
    }
    fn next_survival_stage_countdown(&self) -> Result<i32> {
        // SAFETY: current Board and verified native countdown field.
        Ok(unsafe { ptrs::Board::next_survival_stage_counter(self.board()?.as_ptr()) })
    }
}

impl rsvz_backend_api::FrameTimingBackend for Pvz1051Backend {
    fn native_frame_duration_ms(&self) -> Result<u32> {
        // SAFETY: current live App; verified native scalar offset.
        let value = unsafe { ptrs::LawnApp::frame_duration_ms(self.app().as_ptr()) };
        u32::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .ok_or(Pvz1051Error::OperationRejected("invalid native frame duration"))
    }
}
