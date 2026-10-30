use crate::{
    Pvz1051Backend,
    error::Result,
    raw::{abi, layout as ptrs},
};
use rsvz_backend_api::TransientClearBackend;
impl TransientClearBackend for Pvz1051Backend {
    fn clear_native_projectiles(&mut self) -> Result<()> {
        let board = self.board()?;
        // SAFETY: exclusive Board/pool lifetime. Die destroys attachments but
        // does not recycle the current slot; every occupied slot is included.
        unsafe {
            let pool = ptrs::Board::projectiles(board.as_ptr());
            for index in 0..ptrs::DataArray::max_used_count(pool) {
                if let Some(projectile) = ptrs::DataArray::occupied_item_at(pool, index) {
                    if !ptrs::Projectile::dead(projectile.as_ptr()) {
                        abi::scene_projectile_die(projectile.as_ptr());
                    }
                }
            }
        }
        Ok(())
    }
    fn clear_native_items(&mut self) -> Result<()> {
        let board = self.board()?;
        // SAFETY: exclusive current pool; the caller releases the held cursor
        // before clearing collectible objects referenced by that cursor.
        unsafe {
            let pool = ptrs::Board::coins(board.as_ptr());
            for index in 0..ptrs::DataArray::max_used_count(pool) {
                if let Some(coin) = ptrs::DataArray::occupied_coin_at(pool, index) {
                    if !ptrs::Coin::is_dead(coin.as_ptr()) {
                        abi::scene_coin_die(coin.as_ptr());
                    }
                }
            }
        }
        Ok(())
    }
}
