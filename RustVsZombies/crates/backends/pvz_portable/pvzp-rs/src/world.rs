#![allow(
    unsafe_code,
    reason = "this module binds borrowed native Portable objects to world lifetimes"
)]

use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::{Error, Result, raw};

pub fn chooser_card_available(kind: i32) -> Result<bool> {
    // SAFETY: writable scalar output; the bridge checks kind and chooser presence.
    out_value(|out| unsafe { raw::pvzp_rs_chooser_card_available(kind, out) }).map(|value| value != 0)
}
pub fn chooser_card_locked(index: u32) -> Result<bool> {
    // SAFETY: writable scalar output; the bridge validates the native bank index.
    out_value(|out| unsafe { raw::pvzp_rs_chooser_card_locked(index, out) }).map(|value| value != 0)
}
pub fn deselect_card(index: u32) -> Result<()> {
    // SAFETY: the bridge validates the live chooser, bank index, motion and lock.
    Error::from_status(unsafe { raw::pvzp_rs_deselect_card(index) })
}
pub fn stop_seed_chooser_fast_forward() -> Result<()> {
    // SAFETY: current game thread; only cancels the native UI-advancement request.
    Error::from_status(unsafe { raw::pvzp_rs_stop_seed_chooser_fast_forward() })
}

pub fn seed_base_cost(kind: i32) -> i32 {
    // SAFETY: the bridge validates the scalar kind before accessing the native definition.
    unsafe { raw::pvzp_rs_seed_base_cost(kind) }
}

pub fn native_game_mode() -> Result<i32> {
    out_value(|out| {
        // SAFETY: writable scalar output; native checks App presence.
        unsafe { raw::pvzp_rs_native_game_mode(out) }
    })
}
pub fn frame_duration_ms() -> Result<i32> {
    out_value(|out| {
        // SAFETY: writable output; native validates the App without requiring a Board.
        unsafe { raw::pvzp_rs_frame_duration_ms(out) }
    })
}

pub fn seed_base_recharge(kind: i32) -> i32 {
    // SAFETY: the bridge validates the scalar kind before accessing the native definition.
    unsafe { raw::pvzp_rs_seed_base_recharge(kind) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolSlot {
    pub active: bool,
    pub id: u32,
    pub next_free: u32,
}

pub struct Borrowed<'a, T> {
    ptr: NonNull<T>,
    _borrow: PhantomData<&'a World<'a>>,
}

impl<T> Copy for Borrowed<'_, T> {}

impl<T> Clone for Borrowed<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Borrowed<'_, T> {
    pub(super) fn new(ptr: NonNull<T>) -> Self {
        Self {
            ptr,
            _borrow: PhantomData,
        }
    }

    #[must_use]
    pub const fn as_ptr(self) -> *mut T {
        self.ptr.as_ptr()
    }

    #[must_use]
    pub const fn as_non_null(self) -> NonNull<T> {
        self.ptr
    }

    /// Rebinds a pointer already validated against the current native world.
    ///
    /// # Safety
    ///
    /// `ptr` must remain valid for `'a` and belong to the same world borrow.
    #[doc(hidden)]
    pub unsafe fn from_non_null<'a>(ptr: NonNull<T>) -> Borrowed<'a, T> {
        Borrowed::new(ptr)
    }
}

pub type PlantRef<'a> = Borrowed<'a, raw::pvzp_rs_plant>;
pub type ZombieRef<'a> = Borrowed<'a, raw::pvzp_rs_zombie>;
pub type ProjectileRef<'a> = Borrowed<'a, raw::pvzp_rs_projectile>;
pub type GridItemRef<'a> = Borrowed<'a, raw::pvzp_rs_grid_item>;
pub type ItemRef<'a> = Borrowed<'a, raw::pvzp_rs_item>;
pub type MowerRef<'a> = Borrowed<'a, raw::pvzp_rs_mower>;
pub type SeedRef<'a> = Borrowed<'a, raw::pvzp_rs_seed>;
pub type ReanimationRef<'a> = Borrowed<'a, raw::pvzp_rs_reanimation>;

pub fn game_ui() -> Result<i32> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_game_ui(out) }
    })
}

pub fn seed_chooser_mouse_visible() -> Result<bool> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_seed_chooser_mouse_visible(out) }
    })
    .map(|value: u8| value != 0)
}

pub fn seed_chooser_parent_present() -> Result<bool> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_seed_chooser_parent_present(out) }
    })
    .map(|value: u8| value != 0)
}

pub fn seed_chooser_widget_manager_present() -> Result<bool> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_seed_chooser_widget_manager_present(out) }
    })
    .map(|value: u8| value != 0)
}

pub fn seed_chooser_modal_present() -> Result<bool> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_seed_chooser_modal_present(out) }
    })
    .map(|value: u8| value != 0)
}

pub fn seed_chooser_choose_state() -> Result<i32> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_seed_chooser_choose_state(out) }
    })
}

pub fn seed_chooser_view_lawn_time() -> Result<i32> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_seed_chooser_view_lawn_time(out) }
    })
}

pub fn seed_chooser_seeds_in_flight() -> Result<i32> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_seed_chooser_seeds_in_flight(out) }
    })
}

pub fn seed_chooser_cancel_view_lawn() -> Result<()> {
    // SAFETY: the bridge validates the current chooser structural state.
    Error::from_status(unsafe { raw::pvzp_rs_seed_chooser_cancel_view_lawn() })
}

pub fn click_continue_dialog_if_present() -> Result<bool> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_click_continue_dialog_if_present(out) }
    })
    .map(|clicked: u8| clicked != 0)
}

pub fn key_is_down(key: u16) -> bool {
    // SAFETY: the bridge bounds-checks the key and only reads game-owned input state.
    unsafe { raw::pvzp_rs_key_is_down(key) != 0 }
}

pub fn input_focused() -> Result<bool> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_input_focused(out) }
    })
    .map(|focused: u8| focused != 0)
}

pub fn app_counter() -> Result<u32> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_app_counter(out) }
    })
}

pub fn set_app_counter(counter: u32) -> Result<()> {
    // SAFETY: the bridge validates that the application exists before writing the scalar.
    Error::from_status(unsafe { raw::pvzp_rs_set_app_counter(counter) })
}

pub fn selected_card_count() -> Result<u32> {
    out_value(|out| {
        // SAFETY: `out` points to writable scalar storage.
        unsafe { raw::pvzp_rs_selected_card_count(out) }
    })
}

