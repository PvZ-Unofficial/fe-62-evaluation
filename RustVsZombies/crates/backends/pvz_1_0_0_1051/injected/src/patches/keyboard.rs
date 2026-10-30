//! Physical WM_KEY call sites only: EAX=WidgetManager, ECX=VK, no stack arguments.
//! Original EXE: 55040f calls 539660; 55042f calls 5396a0. Demo calls are untouched.
use super::memory::{encode_rel32, read_bytes, restore_owned_bytes, write_bytes};
use crate::{
    error::{Pvz1051Error, Result},
    raw::layout::{self, LawnApp},
};
use std::cell::Cell;
use windows_sys::Win32::UI::{Input::KeyboardAndMouse::GetKeyState, WindowsAndMessaging::*};
thread_local! {
    static SINK: Cell<Option<fn(u32)>> = const {Cell::new(None)};
    static PATCH: Cell<Option<[[u8;5];3]>> = const {Cell::new(None)};
    static LAST_PUMP: Cell<u32> = const {Cell::new(0)};
}
const SITES: [usize; 3] = [0x55040f, 0x55042f, 0x538e44];
const ORIGINAL: [[u8; 5]; 3] = [
    [0xe8, 0x4c, 0x92, 0xfe, 0xff],
    [0xe8, 0x6c, 0x92, 0xfe, 0xff],
    [0xe8, 0x57, 0x08, 0, 0],
];
pub(crate) fn set(sink: Option<fn(u32)>) -> Result<()> {
    if sink.is_some() && PATCH.get().is_none() {
        for i in 0..3 {
            if read_bytes(SITES[i], 5) != ORIGINAL[i] {
                return Err(Pvz1051Error::OperationRejected(
                    "physical keyboard call is already patched",
                ));
            }
        }
        let patch = [
            encode_rel32(0xe8, SITES[0], key_down as *const () as usize, "key down")?,
            encode_rel32(0xe8, SITES[1], key_up as *const () as usize, "key up")?,
            encode_rel32(0xe8, SITES[2], focus_release as *const () as usize, "focus release")?,
        ];
        for i in 0..3 {
            if let Err(error) = write_bytes(SITES[i], &patch[i]) {
                for prior in (0..i).rev() {
                    restore_owned_bytes(SITES[prior], &ORIGINAL[prior], &patch[prior], "keyboard")?;
                }
                return Err(error);
            }
        }
        PATCH.set(Some(patch));
    } else if sink.is_none() {
        if let Some(patch) = PATCH.get() {
            for i in 0..3 {
                restore_owned_bytes(SITES[i], &ORIGINAL[i], &patch[i], "keyboard")?;
            }
            PATCH.set(None);
        }
    }
    SINK.set(sink);
    LAST_PUMP.set(0);
    Ok(())
}
fn emit(key: u32, down: bool, repeat: bool, skipping: bool) {
    let Some(sink) = SINK.get() else {
        return;
    };
    if key > 254 {
        return;
    }
    // Event-time OS modifier state; no per-frame keyboard scan or game-state cache.
    unsafe {
        let app = layout::lawn_app();
        if app.is_null() {
            return;
        }
        let focused = GetForegroundWindow() == LawnApp::hwnd(app);
        let modifiers = u32::from(GetKeyState(0x11) < 0)
            | (u32::from(GetKeyState(0x12) < 0) << 1)
            | (u32::from(GetKeyState(0x10) < 0) << 2)
            | (u32::from(GetKeyState(0x5b) < 0 || GetKeyState(0x5c) < 0) << 3);
        sink(
            key | (u32::from(down) << 8)
                | (modifiers << 9)
                | (u32::from(repeat) << 13)
                | (u32::from(focused) << 14)
                | (u32::from(skipping) << 15),
        );
    }
}
extern "C" fn capture(manager: *mut u8, key: u32, down: u32) {
    let _guard = crate::runtime::hook::ActiveHookGuard::enter();
    // Verified WidgetManager::KeyDown/Up read and write manager+0xf4+VK.
    let repeat = key < 255 && down != 0 && unsafe { manager.add(0xf4 + key as usize).read() != 0 };
    emit(key, down != 0, repeat, false);
}
#[unsafe(naked)]
unsafe extern "C" fn key_down() {
    core::arch::naked_asm!("pushfd","pushad","push 1","push [esp+28]","push [esp+36]","call {capture}","add esp,12","popad","popfd","push 0x539660","ret",capture=sym capture);
}
#[unsafe(naked)]
unsafe extern "C" fn key_up() {
    core::arch::naked_asm!("pushfd","pushad","push 0","push [esp+28]","push [esp+36]","call {capture}","add esp,12","popad","popfd","push 0x5396a0","ret",capture=sym capture);
}
pub(crate) fn pump_fast_forward() {
    if SINK.get().is_none() {
        return;
    }
    unsafe {
        let now = windows_sys::Win32::System::SystemInformation::GetTickCount();
        if now.wrapping_sub(LAST_PUMP.get()) < 8 {
            return;
        }
        LAST_PUMP.set(now);
        let app = layout::lawn_app();
        if app.is_null() {
            return;
        }
        let hwnd = LawnApp::hwnd(app);
        let manager = LawnApp::mouse_window(app).cast::<u8>();
        let mut msg: MSG = std::mem::zeroed();
        // Consume only keyboard messages: never dispatch native game shortcuts while skipping.
        for _ in 0..128 {
            if PeekMessageW(&mut msg, hwnd, WM_KEYFIRST, WM_KEYLAST, PM_REMOVE) == 0 {
                break;
            }
            if matches!(msg.message, WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP) {
                let down = matches!(msg.message, WM_KEYDOWN | WM_SYSKEYDOWN);
                emit(msg.wParam as u32, down, down && msg.lParam & (1 << 30) != 0, true);
                if !manager.is_null() && msg.wParam < 255 {
                    manager.add(0xf4 + msg.wParam).write(u8::from(down));
                }
            }
        }
    }
}

// WidgetManager::LostFocus calls KeyUp at 0x538e44 with EAX=EDI, ECX=ESI.
// These synthesized releases reset edge tracking; they must never run release callbacks.
extern "C" fn reset_edges() {
    let _guard = crate::runtime::hook::ActiveHookGuard::enter();
    if let Some(sink) = SINK.get() {
        sink(0);
    }
}
#[unsafe(naked)]
unsafe extern "C" fn focus_release() {
    core::arch::naked_asm!("pushfd","pushad","call {reset}","popad","popfd","push 0x5396a0","ret",reset=sym reset_edges);
}
