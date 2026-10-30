//! One physical native draw boundary, sharing the existing hook-depth proof.
use crate::{
    Pvz1051Backend,
    error::{Pvz1051Error, Result},
    patches::paint::PaintHook,
    raw::{abi, layout},
};
use rsvz_backend_api::error::{RuntimeError, RuntimeResult};
use rsvz_backend_api::{NativePaintBackend, OptionalOverlayBackend, PaintCanvas};
use rsvz_model::{Argb, PaintFont, PixelPos, Rect};
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
};

thread_local! {
    static ENTRY: Cell<Option<fn(&mut Pvz1051Backend)>> = const { Cell::new(None) };
    static HOOK: RefCell<Option<PaintHook>> = const { RefCell::new(None) };
    static DRAWING: Cell<bool> = const { Cell::new(false) };
    static VIEWPORT: Cell<Option<(u32,u32)>> = const { Cell::new(None) };
    static FAILURE: Cell<Option<i32>> = const { Cell::new(None) };
    static FAILURE_REPORTED: Cell<bool> = const { Cell::new(false) };
    static BORROWED: Cell<bool> = const { Cell::new(false) };
}
pub(crate) fn active() -> bool {
    DRAWING.get()
}

unsafe extern "C" {
    #[link_name = "rsvz_canvas_digit_bounds"]
    fn native_rsvz_canvas_digit_bounds(
        family: *const u16, pixels: i32, bold: i32, top: *mut i32, bottom: *mut i32,
    ) -> i32;
    #[link_name = "rsvz_canvas_cursor"]
    fn native_rsvz_canvas_cursor(x: i32, y: i32, hand: i32) -> i32;
    fn rsvz_canvas_needs_cursor() -> i32;
    fn rsvz_canvas_capture_cursor(kind: i32, handle: *mut c_void);

    fn rsvz_canvas_begin(device: *mut c_void, draw: *mut c_void, width: i32, height: i32) -> i32;
    fn rsvz_canvas_end() -> i32;
    fn rsvz_canvas_clear() -> i32;
    #[link_name = "rsvz_canvas_rect"]
    fn native_rsvz_canvas_rect(x: i32, y: i32, w: i32, h: i32, argb: u32) -> i32;
    #[link_name = "rsvz_canvas_clip"]
    fn native_rsvz_canvas_clip(enabled: i32, x: i32, y: i32, w: i32, h: i32) -> i32;
    #[link_name = "rsvz_canvas_text"]
    fn native_rsvz_canvas_text(
        x: i32, y: i32, text: *const u8, length: i32, family: *const u16, pixels: i32, bold: i32, argb: u32,
        measure: i32, width: *mut u32, height: *mut u32,
    ) -> i32;
}
fn observe(result: i32) -> i32 {
    if result < 0 && FAILURE.get().is_none() {
        FAILURE.set(Some(result));
    }
    result
}

// Native diagnostics must not block either update or paint. Source-specific
// canvas failures also reach the caller's logger through the paint entry.
pub(super) fn report_pending_failure() {
    if let Some(code) = FAILURE.take() {
        if !FAILURE_REPORTED.replace(true) {
            let message: Vec<u16> = format!("RSVZ native drawing unavailable ({code:#x})\0")
                .encode_utf16()
                .collect();
            // SAFETY: the local UTF-16 buffer is NUL-terminated and lives through the call.
            unsafe {
                windows_sys::Win32::System::Diagnostics::Debug::OutputDebugStringW(message.as_ptr());
            }
        }
    }
}
fn checked(result: i32) -> RuntimeResult<()> {
    let result = observe(result);
    if result < 0 {
        Err(RuntimeError::new(format!("native game drawing failed ({result:#x})")))
    } else {
        Ok(())
    }
}

