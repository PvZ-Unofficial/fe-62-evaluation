//! Native cursor atoms. Coordinates are deliberately separate: the plant cursor
//! is offset from, and can lag behind, the actual widget-manager mouse.
use crate::Backend;
use rsvz_model::{PixelPos, SeedSlot};

pub trait ScreenButtonBackend: Backend {
    fn replace_screen_buttons(&self, source: u8, buttons: &[rsvz_model::ScreenButton]) -> Result<(), Self::Error>;
    fn take_screen_button(&self, source: u8) -> Result<Option<u32>, Self::Error>;
    fn screen_buttons_available(&self, source: u8) -> Result<bool, Self::Error>;
}

pub trait CursorPositionBackend: Backend {
    /// Actual game-canvas mouse coordinates, including negative/outside values.
    fn cursor_position(&self) -> Result<PixelPos, Self::Error>;
}
pub trait CursorObjectPositionBackend: Backend {
    /// Native CursorObject sprite origin, used by A-TAS smart/six-row actions.
    fn cursor_object_position(&self) -> Result<PixelPos, Self::Error>;
}
pub trait CursorSeedSlotBackend: Backend {
    /// Only bank-held cursors return a slot; malformed bank indices are errors.
    fn cursor_seed_slot(&self) -> Result<Option<SeedSlot>, Self::Error>;
}
pub trait CursorGridBackend: Backend {
    /// Native CursorPreview row, zero-based; negative means no preview row.
    fn cursor_grid_row(&self) -> Result<i32, Self::Error>;
}

/// Access to the six physical native row slots. Ordinary scene-bounded placement
/// remains unchanged. Headless backends omit this explicit interactive capability.
pub trait NativePlantGridBackend: crate::PlantReadBackend {
    fn can_plant_at_native_grid(
        &self, selection: rsvz_model::CheckedCardSelection, grid: rsvz_model::Grid,
    ) -> Result<rsvz_model::Plantability, Self::Error>;
    fn new_plant_at_native_grid<'a>(
        &'a self, selection: rsvz_model::CheckedCardSelection, grid: rsvz_model::Grid,
    ) -> Result<Self::PlantHandle<'a>, Self::Error>;
}
pub trait CobSelectionBackend: crate::PlantReadBackend {
    /// Select a currently ready cannon for the user's next native click.
    fn select_cob(&self, id: rsvz_model::PlantId) -> Result<(), Self::Error>;
}

/// Consumed Board clicks, already mapped by the native window into Board coordinates.
/// Reading returns None when empty; overflow is an error, never a silently lost click.
pub trait BoardClickBackend: Backend {
    fn set_board_click_capture(&self, enabled: bool) -> Result<(), Self::Error>;
    /// Opt-in handoff: a native held card/tool permanently yields this capture.
    fn set_board_click_native_priority(&self, enabled: bool) -> Result<(), Self::Error>;
    fn board_click_capture_active(&self) -> Result<bool, Self::Error>;
    fn set_lawn_preview(&self, preview: Option<rsvz_model::LawnPreview>) -> Result<(), Self::Error>;
    fn take_board_click(&self) -> Result<Option<(i32, i32, i32)>, Self::Error>;
}
