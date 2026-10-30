//! Local edit receipts, not game checkpoints. Restored plants are newly initialized
//! except for their selection, position, health and sleeping flag.
use super::*;
use rsvz_backend_api::{
    GridItemReadBackend, NativePlantPlacementBackend, PlantHealthWriteBackend, PlantRemoveBackend, PlantSleepBackend,
    WorldIdentityBackend,
};
use rsvz_model::{PlantId, PlantPlacementSelection, PositiveHp, WorldId};

struct RemovedPlant {
    selection: PlantPlacementSelection,
    grid: Grid,
    hp: PositiveHp,
    asleep: bool,
}

/// A single placement/removal receipt. Keep it only while editing the same field;
/// never replay it over subsequent native card/shovel operations.
pub struct PlantEdit {
    world: WorldId,
    scene: rsvz_model::SceneKind,
    created: Option<PlantId>,
    removed: Vec<RemovedPlant>,
    ladders: Vec<Grid>,
}

/// Replace a plant, or erase the top layer when `selection` is `None`, retaining
/// only affected objects for undo. An empty erase does not produce a new receipt.
pub fn edit_plant(selection: Option<PlantPlacementSelection>, grid: Grid) -> RuntimeResult<Option<PlantEdit>>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + SceneBackend
        + PlantPresenceBackend
        + PlantStateBackend
        + PlantPoolBackend
        + rsvz_backend_api::NativePlantPlacementBackend
        + PlantRemoveBackend
        + GridItemReadBackend
        + WorldIdentityBackend,
{
    ready()?;
    let world = crate::board::world_id()?.ok_or_else(|| RuntimeError::new("no edit world"))?;
    let (scene, removed, ladders) = crate::access::with_backend(|b| {
        let scene = b.scene().map_err(operation_error)?;
        let mut removed = Vec::new();
        let mut top_layer = None;
        for plant in b.present_plants().map_err(operation_error)? {
            let kind = b.plant_kind(plant).map_err(operation_error)?;
            let anchor = Grid {
                row: b.plant_row(plant),
                col: b.plant_col(plant),
            };
            let layer = plant_layer(kind.code() as u8);
            let affected = match selection {
                Some(selection) => {
                    anchor.row == grid.row
                        && layer == plant_layer(selection.kind())
                        && footprints_overlap(anchor.col, kind.code() as u8, grid.col, selection.kind())
                }
                None => rsvz_model::plant_occupies_grid(anchor, kind, grid) && top_layer.is_none_or(|top| layer >= top),
            };
            if !affected {
                continue;
            }
            if selection.is_none() {
                top_layer = Some(layer);
                removed.clear();
            }
            removed.push(RemovedPlant {
                selection: PlantPlacementSelection::new(kind.code() as u8, b.plant_imitater_kind(plant) == 48)
                    .map_err(RuntimeError::new)?,
                grid: anchor,
                hp: PositiveHp::new(b.plant_hp(plant)).map_err(|e| RuntimeError::new(e.to_string()))?,
                asleep: b.plant_is_sleeping(plant),
            });
        }
        let mut ladders = Vec::new();
        for item in b.grid_items().map_err(operation_error)? {
            let anchor = Grid {
                row: b.grid_item_row(item),
                col: b.grid_item_col(item),
            };
            if b.grid_item_kind(item).map_err(operation_error)? == GridItemKind::Ladder
                && (removed.iter().any(|plant| plant.grid == anchor) || selection.is_some() && anchor == grid)
            {
                ladders.push(anchor);
            }
        }
        Ok::<_, RuntimeError>((scene, removed, ladders))
    })?;
    let created = if let Some(selection) = selection {
        replace_plant(selection, grid)?;
        crate::access::with_backend(|b| {
            Ok::<_, RuntimeError>(b.present_plants().map_err(operation_error)?.find_map(|plant| {
                (b.plant_row(plant) == grid.row
                    && b.plant_col(plant) == grid.col
                    && b.plant_kind(plant)
                        .ok()
                        .is_some_and(|kind| kind.code() as u8 == selection.kind()))
                .then(|| b.plant_id(plant))
            }))
        })?
    } else {
        if removed.is_empty() {
            return Ok(None);
        }
        remove_at(grid)?;
        None
    };
    Ok(Some(PlantEdit {
        world,
        scene,
        created,
        removed,
        ladders,
    }))
}

