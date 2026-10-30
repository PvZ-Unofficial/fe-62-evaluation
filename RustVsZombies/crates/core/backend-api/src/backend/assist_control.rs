//! Small native controls used by explicit tool actions and ordinary scripts.
use crate::Backend;
use rsvz_model::PixelPos;
pub trait BoardViewBackend: Backend {
    fn board_view_position(&self) -> Result<PixelPos, Self::Error>;
    fn set_board_view_position(&self, pos: PixelPos) -> Result<(), Self::Error>;
}
pub trait AnimationSkipBackend: Backend {
    fn animation_skip_enabled(&self) -> Result<bool, Self::Error>;
    fn set_animation_skip_enabled(&self, enabled: bool) -> Result<(), Self::Error>;
}
pub trait HouseEntryPauseBackend: Backend {
    fn house_entry_pause_enabled(&self) -> Result<bool, Self::Error>;
    fn set_house_entry_pause_enabled(&self, enabled: bool) -> Result<(), Self::Error>;
    /// Takes the native prevented-loss occurrence; no callback runs at that point.
    fn take_house_entry(&self) -> Result<bool, Self::Error>;
}
pub trait GameMusicBackend: Backend {
    fn ensure_music_tune(&self, tune: i32) -> Result<(), Self::Error>;
}

pub trait NativeFastModeBackend: Backend {
    /// Clear the game's native fast-motion cheat flag; independent of rsvz speed.
    fn clear_native_fast_mode(&self) -> Result<(), Self::Error>;
}
