//! 1051 keeps its 800x600 draw surface. Only presentation and native mouse
//! source rectangles change. Offsets verified against Redraw 0x562F60,
//! WidgetManager::Resize 0x538D20 and its caller 0x552280..0x5522A7.
use crate::{
    error::{Pvz1051Error, Result},
    raw::layout as raw,
};
use std::{cell::Cell, ptr};
use windows_sys::Win32::{Foundation::*, UI::WindowsAndMessaging::*};

#[derive(Clone, Copy)]
struct Binding {
    hwnd: HWND,
    previous: WNDPROC,
    style: i32,
    width: i32,
    height: i32,
    widescreen: u8,
}
std::thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static BINDING: Cell<Option<Binding>> = const { Cell::new(None) };
    static SOFT_VSYNC: Cell<Option<u8>> = const { Cell::new(None) };
    static ORIGINAL_CURSOR: Cell<usize> = const { Cell::new(0) };
}

// LawnApp::EnforceCursor calls this USER32 import at 0x455973/0x455986.
// Its deferred mouse-exit path also resets the cursor after WM_SETCURSOR, so
// intercept the shared SetCursor call rather than only that window message.
const SET_CURSOR_IAT: usize = 0x6523c8;
fn cursor_hook(enabled: bool) -> Result<()> {
    let hook = resize_cursor as *const () as usize;
    // SAFETY: verified writable-through-protection 1051 USER32 import slot.
    let current = unsafe { ptr::read_volatile(SET_CURSOR_IAT as *const usize) };
    if enabled && ORIGINAL_CURSOR.get() == 0 {
        super::memory::write_bytes(SET_CURSOR_IAT, &hook.to_le_bytes())?;
        ORIGINAL_CURSOR.set(current);
    } else if !enabled && ORIGINAL_CURSOR.get() != 0 {
        super::memory::restore_owned_bytes(
            SET_CURSOR_IAT,
            &ORIGINAL_CURSOR.get().to_le_bytes(),
            &hook.to_le_bytes(),
            "resize cursor",
        )?;
        ORIGINAL_CURSOR.set(0);
    }
    Ok(())
}

unsafe extern "system" fn resize_cursor(mut cursor: HCURSOR) -> HCURSOR {
    let _active = crate::runtime::hook::ActiveHookGuard::enter();
    // SAFETY: standard Win32 SetCursor ABI; no native entities or user callbacks.
    unsafe {
        if let Some(binding) = BINDING.get() {
            let mut point = POINT::default();
            if GetCursorPos(&mut point) != 0 && WindowFromPoint(point) == binding.hwnd {
                let position = ((point.y as u32 & 0xffff) << 16) | (point.x as u32 & 0xffff);
                let hit = DefWindowProcW(binding.hwnd, WM_NCHITTEST, 0, position as isize) as u32;
                let hit = if super::window_view::enabled() {
                    match hit {
                        HTTOP | HTBOTTOM => HTBORDER,
                        other => other,
                    }
                } else {
                    hit
                };
                let shape = match hit {
                    HTLEFT | HTRIGHT => IDC_SIZEWE,
                    HTTOP | HTBOTTOM => IDC_SIZENS,
                    HTTOPLEFT | HTBOTTOMRIGHT => IDC_SIZENWSE,
                    HTTOPRIGHT | HTBOTTOMLEFT => IDC_SIZENESW,
                    _ => ptr::null(),
                };
                if !shape.is_null() {
                    cursor = LoadCursorW(ptr::null_mut(), shape);
                }
            }
        }
        let original = ORIGINAL_CURSOR.get();
        if original == 0 {
            return SetCursor(cursor);
        }
        let call: unsafe extern "system" fn(HCURSOR) -> HCURSOR = std::mem::transmute(original);
        call(cursor)
    }
}

// All callers are on the game's UI thread. No entity pointer is retained.
unsafe fn read<T: Copy>(base: *mut u8, offset: usize) -> T {
    unsafe { ptr::read_unaligned(base.add(offset).cast()) }
}
unsafe fn write<T>(base: *mut u8, offset: usize, value: T) {
    unsafe { ptr::write_unaligned(base.add(offset).cast(), value) };
}

pub(crate) fn enabled() -> bool {
    if let Some(host) = crate::module_link::services() {
        return (host.resize_enabled)();
    }
    ENABLED.get()
}

