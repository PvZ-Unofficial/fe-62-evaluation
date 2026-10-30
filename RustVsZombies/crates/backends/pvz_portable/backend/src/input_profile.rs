use crate::{PortableBackend, Result};
use pvzp_rs::capabilities as native;
use rsvz_backend_api::backend::{
    AudioBackend, CobImpactDelayBackend, CoinProfileBackend, CommonZombieDanceBackend, GardenBackend,
    HiddenModeUnlockBackend, InputBackend, TrophyUnlockBackend,
};
use rsvz_model::model::{Grid, MouseButton, PixelPos, RefreshDance, SoundId};

impl rsvz_backend_api::DanceModeBackend for PortableBackend {
    fn dance_mode_enabled(&self) -> Result<bool> {
        Ok(native::dance_mode_enabled()?)
    }
    fn restore_dance_mode(&self, enabled: bool) -> Result<()> {
        Ok(native::restore_dance_mode(enabled)?)
    }
    fn set_dance_mode(&self, enabled: bool) -> Result<()> {
        native::set_dance_mode(enabled)?;
        Ok(())
    }
}

impl rsvz_backend_api::SeedBankVisibilityBackend for PortableBackend {
    fn seed_bank_visible(&self) -> Result<bool> {
        Ok(native::seed_bank_visible()?)
    }
    fn set_seed_bank_visible(&self, visible: bool) -> Result<()> {
        Ok(native::set_seed_bank_visible(visible)?)
    }
}

impl CommonZombieDanceBackend for PortableBackend {
    fn set_common_zombie_dance(&self, dance: RefreshDance) -> Result<()> {
        native::set_common_dance(match dance {
            RefreshDance::Unchanged | RefreshDance::None => 0,
            RefreshDance::Fast => 1,
            RefreshDance::Slow => 2,
        })?;
        Ok(())
    }
}
impl CobImpactDelayBackend for PortableBackend {
    fn set_cob_impact_delay(&self, enabled: bool) -> Result<()> {
        Ok(native::set_cob_impact_delay(enabled)?)
    }
}
impl CoinProfileBackend for PortableBackend {
    fn coins(&self) -> Result<u32> {
        Ok(native::coins()?)
    }
    fn set_coins(&self, value: u32) -> Result<()> {
        Ok(native::set_coins(value)?)
    }
}

impl rsvz_backend_api::PickupLimitsBackend for PortableBackend {
    fn sunlight_limit(&self) -> Result<u32> {
        Ok(native::pickup_limit(false)?)
    }
    fn set_sunlight_limit(&self, value: u32) -> Result<()> {
        Ok(native::set_pickup_limit(false, value)?)
    }
    fn money_limit(&self) -> Result<u32> {
        Ok(native::pickup_limit(true)?)
    }
    fn set_money_limit(&self, value: u32) -> Result<()> {
        Ok(native::set_pickup_limit(true, value)?)
    }
}
impl InputBackend for PortableBackend {
    fn mouse_move(&mut self, pos: PixelPos) -> Result<()> {
        // SAFETY: &mut token excludes frames/entities; BackendScope guards implicit borrows.
        Ok(unsafe { native::mouse(0, pos.x, pos.y, 1) }?)
    }
    fn mouse_down(&mut self, pos: PixelPos, button: MouseButton) -> Result<()> {
        // SAFETY: exclusive access survives nested UI; the host prevents reentrant dispatch.
        Ok(unsafe { native::mouse(1, pos.x, pos.y, mouse_button(button)) }?)
    }
    fn mouse_up(&mut self, pos: PixelPos, button: MouseButton) -> Result<()> {
        // SAFETY: exclusive access; the existing access epoch invalidates pre-input samples.
        Ok(unsafe { native::mouse(2, pos.x, pos.y, mouse_button(button)) }?)
    }
    fn release_mouse(&mut self) -> Result<()> {
        // SAFETY: the exclusive token excludes cursor/Board/entity borrows.
        Ok(unsafe { native::mouse(3, 0, 0, 1) }?)
    }
}
fn mouse_button(button: MouseButton) -> i32 {
    match button {
        MouseButton::Left => 1,
        MouseButton::Right => -1,
    }
}
impl AudioBackend for PortableBackend {
    fn play_sound(&self, sound: SoundId) -> Result<()> {
        Ok(native::sound(sound.0, false)?)
    }
    fn stop_sound(&self, sound: SoundId) -> Result<()> {
        Ok(native::sound(sound.0, true)?)
    }
}
impl GardenBackend for PortableBackend {
    fn water_plant(&self, grid: Grid) -> Result<()> {
        Ok(native::water_plant(grid.row, grid.col)?)
    }
}
impl TrophyUnlockBackend for PortableBackend {
    fn unlock_trophy(&self) -> Result<()> {
        Ok(native::unlock_trophy()?)
    }
}
impl HiddenModeUnlockBackend for PortableBackend {
    fn unlock_hidden_modes(&self) -> Result<()> {
        Ok(native::unlock_hidden_modes()?)
    }
}

impl rsvz_backend_api::HiddenPagesBackend for PortableBackend {
    fn hidden_pages_visible(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::hidden_pages_visible())
    }
    fn set_hidden_pages_visible(&self, enabled: bool) -> Result<()> {
        Ok(pvzp_rs::modifier::set_hidden_pages_visible(enabled)?)
    }
}

impl rsvz_backend_api::TemporaryUnlockBackend for PortableBackend {
    fn temporary_unlock_enabled(&self) -> Result<bool> {
        Ok(native::temporary_unlock_enabled())
    }
    fn set_temporary_unlock_enabled(&mut self, enabled: bool) -> Result<()> {
        // SAFETY: exclusive token excludes borrows throughout the native menu refresh.
        Ok(unsafe { native::set_temporary_unlock_enabled(enabled) }?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implicit_shared_access_rejects_exclusive_input_scope() {
        let mut backend = PortableBackend::new();
        crate::PortableBackend::scope_backend(&mut backend, || {
            crate::PortableBackend::with_backend_shared(|_| {
                assert!(matches!(
                    crate::PortableBackend::try_with_backend(|_| panic!("exclusive closure must not run")),
                    Err(rsvz_backend_api::access::BackendAccessError::BorrowConflict)
                ));
            })
            .unwrap();
            let before = crate::PortableBackend::backend_access_epoch();
            crate::PortableBackend::try_with_backend(|_| ()).unwrap();
            assert_ne!(before, crate::PortableBackend::backend_access_epoch());
        });
    }
}

impl rsvz_backend_api::ProfileProgressBackend for PortableBackend {
    fn ensure_profile_writable(&self) -> Result<()> {
        Ok(native::profile_writable()?)
    }
    fn finished_adventure(&self) -> Result<u32> {
        Ok(native::finished_adventure()?)
    }
    fn set_finished_adventure(&self, value: u32) -> Result<()> {
        Ok(native::set_finished_adventure(value)?)
    }
    fn challenge_record(&self, index: u32) -> Result<u32> {
        Ok(native::challenge_record(index)?)
    }
    fn set_challenge_record(&self, index: u32, value: u32) -> Result<()> {
        Ok(native::set_challenge_record(index, value)?)
    }
    fn purchase(&self, index: u32) -> Result<u32> {
        Ok(native::purchase(index)?)
    }
    fn set_purchase(&self, index: u32, value: u32) -> Result<()> {
        Ok(native::set_purchase(index, value)?)
    }
    fn sync_game_selector(&mut self, loading: bool) -> Result<()> {
        // SAFETY: exclusive token access excludes outstanding frame/entity borrows.
        Ok(unsafe { native::sync_game_selector(loading) }?)
    }
}
