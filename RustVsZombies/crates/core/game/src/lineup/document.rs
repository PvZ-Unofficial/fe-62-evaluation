//! An ordered lineup document can retain repeated same-cell objects in legacy
//! text. `Lineup` remains the existing single-object-per-layer compact model.
use super::{Lineup, LineupBase, LineupCell, LineupParseError, LineupPlant};
use crate::model::{Grid, PlantKind, SceneKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineupObject {
    Plant(Grid, LineupPlant),
    Grave(Grid),
    Ladder(Grid),
    Rake(Grid),
}
impl LineupObject {
    pub fn grid(self) -> Grid {
        match self {
            Self::Plant(grid, _) | Self::Grave(grid) | Self::Ladder(grid) | Self::Rake(grid) => grid,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineupDocument {
    scene: SceneKind,
    objects: Vec<LineupObject>,
}
impl LineupDocument {
    pub fn new(scene: SceneKind, objects: Vec<LineupObject>) -> Result<Self, LineupParseError> {
        if !scene.supports_regular_lineup() {
            return Err(LineupParseError::UnsupportedLineupData("unsupported lineup scene"));
        }
        for &object in &objects {
            let grid = object.grid();
            if !grid.is_in_bounds(scene.row_count(), 9) {
                return Err(LineupParseError::InvalidGrid {
                    row: grid.row + 1,
                    col: grid.col + 1,
                });
            }
            match object {
                LineupObject::Rake(grid) => super::validate::validate_rake_grid(grid, scene)?,
                LineupObject::Plant(_, plant) => {
                    super::validate::validate_plant_state(plant, plant.selection.checked()?.effective_kind())?;
                    let mut cell = LineupCell::empty();
                    insert_plant(&mut cell, plant, false)?;
                    super::validate::validate_cell_layers(scene, grid, cell)?;
                }
                _ => {}
            }
        }
        Ok(Self { scene, objects })
    }
    pub const fn scene(&self) -> SceneKind {
        self.scene
    }
    pub fn objects(&self) -> &[LineupObject] {
        &self.objects
    }
    pub fn parse_auto(input: &str) -> Result<Self, LineupParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(LineupParseError::EmptyInput);
        }
        if input.contains(',') || input.parse::<u8>().is_ok() {
            Self::from_text(input)
        } else {
            let lineup = Lineup::from_code(input)?;
            Ok(Self::from_lineup(&lineup))
        }
    }
    pub fn from_lineup(lineup: &Lineup) -> Self {
        let mut objects = Vec::new();
        for (grid, cell) in lineup.iter() {
            if let Some(base) = cell.base.plant_selection() {
                objects.push(LineupObject::Plant(grid, LineupPlant::plain(base)));
            }
            for plant in [cell.main, cell.pumpkin, cell.coffee].into_iter().flatten() {
                objects.push(LineupObject::Plant(grid, plant));
            }
            if cell.base == LineupBase::Grave {
                objects.push(LineupObject::Grave(grid));
            }
            if lineup.rake_grid() == Some(grid) {
                objects.push(LineupObject::Rake(grid));
            }
            if cell.ladder {
                objects.push(LineupObject::Ladder(grid));
            }
        }
        Self {
            scene: lineup.scene(),
            objects,
        }
    }
    pub fn from_text(input: &str) -> Result<Self, LineupParseError> {
        use super::parse::*;
        let mut parts = input.trim().split(',');
        let raw = parts
            .next()
            .ok_or(LineupParseError::EmptyInput)?
            .trim()
            .parse::<u8>()
            .map_err(|_| LineupParseError::UnknownFormat)?;
        let scene = scene_from_legacy_raw(raw)?;
        let mut objects = Vec::new();
        for item in parts {
            let fields = item.split_whitespace().collect::<Vec<_>>();
            let [kind, row, col, state_row, state_col, imitator] = fields.as_slice() else {
                return Err(LineupParseError::UnsupportedLegacyField(item.to_owned()));
            };
            let kind = parse_legacy_hex(kind)?;
            let row = parse_i32(row)?;
            let col = parse_i32(col)?;
            let state_row = parse_i32(state_row)?;
            let state_col = parse_i32(state_col)?;
            let imitator = parse_i32(imitator)? == 1;
            let grid = Grid::from_one_based(row, col).map_err(|_| LineupParseError::InvalidGrid { row, col })?;
            let object = match kind {
                0x30 => LineupObject::Ladder(grid),
                0x31 => LineupObject::Rake(grid),
                0x32 => LineupObject::Grave(grid),
                0..=0x2f => {
                    let kind = plant_kind_from_raw(kind)?;
                    LineupObject::Plant(
                        grid,
                        LineupPlant {
                            selection: selection_for_layer(kind, imitator),
                            awake: legacy_awake_state(scene, kind, state_row),
                            grown: legacy_grown_state(kind, state_row, state_col),
                        },
                    )
                }
                _ => return Err(LineupParseError::UnknownPlant(kind)),
            };
            objects.push(object);
        }
        Self::new(scene, objects)
    }
    /// Conversion must be lossless for representable model fields. In
    /// particular repeated layers/items must not silently overwrite each other.
    pub fn to_lineup(&self) -> Result<Lineup, LineupParseError> {
        self.fold_cells(false)
    }
    // Keep the existing normalized Lineup parser's last-entry-wins behavior.
    // Modifier import/export uses the ordered document, never this conversion.
    pub(super) fn normalized_lineup(&self) -> Result<Lineup, LineupParseError> {
        self.fold_cells(true)
    }
    fn fold_cells(&self, overwrite: bool) -> Result<Lineup, LineupParseError> {
        let mut cells = super::parse::empty_cells(self.scene);
        let mut rake = None;
        for &object in &self.objects {
            let grid = object.grid();
            let cell = &mut cells[grid.row as usize * 9 + grid.col as usize];
            match object {
                LineupObject::Plant(_, plant) => insert_plant(cell, plant, overwrite)?,
                LineupObject::Grave(_) => {
                    if cell.base != LineupBase::None && !overwrite {
                        return Err(overlap());
                    }
                    cell.base = LineupBase::Grave;
                }
                LineupObject::Ladder(_) => {
                    if cell.ladder && !overwrite {
                        return Err(overlap());
                    }
                    cell.ladder = true;
                }
                LineupObject::Rake(grid) => {
                    if rake.replace(grid).is_some() && !overwrite {
                        return Err(overlap());
                    }
                }
            }
        }
        Lineup::new(self.scene, cells, rake)
    }
    pub fn to_code(&self) -> Result<String, LineupParseError> {
        self.to_lineup()?.to_code()
    }
    pub fn to_text(&self) -> Result<String, LineupParseError> {
        let mut text = super::encode::legacy_scene(self.scene)?.to_string();
        for &object in &self.objects {
            match object {
                LineupObject::Plant(grid, plant) => super::encode::write_plant(&mut text, grid, plant, self.scene)?,
                LineupObject::Grave(grid) => super::encode::write_item(&mut text, 0x32, grid, 0, 0, false),
                LineupObject::Ladder(grid) => super::encode::write_item(&mut text, 0x30, grid, 0, 0, false),
                LineupObject::Rake(grid) => super::encode::write_item(&mut text, 0x31, grid, 0, 0, false),
            }
        }
        Ok(text)
    }
}
fn overlap() -> LineupParseError {
    LineupParseError::UnsupportedLineupData("compact lineup cannot retain repeated objects or overlapping bases")
}
fn insert_plant(cell: &mut LineupCell, plant: LineupPlant, overwrite: bool) -> Result<(), LineupParseError> {
    let selection = plant.selection.checked()?;
    let imitator = selection.imitator_target().is_some();
    match selection.effective_kind() {
        PlantKind::LilyPad | PlantKind::FlowerPot => {
            if cell.base != LineupBase::None && !overwrite {
                return Err(overlap());
            }
            cell.base = if selection.effective_kind() == PlantKind::LilyPad {
                LineupBase::LilyPad { imitator }
            } else {
                LineupBase::FlowerPot { imitator }
            };
        }
        kind => {
            let slot = match kind {
                PlantKind::Pumpkin => &mut cell.pumpkin,
                PlantKind::CoffeeBean => &mut cell.coffee,
                _ => &mut cell.main,
            };
            if slot.replace(plant).is_some() && !overwrite {
                return Err(overlap());
            }
        }
    }
    Ok(())
}
