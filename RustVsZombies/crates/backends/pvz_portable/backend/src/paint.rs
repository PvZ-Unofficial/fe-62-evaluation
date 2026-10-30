//! Presentation-only ingress using the physical host's existing backend token.
use crate::{PortableBackend, PortableBackendError, Result};
use pvzp_rs::raw;
use rsvz_backend_api::error::{RuntimeError, RuntimeResult};
use rsvz_backend_api::{NativePaintBackend, OptionalOverlayBackend, PaintCanvas};
use rsvz_model::{Argb, PaintFont, PixelPos, Rect};
use std::cell::Cell;
thread_local! {
    static ENTRY: Cell<Option<fn(&mut PortableBackend)>> = const { Cell::new(None) };
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static BORROWED: Cell<bool> = const { Cell::new(false) };
}
#[cfg(unix)]
#[path = "paint_unix.rs"]
pub(crate) mod unix;
#[cfg(unix)]
use unix::draw_text;

pub(crate) fn active() -> bool {
    ACTIVE.get()
}

struct Canvas {
    origin: PixelPos,
    size: (u32, u32),
}
impl Canvas {
    // Keep the native bridge call inside the shared backend. Downstream paint
    // closures instantiate with_paint_canvas in their own module DLL.
    fn new() -> Result<Self> {
        let (mut width, mut height) = (0, 0);
        if unsafe { raw::pvzp_rs_paint_size(&mut width, &mut height) } == 0 {
            return Err(PortableBackendError::OperationRejected("native canvas has no viewport"));
        }
        Ok(Self {
            size: (width, height),
            origin: PixelPos { x: 0, y: 0 },
        })
    }
}
fn checked(success: u8) -> RuntimeResult<()> {
    if success != 0 {
        Ok(())
    } else {
        Err(RuntimeError::new("native game drawing failed"))
    }
}
impl PaintCanvas for Canvas {
    fn board_position(&self) -> RuntimeResult<Option<PixelPos>> {
        // SAFETY: synchronous native paint scope prevents Board invalidation.
        unsafe {
            let Some(board) = std::ptr::NonNull::new(raw::pvzp_rs_board_from_live_app()) else {
                return Ok(None);
            };
            let world = pvzp_rs::World::from_non_null(board);
            let (x, y) = world
                .board_view()
                .map_err(|error| RuntimeError::new(error.to_string()))?;
            Ok(Some(PixelPos { x, y }))
        }
    }
    fn set_origin(&mut self, origin: PixelPos) {
        self.origin = origin;
    }

    fn cursor(&mut self, position: PixelPos, hand: bool) -> RuntimeResult<()> {
        checked(unsafe {
            raw::pvzp_rs_paint_cursor(position.x + self.origin.x, position.y + self.origin.y, u8::from(hand))
        })
    }

    fn cursor_grid_row(&self) -> RuntimeResult<Option<i32>> {
        let mut row = 0;
        Ok((unsafe { raw::pvzp_rs_paint_grid_row(&mut row) } != 0).then_some(row))
    }

