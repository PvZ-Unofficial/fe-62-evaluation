use crate::impls::handles::{PvzMowerHandle, PvzMowerIter};
use crate::{
    error::{Pvz1051Error, Result},
    raw::{abi, layout as ptrs},
    runtime::Pvz1051Backend,
};
use rsvz_backend_api::{LawnMowerEditBackend, LawnMowerReadBackend, LawnMowerRestoreBackend};
use rsvz_model::LawnMowerId;

impl LawnMowerReadBackend for Pvz1051Backend {
    type MowerHandle<'a> = PvzMowerHandle<'a>;
    type MowerIter<'a> = PvzMowerIter<'a>;
    fn lawn_mowers(&self) -> Result<PvzMowerIter<'_>> {
        let board = self.board()?;
        // SAFETY: read only the current Board's embedded pool bounds.
        let max = unsafe { ptrs::DataArray::max_used_count(ptrs::Board::lawn_mowers(board.as_ptr())) };
        Ok(PvzMowerIter {
            backend: self,
            next: 0,
            max,
        })
    }
    fn lawn_mower_id<'a>(&'a self, mower: PvzMowerHandle<'a>) -> LawnMowerId {
        // SAFETY: borrow-bound occupied native slot retains its ID until recycling.
        LawnMowerId::from_raw(unsafe { ptrs::DataArray::item_id(mower.ptr.as_ptr()) })
    }
    fn lawn_mower_row<'a>(&'a self, mower: PvzMowerHandle<'a>) -> i32 {
        // SAFETY: occupied native mower under this backend borrow.
        unsafe { ptrs::LawnMower::row(mower.ptr.as_ptr()) }
    }
    fn lawn_mower_state<'a>(&'a self, mower: PvzMowerHandle<'a>) -> i32 {
        // SAFETY: occupied native mower under this backend borrow.
        unsafe { ptrs::LawnMower::state(mower.ptr.as_ptr()) }
    }
}
impl LawnMowerEditBackend for Pvz1051Backend {
    fn start_lawn_mower<'a>(&'a self, mower: PvzMowerHandle<'a>) -> Result<()> {
        // SAFETY: held handle cannot be recycled; StartMower uses ESI, no stack args.
        unsafe {
            if !ptrs::LawnMower::is_dead(mower.ptr.as_ptr()) {
                abi::lawn_mower_start(mower.ptr.as_ptr());
            }
        }
        Ok(())
    }
    fn kill_lawn_mower<'a>(&'a self, mower: PvzMowerHandle<'a>) -> Result<()> {
        let board = self.board()?;
        // SAFETY: current pool and bonus count are read before native Die may allocate.
        unsafe {
            if ptrs::LawnMower::is_dead(mower.ptr.as_ptr()) {
                return Ok(());
            }
            let pool = ptrs::Board::lawn_mowers(board.as_ptr());
            if *ptrs::Board::bonus_lawn_mowers_remaining_mut(board.as_ptr()) > 0
                && !ptrs::Board::level_award_dropped(board.as_ptr())
                && ptrs::DataArray::active_count(pool) >= ptrs::DataArray::max_size(pool)
            {
                return Err(Pvz1051Error::OperationRejected(
                    "bonus mower requires a free native slot",
                ));
            }
            abi::lawn_mower_die(mower.ptr.as_ptr());
        }
        Ok(())
    }
}
impl LawnMowerRestoreBackend for Pvz1051Backend {
    fn initialize_lawn_mowers_ready(&mut self) -> Result<()> {
        let board = self.board()?;
        // SAFETY: exclusive Board; initializer allocates at most six mowers and
        // consults the actual profile for pool-cleaner type.
        unsafe {
            let pool = ptrs::Board::lawn_mowers(board.as_ptr());
            let mode = ptrs::LawnApp::game_mode(self.app().as_ptr());
            let required = (0..6)
                .filter(|&row| (mode == 38 && row < 5) || *ptrs::Board::row_types(board.as_ptr()).add(row) != 0)
                .count() as u32;
            if ptrs::DataArray::max_size(pool).saturating_sub(ptrs::DataArray::active_count(pool)) < required {
                return Err(Pvz1051Error::OperationRejected("mower pool has insufficient capacity"));
            }
            if ptrs::LawnApp::player_info(self.app().as_ptr()).is_null() {
                return Err(Pvz1051Error::OperationRejected(
                    "mower initialization requires a player profile",
                ));
            }
        }
        crate::patches::mower::initialize(|| {
            // SAFETY: verified Board stack argument / ret4 ABI, pool/profile checked.
            unsafe { abi::board_init_lawn_mowers(board.as_ptr()) };
            Ok(())
        })
    }
}
