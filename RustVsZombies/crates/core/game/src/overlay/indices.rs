use super::info::grid_y;
use super::primitives::{number, rect, seven_segment};
use super::{History, IndexMode, InfoMode, Options};
use crate::paint::{Argb, PaintCanvas, PaintFont};
use crate::runtime::RuntimeResult;
use rsvz_backend_api::{PlantPoolBackend, PlantReadBackend, PlantStateBackend, SceneBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{PixelPos, PlantKind};
pub(super) fn draw(canvas: &mut dyn PaintCanvas, o: Options, history: &mut History) -> RuntimeResult<()>
where
    CurrentBackend: PlantStateBackend + PlantPoolBackend + SceneBackend,
{
    if o.index == IndexMode::Off {
        return Ok(());
    }
    canvas.set_origin(canvas.board_position()?.unwrap_or(PixelPos { x: 0, y: 0 }));
    crate::access::with_backend(|b| -> RuntimeResult<()> {
        let scene = b.scene()?;
        let next = b.plant_pool_next_slot()?;
        let mut right = [-1; 6];
        if o.index == IndexMode::All {
            history.front = [-1; 6];
        }
        for plant in b.plants()? {
            let row = b.plant_row(plant);
            if !(0..6).contains(&row) {
                continue;
            }
            let kind = b.plant_kind(plant)?;
            let col = b.plant_col(plant);
            let edge = col + i32::from(kind == PlantKind::CobCannon);
            let row = row as usize;
            right[row] = right[row].max(edge);
            if edge < history.front[row] {
                continue;
            }
            let idx = u32::from(b.plant_id(plant).index());
            let (width, digit_offset) = if idx < 10 {
                (7, 7)
            } else if idx < 100 {
                (14, 4)
            } else {
                (21, 0)
            };
            let (x, y) = (b.plant_x(plant), b.plant_y(plant));
            let c = o.colors;
            let (x, y, color, hp_enabled) = match kind {
                PlantKind::CobCannon => (x + 108, y + 21, c.cob_gloom_hp, o.cob_gloom_hp),
                PlantKind::GloomShroom => (x + 28, y + 21, c.cob_gloom_hp, o.cob_gloom_hp),
                PlantKind::CoffeeBean => (x + 28, y + 9, c.other_plant_hp, o.other_plant_hp),
                PlantKind::WallNut | PlantKind::TallNut | PlantKind::Spikerock => {
                    (x + 28, y + 21, c.nut_spike_hp, o.nut_spike_hp)
                }
                PlantKind::Pumpkin => (x + 52, y + 41, c.pumpkin_hp, o.pumpkin_hp),
                PlantKind::LilyPad | PlantKind::FlowerPot => (x + 4, y + 53, c.lily_pot_hp, o.lily_pot_hp),
                PlantKind::Squash => (x + 28, y + 21, c.other_plant_hp, o.other_plant_hp),
                _ => (
                    40 + 80 * col + 28,
                    grid_y(scene, row as i32, col) + 21,
                    c.other_plant_hp,
                    o.other_plant_hp,
                ),
            };
            let x = x + digit_offset;
            let height = if hp_enabled && o.info != InfoMode::Off && b.plant_hp(plant) < b.plant_max_hp(plant) {
                0
            } else {
                11
            };
            if idx < next {
                seven_segment(canvas, idx, x + 3, y + 6, Argb(0xff0040ff), color, height != 0)?;
            } else {
                rect(canvas, x + 1, y + 4, width, height, color)?;
                number(
                    canvas,
                    x,
                    y + 4,
                    11,
                    &idx.to_string(),
                    PaintFont::new("Arial", 12, false).unwrap(),
                    Argb(0xff000000),
                )?;
            }
        }
        for (front, right) in history.front.iter_mut().zip(right) {
            *front = (*front).min(right);
        }
        canvas.set_origin(PixelPos { x: 0, y: 0 });
        let (x, width) = if next < 10 {
            (634, 19)
        } else if next < 100 {
            (626, 35)
        } else {
            (618, 51)
        };
        rect(canvas, x, 8, width, 26, Argb(0xff4caf50))?;
        number(
            canvas,
            x,
            8,
            26,
            &next.to_string(),
            PaintFont::new("Arial", 30, false).unwrap(),
            Argb(0xff000000),
        )?;
        Ok(())
    })
}