pub(crate) fn set(value: bool) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.resize_set)(value));
    }
    if !value && super::window_view::enabled() {
        ENABLED.set(false);
        if let Some(binding) = BINDING.get() {
            unsafe {
                SetWindowLongW(binding.hwnd, GWL_STYLE, binding.style);
            }
        }
        return Ok(());
    }
    if value {
        cursor_hook(true)?;
        ENABLED.set(true);
        if let Some(binding) = BINDING.get() {
            unsafe {
                SetWindowLongW(binding.hwnd, GWL_STYLE, binding.style | WS_THICKFRAME as i32);
            }
        }
        if let Err(error) = maintain() {
            ENABLED.set(false);
            cursor_hook(false)?;
            return Err(error);
        }
    } else if let Some(binding) = BINDING.get() {
        // SAFETY: subclass ownership is checked before removing our callback.
        unsafe {
            if IsWindow(binding.hwnd) != 0 {
                if GetWindowLongW(binding.hwnd, GWL_WNDPROC) != window_proc as *const () as i32 {
                    return Err(Pvz1051Error::OperationRejected(
                        "window resize subclass ownership changed",
                    ));
                }
                SetWindowLongW(binding.hwnd, GWL_WNDPROC, binding.previous.unwrap() as *const () as i32);
                SetWindowLongW(binding.hwnd, GWL_STYLE, binding.style);
                resize_client(binding.hwnd, binding.width, binding.height);
                sync_geometry(binding.hwnd);
                let app = raw::lawn_app().cast::<u8>();
                if !app.is_null() {
                    let dd: *mut u8 = read(app, 0x36c);
                    if !dd.is_null() {
                        write(dd, 0x9c, binding.widescreen);
                    }
                }
            }
        }
        BINDING.set(None);
        ENABLED.set(false);
    } else {
        ENABLED.set(false);
    }
    if !value {
        cursor_hook(false)?;
        // SAFETY: restore the original app setting even after a fullscreen HWND recreation.
        unsafe {
            let app = raw::lawn_app().cast::<u8>();
            if let Some(saved) = SOFT_VSYNC.take() {
                if !app.is_null() {
                    write(app, 0x5c9, saved);
                }
            }
        }
    }
    Ok(())
}

/// Fullscreen/3D changes recreate the HWND. Observe only that handle in the
/// existing physical loop; ordinary resizing is handled by window messages.
pub(crate) fn maintain() -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.resize_maintain)());
    }
    if !enabled() && !super::window_view::enabled() {
        return Ok(());
    }
    // SAFETY: game-thread entry, verified LawnApp root and native window fields.
    unsafe {
        let app = raw::lawn_app();
        if app.is_null() {
            return Ok(());
        }
        let hwnd = raw::LawnApp::hwnd(app);
        if hwnd.is_null() || BINDING.get().is_some_and(|binding| binding.hwnd == hwnd) {
            return Ok(());
        }
        let style = GetWindowLongW(hwnd, GWL_STYLE);
        if style as u32 & WS_CAPTION == 0 {
            return Ok(());
        }
        let dd: *mut u8 = read(app.cast(), 0x36c);
        if dd.is_null() {
            return Ok(());
        }
        if SOFT_VSYNC.get().is_none() {
            SOFT_VSYNC.set(Some(read(app.cast(), 0x5c9)));
        }
        let mut client = RECT::default();
        if GetClientRect(hwnd, &mut client) == 0 {
            return Err(Pvz1051Error::GameNotReady);
        }
        let previous = SetWindowLongW(hwnd, GWL_WNDPROC, window_proc as *const () as i32);
        if previous == 0 {
            return Err(Pvz1051Error::OperationRejected("could not subclass game window"));
        }
        let (width, height) = super::window_view::original_size().unwrap_or((client.right, client.bottom));
        BINDING.set(Some(Binding {
            hwnd,
            previous: Some(std::mem::transmute::<
                i32,
                unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            >(previous)),
            style,
            width,
            height,
            widescreen: read(dd, 0x9c),
        }));
        SetWindowLongW(
            hwnd,
            GWL_STYLE,
            if enabled() { style | WS_THICKFRAME as i32 } else { style },
        );
        resize_client(hwnd, client.right, client.bottom);
        sync_geometry(hwnd);
    }
    Ok(())
}