pub fn selected_card(index: u32) -> Result<(i32, i32)> {
    let mut packet = 0;
    let mut imitater = 0;
    // SAFETY: both output pointers refer to writable local scalar storage.
    Error::from_status(unsafe { raw::pvzp_rs_selected_card(index, &raw mut packet, &raw mut imitater) })?;
    Ok((packet, imitater))
}

pub fn select_card(packet_type: i32, imitater_type: i32) -> Result<()> {
    // SAFETY: both enum values are copied and validated by the bridge.
    Error::from_status(unsafe { raw::pvzp_rs_select_card(packet_type, imitater_type) })
}

pub fn start_battle() -> Result<()> {
    // SAFETY: the bridge validates the current chooser lifecycle state.
    Error::from_status(unsafe { raw::pvzp_rs_start_battle() })
}

pub fn enter_endless(scene: i32) -> Result<()> {
    // SAFETY: the bridge validates the scene and current UI transition.
    Error::from_status(unsafe { raw::pvzp_rs_enter_endless(scene) })
}
/// # Safety
/// Requires exclusive native access; this can allocate or replace Board and widgets.
pub unsafe fn enter_native_mode(mode: i32) -> Result<()> {
    // SAFETY: caller owns exclusive access; native validates the mode and phase.
    Error::from_status(unsafe { raw::pvzp_rs_enter_native_mode(mode) })
}
/// # Safety
/// Requires exclusive access; saving may finish a pending native end sequence.
pub unsafe fn save_current_game() -> Result<()> {
    // SAFETY: exclusive caller; native validates the current battle.
    Error::from_status(unsafe { raw::pvzp_rs_save_current_game() })
}

pub fn back_to_main_menu() -> Result<()> {
    // SAFETY: the bridge validates the current live-playing state.
    Error::from_status(unsafe { raw::pvzp_rs_back_to_main_menu() })
}

pub fn set_game_speed(speed: f32) -> Result<()> {
    // SAFETY: the copied scalar is validated by the bridge.
    Error::from_status(unsafe { raw::pvzp_rs_set_game_speed(speed) })
}

pub fn restore_game_speed() -> Result<()> {
    // SAFETY: the bridge only restores previously captured native timing scalars.
    Error::from_status(unsafe { raw::pvzp_rs_restore_game_speed() })
}

pub fn set_fast_forward(enabled: bool, performance: i32, suppress_window: bool) -> Result<()> {
    // SAFETY: all arguments are copied scalars validated by the bridge.
    Error::from_status(unsafe {
        raw::pvzp_rs_set_fast_forward(u8::from(enabled), performance, u8::from(suppress_window))
    })
}

pub fn fast_forward_active() -> bool {
    // SAFETY: this only reads native scalar state; any stream selector is a validated bool.
    unsafe { raw::pvzp_rs_fast_forward_active() != 0 }
}

pub fn request_seed_chooser_fast_forward(max_frames: u32) -> Result<()> {
    // SAFETY: the copied frame cap is validated by the bridge.
    Error::from_status(unsafe { raw::pvzp_rs_request_seed_chooser_fast_forward(max_frames) })
}

pub fn set_advanced_pause(
    enabled: bool, draw_mask: bool, rgba: u32, play_sound: bool, refresh_cursor_preview: bool,
) -> Result<()> {
    // SAFETY: all arguments are copied scalars.
    Error::from_status(unsafe {
        raw::pvzp_rs_set_advanced_pause(
            u8::from(enabled),
            u8::from(draw_mask),
            rgba,
            u8::from(play_sound),
            u8::from(refresh_cursor_preview),
        )
    })
}

pub fn advanced_pause_active() -> bool {
    // SAFETY: this query reads only bridge-owned scalar state and takes no pointers.
    unsafe { raw::pvzp_rs_advanced_pause_active() != 0 }
}

pub fn release_advanced_pause_for_step(play_sound: bool) -> Result<()> {
    // SAFETY: the caller holds the backend scope; only the native pause scalar changes.
    Error::from_status(unsafe { raw::pvzp_rs_release_advanced_pause_for_step(u8::from(play_sound)) })
}

pub fn set_random_mode(mode: i32, value: u32) -> Result<()> {
    // SAFETY: the bridge validates the copied mode discriminant.
    Error::from_status(unsafe { raw::pvzp_rs_set_random_mode(mode, value) })
}

pub fn set_wave_spawn_random_seed(base_seed: Option<u32>) -> Result<()> {
    let (enabled, seed) = base_seed.map_or((0, 0), |seed| (1, seed));
    // SAFETY: the bridge copies the seed and changes only the inactive wave-spawn override.
    Error::from_status(unsafe { raw::pvzp_rs_set_wave_spawn_random_seed(enabled, seed) })
}

pub fn random_seed(stream: i32) -> Result<u32> {
    out_value(|out| {
        // SAFETY: output is writable scalar storage; the bridge checks the stream index.
        unsafe { raw::pvzp_rs_random_seed(stream, out) }
    })
}
pub fn random_locked(level_stream: bool) -> bool {
    // SAFETY: this only reads native scalar state; any stream selector is a validated bool.
    unsafe { raw::pvzp_rs_random_locked(level_stream) != 0 }
}
pub fn random_fixed(level_stream: bool) -> u32 {
    // SAFETY: this only reads native scalar state; any stream selector is a validated bool.
    unsafe { raw::pvzp_rs_random_fixed(level_stream) }
}

pub fn set_sun_production_mode(mode: i32) -> Result<()> {
    // SAFETY: the bridge validates the copied mode discriminant.
    Error::from_status(unsafe { raw::pvzp_rs_set_sun_production_mode(mode) })
}

pub struct World<'a> {
    raw: NonNull<raw::pvzp_rs_world>,
    _borrow: PhantomData<&'a ()>,
}

impl<'a> World<'a> {
    /// Reuses a Board validated by the caller's shared backend scope.
    ///
    /// # Safety
    /// The Board must remain alive without native update or replacement for `'a`.
    #[doc(hidden)]
    pub unsafe fn from_non_null(raw: NonNull<raw::pvzp_rs_world>) -> Self {
        Self {
            raw,
            _borrow: PhantomData,
        }
    }

