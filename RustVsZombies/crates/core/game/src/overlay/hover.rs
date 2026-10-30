use super::primitives::rect;
use super::{History, Options};
use crate::paint::{Argb, PaintCanvas, PaintFont};
use crate::runtime::RuntimeResult;
use rsvz_backend_api::*;
use rsvz_current::CurrentBackend;
use rsvz_model::{PixelPos, PlantKind};
const PLANTS: [&str; 49] = [
    "豌豆射手",
    "向日葵",
    "樱桃炸弹",
    "坚果",
    "土豆雷",
    "寒冰射手",
    "大嘴花",
    "双发射手",
    "小喷菇",
    "阳光菇",
    "大喷菇",
    "墓碑吞噬者",
    "魅惑菇",
    "胆小菇",
    "寒冰菇",
    "毁灭菇",
    "荷叶",
    "窝瓜",
    "三线射手",
    "缠绕水草",
    "火爆辣椒",
    "地刺",
    "火炬树桩",
    "高坚果",
    "海蘑菇",
    "路灯花",
    "仙人掌",
    "三叶草",
    "裂荚射手",
    "杨桃",
    "南瓜头",
    "磁力菇",
    "卷心菜投手",
    "花盆",
    "玉米投手",
    "咖啡豆",
    "大蒜",
    "叶子保护伞",
    "金盏花",
    "西瓜投手",
    "机枪射手",
    "双子向日葵",
    "忧郁菇",
    "香蒲",
    "冰瓜",
    "吸金磁",
    "地刺王",
    "玉米加农炮",
    "模仿者",
];
const ZOMBIES: [&str; 33] = [
    "普僵", "旗帜", "路障", "撑杆", "铁桶", "读报", "铁门", "橄榄", "舞王", "伴舞", "鸭子", "潜水", "冰车", "雪橇",
    "海豚", "小丑", "气球", "矿工", "跳跳", "雪人", "蹦极", "扶梯", "投篮", "白眼", "小鬼", "僵博", "豌豆", "坚果",
    "辣椒", "机枪", "窝瓜", "高坚", "红眼",
];
fn plant_name(kind: PlantKind) -> &'static str {
    PLANTS.get(kind as usize).copied().unwrap_or(kind.report_name())
}
fn tooltip(canvas: &mut dyn PaintCanvas, mouse: PixelPos, quadrant: &mut usize, value: &str) -> RuntimeResult<()> {
    let font = PaintFont::new("", 17, false).expect("fixed ShowMe font");
    let (width, height) = canvas.measure_text(value, font)?;
    let (dx, dy) = [(0, -1), (0, 0), (-1, -1), (-1, 0)][(*quadrant).min(3)];
    *quadrant += 1;
    let pos = PixelPos {
        x: mouse.x + width as i32 * dx,
        y: mouse.y + height as i32 * dy,
    };
    rect(canvas, pos.x, pos.y, width as i32, height as i32, Argb(0xaf000000))?;
    canvas.text(pos, value, font, Argb(0xff00ffff))
}
pub(super) fn draw(canvas: &mut dyn PaintCanvas, o: Options, history: &mut History) -> RuntimeResult<()>
where
    CurrentBackend:
        PlantStateBackend + ZombieStateBackend + ZombieRawFactsBackend + SeedCooldownReadBackend + GridItemStateBackend,
{
    if !o.show_me {
        return Ok(());
    }
    canvas.set_origin(canvas.board_position()?.unwrap_or(PixelPos { x: 0, y: 0 }));
    let mouse = canvas.mouse_position()?;
    if let Some(row) = canvas.cursor_grid_row()?
        && row >= 0
    {
        history.mouse_row = row;
    }
    if let Some(pos) = canvas.cursor_sprite_position()?
        && pos.x >= 0
    {
        history.mouse_col = (pos.x + 25) as f32 / 80.0;
    }
    crate::access::with_backend(|b| -> RuntimeResult<()> {
        let mut quadrant = 0;
        let (mut selected, mut distance) = (None, 65535.0f32);
        for p in b.plants()? {
            let kind = b.plant_kind(p)?;
            let (ox, oy) = match kind {
                PlantKind::LilyPad | PlantKind::FlowerPot => (0.5, 1.0),
                PlantKind::Pumpkin => (0.33, 0.66),
                _ => (0.5, 0.5),
            };
            let dx = mouse.x as f32 - b.plant_x(p) as f32 - b.plant_width(p) as f32 * ox;
            let dy = mouse.y as f32 - b.plant_y(p) as f32 - b.plant_height(p) as f32 * oy;
            let d = dx * dx + dy * dy;
            if d <= 900.0 && d < distance {
                distance = d;
                selected = Some(p);
            }
        }
        if let Some(p) = selected {
            let kind = b.plant_kind(p)?;
            let mut value = format!("{}\nHP:{}\n", plant_name(kind), b.plant_hp(p));
            let cd = if matches!(
                kind,
                PlantKind::Sunflower | PlantKind::SunShroom | PlantKind::TwinSunflower | PlantKind::Marigold
            ) && !b.plant_is_sleeping(p)
            {
                Some(b.plant_launch_counter(p))
            } else if kind == PlantKind::CobCannon {
                let cd = crate::logic::cob::cob_recover_time(b.plant_id(p));
                (cd != 0).then_some(cd)
            } else {
                None
            };
            if let Some(cd) = cd {
                use std::fmt::Write;
                write!(&mut value, "CD:{cd}\n").unwrap();
            }
            tooltip(canvas, mouse, &mut quadrant, &value)?;
        }
        let (mut selected, mut distance) = (None, 65535.0f32);
        for z in b.zombies()? {
            let kind = b.zombie_kind(z)? as i32;
            let phase = b.zombie_phase(z)? as i32;
            let (limit, ox, oy) = match (kind, phase) {
                (11, 59) => (50.0, 0.5, 1.0),
                (12, _) | (22, _) => (50.0, 0.5, 0.5),
                (14, _) | (16, 75) => (50.0, 1.2, 0.8),
                (17, 32) => (50.0, 2.0, 1.0),
                (17, _) => (50.0, 2.0, 0.5),
                (20, _) => (50.0, 0.4, 0.5),
                (23, _) | (32, _) => (50.0, 0.5, 0.3),
                (24, _) => (50.0, 1.3, 0.8),
                _ => (30.0, 1.2, 0.5),
            };
            let dx = mouse.x as f32 - b.zombie_pos_x(z) - b.zombie_hurt_width(z) as f32 * ox;
            let dy = mouse.y as f32 - b.zombie_pos_y(z) - b.zombie_hurt_height(z) as f32 * oy;
            let d = dx * dx + dy * dy;
            if d <= limit * limit && d < distance {
                distance = d;
                selected = Some(z);
            }
        }
        if let Some(z) = selected {
            let kind = b.zombie_kind(z)? as usize;
            let mut value = format!(
                "{}\nHP:{}",
                ZOMBIES.get(kind).copied().unwrap_or("僵尸"),
                b.zombie_hp(z)
            );
            use std::fmt::Write;
            for hp in [b.zombie_accessory_1_hp(z), b.zombie_accessory_2_hp(z)] {
                if hp != 0 {
                    write!(&mut value, "+{hp}").unwrap();
                }
            }
            value.push('\n');
            tooltip(canvas, mouse, &mut quadrant, &value)?;
        }
        for seed in b.seeds()? {
            let Some(bounds) = b.seed_canvas_rect(seed) else {
                continue;
            };
            if mouse.x > bounds.x
                && mouse.x < bounds.x + bounds.width
                && mouse.y > bounds.y
                && mouse.y < bounds.y + bounds.height
            {
                let kind = b.seed_kind_code(seed)?;
                let Some(name) = usize::try_from(kind).ok().and_then(|kind| PLANTS.get(kind)) else {
                    continue;
                };
                let value = format!("{}\nCD:{}\n", name, b.seed_cooldown_remaining(seed));
                tooltip(canvas, mouse, &mut quadrant, &value)?;
                break;
            }
        }
        for item in b.grid_items()? {
            if b.grid_item_kind(item)? as i32 == 2
                && history.mouse_row == b.grid_item_row(item)
                && (history.mouse_col - b.grid_item_col(item) as f32 - 1.0).abs() < 0.5
            {
                tooltip(
                    canvas,
                    mouse,
                    &mut quadrant,
                    &format!("弹坑\n值:{}\n", b.grid_item_countdown(item)),
                )?;
                break;
            }
        }
        Ok(())
    })
}
