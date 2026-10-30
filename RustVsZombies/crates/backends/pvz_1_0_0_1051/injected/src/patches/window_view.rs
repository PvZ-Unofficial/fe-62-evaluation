//! Fixed drawing capacity with a bounded, independently scaled presentation.
//! Native rebuild and intermediate-surface ABI: docs/history/window-capacity-probe.md.
use crate::{
    error::{Pvz1051Error, Result},
    raw::layout as raw,
};
use std::{cell::Cell, ptr};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
};

#[derive(Clone, Copy)]
struct Saved {
    aspect: u8,
    width: i32,
    height: i32,
}
thread_local! {
    static SAVED: Cell<Option<Saved>> = const { Cell::new(None) };
    static SCALE: Cell<f64> = const { Cell::new(0.0) };
    static LEFT_ALIGNED: Cell<bool> = const { Cell::new(false) };
    static WIDTH: Cell<i32> = const { Cell::new(800) };
    // Restoring a source while fullscreen must validate against the previous desktop,
    // not the temporary exclusive display mode. Updated only on windowed geometry changes.
    static WINDOWED_LIMIT: Cell<f64> = const { Cell::new(0.0) };
}
pub(crate) unsafe fn read<T: Copy>(base: *mut u8, offset: usize) -> T {
    unsafe { ptr::read_unaligned(base.add(offset).cast()) }
}
unsafe fn write<T>(base: *mut u8, offset: usize, value: T) {
    unsafe { ptr::write_unaligned(base.add(offset).cast(), value) }
}
pub(crate) fn enabled() -> bool {
    if let Some(host) = crate::module_link::services() {
        return (host.view_enabled)();
    }
    SAVED.get().is_some()
}
pub(crate) fn original_size() -> Option<(i32, i32)> {
    if let Some(host) = crate::module_link::services() {
        let mut size = [0; 2];
        return unsafe { (host.view_original_size)(size.as_mut_ptr()) }.then_some((size[0], size[1]));
    }
    SAVED.get().map(|saved| (saved.width, saved.height))
}
pub(crate) unsafe fn before_reset() {
    if !enabled() {
        return;
    }
    unsafe {
        let app = raw::lawn_app().cast::<u8>();
        if app.is_null() {
            return;
        }
        let dd: *mut u8 = read(app, 0x36c);
        if !dd.is_null() {
            // This forced windowed stretch flag is not reset by native Init.
            // Let native fullscreen initialization derive its own presentation.
            write(dd, 0x9c, 0u8);
        }
    }
}
pub(crate) fn width() -> i32 {
    if let Some(host) = crate::module_link::services() {
        return (host.view_width)();
    }
    if enabled() {
        unsafe {
            let app = raw::lawn_app().cast::<u8>();
            if !app.is_null() {
                let dd: *mut u8 = read(app, 0x36c);
                if !dd.is_null() {
                    return read(dd, 0x7c);
                }
            }
        }
    }
    800
}
pub(crate) fn left(width: i32) -> i32 {
    if let Some(host) = crate::module_link::services() {
        return (host.view_left)(width);
    }
    view_left(width, LEFT_ALIGNED.get())
}

fn view_left(width: i32, left_aligned: bool) -> i32 {
    (if left_aligned { 0 } else { 400 - width / 2 }).clamp(-220, 1180 - width)
}
pub(crate) fn left_aligned() -> bool {
    if let Some(host) = crate::module_link::services() {
        return (host.view_left_aligned)();
    }
    LEFT_ALIGNED.get()
}
pub(crate) fn set_left_aligned(value: bool) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.view_set_left_aligned)(value));
    }
    if crate::runtime::hook::paint::active() {
        return Err(Pvz1051Error::OperationRejected(
            "change viewport outside the draw stack",
        ));
    }
    LEFT_ALIGNED.set(value);
    if enabled() {
        unsafe {
            let app = raw::lawn_app();
            let hwnd = raw::LawnApp::hwnd(app);
            super::window_resize::sync_geometry(hwnd);
            let mut point = POINT::default();
            if GetCursorPos(&mut point) != 0 && WindowFromPoint(point) == hwnd && ScreenToClient(hwnd, &mut point) != 0
            {
                // Refresh the stationary cursor through the existing native input path.
                let position = ((point.y as u32 & 0xffff) << 16) | (point.x as u32 & 0xffff);
                PostMessageW(hwnd, WM_MOUSEMOVE, 0, position as isize);
            }
            dirty(app.cast());
        }
    }
    Ok(())
}