    /// Borrows the current native Board for the caller-controlled backend scope.
    ///
    /// # Safety
    ///
    /// The caller must keep the Board alive and prevent native updates for `'a`.
    pub unsafe fn current() -> Result<Self> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: `raw` points to writable pointer storage.
        Error::from_status(unsafe { raw::pvzp_rs_current_world(&raw mut raw) })?;
        Ok(Self {
            raw: NonNull::new(raw).ok_or(Error::NullPointer)?,
            _borrow: PhantomData,
        })
    }

    #[must_use]
    pub const fn as_ptr(&self) -> *mut raw::pvzp_rs_world {
        self.raw.as_ptr()
    }

    #[doc(hidden)]
    pub const fn as_non_null(&self) -> NonNull<raw::pvzp_rs_world> {
        self.raw
    }

    pub fn plant_slot_id(&self, slot: u32) -> Result<Option<u32>> {
        let mut id = 0;
        // SAFETY: this Board borrow is live; the native atom checks the slot bound.
        Error::from_status(unsafe { raw::pvzp_rs_world_plant_slot_id(self.as_ptr(), slot, &mut id) })?;
        Ok((id != 0).then_some(id))
    }

    pub fn validate_plant_slot_change(&self, source: u32, target: u32, expected_target: u32) -> Result<()> {
        let (mut a, mut b) = (0, 0);
        // SAFETY: commit=0 does not mutate the native pool or retained IDs.
        Error::from_status(unsafe {
            raw::pvzp_rs_world_relocate_plant_slot(self.as_ptr(), source, target, expected_target, &mut a, &mut b, 0)
        })
    }

    /// # Safety
    /// The caller must own exclusive backend access at an outer native update
    /// boundary, with no outstanding entity borrows from this World's lifetime.
    pub unsafe fn relocate_plant_slot(&mut self, source: u32, target: u32, expected_target: u32) -> Result<(u32, u32)> {
        let (mut moved, mut swapped) = (0, 0);
        // SAFETY: the caller proves exclusivity; native code validates the full
        // IDs, pool invariants and phase before copying any entity state.
        Error::from_status(unsafe {
            raw::pvzp_rs_world_relocate_plant_slot(
                self.as_ptr(),
                source,
                target,
                expected_target,
                &mut moved,
                &mut swapped,
                1,
            )
        })?;
        Ok((moved, swapped))
    }

    pub fn plants(&self) -> PlantPool<'a> {
        // SAFETY: the bridge returns an embedded pool of this live Board.
        unsafe { PlantPool::new(NonNull::new_unchecked(raw::pvzp_rs_world_plant_pool(self.raw.as_ptr()))) }
    }

    /// Caller must exclude all native traversal and entity/pool borrows.
    pub unsafe fn process_delete_queue(&mut self) -> Result<()> {
        // SAFETY: caller establishes exclusive outer-frame access; native validates world/phase.
        Error::from_status(unsafe { raw::pvzp_rs_world_process_delete_queue(self.raw.as_ptr()) })
    }

    pub fn zombies(&self) -> ZombiePool<'a> {
        // SAFETY: the bridge returns an embedded pool of this live Board.
        unsafe {
            ZombiePool::new(NonNull::new_unchecked(raw::pvzp_rs_world_zombie_pool(
                self.raw.as_ptr(),
            )))
        }
    }

    pub fn projectiles(&self) -> ProjectilePool<'a> {
        // SAFETY: the bridge returns an embedded pool of this live Board.
        unsafe {
            ProjectilePool::new(NonNull::new_unchecked(raw::pvzp_rs_world_projectile_pool(
                self.raw.as_ptr(),
            )))
        }
    }

    pub fn grid_items(&self) -> GridItemPool<'a> {
        // SAFETY: the bridge returns an embedded pool of this live Board.
        unsafe {
            GridItemPool::new(NonNull::new_unchecked(raw::pvzp_rs_world_grid_item_pool(
                self.raw.as_ptr(),
            )))
        }
    }

    pub fn lawn_mowers(&self) -> MowerPool<'a> {
        // SAFETY: the returned pool is embedded in this borrowed live Board.
        unsafe { MowerPool::new(NonNull::new_unchecked(raw::pvzp_rs_world_mower_pool(self.raw.as_ptr()))) }
    }
    /// # Safety
    /// The caller owns exclusive backend access; no entity/animation borrow may
    /// survive native allocation or deletion-queue reclamation by the caller.
    pub unsafe fn initialize_mowers_ready(&mut self) -> Result<()> {
        // SAFETY: caller supplies the exclusive native access contract.
        Error::from_status(unsafe { raw::pvzp_rs_world_initialize_mowers_ready(self.raw.as_ptr()) })
    }
    pub fn items(&self) -> ItemPool<'a> {
        // SAFETY: the bridge returns an embedded pool of this live Board.
        unsafe { ItemPool::new(NonNull::new_unchecked(raw::pvzp_rs_world_item_pool(self.raw.as_ptr()))) }
    }

    pub fn scene(&self) -> i32 {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_scene(self.raw.as_ptr()) }
    }

    pub fn paused(&self) -> bool {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_paused(self.raw.as_ptr()) != 0 }
    }

    pub fn frame_duration_ms(&self) -> i32 {
        // SAFETY: borrowed live Board owns a live App.
        unsafe { raw::pvzp_rs_world_frame_duration_ms(self.raw.as_ptr()) }
    }
    pub fn survival_stage(&self) -> i32 {
        // SAFETY: borrowed live Board owns a live Challenge.
        unsafe { raw::pvzp_rs_world_survival_stage(self.raw.as_ptr()) }
    }
    pub fn survival_flags_completed(&self) -> i32 {
        // SAFETY: live borrowed Board; native scalar query performs no mutation.
        unsafe { raw::pvzp_rs_world_survival_flags_completed(self.raw.as_ptr()) }
    }
    pub fn set_survival_stage(&self, stage: i32) -> Result<()> {
        // SAFETY: live scalar write; bridge checks the native arithmetic range.
        Error::from_status(unsafe { raw::pvzp_rs_world_set_survival_stage(self.raw.as_ptr(), stage) })
    }
    /// # Safety
    /// Requires exclusive native access; dialog callbacks may touch other game objects.
    pub unsafe fn set_pause_dialog(&mut self, enabled: bool) -> Result<()> {
        // SAFETY: caller holds exclusive native access.
        Error::from_status(unsafe { raw::pvzp_rs_world_set_pause_dialog(self.raw.as_ptr(), enabled.into()) })
    }
    /// # Safety
    /// Requires exclusive native access; native completion may consume cursor and coin objects.
    pub unsafe fn begin_level_completion(&mut self) -> Result<()> {
        // SAFETY: caller holds exclusive native access.
        Error::from_status(unsafe { raw::pvzp_rs_world_begin_level_completion(self.raw.as_ptr()) })
    }
    pub fn validate_level_completion(&self) -> Result<()> {
        // SAFETY: borrowed live Board; validation reads native transition prerequisites.
        Error::from_status(unsafe { raw::pvzp_rs_world_validate_level_completion(self.raw.as_ptr()) })
    }
    /// # Safety
    /// Requires exclusive native access because old text animations can retire.
    pub unsafe fn show_hint(&mut self, text: &str, duration: i32) -> Result<()> {
        let length = u32::try_from(text.len()).map_err(|_| Error::InvalidArgument { detail: 126 })?;
        // SAFETY: bytes stay borrowed for the call; bridge validates bounds and copies text.
        Error::from_status(unsafe {
            raw::pvzp_rs_world_show_hint(self.raw.as_ptr(), text.as_ptr().cast(), length, duration)
        })
    }
    pub fn caption_bytes(&self, output: &mut [u8; 128]) -> Result<usize> {
        let mut length = 0;
        // SAFETY: fixed writable output; only initialized native text is copied.
        Error::from_status(unsafe {
            raw::pvzp_rs_world_caption_text(self.raw.as_ptr(), output.as_mut_ptr().cast(), 128, &mut length)
        })?;
        Ok(length as usize)
    }
    pub fn caption_duration(&self) -> Result<i32> {
        self.scalar(raw::pvzp_rs_world_caption_duration)
    }
    pub fn caption_style(&self) -> Result<i32> {
        self.scalar(raw::pvzp_rs_world_caption_style)
    }

    pub fn seed_choosing(&self) -> Result<bool> {
        self.scalar(raw::pvzp_rs_world_seed_choosing)
            .map(|value: u8| value != 0)
    }

    pub fn main_counter(&self) -> u32 {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_main_counter(self.raw.as_ptr()) }
    }

    pub fn imitater_successor(&self, placeholder_id: u32) -> Option<u32> {
        let mut successor = 0;
        // SAFETY: this world is live and output points to writable scalar storage.
        let found =
            unsafe { raw::pvzp_rs_world_imitater_successor(self.raw.as_ptr(), placeholder_id, &raw mut successor) };
        (found != 0).then_some(successor)
    }

    pub fn reset(&mut self, completed_rounds: u32, seed: u32, initial_sun: u32, ready_cooldowns: bool) -> Result<()> {
        // SAFETY: the bridge validates that this is the current Board before replacing it.
        Error::from_status(unsafe {
            raw::pvzp_rs_world_reset(
                self.raw.as_ptr(),
                completed_rounds,
                seed,
                initial_sun,
                u8::from(ready_cooldowns),
            )
        })
    }

    pub fn cursor_type(&self) -> Result<i32> {
        self.scalar(raw::pvzp_rs_world_cursor_type)
    }
    pub fn cursor_seed_kind(&self) -> Result<i32> {
        self.scalar(raw::pvzp_rs_world_cursor_seed_kind)
    }
    pub fn cursor_imitater_kind(&self) -> Result<i32> {
        self.scalar(raw::pvzp_rs_world_cursor_imitater_kind)
    }

    pub fn has_conveyor(&self) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: this borrowed Board and writable output remain live during the call.
            unsafe { raw::pvzp_rs_world_has_conveyor(self.raw.as_ptr(), out) }
        })
    }

    pub fn is_column_challenge(&self) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: this borrowed Board owns its current App reference.
            unsafe { raw::pvzp_rs_world_is_column_challenge(self.raw.as_ptr(), out) }
        })
    }

    pub fn is_wallnut_bowling(&self) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: this borrowed Board owns its current App reference.
            unsafe { raw::pvzp_rs_world_is_wallnut_bowling(self.raw.as_ptr(), out) }
        })
    }

    pub fn current_wave(&self) -> i32 {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_current_wave(self.raw.as_ptr()) }
    }

    pub fn sun(&self) -> i32 {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_sun(self.raw.as_ptr()) }
    }

    pub fn level_complete(&self) -> bool {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_level_complete(self.raw.as_ptr()) != 0 }
    }

    pub fn next_survival_stage_counter(&self) -> i32 {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_next_survival_stage_counter(self.raw.as_ptr()) }
    }

    pub fn set_sun(&self, sun: u32) -> Result<()> {
        // SAFETY: this world is live and the scalar is copied.
        Error::from_status(unsafe { raw::pvzp_rs_world_set_sun(self.raw.as_ptr(), sun) })
    }

    pub fn natural_sun_generated(&self) -> i32 {
        // SAFETY: this world remains alive throughout its protected borrow.
        unsafe { raw::pvzp_rs_world_natural_sun_generated(self.raw.as_ptr()) }
    }

    pub fn natural_sun_countdown(&self) -> i32 {
        // SAFETY: this world remains alive throughout its protected borrow.
        unsafe { raw::pvzp_rs_world_natural_sun_countdown(self.raw.as_ptr()) }
    }

    pub fn set_natural_sun_generated(&self, count: i32) -> Result<()> {
        // SAFETY: this world is live and the scalar is copied.
        Error::from_status(unsafe { raw::pvzp_rs_world_set_natural_sun_generated(self.raw.as_ptr(), count) })
    }

    pub fn set_natural_sun_countdown(&self, countdown: i32) -> Result<()> {
        // SAFETY: this world is live and the scalar is copied.
        Error::from_status(unsafe { raw::pvzp_rs_world_set_natural_sun_countdown(self.raw.as_ptr(), countdown) })
    }

    pub fn ice_path_x(&self, row: u32) -> Result<i32> {
        out_value(|out| {
            // SAFETY: this world is live, output is writable, and native code checks row bounds.
            unsafe { raw::pvzp_rs_world_ice_path_x(self.raw.as_ptr(), row, out) }
        })
    }
    pub fn ice_path_countdown(&self, row: u32) -> Result<u32> {
        out_value(|out| {
            // SAFETY: this world is live, output is writable, and native code checks row bounds.
            unsafe { raw::pvzp_rs_world_ice_path_countdown(self.raw.as_ptr(), row, out) }
        })
    }
    pub fn row_pick_weight_bits(&self, row: u32) -> Result<u32> {
        out_value(|out| {
            // SAFETY: this world is live, output is writable, and native code checks row bounds.
            unsafe { raw::pvzp_rs_world_row_pick_weight_bits(self.raw.as_ptr(), row, out) }
        })
    }
    pub fn row_pick_last_picked_bits(&self, row: u32) -> Result<u32> {
        out_value(|out| {
            // SAFETY: this world is live, output is writable, and native code checks row bounds.
            unsafe { raw::pvzp_rs_world_row_pick_last_picked_bits(self.raw.as_ptr(), row, out) }
        })
    }
    pub fn row_pick_second_last_picked_bits(&self, row: u32) -> Result<u32> {
        out_value(|out| {
            // SAFETY: this world is live, output is writable, and native code checks row bounds.
            unsafe { raw::pvzp_rs_world_row_pick_second_last_picked_bits(self.raw.as_ptr(), row, out) }
        })
    }
    pub fn spawn_allowed(&self, zombie_type: u32) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_spawn_allowed(self.raw.as_ptr(), zombie_type, out) }
        })
    }

    pub fn spawn_entry(&self, wave: u32, slot: u32) -> Result<i32> {
        out_value(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_spawn_entry(self.raw.as_ptr(), wave, slot, out) }
        })
    }
    pub fn spawn_huge_wave(&self, wave: u32) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: borrowed world and writable output; native validates wave/count.
            unsafe { raw::pvzp_rs_world_spawn_huge_wave(self.raw.as_ptr(), wave, out) }
        })
    }
    pub fn refresh_spawn_preview(&self) -> Result<()> {
        // SAFETY: native checks the current chooser; no scene switch or battle start.
        Error::from_status(unsafe { raw::pvzp_rs_world_refresh_spawn_preview(self.raw.as_ptr()) })
    }
    pub fn board_spawn_seed(&self) -> Result<u32> {
        out_value(|out| {
            // SAFETY: current world and writable scalar, checked by native.
            unsafe { raw::pvzp_rs_world_board_spawn_seed(self.raw.as_ptr(), out) }
        })
    }
    pub fn generate_current_spawn(&self, seed: u32) -> Result<()> {
        // SAFETY: native validates current Board and count, using a scoped RNG.
        Error::from_status(unsafe { raw::pvzp_rs_world_generate_current_spawn(self.raw.as_ptr(), seed) })
    }

    pub fn total_waves(&self) -> i32 {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_total_waves(self.raw.as_ptr()) }
    }

    pub fn refresh_countdown(&self) -> i32 {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_refresh_countdown(self.raw.as_ptr()) }
    }

    pub fn initial_countdown(&self) -> i32 {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_initial_countdown(self.raw.as_ptr()) }
    }

    pub fn huge_wave_countdown(&self) -> i32 {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_huge_wave_countdown(self.raw.as_ptr()) }
    }

    pub fn level_end_countdown(&self) -> i32 {
        // SAFETY: this live borrowed Board owns the copied scalar field.
        unsafe { raw::pvzp_rs_world_level_end_countdown(self.raw.as_ptr()) }
    }

    pub fn commit_wave_refresh(&self, expected_wave: i32, initial_countdown: i32) -> Result<()> {
        // SAFETY: this world is live and the bridge validates both preconditions before writing.
        Error::from_status(unsafe {
            raw::pvzp_rs_world_commit_wave_refresh(self.raw.as_ptr(), expected_wave, initial_countdown)
        })
    }

    pub fn zombie_refresh_health_threshold(&self) -> Result<i32> {
        self.scalar(raw::pvzp_rs_world_zombie_refresh_health_threshold)
    }
    pub fn zombie_health_wave_start(&self) -> Result<i32> {
        self.scalar(raw::pvzp_rs_world_zombie_health_wave_start)
    }

    pub fn total_zombie_health_in_wave(&self, wave: i32) -> Result<i32> {
        out_value(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_total_zombie_health_in_wave(self.raw.as_ptr(), wave, out) }
        })
    }

    /// # Safety
    /// The caller owns exclusive backend access; this operation clears native
    /// objects and recycles mower slots, invalidating outstanding entity views.
    pub unsafe fn set_scene(&mut self, scene: i32) -> Result<()> {
        // SAFETY: this world is live and the bridge validates the enum code.
        Error::from_status(unsafe { raw::pvzp_rs_world_set_scene(self.raw.as_ptr(), scene) })
    }

    pub fn validate_scene_layout(&self, scene: i32) -> Result<()> {
        // SAFETY: borrowed world and scalar input; read-only native validation.
        Error::from_status(unsafe { raw::pvzp_rs_world_validate_scene_layout(self.raw.as_ptr(), scene) })
    }
    /// Caller provides exclusive outer-frame access while terrain/resources change.
    pub unsafe fn set_scene_layout(&mut self, scene: i32) -> Result<()> {
        // SAFETY: caller excludes borrowed views; native validates inputs/dependencies.
        Error::from_status(unsafe { raw::pvzp_rs_world_set_scene_layout(self.raw.as_ptr(), scene) })
    }

    /// # Safety
    /// All native mower borrows must end before this operation frees their slots.
    pub unsafe fn clear_lawn_mowers(&mut self) -> Result<()> {
        // SAFETY: this world is live; the bridge removes native mower objects through their normal death path.
        Error::from_status(unsafe { raw::pvzp_rs_world_clear_lawn_mowers(self.raw.as_ptr()) })
    }

    pub fn set_spawn_allowed(&self, zombie_type: i32, allowed: bool) -> Result<()> {
        // SAFETY: this world is live and the bridge validates the enum code.
        Error::from_status(unsafe {
            raw::pvzp_rs_world_set_spawn_allowed(self.raw.as_ptr(), zombie_type, u8::from(allowed))
        })
    }

    pub fn set_spawn_entry(&self, wave: u32, slot: u32, zombie_type: i32) -> Result<()> {
        // SAFETY: this world is live and the bridge validates all indices and the enum code.
        Error::from_status(unsafe { raw::pvzp_rs_world_set_spawn_entry(self.raw.as_ptr(), wave, slot, zombie_type) })
    }

    pub fn pick_spawn_list(&self) -> Result<()> {
        // SAFETY: this world is live and the native action owns its internal updates.
        Error::from_status(unsafe { raw::pvzp_rs_world_pick_spawn_list(self.raw.as_ptr()) })
    }

    pub fn grid_to_pixel_x(&self, grid_x: i32, grid_y: i32) -> Result<i32> {
        out_value(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_grid_to_pixel_x(self.raw.as_ptr(), grid_x, grid_y, out) }
        })
    }

    pub fn grid_to_pixel_y(&self, grid_x: i32, grid_y: i32) -> Result<i32> {
        out_value(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_grid_to_pixel_y(self.raw.as_ptr(), grid_x, grid_y, out) }
        })
    }

    pub fn pos_y_based_on_row(&self, pos_x: f32, row: i32) -> Result<f32> {
        out_value(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_pos_y_based_on_row(self.raw.as_ptr(), pos_x, row, out) }
        })
    }

    pub fn is_pool_square(&self, grid_x: i32, grid_y: i32) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_is_pool_square(self.raw.as_ptr(), grid_x, grid_y, out) }
        })
    }
    pub fn grid_square_kind(&self, row: i32, col: i32) -> Result<i32> {
        out_value(|out| {
            // SAFETY: borrowed world and writable output; native validates indices.
            unsafe { raw::pvzp_rs_world_grid_square_kind(self.raw.as_ptr(), row, col, out) }
        })
    }

    pub fn grid_row_kind(&self, row: i32) -> Result<i32> {
        out_value(|out| {
            // SAFETY: borrowed world, writable output; native validates the six slots.
            unsafe { raw::pvzp_rs_world_grid_row_kind(self.raw.as_ptr(), row, out) }
        })
    }

    pub fn spawn_challenge_grave(&self, row: i32, col: i32) -> Result<()> {
        // SAFETY: live borrowed world; native checks pointers/grid/capacity before mutation.
        Error::from_status(unsafe { raw::pvzp_rs_world_spawn_challenge_grave(self.raw.as_ptr(), row, col) })
    }

    pub fn place_rake(&self, row: i32, col: i32) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: borrowed native world and output; native validates bounds and capacity.
            unsafe { raw::pvzp_rs_world_place_rake(self.raw.as_ptr(), row, col, out) }
        })
    }

    pub fn row_can_have_zombies(&self, row: i32) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_row_can_have_zombies(self.raw.as_ptr(), row, out) }
        })
    }

    pub fn seed_count(&self) -> Result<u32> {
        self.scalar(raw::pvzp_rs_world_seed_count)
    }

    /// Access an allocated packet even when beyond the displayed bank size.
    pub fn seed_storage_at(&self, index: u32) -> Result<SeedRef<'a>> {
        out_required_ptr(|out| {
            // SAFETY: live borrowed world and output; native bounds the fixed bank.
            unsafe { raw::pvzp_rs_world_seed_storage_at(self.raw.as_ptr(), index, out) }
        })
        .map(Borrowed::new)
    }

    pub fn set_seed_count(&self, count: u8) -> Result<()> {
        // SAFETY: native validates the range and updates the existing bank layout;
        // no packet is freed or reallocated.
        Error::from_status(unsafe { raw::pvzp_rs_world_set_seed_count(self.raw.as_ptr(), count) })
    }

    pub fn seed_at(&self, index: u32) -> Result<SeedRef<'a>> {
        out_required_ptr(|out| {
            // SAFETY: this world is live and `out` points to writable pointer storage.
            unsafe { raw::pvzp_rs_world_seed_at(self.raw.as_ptr(), index, out) }
        })
        .map(Borrowed::new)
    }

    pub fn current_plant_cost(&self, packet_type: i32, imitater_type: i32) -> Result<i32> {
        out_value(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_current_plant_cost(self.raw.as_ptr(), packet_type, imitater_type, out) }
        })
    }

    pub fn can_take_sun(&self, amount: i32) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_can_take_sun(self.raw.as_ptr(), amount, out) }
        })
    }

    pub fn take_sun(&self, amount: i32) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_take_sun(self.raw.as_ptr(), amount, out) }
        })
    }

    pub fn can_plant_at(&self, packet_type: i32, grid_x: i32, grid_y: i32) -> Result<i32> {
        out_value(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { raw::pvzp_rs_world_can_plant_at(self.raw.as_ptr(), packet_type, grid_x, grid_y, out) }
        })
    }

    pub fn new_plant(
        &self, grid_x: i32, grid_y: i32, packet_type: i32, imitater_type: i32,
    ) -> Result<Option<PlantRef<'a>>> {
        out_optional_ptr(|out| {
            // SAFETY: this world is live and `out` points to writable pointer storage.
            unsafe { raw::pvzp_rs_world_new_plant(self.raw.as_ptr(), grid_x, grid_y, packet_type, imitater_type, out) }
        })
        .map(|ptr| ptr.map(Borrowed::new))
    }

    pub fn add_plant(
        &self, grid_x: i32, grid_y: i32, packet_type: i32, imitater_type: i32,
    ) -> Result<Option<PlantRef<'a>>> {
        out_optional_ptr(|out| {
            // SAFETY: this world is live and `out` points to writable pointer storage.
            unsafe { raw::pvzp_rs_world_add_plant(self.raw.as_ptr(), grid_x, grid_y, packet_type, imitater_type, out) }
        })
        .map(|ptr| ptr.map(Borrowed::new))
    }

    pub fn add_zombie_in_row(&self, zombie_type: i32, row: i32, from_wave: i32) -> Result<Option<ZombieRef<'a>>> {
        out_optional_ptr(|out| {
            // SAFETY: this world is live and `out` points to writable pointer storage.
            unsafe { raw::pvzp_rs_world_add_zombie_in_row(self.raw.as_ptr(), zombie_type, row, from_wave, out) }
        })
        .map(|ptr| ptr.map(Borrowed::new))
    }

    pub fn place_zombie(&self, zombie_type: i32, grid_x: i32, grid_y: i32) -> Result<ZombieRef<'a>> {
        out_required_ptr(|out| {
            // SAFETY: this world is live, arguments are copied, and `out` is writable.
            unsafe { raw::pvzp_rs_world_place_zombie(self.raw.as_ptr(), zombie_type, grid_x, grid_y, out) }
        })
        .map(Borrowed::new)
    }

    pub fn add_ladder(&self, grid_x: i32, grid_y: i32) -> Result<Option<GridItemRef<'a>>> {
        self.add_grid_item(raw::pvzp_rs_world_add_ladder, grid_x, grid_y)
    }

    pub fn add_crater(&self, grid_x: i32, grid_y: i32) -> Result<Option<GridItemRef<'a>>> {
        self.add_grid_item(raw::pvzp_rs_world_add_crater, grid_x, grid_y)
    }

    pub fn add_gravestone(&self, grid_x: i32, grid_y: i32) -> Result<Option<GridItemRef<'a>>> {
        self.add_grid_item(raw::pvzp_rs_world_add_gravestone, grid_x, grid_y)
    }

    fn add_grid_item(
        &self,
        add: unsafe extern "C" fn(
            *mut raw::pvzp_rs_world,
            i32,
            i32,
            *mut *mut raw::pvzp_rs_grid_item,
        ) -> raw::pvzp_rs_status,
        grid_x: i32, grid_y: i32,
    ) -> Result<Option<GridItemRef<'a>>> {
        out_optional_ptr(|out| {
            // SAFETY: this world is live and `out` points to writable pointer storage.
            unsafe { add(self.raw.as_ptr(), grid_x, grid_y, out) }
        })
        .map(|ptr| ptr.map(Borrowed::new))
    }

    fn scalar<T>(
        &self, get: unsafe extern "C" fn(*const raw::pvzp_rs_world, *mut T) -> raw::pvzp_rs_status,
    ) -> Result<T> {
        out_value(|out| {
            // SAFETY: this world is live and `out` points to writable storage.
            unsafe { get(self.raw.as_ptr(), out) }
        })
    }
}

