use super::{
    Challenge, CursorPreview, DataArray, Plant, Projectile, SeedBank, Zombie, addr_at_mut, read_unaligned_at,
    write_unaligned_at,
};

#[repr(C)]
pub(crate) struct Board {
    _data: [u8; 0x57b0],
}

impl Board {
    // Native initialisation and draw: 0x4080FF/0x418B99 (+55F1),
    // 0x407E0A/0x41A2DB (+55F4); original menu button is Board+148.
    pub(crate) unsafe fn hud_field_ptr(this: *mut Self, field: rsvz_backend_api::HudField) -> *mut u8 {
        match field {
            rsvz_backend_api::HudField::ShovelVisible => unsafe { this.cast::<u8>().add(0x55f1) },
            rsvz_backend_api::HudField::CoinBankFadeCountdown => unsafe { this.cast::<u8>().add(0x55f4) },
            rsvz_backend_api::HudField::MenuButtonHidden => unsafe {
                let button: *mut u8 = read_unaligned_at(this, 0x148);
                if button.is_null() {
                    std::ptr::null_mut()
                } else {
                    button.add(0xf9)
                }
            },
        }
    }

    pub(crate) unsafe fn message_widget(this: *const Self) -> *mut super::MessageWidget {
        unsafe { read_unaligned_at(this, 0x140) }
    }
    // Inlined HasLevelAwardDropped checks in LawnMower::Die 0x458D54..0x458D6D.
    pub(crate) unsafe fn level_award_dropped(this: *const Self) -> bool {
        unsafe {
            read_unaligned_at::<u8>(this, 0x560c) != 0
                || read_unaligned_at::<i32>(this, 0x5604) > 0
                || read_unaligned_at::<i32>(this, 0x5600) >= 0
        }
    }
    // Six native countdown slots at Board+0x624 (PT NoIceTrail and native ice update).
    // Caller must establish a live Board and row < 6.
    pub(crate) unsafe fn set_ice_trail_countdown(this: *mut Self, row: u8, value: i32) {
        unsafe { write_unaligned_at(this, 0x624 + usize::from(row) * 4, value) }
    }
    // Board::UpdateIce 0x41412C/0x414171: row < 6 is established by the caller.
    pub(crate) unsafe fn ice_particle_id(this: *const Self, row: usize) -> u32 {
        unsafe { read_unaligned_at(this, 0x63c + row * 4) }
    }
    pub(crate) unsafe fn clear_ice_row(this: *mut Self, row: usize) {
        unsafe {
            write_unaligned_at(this, 0x624 + row * 4, 0_i32);
            write_unaligned_at(this, 0x60c + row * 4, 800_i32);
            write_unaligned_at(this, 0x63c + row * 4, 0_u32);
        }
    }
    // mZombiesInWave occupies 0x6B4..0x54D4: 100 waves of 50 i32 slots.
    // PickZombieWaves (0x4092E0) selects the level's actual 8/10/12/20/30/40 waves.
    pub(crate) const SPAWN_WAVE_CAPACITY: usize = (0x54d4 - 0x6b4) / (50 * 4);

