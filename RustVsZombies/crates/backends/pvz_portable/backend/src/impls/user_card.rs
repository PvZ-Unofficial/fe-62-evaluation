use crate::{PortableBackend, PortableBackendError, Result};
use rsvz_backend_api::{CursorGridBackend, CursorObjectPositionBackend, CursorPositionBackend, CursorSeedSlotBackend};
use rsvz_model::{PixelPos, SeedSlot};

#[cfg(target_os = "android")]
impl rsvz_backend_api::ScreenButtonBackend for PortableBackend {
    fn replace_screen_buttons(&self, source: u8, buttons: &[rsvz_model::ScreenButton]) -> Result<()> {
        let labels = buttons
            .iter()
            .map(|button| crate::paint::unix::button_label(&button.label))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| {
                crate::ffi::log(&error.to_string());
                PortableBackendError::OperationRejected("screen button font rendering failed")
            })?;
        // Configuration is installed only at the game safe point; native copies pixels.
        unsafe { pvzp_rs::raw::pvzp_rs_clear_screen_buttons(u32::from(source)) };
        for (button, (pixels, width, height)) in buttons.iter().zip(labels) {
            if !unsafe {
                pvzp_rs::raw::pvzp_rs_set_screen_button(
                    u32::from(source),
                    button.id,
                    button.right,
                    u32::from(button.first),
                    u32::from(button.span),
                    pixels.as_ptr(),
                    width,
                    height,
                )
            } {
                return Err(PortableBackendError::OperationRejected(
                    "screen button placement rejected",
                ));
            }
        }
        Ok(())
    }
    fn take_screen_button(&self, source: u8) -> Result<Option<u32>> {
        match unsafe { pvzp_rs::raw::pvzp_rs_take_screen_button(u32::from(source)) } {
            0 => Ok(None),
            u32::MAX => Err(PortableBackendError::OperationRejected("screen button input overflow")),
            id => Ok(Some(id)),
        }
    }
    fn screen_buttons_available(&self, source: u8) -> Result<bool> {
        Ok(unsafe { pvzp_rs::raw::pvzp_rs_screen_buttons_available(u32::from(source)) })
    }
}
impl CursorPositionBackend for PortableBackend {
    fn cursor_position(&self) -> Result<PixelPos> {
        let (x, y) = self.world()?.cursor_position()?;
        Ok(PixelPos { x, y })
    }
}
impl CursorObjectPositionBackend for PortableBackend {
    fn cursor_object_position(&self) -> Result<PixelPos> {
        let (x, y) = self.world()?.cursor_object_position()?;
        Ok(PixelPos { x, y })
    }
}
impl CursorGridBackend for PortableBackend {
    fn cursor_grid_row(&self) -> Result<i32> {
        self.world()?.cursor_grid_row().map_err(Into::into)
    }
}
impl CursorSeedSlotBackend for PortableBackend {
    fn cursor_seed_slot(&self) -> Result<Option<SeedSlot>> {
        let index = self.world()?.cursor_seed_slot()?;
        if index < 0 {
            return Ok(None);
        }
        SeedSlot::new(index as usize)
            .map(Some)
            .map_err(|_| PortableBackendError::OperationRejected("invalid held seed slot"))
    }
}