macro_rules! pool {
    (
        $name:ident, $raw_pool:ty, $raw_object:ty, $object_ref:ident,
        $max:path, $active:path, $capacity:path, $free:path, $next_key:path, $slot:path,
        $get:path, $try_get:path, $get_id:path
    ) => {
        #[derive(Clone, Copy)]
        pub struct $name<'a> {
            ptr: NonNull<$raw_pool>,
            _borrow: PhantomData<&'a World<'a>>,
        }

        impl<'a> $name<'a> {
            fn new(ptr: NonNull<$raw_pool>) -> Self {
                Self {
                    ptr,
                    _borrow: PhantomData,
                }
            }

            /// Rebinds a pool whose owning Board is protected by the caller.
            /// # Safety
            /// The pool must remain alive without update/reclamation for `'a`.
            #[doc(hidden)]
            pub unsafe fn from_non_null(ptr: NonNull<$raw_pool>) -> Self {
                Self::new(ptr)
            }
            #[doc(hidden)]
            pub const fn as_non_null(self) -> NonNull<$raw_pool> {
                self.ptr
            }

            pub fn max_used_count(self) -> u32 {
                // SAFETY: this non-null pool remains borrowed from its live Board.
                unsafe { $max(self.ptr.as_ptr()) }
            }

            pub fn active_count(self) -> u32 {
                // SAFETY: this non-null pool remains borrowed from its live Board.
                unsafe { $active(self.ptr.as_ptr()) }
            }

            pub fn capacity(self) -> u32 {
                // SAFETY: this non-null pool remains borrowed from its live Board.
                unsafe { $capacity(self.ptr.as_ptr()) }
            }

            pub fn free_list_head(self) -> u32 {
                // SAFETY: this non-null pool remains borrowed from its live Board.
                unsafe { $free(self.ptr.as_ptr()) }
            }

            pub fn next_id_key(self) -> u32 {
                // SAFETY: this non-null pool remains borrowed from its live Board.
                unsafe { $next_key(self.ptr.as_ptr()) }
            }

            pub fn slot(self, index: u32) -> Result<PoolSlot> {
                let mut active = 0;
                let mut id = 0;
                let mut next_free = 0;
                // SAFETY: this pool is live and all output pointers are writable local storage.
                Error::from_status(unsafe {
                    $slot(
                        self.ptr.as_ptr(),
                        index,
                        &raw mut active,
                        &raw mut id,
                        &raw mut next_free,
                    )
                })?;
                Ok(PoolSlot {
                    active: active != 0,
                    id,
                    next_free,
                })
            }

            pub fn get(self, index: i32) -> Result<Option<$object_ref<'a>>> {
                out_optional_ptr(|out| {
                    // SAFETY: this pool is live and `out` is writable.
                    unsafe { $get(self.ptr.as_ptr(), index, out) }
                })
                .map(|ptr| ptr.map(Borrowed::new))
            }

            pub fn try_to_get(self, id: u32) -> Option<$object_ref<'a>> {
                if id >> 16 == 0 {
                    return None;
                }
                // SAFETY: this pool is live; DataArrayTryToGet checks capacity and the complete ID before indexing.
                NonNull::new(unsafe { $try_get(self.ptr.as_ptr(), id) }).map(Borrowed::new)
            }

            pub fn get_id(self, object: $object_ref<'a>) -> Result<u32> {
                out_value(|out| {
                    // SAFETY: the pool and object share the same world borrow.
                    unsafe { $get_id(self.ptr.as_ptr(), object.as_ptr(), out) }
                })
            }
        }
    };
}

