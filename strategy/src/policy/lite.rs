//! Optional coarse action summaries: finite event segments, never frame stepping.
//! These are deliberately approximate features, not safety or legality predicates.
use super::*;
use rsvz::core::contact::circle_hits_rect;
use rsvz::core::model::ZombieTrackProfile;
use rsvz::core::zombie_motion::vanilla_track_frames;

const H: f32 = 1600.;

#[derive(Clone, Copy)]
struct Segment {
    start: f32,
    end: f32,
    left: f32,
    right: f32,
}
#[derive(Clone)]
struct Path {
    segments: Vec<Segment>,
}
impl Path {
    fn x(&self, time: f32) -> f32 {
        let s = self
            .segments
            .iter()
            .find(|s| time <= s.end)
            .unwrap_or_else(|| self.segments.last().unwrap());
        s.left + (s.right - s.left) * ((time - s.start) / (s.end - s.start)).clamp(0., 1.)
    }
    fn windows(&self, lo: f32, hi: f32, until: f32) -> impl Iterator<Item = (f32, f32)> + '_ {
        self.segments.iter().filter_map(move |s| {
            let end = s.end.min(until);
            if end <= s.start {
                return None;
            }
            if (s.right - s.left).abs() < 0.00001 {
                return (s.left >= lo && s.left <= hi).then_some((s.start, end));
            }
            let v = (s.left - s.right) / (s.end - s.start);
            let a = (s.start + (s.left - hi) / v).clamp(s.start, end);
            let b = (s.start + (s.left - lo) / v).clamp(s.start, end);
            (b > a).then_some((a, b))
        })
    }
    fn enter(&self, lo: f32, hi: f32) -> f32 {
        self.windows(lo, hi, H).map(|(a, _)| a).next().unwrap_or(H)
    }
}
fn supported(z: Zombie) -> bool {
    matches!(
        z.kind,
        Z::Normal | Z::Football | Z::Gargantuar | Z::GigaGargantuar | Z::Imp | Z::JackInTheBox
    ) && matches!(z.phase, ZombiePhase::ZombieNormal | ZombiePhase::JackInTheBoxRunning)
        && !z.eating
}
fn mean_speed(z: Zombie, redrawn: bool) -> f32 {
    // Frozen stored speed is never read. Future ice uses the existing maximum
    // compatible redraw parameter (giants 0.37), not its hidden eventual draw.
    let speed = if redrawn || z.frozen > 0 {
        prediction::max_speed(z.kind)
    } else {
        z.speed
    };
    let profile = match z.kind {
        Z::Football => ZombieTrackProfile::FootballWalk,
        Z::Imp => ZombieTrackProfile::ImpWalk,
        Z::JackInTheBox => ZombieTrackProfile::JackBoxWalk,
        Z::Gargantuar | Z::GigaGargantuar => ZombieTrackProfile::GargantuarWalk,
        _ => ZombieTrackProfile::NormalWalkA,
    };
    let n = vanilla_track_frames(profile).len() as f32;
    speed * 0.47 * n / (n - 1.)
}
fn path(z: Zombie, ice: Option<f32>, holds: &[(f32, f32)]) -> Path {
    let freeze = z.frozen.max(0) as f32;
    let chill = z.chilled.max(0) as f32;
    let next_freeze = ice.map(|t| if chill > t { 300. } else { 400. });
    let mut cuts = vec![0., H, freeze.clamp(0., H), chill.clamp(0., H)];
    if let Some(t) = ice {
        cuts.extend([t, t + next_freeze.unwrap(), t + 2000.].map(|v| v.clamp(0., H)));
    }
    for &(a, b) in holds {
        cuts.extend([a.clamp(0., H), b.clamp(0., H)]);
    }
    cuts.sort_by(f32::total_cmp);
    cuts.dedup();
    let mut x = z.x;
    let mut segments = Vec::with_capacity(cuts.len());
    for times in cuts.windows(2) {
        let a = times[0];
        let b = times[1];
        let middle = (a + b) * 0.5;
        let cold = middle < freeze || ice.is_some_and(|t| middle >= t && middle < t + next_freeze.unwrap());
        let slow = middle < chill || ice.is_some_and(|t| middle >= t && middle < t + 2000.);
        let stopped = holds.iter().any(|&(start, end)| middle >= start && middle < end);
        let velocity = if cold || stopped {
            0.
        } else {
            mean_speed(
                z,
                ice.is_some_and(|t| middle >= t) || holds.iter().any(|&(_, end)| middle >= end),
            ) * if slow { 0.5 } else { 1. }
        };
        let right = x - velocity * (b - a);
        segments.push(Segment {
            start: a,
            end: b,
            left: x,
            right,
        });
        x = right;
    }
    Path { segments }
}
fn first_ice(f: &Forecast) -> Option<f32> {
    f.events
        .iter()
        .filter(|e| e.kind == prediction::EffectKind::Ice && e.at > 0 && e.at <= H as i32)
        .map(|e| e.at as f32)
        .min_by(f32::total_cmp)
}
fn contact_time(z: Zombie, p: &Path, pos: Pos) -> f32 {
    let rect = Pad::new(pos, PuffShroom).rect();
    let (lo, hi) = prediction::attack_offsets(z);
    p.enter((rect.left + 20 - hi) as f32, (rect.right - 20 - lo) as f32)
}
fn hold_time(z: Zombie, scene: &Scene, hp: f32, contact: f32) -> f32 {
    let slow = if z.chilled as f32 > contact { 2. } else { 1. };
    if z.giant() {
        207. * slow
    } else {
        let peers = scene
            .zombies
            .iter()
            .filter(|other| {
                other.row == z.row
                    && matches!(other.kind, Z::Normal | Z::Football | Z::Imp | Z::JackInTheBox)
                    && matches!(
                        other.phase,
                        ZombiePhase::ZombieNormal | ZombiePhase::JackInTheBoxRunning
                    )
                    && (other.x - z.x).abs() < 80.
            })
            .count()
            .max(1) as f32;
        hp / peers * slow
    }
}
fn pulses(start: f32, end: f32, first: f32) -> f32 {
    let first_index = ((start - first) / 200.).ceil().max(0.);
    // Half-open exposure intervals avoid counting a pulse twice at segment cuts.
    let last_index = ((end - first) / 200.).ceil() - 1.;
    (last_index - first_index + 1.).max(0.)
}
fn gloom_damage(z: Zombie, p: &Path, scene: &Scene, until: f32) -> f32 {
    let mut damage = 0.;
    for plant in &scene.plants {
        if plant.kind != GloomShroom || (plant.pos.0 - z.row).abs() > 1 {
            continue;
        }
        let lo = plant.attack.left as f32 - (z.rect.right as f32 - z.x);
        let hi = plant.attack.right as f32 - (z.rect.left as f32 - z.x);
        let first = if plant.shooting > 0 {
            plant.shooting as f32 - 126.
        } else {
            274.
        };
        // Closed pulse counts keep the 200cs slow cadence and four actual offsets.
        // Boundary/trigger phase, deaths and interrupted shooting remain approximate.
        for (a, b) in p.windows(lo, hi, until) {
            for pulse in 0..4 {
                damage += 20. * pulses(a, b, first + pulse as f32 * 28.);
            }
        }
    }
    damage
}
#[derive(Clone, Copy)]
struct Estimate {
    risk: f32,
    hp: f32,
    eta: f32,
    gloom: f32,
    ash: f32,
}
fn estimate(z: Zombie, p: &Path, scene: &Scene, f: &Forecast, squash: Option<(i32, f32, f32)>) -> Estimate {
    let edge = scene
        .plants
        .iter()
        .filter(|plant| {
            plant.pos.0 == z.row && matches!(plant.kind, Pumpkin | GloomShroom | TwinSunflower | UmbrellaLeaf)
        })
        .map(|plant| plant.rect.right as f32 + 10.)
        .max_by(f32::total_cmp)
        .unwrap_or(prediction::home_edge(z.kind) as f32);
    let eta = p.enter(f32::NEG_INFINITY, edge);
    let cut = eta.min(H);
    let gloom = gloom_damage(z, p, scene, cut);
    let mut ash = 0.;
    for e in &f.events {
        if e.at as f32 > cut {
            continue;
        }
        if e.kind == prediction::EffectKind::Ice {
            ash += 20.;
        } else if e.kind == prediction::EffectKind::Jalapeno && e.pos.0 == z.row
            || e.shape.is_some_and(|shape| {
                (z.row - e.pos.0).abs() <= shape.row_range
                    && circle_hits_rect(shape.circle, z.rect_at(p.x(e.at as f32)))
            })
        {
            ash += 1800.;
        }
    }
    if let Some((row, at, center)) = squash {
        let rect = z.rect_at(p.x(at));
        if row == z.row && at <= cut && (rect.left as f32) < center + 65. && (rect.right as f32) > center + 20. {
            ash += 1800.;
        }
    }
    let hp = (z.hp as f32 + z.armor as f32 - gloom - ash).max(0.);
    let distance = (p.x(cut) - edge).max(0.);
    let risk = hp / 6000. * (1. + (H - eta) / H) / (1. + distance / 300.);
    Estimate {
        risk,
        hp,
        eta,
        gloom,
        ash,
    }
}
struct Actor {
    z: Zombie,
    path: Path,
    hold: Option<(Pos, f32, f32)>,
    before: Estimate,
}
pub(super) struct Context {
    actors: Vec<Actor>,
    unknown_hp: f32,
    total: usize,
}
impl Context {
    pub(super) fn new(scene: &Scene, f: &Forecast) -> Self {
        let mut actors = Vec::new();
        let mut unknown_hp = 0.;
        for &z in &scene.zombies {
            if !supported(z) {
                unknown_hp += (z.hp + z.armor).max(0) as f32;
                continue;
            }
            let bare = path(z, first_ice(f), &[]);
            let hold = scene
                .plants
                .iter()
                .filter(|p| p.pos.0 == z.row && p.kind == PuffShroom)
                .map(|p| {
                    let at = contact_time(z, &bare, p.pos).max(z.frozen as f32);
                    (p.pos, at, (at + hold_time(z, scene, p.hp as f32, at)).min(H))
                })
                .filter(|(_, at, _)| *at < H)
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            let holds: Vec<_> = hold.map(|(_, a, b)| (a, b)).into_iter().collect();
            let motion = path(z, first_ice(f), &holds);
            let before = estimate(z, &motion, scene, f, None);
            actors.push(Actor {
                z,
                path: motion,
                hold,
                before,
            });
        }
        actors.sort_by(|a, b| {
            a.z.row
                .cmp(&b.z.row)
                .then(a.z.x.total_cmp(&b.z.x))
                .then((a.z.kind as i32).cmp(&(b.z.kind as i32)))
                .then(a.z.hp.cmp(&b.z.hp))
        });
        Self {
            actors,
            unknown_hp,
            total: scene.zombies.len(),
        }
    }
    pub(super) fn features(&self, x: &mut [f32; FEATURES], action: Action, scene: &Scene, forecast: &Forecast) {
        let f = candidate_forecast(forecast, action);
        let squash = if action.kind == 3 {
            let px = (action.pos.1 * 80 - 40) as f32;
            let wait = self
                .actors
                .iter()
                .filter(|a| a.z.row == action.pos.0)
                .map(|a| a.path.enter(px - 130., px + 110.))
                .min_by(f32::total_cmp)
                .unwrap_or(H);
            let at = wait + 126.;
            self.actors
                .iter()
                .filter(|a| a.z.row == action.pos.0 && at + 56. <= H)
                .map(|a| a.path.x(at) + (a.z.rect.left + a.z.rect.right) as f32 * 0.5 - a.z.x - 40.)
                .filter(|cx| (*cx - px).abs() < 200.)
                .min_by(|a, b| (a - px).abs().total_cmp(&(b - px).abs()).then(a.total_cmp(b)))
                .map(|cx| (action.pos.0, at + 56., cx))
        } else {
            None
        };
        let mut before_norm = 0.;
        let mut after_norm = 0.;
        x[144] = -2. - self.actors.len() as f32 / self.total.max(1) as f32;
        x[151] = 1.;
        x[152] = 1.;
        x[159] = self.unknown_hp / 30000.;
        for actor in &self.actors {
            let z = actor.z;
            let before = actor.before;
            let affected = action.pos.0 > 0
                && (z.row == action.pos.0
                    || matches!(action.kind, 6 | 7)
                    || action.kind == 4 && (z.row - action.pos.0).abs() <= 1);
            let mut contact = H;
            let mut delay = 0.;
            let after = if affected {
                let mut holds: Vec<_> = actor
                    .hold
                    .filter(|(p, _, _)| *p != action.pos)
                    .map(|(_, a, b)| (a, b))
                    .into_iter()
                    .collect();
                let base = path(z, first_ice(&f), &holds);
                if z.row == action.pos.0 {
                    contact = contact_time(z, &base, action.pos).max(z.frozen as f32);
                    let life = match action.kind {
                        1 => H,
                        2 => 200.,
                        3 => squash.map_or(0., |(_, at, _)| at - 56.),
                        4..=6 => 100.,
                        7 => 420.,
                        _ => 0.,
                    };
                    if contact < life {
                        delay = hold_time(z, scene, 300., contact).min(life - contact);
                        holds.push((contact, contact + delay));
                    }
                }
                let motion = path(z, first_ice(&f), &holds);
                estimate(z, &motion, scene, &f, squash)
            } else {
                before
            };
            before_norm += before.risk * before.risk;
            after_norm += after.risk * after.risk;
            x[147] += before.hp / 30000.;
            x[148] += after.hp / 30000.;
            x[150] += (before.hp - after.hp) / 30000.;
            x[151] = x[151].min(contact / H);
            x[152] = x[152].min((contact + delay) / H);
            x[153] += (after.ash - before.ash) / 18000.;
            x[154] += bit(before.eta < H) / 10.;
            x[155] += bit(after.eta < H) / 10.;
            x[156] += (after.eta - before.eta) / H / 10.;
            x[157] += (after.gloom - before.gloom) / 30000.;
            x[158] += bit(delay > 0.) / 10.;
        }
        x[145] = before_norm.sqrt();
        x[146] = after_norm.sqrt();
        x[149] = x[145] - x[146];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz::core::model::{ContactRect, ZombieId};
    fn zombie(x: f32) -> Zombie {
        Zombie {
            id: ZombieId::from_raw(65537),
            kind: Z::GigaGargantuar,
            row: 6,
            wave: 1,
            carried: false,
            x,
            hp: 6000,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.37,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(x as i32 - 17, 0, x as i32 + 108, 154),
        }
    }
    #[test]
    fn paths_are_bounded_and_never_read_frozen_speed() {
        let mut a = zombie(800.);
        a.frozen = 300;
        a.chilled = 1000;
        let mut b = a;
        b.speed = 999.;
        let pa = path(a, Some(900.), &[(400., 600.)]);
        let pb = path(b, Some(900.), &[(400., 600.)]);
        assert!(pa.segments.len() <= 10);
        for t in [0., 100., 300., 500., 1000., 1600.] {
            assert_eq!(pa.x(t), pb.x(t));
        }
        assert_eq!(pa.x(100.), 800.);
        assert!(pa.x(H) < 800.);
    }
    #[test]
    fn pulse_count_keeps_two_hundred_cs_cadence() {
        assert_eq!(pulses(0., 599., 74.), 3.);
        assert_eq!(pulses(0., 73., 74.), 0.);
    }
    #[test]
    fn aggregate_includes_all_objects_and_is_permutation_invariant() {
        let mut scene = Scene::default();
        scene.zombies = vec![zombie(500.), zombie(700.)];
        let mut a = [0.; FEATURES];
        Context::new(&scene, &Forecast::default()).features(
            &mut a,
            Action { kind: 0, pos: (0, 0) },
            &scene,
            &Forecast::default(),
        );
        scene.zombies.reverse();
        let mut b = [0.; FEATURES];
        Context::new(&scene, &Forecast::default()).features(
            &mut b,
            Action { kind: 0, pos: (0, 0) },
            &scene,
            &Forecast::default(),
        );
        assert_eq!(a, b);
        assert_eq!(a[147], 0.4);
        assert_eq!(a[144], -3.);
        assert!(a.iter().all(|x| x.is_finite()));
    }
}