    fn mouse_position(&self) -> RuntimeResult<PixelPos> {
        let (mut x, mut y) = (0, 0);
        checked(unsafe { raw::pvzp_rs_paint_pointer(0, &mut x, &mut y) })?;
        Ok(PixelPos {
            x: x - self.origin.x,
            y: y - self.origin.y,
        })
    }
    fn cursor_sprite_position(&self) -> RuntimeResult<Option<PixelPos>> {
        let (mut x, mut y) = (0, 0);
        Ok((unsafe { raw::pvzp_rs_paint_pointer(1, &mut x, &mut y) } != 0).then_some(PixelPos { x, y }))
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }
    fn fill_rect(&mut self, rect: Rect, color: Argb) -> RuntimeResult<()> {
        checked(unsafe {
            raw::pvzp_rs_paint_rect(
                rect.x + self.origin.x,
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
        #[cfg(unix)]
        {
            unix::measure_digit_bounds(font)
        }
        #[cfg(windows)]
        {
            let mut family = [0u16; 32];
            for (slot, value) in family.iter_mut().zip(font.family().encode_utf16()) {
                *slot = value;
            }
            let (mut top, mut bottom) = (0, 0);
            // SAFETY: the family and output pointers remain valid throughout this synchronous query.
            checked(unsafe {
                raw::pvzp_rs_paint_digit_bounds(
                    family.as_ptr(),
                    i32::from(font.pixels()),
                    u8::from(font.bold()),
                    &mut top,
                    &mut bottom,
                )
            })?;
            Ok((top, bottom))
        }
    }
    fn set_clip(&mut self, rect: Option<Rect>) -> RuntimeResult<()> {
        let value = rect.unwrap_or_default();
        checked(unsafe {
            raw::pvzp_rs_paint_clip(
                u8::from(rect.is_some()),
                value.x + self.origin.x,
                value.y + self.origin.y,
                value.width,
                value.height,
            )
        })
    }
}
#[cfg(windows)]
fn draw_text(
    position: PixelPos, text: &str, font: PaintFont<'_>, color: Argb, measure: bool,
) -> RuntimeResult<(u32, u32)> {
    let length = i32::try_from(text.len()).map_err(|_| RuntimeError::new("text exceeds native length"))?;
    let mut family = [0u16; 32];
    for (slot, value) in family.iter_mut().zip(font.family().encode_utf16()) {
        *slot = value;
    }
    let (mut width, mut height) = (0, 0);
    checked(unsafe {
        raw::pvzp_rs_paint_text(
            position.x,
            position.y,
            text.as_ptr().cast(),
            length,
            family.as_ptr(),
            i32::from(font.pixels()),
            u8::from(font.bold()),
            color.0,
            u8::from(measure),
            &mut width,
            &mut height,
        )
    })?;
    Ok((width, height))
}
impl OptionalOverlayBackend for PortableBackend {
    const OVERLAY_IS_NOOP: bool = false;
    fn set_paint_entry(&self, entry: Option<fn(&mut Self)>) -> Result<()> {
        if let Some(result) = crate::module_link::paint(entry.is_some()) {
            result?;
            ENTRY.set(entry);
            #[cfg(unix)]
            if entry.is_none() {
                unix::clear();
            }
            return Ok(());
        }
        if ACTIVE.get() {
            return Err(PortableBackendError::OperationRejected(
                "change paint registration outside paint",
            ));
        }
        if let (Some(current), Some(next)) = (ENTRY.get(), entry) {
            if !std::ptr::fn_addr_eq(current, next) {
                return Err(PortableBackendError::OperationRejected(
                    "another paint entry is installed",
                ));
            }
        }
        // SAFETY: native registration checks the validated live App and refuses
        // mutation inside paint. The callback is revoked before physical unload.
        if unsafe { raw::pvzp_rs_set_paint(entry.map(|_| dispatch as unsafe extern "C" fn())) } == 0 {
            return Err(PortableBackendError::OperationRejected(
                "native paint registration rejected",
            ));
        }
        #[cfg(unix)]
        if entry.is_none() {
            unix::clear();
        }
        ENTRY.set(entry);
        Ok(())
    }
    fn with_paint_canvas<R>(body: impl FnOnce(&mut dyn PaintCanvas) -> R) -> Result<R> {
        if !ACTIVE.get() || BORROWED.replace(true) {
            return Err(PortableBackendError::OperationRejected(
                "native canvas unavailable or already borrowed",
            ));
        }
        struct Release;
        impl Drop for Release {
            fn drop(&mut self) {
                BORROWED.set(false);
            }
        }
        let _release = Release;
        Ok(body(&mut Canvas::new()?))
    }
}
impl NativePaintBackend for PortableBackend {}
pub(crate) unsafe extern "C" fn dispatch() {
    if ACTIVE.replace(true) {
        return;
    }
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            ACTIVE.set(false);
        }
    }
    let _release = Release;
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(entry) = ENTRY.get() {
            if crate::module_link::attached() {
                entry(&mut PortableBackend::new());
            } else {
                crate::host::dispatch_paint(entry);
            }
        }
    }))
    .is_err()
    {
        crate::host::native_event_panicked();
    }
}

impl rsvz_backend_api::GameHudBackend for PortableBackend {
    const HUD_IS_NOOP: bool = false;
    fn hud_field(&self, field: rsvz_backend_api::HudField) -> Result<i32> {
        pvzp_rs::modifier::hud_read(field as u32).map_err(Into::into)
    }
    fn set_hud_field(&self, field: rsvz_backend_api::HudField, value: i32) -> Result<()> {
        pvzp_rs::modifier::hud_write(field as u32, value).map_err(Into::into)
    }
}
