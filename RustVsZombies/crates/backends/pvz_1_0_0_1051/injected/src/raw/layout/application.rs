use super::{
    Board, DataArray, ParticleSystem, Reanimation, SeedChooserScreen, TopMouseWindow, addr_at_mut, c_void,
    read_unaligned_at, write_unaligned_at,
};

#[repr(C)]
pub(crate) struct Challenge {
    _data: [u8; 0],
}

impl Challenge {
    #[inline(always)]
    pub(crate) unsafe fn endless_rounds(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x6c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn endless_rounds_mut(this: *mut Self) -> *mut i32 {
        unsafe { addr_at_mut(this, 0x6c) }
    }
}

#[repr(C)]
pub(crate) struct CursorPreview {
    _data: [u8; 0],
}

#[repr(C)]
pub(crate) struct EffectSystem {
    _data: [u8; 0],
}

impl EffectSystem {
    #[inline(always)]
    pub(crate) unsafe fn particle_system_holder(this: *const Self) -> *mut ParticleSystemHolder {
        unsafe { read_unaligned_at(this, 0x0) }
    }
    #[inline(always)]
    pub(crate) unsafe fn reanimation_holder(this: *const Self) -> *mut ReanimationHolder {
        unsafe { read_unaligned_at(this, 0x8) }
    }
}

#[repr(C)]
pub(crate) struct ParticleSystemHolder {
    _data: [u8; 0],
}

impl ParticleSystemHolder {
    // Inlined DataArray::TryToGet in Board::UpdateIce 0x41414B..0x414169.
    pub(crate) unsafe fn try_get(this: *const Self, id: u32) -> *mut ParticleSystem {
        unsafe {
            if this.is_null() || id == 0 {
                return std::ptr::null_mut();
            }
            let index = (id & 0xffff) as usize;
            if index >= read_unaligned_at::<u32>(this, 0x8) as usize {
                return std::ptr::null_mut();
            }
            let data = Self::particle_systems(this);
            if data.is_null() {
                return std::ptr::null_mut();
            }
            let particle = data.add(index);
            if read_unaligned_at::<u32>(particle, 0x28) == id {
                particle
            } else {
                std::ptr::null_mut()
            }
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn particle_systems(this: *const Self) -> *mut ParticleSystem {
        unsafe { read_unaligned_at(this, 0x0) }
    }
    #[inline(always)]
    pub(crate) unsafe fn particle_system_count_max(this: *const Self) -> u32 {
        unsafe { read_unaligned_at(this, 0x4) }
    }
}

#[repr(C)]
pub(crate) struct LawnApp {
    _data: [u8; 0],
}

impl LawnApp {
    pub(crate) unsafe fn set_replay_game_mode(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x7f8, value) }
    }
    pub(crate) unsafe fn cursor_num(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x4b0) }
    }
    // LawnApp constructor 0x44EC0C/0x44EC16 initializes the actual canvas dimensions.
    pub(crate) unsafe fn canvas_width(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xc0) }
    }
    pub(crate) unsafe fn canvas_height(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xc4) }
    }
    // SexyAppBase constructor 0x544124 initializes +0x454 to 10;
    // UpdateAppStep 0x54C4F8 reads it as the native frame duration.
    pub(crate) unsafe fn frame_duration_ms(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x454) }
    }
    // Board::SetDanceMode 0x41B040 writes App+0x8BC; the Board copy is +0x5765.
    pub(crate) unsafe fn dance_mode(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x8bc) != 0 }
    }
    pub(crate) unsafe fn set_dance_mode(this: *mut Self, enabled: bool) {
        unsafe { write_unaligned_at(this, 0x8bc, u8::from(enabled)) }
    }
    #[inline(always)]
    pub(crate) unsafe fn mouse_window(this: *const Self) -> *mut MouseWindow {
        unsafe { read_unaligned_at(this, 0x320) }
    }
    // 1051 evidence: SexyAppBase::mHWnd, AvZ2 APvzBase::Hwnd(), and AvZ1 address tables
    // all identify the main PvZ window handle at LawnApp + 0x350.
    #[inline(always)]
    pub(crate) unsafe fn hwnd(this: *const Self) -> *mut c_void {
        unsafe { read_unaligned_at(this, 0x350) }
    }
    #[inline(always)]
    pub(crate) unsafe fn update_multiplier(this: *const Self) -> f64 {
        unsafe { read_unaligned_at(this, 0x490) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_update_multiplier(this: *mut Self, value: f64) {
        unsafe { write_unaligned_at(this, 0x490, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn board(this: *const Self) -> *mut Board {
        unsafe { read_unaligned_at(this, 0x768) }
    }
    #[inline(always)]
    pub(crate) unsafe fn seed_chooser(this: *const Self) -> *mut SeedChooserScreen {
        unsafe { read_unaligned_at(this, 0x774) }
    }
    #[inline(always)]
    pub(crate) unsafe fn game_mode(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x7f8) }
    }
    #[inline(always)]
    pub(crate) unsafe fn game_ui(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x7fc) }
    }
    // Objdump SeedPacket::CanPickUp/MouseDown: LawnApp+0x814 gates EasyPlantingCheat.
    #[inline(always)]
    pub(crate) unsafe fn is_easy_planting_cheat_enabled(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x814) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_easy_planting_cheat(this: *mut Self, value: bool) {
        unsafe { write_unaligned_at(this, 0x814, u8::from(value)) }
    }
    #[inline(always)]
    pub(crate) unsafe fn effect_system(this: *const Self) -> *mut EffectSystem {
        unsafe { read_unaligned_at(this, 0x820) }
    }
    #[inline(always)]
    pub(crate) unsafe fn player_info(this: *const Self) -> *mut PlayerInfo {
        unsafe { read_unaligned_at(this, 0x82c) }
    }
    pub(crate) unsafe fn game_selector(this: *const Self) -> *mut GameSelector {
        unsafe { read_unaligned_at(this, 0x770) }
    }
    #[inline(always)]
    pub(crate) unsafe fn app_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x838) }
    }
    #[inline(always)]
    pub(crate) unsafe fn app_counter_mut(this: *mut Self) -> *mut i32 {
        unsafe { addr_at_mut(this, 0x838) }
    }
    #[inline(always)]
    pub(crate) unsafe fn music(this: *const Self) -> *mut u8 {
        unsafe { read_unaligned_at(this, 0x83c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn app_rand_seed(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x870) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_app_rand_seed(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x870, value) }
    }
}

