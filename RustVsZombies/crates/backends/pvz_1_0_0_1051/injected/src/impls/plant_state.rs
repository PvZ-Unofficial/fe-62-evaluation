use rsvz_backend_api::backend::{PlantEffectCountdownWriteBackend, PlantVisualStateBackend};
use rsvz_model::model::{NonNegativeI32, ObjectEditOutcome};

use crate::error::Result;
use crate::raw::layout as ptrs;
use crate::runtime::Pvz1051Backend;

impl PlantEffectCountdownWriteBackend for Pvz1051Backend {
    fn set_plant_effect_countdown<'a>(
        &'a self, handle: Self::PlantHandle<'a>, target_countdown: NonNegativeI32,
    ) -> Result<ObjectEditOutcome> {
        // SAFETY: handle was constructed from a live current-frame plant DataArray slot; this writes
        // only Plant::mDoSpecialCountdown.
        unsafe { ptrs::Plant::set_do_special_countdown(handle.ptr.as_ptr(), target_countdown.get()) };
        Ok(ObjectEditOutcome::Applied)
    }
}

impl PlantVisualStateBackend for Pvz1051Backend {
    fn set_plant_eating_flash_counter<'a>(&'a self, plant: Self::PlantHandle<'a>, value: i32) -> Result<()> {
        // SAFETY: the borrowed handle came from a verified live plant path and this writes only
        // mEatenFlashCountdown @ 0xb8.
        unsafe { ptrs::Plant::set_eaten_flash_countdown(plant.ptr.as_ptr(), value) };
        Ok(())
    }

    fn update_plant_reanim_color<'a>(&'a self, plant: Self::PlantHandle<'a>) -> Result<()> {
        // SAFETY: Plant::UpdateReanimColor @ 0x4635c0 was verified as a stack Plant* wrapper in
        // the original executable and existing objdump notes. The handle is tied to the current backend
        // borrow, so the pointer cannot escape this call.
        unsafe { crate::raw::abi::plant_update_reanim_color(plant.ptr.as_ptr()) };
        Ok(())
    }
}
