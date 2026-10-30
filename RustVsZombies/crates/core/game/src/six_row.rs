//! A-TAS PlantShovelFireForbiddenGrid: native sixth-row tools and water-base
//! planting. This is a single delegated action, never a scene layout toggle.
use crate::{
    access::{operation_error, with_backend},
    runtime::{RuntimeError, RuntimeResult},
    user_card::{CursorRequest, Outcome, UserCursorBackend},
};
use rsvz_backend_api::*;
use rsvz_current::CurrentBackend;
use rsvz_model::*;

pub struct SixRow {
    cursor: CursorRequest,
    grid: Grid,
    started: bool,
}
impl SixRow {
    pub fn capture() -> RuntimeResult<Self>
    where
        CurrentBackend: UserCursorBackend + SceneBackend,
    {
        Self::from_cursor(CursorRequest::capture()?)
    }
    pub fn from_cursor(cursor: CursorRequest) -> RuntimeResult<Self>
    where
        CurrentBackend: SceneBackend,
    {
        let scene = with_backend(|b| b.scene()).map_err(operation_error)?;
        let grid =
            grid_at(scene, cursor.position).ok_or_else(|| RuntimeError::new("cursor is outside the six-row grid"))?;
        Ok(Self {
            cursor,
            grid,
            started: false,
        })
    }
    pub fn started(&self) -> bool {
        self.started
    }
    pub fn cancel(&mut self) {
        self.cursor.cancel();
    }
    pub fn advance(&mut self) -> RuntimeResult<Outcome>
    where
        CurrentBackend: UserCursorBackend
            + SceneBackend
            + InputBackend
            + NativePlantGridBackend
            + CobSelectionBackend
            + PlantRemoveBackend
            + SeedPacketBackend
            + PlantCostBackend
            + SunQueryBackend
            + SunWriteBackend
            + PlantPoolBackend,
    {
        if let Some(outcome) = self.cursor.validate()? {
            self.cursor.cancel();
            return Ok(outcome);
        }
        let layers = plants(self.grid)?;
        let water = layers.lily.is_some() || layers.normal.is_some_and(|(_, kind)| kind == PlantKind::Cattail);
        if !water || self.cursor.card.is_none() {
            if self.grid.row != 5 {
                self.cursor.cancel();
                return Ok(Outcome::Rejected("action requires row six or a lily/cattail base"));
            }
            if self.cursor.cursor_type == 6 {
                self.cursor.cancel();
                let plant = layers
                    .normal
                    .map(|p| p.0)
                    .or(layers.pumpkin)
                    .or(layers.lily)
                    .or(layers.pot);
                let Some(id) = plant else {
                    return Ok(Outcome::Rejected("nothing to shovel"));
                };
                with_backend(|b| -> RuntimeResult<()> {
                    if let Some(p) = b.plant(id).map_err(operation_error)? {
                        b.remove_plant(p).map_err(operation_error)?;
                    }
                    Ok(())
                })?;
                return Ok(Outcome::Removed(id));
            }
            // Original action selects a ready cannon before trying the held card.
            for col in [self.grid.col, self.grid.col - 1] {
                if col < 0 {
                    continue;
                }
                if let Some((id, PlantKind::CobCannon)) = plants(Grid { row: 5, col })?.normal {
                    if crate::logic::cob::cob_recover_time(id) == 0 {
                        self.cursor.release()?;
                        with_backend(|b| b.select_cob(id)).map_err(operation_error)?;
                        return Ok(Outcome::SelectedCob(id));
                    }
                }
            }
        }
        let Some((slot, selection)) = self.cursor.card else {
            self.cursor.cancel();
            return Ok(Outcome::Rejected("hold a card or shovel"));
        };
        let kind = selection.effective_kind();
        if !water {
            let allowed =
                with_backend(|b| b.can_plant_at_native_grid(selection, self.grid)).map_err(operation_error)?;
            if !matches!(allowed, Plantability::Allowed) {
                self.cursor.cancel();
                return Ok(Outcome::Rejected("native planting rules reject this grid"));
            }
        }
        let mut replacements = [None; 2];
        if let Some(base) = upgrade_base(kind).filter(|_| selection.imitator_target().is_none()) {
            let candidate = if base == PlantKind::LilyPad {
                layers.lily
            } else {
                layers.normal.filter(|p| p.1 == base).map(|p| p.0)
            };
            let Some(id) = candidate else {
                self.cursor.cancel();
                return Ok(Outcome::Rejected("upgrade base is missing"));
            };
            replacements[0] = Some(id);
            if kind == PlantKind::CobCannon {
                if self.grid.col >= 8 {
                    self.cursor.cancel();
                    return Ok(Outcome::Rejected("cannon needs two columns"));
                }
                let right = plants(Grid {
                    row: self.grid.row,
                    col: self.grid.col + 1,
                })?
                .normal
                .filter(|p| p.1 == PlantKind::KernelPult)
                .map(|p| p.0);
                if right.is_none() {
                    self.cursor.cancel();
                    return Ok(Outcome::Rejected("second kernel is missing"));
                }
                replacements[1] = right;
            }
        } else {
            if water && kind != PlantKind::Pumpkin && layers.normal.is_some() {
                self.cursor.cancel();
                return Ok(Outcome::Rejected("water-base grid is occupied"));
            }
            replacements[0] = if kind == PlantKind::Pumpkin {
                layers.pumpkin
            } else if matches!(kind, PlantKind::WallNut | PlantKind::TallNut) {
                layers.normal.filter(|p| p.1 == kind).map(|p| p.0)
            } else {
                None
            };
        }
        // Validate all destructive prerequisites while still holding the card.
        let cost = with_backend(|b| -> RuntimeResult<_> {
            if !pool_has_free_slot(
                b.plant_pool_active_count().map_err(operation_error)?,
                b.plant_pool_capacity().map_err(operation_error)?,
            ) {
                return Err(RuntimeError::new("plant pool is full"));
            }
            let cost = b.current_plant_cost(selection).map_err(operation_error)?;
            if b.sun().map_err(operation_error)? < cost.get() as u32 {
                return Err(RuntimeError::new("not enough sun"));
            }
            Ok(cost)
        })?;
        self.cursor.release()?;
        self.started = true;
        let plant = with_backend(|b| -> RuntimeResult<PlantId> {
            let mut packet = None;
            for seed in b.seeds().map_err(operation_error)? {
                if b.seed_slot(seed) == slot && b.seed_selection(seed).map_err(operation_error)? == selection {
                    packet = Some(seed);
                    break;
                }
            }
            let packet = packet.ok_or_else(|| RuntimeError::new("delegated card slot changed"))?;
            if !b.seed_can_pick_up(packet).map_err(operation_error)? {
                return Err(RuntimeError::new("delegated card is unavailable"));
            }
            // A-TAS ClickSunPlantCd deducts the price explicitly, even with
            // free-planting rules. Reject unsigned underflow before any edit.
            let remaining = b
                .sun()
                .map_err(operation_error)?
                .checked_sub(cost.get() as u32)
                .ok_or_else(|| RuntimeError::new("not enough sun"))?;
            b.set_sun(remaining).map_err(operation_error)?;
            for id in replacements.into_iter().flatten() {
                if let Some(plant) = b.plant(id).map_err(operation_error)? {
                    b.remove_plant(plant).map_err(operation_error)?;
                }
            }
            let plant = b
                .new_plant_at_native_grid(selection, self.grid)
                .map_err(operation_error)?;
            let id = b.plant_id(plant);
            b.seed_was_planted(packet).map_err(operation_error)?;
            Ok(id)
        })?;
        Ok(Outcome::Planted { plant, container: None })
    }
}
#[derive(Default)]
struct Layers {
    normal: Option<(PlantId, PlantKind)>,
    pumpkin: Option<PlantId>,
    lily: Option<PlantId>,
    pot: Option<PlantId>,
}
fn plants(grid: Grid) -> RuntimeResult<Layers>
where
    CurrentBackend: PlantReadBackend,
{
    with_backend(|b| {
        let mut found = Layers::default();
        for plant in b.plants().map_err(operation_error)? {
            if b.plant_row(plant) != grid.row
                || b.plant_col(plant) != grid.col
                || b.plant_is_squished(plant)
                || b.plant_on_bungee_state(plant) == 2
            {
                continue;
            }
            let kind = b.plant_kind(plant).map_err(operation_error)?;
            if kind == PlantKind::Squash && b.plant_state(plant) >= 5 {
                continue;
            }
            let id = b.plant_id(plant);
            match kind {
                PlantKind::CoffeeBean => {}
                PlantKind::LilyPad => {
                    found.lily.get_or_insert(id);
                }
                PlantKind::FlowerPot => {
                    found.pot.get_or_insert(id);
                }
                PlantKind::Pumpkin => {
                    found.pumpkin.get_or_insert(id);
                }
                _ => {
                    found.normal.get_or_insert((id, kind));
                }
            }
        }
        Ok(found)
    })
}
fn upgrade_base(kind: PlantKind) -> Option<PlantKind> {
    Some(match kind {
        PlantKind::GatlingPea => PlantKind::Repeater,
        PlantKind::TwinSunflower => PlantKind::Sunflower,
        PlantKind::GloomShroom => PlantKind::FumeShroom,
        PlantKind::Cattail => PlantKind::LilyPad,
        PlantKind::WinterMelon => PlantKind::MelonPult,
        PlantKind::GoldMagnet => PlantKind::MagnetShroom,
        PlantKind::Spikerock => PlantKind::Spikeweed,
        PlantKind::CobCannon => PlantKind::KernelPult,
        _ => return None,
    })
}
fn grid_at(scene: SceneKind, pos: PixelPos) -> Option<Grid> {
    if pos.x < 0 || pos.y < 0 {
        return None;
    }
    let col = (i64::from(pos.x) + 65) / 80;
    let row = if scene.has_roof() {
        (i64::from(pos.y) + 40 - (5 - col).max(0) * 20) / 85
    } else if scene.has_pool() {
        (i64::from(pos.y) + 40) / 85
    } else {
        (i64::from(pos.y) + 55) / 100
    };
    let grid = Grid {
        row: (row - 1) as i32,
        col: (col - 1) as i32,
    };
    grid.is_in_bounds(6, 9).then_some(grid)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_sixth_row_coordinates_and_upgrades() {
        assert_eq!(
            grid_at(SceneKind::Day, PixelPos { x: 55, y: 545 }),
            Some(Grid { row: 5, col: 0 })
        );
        assert_eq!(
            grid_at(SceneKind::Roof, PixelPos { x: 55, y: 550 }),
            Some(Grid { row: 5, col: 0 })
        );
        assert!(grid_at(SceneKind::Day, PixelPos { x: -100, y: 545 }).is_none());
        assert_eq!(upgrade_base(PlantKind::CobCannon), Some(PlantKind::KernelPult));
    }
}