#[repr(C)]
pub(crate) struct MouseWindow {
    _data: [u8; 0],
}

impl MouseWindow {
    pub(crate) unsafe fn x(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xe0) }
    }
    pub(crate) unsafe fn y(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xe4) }
    }
    pub(crate) unsafe fn buttons(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xe8) }
    }
    #[inline(always)]
    pub(crate) unsafe fn top_window(this: *const Self) -> *mut TopMouseWindow {
        unsafe { read_unaligned_at(this, 0x94) }
    }
}

#[repr(C)]
pub(crate) struct PlayerInfo {
    _data: [u8; 0],
}

#[repr(C)]
pub(crate) struct GameSelector {
    _data: [u8; 0],
}

impl PlayerInfo {
    pub(crate) unsafe fn finished_adventure(this: *const Self) -> u32 {
        unsafe { read_unaligned_at(this, 0x2c) }
    }
    pub(crate) unsafe fn set_finished_adventure(this: *mut Self, value: u32) {
        unsafe { write_unaligned_at(this, 0x2c, value) }
    }
    pub(crate) unsafe fn challenge_record(this: *const Self, index: usize) -> u32 {
        unsafe { read_unaligned_at(this, 0x30 + index * 4) }
    }
    pub(crate) unsafe fn set_challenge_record(this: *mut Self, index: usize, value: u32) {
        unsafe { write_unaligned_at(this, 0x30 + index * 4, value) }
    }
    pub(crate) unsafe fn purchase(this: *const Self, index: usize) -> u32 {
        unsafe { read_unaligned_at(this, 0x1c0 + index * 4) }
    }
    pub(crate) unsafe fn set_purchase(this: *mut Self, index: usize, value: u32) {
        unsafe { write_unaligned_at(this, 0x1c0 + index * 4, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn coins(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x28) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_coins(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x28, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn id(this: *const Self) -> u32 {
        unsafe { read_unaligned_at(this, 0x20) }
    }
    #[inline(always)]
    pub(crate) unsafe fn level(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x24) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_level(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x24, value) }
    }
}

#[repr(C)]
pub(crate) struct ReanimationHolder {
    _data: [u8; 0],
}

impl ReanimationHolder {
    #[inline(always)]
    pub(crate) unsafe fn reanimations(this: *mut Self) -> *mut DataArray<Reanimation> {
        unsafe { addr_at_mut(this, 0x0) }
    }
}

#[repr(C)]
pub(crate) struct WidgetContainer {
    _data: [u8; 0],
}