impl PlantEdit {
    /// Editor initialization affects only the newly placed plant, never the rest of the field.
    pub fn prepare_created_plant(&self, wake_mushroom: bool) -> RuntimeResult<()>
    where
        CurrentBackend: WorldIdentityBackend
            + PlantReadBackend
            + PlantSleepBackend
            + rsvz_backend_api::PlantStateWriteBackend
            + rsvz_backend_api::PlantStateCountdownWriteBackend
            + rsvz_backend_api::PlantEffectCountdownWriteBackend
            + rsvz_backend_api::PlantIdleAnimationBackend,
    {
        if crate::board::world_id()? != Some(self.world) {
            return Err(RuntimeError::new("edit belongs to a different field"));
        }
        let Some(id) = self.created else {
            return Ok(());
        };
        crate::access::with_backend(|b| {
            let plant = b
                .plant(id)
                .map_err(operation_error)?
                .ok_or_else(|| RuntimeError::new("edited plant no longer exists"))?;
            if wake_mushroom && b.plant_is_sleeping(plant) {
                b.set_plant_sleeping(plant, false).map_err(operation_error)?;
            }
            if b.plant_kind(plant).map_err(operation_error)? == PlantKind::CobCannon {
                crate::lineup::apply::ready_cannon(b, plant)?;
            }
            Ok(())
        })
    }

    /// Recreates only the affected plants. Cooldowns/animations use native
    /// placement initialization; unrelated entities and game time are untouched.
    pub fn undo(self) -> RuntimeResult<()>
    where
        CurrentBackend: BoardReadinessBackend
            + GameUiBackend
            + SceneBackend
            + WorldIdentityBackend
            + PlantPresenceBackend
            + PlantPoolBackend
            + rsvz_backend_api::NativePlantPlacementBackend
            + PlantRemoveBackend
            + PlantHealthWriteBackend
            + PlantSleepBackend
            + GridItemCreateBackend
            + GridItemPoolBackend,
    {
        ready()?;
        if crate::board::world_id()? != Some(self.world) {
            return Err(RuntimeError::new("undo belongs to a different field"));
        }
        crate::access::with_backend(|b| {
            if b.scene().map_err(operation_error)? != self.scene {
                return Err(RuntimeError::new("undo scene changed"));
            }
            let created = self
                .created
                .map(|id| b.plant(id).map_err(operation_error))
                .transpose()?
                .flatten();
            if self.created.is_some() && created.is_none() {
                return Err(RuntimeError::new("edited plant no longer exists"));
            }
            for plant in b.present_plants().map_err(operation_error)? {
                if Some(b.plant_id(plant)) == self.created {
                    continue;
                }
                let kind = b.plant_kind(plant).map_err(operation_error)?.code() as u8;
                if self.removed.iter().any(|old| {
                    b.plant_row(plant) == old.grid.row
                        && plant_layer(kind) == plant_layer(old.selection.kind())
                        && footprints_overlap(b.plant_col(plant), kind, old.grid.col, old.selection.kind())
                }) {
                    return Err(RuntimeError::new("edited cells have changed"));
                }
            }
            let needed = self
                .removed
                .iter()
                .map(|p| 1 + usize::from(p.selection.imitater() >= 0))
                .sum();
            capacity(
                b.plant_pool_active_count().map_err(operation_error)?,
                b.plant_pool_capacity().map_err(operation_error)?,
                needed,
            )?;
            capacity(
                b.grid_item_pool_active_count().map_err(operation_error)?,
                b.grid_item_pool_capacity().map_err(operation_error)?,
                self.ladders.len(),
            )?;
            // Allocate before removing the new plant, just like normal replacement.
            for old in self.removed {
                let mut plant = b
                    .add_placement_plant(old.selection, old.grid)
                    .map_err(operation_error)?;
                if old.selection.imitater() >= 0 {
                    plant = b.morph_imitator(plant).map_err(operation_error)?;
                }
                b.set_plant_sleeping(plant, old.asleep).map_err(operation_error)?;
                b.set_plant_hp(plant, old.hp).map_err(operation_error)?;
            }
            if let Some(plant) = created {
                b.remove_plant(plant).map_err(operation_error)?;
            }
            for grid in self.ladders {
                let exists = b.grid_items().map_err(operation_error)?.any(|item| {
                    b.grid_item_kind(item).ok() == Some(GridItemKind::Ladder)
                        && b.grid_item_row(item) == grid.row
                        && b.grid_item_col(item) == grid.col
                });
                if !exists {
                    b.add_ladder(grid).map_err(operation_error)?;
                }
            }
            Ok(())
        })
    }
}
