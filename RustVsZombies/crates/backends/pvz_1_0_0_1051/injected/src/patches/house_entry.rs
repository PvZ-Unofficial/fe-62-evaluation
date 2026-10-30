//! A-TAS loss interception at Board::ZombiesWon (1051 0x413400).
//! Verified entry bytes and ret 8 at 0x4138C9. The native shim only records a
//! scalar occurrence and prevents loss; source-owned pause runs at dispatch.
use super::memory::{encode_rel32, read_bytes, write_bytes};
use crate::error::{Pvz1051Error, Result};
static mut OCCURRED: u8 = 0;
std::thread_local! {static INSTALLED:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
const ORIGINAL: [u8; 6] = [0x55, 0x8b, 0xec, 0x83, 0xe4, 0xf8];
#[unsafe(naked)]
unsafe extern "C" fn intercept() {
    core::arch::naked_asm!("mov byte ptr [{hit}], 1", "ret 8",hit=sym OCCURRED);
}
pub(crate) fn enabled() -> bool {
    if let Some(host) = crate::module_link::services() {
        return (host.house_enabled)();
    }
    INSTALLED.get()
}
pub(crate) fn set(enabled: bool) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.house_set)(enabled));
    }
    if enabled == INSTALLED.get() {
        return Ok(());
    }
    let mut patch = [0x90; 6];
    patch[..5].copy_from_slice(&encode_rel32(
        0xe9,
        0x413400,
        intercept as *const () as usize,
        "house entry",
    )?);
    let (expected, replacement) = if enabled {
        (&ORIGINAL, &patch)
    } else {
        (&patch, &ORIGINAL)
    };
    if read_bytes(0x413400, 6) != expected {
        return Err(Pvz1051Error::OperationRejected("house-entry patch ownership changed"));
    }
    write_bytes(0x413400, replacement)?;
    INSTALLED.set(enabled);
    // SAFETY: 1051 host is single-threaded; callback only sets this byte.
    unsafe {
        OCCURRED = 0;
    }
    Ok(())
}
pub(crate) fn take() -> bool {
    if let Some(host) = crate::module_link::services() {
        return (host.house_take)();
    }
    // SAFETY: same game thread as the no-callback native shim.
    unsafe {
        let hit = OCCURRED != 0;
        OCCURRED = 0;
        hit
    }
}
