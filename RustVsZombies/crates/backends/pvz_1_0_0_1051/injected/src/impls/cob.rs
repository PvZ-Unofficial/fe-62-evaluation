use rsvz_backend_api::backend::CobFireBackend;
use rsvz_model::model::PixelPos;

use crate::error::{Pvz1051Error, Result};
use crate::ops::plant::effective_seed_type as plant_effective_seed_type;

use crate::raw::layout as ptrs;
use crate::runtime::Pvz1051Backend;

use rsvz_model::model::PlantKind;

impl CobFireBackend for Pvz1051Backend {
    fn fire_cob<'a>(&'a self, cob: Self::PlantHandle<'a>, target: PixelPos) -> Result<()> {
        let board = self.board()?;
        // SAFETY: current Board and borrow-bound plant. Preserve the user's selected cannon;
        // the native mouse target handler does not recheck readiness after selection.
        unsafe {
            let cursor = ptrs::Board::cursor_object(board.as_ptr());
            if !cursor.is_null()
                && ptrs::CursorObject::cursor_type(cursor) == 8
                && ptrs::CursorObject::cob_cannon_id(cursor)
                    == rsvz_backend_api::PlantReadBackend::plant_id(self, cob).raw()
            {
                return Err(Pvz1051Error::OperationRejected("cob cannon is selected by user"));
            }
        }
        let plant = cob.ptr;
        if plant_effective_seed_type(plant)? != PlantKind::CobCannon {
            return Err(Pvz1051Error::KindUnavailable("plant is not CobCannon"));
        }
        // SAFETY: `plant` is current; CobCannon state 37 is the native ready state.
        if unsafe { ptrs::Plant::state(plant.as_ptr()) } != 37 {
            return Err(Pvz1051Error::AbiPreconditionFailed("cob cannon is not ready"));
        }
        // SAFETY: the borrowed handle is live, was checked as a ready CobCannon, and target
        // coordinates are copied scalar arguments to the objdump-verified native action.
        unsafe { crate::raw::abi::plant_cob_cannon_fire(plant.as_ptr(), target.x, target.y) };
        Ok(())
    }
}
