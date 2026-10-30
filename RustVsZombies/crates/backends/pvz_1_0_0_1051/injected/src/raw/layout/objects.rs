use super::{MouseWindow, WidgetContainer, read_unaligned_at, write_unaligned_at};

#[repr(C)]
pub(crate) struct ChosenSeed {
    _data: [u8; 0x3c],
}

impl ChosenSeed {
    /// Native MouseDown tests this byte before calling ClickedSeedInBank.
    pub(crate) unsafe fn picked_by_dave(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x38) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn seed_state(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x24) }
    }
    #[inline(always)]
    pub(crate) unsafe fn seed_index_in_bank(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x28) }
    }
    #[inline(always)]
    pub(crate) unsafe fn refreshing(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x2c) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn refresh_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x30) }
    }
    #[inline(always)]
    pub(crate) unsafe fn imitater_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x34) }
    }
}

#[repr(C)]
pub(crate) struct Plant {
    _data: [u8; 0x14c],
}

impl Plant {
    #[inline(always)]
    pub(crate) unsafe fn x(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x8) }
    }
    #[inline(always)]
    pub(crate) unsafe fn y(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xc) }
    }
    #[inline(always)]
    pub(crate) unsafe fn width(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x10) }
    }
    #[inline(always)]
    pub(crate) unsafe fn height(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x14) }
    }
    #[inline(always)]
    pub(crate) unsafe fn row(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x1c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn seed_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x24) }
    }
    #[inline(always)]
    pub(crate) unsafe fn col(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x28) }
    }
    #[inline(always)]
    pub(crate) unsafe fn state(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x3c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_state(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x3c, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn health(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x40) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_health(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x40, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn max_health(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x44) }
    }
    #[inline(always)]
    pub(crate) unsafe fn launch_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x58) }
    }
    #[inline(always)]
    pub(crate) unsafe fn shooting_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x90) }
    }
    #[inline(always)]
    pub(crate) unsafe fn disappear_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x4c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn do_special_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x50) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_do_special_countdown(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x50, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn state_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x54) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_state_countdown(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x54, value) }
    }
    // Objdump Zombie::EatPlant @ 0x52fcf7 writes 50 to Plant+0xb4 after bite damage.
    #[inline(always)]
    pub(crate) unsafe fn recently_eaten_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xb4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn eaten_flash_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xb8) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_eaten_flash_countdown(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0xb8, value) }
    }
    // Objdump Board::MouseDownWithPlant/Plant::SetSleeping: Gloom-shroom sleep carry uses
    // wake_up_counter at +0x130 and asleep flag at +0x143.
    #[inline(always)]
    pub(crate) unsafe fn wake_up_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x130) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_wake_up_counter(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x130, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn target_x(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x80) }
    }
    #[inline(always)]
    pub(crate) unsafe fn target_y(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x84) }
    }
    #[inline(always)]
    pub(crate) unsafe fn body_reanim_id(this: *const Self) -> u16 {
        unsafe { read_unaligned_at(this, 0x94) }
    }
    #[inline(always)]
    pub(crate) unsafe fn on_bungee_state(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x134) }
    }
    #[inline(always)]
    pub(crate) unsafe fn imitater_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x138) }
    }
    #[inline(always)]
    pub(crate) unsafe fn target_zombie_id(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x12c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_dead(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x141) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_squished(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x142) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_asleep(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x143) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_on_board(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x144) != 0 }
    }
}

#[repr(C)]
pub(crate) struct Projectile {
    _data: [u8; 0x94],
}

impl Projectile {
    #[inline(always)]
    pub(crate) unsafe fn row(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x1c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn pos_x(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x30) }
    }
    #[inline(always)]
    pub(crate) unsafe fn pos_y(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x34) }
    }
    #[inline(always)]
    pub(crate) unsafe fn pos_z(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x38) }
    }
    #[inline(always)]
    pub(crate) unsafe fn vel_x(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x3c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn vel_y(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x40) }
    }
    #[inline(always)]
    pub(crate) unsafe fn vel_z(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x44) }
    }
    #[inline(always)]
    pub(crate) unsafe fn acc_z(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x48) }
    }
    #[inline(always)]
    pub(crate) unsafe fn dead(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x50) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn motion(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x58) }
    }
    #[inline(always)]
    pub(crate) unsafe fn projectile_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x5c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn age(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x60) }
    }
    #[inline(always)]
    pub(crate) unsafe fn damage_flags(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x74) }
    }
    #[inline(always)]
    pub(crate) unsafe fn torch_col(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x78) }
    }
    #[inline(always)]
    pub(crate) unsafe fn cob_target_x(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x80) }
    }
    #[inline(always)]
    pub(crate) unsafe fn cob_target_row(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x84) }
    }
    #[inline(always)]
    pub(crate) unsafe fn target_zombie_id(this: *const Self) -> u32 {
        unsafe { read_unaligned_at(this, 0x88) }
    }
}

#[repr(C)]
pub(crate) struct SeedBank {
    _data: [u8; 0x350],
}

