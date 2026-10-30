use crate::{PortableBackend, PortableBackendError, Result};
use rsvz_backend_api::NativeLevelFlowBackend;

impl rsvz_backend_api::NativeNavigationBackend for PortableBackend {
    fn save_current_game(&mut self) -> Result<()> {
        // SAFETY: exclusive backend receiver covers the native save operation.
        unsafe { pvzp_rs::save_current_game() }.map_err(Into::into)
    }
    fn enter_native_mode(&mut self, mode: i32) -> Result<()> {
        // SAFETY: exclusive receiver excludes all pre-entry entity borrows.
        unsafe { pvzp_rs::enter_native_mode(mode) }.map_err(Into::into)
    }
    fn continue_saved_game_if_present(&mut self) -> Result<bool> {
        pvzp_rs::click_continue_dialog_if_present().map_err(Into::into)
    }
}

impl rsvz_backend_api::HintCaptionBackend for PortableBackend {
    fn show_hint_caption(&mut self, caption: rsvz_model::HintCaption<'_>) -> Result<()> {
        // SAFETY: exclusive backend receiver bounds native text-animation updates.
        unsafe { self.world()?.show_hint(caption.text(), caption.duration()) }.map_err(Into::into)
    }
}
impl rsvz_backend_api::NativeCaptionReadBackend for PortableBackend {
    fn caption_bytes(&self, output: &mut [u8; 128]) -> Result<usize> {
        self.world()?.caption_bytes(output).map_err(Into::into)
    }
    fn caption_duration(&self) -> Result<i32> {
        self.world()?.caption_duration().map_err(Into::into)
    }
    fn caption_style(&self) -> Result<i32> {
        self.world()?.caption_style().map_err(Into::into)
    }
}

impl NativeLevelFlowBackend for PortableBackend {
    fn validate_level_completion(&self) -> Result<()> {
        self.world()?.validate_level_completion().map_err(Into::into)
    }
    fn ordinary_paused(&self) -> Result<bool> {
        Ok(self.world()?.paused())
    }
    fn set_pause_dialog(&mut self, enabled: bool) -> Result<()> {
        // SAFETY: exclusive backend access covers the native dialog callback.
        unsafe { self.world()?.set_pause_dialog(enabled) }.map_err(Into::into)
    }
    fn begin_level_completion(&mut self) -> Result<()> {
        // SAFETY: exclusive backend access; no entity borrow survives the native call.
        unsafe { self.world()?.begin_level_completion() }.map_err(Into::into)
    }
    fn survival_stage(&self) -> Result<i32> {
        Ok(self.world()?.survival_stage())
    }
    fn survival_flags_completed(&self) -> Result<i32> {
        if self.world()?.total_waves() <= 0 {
            return Err(PortableBackendError::Unsupported(
                "flag count requires initialized waves",
            ));
        }
        Ok(self.world()?.survival_flags_completed())
    }
    fn set_survival_stage(&self, stage: i32) -> Result<()> {
        self.world()?.set_survival_stage(stage).map_err(Into::into)
    }
    fn next_survival_stage_countdown(&self) -> Result<i32> {
        Ok(self.world()?.next_survival_stage_counter())
    }
}

impl rsvz_backend_api::FrameTimingBackend for PortableBackend {
    fn native_frame_duration_ms(&self) -> Result<u32> {
        u32::try_from(pvzp_rs::frame_duration_ms()?)
            .ok()
            .filter(|value| *value > 0)
            .ok_or(PortableBackendError::Unsupported("invalid native frame duration"))
    }
}
