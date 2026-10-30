//! Water imitator fallback: only Dolphin Riders, no generic ice safety policy.
use super::*;
use rsvz::core::backend::*;

const MORPH: i32 = 320;

// Convert normal-speed animation time to wall time; slowdown halves animation
// rate, but DolphinInJump's horizontal movement remains 0.5 pixels per tick.
fn wall_time(units: f32, slow: i32) -> f32 {
    let slow = slow.max(0) as f32;
    if units * 2.0 <= slow {
        units * 2.0
    } else {
        units + slow * 0.5
    }
}

fn jump_remaining(progress: f32, slow: i32, x: f32) -> (i32, f32, f32) {
    // anim_dolphinjump: 23 frames at 10 fps. Account for the pending native
    // animation event and float rounding with a one-tick position interval.
    let stop = wall_time(((0.49 - progress) * 230.0).max(0.0), slow);
    let land = wall_time(((1.0 - progress) * 230.0).max(0.0), slow);
    let low = x - (stop.ceil() + 1.0) * 0.5 - 94.0;
    let high = x - (stop.floor() - 1.0).max(0.0) * 0.5 - 94.0;
    ((land.floor() as i32 - 1).max(0), low, high)
}

fn bite_budget(at: i32, slow: i32) -> i32 {
    if at >= MORPH {
        return 0;
    }
    let at = at.max(0);
    let chilled = (slow - at).clamp(0, MORPH - at);
    let normal = MORPH - at - chilled;
    4 * ((chilled + 7) / 8 + (normal + 3) / 4)
}