// These are native graphics-resource links, read for the current invocation;
// never a cached App/Board address. AvZ's 1051 D3D chain agrees with the native
// DD/D3D constructor and present callsites retained in the paint ABI evidence.
unsafe fn interfaces(app: *mut layout::LawnApp) -> Option<(*mut c_void, *mut c_void)> {
    unsafe fn pointer(base: *mut c_void, offset: usize) -> *mut c_void {
        unsafe { std::ptr::read_unaligned(base.cast::<u8>().add(offset).cast()) }
    }
    unsafe {
        let dd = pointer(app.cast(), 0x36c);
        if dd.is_null() {
            return None;
        }
        let d3d = pointer(dd, 0x30);
        if d3d.is_null() {
            return None;
        }
        let device = pointer(d3d, 0x20);
        let draw = pointer(d3d, 0x10);
        if device.is_null() || draw.is_null() || pointer(d3d, 0x14).is_null() || pointer(d3d, 0x1c).is_null() {
            None
        } else {
            Some((device, draw))
        }
    }
}

struct Canvas {
    origin: PixelPos,
    size: (u32, u32),
}
impl PaintCanvas for Canvas {
    fn board_position(&self) -> RuntimeResult<Option<PixelPos>> {
        unsafe {
            let board = layout::LawnApp::board(layout::lawn_app());
            if board.is_null() {
                return Ok(None);
            }
            let (x, y) = layout::Board::view_position(board);
            Ok(Some(PixelPos { x, y }))
        }
    }
    fn set_origin(&mut self, origin: PixelPos) {
        self.origin = origin;
    }

    fn cursor(&mut self, position: PixelPos, hand: bool) -> RuntimeResult<()> {
        checked(unsafe {
            rsvz_canvas_cursor(
                position.x + view_offset() + self.origin.x,
                position.y + self.origin.y,
                i32::from(hand),
            )
        })
    }

    fn cursor_grid_row(&self) -> RuntimeResult<Option<i32>> {
        // SAFETY: current Board owns CursorPreview; native grid row is +0x28.
        unsafe {
            let board = layout::LawnApp::board(layout::lawn_app());
            if board.is_null() {
                return Ok(None);
            }
            let preview = layout::Board::cursor_preview(board);
            if preview.is_null() {
                return Ok(None);
            }
            Ok(Some(std::ptr::read_unaligned(preview.cast::<u8>().add(0x28).cast())))
        }
    }