use rsvz_backend_api::{CobSelectionBackend, NativePlantGridBackend};
use rsvz_model::{CheckedCardSelection, Grid, PlantId, PlantKind, Plantability};
impl NativePlantGridBackend for PortableBackend {
    fn can_plant_at_native_grid(&self, selection: CheckedCardSelection, grid: Grid) -> Result<Plantability> {
        if !grid.is_in_bounds(6, 9) {
            return Err(PortableBackendError::OperationRejected("invalid physical grid"));
        }
        Ok(Plantability::from_game_rule_code(self.world()?.can_plant_at(
            selection.effective_kind().code(),
            grid.col,
            grid.row,
        )?))
    }
    fn new_plant_at_native_grid<'a>(
        &'a self, selection: CheckedCardSelection, grid: Grid,
    ) -> Result<Self::PlantHandle<'a>> {
        if !grid.is_in_bounds(6, 9) {
            return Err(PortableBackendError::OperationRejected("invalid physical grid"));
        }
        self.world()?
            .new_plant_native_grid(
                grid.col,
                grid.row,
                selection.packet_kind().code(),
                selection.imitator_target().map_or(-1, PlantKind::code),
            )?
            .map(pvzp_rs::Borrowed::as_non_null)
            .map(crate::handles::PortablePlantHandle::new)
            .ok_or(PortableBackendError::OperationRejected("NewPlant returned null"))
    }
}
impl CobSelectionBackend for PortableBackend {
    fn select_cob(&self, id: PlantId) -> Result<()> {
        // Resolve the public identity before crossing the raw FFI boundary. This
        // also enforces world ownership when Replay replaces the native pools.
        if rsvz_backend_api::PlantReadBackend::plant(self, id)?.is_none() {
            return Err(PortableBackendError::OperationRejected(
                "cannon identity is no longer current",
            ));
        }
        self.world()?.select_cob(id.raw()).map_err(Into::into)
    }
}

impl rsvz_backend_api::BoardClickBackend for PortableBackend {
    fn set_board_click_capture(&self, enabled: bool) -> Result<()> {
        if let Some(host) = crate::module_link::services() {
            return crate::module_link::checked((host.click_set)(crate::module_link::source(), enabled));
        }
        // SAFETY: game-thread control, no world pointer crosses the boundary.
        unsafe { pvzp_rs::raw::pvzp_rs_set_board_click_capture(enabled) };
        Ok(())
    }
    fn set_board_click_native_priority(&self, enabled: bool) -> Result<()> {
        if let Some(host) = crate::module_link::services() {
            return crate::module_link::checked((host.click_priority)(enabled));
        }
        unsafe { pvzp_rs::raw::pvzp_rs_set_board_click_native_priority(enabled) };
        Ok(())
    }
    fn board_click_capture_active(&self) -> Result<bool> {
        if let Some(host) = crate::module_link::services() {
            let value = (host.click_active)();
            crate::module_link::checked(u32::from(value > 1))?;
            return Ok(value == 1);
        }
        Ok(unsafe { pvzp_rs::raw::pvzp_rs_board_click_capture_active() })
    }
    fn set_lawn_preview(&self, preview: Option<rsvz_model::LawnPreview>) -> Result<()> {
        if let Some(host) = crate::module_link::services() {
            let (kind, imitate) = match preview {
                None => (-1, false),
                Some(rsvz_model::LawnPreview::Shovel) => (-2, false),
                Some(rsvz_model::LawnPreview::Plant(p)) => (i32::from(p.kind()), p.imitater() >= 0),
            };
            return crate::module_link::checked((host.preview)(crate::module_link::source(), kind, imitate));
        }
        let (kind, imitate) = match preview {
            None => (-1, false),
            Some(rsvz_model::LawnPreview::Shovel) => (-2, false),
            Some(rsvz_model::LawnPreview::Plant(p)) => (i32::from(p.kind()), p.imitater() >= 0),
        };
        unsafe { pvzp_rs::raw::pvzp_rs_set_lawn_preview(kind, imitate) };
        Ok(())
    }
    fn take_board_click(&self) -> Result<Option<(i32, i32, i32)>> {
        if let Some(host) = crate::module_link::services() {
            let mut value = [0; 3];
            let status = unsafe { (host.click_take)(crate::module_link::source(), value.as_mut_ptr()) };
            crate::module_link::checked(u32::from(status > 1))?;
            return Ok((status == 1).then_some((value[0], value[1], value[2])));
        }
        let (mut x, mut y, mut button) = (0, 0, 0);
        // SAFETY: output pointers are live for this call; native queue holds values only.
        match unsafe { pvzp_rs::raw::pvzp_rs_take_board_click(&mut x, &mut y, &mut button) } {
            0 => Ok(None),
            1 => Ok(Some((x, y, button))),
            _ => Err(PortableBackendError::OperationRejected("lawn click queue overflow")),
        }
    }
}
