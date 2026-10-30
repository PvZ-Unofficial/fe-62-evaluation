//! Non-overloaded cannon commands, reexported from their owning game module.
//!
//! The same name has the same result/reporting, initialization and timing semantics
//! as rsvz::cob. Use `_many` for target collections and `_with_manager` for an explicit
//! list. Configuration types and advanced queries live in [`crate::cob`].
//!
//! Framework lifecycle operations are not script operations:
//! ```compile_fail
//! let manager = rsvz::cob::CobManager::new();
//! manager.reset_before_script();
//! ```
//! The old implementation-tree facade is no longer public:
//! ```compile_fail
//! use rsvz::core::logic::cob::CobManager;
//! ```

pub use rsvz_game::cob::{
    auto_set_cobs, auto_set_cobs_by, auto_set_cobs_by_with_manager, auto_set_cobs_with_manager, cob_list,
    cob_list_with_manager, column_manager, erase_cobs_from_list, erase_cobs_from_list_with_manager, fire, fire_many,
    fire_many_with_manager, fire_uncorrected, fire_uncorrected_many, fire_uncorrected_many_with_manager,
    fire_uncorrected_with_manager, fire_with_manager, fix_latest_cob, fix_latest_cob_with_manager, for_each_cob_grid,
    for_each_cob_grid_with_manager, move_cobs_to_list_bottom, move_cobs_to_list_bottom_with_manager,
    move_cobs_to_list_top, move_cobs_to_list_top_with_manager, plant_cob, raw_fire, raw_fire_coordinates,
    raw_fire_uncorrected, raw_fire_uncorrected_coordinates, recover_cob, recover_cob_list,
    recover_cob_list_with_manager, recover_cob_with_manager, recover_fire, recover_fire_many,
    recover_fire_many_with_manager, recover_fire_uncorrected, recover_fire_uncorrected_many,
    recover_fire_uncorrected_many_with_manager, recover_fire_uncorrected_with_manager, recover_fire_with_manager,
    roof_cob_fly_time, roof_recover_cob, roof_recover_cob_list, roof_recover_cob_list_with_manager,
    roof_recover_cob_with_manager, roof_usable_cob, roof_usable_cob_list, roof_usable_cob_list_with_manager,
    roof_usable_cob_with_manager, set_cob_columns, set_cob_sequential_mode, set_cob_sequential_mode_with_manager,
    set_cobs, set_cobs_with_manager, set_next_cob, set_next_cob_slot, set_next_cob_slot_with_manager,
    set_next_cob_with_manager, skip_cobs, skip_cobs_with_manager, try_auto_set_cobs, try_auto_set_cobs_by,
    try_auto_set_cobs_by_with_manager, try_auto_set_cobs_with_manager, try_fire, try_fire_many,
    try_fire_many_with_manager, try_fire_uncorrected, try_fire_uncorrected_many,
    try_fire_uncorrected_many_with_manager, try_fire_uncorrected_with_manager, try_fire_with_manager, try_raw_fire,
    try_raw_fire_coordinates, try_raw_fire_uncorrected, try_raw_fire_uncorrected_coordinates, try_recover_fire,
    try_recover_fire_many, try_recover_fire_many_with_manager, try_recover_fire_uncorrected,
    try_recover_fire_uncorrected_many, try_recover_fire_uncorrected_many_with_manager,
    try_recover_fire_uncorrected_with_manager, try_recover_fire_with_manager, try_set_cobs, try_set_cobs_with_manager,
    usable_cob, usable_cob_list, usable_cob_list_with_manager, usable_cob_with_manager,
};
