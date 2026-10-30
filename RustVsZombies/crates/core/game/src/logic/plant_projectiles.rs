//! Damage to a newly planted mushroom from visible hostile projectiles.
//!
//! Native Projectile::UpdateLobMotion (0x46D490), FindCollisionTargetPlant
//! (0x46CAA0): basketball collision uses horizontal overlap >8, not a 2-D box.
use crate::live_value::read_or_abort;
use rsvz_backend_api::{
    GridGeometryBackend, PlantContactBackend, PlantReadBackend, ProjectileReadBackend, SceneBackend,
    ZombieRawFactsBackend, ZombieReadBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{ContactRect, Grid, PlantKind, ZombieKind, ZombiePhase};

/// Separate budgets because an imitator becomes a fresh 300HP mushroom.
/// No allocation or prediction cache; each projectile is advanced once.
pub(crate) fn damage(grid: Grid, horizon: i32, morph: i32) -> (i32, i32)
where
    CurrentBackend:
        ProjectileReadBackend + SceneBackend + GridGeometryBackend + ZombieRawFactsBackend + PlantContactBackend,
{
    let (covered, shielded_target) = basketball_protection(grid, horizon);
    crate::access::with_backend(|b| {
        let x = read_or_abort(b.grid_to_pixel_x(grid.col, grid.row), "projectile target x");
        let body = ContactRect::new(x + 10, 0, x + 70, 80);
        let pool = read_or_abort(b.scene(), "scene").has_pool() && matches!(grid.row, 2 | 3);
        let mut before = 0;
        let mut after = 0;
        for p in read_or_abort(b.projectiles(), "projectiles") {
            let kind = b.projectile_kind(p);
            if kind == 9 && covered {
                continue;
            }
            if b.projectile_row(p) != grid.row || !matches!(kind, 9 | 13) {
                continue;
            }
            let rect = read_or_abort(b.projectile_collision(p), "projectile collision");
            let mut x = b.projectile_pos_x(p);
            let mut z = b.projectile_pos_z(p);
            let mut vz = b.projectile_vel_z(p);
            let vx = b.projectile_vel_x(p);
            let az = b.projectile_acceleration(p);
            let offset = rect.left - x as i32;
            let width = rect.right - rect.left;
            for t in 1..horizon {
                let collision = if kind == 9 {
                    vz += az;
                    z += vz;
                    // Native collision still uses the previous integer mX this tick.
                    vz >= 0.0 && (b.projectile_age(p) + t <= 20 || z > if pool { 100.0 } else { 60.0 })
                } else {
                    true
                };
                if collision && hits(x as i32 + offset, width, body) {
                    let amount = if kind == 9 { 75 } else { 20 };
                    if t < morph {
                        before += amount;
                    } else {
                        after += amount;
                    }
                    break;
                }
                if kind == 9 && collision && z > 80.0 {
                    break;
                }
                x += if kind == 13 { -3.33 } else { vx };
                if x + (width as f32) < 0.0 {
                    break;
                }
            }
        }
        // Potential future basketballs: no hidden ammunition read. Only the
        // visible protection certified above can waive this conservative budget.
        // Otherwise allow one launch per >=300cs.
        // A lone catapult cannot kill a fresh mushroom in either phase.
        let target_y = read_or_abort(b.grid_to_pixel_y(grid.col, grid.row), "projectile target y");
        for z in read_or_abort(b.zombies(), "catapult launchers") {
            if shielded_target {
                break;
            }
            if !b.zombie_is_alive(z)
                || b.zombie_is_mind_controlled(z)
                || b.zombie_row(z) != grid.row
                || read_or_abort(b.zombie_kind(z), "zombie kind") != ZombieKind::Catapult
                || b.zombie_has_flat_tires(z)
                || b.zombie_pos_x(z) < x as f32 + 100.0
            {
                continue;
            }
            let phase = read_or_abort(b.zombie_phase(z), "catapult phase");
            let mut earliest = b.zombie_frozen_countdown(z).max(b.zombie_buttered_countdown(z)).max(0);
            if phase == ZombiePhase::CatapultReloading {
                earliest += b.zombie_phase_counter(z).max(0);
            } else if phase == ZombiePhase::CatapultLaunching {
                let last = read_or_abort(b.zombie_reanim_last_time(z), "catapult animation");
                if last.is_some_and(|t| t > 0.545) {
                    earliest += b.zombie_phase_counter(z).max(0);
                }
            }
            let origin = b.zombie_pos_x(z) + 113.0;
            let vx = -(origin - x as f32 - 20.0).max(40.0) / 120.0;
            let vz = (target_y as f32 - ((b.zombie_int_y(z) as f32) - 44.0)) / 120.0 - 7.0;
            let Some(flight) = aimed_basket_hit(origin, vx, vz, body, pool, horizon) else {
                continue;
            };
            // Launch time is a lower bound, so assign an uncertain shot to both
            // phase budgets if its possible arrival straddles the morph.
            for at in (earliest + flight..horizon).step_by(300) {
                if at < morph {
                    before += 75;
                }
                if horizon > morph {
                    after += 75;
                }
            }
        }
        (before, after)
    })
}

/// Credit a visible umbrella only when no currently visible non-basketball
/// attacker can remove it in this window. Unknown/special movement falls back
/// to the old conservative budget. Future spawns remain outside this query's
/// contract, just as in the enclosing instantaneous-plant safety queries.
fn basketball_protection(grid: Grid, horizon: i32) -> (bool, bool)
where
    CurrentBackend:
        ProjectileReadBackend + SceneBackend + GridGeometryBackend + ZombieRawFactsBackend + PlantContactBackend,
{
    crate::access::with_backend(|b| {
        let launcher = read_or_abort(b.zombies(), "basketball launchers").any(|z| {
            b.zombie_is_alive(z)
                && b.zombie_row(z) == grid.row
                && read_or_abort(b.zombie_kind(z), "basketball launcher kind") == ZombieKind::Catapult
        });
        let airborne = read_or_abort(b.projectiles(), "basketballs")
            .any(|p| b.projectile_row(p) == grid.row && b.projectile_kind(p) == 9);
        if !launcher && !airborne {
            return (false, false);
        }
        let survives = |body: ContactRect, row: i32| {
            for shot in read_or_abort(b.projectiles(), "shield projectiles") {
                if b.projectile_kind(shot) == 13 && b.projectile_row(shot) == row {
                    let rect = read_or_abort(b.projectile_collision(shot), "shield projectile bounds");
                    if rect.right >= body.left && rect.left as f32 - 3.33 * horizon as f32 <= body.right as f32 {
                        return false;
                    }
                }
            }
            for z in read_or_abort(b.zombies(), "shield attackers") {
                if !b.zombie_is_alive(z) || b.zombie_is_mind_controlled(z) || b.zombie_is_blown_away(z) {
                    continue;
                }
                let kind = read_or_abort(b.zombie_kind(z), "shield attacker kind");
                let phase = read_or_abort(b.zombie_phase(z), "shield attacker phase");
                if !b.zombie_has_head(z) && phase != ZombiePhase::JackInTheBoxPopping {
                    continue;
                }
                let zr = b.zombie_row(z);
                let x = b.zombie_pos_x(z);
                if kind == ZombieKind::JackInTheBox && (zr - row).abs() <= 1 {
                    if x + 200. >= body.left as f32 && x - 1.1 * horizon as f32 - 200. <= body.right as f32 {
                        return false;
                    }
                }
                if zr != row {
                    continue;
                }
                if matches!(kind, ZombieKind::Bungee | ZombieKind::Digger) {
                    return false;
                }
                if kind == ZombieKind::Balloon && phase == ZombiePhase::BalloonFlying {
                    continue;
                }
                if matches!(kind, ZombieKind::Gargantuar | ZombieKind::GigaGargantuar) && b.zombie_has_object(z) {
                    return false;
                }
                let speed = match kind {
                    ZombieKind::Zomboni => 0.25,
                    ZombieKind::Catapult => 0.5,
                    ZombieKind::Gargantuar | ZombieKind::GigaGargantuar => 0.4,
                    ZombieKind::Football => 0.85,
                    ZombieKind::Normal | ZombieKind::Conehead | ZombieKind::Buckethead => 0.7,
                    _ => return false,
                };
                let attack = read_or_abort(
                    crate::logic::contact::zombie_attack_bounds_from_handle(b, z),
                    "shield attacker bounds",
                );
                let Some(attack) = attack else {
                    return false;
                };
                if attack.range.right >= body.left
                    && attack.range.left as f32 - speed * horizon as f32 <= body.right as f32
                {
                    return false;
                }
            }
            true
        };
        let shielded = |row: i32, col: i32| {
            read_or_abort(b.plants(), "umbrellas").any(|p| {
                b.plant_is_alive(p)
                    && !b.plant_is_squished(p)
                    && read_or_abort(b.plant_raw_kind(p), "umbrella kind") == PlantKind::UmbrellaLeaf
                    && (b.plant_row(p) - row).abs() <= 1
                    && (b.plant_col(p) - col).abs() <= 1
                    && survives(read_or_abort(b.plant_hit_box(p), "umbrella body"), b.plant_row(p))
            })
        };
        if shielded(grid.row, grid.col) {
            return (true, true);
        }
        let left = read_or_abort(b.plants(), "catapult permanent targets").any(|p| {
            b.plant_is_alive(p)
                && !b.plant_is_squished(p)
                && b.plant_row(p) == grid.row
                && b.plant_col(p) < grid.col
                && matches!(
                    read_or_abort(b.plant_raw_kind(p), "permanent target kind"),
                    PlantKind::GloomShroom | PlantKind::TwinSunflower | PlantKind::UmbrellaLeaf
                )
                && shielded(b.plant_row(p), b.plant_col(p))
                && survives(
                    read_or_abort(b.plant_hit_box(p), "permanent target body"),
                    b.plant_row(p),
                )
        });
        (false, left)
    })
}

fn hits(left: i32, width: i32, target: ContactRect) -> bool {
    (left + width).min(target.right) - left.max(target.left) > 8
}

fn aimed_basket_hit(mut x: f32, vx: f32, mut vz: f32, target: ContactRect, pool: bool, horizon: i32) -> Option<i32> {
    let mut z = 0.0;
    for t in 1..horizon {
        vz += 0.115;
        z += vz;
        if vz >= 0.0 && (t <= 20 || z > if pool { 100.0 } else { 60.0 }) {
            if hits(x as i32, 40, target) {
                return Some(t);
            }
            if z > 80.0 {
                return None;
            }
        }
        x += vx;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_projectile_overlap_is_strictly_more_than_eight_pixels() {
        let target = ContactRect::new(100, 0, 160, 80);
        assert!(!hits(68, 40, target));
        assert!(hits(69, 40, target));
        assert!(!hits(160, 40, target));
    }
}