#[repr(C)]
pub(crate) struct MessageWidget {
    _data: [u8; 0x31c],
}
impl MessageWidget {
    pub(crate) unsafe fn label_byte(this: *const Self, index: usize) -> u8 {
        unsafe { read_unaligned_at(this, 4 + index) }
    }
    pub(crate) unsafe fn duration(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x88) }
    }
    pub(crate) unsafe fn style(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x8c) }
    }
    pub(crate) unsafe fn set_duration(this: *mut Self, duration: i32) {
        unsafe { write_unaligned_at(this, 0x88, duration) }
    }
}

impl SeedBank {
    // Board::DrawUIBottom 0x41A1F0 loads Board+0x144 then checks bank+0x18.
    pub(crate) unsafe fn visible(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x18) != 0 }
    }
    pub(crate) unsafe fn set_visible(this: *mut Self, visible: bool) {
        unsafe { write_unaligned_at(this, 0x18, u8::from(visible)) }
    }
    #[inline(always)]
    pub(crate) unsafe fn packet_count(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x24) }
    }
    #[inline(always)]
    pub(crate) unsafe fn packet(this: *mut Self, index: usize) -> *mut SeedPacket {
        unsafe { this.cast::<u8>().add(0x28 + index * 0x50).cast::<SeedPacket>() }
    }
}

#[repr(C)]
pub(crate) struct SeedChooserScreen {
    _data: [u8; 0],
}

impl SeedChooserScreen {
    #[inline(always)]
    pub(crate) unsafe fn widget_manager(this: *const Self) -> *mut MouseWindow {
        unsafe { read_unaligned_at(this, 0x10) }
    }
    #[inline(always)]
    pub(crate) unsafe fn parent(this: *const Self) -> *mut WidgetContainer {
        unsafe { read_unaligned_at(this, 0x14) }
    }
    #[inline(always)]
    pub(crate) unsafe fn mouse_visible(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x55) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn chosen_seed(this: *mut Self, index: usize) -> *mut ChosenSeed {
        unsafe { this.cast::<u8>().add(0xa4 + index * 0x3c).cast::<ChosenSeed>() }
    }
    #[inline(always)]
    pub(crate) unsafe fn seeds_in_flight(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xd20) }
    }
    #[inline(always)]
    pub(crate) unsafe fn seeds_in_bank(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xd24) }
    }
    #[inline(always)]
    pub(crate) unsafe fn choose_state(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xd38) }
    }
    #[inline(always)]
    pub(crate) unsafe fn view_lawn_time(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xd3c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_view_lawn_time(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0xd3c, value) }
    }
}

#[repr(C)]
pub(crate) struct TopMouseWindow {
    _data: [u8; 0],
}

#[repr(C)]
pub(crate) struct SeedPacket {
    _data: [u8; 0x50],
}

impl SeedPacket {
    // Native MouseHitTest 0x488F9C..0x488FBC uses x+offset, y, width, height.
    pub(crate) unsafe fn canvas_rect(this: *const Self) -> rsvz_model::Rect {
        unsafe {
            rsvz_model::Rect {
                x: read_unaligned_at::<i32>(this, 8) + read_unaligned_at::<i32>(this, 0x30),
                y: read_unaligned_at(this, 12),
                width: read_unaligned_at(this, 16),
                height: read_unaligned_at(this, 20),
            }
        }
    }

    pub(crate) unsafe fn visible(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x18) != 0 }
    }
    pub(crate) unsafe fn set_visible(this: *mut Self, visible: bool) {
        unsafe { write_unaligned_at(this, 0x18, u8::from(visible)) }
    }
    pub(crate) unsafe fn set_packet_type(this: *mut Self, kind: i32) {
        unsafe { write_unaligned_at(this, 0x34, kind) }
    }
    pub(crate) unsafe fn set_imitater_type(this: *mut Self, kind: i32) {
        unsafe { write_unaligned_at(this, 0x38, kind) }
    }
    // Objdump SeedPacket::MouseDown/WasPlanted: Deactivate writes +0x24/+0x28/+0x48/+0x49;
    // WasPlanted increments +0x4c before setting refresh state.
    #[inline(always)]
    pub(crate) unsafe fn refresh_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x24) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_refresh_counter(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x24, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn refresh_time(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x28) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_refresh_time(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x28, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn packet_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x34) }
    }
    #[inline(always)]
    pub(crate) unsafe fn imitater_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x38) }
    }
    #[cfg(test)]
    #[inline(always)]
    pub(crate) unsafe fn is_active(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x48) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_active(this: *mut Self, value: bool) {
        unsafe { write_unaligned_at(this, 0x48, u8::from(value)) }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_refreshing(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x49) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_refreshing(this: *mut Self, value: bool) {
        unsafe { write_unaligned_at(this, 0x49, u8::from(value)) }
    }
}

#[repr(C)]
pub(crate) struct Zombie {
    _data: [u8; 0x15c],
}

