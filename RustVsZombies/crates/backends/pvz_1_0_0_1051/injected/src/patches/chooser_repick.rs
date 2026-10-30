//! Bounds protection for native survival-repick cooldown import.
//! Does not select, replace, or fill any card; valid inputs retain the native path.
use super::memory::{encode_rel32, read_bytes, restore_owned_bytes, write_bytes};
use crate::error::{Pvz1051Error, Result};

const SITE: usize = 0x483e60;
const ORIGINAL: [u8; 6] = [0x8b, 0x85, 0x14, 0x0d, 0, 0];
pub(crate) struct RepickGuard {
    patch: [u8; 6],
}
impl RepickGuard {
    pub(crate) fn install() -> Result<Self> {
        if read_bytes(SITE, ORIGINAL.len()) != ORIGINAL {
            return Err(Pvz1051Error::Patch(
                "native repick cooldown loop is owned by unknown code".into(),
            ));
        }
        let mut patch = [0x90; 6];
        patch[..5].copy_from_slice(&encode_rel32(
            0xe9,
            SITE,
            bounded_repick as *const () as usize,
            "repick bounds",
        )?);
        write_bytes(SITE, &patch)?;
        Ok(Self { patch })
    }
    pub(crate) fn restore(&mut self) -> Result<()> {
        // Leaving an invalid active survival bank behind would re-enable the
        // original out-of-bounds write at the next repick. Keep the guarded host
        // mapped instead of choosing/replacing the user's cards during unload.
        use crate::raw::layout as ptrs;
        unsafe {
            let app = ptrs::lawn_app();
            if !app.is_null() && (1..=15).contains(&ptrs::LawnApp::game_mode(app)) {
                let board = ptrs::LawnApp::board(app);
                if !board.is_null() {
                    let bank = ptrs::Board::seed_bank(board);
                    if !bank.is_null() {
                        let count = ptrs::SeedBank::packet_count(bank);
                        if !(0..=10).contains(&count)
                            || (0..count as usize).any(|index| {
                                !(0..=48).contains(&ptrs::SeedPacket::packet_type(ptrs::SeedBank::packet(bank, index)))
                            })
                        {
                            return Err(Pvz1051Error::OperationRejected(
                                "invalid active survival cards still require repick protection; host remains mapped",
                            ));
                        }
                    }
                }
            }
        }
        restore_owned_bytes(SITE, &ORIGINAL, &self.patch, "repick bounds")
    }
}
impl Drop for RepickGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

// Original constructor 483E60..483E8D: EBP=chooser, ESI=packet index,
// EDI=packet byte offset. Native packets stride 50h, max 10; mChosenSeeds
// has 49 entries stride 3Ch. Invalid cards skip only cooldown bookkeeping.
#[unsafe(naked)]
unsafe extern "C" fn bounded_repick() {
    core::arch::naked_asm!(
        "cmp esi, 10",
        "jae 3f",
        "mov eax, [ebp + 0xd14]",
        "mov ecx, [eax + 0x144]",
        "lea eax, [ecx + edi + 0x28]",
        "mov ecx, [eax + 0x34]",
        "cmp ecx, 48",
        "ja 2f",
        "mov edx, ecx",
        "shl edx, 4",
        "sub edx, ecx",
        "lea ecx, [ebp + edx * 4 + 0xa4]",
        "mov dl, [eax + 0x49]",
        "mov [ecx + 0x2c], dl",
        "mov eax, [eax + 0x24]",
        "mov [ecx + 0x30], eax",
        "2:",
        "push 0x483e8d",
        "ret",
        "3:",
        "push 0x483ea3",
        "ret",
    );
}
