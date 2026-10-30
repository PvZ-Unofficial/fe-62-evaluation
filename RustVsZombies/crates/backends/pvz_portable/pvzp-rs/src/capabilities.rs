//! Atomic capabilities backed by the current native App/profile.
use crate::{Error, Result, raw};

pub fn temporary_unlock_enabled() -> bool {
    // SAFETY: the native getter reads one scalar and takes no pointers.
    unsafe { raw::pvzp_rs_temporary_unlock_enabled() != 0 }
}

/// # Safety
/// Requires exclusive backend access; menu refresh may run nested native UI.
pub unsafe fn set_temporary_unlock_enabled(enabled: bool) -> Result<()> {
    // SAFETY: caller excludes frame/entity borrows; native validates the current App/menu.
    Error::from_status(unsafe { raw::pvzp_rs_set_temporary_unlock_enabled(u8::from(enabled)) })
}

pub fn set_dance_mode(enabled: bool) -> Result<()> {
    // SAFETY: the wrapper checks Board and performs a non-recycling native operation.
    Error::from_status(unsafe { raw::pvzp_rs_set_dance_mode(u8::from(enabled)) })
}

pub fn dance_mode_enabled() -> Result<bool> {
    let mut value = 0;
    // SAFETY: writable scalar, native validates App.
    Error::from_status(unsafe { raw::pvzp_rs_dance_mode_enabled(&mut value) })?;
    Ok(value != 0)
}
pub fn restore_dance_mode(enabled: bool) -> Result<()> {
    // SAFETY: native validates App and handles absent Board during menu cleanup.
    Error::from_status(unsafe { raw::pvzp_rs_restore_dance_mode(u8::from(enabled)) })
}

pub fn seed_bank_visible() -> Result<bool> {
    let mut value = 0;
    // SAFETY: writable scalar, native validates Board and bank.
    Error::from_status(unsafe { raw::pvzp_rs_seed_bank_visible(&mut value) })?;
    Ok(value != 0)
}
pub fn set_seed_bank_visible(visible: bool) -> Result<()> {
    // SAFETY: native validates Board and bank before a scalar write.
    Error::from_status(unsafe { raw::pvzp_rs_set_seed_bank_visible(u8::from(visible)) })
}

pub fn set_common_dance(dance: i32) -> Result<()> {
    // SAFETY: the native wrapper validates the value and current Board.
    Error::from_status(unsafe { raw::pvzp_rs_set_common_dance(dance) })
}
pub fn set_cob_impact_delay(enabled: bool) -> Result<()> {
    // SAFETY: the native wrapper checks the current Board.
    Error::from_status(unsafe { raw::pvzp_rs_set_cob_impact_delay(u8::from(enabled)) })
}
pub fn coins() -> Result<u32> {
    let mut value = 0;
    // SAFETY: the out pointer is live and the native wrapper validates the profile.
    Error::from_status(unsafe { raw::pvzp_rs_coins(&raw mut value) })?;
    Ok(value)
}
pub fn set_coins(value: u32) -> Result<()> {
    // SAFETY: native validates the profile and amount before writing.
    Error::from_status(unsafe { raw::pvzp_rs_set_coins(value) })
}