    // MouseDownWithTool 0x4111D8 / 0x41123F and SetTutorialState 0x415902.
    pub(crate) unsafe fn tutorial_state(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5584) }
    }
    pub(crate) unsafe fn increment_plants_shoveled(this: *mut Self) {
        unsafe {
            let count: i32 = read_unaligned_at(this, 0x579c);
            write_unaligned_at(this, 0x579c, count.wrapping_add(1));
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn draw_count(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5570) }
    }
    #[inline(always)]
    pub(crate) unsafe fn lawn_mower_count(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x110) }
    }
    #[inline(always)]
    pub(crate) unsafe fn challenge(this: *const Self) -> *mut Challenge {
        unsafe { read_unaligned_at(this, 0x160) }
    }
    #[inline(always)]
    pub(crate) unsafe fn cursor_object(this: *const Self) -> *mut CursorObject {
        unsafe { read_unaligned_at(this, 0x138) }
    }
    #[inline(always)]
    pub(crate) unsafe fn cursor_preview(this: *const Self) -> *mut CursorPreview {
        unsafe { read_unaligned_at(this, 0x13c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn plants(this: *mut Self) -> *mut DataArray<Plant> {
        unsafe { addr_at_mut(this, 0xac) }
    }
    #[inline(always)]
    pub(crate) unsafe fn projectiles(this: *mut Self) -> *mut DataArray<Projectile> {
        unsafe { addr_at_mut(this, 0xc8) }
    }
    #[inline(always)]
    pub(crate) unsafe fn coins(this: *mut Self) -> *mut DataArray<Coin> {
        unsafe { addr_at_mut(this, 0xe4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn lawn_mowers(this: *mut Self) -> *mut DataArray<LawnMower> {
        unsafe { addr_at_mut(this, 0x100) }
    }
    #[inline(always)]
    pub(crate) unsafe fn zombie_list(this: *mut Self) -> *mut u32 {
        unsafe { addr_at_mut(this, 0x6b4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn zombies(this: *mut Self) -> *mut DataArray<Zombie> {
        unsafe { addr_at_mut(this, 0x90) }
    }
    #[inline(always)]
    pub(crate) unsafe fn zombie_type_list(this: *mut Self) -> *mut u8 {
        unsafe { this.cast::<u8>().add(0x54d4).cast::<u8>() }
    }
    #[inline(always)]
    pub(crate) unsafe fn grid_items(this: *mut Self) -> *mut DataArray<GridItem> {
        unsafe { addr_at_mut(this, 0x11c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn seed_bank(this: *const Self) -> *mut SeedBank {
        unsafe { read_unaligned_at(this, 0x144) }
    }
    #[inline(always)]
    pub(crate) unsafe fn cut_scene(this: *const Self) -> *mut CutScene {
        unsafe { read_unaligned_at(this, 0x15c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn paused(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x164) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn block_types(this: *mut Self) -> *mut i32 {
        unsafe { addr_at_mut(this, 0x168) }
    }
    #[inline(always)]
    pub(crate) unsafe fn row_types(this: *mut Self) -> *mut i32 {
        unsafe { addr_at_mut(this, 0x5d8) }
    }
    #[inline(always)]
    pub(crate) unsafe fn ice_min_x(this: *const Self, row: usize) -> i32 {
        unsafe { read_unaligned_at(this, 0x60c + row * 4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn ice_timer(this: *const Self, row: usize) -> i32 {
        unsafe { read_unaligned_at(this, 0x624 + row * 4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn zombie_in_wave(this: *const Self, wave: usize, slot: usize) -> i32 {
        unsafe { read_unaligned_at(this, 0x6b4 + (wave * 50 + slot) * 4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn zombie_allowed(this: *const Self, raw_kind: usize) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x54d4 + raw_kind) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn bonus_lawn_mowers_remaining_mut(this: *mut Self) -> *mut i32 {
        unsafe { addr_at_mut(this, 0x608) }
    }
    #[inline(always)]
    pub(crate) unsafe fn scene(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x554c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn scene_mut(this: *mut Self) -> *mut i32 {
        unsafe { addr_at_mut(this, 0x554c) }
    }
    // Decomp `Board::mNumSunsFallen` (+0x553c) feeds natural sun drop countdown aging.
    #[inline(always)]
    pub(crate) unsafe fn num_suns_fallen(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x553c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn sun_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5538) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_sun_countdown(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x5538, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_num_suns_fallen(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x553c, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn sun(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5560) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_sun(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x5560, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn num_waves(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5564) }
    }
    #[inline(always)]
    pub(crate) unsafe fn clock(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5568) }
    }
    #[inline(always)]
    pub(crate) unsafe fn clock_mut(this: *mut Self) -> *mut i32 {
        unsafe { addr_at_mut(this, 0x5568) }
    }
    #[inline(always)]
    pub(crate) unsafe fn effect_counter_mut(this: *mut Self) -> *mut i32 {
        unsafe { addr_at_mut(this, 0x556c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn current_wave(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x557c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn time_stop_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5748) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_zombie_refresh_hp(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x5594, value) }
    }
    #[inline(always)]
    // Native refresh gate compares this scalar at 0x413F42.
    pub(crate) unsafe fn zombie_refresh_health_threshold(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5594) }
    }
    pub(crate) unsafe fn zombie_health_wave_start(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5598) }
    }
    #[inline(always)]
    pub(crate) unsafe fn refresh_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x559c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_refresh_countdown(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x559c, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn initial_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x55a0) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_initial_countdown(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x55a0, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn huge_wave_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x55a4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn level_complete(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x55fc) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn level_end_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5604) }
    }
    #[inline(always)]
    pub(crate) unsafe fn next_survival_stage_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5604) }
    }
    #[inline(always)]
    pub(crate) unsafe fn board_rand_seed(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x561c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_board_rand_seed(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x561c, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_pool_particle_system_id(this: *mut Self, value: u32) {
        unsafe { write_unaligned_at(this, 0x5620, value) }
    }
    // Decomp `Board::mPlantsEaten`; `Board::KillAllPlantsInRadius` increments this
    // before killing each plant hit by a Jack-in-the-box explosion.
    #[inline(always)]
    pub(crate) unsafe fn plants_eaten_mut(this: *mut Self) -> *mut i32 {
        unsafe { addr_at_mut(this, 0x5798) }
    }
}

#[repr(C)]
pub(crate) struct LawnMower {
    _data: [u8; 0x48],
}

impl LawnMower {
    pub(crate) unsafe fn row(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x14) }
    }
    pub(crate) unsafe fn state(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x2c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_dead(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x30) != 0 }
    }
}

#[repr(C)]
pub(crate) struct Coin {
    _data: [u8; 0xd8],
}

impl Coin {
    #[inline(always)]
    pub(crate) unsafe fn pos_x(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x24) }
    }
    #[inline(always)]
    pub(crate) unsafe fn pos_y(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x28) }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_dead(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x38) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_being_collected(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x50) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn coin_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x58) }
    }
}

#[repr(C)]
pub(crate) struct GridItem {
    _data: [u8; 0xec],
}

impl GridItem {
    // GridItem constructor 0x44CFDE..0x44CFE4 and DrawScaryPot 0x44DC2C,
    // 0x44DC3A, 0x44DCA4 verify these three distinct scalar fields.
    pub(crate) unsafe fn vase_content_kind(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x44) }
    }
    pub(crate) unsafe fn vase_plant_kind(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x40) }
    }
    pub(crate) unsafe fn vase_zombie_kind(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x3c) }
    }

    #[inline(always)]
    pub(crate) unsafe fn grid_item_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x8) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_grid_item_type(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x8, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn grid_x(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x10) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_grid_x(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x10, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn grid_y(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x14) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_grid_y(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x14, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn grid_item_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x18) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_grid_item_counter(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x18, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_render_order(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x1c, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_dead(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x20) != 0 }
    }
}

#[repr(C)]
pub(crate) struct Reanimation {
    _data: [u8; 0xa0],
}

impl Reanimation {
    // Temporary same-offset alias for existing cob animation code.
    #[inline(always)]
    pub(crate) unsafe fn anim_time(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn circulation_rate(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn anim_rate(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x8) }
    }
    #[inline(always)]
    pub(crate) unsafe fn loop_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x10) }
    }
    #[inline(always)]
    pub(crate) unsafe fn frame_start(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x18) }
    }
    #[inline(always)]
    pub(crate) unsafe fn frame_count(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x1c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn last_frame_time(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x94) }
    }
}

