use super::memory::{read_bytes, restore_owned_bytes, write_bytes};
use crate::error::{Pvz1051Error, Result};

const TABLES: [usize; 3] = [0x667d0c, 0x671578, 0x676968];
const DRAW: [u8; 4] = 0x54c650_u32.to_le_bytes();
const PRESENT_SITE: usize = 0x54c8cd;
// These are the same three App vtables at +0xC8. Original EXE contains
// 0x5509A0 (a single RET); InitDDInterface calls them at 0x5521FD, ECX=App.
const PRE_RESET_TABLES: [usize; 3] = [0x667c68, 0x6714d4, 0x6768c4];
const PRE_RESET: [u8; 4] = 0x5509a0_u32.to_le_bytes();

/// Exact AvZ draw/present boundary. Original vtables and actual callers are in
/// paint-vtable-proof.txt / paint-draw-abi-proof.txt (0x551C8B and 0x551F48).
pub(crate) struct PaintHook {
    replacement: [u8; 4],
    reset_replacement: [u8; 4],
    tables: [bool; 3],
    reset_tables: [bool; 3],
    deferred_present: bool,
}
impl PaintHook {
    pub(crate) fn new(target: usize, reset: usize) -> Self {
        Self {
            replacement: (target as u32).to_le_bytes(),
            reset_replacement: (reset as u32).to_le_bytes(),
            tables: [false; 3],
            reset_tables: [false; 3],
            deferred_present: false,
        }
    }
    pub(crate) fn installed(&self) -> bool {
        self.deferred_present && self.tables.iter().chain(&self.reset_tables).all(|value| *value)
    }
    pub(crate) fn install(&mut self) -> Result<()> {
        for address in TABLES {
            if read_bytes(address, 4) != DRAW {
                return Err(Pvz1051Error::Patch(format!(
                    "native paint vtable changed at {address:#x}"
                )));
            }
        }
        for address in PRE_RESET_TABLES {
            if read_bytes(address, 4) != PRE_RESET {
                return Err(Pvz1051Error::Patch(format!(
                    "native pre-reset vtable changed at {address:#x}"
                )));
            }
        }
        if read_bytes(PRESENT_SITE, 2) != [0xff, 0xd0] {
            return Err(Pvz1051Error::Patch("native present call is already modified".into()));
        }
        // The original call takes one stack argument. NOP/POP EAX removes it;
        // the wrapper presents exactly once after the overlay.
        write_bytes(PRESENT_SITE, &[0x90, 0x58])?;
        self.deferred_present = true;
        for (index, address) in TABLES.into_iter().enumerate() {
            write_bytes(PRE_RESET_TABLES[index], &self.reset_replacement)?;
            self.reset_tables[index] = true;
            write_bytes(address, &self.replacement)?;
            self.tables[index] = true;
        }
        Ok(())
    }
    pub(crate) fn restore(&mut self) -> Result<()> {
        let mut error = None;
        for (index, address) in TABLES.into_iter().enumerate().rev() {
            if self.reset_tables[index] {
                match restore_owned_bytes(
                    PRE_RESET_TABLES[index],
                    &PRE_RESET,
                    &self.reset_replacement,
                    "native pre-reset vtable",
                ) {
                    Ok(()) => self.reset_tables[index] = false,
                    Err(failure) => {
                        error.get_or_insert(failure);
                    }
                }
            }
            if self.tables[index] {
                match restore_owned_bytes(address, &DRAW, &self.replacement, "native paint vtable") {
                    Ok(()) => self.tables[index] = false,
                    Err(failure) => {
                        error.get_or_insert(failure);
                    }
                }
            }
        }
        if self.deferred_present {
            match restore_owned_bytes(PRESENT_SITE, &[0xff, 0xd0], &[0x90, 0x58], "native present call") {
                Ok(()) => self.deferred_present = false,
                Err(failure) => {
                    error.get_or_insert(failure);
                }
            }
        }
        error.map_or(Ok(()), Err)
    }
}
