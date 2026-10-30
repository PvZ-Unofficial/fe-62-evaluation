use super::primitives::{Bar, Direction, bar, number, rect};
use super::{History, InfoMode, Options};
use crate::paint::{Argb, PaintCanvas, PaintFont};
use crate::runtime::RuntimeResult;
use rsvz_backend_api::*;
use rsvz_current::CurrentBackend;
use rsvz_model::{PixelPos, PlantKind, SceneKind};

pub(super) fn grid_y(scene: SceneKind, row: i32, col: i32) -> i32 {
    if scene.has_roof() {
        70 + row * 85 + (5 - col).max(0) * 20
    } else {
        80 + row * if scene.has_pool() { 85 } else { 100 }
    }
}
pub(super) fn text(
    canvas: &mut dyn PaintCanvas, x: i32, y: i32, value: &str, size: u16, color: Argb,
) -> RuntimeResult<()> {
    canvas.text(
        PixelPos { x, y },
        value,
        PaintFont::new("", size, false).expect("fixed game font"),
        color,
    )
}
pub(super) fn draw(canvas: &mut dyn PaintCanvas, o: Options, history: &mut History) -> RuntimeResult<()>
where
    CurrentBackend: CursorQueryBackend
        + PlantStateBackend
        + ZombieRawFactsBackend
        + ZombieStateBackend
        + GridItemStateBackend
        + SceneBackend
        + BoardStateBackend
        + WaveTimingBackend
        + GridTerrainBackend
        + GameSpeedHintBackend
        + WaveHealthBackend,
{
    if o.info == InfoMode::Off {
        return Ok(());
    }
    canvas.set_origin(canvas.board_position()?.unwrap_or(PixelPos { x: 0, y: 0 }));
    crate::access::with_backend(|b| -> RuntimeResult<()> {
        let scene = b.scene()?;
        let colors = o.colors;
        for plant in b.plants()? {
            let kind = b.plant_kind(plant)?;
            let (px, py, row, col, hp, max) = (
                b.plant_x(plant),
                b.plant_y(plant),
                b.plant_row(plant),
                b.plant_col(plant),
                b.plant_hp(plant),
                b.plant_max_hp(plant),
            );
            let y = grid_y(scene, row, col);
            if matches!(
                kind,
                PlantKind::Sunflower | PlantKind::SunShroom | PlantKind::Marigold | PlantKind::TwinSunflower
            ) && !b.plant_is_sleeping(plant)
                && o.produce_cd
            {
                let cur = b.plant_launch_counter(plant);
                let data = history.production.entry(b.plant_id(plant)).or_insert((2500, i32::MAX));
                if cur > data.1 {
                    data.0 = cur;
                }
                data.1 = cur;
                bar(
                    canvas,
                    px + 4,
                    y + 17,
                    2500,
                    data.0 - cur,
                    &[2350.0],
                    Direction::Right,
                    72,
                    7,
                    colors.produce_cd,
                )?;
            }
            if kind == PlantKind::CobCannon && o.cob_cd {
                let remaining = crate::logic::cob::cob_recover_time(b.plant_id(plant));
                if remaining > 0 {
                    bar(
                        canvas,
                        px + 4,
                        py + 17,
                        3475,
                        3475 - remaining,
                        &[350.0, 3350.0],
                        Direction::Right,
                        152,
                        7,
                        colors.cob_cd,
                    )?;
                }
            }
            if hp != max {
                let (enabled, color, x, y, width, thirds) = match kind {
                    PlantKind::CobCannon => (o.cob_gloom_hp, colors.cob_gloom_hp, px + 4, py + 25, 152, false),
                    PlantKind::GloomShroom => (o.cob_gloom_hp, colors.cob_gloom_hp, px + 4, py + 25, 72, false),
                    PlantKind::CoffeeBean => (o.other_plant_hp, colors.other_plant_hp, px + 4, py + 13, 72, false),
                    PlantKind::WallNut | PlantKind::TallNut | PlantKind::Spikerock => {
                        (o.nut_spike_hp, colors.nut_spike_hp, px + 4, py + 25, 72, true)
                    }
                    PlantKind::Pumpkin => (o.pumpkin_hp, colors.pumpkin_hp, px + 4, py + 45, 72, true),
                    PlantKind::LilyPad | PlantKind::FlowerPot => {
                        (o.lily_pot_hp, colors.lily_pot_hp, px + 4, py + 57, 72, false)
                    }
                    PlantKind::Squash => (o.other_plant_hp, colors.other_plant_hp, px + 4, py + 25, 72, false),
                    _ => (
                        o.other_plant_hp,
                        colors.other_plant_hp,
                        44 + col * 80,
                        y + 25,
                        72,
                        false,
                    ),
                };
                let marks = [f64::from(max / 3), f64::from(max * 2 / 3)];
                if enabled {
                    bar(
                        canvas,
                        x,
                        y,
                        max,
                        hp,
                        if thirds { &marks } else { &[] },
                        Direction::Right,
                        width,
                        11,
                        color,
                    )?;
                }
            }
        }
        if o.crater {
            for item in b.grid_items()? {
                if b.grid_item_kind(item)? as i32 != 2 {
                    continue;
                }
                let (row, col) = (b.grid_item_row(item), b.grid_item_col(item));
                Bar {
                    x: 44 + col * 80,
                    y: grid_y(scene, row, col) + 62,
                    denominator: 18000.0,
                    numerator: f64::from(b.grid_item_countdown(item)),
                    separators: &[],
                    direction: Direction::Right,
                    length: 72,
                    width: 6,
                    color: colors.crater,
                    background: Argb(0xa0ffffff),
                    frame: Argb(0xff000000),
                    border: 0,
                }
                .draw(canvas)?;
            }
        }
        if o.ice_trail {
            for row in 0..6 {
                let edge = b.ice_path_x(row)?;
                let remaining = b.ice_path_countdown(row)?;
                // Native IsIceAt (0x40DFC0): active timer, edge <= 750,
                // then PixelToGridXKeepOnBoard(edge + 12, 0).
                if remaining != 0 && edge <= 750 {
                    let col = ((edge.saturating_add(12) - 40) / 80).clamp(0, 8);
                    Bar {
                        x: 44 + col * 80,
                        y: grid_y(scene, row as i32, col) + 50,
                        denominator: 3000.0,
                        numerator: f64::from(remaining),
                        separators: &[],
                        direction: Direction::Right,
                        length: 72,
                        width: 6,
                        color: colors.ice_trail,
                        background: Argb(0xa0ffffff),
                        frame: Argb(0xff000000),
                        border: 0,
                    }
                    .draw(canvas)?;
                }
            }
        }
        let wave = b.current_wave()?.0;
        let mut counts = [[0i32; 6]; 3];
        let mut this_wave = [[0i32; 6]; 3];
        // Keep native A-TAS overlap order: football, gargantuar, giga.
        for pass in 0..3 {
            for zombie in b.zombies()? {
                let kind = b.zombie_kind(zombie)? as i32;
                let (x, y, hp, row) = (
                    b.zombie_pos_x(zombie) as i32,
                    b.zombie_pos_y(zombie) as i32,
                    b.zombie_hp(zombie),
                    b.zombie_row(zombie),
                );
                if pass == 0 && (kind == 7 && hp >= 90 || kind == 12) || pass == 2 && kind == 32 {
                    if let Ok(row) = usize::try_from(row)
                        && row < 6
                    {
                        let group = if kind == 7 {
                            2
                        } else if kind == 12 {
                            1
                        } else {
                            0
                        };
                        counts[group][row] += 1;
                        if b.zombie_from_wave(zombie) == wave - 1 {
                            this_wave[group][row] += 1;
                        }
                    }
                }
                if pass == 0 && kind == 7 && hp >= 90 && o.football_hp {
                    bar(
                        canvas,
                        x + 81,
                        y + 69,
                        1580,
                        b.zombie_accessory_1_hp(zombie) + hp - 90,
                        &[180.0],
                        Direction::Up,
                        36,
                        6,
                        colors.football_hp,
                    )?;
                } else if pass == 1 && kind == 23 && o.garg_hp {
                    bar(
                        canvas,
                        x + 49,
                        y + 59,
                        3000,
                        hp,
                        if o.hp_style { &[1200.0] } else { &[1500.0, 1800.0] },
                        Direction::Up,
                        40,
                        8,
                        colors.garg_hp,
                    )?;
                } else if pass == 2 && kind == 32 && o.giga_hp {
                    bar(
                        canvas,
                        x + 49,
                        y + 79,
                        6000,
                        hp,
                        if o.hp_style {
                            &[600.0, 2400.0, 4200.0]
                        } else {
                            &[1800.0, 3000.0, 4800.0]
                        },
                        Direction::Up,
                        80,
                        8,
                        colors.giga_hp,
                    )?;
                }
            }
        }
        if o.jack_explosion_range {
            for plant in b.plants()? {
                let kind = b.plant_kind(plant)?;
                let (x, y) = (b.plant_x(plant), b.plant_y(plant));
                let (left, right) = match kind {
                    PlantKind::TallNut => (10, 90),
                    PlantKind::Pumpkin => (0, 100),
                    PlantKind::CobCannon => (0, 140),
                    _ => (10, 70),
                };
                let mut hit = false;
                for z in b.zombies()? {
                    if b.zombie_kind(z)? as i32 != 15 || b.zombie_phase(z)? as i32 != 16 {
                        continue;
                    }
                    let (zx, zy) = (b.zombie_pos_x(z) as i32 + 60, b.zombie_pos_y(z) as i32 + 60);
                    let dy = if zy < y { y - zy } else { (zy - y - 80).max(0) };
                    if dy > 90 {
                        continue;
                    }
                    let dx = f64::from(8100 - dy * dy).sqrt() as i32;
                    if x + left - dx <= zx && zx <= x + right + dx {
                        hit = true;
                        break;
                    }
                }
                if hit {
                    let (bx, by, width) = match kind {
                        PlantKind::CobCannon => (x + 4, y + 25, 152),
                        PlantKind::CoffeeBean => (x + 4, y + 13, 72),
                        PlantKind::Pumpkin => (x + 4, y + 45, 72),
                        PlantKind::LilyPad | PlantKind::FlowerPot => (x + 4, y + 57, 72),
                        PlantKind::Squash => (x + 4, y + 25, 72),
                        _ => (
                            44 + b.plant_col(plant) * 80,
                            grid_y(scene, b.plant_row(plant), b.plant_col(plant)) + 25,
                            72,
                        ),
                    };
                    rect(canvas, bx, by, width, 11, colors.jack_explosion_range)?;
                }
            }
        }
        if o.plant_offset {
            for plant in b.plants()? {
                let (row, col) = (b.plant_row(plant), b.plant_col(plant));
                let offset = b.plant_x(plant) - (40 + col * 80);
                if offset == 0 {
                    continue;
                }
                let height = if b.plant_hp(plant) < b.plant_max_hp(plant) || !o.other_plant_hp {
                    0
                } else {
                    11
                };
                let x = 40 + col * 80 + if offset > 0 { 56 } else { 8 };
                let y = grid_y(scene, row, col);
                rect(canvas, x + 1, y + 25, 14, height, colors.other_plant_hp)?;
                number(
                    canvas,
                    x,
                    y + 25,
                    11,
                    &format!("{}{}", if offset > 0 { "R" } else { "L" }, offset.unsigned_abs()),
                    PaintFont::new("Arial", 12, false).unwrap(),
                    Argb(0xff000000),
                )?;
            }
        }
        if o.jack_countdown {
            for z in b.zombies()? {
                if b.zombie_kind(z)? as i32 == 15 && b.zombie_phase(z)? as i32 == 16 {
                    bar(
                        canvas,
                        b.zombie_pos_x(z) as i32 + 65,
                        b.zombie_pos_y(z) as i32 + 87,
                        110,
                        b.zombie_phase_counter(z),
                        &[100.0],
                        Direction::Up,
                        55,
                        10,
                        colors.jack_countdown,
                    )?;
                }
            }
        }
        for row in 0..6 {
            if b.grid_row_kind(row as u8)? != rsvz_model::GridRowKind::Grass {
                continue;
            }
            let y = grid_y(scene, row as i32, 0)
                + if scene.has_pool() || scene.has_roof() {
                    85 / 2
                } else {
                    100 / 2
                };
            for (group, (enabled, color)) in [
                (o.giga_count, colors.giga_count),
                (o.zomboni_count, colors.zomboni_count),
                (o.football_count, colors.football_count),
            ]
            .into_iter()
            .enumerate()
            {
                if !enabled || counts[group][row] == 0 {
                    continue;
                }
                let value = format!("{}/{}", this_wave[group][row], counts[group][row]);
                let y = y + (group as i32 - 1) * 16;
                rect(canvas, 0, y + 3, value.len() as i32 * 9 + 2, 16, color)?;
                text(canvas, 0, y, &value, 17, Argb(0xffffffff))?;
            }
        }
        canvas.set_origin(PixelPos { x: 0, y: 0 });
        if o.giga_stat {
            let total = b.total_waves()?;
            let (mut current, mut cumulative, mut all) = (0, 0, 0);
            for w in 0..total {
                let mut n = 0;
                for slot in 0..50 {
                    if b.spawn_entry(w as u32, slot)? == 32 {
                        n += 1;
                    }
                }
                all += n;
                if w < wave {
                    cumulative += n;
                }
                if w == wave - 1 {
                    current = n;
                }
            }
            if all > 0 {
                rect(
                    canvas,
                    19,
                    7,
                    61,
                    53,
                    if cumulative < 50 {
                        colors.giga_stat_low
                    } else {
                        colors.giga_stat_high
                    },
                )?;
                let advanced = o.info == InfoMode::Advanced;
                text(
                    canvas,
                    22,
                    if advanced { 6 } else { 11 },
                    &format!("Wave{current:2}"),
                    17,
                    Argb(0xffffffff),
                )?;
                text(
                    canvas,
                    22,
                    if advanced { 22 } else { 33 },
                    &format!("Sum{cumulative:3}"),
                    17,
                    Argb(0xffffffff),
                )?;
                if advanced {
                    text(canvas, 22, 38, &format!("All{all:3}"), 17, Argb(0xffffffff))?;
                }
            }
        }
        if o.vase_stat && b.endless_vases_active()? {
            let mut counts = [0; 13];
            for item in b.grid_items()? {
                if b.grid_item_kind(item)? as i32 != 7 {
                    continue;
                }
                let group = match b.vase_content_kind(item)? {
                    1 => {
                        let kind = b.vase_plant_kind(item)?;
                        [0, 52, 18, 5, 17, 4, 3, 25]
                            .iter()
                            .position(|value| *value == kind)
                            .map(|index| index + 1)
                    }
                    2 => {
                        let kind = b.vase_zombie_kind(item)?;
                        [0, 4, 15, 23]
                            .iter()
                            .position(|value| *value == kind)
                            .map(|index| index + 9)
                    }
                    3 => Some(0),
                    _ => None,
                };
                if let Some(group) = group {
                    counts[group] += 1;
                }
            }
            let labels = [
                "阳", "单", "双", "三", "冰", "窝", "雷", "坚", "灯", "普", "桶", "丑", "巨",
            ];
            let mut value = String::new();
            use std::fmt::Write;
            for (index, (count, label)) in counts.into_iter().zip(labels).enumerate() {
                if index != 0 {
                    value.push_str("\n\n");
                }
                write!(&mut value, "{count}{label}").expect("String formatting");
            }
            text(canvas, 770, 120, &value, 17, colors.vase_stat)?;
        }
        if o.cob_col_preview
            && b.cursor_type()? == 8
            && let Some(pos) = canvas.cursor_sprite_position()?
        {
            rect(canvas, pos.x + 1, pos.y + 22, 49, 14, Argb(0xc0ffffff))?;
            let col = (pos.x + 25) / 80;
            text(
                canvas,
                pos.x,
                pos.y + 18,
                &if col < 10 { format!("{col}.") } else { "X.".to_owned() },
                17,
                Argb(0xff000000),
            )?;
            text(
                canvas,
                pos.x + 13,
                pos.y + 18,
                &format!("{:04}", (pos.x + 25) % 80 * 125),
                17,
                Argb(0xff000000),
            )?;
        }
        super::waves::draw(canvas, o, history)?;
        if o.show_speed {
            let speed = b.game_speed_multiplier()?;
            if speed != 1.0 {
                let color = if speed < 1.0 {
                    colors.speed_slow
                } else {
                    colors.speed_fast
                };
                Bar {
                    x: 6,
                    y: 574,
                    denominator: 1.0,
                    numerator: 1.0,
                    separators: &[],
                    direction: Direction::Right,
                    length: 46,
                    width: 24,
                    color,
                    background: Argb(0xffffffff),
                    frame: Argb(0xff000000),
                    border: 1,
                }
                .draw(canvas)?;
                number(
                    canvas,
                    8,
                    574,
                    24,
                    &format!("{speed:.2}x"),
                    PaintFont::new("", 17, false).unwrap(),
                    Argb(0xff000000),
                )?;
            }
        }
        Ok(())
    })
}
