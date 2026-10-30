use crate::{
    error::{Pvz1051Error, Result},
    raw::layout as ptrs,
    runtime::Pvz1051Backend,
};
use rsvz_backend_api::{CursorGridBackend, CursorObjectPositionBackend, CursorPositionBackend, CursorSeedSlotBackend};
use rsvz_model::{PixelPos, SeedSlot};
impl CursorPositionBackend for Pvz1051Backend {
    fn cursor_position(&self) -> Result<PixelPos> {
        // SAFETY: live App; WidgetManager coordinates are proven by 0x4387F3..0x438814.
        unsafe {
            let p = ptrs::LawnApp::mouse_window(self.app().as_ptr());
            if p.is_null() {
                return Err(Pvz1051Error::OperationRejected("no mouse window"));
            }
            let (x, y) = ptrs::MouseWindow::position(p);
            Ok(PixelPos { x, y })
        }
    }
}
impl CursorObjectPositionBackend for Pvz1051Backend {
    fn cursor_object_position(&self) -> Result<PixelPos> {
        // SAFETY: current Board and checked CursorObject, same native Update offsets.
        unsafe {
            let p = ptrs::Board::cursor_object(self.board()?.as_ptr());
            if p.is_null() {
                return Err(Pvz1051Error::OperationRejected("no cursor object"));
            }
            let (x, y) = ptrs::CursorObject::position(p);
            Ok(PixelPos { x, y })
        }
    }
}
impl CursorGridBackend for Pvz1051Backend {
    fn cursor_grid_row(&self) -> Result<i32> {
        // SAFETY: current Board owns checked native CursorPreview; mGridY is +0x28.
        unsafe {
            let p = ptrs::Board::cursor_preview(self.board()?.as_ptr());
            if p.is_null() {
                return Err(Pvz1051Error::OperationRejected("no cursor preview"));
            }
            Ok(ptrs::CursorPreview::grid_row(p))
        }
    }
}
impl CursorSeedSlotBackend for Pvz1051Backend {
    fn cursor_seed_slot(&self) -> Result<Option<SeedSlot>> {
        // SAFETY: current cursor and bank, existing MouseDownWithPlant-verified fields.
        unsafe {
            let p = ptrs::Board::cursor_object(self.board()?.as_ptr());
            if p.is_null() {
                return Err(Pvz1051Error::OperationRejected("no cursor object"));
            }
            if ptrs::CursorObject::cursor_type(p) != 1 {
                return Ok(None);
            }
            let index = ptrs::CursorObject::seed_bank_index(p);
            let count = ptrs::SeedBank::packet_count(self.seed_bank()?.as_ptr());
            if index < 0 || index >= count {
                return Err(Pvz1051Error::OperationRejected("invalid held seed slot"));
            }
            SeedSlot::new(index as usize)
                .map(Some)
                .map_err(|_| Pvz1051Error::OperationRejected("invalid held seed slot"))
        }
    }
}

use rsvz_backend_api::{CobSelectionBackend, NativePlantGridBackend, PlantPoolBackend, PlantReadBackend};
use rsvz_model::{CheckedCardSelection, Grid, PlantId, PlantKind, Plantability};
impl NativePlantGridBackend for Pvz1051Backend {
    fn can_plant_at_native_grid(&self, selection: CheckedCardSelection, grid: Grid) -> Result<Plantability> {
        self.ensure_playing()?;
        if !grid.is_in_bounds(6, 9) {
            return Err(Pvz1051Error::OperationRejected("invalid physical grid"));
        }
        self.board()?;
        // SAFETY: native six-row array bounds, live Board, existing verified CanPlantAt ABI.
        Ok(Plantability::from_game_rule_code(unsafe {
            crate::raw::abi::board_can_plant_at(selection.effective_kind().code(), grid.row, grid.col)
        }))
    }
    fn new_plant_at_native_grid<'a>(
        &'a self, selection: CheckedCardSelection, grid: Grid,
    ) -> Result<Self::PlantHandle<'a>> {
        self.ensure_playing()?;
        if !grid.is_in_bounds(6, 9)
            || !rsvz_backend_api::pool_has_free_slot(self.plant_pool_active_count()?, self.plant_pool_capacity()?)
        {
            return Err(Pvz1051Error::OperationRejected(
                "physical grid or plant pool unavailable",
            ));
        }
        // SAFETY: live Board, six physical rows, free slot, validated kinds; existing NewPlant ABI.
        let p = unsafe {
            crate::raw::abi::board_new_plant(
                grid.row,
                grid.col,
                selection.packet_kind().code(),
                selection.imitator_target().map_or(-1, PlantKind::code),
            )
        };
        std::ptr::NonNull::new(p)
            .map(crate::impls::handles::PvzPlantHandle::new)
            .ok_or(Pvz1051Error::OperationRejected("NewPlant returned null"))
    }
}
impl CobSelectionBackend for Pvz1051Backend {
    fn select_cob(&self, id: PlantId) -> Result<()> {
        self.ensure_playing()?;
        let plant = self
            .plant(id)?
            .ok_or(Pvz1051Error::OperationRejected("cannon disappeared"))?;
        if self.plant_kind(plant)? != PlantKind::CobCannon || self.plant_state(plant) != 37 {
            return Err(Pvz1051Error::OperationRejected("cannon not ready"));
        }
        // SAFETY: live checked cannon ID and native cursor fields (MouseDownCobcannonFire +0x40/+0x30).
        unsafe {
            let cursor = ptrs::Board::cursor_object(self.board()?.as_ptr());
            if cursor.is_null() {
                return Err(Pvz1051Error::OperationRejected("no cursor"));
            }
            ptrs::CursorObject::select_cob(cursor, id.raw());
        }
        Ok(())
    }
}
