use crate::{
    error::{Pvz1051Error, Result},
    raw::layout as ptrs,
    runtime::Pvz1051Backend,
};
use rsvz_backend_api::{AnimationSkipBackend, BoardViewBackend, GameMusicBackend, HouseEntryPauseBackend};
use rsvz_model::PixelPos;
impl BoardViewBackend for Pvz1051Backend {
    fn board_view_position(&self) -> Result<PixelPos> {
        // SAFETY: live Board widget x/y at +0x30/+0x34.
        let (x, y) = unsafe { ptrs::Board::view_position(self.board()?.as_ptr()) };
        Ok(PixelPos { x, y })
    }
    fn set_board_view_position(&self, p: PixelPos) -> Result<()> {
        // SAFETY: writes the native widget origin, no object lifecycle change.
        unsafe { ptrs::Board::set_view_position(self.board()?.as_ptr(), p.x, p.y) };
        Ok(())
    }
}
impl AnimationSkipBackend for Pvz1051Backend {
    fn animation_skip_enabled(&self) -> Result<bool> {
        // SAFETY: live App, native mTodCheatKeys field.
        Ok(unsafe { ptrs::LawnApp::animation_skip(self.app().as_ptr()) })
    }
    fn set_animation_skip_enabled(&self, v: bool) -> Result<()> {
        // SAFETY: native cheat-key flag used by cutscenes and chooser.
        unsafe { ptrs::LawnApp::set_animation_skip(self.app().as_ptr(), v) };
        Ok(())
    }
}
impl HouseEntryPauseBackend for Pvz1051Backend {
    fn house_entry_pause_enabled(&self) -> Result<bool> {
        Ok(crate::patches::house_entry::enabled())
    }
    fn set_house_entry_pause_enabled(&self, v: bool) -> Result<()> {
        crate::patches::house_entry::set(v)
    }
    fn take_house_entry(&self) -> Result<bool> {
        Ok(crate::patches::house_entry::take())
    }
}
impl GameMusicBackend for Pvz1051Backend {
    fn ensure_music_tune(&self, tune: i32) -> Result<()> {
        if !(1..=13).contains(&tune) {
            return Err(Pvz1051Error::OperationRejected("invalid scene music tune"));
        }
        // SAFETY: native Music owner checked; EAX/EDI/no-stack proven at 0x45B750.
        unsafe {
            let music = ptrs::LawnApp::music(self.app().as_ptr());
            if music.is_null() {
                return Err(Pvz1051Error::OperationRejected("no music owner"));
            }
            crate::raw::abi::lawn_app_play_music(tune);
        }
        Ok(())
    }
}

impl rsvz_backend_api::NativeFastModeBackend for Pvz1051Backend {
    fn clear_native_fast_mode(&self) -> Result<()> {
        // SAFETY: original gFastMo static byte, read by LawnApp::UpdateFrames;
        // A-TAS ResetGame clears exactly this flag, not the independent gSlowMo.
        unsafe { core::ptr::write_volatile(0x6a9eab as *mut u8, 0) };
        Ok(())
    }
}
