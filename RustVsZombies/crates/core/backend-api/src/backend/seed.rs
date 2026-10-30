//! Seed capabilities. Entity operations use current borrowed handles.

use crate::backend::Backend;
use rsvz_model::model::{CheckedCardSelection, SeedSlot};

/// Whole-bank visibility, independent of individual packets and other game UI.
pub trait SeedBankVisibilityBackend: Backend {
    const SEED_BANK_VISIBILITY_IS_NOOP: bool = false;
    fn seed_bank_visible(&self) -> Result<bool, Self::Error>;
    fn set_seed_bank_visible(&self, visible: bool) -> Result<(), Self::Error>;
}

/// Native packet storage and layout edits. Slots address the ten allocated
/// packets, including inactive slots beyond the current displayed count.
pub trait SeedBankEditBackend: Backend {
    fn seed_packet_count(&self) -> Result<u8, Self::Error>;
    /// Exact native width/position recomputation, with count restricted to 1..=10.
    fn set_seed_packet_count(&self, count: u8) -> Result<(), Self::Error>;
    fn seed_packet_kind(&self, slot: SeedSlot) -> Result<i32, Self::Error>;
    fn seed_packet_imitater_kind(&self, slot: SeedSlot) -> Result<i32, Self::Error>;
    fn seed_packet_visible(&self, slot: SeedSlot) -> Result<bool, Self::Error>;
    fn set_seed_packet_content(
        &self, slot: SeedSlot, content: rsvz_model::model::SeedPacketContent,
    ) -> Result<(), Self::Error>;
    fn set_seed_packet_visible(&self, slot: SeedSlot, visible: bool) -> Result<(), Self::Error>;
}

/// Base-definition queries (not current survival upgrade prices or packet cooldowns).
pub trait SeedDefinitionBackend: Backend {
    fn seed_base_cost(&self, selection: CheckedCardSelection) -> Result<i32, Self::Error>;
    fn seed_base_recharge(&self, selection: CheckedCardSelection) -> Result<i32, Self::Error>;
}

/// Live seed-bank read capability.
pub trait SeedBankReadBackend: Backend {
    /// Packet kind before Imitater interpretation; preserves native special cards.
    fn seed_kind_code<'a>(&'a self, seed: Self::SeedHandle<'a>) -> Result<i32, Self::Error>;

    /// Render rectangle including conveyor/slot-machine offset. None on a
    /// headless backend with no packet geometry.
    fn seed_canvas_rect<'a>(&'a self, seed: Self::SeedHandle<'a>) -> Option<rsvz_model::Rect>;

    /// Seed handle valid for the current read lifetime.
    type SeedHandle<'a>: Copy
    where
        Self: 'a;
    /// Live seed handle iterator.
    type SeedIter<'a>: Iterator<Item = Self::SeedHandle<'a>>
    where
        Self: 'a;

    /// Current seed-bank packets, retaining their native slot indices. The 1051/Portable
    /// conveyor views skip empty display slots; their ordinary banks retain all slots.
    /// Headless backends retain their existing card-model enumeration.
    fn seeds(&self) -> Result<Self::SeedIter<'_>, Self::Error>;

    /// Resolves one native slot with exactly the same membership rules as `seeds()`.
    fn seed(&self, slot: SeedSlot) -> Result<Option<Self::SeedHandle<'_>>, Self::Error>;

    /// Reads seed slot from a validated handle.
    fn seed_slot<'a>(&'a self, handle: Self::SeedHandle<'a>) -> SeedSlot;
    /// Reads seed selection from a validated handle.
    fn seed_selection<'a>(&'a self, handle: Self::SeedHandle<'a>) -> Result<CheckedCardSelection, Self::Error>;
    /// Reads whether a validated seed slot is currently usable.
    fn seed_is_usable<'a>(&'a self, handle: Self::SeedHandle<'a>) -> bool;
}

/// Current card selection in bank order, whether it still lives in the chooser or in a live seed bank.
pub trait CardSelectionReadBackend: Backend {
    fn selected_card_count(&self) -> Result<usize, Self::Error>;

    fn selected_card(&self, index: usize) -> Result<CheckedCardSelection, Self::Error>;
}

/// Native chooser actions and facts, independent of automatic-selection policy.
/// Headless backends omit this interactive capability.
pub trait SeedChooserControlBackend: CardSelectionReadBackend {
    fn chooser_readiness(&self) -> Result<crate::opening::SeedChooserReadiness, Self::Error>;
    fn chooser_card_available(&self, selection: CheckedCardSelection) -> Result<bool, Self::Error>;
    fn chooser_card_locked(&self, index: usize) -> Result<bool, Self::Error>;
    fn deselect_card(&self, index: usize) -> Result<(), Self::Error>;
    fn stop_seed_chooser_fast_forward(&self) -> Result<(), Self::Error>;
}

/// Native cooldown remaining on one borrowed live seed packet.
pub trait SeedCooldownReadBackend: SeedBankReadBackend {
    fn seed_cooldown_remaining<'a>(&'a self, seed: Self::SeedHandle<'a>) -> i32;
}

/// Preserved chooser cooldown state used outside a live seed bank.
pub trait ChooserCooldownReadBackend: Backend {
    fn chooser_cooldown_remaining(&self, _selection: CheckedCardSelection) -> Result<Option<i32>, Self::Error> {
        Ok(None)
    }
}

/// Append one card to the current card selection.
pub trait CardAppendSelectionBackend: Backend {
    fn select_card(&self, selection: CheckedCardSelection) -> Result<(), Self::Error>;

    /// Commits a staged replacement bank. Native choosers commit each click already.
    fn finish_card_selection(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Native seed-packet actions over a borrowed seed-bank slot.
pub trait SeedPacketBackend: SeedBankReadBackend {
    /// Native Board::HasConveyorBeltSeedBank; consumption shifts following slots.
    fn has_conveyor_belt_seed_bank(&self) -> Result<bool, Self::Error>;

    /// A packet held by the user must remain unavailable, even with planting cheats.
    fn seed_can_pick_up<'a>(&'a self, seed: Self::SeedHandle<'a>) -> Result<bool, Self::Error>;

    /// Commits the complete seed-packet state transition after a successful planting.
    ///
    /// Backends whose native cursor path deactivates the packet before planting must include
    /// that transition here.
    fn seed_was_planted<'a>(&'a self, seed: Self::SeedHandle<'a>) -> Result<(), Self::Error>;
}