pool!(
    PlantPool,
    raw::pvzp_rs_plant_pool,
    raw::pvzp_rs_plant,
    PlantRef,
    raw::pvzp_rs_plant_pool_max_used_count,
    raw::pvzp_rs_plant_pool_active_count,
    raw::pvzp_rs_plant_pool_capacity,
    raw::pvzp_rs_plant_pool_free_list_head,
    raw::pvzp_rs_plant_pool_next_id_key,
    raw::pvzp_rs_plant_pool_slot,
    raw::pvzp_rs_plant_pool_get,
    raw::pvzp_rs_plant_pool_try_to_get,
    raw::pvzp_rs_plant_pool_get_id
);
pool!(
    ZombiePool,
    raw::pvzp_rs_zombie_pool,
    raw::pvzp_rs_zombie,
    ZombieRef,
    raw::pvzp_rs_zombie_pool_max_used_count,
    raw::pvzp_rs_zombie_pool_active_count,
    raw::pvzp_rs_zombie_pool_capacity,
    raw::pvzp_rs_zombie_pool_free_list_head,
    raw::pvzp_rs_zombie_pool_next_id_key,
    raw::pvzp_rs_zombie_pool_slot,
    raw::pvzp_rs_zombie_pool_get,
    raw::pvzp_rs_zombie_pool_try_to_get,
    raw::pvzp_rs_zombie_pool_get_id
);
pool!(
    ProjectilePool,
    raw::pvzp_rs_projectile_pool,
    raw::pvzp_rs_projectile,
    ProjectileRef,
    raw::pvzp_rs_projectile_pool_max_used_count,
    raw::pvzp_rs_projectile_pool_active_count,
    raw::pvzp_rs_projectile_pool_capacity,
    raw::pvzp_rs_projectile_pool_free_list_head,
    raw::pvzp_rs_projectile_pool_next_id_key,
    raw::pvzp_rs_projectile_pool_slot,
    raw::pvzp_rs_projectile_pool_get,
    raw::pvzp_rs_projectile_pool_try_to_get,
    raw::pvzp_rs_projectile_pool_get_id
);
pool!(
    GridItemPool,
    raw::pvzp_rs_grid_item_pool,
    raw::pvzp_rs_grid_item,
    GridItemRef,
    raw::pvzp_rs_grid_item_pool_max_used_count,
    raw::pvzp_rs_grid_item_pool_active_count,
    raw::pvzp_rs_grid_item_pool_capacity,
    raw::pvzp_rs_grid_item_pool_free_list_head,
    raw::pvzp_rs_grid_item_pool_next_id_key,
    raw::pvzp_rs_grid_item_pool_slot,
    raw::pvzp_rs_grid_item_pool_get,
    raw::pvzp_rs_grid_item_pool_try_to_get,
    raw::pvzp_rs_grid_item_pool_get_id
);
pool!(
    ItemPool,
    raw::pvzp_rs_item_pool,
    raw::pvzp_rs_item,
    ItemRef,
    raw::pvzp_rs_item_pool_max_used_count,
    raw::pvzp_rs_item_pool_active_count,
    raw::pvzp_rs_item_pool_capacity,
    raw::pvzp_rs_item_pool_free_list_head,
    raw::pvzp_rs_item_pool_next_id_key,
    raw::pvzp_rs_item_pool_slot,
    raw::pvzp_rs_item_pool_get,
    raw::pvzp_rs_item_pool_try_to_get,
    raw::pvzp_rs_item_pool_get_id
);