unsafe fn dirty(app: *mut u8) {
    unsafe {
        let manager: *mut u8 = read(app, 0x320);
        let table: *mut u8 = read(manager, 0);
        let call: unsafe extern "thiscall" fn(*mut u8) = std::mem::transmute(read::<usize>(table, 0x28));
        call(manager);
    }
}

unsafe fn rebuild(app: *mut u8, capacity: i32) -> Result<()> {
    unsafe {
        let original: i32 = read(app, 0xc0);
        let manager: *mut u8 = read(app, 0x320);
        write(manager, 0x60, 0usize);
        write(app, 0xc0, capacity);
        let result: i32;
        core::arch::asm!("push esi", "mov esi, eax", "call ecx", "pop esi",
            inout("eax") app => result, inout("ecx") 0x5521f0usize => _, lateout("edx") _);
        write(app, 0xc0, original);
        if result != 0 {
            return Err(Pvz1051Error::OperationRejected(
                "native drawing capacity rebuild failed",
            ));
        }
        let table: *mut u8 = read(app, 0);
        let reinit: unsafe extern "thiscall" fn(*mut u8) = std::mem::transmute(read::<usize>(table, 0x34));
        reinit(app);
        let dd: *mut u8 = read(app, 0x36c);
        write(manager, 0x60, read::<usize>(dd, 0xce8));
        // EAX=DD; stack width,height; RET 8. Equal dimensions remove the old
        // window-sized intermediate surfaces without reallocating the target.
        core::arch::asm!("push 600", "push edx", "call ecx",
            inout("eax") dd => _, inout("ecx") 0x5629b0usize => _, inout("edx") capacity => _);
        dirty(app);
        Ok(())
    }
}

pub(crate) unsafe fn dpi_scale(hwnd: HWND) -> f64 {
    // Unaware windows return 96: their Win32 coordinates are already virtualized.
    unsafe { GetDpiForWindow(hwnd).max(96) as f64 / 96.0 }
}
pub(crate) fn scale() -> Result<f64> {
    if let Some(host) = crate::module_link::services() {
        let mut value = 0.0;
        crate::module_link::checked(unsafe { (host.view_scale)(&mut value) })?;
        return Ok(value);
    }
    if enabled() && SCALE.get() > 0.0 {
        return Ok(SCALE.get());
    }
    unsafe {
        let app = raw::lawn_app();
        if app.is_null() {
            return Err(Pvz1051Error::GameNotReady);
        }
        let mut rect = RECT::default();
        if GetClientRect(raw::LawnApp::hwnd(app), &mut rect) == 0 {
            return Err(Pvz1051Error::GameNotReady);
        }
        Ok(rect.bottom as f64 / 600.0 / dpi_scale(raw::LawnApp::hwnd(app)))
    }
}