    fn mouse_position(&self) -> RuntimeResult<PixelPos> {
        // SAFETY: Canvas cannot escape this native App paint scope. WidgetManager
        // reads at 0x4387F3..0x438814 prove +0xE0/+0xE4 canvas coordinates.
        unsafe {
            let mouse = layout::LawnApp::mouse_window(layout::lawn_app());
            if mouse.is_null() {
                return Err(RuntimeError::new("native mouse window unavailable"));
            }
            let base = mouse.cast::<u8>();
            Ok(PixelPos {
                x: std::ptr::read_unaligned::<i32>(base.add(0xe0).cast()) - self.origin.x,
                y: std::ptr::read_unaligned::<i32>(base.add(0xe4).cast()) - self.origin.y,
            })
        }
    }
    fn cursor_sprite_position(&self) -> RuntimeResult<Option<PixelPos>> {
        // SAFETY: native cursor Update writes its GameObject x/y at +8/+12.
        unsafe {
            let board = layout::LawnApp::board(layout::lawn_app());
            if board.is_null() {
                return Ok(None);
            }
            let cursor = layout::Board::cursor_object(board);
            if cursor.is_null() {
                return Ok(None);
            }
            let base = cursor.cast::<u8>();
            Ok(Some(PixelPos {
                x: std::ptr::read_unaligned(base.add(8).cast()),
                y: std::ptr::read_unaligned(base.add(12).cast()),
            }))
        }
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }
    fn fill_rect(&mut self, rect: Rect, color: Argb) -> RuntimeResult<()> {
        // SAFETY: constructed only within the borrowed native paint scope.
        checked(unsafe {
            rsvz_canvas_rect(
                rect.x + view_offset() + self.origin.x,
                rect.y + self.origin.y,
                rect.width,
                rect.height,
                color.0,
            )
        })
    }
    fn text(&mut self, position: PixelPos, text: &str, font: PaintFont<'_>, color: Argb) -> RuntimeResult<()> {
        draw_text(
            PixelPos {
                x: position.x + self.origin.x,
                y: position.y + self.origin.y,
            },
            text,
            font,
            color,
            false,
        )
        .map(|_| ())
    }
    fn measure_text(&mut self, text: &str, font: PaintFont<'_>) -> RuntimeResult<(u32, u32)> {
        draw_text(PixelPos { x: 0, y: 0 }, text, font, Argb(0), true)
    }
    fn measure_digit_bounds(&mut self, font: PaintFont<'_>) -> RuntimeResult<(i32, i32)> {
        let mut family = [0u16; 32];
        for (slot, value) in family.iter_mut().zip(font.family().encode_utf16()) {
            *slot = value;
        }
        let (mut top, mut bottom) = (0, 0);
        // SAFETY: the font name and scalar outputs live through this synchronous native query.
        checked(unsafe {
            rsvz_canvas_digit_bounds(
                family.as_ptr(),
                i32::from(font.pixels()),
                i32::from(font.bold()),
                &mut top,
                &mut bottom,
            )
        })?;
        Ok((top, bottom))
    }
    fn set_clip(&mut self, rect: Option<Rect>) -> RuntimeResult<()> {
        let value = rect.unwrap_or_default();
        checked(unsafe {
            rsvz_canvas_clip(
                i32::from(rect.is_some()),
                value.x + view_offset() + self.origin.x,
                value.y + self.origin.y,
                value.width,
                value.height,
            )
        })
    }
}
fn draw_text(
    position: PixelPos, text: &str, font: PaintFont<'_>, color: Argb, measure: bool,
) -> RuntimeResult<(u32, u32)> {
    let length = i32::try_from(text.len()).map_err(|_| RuntimeError::new("text is too long for native drawing"))?;
    let mut family = [0u16; 32];
    for (slot, value) in family.iter_mut().zip(font.family().encode_utf16()) {
        *slot = value;
    }
    let (mut width, mut height) = (0, 0);
    checked(unsafe {
        rsvz_canvas_text(
            position.x + view_offset(),
            position.y,
            text.as_ptr(),
            length,
            family.as_ptr(),
            i32::from(font.pixels()),
            i32::from(font.bold()),
            color.0,
            i32::from(measure),
            &mut width,
            &mut height,
        )
    })?;
    Ok((width, height))
}

fn ensure_hook() -> Result<()> {
    if HOOK.with_borrow(|slot| slot.as_ref().is_some_and(PaintHook::installed)) {
        return Ok(());
    }
    HOOK.with_borrow_mut(|slot| {
        if slot.is_none() {
            *slot = Some(PaintHook::new(
                draw_hook as *const () as usize,
                before_device_reset as *const () as usize,
            ));
        }
        let hook = slot.as_mut().expect("owned paint hook");
        if let Err(error) = hook.install() {
            // Keep an unsuccessfully restored owner reachable for cleanup.
            if hook.restore().is_ok() {
                *slot = None;
            }
            return Err(error);
        }
        Ok(())
    })?;
    Ok(())
}
extern "C" fn before_device_reset() {
    let _depth = super::lifecycle::ActiveHookGuard::enter();
    // Drop our COM references before native DD Cleanup changes cooperative mode.
    // Keeping the previous device alive can block SetCooperativeLevel on Windows.
    // The replaced original is the verified no-op RET at 0x5509A0.
    observe(unsafe { rsvz_canvas_clear() });
    unsafe {
        crate::patches::window_view::before_reset();
    }
}
pub(crate) fn set_view_enabled(enabled: bool) -> Result<()> {
    if active() {
        return Err(Pvz1051Error::OperationRejected(
            "change viewport outside the draw stack",
        ));
    }
    if enabled {
        ensure_hook()
    } else if ENTRY.get().is_none() {
        restore()
    } else {
        Ok(())
    }
}
fn view_offset() -> i32 {
    if crate::patches::window_view::enabled() {
        -crate::patches::window_view::left(crate::patches::window_view::width())
    } else {
        0
    }
}