pub(crate) unsafe fn resize_client(hwnd: HWND, width: i32, height: i32) {
    // SAFETY: live native window, client dimensions exclude decorations.
    unsafe {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        AdjustWindowRectEx(
            &mut rect,
            GetWindowLongW(hwnd, GWL_STYLE) as u32,
            0,
            GetWindowLongW(hwnd, GWL_EXSTYLE) as u32,
        );
        SetWindowPos(
            hwnd,
            ptr::null_mut(),
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

pub(crate) unsafe fn sync_geometry(hwnd: HWND) {
    // SAFETY: verified native rectangle fields, read fresh after the original
    // window procedure. No world edits or user callbacks run here.
    unsafe {
        let app = raw::lawn_app();
        if app.is_null() || raw::LawnApp::hwnd(app) != hwnd {
            return;
        }
        let dd: *mut u8 = read(app.cast(), 0x36c);
        let manager = raw::LawnApp::mouse_window(app).cast::<u8>();
        let mut client = RECT::default();
        if dd.is_null()
            || manager.is_null()
            || GetClientRect(hwnd, &mut client) == 0
            || client.right <= 0
            || client.bottom <= 0
        {
            return;
        }
        if super::window_view::enabled() {
            super::window_view::sync(hwnd, &client);
            return;
        }
        // +0xCE4 is DDInterface::mIsWindowed (0x563125); don't alter fullscreen.
        if read::<u8>(dd, 0xce4) == 0 {
            return;
        }
        let width = raw::LawnApp::canvas_width(app);
        let height = raw::LawnApp::canvas_height(app);
        let presentation = [0i32, 0, client.right, client.bottom];
        write(dd, 0xb0, presentation);
        write(manager, 0xbc, [0i32, 0, width, height]);
        write(manager, 0xcc, presentation);
        // Widescreen selects a full-surface stretch instead of 1:1 dirty rects.
        write(dd, 0x9c, 1u8);
        // Native split scanline blits assume 1:1 heights (0x563293..0x5632C9).
        // Keep the existing soft-vsync path while the window is resizable.
        write(app.cast(), 0x5c9, 1u8);
    }
}

fn constrain(rect: &mut RECT, edge: usize, border_width: i32, border_height: i32) {
    if edge == WMSZ_TOP as usize || edge == WMSZ_BOTTOM as usize {
        let height = (rect.bottom - rect.top - border_height).max(3);
        rect.right = rect.left + height * 4 / 3 + border_width;
    } else {
        let width = (rect.right - rect.left - border_width).max(4);
        let height = width * 3 / 4 + border_height;
        if edge == WMSZ_TOPLEFT as usize || edge == WMSZ_TOPRIGHT as usize {
            rect.top = rect.bottom - height;
        } else {
            rect.bottom = rect.top + height;
        }
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let _active = crate::runtime::hook::ActiveHookGuard::enter();
    let Some(binding) = BINDING.get() else {
        // SAFETY: fallback for a window no longer owned by the resize feature.
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    };
    // SAFETY: Win32 provides these message-specific pointers synchronously.
    unsafe {
        if super::window_view::enabled() && message == WM_NCHITTEST {
            return match DefWindowProcW(hwnd, message, wparam, lparam) as u32 {
                HTTOP | HTBOTTOM => HTBORDER as isize,
                hit => hit as isize,
            };
        }
        if message == WM_SIZING && lparam != 0 {
            let mut outer = RECT::default();
            let mut client = RECT::default();
            GetWindowRect(hwnd, &mut outer);
            GetClientRect(hwnd, &mut client);
            if super::window_view::enabled() {
                let proposal = &mut *(lparam as *mut RECT);
                let scale = super::window_view::scale().unwrap_or(1.0) * super::window_view::dpi_scale(hwnd);
                let border = outer.right - outer.left - client.right;
                if matches!(
                    wparam as u32,
                    WMSZ_TOPLEFT | WMSZ_TOPRIGHT | WMSZ_BOTTOMLEFT | WMSZ_BOTTOMRIGHT
                ) {
                    let view_width = super::window_view::width();
                    let max_scale = super::window_view::maximum_scale(hwnd, view_width, false)
                        .unwrap_or(scale / super::window_view::dpi_scale(hwnd))
                        * super::window_view::dpi_scale(hwnd);
                    let width = (proposal.right - proposal.left - border)
                        .clamp(1, ((view_width as f64 * max_scale).round() as i32).max(1));
                    let height = (width as f64 * 600.0 / view_width as f64).round().max(1.0) as i32;
                    let border_height = outer.bottom - outer.top - client.bottom;
                    if matches!(wparam as u32, WMSZ_TOPLEFT | WMSZ_BOTTOMLEFT) {
                        proposal.left = proposal.right - width - border;
                    } else {
                        proposal.right = proposal.left + width + border;
                    }
                    if matches!(wparam as u32, WMSZ_TOPLEFT | WMSZ_TOPRIGHT) {
                        proposal.top = proposal.bottom - height - border_height;
                    } else {
                        proposal.bottom = proposal.top + height + border_height;
                    }
                    return 1;
                }
                let width = (proposal.right - proposal.left - border)
                    .clamp((800.0 * scale).round() as i32, (1400.0 * scale).round() as i32);
                if wparam == WMSZ_LEFT as usize {
                    proposal.left = proposal.right - width - border;
                } else {
                    proposal.right = proposal.left + width + border;
                }
                proposal.top = outer.top;
                proposal.bottom = outer.bottom;
                if wparam == WMSZ_TOP as usize || wparam == WMSZ_BOTTOM as usize {
                    *proposal = outer;
                }
                return 1;
            }
            constrain(
                &mut *(lparam as *mut RECT),
                wparam,
                outer.right - outer.left - client.right,
                outer.bottom - outer.top - client.bottom,
            );
            return 1;
        }
        let result = CallWindowProcW(binding.previous, hwnd, message, wparam, lparam);
        if message == WM_SIZE && wparam != SIZE_MINIMIZED as usize {
            sync_geometry(hwnd);
        }
        if message == WM_NCDESTROY {
            BINDING.set(None);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dragging_preserves_client_aspect_and_opposite_edge() {
        for edge in 1..=8 {
            let mut rect = RECT {
                left: 100,
                top: 200,
                right: 1316,
                bottom: 939,
            };
            constrain(&mut rect, edge, 16, 39);
            assert!(((rect.right - rect.left - 16) * 3 - (rect.bottom - rect.top - 39) * 4).abs() <= 3);
            assert_eq!(rect.left, 100);
            if edge == WMSZ_TOPLEFT as usize || edge == WMSZ_TOPRIGHT as usize {
                assert_eq!(rect.bottom, 939);
            } else {
                assert_eq!(rect.top, 200);
            }
        }
    }
}