pub(crate) fn set(value: bool) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.view_set)(value));
    }
    if crate::runtime::hook::paint::active() {
        return Err(Pvz1051Error::OperationRejected(
            "change viewport outside the draw stack",
        ));
    }
    if value == enabled() {
        return Ok(());
    }
    unsafe {
        let app = raw::lawn_app();
        if app.is_null() {
            return Err(Pvz1051Error::GameNotReady);
        }
        let hwnd = raw::LawnApp::hwnd(app);
        if value {
            let dd: *mut u8 = read(app.cast(), 0x36c);
            if dd.is_null() || read::<u8>(dd, 0x38) == 0 {
                return Err(Pvz1051Error::OperationRejected(
                    "expanded canvas requires native hardware rendering",
                ));
            }
            let mut rect = RECT::default();
            if GetClientRect(hwnd, &mut rect) == 0 {
                return Err(Pvz1051Error::GameNotReady);
            }
            let saved = Saved {
                aspect: read(app.cast(), 0x5d8),
                width: rect.right,
                height: rect.bottom,
            };
            let initial_scale = scale()?;
            crate::runtime::hook::paint::set_view_enabled(true)?;
            if read::<u8>(dd, 0xce4) != 0
                && let Err(error) = rebuild(app.cast(), 1400)
            {
                rebuild(app.cast(), 800)?;
                crate::runtime::hook::paint::set_view_enabled(false)?;
                return Err(error);
            }
            write(app.cast(), 0x5d8, 0u8);
            SAVED.set(Some(saved));
            SCALE.set(if read::<u8>(dd, 0xce4) != 0 { initial_scale } else { 0.0 });
            WIDTH.set(800);
            if let Err(error) = super::window_resize::maintain() {
                set(false)?;
                return Err(error);
            }
            super::window_resize::sync_geometry(hwnd);
        } else if let Some(saved) = SAVED.get() {
            let dd: *mut u8 = read(app.cast(), 0x36c);
            let windowed = read::<u8>(dd, 0xce4) != 0;
            if windowed {
                rebuild(app.cast(), 800)?;
            }
            write(app.cast(), 0x5d8, saved.aspect);
            SAVED.set(None);
            SCALE.set(0.0);
            WIDTH.set(800);
            if windowed {
                super::window_resize::resize_client(hwnd, saved.width, saved.height);
            }
            super::window_resize::set(super::window_resize::enabled())?;
            crate::runtime::hook::paint::set_view_enabled(false)?;
        }
    }
    Ok(())
}

/// Native mode switches rebuild DD independently. Recover capacity only after
/// returning to a normal window; fullscreen keeps the game's own display mode.
pub(crate) fn maintain() -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.view_maintain)());
    }
    if !enabled() {
        return Ok(());
    }
    unsafe {
        let app = raw::lawn_app();
        if app.is_null() {
            return Ok(());
        }
        let dd: *mut u8 = read(app.cast(), 0x36c);
        if dd.is_null() || read::<u8>(dd, 0xce4) == 0 {
            return Ok(());
        }
        let d3d: *mut u8 = read(dd, 0x30);
        // InitFromDDInterface 0x564c52..0x564c5e copies capacity into +8/+12.
        if d3d.is_null() || read::<i32>(d3d, 8) == 1400 {
            return Ok(());
        }
        let hwnd = raw::LawnApp::hwnd(app);
        if SCALE.get() == 0.0 {
            SCALE.set(scale()?);
        }
        rebuild(app.cast(), 1400)?;
        write(app.cast(), 0x5d8, 0u8);
        let factor = SCALE.get() * dpi_scale(hwnd);
        super::window_resize::resize_client(
            hwnd,
            (WIDTH.get() as f64 * factor).round() as i32,
            (600.0 * factor).round() as i32,
        );
        super::window_resize::sync_geometry(hwnd);
    }
    Ok(())
}

