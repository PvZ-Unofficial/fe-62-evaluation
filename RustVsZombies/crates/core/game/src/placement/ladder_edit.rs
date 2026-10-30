//! One-cell ladder edits with identity-checked undo, independent of plant layers.
use super::*;
use rsvz_backend_api::{GridItemEditBackend, GridItemReadBackend, WorldIdentityBackend};
use rsvz_model::{GridItemId, SceneKind, WorldId};

pub struct LadderEdit {
    world: WorldId,
    scene: SceneKind,
    grid: Grid,
    removed: usize,
    created: Option<GridItemId>,
}

pub fn edit_ladder(grid: Grid, place: bool) -> RuntimeResult<Option<LadderEdit>>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + SceneBackend
        + WorldIdentityBackend
        + GridItemCreateBackend
        + GridItemPoolBackend,
{
    ready()?;
    let world = crate::board::world_id()?.ok_or_else(|| RuntimeError::new("no edit world"))?;
    crate::access::with_backend(|b| {
        let scene = b.scene().map_err(operation_error)?;
        if !FieldInfo::from_scene(scene).contains_grid(grid) {
            return Err(RuntimeError::new("outside the lawn"));
        }
        let matches = |item| {
            b.grid_item_kind(item).ok() == Some(GridItemKind::Ladder)
                && b.grid_item_row(item) == grid.row
                && b.grid_item_col(item) == grid.col
        };
        let count = b
            .grid_items()
            .map_err(operation_error)?
            .filter(|&item| matches(item))
            .count();
        if place == (count > 0) {
            return Ok(None);
        }
        let created = if place {
            capacity(
                b.grid_item_pool_active_count().map_err(operation_error)?,
                b.grid_item_pool_capacity().map_err(operation_error)?,
                1,
            )?;
            Some(b.grid_item_id(b.add_ladder(grid).map_err(operation_error)?))
        } else {
            for item in b.grid_items().map_err(operation_error)? {
                if matches(item) {
                    b.remove_grid_item(item).map_err(operation_error)?;
                }
            }
            None
        };
        Ok(Some(LadderEdit {
            world,
            scene,
            grid,
            removed: count,
            created,
        }))
    })
}

impl LadderEdit {
    pub fn undo(self) -> RuntimeResult<()>
    where
        CurrentBackend: BoardReadinessBackend
            + GameUiBackend
            + SceneBackend
            + WorldIdentityBackend
            + GridItemCreateBackend
            + GridItemPoolBackend,
    {
        ready()?;
        if crate::board::world_id()? != Some(self.world) {
            return Err(RuntimeError::new("edit belongs to a different field"));
        }
        crate::access::with_backend(|b| {
            if b.scene().map_err(operation_error)? != self.scene {
                return Err(RuntimeError::new("edit scene changed"));
            }
            let mut created = None;
            for item in b.grid_items().map_err(operation_error)? {
                if b.grid_item_kind(item).map_err(operation_error)? == GridItemKind::Ladder
                    && b.grid_item_row(item) == self.grid.row
                    && b.grid_item_col(item) == self.grid.col
                {
                    if Some(b.grid_item_id(item)) != self.created {
                        return Err(RuntimeError::new("edited ladders have changed"));
                    }
                    created = Some(item);
                }
            }
            if self.created.is_some() && created.is_none() {
                return Err(RuntimeError::new("edited ladder no longer exists"));
            }
            capacity(
                b.grid_item_pool_active_count().map_err(operation_error)?,
                b.grid_item_pool_capacity().map_err(operation_error)?,
                self.removed,
            )?;
            if let Some(item) = created {
                b.remove_grid_item(item).map_err(operation_error)?;
            }
            for _ in 0..self.removed {
                b.add_ladder(self.grid).map_err(operation_error)?;
            }
            Ok(())
        })
    }
}
