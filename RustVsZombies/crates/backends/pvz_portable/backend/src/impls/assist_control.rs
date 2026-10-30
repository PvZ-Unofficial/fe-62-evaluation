use crate::{PortableBackend, Result};
use rsvz_backend_api::{AnimationSkipBackend, BoardViewBackend, GameMusicBackend, HouseEntryPauseBackend};
use rsvz_model::PixelPos;
impl BoardViewBackend for PortableBackend {
    fn board_view_position(&self) -> Result<PixelPos> {
        let (x, y) = self.world()?.board_view()?;
        Ok(PixelPos { x, y })
    }
    fn set_board_view_position(&self, p: PixelPos) -> Result<()> {
        self.world()?.set_board_view(p.x, p.y).map_err(Into::into)
    }
}
impl AnimationSkipBackend for PortableBackend {
    fn animation_skip_enabled(&self) -> Result<bool> {
        pvzp_rs::animation_skip().map_err(Into::into)
    }
    fn set_animation_skip_enabled(&self, v: bool) -> Result<()> {
        pvzp_rs::set_animation_skip(v).map_err(Into::into)
    }
}
impl HouseEntryPauseBackend for PortableBackend {
    fn house_entry_pause_enabled(&self) -> Result<bool> {
        pvzp_rs::house_pause().map_err(Into::into)
    }
    fn set_house_entry_pause_enabled(&self, v: bool) -> Result<()> {
        pvzp_rs::set_house_pause(v).map_err(Into::into)
    }
    fn take_house_entry(&self) -> Result<bool> {
        pvzp_rs::take_house().map_err(Into::into)
    }
}
impl GameMusicBackend for PortableBackend {
    fn ensure_music_tune(&self, tune: i32) -> Result<()> {
        pvzp_rs::scene_music(tune).map_err(Into::into)
    }
}

impl rsvz_backend_api::NativeFastModeBackend for PortableBackend {
    fn clear_native_fast_mode(&self) -> Result<()> {
        pvzp_rs::clear_fast_mode().map_err(Into::into)
    }
}
