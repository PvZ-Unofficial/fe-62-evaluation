use super::{Lineup, LineupBase, LineupParseError, LineupPlant};
use crate::model::{CardSelection, Grid, PlantKind, SceneKind};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::fmt::Write;

impl Lineup {
    /// PT/PTK compact code. Refuse states the compact format cannot retain;
    /// callers may explicitly choose the legacy representation instead.
    pub fn to_code(&self) -> Result<String, LineupParseError> {
        self.validate()?;
        if self.rake_grid().is_some_and(|grid| grid.col != 7) {
            return Err(LineupParseError::UnsupportedLineupData(
                "compact codes require the rake in column eight",
            ));
        }
        let mut bytes = Vec::with_capacity(self.cells().len() * 2);
        for cell in self.cells() {
            let mut bits = 0_u16;
            if let Some(plant) = cell.main {
                let selection = plant.selection.checked()?;
                let kind = selection.effective_kind();
                if matches!(kind, PlantKind::PotatoMine | PlantKind::SunShroom) && !plant.grown {
                    return Err(LineupParseError::UnsupportedLineupData(
                        "compact codes initialize potato mines and sun-shrooms as grown",
                    ));
                }
                bits |= ((kind.code() + 1) as u16) << 10;
                bits |= u16::from(selection.imitator_target().is_some()) << 9;
                bits |= u16::from(plant.awake.unwrap_or(false)) << 8;
            }
            let (base, imitator) = match cell.base {
                LineupBase::None => (0, false),
                LineupBase::LilyPad { imitator } => (1, imitator),
                LineupBase::FlowerPot { imitator } => (2, imitator),
                LineupBase::Grave => (3, false),
            };
            bits |= base << 6;
            bits |= u16::from(imitator) << 5;
            for (plant, shift) in [(cell.pumpkin, 4), (cell.coffee, 2)] {
                if let Some(plant) = plant {
                    bits |= 1 << shift;
                    bits |= u16::from(matches!(plant.selection, CardSelection::Imitator(_))) << (shift - 1);
                }
            }
            bits |= u16::from(cell.ladder);
            bytes.extend_from_slice(&bits.to_le_bytes());
        }
        let mut payload = miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 9);
        payload.push(((self.rake_row().unwrap_or(0) as u8) << 4) | (self.scene().code() as u8));
        for byte in &mut payload {
            *byte ^= 0x54;
        }
        Ok(STANDARD.encode(payload))
    }

    /// PT legacy text, omitting durability (outside the supported model).
    /// Entries follow PT's row/column/layer priority ordering.
    pub fn to_text(&self) -> Result<String, LineupParseError> {
        self.validate()?;
        let code = legacy_scene(self.scene())?;
        let mut output = code.to_string();
        for (grid, cell) in self.iter() {
            if let Some(base) = cell.base.plant_selection() {
                write_plant(&mut output, grid, LineupPlant::plain(base), self.scene())?;
            }
            for plant in [cell.main, cell.pumpkin, cell.coffee].into_iter().flatten() {
                write_plant(&mut output, grid, plant, self.scene())?;
            }
            if cell.base == LineupBase::Grave {
                write_item(&mut output, 0x32, grid, 0, 0, false);
            }
            if self.rake_grid() == Some(grid) {
                write_item(&mut output, 0x31, grid, 0, 0, false);
            }
            if cell.ladder {
                write_item(&mut output, 0x30, grid, 0, 0, false);
            }
        }
        Ok(output)
    }
}
pub(super) fn write_plant(
    output: &mut String, grid: Grid, plant: LineupPlant, scene: SceneKind,
) -> Result<(), LineupParseError> {
    let selection = plant.selection.checked()?;
    let kind = selection.effective_kind();
    if kind == PlantKind::PotatoMine && !plant.grown {
        return Err(LineupParseError::UnsupportedLineupData(
            "PT legacy initializes potato mines as grown",
        ));
    }
    let awake = if super::validate::may_sleep(kind) {
        plant.awake.unwrap_or(scene.is_night())
    } else {
        false
    };
    if kind == PlantKind::SunShroom && plant.grown && !awake {
        return Err(LineupParseError::UnsupportedLineupData(
            "PT legacy cannot restore a sleeping grown sun-shroom",
        ));
    }
    write_item(
        output,
        kind.code(),
        grid,
        i32::from(awake),
        if kind == PlantKind::SunShroom && plant.grown {
            2
        } else {
            0
        },
        selection.imitator_target().is_some(),
    );
    Ok(())
}
pub(super) fn write_item(output: &mut String, kind: i32, grid: Grid, state_row: i32, state_col: i32, imitator: bool) {
    // Writing to String is infallible; no I/O or formatting implementation runs.
    let _ = write!(
        output,
        ",{kind:X} {} {} {state_row} {state_col} {}",
        grid.row + 1,
        grid.col + 1,
        i32::from(imitator)
    );
}

pub(super) fn legacy_scene(scene: SceneKind) -> Result<i32, LineupParseError> {
    match scene {
        SceneKind::Pool => Ok(0),
        SceneKind::Fog => Ok(1),
        SceneKind::Day => Ok(2),
        SceneKind::Night => Ok(3),
        SceneKind::Roof => Ok(4),
        SceneKind::MoonNight => Ok(5),
        _ => Err(LineupParseError::UnsupportedLineupData(
            "this scene has no PT legacy scene code",
        )),
    }
}