impl OptionalOverlayBackend for Pvz1051Backend {
    const OVERLAY_IS_NOOP: bool = false;
    fn set_paint_entry(&self, entry: Option<fn(&mut Self)>) -> Result<()> {
        if let Some(result) = crate::module_link::paint(entry.is_some()) {
            result?;
            ENTRY.set(entry);
            return Ok(());
        }
        if active() {
            return Err(Pvz1051Error::OperationRejected(
                "change paint registration outside the draw stack",
            ));
        }
        if let Some(next) = entry {
            if let Some(current) = ENTRY.get() {
                return if std::ptr::fn_addr_eq(current, next) {
                    Ok(())
                } else {
                    Err(Pvz1051Error::OperationRejected(
                        "another physical paint entry is installed",
                    ))
                };
            }
            // Device availability is checked at each draw: registering overlays
            // must not prevent using nonvisual controls while 3D is disabled.
            ensure_hook()?;
            ENTRY.set(Some(next));
            Ok(())
        } else {
            ENTRY.set(None);
            if crate::patches::window_view::enabled() {
                checked(unsafe { rsvz_canvas_clear() }).map_err(|error| Pvz1051Error::Patch(error.to_string()))
            } else {
                restore()
            }
        }
    }
    fn with_paint_canvas<R>(body: impl FnOnce(&mut dyn PaintCanvas) -> R) -> Result<R> {
        let size = VIEWPORT.get().ok_or(Pvz1051Error::OperationRejected(
            "native drawing device unavailable; enable 3D acceleration",
        ))?;
        if BORROWED.replace(true) {
            return Err(Pvz1051Error::OperationRejected("paint canvas is already borrowed"));
        }
        struct Release;
        impl Drop for Release {
            fn drop(&mut self) {
                BORROWED.set(false);
            }
        }
        let _release = Release;
        Ok(body(&mut Canvas {
            size,
            origin: PixelPos { x: 0, y: 0 },
        }))
    }
}
impl NativePaintBackend for Pvz1051Backend {}

pub(super) fn restore() -> Result<()> {
    if active() {
        return Err(Pvz1051Error::OperationRejected("paint callback is still active"));
    }
    HOOK.with_borrow_mut(|slot| {
        if let Some(hook) = slot.as_mut() {
            hook.restore()?;
        }
        *slot = None;
        Ok(())
    })?;
    ENTRY.set(None);
    FAILURE.set(None);
    FAILURE_REPORTED.set(false);
    checked(unsafe { rsvz_canvas_clear() }).map_err(|error| Pvz1051Error::Patch(error.to_string()))
}

extern "C" fn draw_hook() -> u8 {
    if crate::diagnostics::modal_active() {
        return 0;
    }
    let _depth = super::lifecycle::ActiveHookGuard::enter();
    let previous = DRAWING.replace(true);
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            DRAWING.set(self.0);
        }
    }
    let _drawing = Restore(previous);
    // Verified native thiscall: ECX App, no stack arguments, returns AL, RET.
    if !previous && crate::patches::window_view::enabled() {
        if unsafe { crate::patches::window_view::prepare_paint() }.is_err() {
            // Let native Draw/Present recover a lost device; skipping that path
            // here would prevent the game's own surface restoration.
            observe(0x80004005u32 as i32);
        }
    }
    let drawn = unsafe { abi::lawn_app_draw_dirty() };
    if drawn != 0 {
        if !previous {
            let result = catch_unwind(AssertUnwindSafe(|| {
                let Some(entry) = ENTRY.get() else {
                    return;
                };
                super::dispatch_entry::with_installed(|backend, _| {
                    let app = backend.app().as_ptr();
                    let Some((device, draw)) = (unsafe { interfaces(app) }) else {
                        observe(0x80004002u32 as i32);
                        entry(backend);
                        return;
                    };
                    let (width, height) =
                        unsafe { (layout::LawnApp::canvas_width(app), layout::LawnApp::canvas_height(app)) };
                    if observe(unsafe {
                        rsvz_canvas_begin(
                            device,
                            draw,
                            if crate::patches::window_view::enabled() {
                                crate::patches::window_view::width()
                            } else {
                                width
                            },
                            height,
                        )
                    }) < 0
                    {
                        entry(backend);
                        return;
                    }
                    // Capture the game's own cursor resources when they are shown,
                    // as AvZ does; use system arrow/hand until a resource is seen.
                    if unsafe { rsvz_canvas_needs_cursor() } != 0 {
                        use windows_sys::Win32::UI::WindowsAndMessaging::{CURSORINFO, GetCursorInfo};
                        unsafe {
                            let mouse = layout::LawnApp::mouse_window(app);
                            if !mouse.is_null() && std::ptr::read(mouse.cast::<u8>().add(0xdc)) != 0 {
                                let kind: i32 = std::ptr::read_unaligned(app.cast::<u8>().add(0x4b0).cast());
                                if matches!(kind, 0 | 1) {
                                    let mut cursor = CURSORINFO {
                                        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
                                        ..std::mem::zeroed()
                                    };
                                    if GetCursorInfo(&mut cursor) != 0 {
                                        rsvz_canvas_capture_cursor(kind, cursor.hCursor);
                                    }
                                }
                            }
                        }
                    }
                    struct Scope;
                    impl Drop for Scope {
                        fn drop(&mut self) {
                            VIEWPORT.set(None);
                            observe(unsafe { rsvz_canvas_end() });
                        }
                    }
                    let _scope = Scope;
                    VIEWPORT.set(Some((width as u32, height as u32)));
                    entry(backend);
                });
            }));
            if result.is_err() {
                // Request only; actual teardown waits until the draw hook exits.
                super::lifecycle::fail_and_request_unload();
            }
        }
        // The displaced 0x54C8CD call pushes 0 and uses ECX App; native RET 4.
        unsafe { abi::lawn_app_present() };
    }
    drawn
}