pub fn safe(pos: Pos) -> bool {
    // Native mushroom defense: plant x+30 .. x+50; landed dolphin: x+30 .. x+60.
    let plant_x = (pos.1 - 1) * 80 + 40;
    let left = (plant_x + 30) as f32;
    let right = (plant_x + 50) as f32;
    let damage = rsvz::with_backend(|b| {
        let mut damage = 0;
        for z in b.zombies().expect("dolphin query") {
            if !b.zombie_is_alive(z)
                || !b.zombie_has_head(z)
                || b.zombie_is_mind_controlled(z)
                || b.zombie_row(z) + 1 != pos.0
                || b.zombie_kind(z).expect("kind") != Z::DolphinRider
            {
                continue;
            }
            let phase = b.zombie_phase(z).expect("phase");
            let x = b.zombie_pos_x(z);
            let slow = b.zombie_chilled_countdown(z).max(0);
            let (delay, low, high) = match phase {
                ZombiePhase::DolphinInJump => {
                    let progress = b.zombie_reanim_anim_time(z).expect("animation").unwrap_or(0.0);
                    jump_remaining(progress, slow, x)
                }
                ZombiePhase::DolphinWalkingInPool => (0, x, x),
                ZombiePhase::DolphinRiding => {
                    // A new jump cannot bite for >=230cs. Allow the earliest
                    // possible launch location, without enumerating plant orders.
                    let max_jump = 94.0 + 0.5 * wall_time(0.49 * 230.0, slow);
                    let ride = ((x - max_jump + 30.0 - right).max(0.0) / 0.91).floor() as i32;
                    (230 + ride, right - 30.0, x - 94.0)
                }
                // Entering the pool still needs its animation and a complete
                // obstacle jump; it cannot reach a vulnerable bite in 320cs.
                ZombiePhase::DolphinWalking | ZombiePhase::DolphinIntoPool => {
                    let progress = if phase == ZombiePhase::DolphinIntoPool {
                        b.zombie_reanim_anim_time(z).expect("animation").unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    let enter = (((1.0 - progress) * 45.0 / 0.16).floor() as i32 - 1).max(0);
                    {
                        let max_jump = 94.0 + 0.5 * wall_time(0.49 * 230.0, slow);
                        let ride = ((x - 70.0 - max_jump + 30.0 - right).max(0.0) / 0.91).floor() as i32;
                        (enter + 230 + ride, right - 30.0, x - 70.0 - 94.0)
                    }
                }
                _ => continue,
            };
            if delay >= MORPH || high + 60.0 < left {
                continue;
            }
            // The maximum swim-track displacement at speed .37 is <.37 per
            // tick (42 frames, span 61.3, max increment 2.8). Also bounds the
            // temporary uniform movement after a state change. No phase scan.
            let walk = ((low + 30.0 - right).max(0.0) / 0.37).floor() as i32;
            damage += bite_budget(delay + walk, slow);
            if damage >= 300 {
                break;
            }
        }
        damage
    });
    if damage >= 300 {
        telemetry::record(
            "water_copy_dolphin_reject",
            &[pos.0 as i64, pos.1 as i64, damage as i64],
        );
    }
    damage < 300
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_form_landing_contains_native_pe_jump() {
        use rsvz_pvz_emulator_backend::{PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        for sample in [0.0, 0.2, 0.5, 0.8] {
            let mut world = PeWorldOwner::new_reset(PeWorldConfig {
                scene: pe_rs::SceneType::Fog,
                ..Default::default()
            })
            .unwrap()
            .install_current()
            .unwrap();
            let id = world.with_backend(|b| {
                b.set_zombie_spawn_stopped(true).unwrap();
                b.new_plant(sel(LilyPad).checked().unwrap(), grid((3, 6))).unwrap();
                b.new_plant(sel(WallNut).checked().unwrap(), grid((3, 6))).unwrap();
                let z = b.place_zombie(Z::DolphinRider, grid((3, 9))).unwrap();
                b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(750.0).unwrap())
                    .unwrap();
                b.zombie_id(z)
            });
            let mut predicted = None;
            for _ in 0..2000 {
                world.update_world().unwrap();
                let state = world.with_backend(|b| {
                    let z = b.zombie(id).unwrap().unwrap();
                    (
                        b.zombie_phase(z).unwrap(),
                        b.zombie_reanim_anim_time(z).unwrap().unwrap(),
                        b.zombie_pos_x(z),
                    )
                });
                if state.0 == ZombiePhase::DolphinInJump && state.1 >= sample {
                    predicted = Some(jump_remaining(state.1, 0, state.2));
                    break;
                }
            }
            let (earliest, low, high) = predicted.expect("native jump reached");
            let mut landed = false;
            for elapsed in 1..=600 {
                world.update_world().unwrap();
                let (phase, x) = world.with_backend(|b| {
                    let z = b.zombie(id).unwrap().unwrap();
                    (b.zombie_phase(z).unwrap(), b.zombie_pos_x(z))
                });
                if phase == ZombiePhase::DolphinWalkingInPool {
                    assert!(
                        elapsed >= earliest && x >= low - 0.01 && x <= high + 0.01,
                        "sample={sample}: dt={elapsed}, x={x}, predicted={earliest},[{low},{high}]"
                    );
                    landed = true;
                    break;
                }
            }
            assert!(landed);
        }
    }
    #[test]
    fn jump_uses_closed_form_stop_and_landing() {
        let (t, lo, hi) = jump_remaining(0.0, 0, 500.0);
        assert_eq!(t, 229);
        assert!((348.0..=350.0).contains(&lo) && hi > lo);
        let (slow_t, slow_lo, _) = jump_remaining(0.0, 1000, 500.0);
        assert_eq!(slow_t, 459);
        assert!(slow_lo < lo); // Movement itself is not slowed during the jump.
        let (_, lo, hi) = jump_remaining(0.7, 0, 500.0);
        assert_eq!(hi, 406.0);
        assert_eq!(lo, 405.5);
    }
    #[test]
    fn new_jump_is_not_an_automatic_rejection() {
        assert!(bite_budget(230, 0) < 300);
        assert!(bite_budget(230, 0) * 4 >= 300);
        assert_eq!(bite_budget(320, 0), 0);
        assert!(bite_budget(0, 1000) < 300);
    }
}
