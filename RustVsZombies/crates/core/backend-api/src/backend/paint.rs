//! Native paint scope. These operations draw on the real game canvas, never on
//! the modifier window or an invented headless framebuffer.
use crate::{backend::Backend, error::RuntimeResult};
use rsvz_model::{Argb, PaintFont, PixelPos, Rect};

/// Only native paint callbacks receive this borrowed canvas. Implementations
/// preserve the game's graphics state and report native resource/device errors.
pub trait PaintCanvas {
    /// Native Board origin, independent of automatic viewport alignment.
    fn board_position(&self) -> RuntimeResult<Option<PixelPos>>;
    /// Set the origin for subsequent drawing and mouse-coordinate reads in this scope.
    fn set_origin(&mut self, origin: PixelPos);
    fn size(&self) -> (u32, u32);
    fn cursor(&mut self, position: PixelPos, hand: bool) -> RuntimeResult<()>;
    /// Current native canvas pointer; never the operating system screen coordinates.
    fn mouse_position(&self) -> RuntimeResult<PixelPos>;
    /// The game cursor sprite has a different origin and may be absent in menus.
    fn cursor_sprite_position(&self) -> RuntimeResult<Option<PixelPos>>;
    fn cursor_grid_row(&self) -> RuntimeResult<Option<i32>>;

    fn fill_rect(&mut self, rect: Rect, color: Argb) -> RuntimeResult<()>;
    /// Top-left anchored UTF-8 text, with newline support.
    fn text(&mut self, position: PixelPos, text: &str, font: PaintFont<'_>, color: Argb) -> RuntimeResult<()>;
    fn measure_text(&mut self, text: &str, font: PaintFont<'_>) -> RuntimeResult<(u32, u32)>;
    /// Union of 0–9 ink bounds relative to `text`'s y, as (top, exclusive bottom).
    /// Independent of the displayed number, so changing digits preserves the baseline.
    fn measure_digit_bounds(&mut self, font: PaintFont<'_>) -> RuntimeResult<(i32, i32)>;
    /// None restores the native canvas bounds for this scope.
    fn set_clip(&mut self, rect: Option<Rect>) -> RuntimeResult<()>;
}

/// Optional game-overlay service. Pure-display configuration may accept a
/// headless no-op; it must return before creating callbacks, tasks or resources.
pub trait OptionalOverlayBackend: Backend {
    const OVERLAY_IS_NOOP: bool;
    fn set_paint_entry(&self, entry: Option<fn(&mut Self)>) -> Result<(), Self::Error>;
    /// Errors outside the native paint scope. A no-op backend never creates a
    /// canvas or calls `body`; it rejects this low-level request as unavailable.
    fn with_paint_canvas<R>(body: impl FnOnce(&mut dyn PaintCanvas) -> R) -> Result<R, Self::Error>;
}

/// Actual native paint availability. Public paint callbacks/image-facing APIs
/// require this capability; a headless backend intentionally does not implement it.
pub trait NativePaintBackend: OptionalOverlayBackend {}

/// Atomic original HUD fields. The coin-bank value is a native fade countdown,
/// not a second visibility boolean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum HudField {
    ShovelVisible,
    CoinBankFadeCountdown,
    MenuButtonHidden,
}
pub trait GameHudBackend: Backend {
    const HUD_IS_NOOP: bool;
    fn hud_field(&self, field: HudField) -> Result<i32, Self::Error>;
    fn set_hud_field(&self, field: HudField, value: i32) -> Result<(), Self::Error>;
}