pub(crate) unsafe fn maximum_scale(hwnd: HWND, view_width: i32, desktop_mode: bool) -> Result<f64> {
    unsafe {
        let mut extended: MONITORINFOEXW = std::mem::zeroed();
        extended.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
            &mut extended.monitorInfo,
        ) == 0
        {
            return Err(Pvz1051Error::GameNotReady);
        }
        let mut monitor = extended.monitorInfo;
        if desktop_mode {
            // Exclusive fullscreen may temporarily select 800x600. Registry
            // display settings retain the desktop resolution used on return.
            let mut mode: DEVMODEW = std::mem::zeroed();
            mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
            if EnumDisplaySettingsW(extended.szDevice.as_ptr(), ENUM_REGISTRY_SETTINGS, &mut mode) == 0 {
                return Err(Pvz1051Error::GameNotReady);
            }
            monitor.rcWork.right += mode.dmPelsWidth as i32 - (monitor.rcMonitor.right - monitor.rcMonitor.left);
            monitor.rcWork.bottom += mode.dmPelsHeight as i32 - (monitor.rcMonitor.bottom - monitor.rcMonitor.top);
        }
        let mut border = RECT::default();
        AdjustWindowRectEx(
            &mut border,
            if desktop_mode {
                WS_OVERLAPPEDWINDOW
            } else {
                GetWindowLongW(hwnd, GWL_STYLE) as u32
            },
            0,
            GetWindowLongW(hwnd, GWL_EXSTYLE) as u32,
        );
        Ok(
            ((monitor.rcWork.right - monitor.rcWork.left - border.right + border.left) as f64 / view_width as f64)
                .min((monitor.rcWork.bottom - monitor.rcWork.top - border.bottom + border.top) as f64 / 600.0)
                / dpi_scale(hwnd),
        )
    }
}

pub(crate) fn set_scale(value: f64) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.view_set_scale)(value));
    }
    if crate::runtime::hook::paint::active() {
        return Err(Pvz1051Error::OperationRejected(
            "change window scale outside the draw stack",
        ));
    }
    if !value.is_finite() || value <= 0.0 {
        return Err(Pvz1051Error::OperationRejected("invalid window scale"));
    }
    unsafe {
        let app = raw::lawn_app();
        if app.is_null() {
            return Err(Pvz1051Error::GameNotReady);
        }
        let hwnd = raw::LawnApp::hwnd(app);
        let dd: *mut u8 = read(app.cast(), 0x36c);
        if read::<u8>(dd, 0xce4) == 0 {
            if WINDOWED_LIMIT.get() == 0.0 {
                WINDOWED_LIMIT.set(maximum_scale(hwnd, WIDTH.get(), true)?);
            }
            if value > WINDOWED_LIMIT.get() {
                return Err(Pvz1051Error::OperationRejected(
                    "selected game window scale does not fit the saved windowed display",
                ));
            }
            SCALE.set(value);
            return Ok(());
        }
        let limit = maximum_scale(hwnd, WIDTH.get(), false)?;
        WINDOWED_LIMIT.set(limit);
        if value > limit {
            return Err(Pvz1051Error::OperationRejected(
                "selected game window scale does not fit the display",
            ));
        }
        let physical_scale = value * dpi_scale(hwnd);
        let width = WIDTH.get() as f64 * physical_scale;
        let height = 600.0 * physical_scale;
        SCALE.set(value);
        super::window_resize::resize_client(hwnd, width.round() as i32, height.round() as i32);
        super::window_resize::sync_geometry(hwnd);
        dirty(app.cast());
    }
    Ok(())
}

pub(crate) unsafe fn sync(hwnd: HWND, client: &RECT) {
    unsafe {
        let app = raw::lawn_app().cast::<u8>();
        let dd: *mut u8 = read(app, 0x36c);
        let manager: *mut u8 = read(app, 0x320);
        // A corner drag changes scale while preserving the current logical width.
        if IsZoomed(hwnd) == 0
            && read::<u8>(dd, 0xce4) != 0
            && client.bottom > 0
            && (client.right - (WIDTH.get() as f64 * client.bottom as f64 / 600.0).round() as i32).abs() <= 1
        {
            SCALE.set(client.bottom as f64 / 600.0 / dpi_scale(hwnd));
        }
        let physical_scale = SCALE.get() * dpi_scale(hwnd);
        let unchanged_view = (client.right - (WIDTH.get() as f64 * physical_scale).round() as i32).abs() <= 1
            && client.bottom == (600.0 * physical_scale).round() as i32;
        let width = if unchanged_view {
            WIDTH.get()
        } else {
            ((client.right as f64 * 600.0 / client.bottom as f64).round() as i32).clamp(800, 1400)
        };
        if IsZoomed(hwnd) == 0 && read::<u8>(dd, 0xce4) != 0 && client.bottom == (600.0 * physical_scale).round() as i32
        {
            WIDTH.set(width);
            if let Ok(limit) = maximum_scale(hwnd, width, false) {
                WINDOWED_LIMIT.set(limit);
            }
        }
        let scale = (client.right as f64 / width as f64).min(client.bottom as f64 / 600.0);
        let (w, h) = ((width as f64 * scale).round() as i32, (600.0 * scale).round() as i32);
        let present = [(client.right - w) / 2, (client.bottom - h) / 2, w, h];
        write(dd, 0x7c, width);
        write(dd, 0xb0, present);
        write(dd, 0x9c, 1u8);
        write(app, 0x5c9, 1u8);
        write(manager, 0xbc, [left(width), 0, width, 600]);
        write(manager, 0xcc, present);
    }
}