impl Zombie {
    #[inline(always)]
    pub(crate) unsafe fn x(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x8) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_x(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x8, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn y(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xc) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_y(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0xc, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn width(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x10) }
    }
    #[inline(always)]
    pub(crate) unsafe fn height(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x14) }
    }
    #[inline(always)]
    pub(crate) unsafe fn row(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x1c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_row(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x1c, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn render_order(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x20) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_render_order(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x20, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn zombie_type(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x24) }
    }
    #[inline(always)]
    pub(crate) unsafe fn phase(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x28) }
    }
    #[inline(always)]
    pub(crate) unsafe fn pos_x(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x2c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_pos_x(this: *mut Self, value: f32) {
        unsafe { write_unaligned_at(this, 0x2c, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn pos_y(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x30) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_pos_y(this: *mut Self, value: f32) {
        unsafe { write_unaligned_at(this, 0x30, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn speed_x(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x34) }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_eating(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x51) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn spawn_age(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x60) }
    }
    #[inline(always)]
    pub(crate) unsafe fn zombie_height(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x64) }
    }
    #[inline(always)]
    pub(crate) unsafe fn phase_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x68) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_phase_counter(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x68, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn from_wave(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x6c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn flat_tires(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0x78) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn altitude(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x84) }
    }
    #[inline(always)]
    pub(crate) unsafe fn chilled_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xac) }
    }
    #[inline(always)]
    pub(crate) unsafe fn buttered_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xb0) }
    }
    #[inline(always)]
    pub(crate) unsafe fn frozen_countdown(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xb4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn mind_controlled(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0xb8) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn blowing_away(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0xb9) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn has_head(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0xba) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn has_object(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0xbc) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn in_pool(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0xbd) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn on_high_ground(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0xbe) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn yucky_face(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0xbf) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn yucky_face_counter(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xc0) }
    }
    #[inline(always)]
    pub(crate) unsafe fn body_health(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xc8) }
    }
    #[inline(always)]
    pub(crate) unsafe fn body_max_health(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xcc) }
    }
    #[inline(always)]
    pub(crate) unsafe fn accessory_1_health(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xd0) }
    }
    #[inline(always)]
    pub(crate) unsafe fn accessory_1_max_health(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xd4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn accessory_2_health(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xdc) }
    }
    #[inline(always)]
    pub(crate) unsafe fn accessory_2_max_health(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0xe0) }
    }

    #[inline(always)]
    pub(crate) unsafe fn related_zombie_id(this: *const Self) -> u32 {
        unsafe { read_unaligned_at(this, 0xf0) }
    }
    #[inline(always)]
    pub(crate) unsafe fn follower_zombie_id(this: *const Self, index: usize) -> u32 {
        unsafe { read_unaligned_at(this, 0xf4 + index * 4) }
    }
    #[inline(always)]
    pub(crate) unsafe fn target_plant_id(this: *const Self) -> u32 {
        unsafe { read_unaligned_at(this, 0x128) }
    }
    #[inline(always)]
    pub(crate) unsafe fn set_body_health(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0xc8, value) }
    }
    #[inline(always)]
    pub(crate) unsafe fn is_disappeared(this: *const Self) -> bool {
        unsafe { read_unaligned_at::<u8>(this, 0xec) != 0 }
    }
    #[inline(always)]
    pub(crate) unsafe fn body_reanim_id(this: *const Self) -> u32 {
        unsafe { read_unaligned_at(this, 0x118) }
    }
    #[inline(always)]
    pub(crate) unsafe fn scale_zombie(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x11c) }
    }
    #[inline(always)]
    pub(crate) unsafe fn vel_z(this: *const Self) -> f32 {
        unsafe { read_unaligned_at(this, 0x120) }
    }
}

impl Zombie {
    // GetZombieRect 0x5320B0/0x5320C6 reads the untransformed hit dimensions.
    pub(crate) unsafe fn hurt_width(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x94) }
    }
    pub(crate) unsafe fn hurt_height(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x98) }
    }

    pub(crate) unsafe fn set_status_death_state(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0x28, value) }
    }
    pub(crate) unsafe fn set_status_mind_controlled(this: *mut Self, value: bool) {
        unsafe { write_unaligned_at(this, 0xb8, value) }
    }
    pub(crate) unsafe fn set_status_frozen_countdown(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0xb4, value) }
    }
    pub(crate) unsafe fn set_status_chilled_countdown(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0xac, value) }
    }
    pub(crate) unsafe fn set_status_buttered_countdown(this: *mut Self, value: i32) {
        unsafe { write_unaligned_at(this, 0xb0, value) }
    }
    pub(crate) unsafe fn set_status_blown_away(this: *mut Self, value: bool) {
        unsafe { write_unaligned_at(this, 0xb9, value) }
    }
    pub(crate) unsafe fn set_status_has_head(this: *mut Self, value: bool) {
        unsafe { write_unaligned_at(this, 0xba, value) }
    }
    pub(crate) unsafe fn set_status_scale(this: *mut Self, value: f32) {
        unsafe { write_unaligned_at(this, 0x11c, value) }
    }
}