pub(crate) fn viewport() -> Option<(u32, u32)> {
    VIEWPORT.get()
}
pub(crate) fn dispatch_module(width: u32, height: u32) {
    if DRAWING.replace(true) {
        return;
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            DRAWING.set(false);
            VIEWPORT.set(None);
        }
    }
    let _reset = Reset;
    VIEWPORT.set((width != 0 && height != 0).then_some((width, height)));
    if let Some(entry) = ENTRY.get() {
        entry(&mut crate::module_link::token());
    }
}

pub(crate) unsafe extern "C" fn rsvz_canvas_cursor(x: i32, y: i32, hand: i32) -> i32 {
    if let Some(host) = crate::module_link::services() {
        return unsafe { (host.canvas_cursor)(x, y, hand) };
    }
    unsafe { native_rsvz_canvas_cursor(x, y, hand) }
}
pub(crate) unsafe extern "C" fn rsvz_canvas_digit_bounds(
    family: *const u16, pixels: i32, bold: i32, top: *mut i32, bottom: *mut i32,
) -> i32 {
    if let Some(host) = crate::module_link::services() {
        return unsafe { (host.canvas_digit_bounds)(family, pixels, bold, top, bottom) };
    }
    unsafe { native_rsvz_canvas_digit_bounds(family, pixels, bold, top, bottom) }
}

pub(crate) unsafe extern "C" fn rsvz_canvas_rect(x: i32, y: i32, w: i32, h: i32, argb: u32) -> i32 {
    if let Some(host) = crate::module_link::services() {
        return unsafe { (host.canvas_rect)(x, y, w, h, argb) };
    }
    unsafe { native_rsvz_canvas_rect(x, y, w, h, argb) }
}

pub(crate) unsafe extern "C" fn rsvz_canvas_clip(enabled: i32, x: i32, y: i32, w: i32, h: i32) -> i32 {
    if let Some(host) = crate::module_link::services() {
        return unsafe { (host.canvas_clip)(enabled, x, y, w, h) };
    }
    unsafe { native_rsvz_canvas_clip(enabled, x, y, w, h) }
}

pub(crate) unsafe extern "C" fn rsvz_canvas_text(
    x: i32, y: i32, text: *const u8, length: i32, family: *const u16, pixels: i32, bold: i32, argb: u32, measure: i32,
    width: *mut u32, height: *mut u32,
) -> i32 {
    if let Some(host) = crate::module_link::services() {
        return unsafe { (host.canvas_text)(x, y, text, length, family, pixels, bold, argb, measure, width, height) };
    }
    unsafe { native_rsvz_canvas_text(x, y, text, length, family, pixels, bold, argb, measure, width, height) }
}
