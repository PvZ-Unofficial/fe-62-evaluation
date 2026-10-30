//! Explicit, bounded delegation of the user's current cursor. This never changes
//! ordinary script card admission and never looks up a replacement card by kind.
use crate::{
    access::{operation_error, with_backend, with_exclusive_backend},
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::*;
use rsvz_current::CurrentBackend;
use rsvz_model::*;
use rsvz_schedule::module::{ModuleSlot, current_module};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cancellation {
    CursorChanged,
    SlotChanged,
    PositionChanged,
    WorldChanged,
    SceneChanged,
    SourceStopped,
    ScreenChanged,
    Completed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Waiting,
    Planted { plant: PlantId, container: Option<PlantId> },
    Removed(PlantId),
    SelectedCob(PlantId),
    Cancelled(Cancellation),
    Invalidated(Cancellation),
    Rejected(&'static str),
}
impl Outcome {
    pub const fn pending(self) -> bool {
        matches!(self, Self::Waiting)
    }
}

/// Backend requirement aliases contain no state or aggregate reads.
pub trait UserCursorBackend:
    CursorPositionBackend
    + CursorObjectPositionBackend
    + CursorGridBackend
    + CursorQueryBackend
    + CursorSeedSlotBackend
    + CursorSeedReadBackend
    + SeedBankReadBackend
    + SeedBankEditBackend
    + GameUiBackend
    + WorldIdentityBackend
    + SceneBackend
{
}
impl<T> UserCursorBackend for T where
    T: CursorPositionBackend
        + CursorObjectPositionBackend
        + CursorGridBackend
        + CursorQueryBackend
        + CursorSeedSlotBackend
        + CursorSeedReadBackend
        + SeedBankReadBackend
        + SeedBankEditBackend
        + GameUiBackend
        + WorldIdentityBackend
        + SceneBackend
{
}

/// Captured only by a native current-operation call, not deserializable from GUI.
/// Values and IDs are retained; native pointers/handles never survive a borrow.
pub struct CursorRequest {
    pub(crate) world: WorldId,
    scene: SceneKind,
    source: ModuleSlot,
    execution: Option<crate::modules::ExecutionId>,
    pub(crate) position: PixelPos,
    mouse: PixelPos,
    pub(crate) preview_row: i32,
    pub(crate) cursor_type: i32,
    pub(crate) card: Option<(SeedSlot, CheckedCardSelection)>,
    count: u8,
    done: bool,
}
impl CursorRequest {
    pub fn capture() -> RuntimeResult<Self>
    where
        CurrentBackend: UserCursorBackend,
    {
        with_backend(|b| {
            if b.game_ui().map_err(operation_error)? != GameUi::Playing {
                return Err(RuntimeError::new("cursor action requires battle"));
            }
            let world = b
                .world_identity()
                .map_err(operation_error)?
                .ok_or_else(|| RuntimeError::new("no current world"))?;
            let source = current_module();
            let slot = b.cursor_seed_slot().map_err(operation_error)?;
            let mut card = None;
            if let Some(slot) = slot {
                for seed in b.seeds().map_err(operation_error)? {
                    if b.seed_slot(seed) == slot {
                        let selection = b.seed_selection(seed).map_err(operation_error)?;
                        if selection.packet_kind().code() != b.cursor_seed_kind().map_err(operation_error)?
                            || selection.imitator_target().map_or(-1, PlantKind::code)
                                != b.cursor_imitater_kind().map_err(operation_error)?
                        {
                            return Err(RuntimeError::new("held card and bank slot disagree"));
                        }
                        card = Some((slot, selection));
                        break;
                    }
                }
                if card.is_none() {
                    return Err(RuntimeError::new("held card slot is missing"));
                }
            }
            Ok(Self {
                world,
                scene: b.scene().map_err(operation_error)?,
                source,
                execution: crate::modules::current_execution(source),
                position: b.cursor_object_position().map_err(operation_error)?,
                mouse: b.cursor_position().map_err(operation_error)?,
                preview_row: b.cursor_grid_row().map_err(operation_error)?,
                cursor_type: b.cursor_type().map_err(operation_error)?,
                card,
                count: b.seed_packet_count().map_err(operation_error)?,
                done: false,
            })
        })
    }
    pub fn world(&self) -> WorldId {
        self.world
    }
    pub fn cancel(&mut self) {
        self.done = true;
    }
    pub(crate) fn validate(&self) -> RuntimeResult<Option<Outcome>>
    where
        CurrentBackend: UserCursorBackend,
    {
        if self.done {
            return Ok(Some(Outcome::Cancelled(Cancellation::Completed)));
        }
        if current_module() != self.source
            || crate::modules::current_execution(self.source) != self.execution
            || self
                .execution
                .is_some_and(|id| crate::modules::state(id).ok() != Some(crate::modules::ExecutionState::Running))
        {
            return Ok(Some(Outcome::Invalidated(Cancellation::SourceStopped)));
        }
        with_backend(|b| {
            if b.world_identity().map_err(operation_error)? != Some(self.world) {
                return Ok(Some(Outcome::Invalidated(Cancellation::WorldChanged)));
            }
            if b.scene().map_err(operation_error)? != self.scene {
                return Ok(Some(Outcome::Invalidated(Cancellation::SceneChanged)));
            }
            if b.game_ui().map_err(operation_error)? != GameUi::Playing {
                return Ok(Some(Outcome::Invalidated(Cancellation::ScreenChanged)));
            }
            if b.cursor_type().map_err(operation_error)? != self.cursor_type
                || b.cursor_seed_slot().map_err(operation_error)? != self.card.map(|(s, _)| s)
            {
                return Ok(Some(Outcome::Cancelled(Cancellation::CursorChanged)));
            }
            if b.cursor_grid_row().map_err(operation_error)? != self.preview_row
                || b.cursor_position().map_err(operation_error)? != self.mouse
                || b.cursor_object_position().map_err(operation_error)? != self.position
            {
                return Ok(Some(Outcome::Cancelled(Cancellation::PositionChanged)));
            }
            if let Some((slot, selection)) = self.card {
                if b.seed_packet_count().map_err(operation_error)? != self.count
                    || b.cursor_seed_kind().map_err(operation_error)? != selection.packet_kind().code()
                    || b.cursor_imitater_kind().map_err(operation_error)?
                        != selection.imitator_target().map_or(-1, PlantKind::code)
                {
                    return Ok(Some(Outcome::Cancelled(Cancellation::SlotChanged)));
                }
                let mut found = false;
                for seed in b.seeds().map_err(operation_error)? {
                    if b.seed_slot(seed) == slot {
                        found = b.seed_selection(seed).map_err(operation_error)? == selection;
                        break;
                    }
                }
                if !found {
                    return Ok(Some(Outcome::Cancelled(Cancellation::SlotChanged)));
                }
            }
            Ok(None)
        })
    }
    /// Consumes delegation just once. Recheck the world after exclusive input,
    /// which is permitted to replace Board; callers then reborrow fixed slots.
    pub(crate) fn release(&mut self) -> RuntimeResult<()>
    where
        CurrentBackend: InputBackend + WorldIdentityBackend + GameUiBackend + SceneBackend,
    {
        self.done = true;
        with_exclusive_backend(|b| b.release_mouse()).map_err(operation_error)?;
        with_backend(|b| {
            if b.world_identity().map_err(operation_error)? != Some(self.world)
                || b.scene().map_err(operation_error)? != self.scene
                || b.game_ui().map_err(operation_error)? != GameUi::Playing
            {
                return Err(RuntimeError::new("world changed during cursor release"));
            }
            Ok(())
        })
    }
}