pub(crate) unsafe fn prepare_paint() -> Result<()> {
    unsafe {
        let app = raw::lawn_app().cast::<u8>();
        if app.is_null() {
            return Ok(());
        }
        let dd: *mut u8 = read(app, 0x36c);
        // The DirectDraw presentation touches only its destination rectangle.
        // Fill uncovered client bars separately; never erase the presented UI.
        if read::<u8>(dd, 0xce4) != 0 {
            let hwnd = raw::LawnApp::hwnd(app.cast());
            let mut client = RECT::default();
            if GetClientRect(hwnd, &mut client) != 0 {
                let [x, y, w, h]: [i32; 4] = read(dd, 0xb0);
                if x > 0 || y > 0 {
                    let dc = GetDC(hwnd);
                    if !dc.is_null() {
                        let brush = GetStockObject(BLACK_BRUSH) as HBRUSH;
                        for bar in [
                            RECT {
                                left: 0,
                                top: 0,
                                right: x,
                                bottom: client.bottom,
                            },
                            RECT {
                                left: x + w,
                                top: 0,
                                right: client.right,
                                bottom: client.bottom,
                            },
                            RECT {
                                left: x,
                                top: 0,
                                right: x + w,
                                bottom: y,
                            },
                            RECT {
                                left: x,
                                top: y + h,
                                right: x + w,
                                bottom: client.bottom,
                            },
                        ] {
                            if bar.right > bar.left && bar.bottom > bar.top {
                                FillRect(dc, &bar, brush);
                            }
                        }
                        ReleaseDC(hwnd, dc);
                    }
                }
            }
        }
        let board = raw::LawnApp::board(app.cast()).cast::<u8>();
        if width() == 800 && (board.is_null() || raw::Board::view_position(board.cast()) == (0, 0)) {
            return Ok(());
        }
        let d3d: *mut u8 = read(dd, 0x30);
        if d3d.is_null() {
            return Err(Pvz1051Error::GameNotReady);
        }
        let device: *mut u8 = read(d3d, 0x20);
        if device.is_null() {
            return Err(Pvz1051Error::GameNotReady);
        }
        let table: *mut u8 = read(device, 0);
        // Original D3D7 Clear call at 0x564bf2..0x564c0e, before native drawing.
        let clear: unsafe extern "system" fn(*mut u8, u32, usize, u32, u32, f32, u32) -> i32 =
            std::mem::transmute(read::<usize>(table, 0x28));
        if clear(device, 0, 0, 1, 0xff000000, 1.0, 0) < 0 {
            return Err(Pvz1051Error::OperationRejected("native viewport clear failed"));
        }
        dirty(app);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn left_alignment_stops_at_right_resource_edge() {
        for (width, x) in [(800, 0), (1067, 0), (1180, 0), (1240, -60), (1333, -153), (1400, -220)] {
            assert_eq!(super::view_left(width, true), x);
            assert!(x >= -220 && x + width <= 1180);
        }
    }
    #[test]
    fn view_stops_at_resource_edges() {
        for (width, x) in [(800, 0), (1067, -133), (1240, -220), (1333, -220), (1400, -220)] {
            assert_eq!(super::left(width), x);
            assert!(x >= -220 && x + width <= 1180);
        }
    }
}
