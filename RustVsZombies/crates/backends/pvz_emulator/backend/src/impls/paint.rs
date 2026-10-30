//! Pure overlay configuration is accepted on PE; actual canvas access is not.
use crate::{PeBackend, PeBackendError};
use rsvz_backend_api::{OptionalOverlayBackend, PaintCanvas};
impl OptionalOverlayBackend for PeBackend {
    const OVERLAY_IS_NOOP: bool = true;
    fn set_paint_entry(&self, entry: Option<fn(&mut Self)>) -> Result<(), PeBackendError> {
        if entry.is_some() {
            Err(PeBackendError::Unsupported("native painting"))
        } else {
            Ok(())
        }
    }
    fn with_paint_canvas<R>(_: impl FnOnce(&mut dyn PaintCanvas) -> R) -> Result<R, PeBackendError> {
        Err(PeBackendError::Unsupported("native painting"))
    }
}

impl rsvz_backend_api::GameHudBackend for PeBackend {
    const HUD_IS_NOOP: bool = true;
    fn hud_field(&self, _: rsvz_backend_api::HudField) -> Result<i32, PeBackendError> {
        Err(PeBackendError::Unsupported("native HUD"))
    }
    fn set_hud_field(&self, _: rsvz_backend_api::HudField, _: i32) -> Result<(), PeBackendError> {
        Err(PeBackendError::Unsupported("native HUD"))
    }
}