/// The caller must ensure the call initializes the complete output before returning.
pub(super) unsafe fn out_native_value<T>(call: impl FnOnce(*mut T)) -> T {
    let mut value = std::mem::MaybeUninit::uninit();
    call(value.as_mut_ptr());
    // SAFETY: required by the caller contract above.
    unsafe { value.assume_init() }
}

pub(super) fn out_value<T>(call: impl FnOnce(*mut T) -> raw::pvzp_rs_status) -> Result<T> {
    let mut value = std::mem::MaybeUninit::uninit();
    Error::from_status(call(value.as_mut_ptr()))?;
    // SAFETY: a successful bridge call initializes its required out parameter.
    Ok(unsafe { value.assume_init() })
}

pub(super) fn out_optional_ptr<T>(call: impl FnOnce(*mut *mut T) -> raw::pvzp_rs_status) -> Result<Option<NonNull<T>>> {
    let mut ptr = std::ptr::null_mut();
    Error::from_status(call(&raw mut ptr))?;
    Ok(NonNull::new(ptr))
}

fn out_required_ptr<T>(call: impl FnOnce(*mut *mut T) -> raw::pvzp_rs_status) -> Result<NonNull<T>> {
    out_optional_ptr(call)?.ok_or(Error::NullPointer)
}

pub(super) fn out_bool(call: impl FnOnce(*mut u8) -> raw::pvzp_rs_status) -> Result<bool> {
    out_value(call).map(|value: u8| value != 0)
}

