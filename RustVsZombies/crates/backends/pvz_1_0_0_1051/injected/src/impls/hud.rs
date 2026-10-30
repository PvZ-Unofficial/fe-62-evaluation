use crate::{
    Pvz1051Backend,
    error::{Pvz1051Error, Result},
    raw::layout,
};
use rsvz_backend_api::{GameHudBackend, HudField};
impl GameHudBackend for Pvz1051Backend {
    const HUD_IS_NOOP: bool = false;
    fn hud_field(&self, field: HudField) -> Result<i32> {
        let board = self.board()?;
        // SAFETY: current Board with verified field addresses; null menu rejected.
        unsafe {
            let ptr = layout::Board::hud_field_ptr(board.as_ptr(), field);
            if ptr.is_null() {
                return Err(Pvz1051Error::OperationRejected("menu button unavailable"));
            }
            Ok(if field == HudField::CoinBankFadeCountdown {
                std::ptr::read_unaligned(ptr.cast())
            } else {
                i32::from(std::ptr::read(ptr) != 0)
            })
        }
    }
    fn set_hud_field(&self, field: HudField, value: i32) -> Result<()> {
        if field != HudField::CoinBankFadeCountdown && !matches!(value, 0 | 1) {
            return Err(Pvz1051Error::OperationRejected("HUD flag must be boolean"));
        }
        let board = self.board()?;
        // SAFETY: changes only one scalar, no entity recycling or callback.
        unsafe {
            let ptr = layout::Board::hud_field_ptr(board.as_ptr(), field);
            if ptr.is_null() {
                return Err(Pvz1051Error::OperationRejected("menu button unavailable"));
            }
            if field == HudField::CoinBankFadeCountdown {
                std::ptr::write_unaligned(ptr.cast(), value);
            } else {
                std::ptr::write(ptr, value as u8);
            }
        }
        Ok(())
    }
}
