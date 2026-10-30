//! Short squash projection: all observable lock ties, then a linear guaranteed-hit scan.
use super::*;
use prediction::{EffectKind, Forecast, Pad, Scene, Zombie};
use rsvz::core::model::ZombieId;

const LAUNCH: i32 = 126;
const IMPACT: i32 = 182;
#[derive(Clone, Copy)]
pub struct Live {
    pub pos: Pos,
    pub state: i32,
    pub count: i32,
    pub target: u32,
    pub x: i32,
}
impl Live {
    fn timing(self) -> Option<(i32, i32)> {
        match self.state {
            3 => Some((self.count + 45, self.count + 100)),
            4 => Some((self.count, self.count + 55)),
            5 => Some((0, self.count + 5)),
            6 if self.count > 5 => Some((0, self.count - 5)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Landing {
    pub left: f32,
    pub right: f32,
    pub launch: i32,
}
#[cfg(test)]
mod committed_tests {
    use super::*;
    #[test]
    fn takeoff_next_frame_uses_current_target_and_decapitation_boundary() {
        let z = Zombie {
            id: ZombieId::from_raw(65537),
            kind: Z::Football,
            row: 6,
            wave: 1,
            carried: false,
            x: 300.,
            hp: 90,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.68,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(350, 0, 407, 115),
        };
        let (lift, _) = project(
            z,
            &Scene::default(),
            &Forecast::default(),
            (6, 5),
            false,
            0,
            1,
            56,
            None,
        );
        assert_eq!(lift.hp, 90);
        assert_eq!(lift.left, 300.);
        assert!(lift.aim_left > 0.);
        assert!(alive_hp(z, 90));
        assert!(!alive_hp(z, 89));
        assert!(!alive_hp(z, 60));
    }

    use rsvz::core::model::ContactRect;
    #[test]
    fn observed_phase_deadlines_match_native_landing() {
        use rsvz::core::backend::*;
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            initial_sun: 8000,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        let id = world.with_backend(|b| {
            b.set_zombie_spawn_stopped(true).unwrap();
            b.select_card(sel(Squash).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            let z = b.place_zombie(Z::GigaGargantuar, grid((5, 6))).unwrap();
            b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(420.).unwrap())
                .unwrap();
            let id = b.zombie_id(z);
            b.new_plant(sel(Squash).checked().unwrap(), grid((5, 5))).unwrap();
            id
        });
        let mut deadlines = Vec::new();
        let mut hit = None;
        for frame in 1..=200 {
            world.update_world().unwrap();
            world.with_backend(|b| {
                let hp = b
                    .zombies()
                    .unwrap()
                    .into_iter()
                    .find(|&z| b.zombie_id(z) == id)
                    .map(|z| b.zombie_hp(z))
                    .unwrap();
                if hp < 6000 && hit.is_none() {
                    hit = Some(frame);
                }
                if [20, 100, 125, 140, 175].contains(&frame) {
                    PeBackend::scope_backend(b, || {
                        let mut scene = Scene::default();
                        scene.sample(frame, -99999);
                        let mut f = Forecast::default();
                        add_committed(&scene, &mut f, &mut Predictor::default());
                        let e = f.events.first().expect("live squash must remain in shared forecast");
                        deadlines.push(frame + e.at);
                        let z = *scene.zombies.iter().find(|z| z.id == id).unwrap();
                        assert!(e.hits(z, z.x));
                    });
                }
            });
        }
        let actual = hit.expect("native landing");
        assert_eq!(deadlines.len(), 5);
        assert!(
            deadlines.iter().all(|&at| (at - actual).abs() <= 1),
            "{deadlines:?}, actual={actual}"
        );
    }
    #[test]
    fn live_squash_damage_is_shared_and_geometry_bounded() {
        let target = Zombie {
            id: ZombieId::from_raw(65537),
            kind: Z::GigaGargantuar,
            row: 5,
            wave: 1,
            carried: false,
            x: 420.,
            hp: 6000,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.3,
            frozen: 500,
            chilled: 2000,
            object: false,
            eating: false,
            rect: ContactRect::new(403, 390, 528, 544),
        };
        let remote = Zombie {
            id: ZombieId::from_raw(65538),
            x: 800.,
            rect: ContactRect::new(783, 390, 908, 544),
            ..target
        };
        let mut scene = Scene::fixture(vec![target, remote]);
        scene.squashes.push(Live {
            pos: (5, 5),
            state: 3,
            count: 20,
            target: target.id.raw(),
            x: 425,
        });
        let mut predictor = Predictor::default();
        let mut f = Forecast::default();
        add_committed(&scene, &mut f, &mut predictor);
        assert_eq!(f.events.len(), 1);
        assert_eq!(f.events[0].at, 120);
        assert!(
            with_intervention(&scene, &f, Pad::new((5, 6), PuffShroom)).is_none(),
            "frozen actors cannot change aim before takeoff"
        );
        assert_eq!(
            prediction::simulate_until(target, &scene, &f, None, 120).hp_at_end,
            4200
        );
        assert_eq!(
            prediction::simulate_until(remote, &scene, &f, None, 120).hp_at_end,
            6000
        );
        let mut shifted = f.events[0];
        shifted.landing.as_mut().unwrap().left += 300.;
        shifted.landing.as_mut().unwrap().right += 300.;
        assert!(!shifted.hits(target, target.x));
        scene.squashes[0].state = 5;
        scene.squashes[0].count = 30;
        let mut airborne = Forecast::default();
        add_committed(&scene, &mut airborne, &mut predictor);
        assert_eq!(airborne.events[0].at, 35);
        assert_eq!(airborne.events[0].landing.unwrap().launch, 0);
        scene.squashes[0].state = 6;
        scene.squashes[0].count = 5;
        let mut landed = Forecast::default();
        add_committed(&scene, &mut landed, &mut predictor);
        assert!(
            landed.events.is_empty(),
            "do not apply the same1800 again after landing"
        );
    }
}
pub fn add_committed(scene: &Scene, f: &mut Forecast, predictor: &mut Predictor) {
    if scene.squashes.is_empty() {
        return;
    }
    for &live in &scene.squashes {
        let Some((launch, at)) = live.timing() else { continue };
        if at <= 0 {
            continue;
        }
        let landing = if launch <= 0 {
            Landing {
                left: live.x as f32,
                right: live.x as f32,
                launch: 0,
            }
        } else {
            let Some(choice) = predictor.evaluate_after(scene, f, live.pos, &[], false, 0, Some(live), None) else {
                continue;
            };
            Landing {
                left: choice.left,
                right: choice.right,
                launch,
            }
        };
        f.events.push(prediction::Effect {
            at,
            kind: EffectKind::Squash,
            pos: live.pos,
            certain: true,
            shape: None,
            body_at: None,
            landing: Some(landing),
        });
    }
}
pub(super) fn with_intervention(scene: &Scene, f: &Forecast, pad: Pad) -> Option<Forecast> {
    let affected = |e: &prediction::Effect| {
        e.landing.is_some_and(|l| {
            l.launch > pad.plant_at
                && e.pos.0 == pad.pos.0
                && scene.row_zombies(pad.pos.0).any(|z| {
                    let moving = (l.launch - 1 - z.frozen).max(0);
                    moving > 0
                        && z.x - prediction::max_step(z.kind) * moving as f32 + prediction::attack_offsets(*z).0 as f32
                            <= pad.rect().right as f32
                        && prediction::could_contact_pad(*z, pad)
                })
        })
    };
    if !f.events.iter().any(affected) {
        return None;
    }
    let mut adjusted = Forecast {
        events: f.events.clone(),
    };
    for i in 0..adjusted.events.len() {
        if !affected(&f.events[i]) {
            continue;
        }
        let Some(live) = scene.squashes.iter().find(|p| p.pos == f.events[i].pos).copied() else {
            continue;
        };
        let without = Forecast {
            events: f
                .events
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, e)| *e)
                .collect(),
        };
        if let Some(choice) =
            Predictor::default().evaluate_after(scene, &without, live.pos, &[], false, 0, Some(live), Some(pad))
        {
            let landing = adjusted.events[i].landing.as_mut().unwrap();
            landing.left = choice.left;
            landing.right = choice.right;
        }
    }
    Some(adjusted)
}
#[derive(Clone, Copy, Default)]
struct Point {
    left: f32,
    right: f32,
    hp: i32,
    may_die: bool,
    aim_left: f32,
    aim_right: f32,
}
struct Projected {
    zombie: Zombie,
    initial: bool,
    required: bool,
    launch: Point,
    impact: Point,
}
pub struct Predictor {
    projected: Vec<Projected>,
    required: std::collections::HashSet<ZombieId>,
}
impl Default for Predictor {
    fn default() -> Self {
        Self {
            projected: Vec::with_capacity(1024),
            required: std::collections::HashSet::with_capacity(1024),
        }
    }
}
#[derive(Clone, Copy)]
pub struct Choice {
    pub pos: Pos,
    pub locks: i32,
    pub left: f32,
    pub right: f32,
    pub guaranteed: i32,
    pub wait: i32,
}
pub(super) fn targetable(z: Zombie) -> bool {
    !matches!(
        z.phase,
        ZombiePhase::JackInTheBoxPopping
            | ZombiePhase::BalloonFlying
            | ZombiePhase::BalloonPopping
            | ZombiePhase::DiggerTunneling
            | ZombiePhase::DiggerRising
            | ZombiePhase::ImpGettingThrown
    ) && !matches!(z.kind, Z::Bungee)
}
fn alive_hp(z: Zombie, hp: i32) -> bool {
    hp > 0 && (z.giant() || matches!(z.kind, Z::Zomboni | Z::Catapult) || hp >= 90)
}
fn overlap(z: Zombie, x: f32, px: f32) -> f32 {
    let r = z.rect_at(x);
    (px + 65.).min(r.right as f32) - (px + 20.).max(r.left as f32)
}
fn eligible(z: Zombie, x: f32, px: f32) -> bool {
    let r = z.rect_at(x);
    let extra = if matches!(z.kind, Z::Imp | Z::Football) {
        60.
    } else {
        0.
    };
    r.left <= 800 && overlap(z, x, px) >= if z.eating { -110. } else { -70. } && r.right as f32 >= px + 20. - extra
}
/// Necessary geometry only: bound every possible launch target without advancing frames.
pub fn possible_landing(scene: &Scene, pos: Pos) -> Option<(f32, f32)> {
    let px = (pos.1 * 80 - 40) as f32;
    let mut initial = false;
    let mut left = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    for z in &scene.zombies {
        if z.row != pos.0 || !targetable(*z) {
            continue;
        }
        let starts = eligible(*z, z.x, px);
        initial |= starts;
        let reach = prediction::max_step(z.kind) * LAUNCH as f32;
        let point = Point {
            left: z.x - reach,
            right: z.x + if z.kind == Z::Digger { reach } else { 0. },
            ..Point::default()
        };
        if starts || possible_eligible(*z, point, px) {
            let center = (z.rect.left + z.rect.right) as f32 * 0.5 - z.x - 40.;
            let lead = prediction::max_speed(z.kind) * 30.;
            left = left.min(point.left + center - lead - 2.);
            right = right.max(point.right + center + if z.kind == Z::Digger { lead } else { 0. } + 2.);
        }
    }
    initial.then_some((left, right))
}
pub fn could_hit(victim: Zombie, (left, right): (f32, f32)) -> bool {
    let near = victim
        .rect_at(victim.x - prediction::max_step(victim.kind) * IMPACT as f32)
        .left as f32;
    let far = victim.rect.right as f32;
    let margin = if victim.kind == Z::Football { 20. } else { 0. };
    right + 65. + margin > near && left + 20. - margin < far
}
fn possible_eligible(z: Zombie, p: Point, px: f32) -> bool {
    let aligned = (px + 20. - (z.rect.left as f32 - z.x)).clamp(p.left, p.right);
    eligible(z, p.left, px) || eligible(z, p.right, px) || eligible(z, aligned, px)
}
fn overlap_bounds(z: Zombie, p: Point, px: f32) -> (f32, f32) {
    let a = overlap(z, p.left, px);
    let b = overlap(z, p.right, px);
    let aligned = (px + 20. - (z.rect.left as f32 - z.x)).clamp(p.left, p.right);
    (a.min(b), a.max(b).max(overlap(z, aligned, px)))
}
fn project(
    z: Zombie, scene: &Scene, forecast: &Forecast, pos: Pos, replacing_jalapeno: bool, wait: i32, launch_at: i32,
    impact_at: i32, intervention: Option<Pad>,
) -> (Point, Point) {
    let mut x = z.x;
    let mut progress = z.progress;
    let mut speed = z.speed;
    let mut freeze = z.frozen;
    let mut slow = z.chilled;
    let mut action = if z.phase == ZombiePhase::GargantuarSmashing {
        1
    } else if z.phase == ZombiePhase::GargantuarThrowing {
        2
    } else {
        0
    };
    let mut action_progress = if action == 0 { 0. } else { progress };
    let mut object = z.object;
    let mut hp = z.hp + z.armor;
    let mut was_blocked = z.eating;
    let mut uncertain = matches!(z.kind, Z::Catapult | Z::Digger) || z.phase == ZombiePhase::ImpLanding;
    let mut left = x;
    let mut right = x;
    let mut shot = [0; 8];
    let mut launch = [200; 8];
    let mut guns = [None; 8];
    let mut n = 0;
    for p in &scene.plants {
        if p.kind == GloomShroom && (p.pos.0 - z.row).abs() <= 1 && n < 8 {
            guns[n] = Some(*p);
            shot[n] = p.shooting;
            launch[n] = p.launch.max(1);
            n += 1;
        }
    }
    let mut possible_damage = 0;
    let mut saw_gun = [false; 8];
    let center = (z.rect.left + z.rect.right) as f32 * 0.5 - z.x - 40.;
    let lead = if z.frozen > 0 || z.eating || action != 0 {
        0.
    } else {
        z.speed * 30.
    };
    let mut lift = if launch_at == 1 {
        Point {
            left: x,
            right: x,
            hp,
            may_die: false,
            aim_left: (x + center - lead).floor() - 1.,
            aim_right: (x + center + if z.kind == Z::Digger { lead } else { 0. }).ceil() + 1.,
        }
    } else {
        Point::default()
    };
    let mut impact = Point::default();
    let pad = Pad::new(pos, Squash);
    for dt in 1..=impact_at {
        for e in &forecast.events {
            if e.at != dt + wait
                || replacing_jalapeno && e.kind == EffectKind::Jalapeno && e.pos.0 == pos.0 && !e.certain
            {
                continue;
            }
            if e.kind == EffectKind::Ice {
                if z.kind == Z::Zomboni {
                    continue;
                }
                if z.phase == ZombiePhase::ImpLanding {
                    // Landing imps can be chilled but cannot be frozen until landing finishes.
                    // Keep the movable/stopped envelope rather than inventing a hidden finish time.
                    slow = 2000;
                    possible_damage += 20;
                    continue;
                }
                freeze = if slow > 0 { 300 } else { 400 };
                slow = 2000;
                hp -= 20;
                possible_damage += 20;
            } else {
                if e.kind == EffectKind::Jalapeno && e.pos.0 == z.row && freeze > 0 {
                    uncertain = true;
                }
                let hits = |at| e.hits(z, at);
                if hits(left) && hits(right) {
                    hp -= 1800;
                }
                if hits(left) || hits(right) || hits((left + right) * 0.5) {
                    possible_damage += 1800;
                }
                if e.kind == EffectKind::Jalapeno && e.pos.0 == z.row {
                    freeze = 0;
                    slow = 0;
                }
            }
        }
        for i in 0..n {
            let gun = guns[i].unwrap();
            let in_range = |at| {
                let r = z.rect_at(at);
                gun.attack.right.min(r.right) > gun.attack.left.max(r.left)
            };
            if !saw_gun[i] && (in_range(left) || in_range(right)) {
                saw_gun[i] = true;
                // Two bursts bound possible early-phase damage; used only to retain relock branches.
                possible_damage += 160;
            }
            launch[i] -= 1;
            if launch[i] <= 0 {
                launch[i] = 200;
                if in_range(left) && in_range(right) {
                    shot[i] = 200;
                }
            }
            if shot[i] > 0 {
                shot[i] -= 1;
                if matches!(shot[i], 126 | 98 | 70 | 42) && in_range(left) && in_range(right) {
                    hp -= 20;
                }
            }
        }
        let was_frozen = freeze > 0;
        freeze = (freeze - 1).max(0);
        slow = (slow - 1).max(0);
        if was_frozen && freeze == 0 {
            uncertain = true;
        }
        // A triggered Squash is chewable but takes no bite/smash damage until
        // takeoff. Contact with its real rectangle therefore proves a stop.
        let planned_block = forecast.events.iter().any(|e| {
            e.body_at.is_some_and(|at| at <= dt + wait)
                && e.at > dt + wait
                && e.pos.0 == z.row
                && prediction::contact(z, left, Pad::new(e.pos, IceShroom).rect())
                && prediction::contact(z, right, Pad::new(e.pos, IceShroom).rect())
        });
        let blocked = planned_block
            || dt < launch_at && prediction::contact(z, left, pad.rect()) && prediction::contact(z, right, pad.rect());
        let mut stopped = freeze > 0;
        if freeze == 0 {
            if z.kind == Z::Zomboni {
                prediction::zomboni_step(&mut x, &mut speed);
                left = x;
                right = x;
            } else if z.giant() && action != 0 {
                stopped = true;
                let recovered = action_progress >= 1.;
                action_progress += if action == 1 { 0.16 / 33. } else { 0.24 / 34. } * if slow > 0 { 0.5 } else { 1. };
                if action == 2 && action_progress >= 0.74 {
                    object = false;
                }
                if recovered {
                    action = 0;
                    progress = 0.;
                    uncertain = true;
                }
            } else if z.giant() && object && hp < if z.kind == Z::GigaGargantuar { 3000 } else { 1500 } && left > 400. {
                action = 2;
                action_progress = 0.;
                stopped = true;
            } else if blocked {
                stopped = true;
                if z.giant() {
                    action = 1;
                    action_progress = 0.;
                }
                was_blocked = true;
            } else {
                // Releasing an eating actor redraws its walk. Preserve that
                // uncertainty, rather than extending the eating stop into flight.
                if was_blocked {
                    uncertain = true;
                    was_blocked = false;
                }
                let other_block = intervention.is_some_and(|p| {
                    p.pos.0 == z.row
                        && dt + wait >= p.plant_at
                        && dt + wait < p.lifetime
                        && prediction::contact(z, left, p.rect())
                }) || scene.plants.iter().any(|p| {
                    p.pos.0 == z.row
                        && !matches!(p.kind, LilyPad | FlowerPot)
                        && !(p.kind == Squash && p.state >= 5)
                        && prediction::contact(z, left, p.rect)
                });
                if other_block {
                    uncertain = true;
                }
                if uncertain {
                    // A possible contact can only stop this endpoint; it cannot
                    // walk through the new, immune Squash before takeoff.
                    if !(dt < launch_at && prediction::contact(z, left, pad.rect())) {
                        left -= prediction::max_step(z.kind) * if slow > 0 { 0.5 } else { 1. };
                    }
                    if z.kind == Z::Digger {
                        right += prediction::max_step(z.kind);
                    }
                } else {
                    prediction::walk_kind(&mut x, &mut progress, speed, slow > 0, z.kind);
                    left = x;
                    right = x;
                }
            }
        }
        if dt == launch_at - 1 || (impact_at - 2..=impact_at).contains(&dt) {
            let center = (z.rect.left + z.rect.right) as f32 * 0.5 - z.x - 40.;
            let point = Point {
                left,
                right,
                hp,
                may_die: !alive_hp(z, z.hp + z.armor - possible_damage),
                // Native aim has a 30cs velocity lead, zero while stopped. Preserve both
                // around a contact/recovery boundary; never substitute exact future position.
                aim_left: (left + center
                    - if stopped {
                        0.
                    } else {
                        (if z.kind == Z::Zomboni {
                            speed
                        } else {
                            prediction::max_speed(z.kind)
                        }) * 30.
                    })
                .floor()
                    - 1.,
                aim_right: (right
                    + center
                    + if z.kind == Z::Digger {
                        prediction::max_speed(z.kind) * 30.
                    } else {
                        0.
                    })
                .ceil()
                    + 1.,
            };
            if dt == launch_at - 1 {
                // Plants aim before zombies update on the takeoff frame.
                lift = point;
            }
            if dt >= impact_at - 2 {
                if dt == (impact_at - 2).max(1) {
                    impact = point;
                } else {
                    impact.left = impact.left.min(point.left);
                    impact.right = impact.right.max(point.right);
                    impact.hp = impact.hp.max(point.hp);
                }
                if dt == impact_at {
                    return (lift, impact);
                }
            }
        }
    }
    unreachable!()
}
fn advance_quiet(z: &mut Zombie) {
    let rect = z.rect;
    let old = z.x as i32;
    z.chilled = (z.chilled - 1).max(0);
    if z.kind == Z::Zomboni {
        prediction::zomboni_step(&mut z.x, &mut z.speed);
    } else {
        prediction::walk_kind(&mut z.x, &mut z.progress, z.speed, z.chilled > 0, z.kind);
    }
    let dx = z.x as i32 - old;
    z.rect = rsvz::core::model::ContactRect::new(rect.left + dx, rect.top, rect.right + dx, rect.bottom);
}
fn after_quiet(mut z: Zombie, wait: i32) -> Zombie {
    for _ in 0..wait {
        advance_quiet(&mut z);
    }
    z
}
fn waiting_relevant(z: Zombie, px: f32, horizon: i32, required: &[ZombieId]) -> bool {
    if required.contains(&z.id) {
        return true;
    }
    // Blown-away flyers do not become ground lock stealers in this lineup.
    if z.kind == Z::Balloon && z.phase == ZombiePhase::BalloonFlying {
        return false;
    }
    if matches!(z.kind, Z::Normal | Z::Conehead | Z::Buckethead | Z::Football)
        || z.giant() && (!z.object || z.frozen >= horizon)
    {
        let reach = prediction::max_step(z.kind) * (horizon - z.frozen.min(horizon)) as f32;
        return possible_eligible(
            z,
            Point {
                left: z.x - reach,
                right: z.x,
                ..Point::default()
            },
            px,
        );
    }
    true
}
impl Predictor {
    pub fn evaluate(
        &mut self, scene: &Scene, forecast: &Forecast, pos: Pos, required: &[ZombieId], replacing_jalapeno: bool,
    ) -> Option<Choice> {
        self.evaluate_after(scene, forecast, pos, required, replacing_jalapeno, 0, None, None)
    }
    /// One bounded waiting trajectory, not a search over future planting times.
    pub fn prestore(&mut self, scene: &Scene, forecast: &Forecast, pos: Pos, required: &[ZombieId]) -> Option<Choice> {
        let px = (pos.1 * 80 - 40) as f32;
        let horizon = forecast.events.iter().map(|e| e.at - 1).min().unwrap_or(500).min(500);
        if horizon <= 0 {
            return None;
        }
        let mut wait = horizon + 1;
        for &z in scene.row_zombies(pos.0) {
            if !waiting_relevant(z, px, horizon, required) {
                continue;
            }
            // Do not forecast a hidden redraw, a new action, or a special movement mode.
            if z.phase != ZombiePhase::ZombieNormal
                || z.frozen > 0
                || z.eating
                || !matches!(z.kind, Z::Gargantuar | Z::GigaGargantuar | Z::Football | Z::Zomboni)
                || z.giant() && z.object && z.hp < if z.kind == Z::GigaGargantuar { 3000 } else { 1500 }
            {
                return None;
            }
            if eligible(z, z.x, px) {
                return None;
            }
            let mut future = z;
            for dt in 1..=horizon {
                advance_quiet(&mut future);
                if eligible(future, future.x, px) {
                    wait = wait.min(dt);
                    break;
                }
            }
        }
        if wait > horizon {
            return None;
        }
        // Existing plants/damage could interrupt the waiting trajectory. Such cases
        // keep using the normal observed-state predictor on subsequent ticks.
        for &z in scene.row_zombies(pos.0) {
            if !waiting_relevant(z, px, horizon, required) {
                continue;
            }
            let future = after_quiet(z, wait);
            if prediction::external_destruction_bound(scene, z, Pad::new(pos, Squash)) <= wait + IMPACT {
                return None;
            }
            for p in &scene.plants {
                if p.pos.0 == z.row
                    && !matches!(p.kind, LilyPad | FlowerPot)
                    && future.x - 30. < p.rect.right as f32
                    && z.x + 70. > p.rect.left as f32
                {
                    return None;
                }
                if p.kind == GloomShroom
                    && (p.pos.0 - z.row).abs() <= 1
                    && future.rect.left < p.attack.right
                    && z.rect.right > p.attack.left
                {
                    return None;
                }
            }
        }
        telemetry::work(scene.row_zombies(pos.0).count() as i32 * (horizon + wait));
        self.evaluate_after(scene, forecast, pos, required, false, wait, None, None)
    }
    fn evaluate_after(
        &mut self, scene: &Scene, forecast: &Forecast, pos: Pos, required: &[ZombieId], replacing_jalapeno: bool,
        wait: i32, live: Option<Live>, intervention: Option<Pad>,
    ) -> Option<Choice> {
        let (launch_at, impact_at) = live.map_or((LAUNCH, IMPACT), |p| p.timing().unwrap());
        let px = (pos.1 * 80 - 40) as f32;
        let mut maximum = f32::NEG_INFINITY;
        for original in &scene.zombies {
            if original.row != pos.0 {
                continue;
            }
            let shifted = after_quiet(*original, wait);
            let z = &shifted;
            if z.row == pos.0 && targetable(*z) && eligible(*z, z.x, px) {
                maximum = maximum.max(overlap(*z, z.x, px));
            }
        }
        if !maximum.is_finite() && live.is_none() {
            return None;
        }
        self.projected.clear();
        self.required.clear();
        self.required.extend(required.iter().copied());
        for original in &scene.zombies {
            if original.row != pos.0 {
                continue;
            }
            let shifted = after_quiet(*original, wait);
            let z = &shifted;
            if z.row != pos.0 || !targetable(*z) {
                continue;
            }
            let initial =
                eligible(*z, z.x, px) && live.map_or(overlap(*z, z.x, px) == maximum, |p| z.id.raw() == p.target);
            // Keep potential lock stealers and requested victims; distant actors cannot enter in 126cs.
            if !initial && !self.required.contains(&z.id) && (z.x - px).abs() > 400. {
                continue;
            }
            let (launch, impact) = project(
                *z,
                scene,
                forecast,
                pos,
                replacing_jalapeno,
                wait,
                launch_at,
                impact_at,
                intervention,
            );
            self.projected.push(Projected {
                zombie: *z,
                initial,
                required: self.required.contains(&z.id),
                launch,
                impact,
            });
        }
        telemetry::work(self.projected.len() as i32 * IMPACT);
        let mut relock = live.is_some() && !self.projected.iter().any(|p| p.initial);
        let mut best_min = f32::NEG_INFINITY;
        for p in &self.projected {
            let possible = alive_hp(p.zombie, p.launch.hp) && possible_eligible(p.zombie, p.launch, px);
            let definite = possible
                && !p.launch.may_die
                && eligible(p.zombie, p.launch.left, px)
                && eligible(p.zombie, p.launch.right, px);
            if p.initial && !definite {
                relock = true;
            }
            if definite {
                best_min = best_min.max(overlap_bounds(p.zombie, p.launch, px).0);
            }
        }
        let mut left = f32::INFINITY;
        let mut right = f32::NEG_INFINITY;
        let mut locks = 0;
        for p in &self.projected {
            let possible = alive_hp(p.zombie, p.launch.hp) && possible_eligible(p.zombie, p.launch, px);
            if possible && (p.initial || relock && overlap_bounds(p.zombie, p.launch, px).1 >= best_min) {
                left = left.min(p.launch.aim_left);
                right = right.max(p.launch.aim_right);
                locks += 1;
            }
            if relock && !best_min.is_finite() && p.initial {
                // If no legal target survives the second scan, native retains the first target position.
                let first = (p.zombie.rect.left + (p.zombie.rect.right - p.zombie.rect.left) / 2 - 40) as f32;
                left = left.min(first - 1.);
                right = right.max(first + 1.);
                locks += 1;
            }
        }
        if let Some(p) = live {
            if launch_at == 0 || locks == 0 {
                left = p.x as f32;
                right = left;
                locks = 1;
            }
        }
        if locks == 0 {
            return None;
        }
        let mut guaranteed = 0;
        for p in self.projected.iter().filter(|p| p.required) {
            if !alive_hp(p.zombie, p.impact.hp) {
                guaranteed += 1;
                continue;
            }
            let margin = if p.zombie.kind == Z::Football { -20 } else { 0 };
            let lo = p.zombie.rect_at(p.impact.right).left + margin - 65;
            let hi = p.zombie.rect_at(p.impact.left).right - margin - 20;
            if p.zombie.rect_at(p.impact.right).left > 800 || left <= lo as f32 || right >= hi as f32 {
                return None;
            }
            guaranteed += 1;
        }
        if guaranteed as usize != self.required.len() {
            return None;
        }
        Some(Choice {
            pos,
            locks,
            left,
            right,
            guaranteed,
            wait,
        })
    }
    /// Reuse the successful choice's landing envelope for an additional actor.
    /// The caller must query before evaluating another candidate.
    pub fn guarantees_hit(&self, choice: Choice, id: ZombieId) -> bool {
        self.projected.iter().find(|p| p.zombie.id == id).is_some_and(|p| {
            let margin = if p.zombie.kind == Z::Football { -20 } else { 0 };
            let lo = p.zombie.rect_at(p.impact.right).left + margin - 65;
            let hi = p.zombie.rect_at(p.impact.left).right - margin - 20;
            p.impact.hp > 0
                && p.zombie.rect_at(p.impact.right).left <= 800
                && choice.left > lo as f32
                && choice.right < hi as f32
        })
    }
    /// Ordinary pre-2300 interventions must not spend current-wave health.
    /// Use the union of possible landings/victim positions, not just the lock
    /// target or number of victims. No hidden refresh threshold is inspected.
    pub fn preserves_early_refresh(&self, choice: Choice, wave: i32, wave_time: i32) -> bool {
        // The impact envelope includes the two preceding update boundaries.
        if wave == 20 || wave_time + choice.wait + IMPACT - 2 >= 2300 {
            return true;
        }
        !self.projected.iter().any(|p| {
            if p.zombie.wave != wave || p.zombie.carried {
                return false;
            }
            let margin = if p.zombie.kind == Z::Football { 20. } else { 0. };
            let left = p.zombie.rect_at(p.impact.left).left as f32;
            let right = p.zombie.rect_at(p.impact.right).right as f32;
            left <= 800. && left - margin < choice.right + 65. && right + margin > choice.left + 20.
        })
    }
    pub fn log_choice(&self) {
        for p in &self.projected {
            telemetry::record(
                "squash_actor",
                &[
                    p.zombie.id.raw() as i64,
                    p.zombie.kind as i64,
                    p.zombie.phase as i64,
                    (p.zombie.x * 100.) as i64,
                    p.zombie.hp as i64,
                    i64::from(p.initial),
                    i64::from(p.required),
                    (p.launch.left * 100.) as i64,
                    (p.launch.right * 100.) as i64,
                    (p.impact.left * 100.) as i64,
                    (p.impact.right * 100.) as i64,
                ],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz::core::model::{ContactRect, ZombiePhase};
    #[test]
    fn early_squash_protects_current_wave_collateral_not_just_required_victim() {
        let mut old = actor(Z::GigaGargantuar, 410.5);
        old.wave = 2;
        old.frozen = 500;
        let current = Zombie {
            id: ZombieId::from_raw(65538),
            wave: 3,
            ..old
        };
        let scene = Scene::fixture(vec![old, current]);
        let mut predictor = Predictor::default();
        let choice = predictor
            .evaluate(&scene, &Forecast::default(), (6, 5), &[old.id], false)
            .unwrap();
        assert!(!predictor.preserves_early_refresh(choice, 3, 2000));
        assert!(!predictor.preserves_early_refresh(choice, 3, 2119));
        assert!(predictor.preserves_early_refresh(choice, 3, 2120));
        assert!(predictor.preserves_early_refresh(choice, 20, 500));
        let scene = Scene::fixture(vec![old]);
        let choice = predictor
            .evaluate(&scene, &Forecast::default(), (6, 5), &[old.id], false)
            .unwrap();
        assert!(
            predictor.preserves_early_refresh(choice, 3, 2000),
            "old target at x>400 remains usable"
        );
    }
    #[test]
    fn offscreen_target_must_enter_damage_boundary_before_locking() {
        let mut z = actor(Z::GigaGargantuar, 818.68);
        z.rect = ContactRect::new(801, 0, 926, 154);
        assert!(!eligible(z, z.x, 680.));
        assert!(eligible(z, 817.96, 680.));
        let mut predictor = Predictor::default();
        let car = actor(Z::Zomboni, 800.);
        z.speed = 0.;
        let scene = Scene::fixture(vec![car, z]);
        assert!(
            predictor
                .evaluate(&scene, &Forecast::default(), (6, 9), &[z.id], false)
                .is_none()
        );
    }
    fn actor(kind: Z, x: f32) -> Zombie {
        Zombie {
            id: ZombieId::from_raw(65537),
            kind,
            row: 6,
            wave: 20,
            carried: false,
            x,
            hp: 6000,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: prediction::max_speed(kind),
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(x as i32 + 36, 0, x as i32 + 78, 115),
        }
    }
    #[test]
    fn football_eats_squash_until_takeoff_then_moves() {
        let z = actor(Z::Football, 100.);
        let (lift, impact) = project(
            z,
            &Scene::default(),
            &Forecast::default(),
            (6, 2),
            false,
            0,
            LAUNCH,
            IMPACT,
            None,
        );
        assert_eq!((lift.left, lift.right), (100., 100.));
        assert!(lift.aim_right - lift.aim_left <= 2.);
        assert!(impact.left < lift.left);
    }
    #[test]
    fn new_hammer_holds_giant_even_after_squash_takeoff() {
        let z = actor(Z::GigaGargantuar, 150.);
        let (lift, impact) = project(
            z,
            &Scene::default(),
            &Forecast::default(),
            (6, 2),
            false,
            0,
            LAUNCH,
            IMPACT,
            None,
        );
        assert_eq!((lift.left, lift.right), (150., 150.));
        assert_eq!((impact.left, impact.right), (150., 150.));
        assert!(lift.aim_right - lift.aim_left <= 2.);
    }
}
