use super::{card_parts, card_selection};
use crate::handles::{PortableSeedHandle, PortableSeedIter};
use crate::{PortableBackend, PortableBackendError, Result};
use rsvz_backend_api::SeedBankEditBackend;
use rsvz_backend_api::backend::{
    CardAppendSelectionBackend, CardSelectionReadBackend, ChooserCooldownReadBackend, SeedBankReadBackend,
    SeedChooserFastForwardBackend, SeedCooldownReadBackend, SeedPacketBackend, SeedRuleEditBackend,
};
use rsvz_model::SeedPacketContent;
use rsvz_model::model::{CheckedCardSelection, SeedChooserFastForwardOptions, SeedSlot};

impl rsvz_backend_api::SeedChooserControlBackend for PortableBackend {
    fn chooser_readiness(&self) -> Result<rsvz_backend_api::opening::SeedChooserReadiness> {
        self.seed_chooser_readiness()
    }
    fn chooser_card_available(&self, selection: CheckedCardSelection) -> Result<bool> {
        Ok(pvzp_rs::chooser_card_available(selection.packet_kind().code())?
            && match selection.imitator_target() {
                Some(kind) => pvzp_rs::chooser_card_available(kind.code())?,
                None => true,
            })
    }
    fn chooser_card_locked(&self, index: usize) -> Result<bool> {
        let index = u32::try_from(index).map_err(|_| PortableBackendError::NumericOutOfRange("selected card index"))?;
        pvzp_rs::chooser_card_locked(index).map_err(Into::into)
    }
    fn deselect_card(&self, index: usize) -> Result<()> {
        self.ensure_seed_chooser_ready_for_auto_selection()?;
        let index = u32::try_from(index).map_err(|_| PortableBackendError::NumericOutOfRange("selected card index"))?;
        pvzp_rs::deselect_card(index).map_err(Into::into)
    }
    fn stop_seed_chooser_fast_forward(&self) -> Result<()> {
        pvzp_rs::stop_seed_chooser_fast_forward().map_err(Into::into)
    }
}

impl rsvz_backend_api::SeedDefinitionBackend for PortableBackend {
    fn seed_base_cost(&self, selection: CheckedCardSelection) -> Result<i32> {
        Ok(pvzp_rs::seed_base_cost(selection.effective_kind() as i32))
    }

    fn seed_base_recharge(&self, selection: CheckedCardSelection) -> Result<i32> {
        Ok(pvzp_rs::seed_base_recharge(selection.effective_kind() as i32))
    }
}

impl SeedBankReadBackend for PortableBackend {
    fn seed_kind_code<'a>(&'a self, seed: Self::SeedHandle<'a>) -> Result<i32> {
        Ok(read_raw!(seed, mPacketType))
    }

    fn seed_canvas_rect<'a>(&'a self, seed: Self::SeedHandle<'a>) -> Option<rsvz_model::Rect> {
        Some(rsvz_model::Rect {
            x: read_raw!(seed, _base.mX) + read_raw!(seed, mOffsetX),
            y: read_raw!(seed, _base.mY),
            width: read_raw!(seed, _base.mWidth),
            height: read_raw!(seed, _base.mHeight),
        })
    }

    type SeedHandle<'a> = PortableSeedHandle<'a>;
    type SeedIter<'a> = PortableSeedIter<'a>;

    fn seeds(&self) -> Result<Self::SeedIter<'_>> {
        let world = self.world()?;
        let count = world.seed_count()?;
        Ok(PortableSeedIter::new(self, count))
    }

    fn seed(&self, slot: SeedSlot) -> Result<Option<Self::SeedHandle<'_>>> {
        let world = self.world()?;
        if slot.index() >= world.seed_count()? as usize {
            return Ok(None);
        }
        let seed = PortableSeedHandle::new(world.seed_at(slot.index() as u32)?.as_non_null());
        if read_raw!(seed, mPacketType) == -1 && world.has_conveyor()? {
            return Ok(None);
        }
        Ok(Some(seed))
    }

    fn seed_slot<'a>(&'a self, handle: Self::SeedHandle<'a>) -> SeedSlot {
        let raw = read_raw!(handle, mIndex).max(0) as usize;
        SeedSlot::new(raw).expect("Portable seed handles use native seed-bank indices")
    }

    fn seed_selection<'a>(&'a self, handle: Self::SeedHandle<'a>) -> Result<CheckedCardSelection> {
        card_selection(read_raw!(handle, mPacketType), read_raw!(handle, mImitaterType))
    }

    fn seed_is_usable<'a>(&'a self, handle: Self::SeedHandle<'a>) -> bool {
        SeedPacketBackend::seed_can_pick_up(self, handle).unwrap_or(false)
    }
}

impl SeedCooldownReadBackend for PortableBackend {
    fn seed_cooldown_remaining<'a>(&'a self, seed: Self::SeedHandle<'a>) -> i32 {
        if !read_raw!(seed, mRefreshing) {
            return 0;
        }
        read_raw!(seed, mRefreshTime)
            .saturating_sub(read_raw!(seed, mRefreshCounter))
            .saturating_add(1)
            .max(0)
    }
}

