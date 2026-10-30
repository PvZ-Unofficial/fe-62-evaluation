//! PT legacy export of present native objects. No health/damage tiers, timers,
//! mower state, or untransformed imitator placeholders are encoded by this form.
use crate::{
    access::operation_error,
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::{GridItemReadBackend, PlantPresenceBackend, PlantReadBackend, PlantStateBackend, SceneBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{Grid, GridItemKind, PlantKind};

#[derive(Clone, Copy)]
struct Item {
    grid: Grid,
    kind: i32,
    row_state: i32,
    col_state: i32,
    imitator: bool,
}
impl Item {
    fn key(self) -> (i32, i32, i32, i32, i32, bool) {
        let priority = match self.kind {
            16 => -2,
            33 => -1,
            30 => 51,
            35 => 52,
            50 => 53,
            49 => 54,
            48 => 55,
            kind => kind,
        };
        (
            self.grid.row,
            self.grid.col,
            priority,
            self.row_state,
            self.col_state,
            self.imitator,
        )
    }
}

/// Retains repeated same-cell entries and the actual rake column in PT text.
/// Converting that text to a compact single-layer-per-cell model may lose data;
/// this function never silently folds those objects into one representative.
pub fn capture_text() -> RuntimeResult<String>
where
    CurrentBackend: SceneBackend + PlantPresenceBackend + PlantStateBackend + GridItemReadBackend,
{
    crate::access::with_backend(|backend| {
        let scene = backend.scene().map_err(operation_error)?;
        let scene_code = super::encode::legacy_scene(scene).map_err(|e| RuntimeError::new(e.to_string()))?;
        let mut items = Vec::new();
        for plant in backend.present_plants().map_err(operation_error)? {
            if backend.plant_is_squished(plant) {
                continue;
            }
            let kind = backend.plant_raw_kind(plant).map_err(operation_error)?;
            if kind == PlantKind::Imitator {
                continue;
            }
            let grid = Grid {
                row: backend.plant_row(plant),
                col: backend.plant_col(plant),
            };
            if !grid.is_in_bounds(scene.row_count(), 9) {
                return Err(RuntimeError::new("plant outside the encoded lawn"));
            }
            items.push(Item {
                grid,
                kind: kind.code(),
                row_state: i32::from(super::validate::may_sleep(kind) && !backend.plant_is_sleeping(plant)),
                col_state: if kind == PlantKind::SunShroom && backend.plant_state_countdown(plant) == 0 {
                    2
                } else {
                    0
                },
                imitator: backend.plant_imitater_kind(plant) == 48,
            });
        }
        for item in backend.grid_items().map_err(operation_error)? {
            let kind = match backend.grid_item_kind(item).map_err(operation_error)? {
                GridItemKind::Grave => 0x32,
                GridItemKind::Rake => 0x31,
                GridItemKind::Ladder => 0x30,
                _ => continue,
            };
            let grid = Grid {
                row: backend.grid_item_row(item),
                col: backend.grid_item_col(item),
            };
            if !grid.is_in_bounds(scene.row_count(), 9) {
                return Err(RuntimeError::new("grid item outside the encoded lawn"));
            }
            items.push(Item {
                grid,
                kind,
                row_state: 0,
                col_state: 0,
                imitator: false,
            });
        }
        items.sort_unstable_by_key(|item| item.key());
        let mut text = scene_code.to_string();
        for item in items {
            super::encode::write_item(
                &mut text,
                item.kind,
                item.grid,
                item.row_state,
                item.col_state,
                item.imitator,
            );
        }
        Ok(text)
    })
}
