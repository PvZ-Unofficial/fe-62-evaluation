//! Immediate native seed-bank edits shared by scripts and external tools.
use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::{BoardReadinessBackend, GameUiBackend, SeedBankEditBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{GameUi, SeedPacketContent, SeedSlot};

fn ensure_ready() -> RuntimeResult<()>
where
    CurrentBackend: BoardReadinessBackend + GameUiBackend,
{
    crate::access::with_backend(|backend| {
        let ready = match backend.game_ui().map_err(crate::access::operation_error)? {
            GameUi::LevelIntro => backend.level_intro_board_ready(),
            GameUi::Playing => backend.playing_board_ready(),
            _ => return Err(RuntimeError::new("seed editing requires a chooser or battle")),
        }
        .map_err(crate::access::operation_error)?;
        if ready {
            Ok(())
        } else {
            Err(RuntimeError::new("seed bank is not ready"))
        }
    })
}
pub fn set_count(count: u8) -> RuntimeResult<()>
where
    CurrentBackend: SeedBankEditBackend + BoardReadinessBackend + GameUiBackend,
{
    if !(1..=10).contains(&count) {
        return Err(RuntimeError::new("seed count must be in 1..=10"));
    }
    ensure_ready()?;
    crate::access::with_backend(|backend| backend.set_seed_packet_count(count)).map_err(crate::access::operation_error)
}
pub fn set_packet(slot: SeedSlot, content: SeedPacketContent, visible: bool) -> RuntimeResult<()>
where
    CurrentBackend: SeedBankEditBackend + BoardReadinessBackend + GameUiBackend,
{
    if slot.index() >= 10 {
        return Err(RuntimeError::new("invalid seed packet slot"));
    }
    ensure_ready()?;
    crate::access::with_backend(|backend| {
        backend
            .set_seed_packet_content(slot, content)
            .map_err(crate::access::operation_error)?;
        backend
            .set_seed_packet_visible(slot, visible)
            .map_err(crate::access::operation_error)
    })
}
