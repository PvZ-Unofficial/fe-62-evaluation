//! Shared basketball trajectories, built once per observed row/candidate.
use super::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy)]
pub(super) struct Ball {
    pub row: i32,
    pub x: f32,
    pub vx: f32,
    pub height: f32,
    pub vz: f32,
    pub az: f32,
    pub age: i32,
}
#[derive(Clone, Copy)]
pub(super) struct Catapult {
    pub z: Zombie,
    pub count: i32,
    pub y: i32,
    pub known_age: i32,
    pub previous: f32,
}
#[derive(Clone, Copy)]
pub(super) struct Impact {
    pub at: i32,
    pub shot: usize,
    pub left: i32,
    pub right: i32,
}
type Key = (i32, i32, i32, i32, i32, i32, i32, i32);
#[derive(Clone, Default)]
pub(super) struct Observation {
    pub balls: Vec<Ball>,
    pub cars: Vec<Catapult>,
    cache: RefCell<HashMap<Key, Rc<[Impact]>>>,
    row_keys: [u64; 6],
}
impl Observation {
    pub fn clear(&mut self) {
        self.balls.clear();
        self.cars.clear();
        self.cache.get_mut().clear();
        self.row_keys.fill(0);
    }
    pub fn finish(&mut self) {
        let mut add = |row: i32, values: &[u32]| {
            let key = &mut self.row_keys[(row - 1) as usize];
            for &v in values {
                *key = (*key ^ v as u64).wrapping_mul(0x100000001b3);
            }
        };
        for b in &self.balls {
            add(
                b.row,
                &[b.x.to_bits(), b.height.to_bits(), b.vz.to_bits(), b.vx.to_bits()],
            );
        }
        for c in &self.cars {
            add(
                c.z.row,
                &[
                    c.z.id.raw(),
                    c.z.x.to_bits(),
                    c.z.progress.to_bits(),
                    c.count as u32,
                    c.z.frozen as u32,
                    c.z.chilled as u32,
                ],
            );
        }
    }
    pub fn key(&self, row: i32) -> u64 {
        self.row_keys[(row - 1) as usize]
    }
    pub fn active(&self, row: i32) -> bool {
        self.balls.iter().any(|b| b.row == row) || self.cars.iter().any(|c| c.z.row == row)
    }
    pub fn advance(&mut self, at: i32) {
        self.cache.get_mut().clear();
        self.balls.retain_mut(|b| {
            for _ in 0..at.min(250) {
                b.vz += b.az;
                b.height += b.vz;
                b.x += b.vx;
            }
            b.age += at;
            at < 250 && b.height <= 80. && b.x + 40. >= 0.
        });
        // Future candidates are rechecked on the next real observation. Do not
        // reuse an old launcher animation as a fresh shot in a projected scene.
        self.cars.clear();
        self.row_keys.fill(0);
        self.finish();
    }
    pub fn events(&self, scene: &Scene, row: i32, extra: Option<Pad>, f: &Forecast) -> Rc<[Impact]> {
        // Future launches are extrapolated only until the next planned change;
        // launched balls continue through it. Reobserve at that event.
        let stop = f.events.iter().map(|e| e.at).min().unwrap_or(HORIZON);
        let body = f.events.iter().find(|e| e.pos.0 == row && e.body_at.is_some());
        let key = (
            row,
            extra.map_or(0, |p| p.pos.1),
            extra.map_or(-1, |p| p.kind as i32),
            extra.map_or(0, |p| p.offset_x),
            extra.map_or(0, |p| p.plant_at),
            stop,
            body.map_or(0, |e| e.pos.1),
            body.and_then(|e| e.body_at).unwrap_or(-1),
        );
        if let Some(cached) = self.cache.borrow().get(&key) {
            return cached.clone();
        }
        let mut out = Vec::new();
        let mut shot = 0;
        for &ball in self.balls.iter().filter(|b| b.row == row) {
            trace(ball, 0, shot, &mut out);
            shot += 1;
        }
        for car in self.cars.iter().filter(|c| c.z.row == row) {
            let z = car.z;
            let cold = z.frozen;
            let mut x = z.x;
            let mut start = cold;
            let mut progress = 0.;
            match z.phase {
                ZombiePhase::CatapultLaunching if z.progress < 0.545 || car.previous <= 0.545 => progress = z.progress,
                ZombiePhase::CatapultLaunching | ZombiePhase::CatapultReloading => {
                    if car.known_age >= 5700 {
                        continue;
                    }
                    start += car.count.max(0) + 1;
                }
                ZombiePhase::ZombieNormal => {
                    if car.known_age >= 5700 {
                        continue;
                    }
                    let travel = ((x - 650.).max(0.) / z.speed.max(0.001)).ceil() as i32;
                    start += active_time(travel as f32, cold, z.chilled);
                    if aim(scene, row, extra, start, x.min(650.), f).is_none() {
                        start = start.max(extra.map_or(stop, |p| p.plant_at));
                    }
                    let moving = (start - cold).max(0);
                    let slow = moving.min((z.chilled - cold).max(0));
                    x -= z.speed * (moving as f32 - slow as f32 * 0.5);
                    start += 1;
                }
                _ => continue,
            }
            // Only the next concrete firing opportunity. Later shots are
            // reobserved rather than repeated from a stale target and gun state.
            if start >= stop {
                continue;
            }
            let launch = start + active_time((0.545 - progress).max(0.) * 32. / 0.24, start, z.chilled) + 1;
            if launch >= stop {
                continue;
            }
            if naturally_killed_before(scene, z, launch) {
                continue;
            }
            if let Some((tx, ty)) = aim(scene, row, extra, launch, x, f) {
                let origin = x + 113.;
                trace(
                    Ball {
                        row,
                        x: origin as i32 as f32,
                        vx: -(origin - tx as f32 - 20.).max(40.) / 120.,
                        height: 0.,
                        vz: (ty - (car.y - 44)) as f32 / 120. - 7.,
                        az: 0.115,
                        age: 0,
                    },
                    launch,
                    shot,
                    &mut out,
                );
                shot += 1;
            }
        }
        out.sort_unstable_by_key(|e| (e.at, e.shot));
        let out: Rc<[Impact]> = out.into();
        self.cache.borrow_mut().insert(key, out.clone());
        out
    }
}
fn active_time(work: f32, start: i32, chilled: i32) -> i32 {
    let slow = (chilled - start).max(0) as f32;
    if work * 2. <= slow {
        (work * 2.).ceil() as i32
    } else {
        (slow + work - slow * 0.5).ceil() as i32
    }
}
fn naturally_killed_before(scene: &Scene, z: Zombie, until: i32) -> bool {
    let mut damage = 0;
    for p in scene
        .plants
        .iter()
        .filter(|p| p.kind == GloomShroom && (p.pos.0 - z.row).abs() <= 1)
    {
        // Credit only the observed burst and next observed scan. Both movement
        // extremes must remain inside this gun: no assumed stationary launcher
        // and no guessed later firing counters.
        for threshold in [126, 98, 70, 42] {
            for (current, at) in [
                (true, p.shooting - threshold),
                (false, p.launch.max(1) + 199 - threshold),
            ] {
                if at <= 0 || at >= until {
                    continue;
                }
                if current && at >= p.launch.max(1) {
                    continue;
                }
                let walking = (at - z.frozen).max(0);
                let slow = walking.min((z.chilled - z.frozen).max(0));
                let left = z.x - max_step(z.kind) * (walking as f32 - slow as f32 * 0.5);
                let r = z.rect_at(left);
                if z.rect.left < p.attack.right && r.right > p.attack.left {
                    damage += 20;
                }
            }
        }
    }
    damage >= z.hp + z.armor
}
fn aim(scene: &Scene, row: i32, extra: Option<Pad>, at: i32, x: f32, f: &Forecast) -> Option<(i32, i32)> {
    let body = f
        .events
        .iter()
        .find(|e| e.pos.0 == row && e.body_at.is_some_and(|t| t <= at));
    let mut result = scene
        .plants
        .iter()
        .filter(|p| {
            p.pos.0 == row
                && p.hp >= 0
                && !body.is_some_and(|e| e.pos == p.pos && p.kind == PuffShroom)
                && !matches!(p.kind, LilyPad | FlowerPot)
                && !(p.kind == Squash && p.state >= 5)
                && !(matches!(
                    p.kind,
                    IceShroom | Imitator | CherryBomb | DoomShroom | Jalapeno | Blover
                ) && p.effect <= at)
                && extra.is_none_or(|q| !q.replaces_occupant || q.pos != p.pos)
        })
        .filter_map(|p| {
            let px = if p.kind == PuffShroom {
                p.rect.left - 10
            } else {
                p.pos.1 * 80 - 40
            };
            (x >= (px + 100) as f32).then_some((px, p.rect.top))
        })
        .min_by_key(|p| p.0);
    if let Some(pad) = extra.filter(|p| {
        p.pos.0 == row
            && p.plant_at <= at
            && p.lifetime > at
            && !body.is_some_and(|e| e.pos == p.pos && p.kind == PuffShroom)
    }) {
        let px = pad.pos.1 * 80 - 40 + pad.offset_x;
        if x >= (px + 100) as f32 && result.is_none_or(|p| px < p.0) {
            result = Some((px, 80 + (row - 1) * 85));
        }
    }
    if let Some(e) = body.filter(|e| e.at > at) {
        let px = e.pos.1 * 80 - 40;
        if x >= (px + 100) as f32 && result.is_none_or(|p| px < p.0) {
            result = Some((px, 80 + (row - 1) * 85));
        }
    }
    result
}
fn trace(mut ball: Ball, launch: i32, shot: usize, out: &mut Vec<Impact>) {
    // A basketball is collision-active for only a few landing frames. This
    // short trajectory is shared by every target prediction at this location.
    for t in 1..=250 {
        ball.vz += ball.az;
        ball.height += ball.vz;
        if ball.vz >= 0. && (ball.age + t <= 20 || ball.height > if matches!(ball.row, 3 | 4) { 100. } else { 60. }) {
            out.push(Impact {
                at: launch + t,
                shot,
                left: ball.x as i32,
                right: ball.x as i32 + 40,
            });
            if ball.height > 80. {
                break;
            }
        }
        ball.x += ball.vx;
        if ball.x + 40. < 0. {
            break;
        }
    }
}
pub(super) fn hit(left: i32, right: i32, rect: ContactRect) -> bool {
    right.min(rect.right) - left.max(rect.left) > 8
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
    #[test]
    fn basketball_and_bites_consume_the_same_plant_hp() {
        let giant = Zombie {
            age: 0,
            id: ZombieId::from_raw(65537),
            kind: Z::GigaGargantuar,
            row: 1,
            wave: 1,
            carried: false,
            x: 400.,
            hp: 6000,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.23,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(383, 12, 508, 166),
        };
        let foot = Zombie {
            id: ZombieId::from_raw(65538),
            kind: Z::Football,
            x: 100.,
            hp: 270,
            armor: 1400,
            speed: 0.68,
            rect: ContactRect::new(150, 50, 207, 165),
            ..giant
        };
        let car = Zombie {
            id: ZombieId::from_raw(65539),
            kind: Z::Catapult,
            x: 500.,
            hp: 850,
            phase: ZombiePhase::CatapultLaunching,
            rect: ContactRect::new(500, 37, 653, 177),
            ..giant
        };
        let mut scene = Scene::fixture(vec![giant, foot, car]);
        let pad = Pad::new((1, 2), PuffShroom);
        let f = Forecast::default();
        let bites = simulate_until(giant, &scene, &f, Some(pad), 400);
        scene.projectiles.cars.push(Catapult {
            z: car,
            count: 300,
            y: 50,
            known_age: 0,
            previous: 0.,
        });
        let both = simulate_until(giant, &scene, &f, Some(pad), 400);
        assert_eq!(both.trigger, NEVER);
        assert!(
            (bites.impact - both.impact - 75).abs() <= 4,
            "{} -> {}",
            bites.impact,
            both.impact
        );
        scene.zombies[2].phase = ZombiePhase::ZombieNormal;
        scene.zombies[2].x = 170.;
        scene.zombies[2].frozen = 200;
        scene.zombies[2].chilled = 1900;
        assert!(external_destruction_bound(&scene, giant, pad) >= 200);
        scene.plants.push(Plant {
            id: PlantId::from_raw(65537),
            kind: GloomShroom,
            pos: (2, 6),
            hp: 300,
            rect: ContactRect::new(450, 165, 510, 245),
            attack: ContactRect::new(360, 85, 600, 325),
            shooting: 150,
            launch: 50,
            state: 0,
            phase_left: 0,
            effect: 0,
        });
        let weak = Zombie { hp: 40, ..car };
        assert!(
            !naturally_killed_before(&scene, weak, 100),
            "a new scan replaces the old shooting counter"
        );
        assert!(naturally_killed_before(&scene, weak, 160));
    }
    #[test]
    fn basketball_zero_hp_survival_and_fifth_hit_match_native() {
        for cars in [4, 5] {
            let mut world = PeWorldOwner::new_reset(PeWorldConfig {
                scene: pe_rs::SceneType::Fog,
                initial_sun: 8000,
                ..Default::default()
            })
            .unwrap()
            .install_current()
            .unwrap();
            let (puff, giant) = world.with_backend(|b| {
                b.set_zombie_spawn_stopped(true).unwrap();
                b.select_card(sel(PuffShroom).checked().unwrap()).unwrap();
                b.finish_card_selection().unwrap();
                let giant = b.place_zombie(Z::GigaGargantuar, grid((1, 8))).unwrap();
                b.set_zombie_x(giant, rsvz::core::model::I32RepresentableF32::new(600.).unwrap())
                    .unwrap();
                let id = b.zombie_id(giant);
                for _ in 0..cars {
                    let z = b.place_zombie(Z::Catapult, grid((1, 6))).unwrap();
                    b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(500.).unwrap())
                        .unwrap();
                }
                let p = b.new_plant(sel(PuffShroom).checked().unwrap(), grid((1, 2))).unwrap();
                (b.plant_id(p), id)
            });
            world.update_world().unwrap();
            let expected = world.with_backend(|b| {
                PeBackend::scope_backend(b, || {
                    let mut scene = Scene::default();
                    scene.sample(1, -99999);
                    let p = *scene.plants.iter().find(|p| p.id == puff).unwrap();
                    let mut pad = Pad::new((1, 2), PuffShroom);
                    pad.offset_x = p.rect.left - 130;
                    pad.order = p.id.index();
                    pad.replaces_occupant = true;
                    let z = *scene.zombies.iter().find(|z| z.id == giant).unwrap();
                    let f = Forecast::default();
                    let out = simulate_until(z, &scene, &f, Some(pad), 400);
                    assert_eq!(out.trigger, NEVER);
                    if cars == 4 {
                        assert_eq!(out.impact, NEVER);
                    } else {
                        assert!(out.impact < 300, "predicted impact {}", out.impact);
                    }
                    let a = scene.projectiles.events(&scene, 1, Some(pad), &f);
                    let b = scene.projectiles.events(&scene, 1, Some(pad), &f);
                    assert!(Rc::ptr_eq(&a, &b));
                    out.impact
                })
            });
            let mut actual = None;
            for dt in 1..=400 {
                world.update_world().unwrap();
                let alive = world.with_backend(|b| {
                    b.plants()
                        .unwrap()
                        .any(|p| b.plant_id(p) == puff && b.plant_is_alive(p))
                });
                if !alive {
                    actual = Some(dt);
                    break;
                }
            }
            if cars == 4 {
                assert_eq!(actual, None);
                world.with_backend(|b| {
                    let p = b.plants().unwrap().find(|p| b.plant_id(*p) == puff).unwrap();
                    assert_eq!(b.plant_hp(p), 0);
                });
            } else {
                let actual = actual.expect("native basketball death");
                assert!((actual - expected).abs() <= 4, "actual {actual}, expected {expected}");
            }
        }
    }
}
