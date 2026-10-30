//! Frozen previews for real plant-pool relocation. A preview owns only its input
//! identities and destination assumptions, never native pointers or plant state.
use crate::{
    access::{operation_error, with_backend, with_exclusive_backend},
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::{PlantPoolBackend, PlantSlotEditBackend, WorldIdentityBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{PlantId, WorldId};

#[derive(Clone, Copy, Debug)]
pub struct SlotMove {
    pub original: PlantId,
    pub current: PlantId,
    pub target: u32,
    expected: Option<PlantId>,
}

pub struct RelocationPlan {
    world: WorldId,
    moves: Vec<SlotMove>,
    high: bool,
    swap: bool,
    started: bool,
}

impl RelocationPlan {
    pub fn world(&self) -> WorldId {
        self.world
    }

    /// Includes the displaced plant of an explicitly requested swap. After an
    /// attempted commit, `current` also reports any completed partial changes.
    pub fn moves(&self) -> &[SlotMove] {
        &self.moves
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn changed_count(&self) -> usize {
        self.moves
            .iter()
            .filter(|entry| entry.original != entry.current)
            .count()
    }

    /// Check before requesting script cleanup, and again at the commit boundary.
    pub fn validate(&self) -> RuntimeResult<()>
    where
        CurrentBackend: PlantSlotEditBackend + WorldIdentityBackend,
    {
        if self.started {
            return Err(RuntimeError::new("plant-slot preview has already been applied"));
        }
        with_backend(|backend| {
            if backend.world_identity().map_err(operation_error)? != Some(self.world) {
                return Err(RuntimeError::new("plant-slot preview belongs to an old world"));
            }
            for entry in &self.moves {
                backend
                    .validate_plant_slot_change(entry.original, entry.target, entry.expected)
                    .map_err(operation_error)?;
            }
            Ok(())
        })
    }

    /// Execute only at the native outer safe boundary, after the ordinary script
    /// has stopped and released its resources. No update occurs between moves.
    pub fn apply(&mut self) -> RuntimeResult<()>
    where
        CurrentBackend: PlantSlotEditBackend + WorldIdentityBackend,
    {
        self.validate()?;
        with_exclusive_backend(|backend| {
            self.started = true;
            // Packing low-to-high (or high-to-low) vacates selected destinations
            // before reaching them. It never displaces an unselected plant.
            for offset in 0..self.moves.len() {
                let index = if self.high {
                    self.moves.len() - offset - 1
                } else {
                    offset
                };
                let entry = self.moves[index];
                if u32::from(entry.current.index()) == entry.target {
                    continue;
                }
                let expected = if self.swap { entry.expected } else { None };
                let change = backend
                    .relocate_plant_slot(entry.current, entry.target, expected)
                    .map_err(operation_error)?;
                self.moves[index].current = change.moved.current;
                if let Some(displaced) = change.swapped {
                    // Only the two-entry explicit swap can reach this branch.
                    self.moves[1].current = displaced.current;
                    break;
                }
            }
            Ok(())
        })
    }
}

/// Freeze the exact occupant shown by the caller. `None` means move into an
/// empty slot, not permission to swap with whoever happens to occupy it later.
pub fn prepare_move(source: PlantId, target: u32, expected_target: Option<PlantId>) -> RuntimeResult<RelocationPlan>
where
    CurrentBackend: PlantSlotEditBackend + WorldIdentityBackend,
{
    with_backend(|backend| {
        backend
            .validate_plant_slot_change(source, target, expected_target)
            .map_err(operation_error)?;
        let world = backend
            .world_identity()
            .map_err(operation_error)?
            .ok_or_else(|| RuntimeError::new("plant-slot operation requires a world"))?;
        let mut moves = Vec::with_capacity(if expected_target.is_some_and(|id| id != source) {
            2
        } else {
            1
        });
        moves.push(SlotMove {
            original: source,
            current: source,
            target,
            expected: expected_target,
        });
        if let Some(occupant) = expected_target.filter(|&id| id != source) {
            moves.push(SlotMove {
                original: occupant,
                current: occupant,
                target: u32::from(source.index()),
                expected: Some(source),
            });
        }
        Ok(RelocationPlan {
            world,
            moves,
            high: false,
            swap: expected_target.is_some(),
            started: false,
        })
    })
}

/// Pack selected plants in their current relative order. Occupied unselected
/// slots are excluded; the backend's actual capacity determines the range.
pub fn prepare_pack(selected: &[PlantId], high: bool) -> RuntimeResult<RelocationPlan>
where
    CurrentBackend: PlantSlotEditBackend + WorldIdentityBackend,
{
    if selected.is_empty() {
        return Err(RuntimeError::new("select at least one plant"));
    }
    with_backend(|backend| {
        let capacity = backend.plant_pool_capacity().map_err(operation_error)?;
        if selected.len() > capacity as usize {
            return Err(RuntimeError::new("plant selection exceeds pool capacity"));
        }
        let world = backend
            .world_identity()
            .map_err(operation_error)?
            .ok_or_else(|| RuntimeError::new("plant-slot operation requires a world"))?;
        let mut moves: Vec<_> = selected
            .iter()
            .map(|&id| SlotMove {
                original: id,
                current: id,
                target: u32::from(id.index()),
                expected: Some(id),
            })
            .collect();
        moves.sort_unstable_by_key(|entry| entry.original.index());
        if moves
            .windows(2)
            .any(|pair| pair[0].original.index() == pair[1].original.index())
        {
            return Err(RuntimeError::new("plant selection contains duplicate slots"));
        }
        // Validate all sources even if the first target is already invalid.
        for entry in &moves {
            backend
                .validate_plant_slot_change(entry.original, entry.target, entry.expected)
                .map_err(operation_error)?;
        }
        let mut assigned = 0;
        for offset in 0..capacity {
            let target = if high { capacity - offset - 1 } else { offset };
            let expected = backend.plant_slot_id(target).map_err(operation_error)?;
            let available = expected.is_none_or(|id| {
                moves
                    .binary_search_by_key(&id.index(), |entry| entry.original.index())
                    .is_ok_and(|index| moves[index].original == id)
            });
            if !available {
                continue;
            }
            let index = if high { moves.len() - assigned - 1 } else { assigned };
            moves[index].target = target;
            moves[index].expected = expected;
            assigned += 1;
            if assigned == moves.len() {
                break;
            }
        }
        if assigned != moves.len() {
            return Err(RuntimeError::new("not enough valid plant slots"));
        }
        let plan = RelocationPlan {
            world,
            moves,
            high,
            swap: false,
            started: false,
        };
        // Validation also rejects a retiring target or damaged native allocator.
        for entry in &plan.moves {
            backend
                .validate_plant_slot_change(entry.original, entry.target, entry.expected)
                .map_err(operation_error)?;
        }
        Ok(plan)
    })
}
