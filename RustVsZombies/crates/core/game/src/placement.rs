//! One-shot placement helpers. No tool defaults, script-stop decisions or
//! persistent controllers are installed by these ordinary game operations.
use crate::{
    access::operation_error,
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::{
    BoardReadinessBackend, GameUiBackend, GridItemCreateBackend, GridItemPoolBackend, GridTerrainBackend,
    PlantCreateBackend, PlantPoolBackend, PlantPresenceBackend, PlantReadBackend, PlantStateBackend, SceneBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{CardSelection, FieldInfo, GameUi, Grid, GridItemKind, GridSquareKind, PlantKind};

mod edit;
mod ladder_edit;
pub use edit::{PlantEdit, edit_plant};
pub use ladder_edit::{LadderEdit, edit_ladder};

#[derive(Clone, Copy, Debug)]
pub enum BaseKind {
    WaterLily,
    RoofPot,
}

#[derive(Clone, Copy, Debug)]
pub enum ZombiePosition {
    /// None selects every field column.
    Grid(Option<u8>),
    /// Retain native initialization's position and forward the selected rows.
    /// A special initializer (notably bungee) may select its own target row.
    Natural,
}

/// Place one requested zombie per target (bobsled creates its native four-member
/// team). Boss uses native fixed row zero and ignores grid/natural coordinates.
pub fn place_zombies(kind: rsvz_model::ZombieKind, row: Option<u8>, position: ZombiePosition) -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + SceneBackend
        + rsvz_backend_api::ZombieCreateBackend
        + rsvz_backend_api::ZombiePoolBackend,
{
    use rsvz_backend_api::{ZombieCreateBackend, ZombiePoolBackend};
    use rsvz_model::ZombieKind;
    ready()?;
    crate::access::with_backend(|backend| {
        let field = FieldInfo::from_scene(backend.scene().map_err(operation_error)?);
        let boss = kind == ZombieKind::Boss;
        let targets = if boss {
            vec![Grid { row: 0, col: 0 }]
        } else {
            let col = match position {
                ZombiePosition::Grid(col) => col,
                ZombiePosition::Natural => Some(0),
            };
            plant_targets(field, 0, row, col)?
        };
        let needed = if kind == ZombieKind::Bobsled {
            targets.len() * 4
        } else {
            targets.len() + 1
        };
        capacity(
            backend.zombie_pool_active_count().map_err(operation_error)?,
            backend.zombie_pool_capacity().map_err(operation_error)?,
            needed,
        )?;
        for (completed, grid) in targets.iter().copied().enumerate() {
            let result = if boss || matches!(position, ZombiePosition::Natural) {
                // Same from-wave zero as the native IZombiePlaceZombie route;
                // the position choice changes only how placement is performed.
                backend
                    .add_zombie_in_row(kind, grid.row, 0)
                    .map_err(operation_error)
                    .and_then(|zombie| zombie.ok_or_else(|| RuntimeError::new("native zombie creation declined")))
            } else {
                backend.place_zombie(kind, grid).map_err(operation_error)
            };
            if let Err(error) = result {
                return Err(RuntimeError::new(format!(
                    "zombie placement failed after {completed} completed targets; current target may be partial: {error}"
                )));
            }
        }
        Ok(targets.len())
    })
}

fn ready() -> RuntimeResult<()>
where
    CurrentBackend: BoardReadinessBackend + GameUiBackend,
{
    crate::access::with_backend(|backend| {
        let ready = match backend.game_ui().map_err(operation_error)? {
            GameUi::LevelIntro => backend.level_intro_board_ready(),
            GameUi::Playing => backend.playing_board_ready(),
            _ => return Err(RuntimeError::new("placement requires a chooser or battle")),
        }
        .map_err(operation_error)?;
        if ready {
            Ok(())
        } else {
            Err(RuntimeError::new("placement board is not ready"))
        }
    })
}
fn capacity(used: u32, total: u32, needed: usize) -> RuntimeResult<()> {
    if used > total || needed > total.saturating_sub(used) as usize {
        Err(RuntimeError::new(
            "not enough native object slots for the complete placement",
        ))
    } else {
        Ok(())
    }
}

/// PT AddPlant placement over one immutable set of cells. All-column cannons
/// occupy columns 1/3/5/7; an explicitly selected column uses the normal bounds.
/// The caller chooses IZ styling; this operation does not stop any module.
pub fn place_plants(
    selection: rsvz_model::PlantPlacementSelection, row: Option<u8>, col: Option<u8>, iz_style: bool,
) -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + SceneBackend
        + PlantPoolBackend
        + rsvz_backend_api::NativePlantPlacementBackend
        + rsvz_backend_api::IZombiePlantStyleBackend,
{
    use rsvz_backend_api::{IZombiePlantStyleBackend, NativePlantPlacementBackend};
    ready()?;
    crate::access::with_backend(|backend| {
        let field = FieldInfo::from_scene(backend.scene().map_err(operation_error)?);
        let targets = plant_targets(field, selection.kind(), row, col)?;
        // Native ImitaterMorph allocates one successor while its placeholder
        // stays occupied until the ordinary delete queue. Do not reclaim other
        // scripts' pending dead entities merely to make room for this placement.
        let needed = targets.len() * if selection.imitater() >= 0 { 2 } else { 1 };
        capacity(
            backend.plant_pool_active_count().map_err(operation_error)?,
            backend.plant_pool_capacity().map_err(operation_error)?,
            needed,
        )?;
        for (completed, grid) in targets.iter().copied().enumerate() {
            let result = (|| {
                let mut plant = backend.add_placement_plant(selection, grid).map_err(operation_error)?;
                if selection.imitater() >= 0 {
                    plant = backend.morph_imitator(plant).map_err(operation_error)?;
                }
                if iz_style {
                    backend.setup_izombie_plant(plant).map_err(operation_error)?;
                }
                Ok::<_, RuntimeError>(())
            })();
            if let Err(error) = result {
                return Err(RuntimeError::new(format!(
                    "plant placement failed after {completed} completed cells; the current cell may be partial: {error}"
                )));
            }
        }
        Ok(targets.len())
    })
}

fn plant_targets(field: FieldInfo, kind: u8, row: Option<u8>, col: Option<u8>) -> RuntimeResult<Vec<Grid>> {
    if row.is_some_and(|row| usize::from(row) >= field.row_count)
        || col.is_some_and(|col| usize::from(col) >= field.col_count)
    {
        return Err(RuntimeError::new("placement coordinate is outside the current field"));
    }
    let rows = row.map_or(0..field.row_count, |row| usize::from(row)..usize::from(row) + 1);
    let cannon_all = kind == 47 && col.is_none();
    let cols = col.map_or(0..if cannon_all { 8 } else { field.col_count }, |col| {
        usize::from(col)..usize::from(col) + 1
    });
    let mut targets = Vec::new();
    for row in rows {
        for col in cols.clone().step_by(if cannon_all { 2 } else { 1 }) {
            targets.push(Grid {
                row: row as i32,
                col: col as i32,
            });
        }
    }
    Ok(targets)
}

#[cfg(test)]
mod plant_placement_tests {
    use super::*;
    #[test]
    fn all_cannons_have_pt_spacing_and_explicit_cells_keep_safe_field_bounds() {
        let pool = FieldInfo::from_scene(rsvz_model::SceneKind::Pool);
        let targets = plant_targets(pool, 47, None, None).unwrap();
        assert_eq!(targets.len(), 24);
        assert_eq!(
            &targets[..4],
            &[
                Grid { row: 0, col: 0 },
                Grid { row: 0, col: 2 },
                Grid { row: 0, col: 4 },
                Grid { row: 0, col: 6 }
            ]
        );
        assert_eq!(
            plant_targets(pool, 47, Some(1), Some(8)).unwrap(),
            [Grid { row: 1, col: 8 }]
        );
        assert_eq!(plant_targets(pool, 49, None, None).unwrap().len(), 54);
        assert!(plant_targets(FieldInfo::from_scene(rsvz_model::SceneKind::Day), 0, Some(5), None).is_err());
    }
}

/// Fill only completely unoccupied cells in the first N columns. Any present,
/// unsquished plant blocks the cell, not just an existing lily pad or flower pot.
pub fn fill_empty_bases(kind: BaseKind, columns: u8) -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + SceneBackend
        + PlantPresenceBackend
        + PlantPoolBackend
        + PlantCreateBackend
        + GridTerrainBackend,
{
    if !(1..=9).contains(&columns) {
        return Err(RuntimeError::new("column count must be in 1..=9"));
    }
    ready()?;
    crate::access::with_backend(|backend| {
        let scene = backend.scene().map_err(operation_error)?;
        if matches!(kind, BaseKind::RoofPot) && !scene.has_roof() {
            return Ok(0);
        }
        let field = FieldInfo::from_scene(scene);
        let mut occupied = [[false; 9]; 6];
        for plant in backend.present_plants().map_err(operation_error)? {
            let grid = Grid {
                row: backend.plant_row(plant),
                col: backend.plant_col(plant),
            };
            if !backend.plant_is_squished(plant) && field.contains_grid(grid) {
                occupied[grid.row as usize][grid.col as usize] = true;
            }
        }
        let mut targets = [[false; 9]; 6];
        let mut needed = 0;
        for row in 0..field.row_count {
            for col in 0..usize::from(columns) {
                if occupied[row][col] {
                    continue;
                }
                let grid = Grid {
                    row: row as i32,
                    col: col as i32,
                };
                if matches!(kind, BaseKind::RoofPot)
                    || backend.grid_square_kind(grid).map_err(operation_error)? == GridSquareKind::Pool
                {
                    targets[row][col] = true;
                    needed += 1;
                }
            }
        }
        capacity(
            backend.plant_pool_active_count().map_err(operation_error)?,
            backend.plant_pool_capacity().map_err(operation_error)?,
            needed,
        )?;
        let selection = CardSelection::Plant(match kind {
            BaseKind::WaterLily => PlantKind::LilyPad,
            BaseKind::RoofPot => PlantKind::FlowerPot,
        })
        .checked()
        .expect("fixed base plant kind");
        for (row, columns) in targets.iter().enumerate() {
            for (col, &place) in columns.iter().enumerate() {
                if place {
                    backend
                        .add_plant(
                            selection,
                            Grid {
                                row: row as i32,
                                col: col as i32,
                            },
                        )
                        .map_err(operation_error)?;
                }
            }
        }
        Ok(needed)
    })
}

fn pumpkin_grid<'a>(
    backend: &'a CurrentBackend, plant: <CurrentBackend as PlantReadBackend>::PlantHandle<'a>, field: FieldInfo,
    imitater_only: bool,
) -> RuntimeResult<Option<Grid>>
where
    CurrentBackend: PlantStateBackend + GridTerrainBackend,
{
    if backend.plant_is_squished(plant) || backend.plant_kind(plant).map_err(operation_error)? != PlantKind::Pumpkin {
        return Ok(None);
    }
    // Both native backends and PE preserve 48 on the transformed successor.
    // A still-transforming imitater has raw kind 48 and was excluded above.
    if imitater_only && backend.plant_imitater_kind(plant) != PlantKind::Imitator.code() {
        return Ok(None);
    }
    let grid = Grid {
        row: backend.plant_row(plant),
        col: backend.plant_col(plant),
    };
    if grid.col == 0 || !field.contains_grid(grid) {
        return Ok(None);
    }
    Ok((backend.grid_square_kind(grid).map_err(operation_error)? == GridSquareKind::Grass).then_some(grid))
}

/// Read-only preflight for callers that coordinate an exclusive world edit.
pub fn validate_pumpkin_ladders(imitater_only: bool) -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + SceneBackend
        + PlantPresenceBackend
        + PlantStateBackend
        + GridTerrainBackend
        + GridItemPoolBackend,
{
    ready()?;
    crate::access::with_backend(|backend| {
        let field = FieldInfo::from_scene(backend.scene().map_err(operation_error)?);
        let mut needed = 0;
        for plant in backend.present_plants().map_err(operation_error)? {
            needed += usize::from(pumpkin_grid(backend, plant, field, imitater_only)?.is_some());
        }
        // Die marks old ladders but does not immediately reclaim their slots.
        capacity(
            backend.grid_item_pool_active_count().map_err(operation_error)?,
            backend.grid_item_pool_capacity().map_err(operation_error)?,
            needed,
        )?;
        Ok(needed)
    })
}

/// Clear all old ladders, then add one for each qualifying pumpkin. PT does not
/// mark its position mask after insertion, so stacked pumpkins remain separate.
pub fn ladder_pumpkins(imitater_only: bool) -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + SceneBackend
        + PlantPresenceBackend
        + PlantStateBackend
        + GridTerrainBackend
        + GridItemPoolBackend
        + GridItemCreateBackend,
{
    validate_pumpkin_ladders(imitater_only)?;
    crate::modifier::cleanup::clear_grid_items(GridItemKind::Ladder)?;
    crate::access::with_backend(|backend| {
        let field = FieldInfo::from_scene(backend.scene().map_err(operation_error)?);
        let mut placed = 0;
        for plant in backend.present_plants().map_err(operation_error)? {
            if let Some(grid) = pumpkin_grid(backend, plant, field, imitater_only)? {
                backend.add_ladder(grid).map_err(operation_error)?;
                placed += 1;
            }
        }
        Ok(placed)
    })
}

/// `None` selects all legal rows/columns; `Some` uses zero-based coordinates.
pub fn place_ladders(row: Option<u8>, col: Option<u8>) -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend + GameUiBackend + SceneBackend + GridItemPoolBackend + GridItemCreateBackend,
{
    let (rows, cols) = grid_item_area(row, col)?;
    let needed = rows.len() * cols.len();
    crate::access::with_backend(|backend| {
        for row in rows {
            for col in cols.clone() {
                backend
                    .add_ladder(Grid {
                        row: row as i32,
                        col: col as i32,
                    })
                    .map_err(operation_error)?;
            }
        }
        Ok(needed)
    })
}

fn grid_item_area(row: Option<u8>, col: Option<u8>) -> RuntimeResult<(std::ops::Range<usize>, std::ops::Range<usize>)>
where
    CurrentBackend: BoardReadinessBackend + GameUiBackend + SceneBackend + GridItemPoolBackend,
{
    ready()?;
    crate::access::with_backend(|backend| {
        let field = FieldInfo::from_scene(backend.scene().map_err(operation_error)?);
        let axis = |value: Option<u8>, limit: usize| -> RuntimeResult<std::ops::Range<usize>> {
            match value {
                None => Ok(0..limit),
                Some(v) if usize::from(v) < limit => Ok(usize::from(v)..usize::from(v) + 1),
                _ => Err(RuntimeError::new("placement coordinate is outside the current field")),
            }
        };
        let rows = axis(row, field.row_count)?;
        let cols = axis(col, field.col_count)?;
        capacity(
            backend.grid_item_pool_active_count().map_err(operation_error)?,
            backend.grid_item_pool_capacity().map_err(operation_error)?,
            rows.len() * cols.len(),
        )?;
        Ok((rows, cols))
    })
}

pub fn validate_challenge_graves(row: Option<u8>, col: Option<u8>) -> RuntimeResult<()>
where
    CurrentBackend: BoardReadinessBackend + GameUiBackend + SceneBackend + GridItemPoolBackend,
{
    grid_item_area(row, col).map(|_| ())
}

/// Native challenge placement also removes plants on each target grid.
pub fn place_challenge_graves(row: Option<u8>, col: Option<u8>) -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + SceneBackend
        + GridItemPoolBackend
        + rsvz_backend_api::ChallengeGravePlacementBackend,
{
    use rsvz_backend_api::ChallengeGravePlacementBackend;
    let (rows, cols) = grid_item_area(row, col)?;
    let count = rows.len() * cols.len();
    crate::access::with_backend(|backend| {
        for row in rows {
            for col in cols.clone() {
                backend
                    .spawn_challenge_grave(Grid {
                        row: row as i32,
                        col: col as i32,
                    })
                    .map_err(operation_error)?;
            }
        }
        Ok(count)
    })
}

pub fn place_rakes(row: Option<u8>, col: Option<u8>) -> RuntimeResult<usize>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + SceneBackend
        + GridItemPoolBackend
        + rsvz_backend_api::RakePlacementBackend,
{
    use rsvz_backend_api::RakePlacementBackend;
    let (rows, cols) = grid_item_area(row, col)?;
    crate::access::with_backend(|backend| {
        let mut placed = 0;
        for row in rows {
            for col in cols.clone() {
                placed += usize::from(
                    backend
                        .place_rake(Grid {
                            row: row as i32,
                            col: col as i32,
                        })
                        .map_err(operation_error)?,
                );
            }
        }
        Ok(placed)
    })
}

/// Same-layer footprint replacement for explicit editing. Create first, so an allocation
/// failure does not erase the existing plant. Bases are never supplied implicitly.
pub fn replace_plant(selection: rsvz_model::PlantPlacementSelection, grid: Grid) -> RuntimeResult<()>
where
    CurrentBackend: BoardReadinessBackend
        + GameUiBackend
        + SceneBackend
        + PlantPoolBackend
        + rsvz_backend_api::NativePlantPlacementBackend
        + PlantPresenceBackend
        + rsvz_backend_api::PlantRemoveBackend,
{
    use rsvz_backend_api::{NativePlantPlacementBackend, PlantRemoveBackend};
    ready()?;
    crate::access::with_backend(|b| {
        let field = FieldInfo::from_scene(b.scene().map_err(operation_error)?);
        if !field.contains_grid(grid) || selection.kind() == 47 && grid.col + 1 >= field.col_count as i32 {
            return Err(RuntimeError::new("outside the lawn"));
        }
        capacity(
            b.plant_pool_active_count().map_err(operation_error)?,
            b.plant_pool_capacity().map_err(operation_error)?,
            if selection.imitater() >= 0 { 2 } else { 1 },
        )?;
        let mut plant = b.add_placement_plant(selection, grid).map_err(operation_error)?;
        if selection.imitater() >= 0 {
            plant = b.morph_imitator(plant).map_err(operation_error)?;
        }
        let id = b.plant_id(plant);
        for old in b.present_plants().map_err(operation_error)? {
            if b.plant_id(old) != id
                && b.plant_row(old) == grid.row
                && footprints_overlap(
                    b.plant_col(old),
                    b.plant_kind(old).map_err(operation_error)?.code() as u8,
                    grid.col,
                    selection.kind(),
                )
                && plant_layer(b.plant_kind(old).map_err(operation_error)?.code() as u8)
                    == plant_layer(selection.kind())
            {
                b.remove_plant(old).map_err(operation_error)?;
            }
        }
        Ok(())
    })
}
/// Remove the top occupied editor layer: coffee, pumpkin, main plant, then base.
pub fn remove_at(grid: Grid) -> RuntimeResult<()>
where
    CurrentBackend: BoardReadinessBackend + GameUiBackend + PlantPresenceBackend + rsvz_backend_api::PlantRemoveBackend,
{
    use rsvz_backend_api::PlantRemoveBackend;
    ready()?;
    crate::access::with_backend(|b| {
        let mut target = None;
        let mut layer = 0;
        for plant in b.present_plants().map_err(operation_error)? {
            let candidate = plant_layer(b.plant_kind(plant).map_err(operation_error)?.code() as u8);
            if rsvz_model::plant_occupies_grid(
                Grid {
                    row: b.plant_row(plant),
                    col: b.plant_col(plant),
                },
                b.plant_kind(plant).map_err(operation_error)?,
                grid,
            ) && (target.is_none() || candidate >= layer)
            {
                target = Some(plant);
                layer = candidate;
            }
        }
        if let Some(plant) = target {
            b.remove_plant(plant).map_err(operation_error)?;
        }
        Ok(())
    })
}
fn plant_layer(kind: u8) -> u8 {
    match kind {
        16 | 33 => 0,
        30 => 2,
        35 => 3,
        _ => 1,
    }
}
fn footprints_overlap(left: i32, kind: u8, other: i32, other_kind: u8) -> bool {
    left <= other + i32::from(other_kind == 47) && other <= left + i32::from(kind == 47)
}
#[cfg(test)]
mod editor_tests {
    #[test]
    fn replacement_preserves_other_layers() {
        use super::plant_layer as layer;
        assert_eq!(layer(16), layer(33));
        assert_eq!(layer(0), layer(47));
        assert_ne!(layer(30), layer(47));
        assert_ne!(layer(35), layer(30));
    }
    #[test]
    fn cannon_overlap_includes_both_halves() {
        use super::footprints_overlap as overlaps;
        assert!(overlaps(1, 47, 2, 47));
        assert!(overlaps(2, 47, 1, 47));
        assert!(overlaps(1, 47, 2, 0));
        assert!(overlaps(2, 0, 1, 47));
        assert!(!overlaps(1, 47, 3, 47));
        assert!(!overlaps(1, 0, 2, 0));
    }
}
