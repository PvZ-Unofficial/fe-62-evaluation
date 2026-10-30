//! 1051 CursorObject::Update 0x4387F3..0x438814 proves the two coordinate spaces.
use super::{CursorObject, CursorPreview, MouseWindow, read_unaligned_at};
impl MouseWindow {
    pub(crate) unsafe fn position(this: *const Self) -> (i32, i32) {
        unsafe { (read_unaligned_at(this, 0xe0), read_unaligned_at(this, 0xe4)) }
    }
}
impl CursorObject {
    pub(crate) unsafe fn position(this: *const Self) -> (i32, i32) {
        unsafe { (read_unaligned_at(this, 8), read_unaligned_at(this, 12)) }
    }
}
impl CursorPreview {
    pub(crate) unsafe fn grid_row(this: *const Self) -> i32 {
        unsafe { read_unaligned_at(this, 0x28) }
    }
}

impl CursorObject {
    pub(crate) unsafe fn select_cob(this: *mut Self, id: u32) {
        unsafe {
            super::write_unaligned_at(this, 0x40, id);
            super::write_unaligned_at(this, 0x30, 8i32);
        }
    }
}

impl super::Board {
    pub(crate) unsafe fn view_position(this: *const Self) -> (i32, i32) {
        unsafe {
            (
                super::read_unaligned_at(this, 0x30),
                super::read_unaligned_at(this, 0x34),
            )
        }
    }
    pub(crate) unsafe fn set_view_position(this: *mut Self, x: i32, y: i32) {
        unsafe {
            super::write_unaligned_at(this, 0x30, x);
            super::write_unaligned_at(this, 0x34, y);
        }
    }
}
impl super::LawnApp {
    pub(crate) unsafe fn animation_skip(this: *const Self) -> bool {
        unsafe { super::read_unaligned_at::<u8>(this, 0x7f5) != 0 }
    }
    pub(crate) unsafe fn set_animation_skip(this: *mut Self, v: bool) {
        unsafe {
            super::write_unaligned_at(this, 0x7f5, u8::from(v));
        }
    }
}