#[repr(C)]
pub(crate) struct ParticleSystem {
    _data: [u8; 0x2c],
}

impl ParticleSystem {
    #[inline(always)]
    pub(crate) unsafe fn particle_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x0) }
    }
    #[inline(always)]
    pub(crate) unsafe fn dead(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x1c) != 0 }
    }
}

#[repr(C)]
pub(crate) struct CutScene {
    _data: [u8; 0],
}

impl CutScene {
    #[inline(always)]
    pub(crate) unsafe fn seed_choosing(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x2c) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_zombie_preview_created(this: *mut Self, value: bool) {
        unsafe { write_unaligned_at(this, 0x35, u8::from(value)) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_lawn_items_placed(this: *mut Self, value: bool) {
        unsafe { write_unaligned_at(this, 0x36, u8::from(value)) }
    }
}

#[repr(C)]
pub(crate) struct CursorObject {
    _data: [u8; 0x4c],
}

impl CursorObject {
    pub(crate) unsafe fn coin_id(this: *const Self) -> u32 {
        unsafe { read_unaligned_at(this, 0x34) }
    }
    pub(crate) unsafe fn seed_kind(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x28) }
    }
    pub(crate) unsafe fn imitater_kind(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x2c) }
    }
    // CursorObject.h and native MouseDownWithPlant / MouseDownCobcannonFire offsets.
    pub(crate) unsafe fn seed_bank_index(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x24) }
    }
    pub(crate) unsafe fn set_seed_bank_index(this: *mut Self, index: i32) {
        unsafe { write_unaligned_at(this, 0x24, index) }
    }
    pub(crate) unsafe fn cob_cannon_id(this: *const Self) -> u32 {
        unsafe { read_unaligned_at(this, 0x40) }
    }
    #[inline(always)]
    pub(crate) unsafe fn cursor_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x30) }
    }
}
