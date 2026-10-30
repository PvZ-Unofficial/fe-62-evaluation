use super::*;
use diagnostics::Condition as C;

#[derive(Clone, Copy)]
pub struct Row {
    pub foot: f32,
    pub old_foot: f32,
    pub giant: f32,
    pub giant_hp: i32,
    pub red: bool,
    pub imp: f32,
    pub car: f32,
    pub balloon: f32,
    pub smashing: bool,
    pub giant_frozen: bool,
}
impl Default for Row {
    fn default() -> Self {
        Self {
            foot: 9999.,
            old_foot: 9999.,
            giant: 9999.,
            giant_hp: 0,
            red: false,
            imp: 9999.,
            car: 9999.,
            balloon: 9999.,
            smashing: false,
            giant_frozen: false,
        }
    }
}
pub fn scan(scene: &prediction::Scene, w: i32) -> [Row; 6] {
    let mut rows = [Row::default(); 6];
    // The intervening Ice/Doom/Lily planting and Puff removal do not change sampled zombie state.
    for z in &scene.zombies {
        let r = &mut rows[(z.row - 1) as usize];
        let x = z.x;
        match z.kind {
            Z::Football => {
                r.foot = r.foot.min(x);
                if z.carried || z.wave < w {
                    r.old_foot = r.old_foot.min(x);
                }
            }
            Z::GigaGargantuar | Z::Gargantuar => {
                r.red |= z.kind == Z::GigaGargantuar;
                if x < r.giant || x == r.giant && z.hp > r.giant_hp {
                    r.giant = x;
                    r.giant_hp = z.hp;
                    r.smashing = z.phase == ZombiePhase::GargantuarSmashing;
                    r.giant_frozen = z.frozen > 0;
                }
            }
            Z::Imp => r.imp = r.imp.min(x),
            Z::Zomboni | Z::Catapult => r.car = r.car.min(x),
            Z::Balloon if z.phase == ZombiePhase::BalloonFlying => r.balloon = r.balloon.min(x),
            _ => {}
        }
    }
    rows
}
pub fn candidate(c: CardSelection, p: Pos) -> bool {
    if main_kind(p).is_some_and(|k| k != PuffShroom) {
        return false;
    }
    if (p.0 == 3 || p.0 == 4) && hp(LilyPad, p).is_none() {
        ready(sel(LilyPad)) && legal(sel(LilyPad), p)
    } else {
        legal(c, p) || main_kind(p) == Some(PuffShroom)
    }
}
pub fn cherry_preserves_ladders(pos: Pos) -> bool {
    // Native KillAllZombiesInRadius removes ladders by a3x3 grid test,
    // independently of the circular zombie damage geometry.
    rsvz::with_frame(|frame| {
        !frame.grid_items().any(|item| {
            let grid = item.grid();
            item.is_alive()
                && item.kind() == rsvz::core::model::GridItemKind::Ladder
                && (grid.row + 1 - pos.0).abs() <= 1
                && (grid.col + 1 - pos.1).abs() <= 1
        })
    })
}
pub fn jack_safe(p: Pos, horizon: i32) -> bool {
    let px = (p.1 * 80 - 40) as f32;
    rsvz::with_frame(|b| {
        !b.zombies().any(|z| {
            if !z.is_alive() || z.kind() != Z::JackInTheBox || (z.row() + 1 - p.0).abs() > 1 {
                return false;
            }
            let x = z.x();
            if z.phase() == ZombiePhase::JackInTheBoxPopping {
                // The open-box animation is visible. Never read the unopened fuse.
                z.phase_counter() <= horizon + 2 && x >= px - 135. && x <= px + 130.
            } else {
                // Unopened boxes need 110cs before exploding; ordinary 100cs ash finishes first.
                let freeze = if z.is_frozen() {
                    effects::frozen_lower_bound(rsvz::timing::clock())
                } else {
                    0
                };
                z.has_head() && unopened_jack_reaches(x, px, horizon, freeze)
            }
        })
    })
}
fn unopened_jack_reaches(x: f32, px: f32, horizon: i32, freeze: i32) -> bool {
    let walking = horizon - 110 - freeze;
    walking >= 0 && x >= px - 135. && x - 0.7 * walking as f32 <= px + 130.
}
pub fn ice(c: CardSelection, timing_safe: impl Fn(Pos) -> bool) -> bool {
    // strategy::ice_tick has already tried useful land fodder positions.
    if c != COPY {
        for pos in [(5, 1), (2, 3), (5, 3), (1, 3), (6, 3), (1, 1), (6, 1), (2, 4), (5, 4)] {
            if candidate(c, pos)
                && rsvz::is_safe_ice(grid(pos)).unwrap_or(false)
                && jack_safe(pos, 100)
                && timing_safe(pos)
                && place(c, pos, 2, true)
            {
                return true;
            }
        }
        return false;
    }
    for p in [
        (5, 1),
        (4, 5),
        (3, 5),
        (1, 3),
        (6, 3),
        (2, 3),
        (5, 3),
        (3, 7),
        (4, 7),
        (3, 8),
        (4, 8),
        (3, 9),
        (4, 9),
    ] {
        // This formation uses water ice without Jack-in-the-Box screening.
        let water = matches!(p.0, 3 | 4);
        if !candidate(c, p) {
            continue;
        }
        // Only imitator ice reaches the fallback list; water checks dolphins only.
        let safe = if water {
            dolphin::safe(p)
        } else {
            rsvz::is_safe_imitator_ice(grid(p)).unwrap_or(false)
        };
        if safe && place(c, p, 2, true) {
            return true;
        }
    }
    diagnostics::mark(C::IceNoGrid);
    false
}
pub fn doom_position(preferred: Pos, nine_only: bool) -> Option<Pos> {
    doom_position_except(preferred, nine_only, None)
}
pub fn doom_position_except(preferred: Pos, nine_only: bool, excluded: Option<Pos>) -> Option<Pos> {
    let candidates = [preferred, (3, 7), (4, 7), (3, 8), (4, 8), (3, 9), (4, 9)];
    candidates.into_iter().find(|&p| {
        Some(p) != excluded && (!nine_only || p.1 == 9) && candidate(sel(DoomShroom), p) && jack_safe(p, 100)
    })
}
pub fn doom(preferred: Pos, nine_only: bool) -> bool {
    if !ready(sel(DoomShroom)) {
        return false;
    }
    if let Some(p) = doom_position(preferred, nine_only) {
        return place(sel(DoomShroom), p, 3, true);
    }
    diagnostics::mark(C::DoomNoGrid);
    false
}
pub fn jalapeno(row: i32, reason: i32) -> bool {
    if !ready(sel(Jalapeno)) {
        return false;
    }
    for col in [2, 3, 1, 4, 5, 6, 7, 8, 9] {
        let p = (row, col);
        if candidate(sel(Jalapeno), p) && jack_safe(p, 100) && place(sel(Jalapeno), p, reason, true) {
            return true;
        }
    }
    false
}
pub fn maintenance() {
    if ready(sel(Pumpkin)) {
        let target = PUMPKINS
            .into_iter()
            .filter_map(|p| {
                let health = hp(Pumpkin, p).unwrap_or(0);
                (health < 900).then_some((p, health))
            })
            .min_by_key(|&(p, h)| h - if p.0 == 3 || p.0 == 4 { 400 } else { 0 });
        if let Some((p, _)) = target {
            if play(sel(Pumpkin), p, 7) {
                diagnostics::mark(C::PumpkinRepaired);
            }
        }
    } else if PUMPKINS.iter().any(|&p| hp(Pumpkin, p).is_none()) {
        diagnostics::mark(C::PumpkinWaiting);
    }
}
