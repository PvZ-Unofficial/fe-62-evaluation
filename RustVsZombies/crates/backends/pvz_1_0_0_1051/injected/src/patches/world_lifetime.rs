//! Observe the native Board assignment/clear stores, including same-address reuse.
use super::memory::{encode_rel32, read_bytes, restore_owned_bytes, write_bytes};
use crate::error::{Pvz1051Error, Result};
use std::{cell::Cell, num::NonZeroU64};

const ASSIGN: usize = 0x407cae;
const CLEAR: usize = 0x44f3ec;
const ASSIGN_BYTES: [u8; 6] = [0x89, 0xae, 0x68, 0x07, 0, 0];
const CLEAR_BYTES: [u8; 10] = [0xc7, 0x87, 0x68, 0x07, 0, 0, 0, 0, 0, 0];
std::thread_local! {
    static GENERATION: Cell<Option<NonZeroU64>> = const { Cell::new(NonZeroU64::new(1)) };
}

pub(crate) fn generation() -> Result<u64> {
    if let Some(host) = crate::module_link::services() {
        let value = (host.world_generation)();
        return if value == 0 {
            Err(Pvz1051Error::InvariantViolated("world identity exhausted"))
        } else {
            Ok(value)
        };
    }
    GENERATION
        .get()
        .map(NonZeroU64::get)
        .ok_or(Pvz1051Error::InvariantViolated("world identity exhausted"))
}
pub(crate) fn note_mutation() {
    if let Some(host) = crate::module_link::services() {
        (host.world_mutated)();
        return;
    }
    GENERATION.set(next_generation(GENERATION.get()));
}
fn next_generation(previous: Option<NonZeroU64>) -> Option<NonZeroU64> {
    previous
        .and_then(|value| value.get().checked_add(1))
        .and_then(NonZeroU64::new)
}

pub(crate) struct WorldLifetimeHookGuard {
    assign: [u8; 6],
    clear: [u8; 10],
    installed: [bool; 2],
}
impl WorldLifetimeHookGuard {
    pub(crate) fn install() -> Result<Self> {
        if read_bytes(ASSIGN, ASSIGN_BYTES.len()) != ASSIGN_BYTES || read_bytes(CLEAR, CLEAR_BYTES.len()) != CLEAR_BYTES
        {
            return Err(Pvz1051Error::Patch(
                "Board lifetime stores are owned by unknown code".into(),
            ));
        }
        // Initialize constant TLS before exposing either native observation site.
        generation()?;
        let mut guard = Self {
            assign: [0x90; 6],
            clear: [0x90; 10],
            installed: [false; 2],
        };
        guard.assign[..5].copy_from_slice(&encode_rel32(
            0xe8,
            ASSIGN,
            board_assigned as *const () as usize,
            "Board assignment",
        )?);
        guard.clear[..5].copy_from_slice(&encode_rel32(
            0xe8,
            CLEAR,
            board_cleared as *const () as usize,
            "Board clear",
        )?);
        write_bytes(ASSIGN, &guard.assign)?;
        guard.installed[0] = true;
        if let Err(error) = write_bytes(CLEAR, &guard.clear) {
            return super::memory::finish_restoration(Err(error), guard.restore(), "Board lifetime install");
        }
        guard.installed[1] = true;
        Ok(guard)
    }
    pub(crate) fn restore(&mut self) -> Result<()> {
        let mut error = None;
        for (index, addr, original, installed) in [
            (0, ASSIGN, ASSIGN_BYTES.as_slice(), self.assign.as_slice()),
            (1, CLEAR, CLEAR_BYTES.as_slice(), self.clear.as_slice()),
        ] {
            if self.installed[index] {
                match restore_owned_bytes(addr, original, installed, "Board lifetime store") {
                    Ok(()) => self.installed[index] = false,
                    Err(failure) => {
                        error.get_or_insert(failure);
                    }
                }
            }
        }
        error.map_or(Ok(()), Err)
    }
}
impl Drop for WorldLifetimeHookGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

extern "C" fn record_change() {
    // No gameplay dispatch, allocation, pointers or I/O in this native observer.
    if std::panic::catch_unwind(note_mutation).is_err() {
        crate::runtime::hook::fail_and_request_unload();
    }
}

macro_rules! lifetime_shim {
    ($name:ident, $store:literal) => {
        /// Replays the verified store and preserves every native register/flag.
        #[unsafe(naked)]
        unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                $store,
                "pushfd", "pushad", "mov eax, esp", "sub esp, 528", "and esp, -16",
                "mov [esp + 512], eax", "fxsave [esp]", "fninit",
                "mov dword ptr [esp + 516], 0x1f80", "ldmxcsr [esp + 516]", "cld",
                "call {record}", "fxrstor [esp]", "mov esp, [esp + 512]", "popad", "popfd", "ret",
                record = sym record_change,
            );
        }
    };
}
// Board::Board, ESI=LawnApp*, EBP=Board*. Return to the remaining NOP then malloc.
lifetime_shim!(board_assigned, "mov dword ptr [esi + 0x768], ebp");
// LawnApp::KillBoard, EDI=LawnApp*. Return through five NOPs to the cursor reset.
lifetime_shim!(board_cleared, "mov dword ptr [edi + 0x768], 0");

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_changes_on_each_lifetime_event_and_never_wraps() {
        let first = NonZeroU64::new(1);
        let removed = next_generation(first);
        let reused_address = next_generation(removed);
        assert_eq!(reused_address.unwrap().get(), 3);
        assert_eq!(next_generation(NonZeroU64::new(u64::MAX)), None);
        assert_eq!(next_generation(None), None);
    }
}