pool!(
    MowerPool,
    raw::pvzp_rs_mower_pool,
    raw::pvzp_rs_mower,
    MowerRef,
    raw::pvzp_rs_mower_pool_max_used_count,
    raw::pvzp_rs_mower_pool_active_count,
    raw::pvzp_rs_mower_pool_capacity,
    raw::pvzp_rs_mower_pool_free_list_head,
    raw::pvzp_rs_mower_pool_next_id_key,
    raw::pvzp_rs_mower_pool_slot,
    raw::pvzp_rs_mower_pool_get,
    raw::pvzp_rs_mower_pool_try_to_get,
    raw::pvzp_rs_mower_pool_get_id
);

impl World<'_> {
    pub fn cursor_position(&self) -> Result<(i32, i32)> {
        let (mut x, mut y) = (0, 0);
        // SAFETY: live world, writable scalar outputs.
        Error::from_status(unsafe { raw::pvzp_rs_cursor_position(self.raw.as_ptr(), &mut x, &mut y) })?;
        Ok((x, y))
    }
    pub fn cursor_object_position(&self) -> Result<(i32, i32)> {
        let (mut x, mut y) = (0, 0);
        // SAFETY: live world, writable scalar outputs.
        Error::from_status(unsafe { raw::pvzp_rs_cursor_object_position(self.raw.as_ptr(), &mut x, &mut y) })?;
        Ok((x, y))
    }
    pub fn cursor_seed_slot(&self) -> Result<i32> {
        self.scalar(raw::pvzp_rs_cursor_seed_slot)
    }
    pub fn cursor_grid_row(&self) -> Result<i32> {
        self.scalar(raw::pvzp_rs_cursor_grid_row)
    }
}

