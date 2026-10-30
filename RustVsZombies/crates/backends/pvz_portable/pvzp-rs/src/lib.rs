//! Narrow Rust bindings for PvZ-Portable's native facts and actions.
//!
//! Object pointers stay bounded by world/backend borrows and are never copied
//! into snapshots or collections.

pub mod capabilities;
pub mod raw;
pub mod replay;

pub const SPAWN_WAVE_CAPACITY: usize = raw::pvzp_rs_spawn_bounds_PVZP_RS_SPAWN_WAVE_CAPACITY as usize;

mod entity;
mod error;
pub mod modifier;
mod world;
pub use world::frame_duration_ms;
pub use world::release_advanced_pause_for_step;
pub use world::{chooser_card_available, chooser_card_locked, deselect_card, stop_seed_chooser_fast_forward};
pub use world::{enter_native_mode, save_current_game};

pub use error::{Error, Result};
pub use world::{
    Borrowed, GridItemPool, GridItemRef, ItemPool, ItemRef, MowerPool, MowerRef, PlantPool, PlantRef, PoolSlot,
    ProjectilePool, ProjectileRef, ReanimationRef, SeedRef, World, ZombiePool, ZombieRef, advanced_pause_active,
    app_counter, back_to_main_menu, click_continue_dialog_if_present, enter_endless, fast_forward_active, game_ui,
    input_focused, key_is_down, random_fixed, random_locked, random_seed, request_seed_chooser_fast_forward,
    restore_game_speed, seed_chooser_cancel_view_lawn, seed_chooser_choose_state, seed_chooser_modal_present,
    seed_chooser_mouse_visible, seed_chooser_parent_present, seed_chooser_seeds_in_flight, seed_chooser_view_lawn_time,
    seed_chooser_widget_manager_present, select_card, selected_card, selected_card_count, set_advanced_pause,
    set_app_counter, set_fast_forward, set_game_speed, set_random_mode, set_sun_production_mode,
    set_wave_spawn_random_seed, start_battle,
};
pub use world::{native_game_mode, seed_base_cost, seed_base_recharge};

pub fn verify_layout() -> Result<()> {
    // SAFETY: only fixed layout entries are compared; no game objects are touched.
    if unsafe {
        raw::pvzp_rs_check_raw_layout(
            raw_layout_checks::RAW_LAYOUT.as_ptr(),
            raw_layout_checks::RAW_LAYOUT.len() as u32,
        )
    } != 0
    {
        Ok(())
    } else {
        Err(Error::LayoutMismatch)
    }
}

mod raw_layout_checks {
    include!(concat!(env!("OUT_DIR"), "/raw_layout_checks.rs"));
}

pub use world::clear_fast_mode;
pub use world::{animation_skip, house_pause, scene_music, set_animation_skip, set_house_pause, take_house};

/// Called on the game's window thread, outside any native callback trampoline.
pub fn diagnostic_dialog(message: &str) -> i32 {
    let text = std::ffi::CString::new(message.replace('\0', " ")).expect("NUL removed");
    unsafe { raw::pvzp_rs_diagnostic_dialog(text.as_ptr()) }
}
pub fn diagnostic_exit() {
    unsafe {
        raw::pvzp_rs_diagnostic_exit();
    }
}
