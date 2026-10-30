//! Synchronous safety of planting an ordinary Blover now.

use crate::live_value::read_or_abort;
use crate::logic::contact::{circle_hits_rect, zombie_attack_bounds_from_handle};
use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::{
    GridGeometryBackend, PlantContactBackend, PlantReadBackend, ProjectileReadBackend, SceneBackend,
    ZombieRawFactsBackend, ZombieReadBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{AbsoluteContactRange, ContactCircle, ContactRect, Grid, PlantKind, ZombieKind, ZombiePhase};

const BLOVER_DELAY: i32 = 50;
const SMASH_EVENT: f32 = 0.64;

/// Whether ordinary Blover planted at this zero-based grid now can safely reach its blow effect.
///
/// Checks visible Jack explosions, Gargantuar smashes, Bungee grabs and vehicle
/// crushes through a pumpkin/container, and visible hostile projectiles.
/// Assumes normal endless mechanics, an ordinary (not imitator) Blover, and no
/// subsequent script edits to the square or zombie movement/animation parameters.
/// Card availability, sun, planting legality, retries and shovelling belong to the caller.
/// This query neither plants a card nor installs a task.
/// Native read faults abort the current callback; returned errors describe invalid
/// inputs or states outside the safety model. They must not be interpreted as safe.
pub fn is_safe_blover(grid: Grid) -> RuntimeResult<bool>
where
    CurrentBackend:
        SceneBackend + GridGeometryBackend + PlantContactBackend + ZombieRawFactsBackend + ProjectileReadBackend,
{
    instant_safe(grid, PlantKind::Blover)
}

/// Safety of an ordinary awake Ice-shroom in a flat night scene.
/// An early hammer activates the ice and is therefore successful.
/// Availability, sun and planting legality remain the caller's responsibility.
pub fn is_safe_ice(grid: Grid) -> RuntimeResult<bool>
where
    CurrentBackend:
        SceneBackend + GridGeometryBackend + PlantContactBackend + ZombieRawFactsBackend + ProjectileReadBackend,
{
    instant_safe(grid, PlantKind::IceShroom)
}

/// Safety of a Jalapeno through its100cs activation window on a flat scene.
/// Includes visible projectiles and conservative future basketball budgets.
/// A hammer can activate it early; timing constraints belong to the caller.
pub fn is_safe_jalapeno(grid: Grid) -> RuntimeResult<bool>
where
    CurrentBackend:
        SceneBackend + GridGeometryBackend + PlantContactBackend + ZombieRawFactsBackend + ProjectileReadBackend,
{
    instant_safe(grid, PlantKind::Jalapeno)
}

fn instant_safe(grid: Grid, kind: PlantKind) -> RuntimeResult<bool>
where
    CurrentBackend:
        SceneBackend + GridGeometryBackend + PlantContactBackend + ZombieRawFactsBackend + ProjectileReadBackend,
{
    let instant_ash = kind != PlantKind::Blover;
    let delay = if instant_ash { 100 } else { BLOVER_DELAY };
    crate::access::with_backend(|b| {
        if !grid.is_in_bounds(read_or_abort(b.scene(), "scene").row_count(), 9) {
            return Err(RuntimeError::new("Blover grid is outside the current lawn"));
        }
        let scene = read_or_abort(b.scene(), "scene");
        if instant_ash && scene.has_roof() || kind == PlantKind::IceShroom && !scene.is_night() {
            return Err(RuntimeError::new("safe ice requires a flat night scene"));
        }
        if super::plant_projectiles::damage(grid, delay, 0).1 > 300 {
            return Ok(false);
        }
        let x = read_or_abort(b.grid_to_pixel_x(grid.col, grid.row), "grid_to_pixel_x");
        let y = read_or_abort(b.grid_to_pixel_y(grid.col, grid.row), "grid_to_pixel_y");
        let body = ContactRect::new(x + 10, y, x + 70, y + 80);
        let mut hammer = defense(body);
        let mut crush: Option<AbsoluteContactRange> = None;
        read_or_abort(
            b.for_each_plant_at_grid(grid, |p| {
                if b.plant_is_squished(p) {
                    return Ok(());
                }
                match b.plant_raw_kind(p)? {
                    PlantKind::Pumpkin => {
                        let bounds = defense(b.plant_hit_box(p)?);
                        hammer = union(hammer, bounds);
                        crush = Some(crush.map_or(bounds, |old| union(old, bounds)));
                    }
                    PlantKind::FlowerPot | PlantKind::LilyPad => {
                        // CHEW cannot target a covered container, but DRIVE_OVER can.
                        let bounds = defense(b.plant_hit_box(p)?);
                        crush = Some(crush.map_or(bounds, |old| union(old, bounds)));
                    }
                    _ => {}
                }
                Ok(())
            }),
            "plant safety geometry",
        );

        for z in read_or_abort(b.zombies(), "zombies") {
            if !b.zombie_is_alive(z) || b.zombie_is_disappeared(z) || b.zombie_is_mind_controlled(z) {
                continue;
            }
            let kind = read_or_abort(b.zombie_kind(z), "zombie_kind");
            if !matches!(
                kind,
                ZombieKind::JackInTheBox
                    | ZombieKind::Gargantuar
                    | ZombieKind::GigaGargantuar
                    | ZombieKind::Bungee
                    | ZombieKind::Zomboni
                    | ZombieKind::Catapult
            ) {
                continue;
            }
            let phase = read_or_abort(b.zombie_phase(z), "zombie_phase");
            if matches!(
                phase,
                ZombiePhase::ZombieDying | ZombiePhase::ZombieBurned | ZombiePhase::ZombieMowered
            ) {
                continue;
            }
            if kind == ZombieKind::JackInTheBox {
                // A newly opened jack needs 110cs; a popping jack cannot be frozen/buttered.
                if phase == ZombiePhase::JackInTheBoxPopping && b.zombie_phase_counter(z) <= delay {
                    let circle = ContactCircle::new(
                        b.zombie_int_x(z) + b.zombie_width(z) / 2,
                        b.zombie_int_y(z) + b.zombie_height(z) / 2,
                        90,
                    );
                    if circle_hits_rect(circle, body) {
                        return Ok(false);
                    }
                }
                continue;
            }
            if b.zombie_row(z) != grid.row {
                continue;
            }
            let frozen = b.zombie_frozen_countdown(z).max(b.zombie_buttered_countdown(z));
            if kind == ZombieKind::Bungee {
                // Vanilla bungees keep x=GridToPixelX(target_col,row) throughout descent/grab.
                if phase == ZombiePhase::BungeeAtBottom
                    && b.zombie_pos_x(z) == x as f32
                    && b.zombie_phase_counter(z) <= delay.saturating_sub(frozen.saturating_sub(1).max(0))
                {
                    return Ok(false);
                }
                continue;
            }
            if matches!(kind, ZombieKind::Gargantuar | ZombieKind::GigaGargantuar) {
                // Starting a new smash takes at least 134cs, longer than this window.
                if instant_ash || phase != ZombiePhase::GargantuarSmashing {
                    continue;
                }
                let Some(attack) = zombie_attack_bounds_from_handle(b, z).map_err(error)? else {
                    continue;
                };
                if !overlap(attack.range, hammer) {
                    continue;
                }
                let time = read_or_abort(b.zombie_reanim_anim_time(z), "zombie_reanim_anim_time");
                let last = read_or_abort(b.zombie_reanim_last_time(z), "zombie_reanim_last_time");
                let rate = read_or_abort(b.zombie_reanim_rate(z), "zombie_reanim_rate");
                let frames = read_or_abort(b.zombie_reanim_frame_count(z), "zombie_reanim_frame_count");
                let Some(((time, last), (rate, frames))) = time.zip(last).zip(rate.zip(frames)) else {
                    return Err(RuntimeError::new("smashing Gargantuar has no body animation"));
                };
                if smash_within(time, last, rate, frames, frozen, b.zombie_chilled_countdown(z))? {
                    return Ok(false);
                }
            } else if let Some(target) = crush {
                if b.zombie_has_flat_tires(z) {
                    continue;
                }
                let Some(attack) = zombie_attack_bounds_from_handle(b, z).map_err(error)? else {
                    continue;
                };
                // A stopped catapult can resume driving. Use a conservative short sweep;
                // intervening targets can stop it, and Zomboni's natural speed never exceeds .25.
                let speed = b.zombie_speed_x(z).abs();
                if !speed.is_finite() {
                    return Err(RuntimeError::new("invalid vehicle speed"));
                }
                let speed = if kind == ZombieKind::Zomboni {
                    speed.max(0.25)
                } else {
                    speed
                };
                let distance = vehicle_reach_for(
                    delay,
                    speed,
                    if kind == ZombieKind::Zomboni { 0 } else { frozen },
                    if kind == ZombieKind::Zomboni {
                        0
                    } else {
                        b.zombie_chilled_countdown(z)
                    },
                );
                let swept = AbsoluteContactRange::new(attack.range.left.saturating_sub(distance), attack.range.right);
                if overlap(swept, target) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    })
}

fn error(e: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::new(e.to_string())
}
fn defense(rect: ContactRect) -> AbsoluteContactRange {
    AbsoluteContactRange::new(rect.left + 20, rect.right - 20)
}
fn union(a: AbsoluteContactRange, b: AbsoluteContactRange) -> AbsoluteContactRange {
    AbsoluteContactRange::new(a.left.min(b.left), a.right.max(b.right))
}
fn overlap(a: AbsoluteContactRange, b: AbsoluteContactRange) -> bool {
    a.left <= b.right && a.right >= b.left
}
#[cfg(test)]
fn counter_expires(counter: i32, frozen: i32) -> bool {
    counter <= 0 || counter <= BLOVER_DELAY.saturating_sub(frozen.saturating_sub(1).max(0))
}
fn vehicle_reach_for(delay: i32, speed: f32, mut frozen: i32, mut chilled: i32) -> i32 {
    let mut distance = 0.0;
    for _ in 0..delay {
        frozen = frozen.saturating_sub(1);
        chilled = chilled.saturating_sub(1);
        if frozen <= 0 {
            distance += speed * if chilled > 0 { 0.4 } else { 1.0 };
        }
    }
    distance.ceil() as i32
}

fn smash_within(
    mut time: f32, mut last: f32, rate: f32, frames: i32, mut frozen: i32, mut chilled: i32,
) -> RuntimeResult<bool> {
    if !time.is_finite() || !last.is_finite() || !rate.is_finite() || rate < 0.0 || frames <= 0 {
        return Err(RuntimeError::new("invalid Gargantuar smash animation"));
    }
    // Vanilla anim_smash is 16fps; its observable rate is zero while immobilized.
    let base_rate = if frozen > 0 && rate == 0.0 {
        16.0
    } else if chilled > 0 {
        rate * 2.0
    } else {
        rate
    };
    for _ in 0..BLOVER_DELAY {
        frozen = frozen.saturating_sub(1);
        chilled = chilled.saturating_sub(1);
        let fps = if frozen > 0 {
            0.0
        } else if chilled > 0 {
            base_rate * 0.5
        } else {
            base_rate
        };
        // Native event interval is [last,time); a crossing sampled now can still be pending.
        if fps > 0.0 && last > 0.0 && last <= SMASH_EVENT && SMASH_EVENT < time {
            return Ok(true);
        }
        if last > SMASH_EVENT {
            return Ok(false);
        }
        last = time;
        let scaled = (f64::from(fps) * 0.00999999977648258) as f32;
        let step = (f64::from(scaled) / f64::from(frames)) as f32;
        time = (f64::from(time) + f64::from(step)) as f32;
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smash_window_respects_pending_event_and_thaw() {
        assert!(!smash_within(0.0, -1.0, 16.0, 33, 0, 0).unwrap());
        assert!(smash_within(0.5, 0.495, 16.0, 33, 0, 0).unwrap());
        assert!(!smash_within(0.5, 0.495, 8.0, 33, 0, 100).unwrap());
        assert!(smash_within(0.5, 0.495, 8.0, 33, 0, 1).unwrap());
        assert!(smash_within(0.65, 0.639, 16.0, 33, 0, 0).unwrap());
        assert!(!smash_within(0.65, 0.645, 16.0, 33, 0, 0).unwrap());
        assert!(!smash_within(0.6, 0.59, 0.0, 33, 60, 100).unwrap());
        assert!(smash_within(0.6, 0.59, 0.0, 33, 2, 100).unwrap());
    }
    #[test]
    fn grab_countdown_and_vehicle_sweep_are_bounded() {
        assert!(counter_expires(50, 0));
        assert!(!counter_expires(51, 0));
        assert!(!counter_expires(10, 50));
        assert!(counter_expires(1, 50));
        assert_eq!(vehicle_reach_for(50, 0.25, 0, 0), 13);
        assert_eq!(vehicle_reach_for(50, 0.25, 60, 0), 0);
    }
}
