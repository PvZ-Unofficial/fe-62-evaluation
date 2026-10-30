use crate::{
    error::{Pvz1051Error, Result},
    patches::leases::{self, BoolPatchId},
    raw::layout as ptrs,
    runtime::Pvz1051Backend,
};
use rsvz_backend_api::SeedBankEditBackend;
use rsvz_model::{SeedPacketContent, SeedSlot};

impl Pvz1051Backend {
    fn packet_for_edit(&self, slot: SeedSlot) -> Result<*mut ptrs::SeedPacket> {
        if slot.index() >= 10 {
            return Err(Pvz1051Error::InvalidSeedSlot);
        }
        let bank = self.seed_bank()?;
        // SAFETY: the native bank allocates ten packets independently of the
        // currently displayed count; the index has been checked above.
        Ok(unsafe { ptrs::SeedBank::packet(bank.as_ptr(), slot.index()) })
    }
}
impl SeedBankEditBackend for Pvz1051Backend {
    fn seed_packet_count(&self) -> Result<u8> {
        // SAFETY: current non-null bank and verified mNumPackets offset.
        let count = unsafe { ptrs::SeedBank::packet_count(self.seed_bank()?.as_ptr()) };
        if !(0..=10).contains(&count) {
            return Err(Pvz1051Error::InvariantViolated("invalid seed packet count"));
        }
        Ok(count as u8)
    }
    fn set_seed_packet_count(&self, count: u8) -> Result<()> {
        if !(1..=10).contains(&count) {
            return Err(Pvz1051Error::OperationRejected("seed count must be in 1..=10"));
        }
        let bank = self.seed_bank()?;
        if BoolPatchId::SeedCountReadDisabled.enabled()? {
            return Err(Pvz1051Error::OperationRejected("seed count override is already active"));
        }
        leases::set_bool_patch(BoolPatchId::SeedCountReadDisabled, true)?;
        // SAFETY: verified ESI/EAX ABI, valid bank/count; no callbacks or Rust
        // unwind occur in UpdateWidth. The lease retains cleanup if restore fails.
        unsafe { crate::raw::abi::seed_bank_update_width_with_count(bank.as_ptr(), i32::from(count)) };
        crate::patches::memory::finish_restoration(
            Ok(()),
            leases::set_bool_patch(BoolPatchId::SeedCountReadDisabled, false),
            "seed-bank count override",
        )
    }
    fn seed_packet_kind(&self, slot: SeedSlot) -> Result<i32> {
        // SAFETY: bounded current bank slot.
        Ok(unsafe { ptrs::SeedPacket::packet_type(self.packet_for_edit(slot)?) })
    }
    fn seed_packet_imitater_kind(&self, slot: SeedSlot) -> Result<i32> {
        // SAFETY: bounded current bank slot.
        Ok(unsafe { ptrs::SeedPacket::imitater_type(self.packet_for_edit(slot)?) })
    }
    fn seed_packet_visible(&self, slot: SeedSlot) -> Result<bool> {
        // SAFETY: bounded current bank slot.
        Ok(unsafe { ptrs::SeedPacket::visible(self.packet_for_edit(slot)?) })
    }
    fn set_seed_packet_content(&self, slot: SeedSlot, content: SeedPacketContent) -> Result<()> {
        let packet = self.packet_for_edit(slot)?;
        // SAFETY: typed content excludes unsafe packet kinds; scalar writes
        // preserve cooldown and every other native packet field, as in PT.
        unsafe {
            ptrs::SeedPacket::set_packet_type(packet, content.packet());
            ptrs::SeedPacket::set_imitater_type(packet, content.imitater());
        }
        Ok(())
    }
    fn set_seed_packet_visible(&self, slot: SeedSlot, visible: bool) -> Result<()> {
        // SAFETY: bounded current packet; visibility does not change pool size.
        unsafe { ptrs::SeedPacket::set_visible(self.packet_for_edit(slot)?, visible) };
        Ok(())
    }
}