pub fn pickup_limit(money: bool) -> Result<u32> {
    let mut value = 0;
    // SAFETY: writable scalar; native validates the selector.
    Error::from_status(unsafe { raw::pvzp_rs_pickup_limit(u8::from(money), &mut value) })?;
    Ok(value)
}
pub fn set_pickup_limit(money: bool, value: u32) -> Result<()> {
    // SAFETY: native validates the selector and nonnegative i32 limit.
    Error::from_status(unsafe { raw::pvzp_rs_set_pickup_limit(u8::from(money), value) })
}
/// # Safety
/// Must own exclusive backend access, without any live entity/Board borrows.
/// A modal input runs native UI until the modal closes, and may replace Board.
pub unsafe fn mouse(operation: i32, x: i32, y: i32, button: i32) -> Result<()> {
    // SAFETY: the caller holds exclusive access; native validates all inputs.
    Error::from_status(unsafe { raw::pvzp_rs_mouse(operation, x, y, button) })
}
pub fn sound(id: u32, stop: bool) -> Result<()> {
    // SAFETY: native validates the sound and owns its playback instances.
    Error::from_status(unsafe { raw::pvzp_rs_sound(id, u8::from(stop)) })
}
pub fn water_plant(row: i32, col: i32) -> Result<()> {
    // SAFETY: native validates the garden and plant, and does not recycle slots.
    Error::from_status(unsafe { raw::pvzp_rs_water_plant(row, col) })
}
pub fn unlock_trophy() -> Result<()> {
    // SAFETY: native validates the profile and only edits trophy records.
    Error::from_status(unsafe { raw::pvzp_rs_unlock_trophy() })
}
pub fn unlock_hidden_modes() -> Result<()> {
    // SAFETY: native validates the current screen and updates existing buttons.
    Error::from_status(unsafe { raw::pvzp_rs_unlock_hidden_modes() })
}

pub fn profile_writable() -> Result<()> {
    // SAFETY: native checks the current profile and the read-only invariant.
    Error::from_status(unsafe { raw::pvzp_rs_profile_writable() })
}
pub fn finished_adventure() -> Result<u32> {
    let mut value = 0;
    // SAFETY: scalar output stays writable through the checked native read.
    Error::from_status(unsafe { raw::pvzp_rs_finished_adventure(&mut value) })?;
    Ok(value)
}
pub fn set_finished_adventure(value: u32) -> Result<()> {
    // SAFETY: native checks the profile and write permission.
    Error::from_status(unsafe { raw::pvzp_rs_set_finished_adventure(value) })
}

pub fn challenge_record(index: u32) -> Result<u32> {
    let mut value = 0;
    // SAFETY: native validates array bounds/profile; output is a live scalar.
    Error::from_status(unsafe { raw::pvzp_rs_challenge_record(index, &mut value) })?;
    Ok(value)
}
pub fn set_challenge_record(index: u32, value: u32) -> Result<()> {
    // SAFETY: native checks bounds, profile and write permission.
    Error::from_status(unsafe { raw::pvzp_rs_set_challenge_record(index, value) })
}

pub fn purchase(index: u32) -> Result<u32> {
    let mut value = 0;
    // SAFETY: native validates array bounds/profile; output is a live scalar.
    Error::from_status(unsafe { raw::pvzp_rs_purchase(index, &mut value) })?;
    Ok(value)
}
pub fn set_purchase(index: u32, value: u32) -> Result<()> {
    // SAFETY: native checks bounds, profile and write permission.
    Error::from_status(unsafe { raw::pvzp_rs_set_purchase(index, value) })
}

/// # Safety
/// Requires exclusive backend access; the native loading dialog may run nested UI.
pub unsafe fn sync_game_selector(loading: bool) -> Result<()> {
    // SAFETY: exclusivity is guaranteed by the caller; native validates the current menu.
    Error::from_status(unsafe { raw::pvzp_rs_sync_game_selector(u8::from(loading)) })
}

pub fn world_generation() -> Result<Option<u64>> {
    let mut generation = 0;
    // SAFETY: live scalar output; native checks App and the world-generation invariant.
    Error::from_status(unsafe { raw::pvzp_rs_world_generation(&mut generation) })?;
    Ok((generation != 0).then_some(generation))
}

pub fn game_speed_multiplier() -> Result<f64> {
    let mut value = 0.0;
    // SAFETY: scalar output lives for the call; native checks current App.
    Error::from_status(unsafe { raw::pvzp_rs_game_speed_multiplier(&mut value) })?;
    Ok(value)
}
pub fn set_game_speed_multiplier(value: f64) -> Result<()> {
    // SAFETY: native validates the finite nonnegative scalar.
    Error::from_status(unsafe { raw::pvzp_rs_set_game_speed_multiplier(value) })
}