impl SeedPacketBackend for PortableBackend {
    fn has_conveyor_belt_seed_bank(&self) -> Result<bool> {
        self.world()?.has_conveyor().map_err(Into::into)
    }
    fn seed_can_pick_up<'a>(&'a self, seed: Self::SeedHandle<'a>) -> Result<bool> {
        seed.borrowed().can_pick_up().map_err(Into::into)
    }

    fn seed_was_planted<'a>(&'a self, seed: Self::SeedHandle<'a>) -> Result<()> {
        seed.borrowed().was_planted().map_err(Into::into)
    }
}

impl CardSelectionReadBackend for PortableBackend {
    fn selected_card_count(&self) -> Result<usize> {
        usize::try_from(pvzp_rs::selected_card_count()?)
            .map_err(|_| PortableBackendError::NumericOutOfRange("selected card count"))
    }

    fn selected_card(&self, index: usize) -> Result<CheckedCardSelection> {
        let index = u32::try_from(index).map_err(|_| PortableBackendError::NumericOutOfRange("selected card index"))?;
        let (packet, imitater) = pvzp_rs::selected_card(index)?;
        card_selection(packet, imitater)
    }
}

impl ChooserCooldownReadBackend for PortableBackend {}

impl CardAppendSelectionBackend for PortableBackend {
    fn select_card(&self, selection: CheckedCardSelection) -> Result<()> {
        self.ensure_seed_chooser_ready_for_auto_selection()?;
        let (packet, imitater) = card_parts(selection);
        pvzp_rs::select_card(packet, imitater).map_err(Into::into)
    }
}

impl PortableBackend {
    fn packet_for_edit(&self, slot: SeedSlot) -> Result<pvzp_rs::SeedRef<'_>> {
        if slot.index() >= rsvz_model::MAX_SEED_SLOTS {
            return Err(PortableBackendError::NumericOutOfRange("seed packet slot"));
        }
        Ok(self.world()?.seed_storage_at(slot.index() as u32)?)
    }
}
impl SeedBankEditBackend for PortableBackend {
    fn seed_packet_count(&self) -> Result<u8> {
        u8::try_from(self.world()?.seed_count()?).map_err(|_| PortableBackendError::NumericOutOfRange("seed count"))
    }
    fn set_seed_packet_count(&self, count: u8) -> Result<()> {
        Ok(self.world()?.set_seed_count(count)?)
    }
    fn seed_packet_kind(&self, slot: SeedSlot) -> Result<i32> {
        let packet = self.packet_for_edit(slot)?;
        // SAFETY: checked native storage slot under the current backend borrow.
        Ok(unsafe { std::ptr::addr_of!((*packet.as_ptr()).mPacketType).read() })
    }
    fn seed_packet_imitater_kind(&self, slot: SeedSlot) -> Result<i32> {
        let packet = self.packet_for_edit(slot)?;
        // SAFETY: checked native storage slot under the current backend borrow.
        Ok(unsafe { std::ptr::addr_of!((*packet.as_ptr()).mImitaterType).read() })
    }
    fn seed_packet_visible(&self, slot: SeedSlot) -> Result<bool> {
        let packet = self.packet_for_edit(slot)?;
        // SAFETY: checked native storage slot under the current backend borrow.
        Ok(unsafe { std::ptr::addr_of!((*packet.as_ptr())._base.mVisible).read() })
    }
    fn set_seed_packet_content(&self, slot: SeedSlot, content: SeedPacketContent) -> Result<()> {
        let packet = self.packet_for_edit(slot)?;
        // SAFETY: typed content and checked slot; scalar writes preserve native
        // refresh timers, activation and all other packet state.
        unsafe {
            std::ptr::addr_of_mut!((*packet.as_ptr()).mPacketType).write(content.packet());
            std::ptr::addr_of_mut!((*packet.as_ptr()).mImitaterType).write(content.imitater());
        }
        Ok(())
    }
    fn set_seed_packet_visible(&self, slot: SeedSlot, visible: bool) -> Result<()> {
        let packet = self.packet_for_edit(slot)?;
        // SAFETY: checked packet, independent visibility scalar.
        unsafe { std::ptr::addr_of_mut!((*packet.as_ptr())._base.mVisible).write(visible) };
        Ok(())
    }
}

impl SeedChooserFastForwardBackend for PortableBackend {
    fn request_seed_chooser_fast_forward(&self, options: SeedChooserFastForwardOptions) -> Result<()> {
        pvzp_rs::request_seed_chooser_fast_forward(options.max_frames).map_err(Into::into)
    }
}

impl SeedRuleEditBackend for PortableBackend {
    fn set_seed_recharge_ignored(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_seed_recharge_ignored(enabled).map_err(Into::into)
    }

    fn seed_recharge_ignored(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::seed_recharge_ignored())
    }
}
