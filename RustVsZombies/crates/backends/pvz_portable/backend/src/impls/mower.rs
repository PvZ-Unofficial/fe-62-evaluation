use crate::handles::{PortableMowerHandle, PortableMowerIter};
use crate::{PortableBackend, Result};
use rsvz_backend_api::{LawnMowerEditBackend, LawnMowerReadBackend, LawnMowerRestoreBackend};
use rsvz_model::LawnMowerId;

impl LawnMowerReadBackend for PortableBackend {
    type MowerHandle<'a> = PortableMowerHandle<'a>;
    type MowerIter<'a> = PortableMowerIter<'a>;
    fn lawn_mowers(&self) -> Result<Self::MowerIter<'_>> {
        Ok(PortableMowerIter::new(
            self,
            self.world()?.lawn_mowers().max_used_count(),
        ))
    }
    fn lawn_mower_id<'a>(&'a self, mower: Self::MowerHandle<'a>) -> LawnMowerId {
        // SAFETY: occupied slot held under the current backend borrow.
        LawnMowerId::from_raw(unsafe { pvzp_rs::raw::pvzp_rs_mower_id(mower.as_ptr()) })
    }
    fn lawn_mower_row<'a>(&'a self, mower: Self::MowerHandle<'a>) -> i32 {
        read_raw!(mower, mRow)
    }
    fn lawn_mower_state<'a>(&'a self, mower: Self::MowerHandle<'a>) -> i32 {
        read_raw!(mower, mMowerState)
    }
}
impl LawnMowerEditBackend for PortableBackend {
    fn start_lawn_mower<'a>(&'a self, mower: Self::MowerHandle<'a>) -> Result<()> {
        mower.borrowed().start().map_err(Into::into)
    }
    fn kill_lawn_mower<'a>(&'a self, mower: Self::MowerHandle<'a>) -> Result<()> {
        mower.borrowed().die().map_err(Into::into)
    }
}
impl LawnMowerRestoreBackend for PortableBackend {
    fn initialize_lawn_mowers_ready(&mut self) -> Result<()> {
        // SAFETY: exclusive backend receiver; no mower borrow can survive allocation.
        unsafe { self.world()?.initialize_mowers_ready() }.map_err(Into::into)
    }
}