impl<'a> World<'a> {
    pub fn select_cob(&self, id: u32) -> Result<()> {
        // SAFETY: live world; bridge validates live ready cannon ID.
        Error::from_status(unsafe { raw::pvzp_rs_select_cob(self.raw.as_ptr(), id) })
    }
    pub fn new_plant_native_grid(&self, x: i32, y: i32, kind: i32, imitater: i32) -> Result<Option<PlantRef<'a>>> {
        out_optional_ptr(|out| {
            // SAFETY: live world, writable output; bridge validates physical bounds/kinds/pool.
            unsafe { raw::pvzp_rs_new_plant_native_grid(self.raw.as_ptr(), x, y, kind, imitater, out) }
        })
        .map(|p| p.map(Borrowed::new))
    }
}

impl World<'_> {
    pub fn board_view(&self) -> Result<(i32, i32)> {
        let (mut x, mut y) = (0, 0);
        // SAFETY: live world and writable outputs.
        Error::from_status(unsafe { raw::pvzp_rs_board_view(self.raw.as_ptr(), &mut x, &mut y) })?;
        Ok((x, y))
    }
    pub fn set_board_view(&self, x: i32, y: i32) -> Result<()> {
        // SAFETY: live native widget, scalar coordinates.
        Error::from_status(unsafe { raw::pvzp_rs_set_board_view(self.raw.as_ptr(), x, y) })
    }
}
pub fn animation_skip() -> Result<bool> {
    // SAFETY: bridge validates App and initializes this scalar output.
    out_value(|out| unsafe { raw::pvzp_rs_animation_skip(out) }).map(|v: u8| v != 0)
}
pub fn set_animation_skip(v: bool) -> Result<()> {
    // SAFETY: bridge checks the current App before writing its native flag.
    Error::from_status(unsafe { raw::pvzp_rs_set_animation_skip(u8::from(v)) })
}
pub fn house_pause() -> Result<bool> {
    // SAFETY: scalar native rule state with a writable output.
    out_value(|out| unsafe { raw::pvzp_rs_house_pause(out) }).map(|v: u8| v != 0)
}
pub fn set_house_pause(v: bool) -> Result<()> {
    // SAFETY: game-thread rule update; no callback or ownership transfer.
    Error::from_status(unsafe { raw::pvzp_rs_set_house_pause(u8::from(v)) })
}
pub fn take_house() -> Result<bool> {
    // SAFETY: game-thread take of the native occurrence flag.
    out_value(|out| unsafe { raw::pvzp_rs_take_house(out) }).map(|v: u8| v != 0)
}
pub fn scene_music(tune: i32) -> Result<()> {
    // SAFETY: bridge validates App, Music owner and native tune domain.
    Error::from_status(unsafe { raw::pvzp_rs_scene_music(tune) })
}

pub fn clear_fast_mode() -> Result<()> {
    // SAFETY: game-thread scalar flag update, independent of rendering or callbacks.
    Error::from_status(unsafe { raw::pvzp_rs_clear_fast_mode() })
}
