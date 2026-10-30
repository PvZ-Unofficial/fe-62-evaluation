//! First-intervention prediction. Each candidate is one streaming O(H) trajectory.
//! Native facts and pool slots are copied once per observation; hidden frozen speed is not sampled.
use super::*;
use rsvz::core::backend::*;
use rsvz::core::contact::{circle_hits_rect, grid_explosion_shape};
use rsvz::core::model::{
    ContactRect, GridExplosionKind, PlantId, PlantThreatShape, PlantWeapon, ZombieId, ZombieTrackProfile,
};
use rsvz::core::zombie_motion::vanilla_track_frames;
use std::collections::HashMap;
use std::sync::OnceLock;
pub(crate) mod imp_landing;
mod projectiles;

pub const NEVER: i32 = 100_000;
pub const HORIZON: i32 = 6000;
pub const ASSUME_PLAN: u16 = 1;
pub const ASSUME_SPEED: u16 = 2;
pub const ASSUME_COLD: u16 = 4;
pub const ASSUME_GUN: u16 = 8;
pub const ASSUME_PEER: u16 = 16;
pub const ASSUME_CACHE: u16 = 32;
pub const ASSUME_PUFF_OFFSET: u16 = 64;
pub const ASSUME_UNTRACKED: u16 = 128;
const HAMMER_TRIGGER: f32 = 0.64;
const HAMMER_STEP: f32 = 0.16 / 33.;

fn hammer_already_landed(progress: f32, chilled: bool) -> bool {
    // Native action update precedes animation advance. In the first visible
    // interval beyond .64, the hit may still be due on the NEXT native frame.
    // Without reading a hidden previous-animation field, retain that one-frame
    // uncertainty instead of treating the just-crossed threshold as a spent hit.
    progress >= HAMMER_TRIGGER + HAMMER_STEP * if chilled { 0.5 } else { 1. } + 0.000001
}

#[derive(Clone, Copy, Debug)]
pub struct Zombie {
    pub id: ZombieId,
    pub kind: Z,
    pub row: i32,
    pub wave: i32,
    pub carried: bool,
    pub x: f32,
    pub hp: i32,
    pub armor: i32,
    pub phase: ZombiePhase,
    pub progress: f32,
    pub speed: f32,
    pub frozen: i32,
    pub chilled: i32,
    pub object: bool,
    pub eating: bool,
    pub rect: ContactRect,
}
impl Zombie {
    pub fn giant(self) -> bool {
        matches!(self.kind, Z::Gargantuar | Z::GigaGargantuar)
    }
    pub fn rect_at(self, x: f32) -> ContactRect {
        let dx = x as i32 - self.x as i32;
        ContactRect::new(
            self.rect.left + dx,
            self.rect.top,
            self.rect.right + dx,
            self.rect.bottom,
        )
    }
}
#[derive(Clone, Copy)]
pub struct Plant {
    pub id: PlantId,
    pub kind: PlantKind,
    pub pos: Pos,
    pub hp: i32,
    pub rect: ContactRect,
    pub attack: ContactRect,
    pub shooting: i32,
    pub launch: i32,
    pub state: i32,
    pub phase_left: i32,
    pub effect: i32,
}
#[derive(Clone)]
pub struct Scene {
    pub now: i32,
    pub revision: u64,
    pub track_evidence: bool,
    skip_frozen: bool,
    trim_peers: bool,
    batch_walk: bool,
    batch_actions: bool,
    edge_stats: bool,
    pub next_plant_order: u16,
    pub zombies: Vec<Zombie>,
    pub plants: Vec<Plant>,
    pub squashes: Vec<squash::Live>,
    zombie_rows: [Vec<usize>; 6],
    cold: HashMap<ZombieId, (i32, i32, i32, i32)>,
    airborne: HashMap<ZombieId, (Zombie, i32)>,
    catapult_seen: HashMap<ZombieId, (i32, i32)>,
    projectiles: projectiles::Observation,
}
impl Default for Scene {
    fn default() -> Self {
        Self {
            now: 0,
            revision: 0,
            track_evidence: true,
            skip_frozen: setting("SIX_SKIP_FROZEN", 1) != 0,
            trim_peers: setting("SIX_TRIM_PEERS", 1) != 0,
            batch_walk: setting("SIX_BATCH_WALK", 1) != 0,
            batch_actions: setting("SIX_BATCH_ACTIONS", 1) != 0,
            edge_stats: setting("SIX_PUFF_EDGE_STATS", 0) != 0,
            next_plant_order: 0,
            zombies: Vec::with_capacity(1024),
            plants: Vec::with_capacity(128),
            squashes: Vec::with_capacity(2),
            zombie_rows: std::array::from_fn(|_| Vec::with_capacity(1024)),
            cold: HashMap::with_capacity(1024),
            airborne: HashMap::new(),
            catapult_seen: HashMap::with_capacity(64),
            projectiles: projectiles::Observation::default(),
        }
    }
}
impl Scene {
    #[cfg(test)]
    pub(super) fn fixture(zombies: Vec<Zombie>) -> Self {
        let mut scene = Self::default();
        scene.zombies = zombies;
        for (i, z) in scene.zombies.iter().enumerate() {
            scene.zombie_rows[(z.row - 1) as usize].push(i);
        }
        scene
    }
    pub fn sample(&mut self, now: i32, last_ice: i32) {
        let _perf = crate::perf::scope("prediction::sample");
        self.now = now;
        self.revision = self.revision.wrapping_add(1);
        self.zombies.clear();
        self.plants.clear();
        self.squashes.clear();
        self.projectiles.clear();
        for row in &mut self.zombie_rows {
            row.clear();
        }
        // The public frame interface does not yet expose shooting counters. Keep this narrow
        // scalar sampling in the script instead of changing the framework for this experiment.
        let result = rsvz::with_backend(|b| -> RuntimeResult<()> {
            self.next_plant_order = b.plant_pool_next_slot().map_err(err)? as u16;
            for p in b.plants().map_err(err)? {
                if !b.plant_is_alive(p) || b.plant_is_squished(p) {
                    continue;
                }
                let kind = b.plant_raw_kind(p).map_err(err)?;
                self.plants.push(Plant {
                    id: b.plant_id(p),
                    kind,
                    pos: (b.plant_row(p) + 1, b.plant_col(p) + 1),
                    hp: b.plant_hp(p),
                    rect: b.plant_hit_box(p).map_err(err)?,
                    attack: b.plant_attack_rect(p, PlantWeapon::Primary).map_err(err)?,
                    shooting: b.plant_shooting_counter(p),
                    launch: b.plant_launch_counter(p),
                    state: b.plant_state(p),
                    phase_left: b.plant_state_countdown(p),
                    effect: if kind == Blover {
                        b.plant_disappear_countdown(p)
                            + 1
                            + if b.plant_state(p) == 2 {
                                0
                            } else {
                                b.plant_effect_countdown(p).max(0)
                            }
                    } else if kind == Imitator {
                        effects::remaining(b.plant_id(p), now).unwrap_or(b.plant_state_countdown(p).max(0) + 220)
                    } else {
                        b.plant_effect_countdown(p)
                    },
                });
                if kind == Squash {
                    let observed = self.plants.last().unwrap();
                    self.squashes.push(squash::Live {
                        pos: observed.pos,
                        state: observed.state,
                        count: observed.phase_left,
                        target: b.plant_target_object(p) as u32,
                        x: b.plant_target_x(p),
                    });
                }
            }
            for z in b.zombies().map_err(err)? {
                if !b.zombie_is_alive(z)
                    || !b.zombie_has_head(z)
                    || b.zombie_is_mind_controlled(z)
                    || b.zombie_is_blown_away(z)
                {
                    continue;
                }
                let kind = b.zombie_kind(z).map_err(err)?;
                let id = b.zombie_id(z);
                let frozen = b.zombie_frozen_countdown(z) > 0;
                let chilled = b.zombie_chilled_countdown(z);
                let previous = self.cold.entry(id).or_insert((-99999, -99999, 0, 0));
                let until = if frozen && previous.0 != last_ice {
                    last_ice + if previous.1 >= last_ice { 300 } else { 400 }
                } else {
                    previous.2
                };
                *previous = (last_ice, now + chilled, until, now);
                // Only the visible frozen/not-frozen state is read. Future duration is the
                // minimum compatible with observed ice; hidden frozen redraws are never read.
                let speed = b.zombie_speed_x(z);
                let row = b.zombie_row(z) + 1;
                if kind == Z::Catapult {
                    // Birth observation, not the hidden ammunition counter.
                    let first = self
                        .catapult_seen
                        .entry(id)
                        .or_insert((if b.zombie_pos_x(z) > 800. { now } else { -NEVER }, now));
                    first.1 = now;
                }
                self.zombies.push(Zombie {
                    id,
                    kind,
                    row,
                    wave: b.zombie_from_wave(z) + 1,
                    carried: telemetry::carried(id),
                    x: b.zombie_pos_x(z),
                    hp: b.zombie_hp(z),
                    armor: b.zombie_accessory_1_hp(z),
                    phase: b.zombie_phase(z).map_err(err)?,
                    progress: b.zombie_reanim_anim_time(z).map_err(err)?.unwrap_or(0.0),
                    speed,
                    frozen: if frozen { (until - now).max(1) } else { 0 },
                    chilled,
                    object: b.zombie_has_object(z),
                    eating: b.zombie_is_eating(z),
                    rect: b.zombie_collision(z).map_err(err)?,
                });
                self.zombie_rows[(row - 1) as usize].push(self.zombies.len() - 1);
                if kind == Z::Catapult {
                    self.projectiles.cars.push(projectiles::Catapult {
                        z: *self.zombies.last().unwrap(),
                        count: b.zombie_phase_counter(z),
                        y: b.zombie_int_y(z),
                        known_age: self.catapult_seen.get(&id).map_or(NEVER, |v| now - v.0),
                        previous: b.zombie_reanim_last_time(z).map_err(err)?.unwrap_or(1.),
                    });
                }
            }
            for p in b.projectiles().map_err(err)? {
                if b.projectile_kind(p) == 9 {
                    self.projectiles.balls.push(projectiles::Ball {
                        row: b.projectile_row(p) + 1,
                        x: b.projectile_pos_x(p),
                        vx: b.projectile_vel_x(p),
                        height: b.projectile_pos_z(p),
                        vz: b.projectile_vel_z(p),
                        az: b.projectile_acceleration(p),
                        age: b.projectile_age(p),
                    });
                }
            }
            self.projectiles.finish();
            Ok(())
        });
        if let Err(e) = result {
            rsvz::fail_script(e);
        }
        self.airborne.clear();
        for &z in &self.zombies {
            if z.kind == Z::Imp && matches!(z.phase, ZombiePhase::ImpGettingThrown | ZombiePhase::ImpLanding) {
                if let Some(landing) = imp_landing::observed_landing(z) {
                    self.airborne.insert(z.id, landing);
                }
            }
        }
        self.cold.retain(|_, value| value.3 == now);
        self.catapult_seen.retain(|_, value| value.1 == now);
    }
    /// Observed birth bounds ammunition without reading hidden ball counters.
    pub fn catapult_can_be_baited(&self, z: Zombie) -> bool {
        z.kind == Z::Catapult
            && z.phase == ZombiePhase::ZombieNormal
            && z.frozen == 0
            && z.hp >= 200
            && z.x <= 650.
            && self
                .catapult_seen
                .get(&z.id)
                .is_some_and(|&(birth, _)| self.now - birth < 5700)
    }

    pub fn protected_catapult_until(&self, z: Zombie) -> i32 {
        if z.kind != Z::Catapult || !matches!(z.phase, ZombiePhase::CatapultLaunching | ZombiePhase::CatapultReloading)
        {
            return 0;
        }
        // It keeps targeting the leftmost permanent plant, covered by an umbrella.
        let Some(target) = self
            .plants
            .iter()
            .filter(|p| p.pos.0 == z.row && matches!(p.kind, GloomShroom | TwinSunflower | UmbrellaLeaf))
            .min_by_key(|p| p.pos.1)
        else {
            return 0;
        };
        if !self.plants.iter().any(|p| {
            p.kind == UmbrellaLeaf && (p.pos.0 - target.pos.0).abs() <= 1 && (p.pos.1 - target.pos.1).abs() <= 1
        }) {
            return 0;
        }
        // Native initialization has 20 balls and launches are >=300cs apart.
        // Deduct one entire interval for observation/update-order uncertainty.
        self.catapult_seen
            .get(&z.id)
            .map_or(0, |&(first, _)| (5700 - (self.now - first)).max(0))
    }
    pub fn contains_zombie(&self, id: ZombieId) -> bool {
        self.cold.get(&id).is_some_and(|entry| entry.3 == self.now)
    }
    pub fn basketball_active(&self, row: i32) -> bool {
        self.projectiles.active(row)
    }

    pub fn projectile_key(&self, row: i32) -> u64 {
        if self
            .plants
            .iter()
            .any(|p| p.pos.0 == row && Pad::observed(*p).is_some())
        {
            self.projectiles.key(row)
        } else {
            0
        }
    }
    pub fn row_zombies(&self, row: i32) -> impl Iterator<Item = &Zombie> {
        // Indices only locate already-observed entities; their order never resolves zombie targets.
        self.zombie_rows[(row - 1) as usize].iter().map(|&i| &self.zombies[i])
    }
    pub fn main(&self, pos: Pos) -> Option<Plant> {
        self.plants
            .iter()
            .copied()
            .find(|p| p.pos == pos && !matches!(p.kind, Pumpkin | LilyPad | FlowerPot))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Ice,
    Doom,
    Cherry,
    Jalapeno,
    Squash,
}
#[derive(Clone, Copy, Debug)]
pub struct Effect {
    pub at: i32,
    pub kind: EffectKind,
    pub pos: Pos,
    pub certain: bool,
    pub shape: Option<PlantThreatShape>,
    pub landing: Option<squash::Landing>,
    pub body_at: Option<i32>,
}
impl Effect {
    pub fn hits(&self, z: Zombie, x: f32) -> bool {
        if self.kind == EffectKind::Jalapeno {
            return self.pos.0 == z.row;
        }
        if let Some(landing) = self.landing {
            let r = z.rect_at(x);
            let margin = if z.kind == Z::Football { 20 } else { 0 };
            return self.pos.0 == z.row
                && squash::targetable(z)
                && r.left <= 800
                && ((r.left - margin) as f32) < landing.left + 65.
                && ((r.right + margin) as f32) > landing.right + 20.;
        }
        self.shape.is_some_and(|shape| {
            (z.row - self.pos.0).abs() <= shape.row_range && circle_hits_rect(shape.circle, z.rect_at(x))
        })
    }
}
pub struct Forecast {
    pub events: Vec<Effect>,
}
impl Default for Forecast {
    fn default() -> Self {
        Self {
            events: Vec::with_capacity(24),
        }
    }
}
impl Forecast {
    pub fn add(&mut self, at: i32, kind: EffectKind, pos: Pos, certain: bool) {
        if at > 0 && at <= HORIZON {
            // The grid geometry belongs to this plan event. All target/candidate queries share it.
            let shape = match kind {
                EffectKind::Doom => Some(grid_explosion_shape(GridExplosionKind::DoomShroom, grid(pos))),
                EffectKind::Cherry => Some(grid_explosion_shape(GridExplosionKind::CherryBomb, grid(pos))),
                _ => None,
            };
            self.events.push(Effect {
                at,
                kind,
                pos,
                certain,
                shape,
                body_at: None,
                landing: None,
            });
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Pad {
    pub plant_at: i32,
    pub pos: Pos,
    pub kind: PlantKind,
    pub lifetime: i32,
    pub hp: i32,
    pub order: u16,
    pub offset_x: i32,
    pub trigger_window: i32,
    /// Policy outcome queries continue after an unused/expired pad; rule probes do not.
    pub complete_outcome: bool,
    pub squash_wait: i32,
    /// Credit damage only to this independently proven victim, never every queried actor.
    pub squash_victim: Option<ZombieId>,
    pub morph_at: i32,
    pub activation_at: i32,
    pub replaces_occupant: bool,
}
impl Pad {
    pub fn new(pos: Pos, kind: PlantKind) -> Self {
        Self {
            plant_at: 0,
            pos,
            kind,
            lifetime: if kind == Imitator {
                420
            } else if matches!(kind, IceShroom | CherryBomb | Jalapeno) {
                100
            } else if kind == Blover {
                251 // Natural activation at50, then201 updates to disappear.
            } else {
                HORIZON
            },
            hp: 300,
            order: 65534,
            offset_x: 0,
            trigger_window: 751,
            complete_outcome: false,
            squash_wait: 0,
            squash_victim: None,
            morph_at: if kind == Imitator { 320 } else { 0 },
            activation_at: match kind {
                Imitator => 420,
                IceShroom | CherryBomb | DoomShroom | Jalapeno => 100,
                Blover => 50,
                _ => NEVER,
            },
            replaces_occupant: false,
        }
    }
    pub fn rect(self) -> ContactRect {
        let x = self.pos.1 * 80 - 40 + self.offset_x;
        // Contact uses the plant's hit box, not Squash's narrower attack box.
        ContactRect::new(x + 10, 0, x + 70, 0)
    }
    fn edible(self, dt: i32) -> bool {
        !matches!(
            self.kind,
            Blover | IceShroom | CherryBomb | DoomShroom | Jalapeno | Squash
        ) && !(self.kind == Imitator && dt >= self.morph_at)
    }
    fn smashable(self, dt: i32) -> bool {
        self.kind != Squash || dt <= self.squash_wait
    }
    fn bitten(&mut self, dt: i32) {
        if self.kind == Blover {
            self.lifetime = self.lifetime.min(dt + 201);
        }
    }
    fn observed(p: Plant) -> Option<Self> {
        if !matches!(
            p.kind,
            PuffShroom | IceShroom | Imitator | CherryBomb | DoomShroom | Jalapeno | Blover | Squash
        ) {
            return None;
        }
        // Rising/falling Squashes are no longer on the ground. An idle
        // Squash cannot rise sooner than126cs; do not promise a longer wait.
        if p.kind == Squash && p.state >= 5 {
            return None;
        }
        let mut pad = Self::new(p.pos, p.kind);
        pad.hp = p.hp;
        pad.order = p.id.index();
        pad.offset_x = p.rect.left - (p.pos.1 * 80 - 40 + 10);
        pad.lifetime = match p.kind {
            PuffShroom => HORIZON,
            Squash => match p.state {
                3 => p.phase_left.max(0) + 46,
                4 => p.phase_left.max(1),
                _ => 126,
            },
            _ => p.effect.max(1),
        };
        pad.activation_at = match p.kind {
            IceShroom | Imitator | CherryBomb | DoomShroom | Jalapeno => p.effect.max(1),
            _ => NEVER,
        };
        pad.morph_at = if p.kind == Imitator { (p.effect - 100).max(0) } else { 0 };
        Some(pad)
    }
}

// The event index stays stable for ash masks; only this plant's effective
// time changes. Destroying one copy must never cancel or advance another Ice.
struct EffectSchedule {
    times: [i32; 32],
    bindings: [Option<usize>; 16],
}
impl EffectSchedule {
    fn new(f: &Forecast, pads: &[Option<Pad>], extra: usize) -> Self {
        assert!(f.events.len() <= 32, "forecast ash-mask capacity");
        let mut result = Self {
            times: [NEVER; 32],
            bindings: [None; 16],
        };
        for (i, e) in f.events.iter().enumerate() {
            result.times[i] = e.at;
        }
        for (i, pad) in pads.iter().enumerate() {
            let Some(p) = pad else { continue };
            let kind = match p.kind {
                IceShroom | Imitator => EffectKind::Ice,
                CherryBomb => EffectKind::Cherry,
                DoomShroom => EffectKind::Doom,
                Jalapeno => EffectKind::Jalapeno,
                _ => continue,
            };
            let available = |index: &usize| !result.bindings.iter().any(|b| *b == Some(*index));
            let event = f
                .events
                .iter()
                .enumerate()
                .filter(|(j, _)| available(j))
                .filter(|(_, e)| e.kind == kind && e.pos == p.pos && e.at == p.activation_at)
                .min_by_key(|(_, e)| (e.at - p.activation_at).abs())
                .map(|(j, _)| j)
                .or_else(|| {
                    if i == extra {
                        f.events
                            .iter()
                            .enumerate()
                            .find(|(j, e)| available(j) && !e.certain && e.kind == kind && e.at == p.activation_at)
                            .map(|(j, _)| j)
                    } else {
                        None
                    }
                });
            result.bindings[i] = event;
        }
        result
    }
    fn next(&self, dt: i32) -> i32 {
        self.times.iter().copied().filter(|&t| t > dt).min().unwrap_or(NEVER)
    }
    fn destroyed(&mut self, index: usize, pad: Pad, dt: i32, hammer: bool) {
        let Some(event) = self.bindings[index] else { return };
        if self.times[event] <= dt {
            return;
        }
        self.times[event] = if hammer && (pad.kind != Imitator || dt >= pad.morph_at) {
            self.times[event].min(dt + 1)
        } else {
            NEVER
        };
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    /// First point where the current-actor projection uses an uncertain transition.
    /// This is provenance, not a calibrated probability or a global safety proof.
    pub uncertain_at: i32,
    pub assumptions: u16,
    pub end_at: i32,
    pub throw_at: i32,
    pub throw_x: f32,
    pub throw_chilled: i32,
    pub puff_deadline: i32,
    pub death: i32,
    pub danger: i32,
    pub hp_at_end: i32,
    pub x_at_end: f32,
    pub trigger: i32,
    pub impact: i32,
    pub activated: i32,
    pub first_ice: i32,
    pub ash_hits: i32,
    pub ash_mask: u32,
    pub squash_hit: bool,
    pub gloom_damage: i32,
    pub x5: f32,
    pub x50: f32,
    pub hp50: i32,
    pub steps: i32,
    pub skipped: i32,
}
impl Default for Outcome {
    fn default() -> Self {
        Self {
            uncertain_at: NEVER,
            assumptions: 0,
            end_at: 0,
            throw_at: NEVER,
            throw_x: f32::NAN,
            throw_chilled: 0,
            puff_deadline: NEVER,
            death: NEVER,
            danger: NEVER,
            hp_at_end: 0,
            x_at_end: 0.,
            trigger: NEVER,
            impact: NEVER,
            activated: NEVER,
            first_ice: NEVER,
            ash_hits: 0,
            ash_mask: 0,
            squash_hit: false,
            gloom_damage: 0,
            x5: f32::NAN,
            x50: f32::NAN,
            hp50: 0,
            steps: 0,
            skipped: 0,
        }
    }
}
impl Outcome {
    fn assumption(&mut self, enabled: bool, at: i32, reason: u16) {
        if !enabled {
            return;
        }
        self.uncertain_at = self.uncertain_at.min(at.max(0));
        self.assumptions |= reason;
    }

    pub fn known_through(self, at: i32) -> bool {
        self.end_at >= at && self.uncertain_at > at
    }

    pub fn checkpoint_hp(self, at: i32) -> Option<i32> {
        if self.death <= at {
            Some(0)
        } else if self.end_at == at {
            Some(self.hp_at_end.max(0))
        } else {
            None
        }
    }
    pub fn safe(self) -> bool {
        self.danger == NEVER
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MotionPoint {
    pub x: f32,
    pub progress: f32,
    pub speed: f32,
    pub frozen: i32,
    pub chilled: i32,
    pub health: i32,
    pub phase: ZombiePhase,
    pub object: bool,
    pub gloom_damage: i32,
}
pub struct MotionPrefix {
    entry_rects: Option<[ContactRect; 2]>,
    entry_shape: Option<PlantThreatShape>,
    entry_at: i32,
    projection_at: i32,
    projection_x: Option<f32>,
    pub points: [Option<MotionPoint>; 51],
    pub first_pad_change: i32,
    pub first_contact: [i32; 9],
}
pub const REUSE_LIMIT: i32 = 200;
impl Default for MotionPrefix {
    fn default() -> Self {
        Self {
            entry_rects: None,
            entry_shape: None,
            entry_at: NEVER,
            projection_at: NEVER,
            projection_x: None,
            points: [None; 51],
            first_pad_change: NEVER,
            first_contact: [NEVER; 9],
        }
    }
}
impl MotionPoint {
    pub fn near(self, z: Zombie) -> bool {
        (self.x - z.x).abs() <= 1.
            && (self.progress - z.progress).abs() <= 0.02
            && (self.speed - z.speed).abs() <= 0.02
            && (self.frozen - z.frozen).abs() <= 5
            && (self.chilled - z.chilled).abs() <= 5
            && (self.health - z.hp - z.armor).abs() <= 40
            && self.phase == z.phase
            && self.object == z.object
    }
    pub fn matches(self, z: Zombie) -> bool {
        self.x.to_bits() == z.x.to_bits()
            && self.progress.to_bits() == z.progress.to_bits()
            && self.speed.to_bits() == z.speed.to_bits()
            && self.frozen == z.frozen
            && self.chilled == z.chilled
            && self.health == z.hp + z.armor
            && self.phase == z.phase
            && self.object == z.object
    }
}
#[derive(Clone, Copy)]
struct Gun {
    row: i32,
    rect: ContactRect,
    shooting: i32,
    launch: i32,
}

fn overlap(a: ContactRect, b: ContactRect) -> i32 {
    a.right.min(b.right) - a.left.max(b.left)
}

fn puff_offset_changes_contact(z: Zombie, x: f32, mut pad: Pad) -> bool {
    if pad.kind != PuffShroom || pad.offset_x != 0 {
        return false;
    }
    pad.offset_x = -5;
    let left = contact(z, x, pad.rect());
    pad.offset_x = 4;
    left != contact(z, x, pad.rect())
}
pub(super) fn attack_offsets(z: Zombie) -> (i32, i32) {
    // Football and Imp retain ZombieInitialize's attack rect (50, 0, 20, 115).
    // In particular it can bite a squash on its right while another Football is the victim.
    if z.giant() {
        (-30, 59)
    } else if matches!(z.kind, Z::Football | Z::Imp) {
        (50, 70)
    } else if z.kind == Z::Digger {
        if z.phase == ZombiePhase::DiggerWalkingWithoutAxe {
            (50, 70)
        } else {
            (10, 30)
        }
    } else {
        (20, 70)
    }
}
pub fn contact(z: Zombie, x: f32, rect: ContactRect) -> bool {
    let (lo, hi) = attack_offsets(z);
    overlap(ContactRect::new(x as i32 + lo, 0, x as i32 + hi, 0), rect) >= 20
}
pub fn could_contact_pad(z: Zombie, pad: Pad) -> bool {
    // Grounded walkers move leftwards. Include an extra pixel for float-to-int
    // boundaries, ignore all stops/slowdowns, and allow the full trigger window: rejection
    // proves no contact; acceptance still requires the unchanged frame-by-frame predictor.
    let horizon = pad.lifetime.min(pad.trigger_window).max(0);
    let (lo, hi) = attack_offsets(z);
    let swept = ContactRect::new(
        (z.x - max_step(z.kind) * horizon as f32) as i32 + lo - 1,
        0,
        z.x as i32 + hi,
        0,
    );
    overlap(swept, pad.rect()) >= 20
}
fn in_gloom(z: Zombie, x: f32, g: Gun) -> bool {
    (z.row - g.row).abs() <= 1 && overlap(z.rect_at(x), g.rect) > 0
}

/// Upper bound on travel before a proposed block must become useful. Existing
/// cold reduces travel; a planned Jalapeno can thaw it. Future ice/throws/stops
/// are ignored because they can only reduce this upper bound.
pub fn could_reach_pad(z: Zombie, pad: Pad, forecast: &Forecast) -> bool {
    let horizon = pad.lifetime.min(pad.trigger_window).max(0);
    let thaw = forecast
        .events
        .iter()
        .filter(|e| e.kind == EffectKind::Jalapeno && e.pos.0 == z.row)
        .map(|e| e.at.max(0))
        .min()
        .unwrap_or(horizon)
        .min(horizon);
    let frozen = (z.frozen - 1).max(0).min(thaw);
    let slow = ((z.chilled - 1).max(0).min(thaw) - frozen).max(0);
    let distance = max_step(z.kind) * ((horizon - frozen) as f32 - slow as f32 * 0.5);
    let (lo, hi) = attack_offsets(z);
    overlap(
        ContactRect::new((z.x - distance) as i32 + lo - 2, 0, z.x as i32 + hi, 0),
        pad.rect(),
    ) >= 20
}

pub fn max_speed(kind: Z) -> f32 {
    match kind {
        Z::Football | Z::JackInTheBox => 0.68,
        Z::Imp => 0.37, // Survival Imp; the 0.9 IZ exception does not apply here.
        Z::Zomboni => 0.25,
        _ => 0.37,
    }
}
pub fn max_step(kind: Z) -> f32 {
    match kind {
        Z::Gargantuar | Z::GigaGargantuar => 0.372,
        Z::Football => 0.85,
        Z::JackInTheBox => 1.047,
        Z::Imp => 1.2,
        Z::Zomboni => 0.25,
        Z::Catapult => 0.37,
        _ => 0.7,
    }
}
/// Native Zamboni speed curve uses the integer x coordinate; below 400 it keeps
/// the last speed. No frozen speed is read (Zambonis cannot be frozen).
pub fn zomboni_step(x: &mut f32, speed: &mut f32) {
    if *x > 400. {
        let t = (((*x as i32 - 700) as f64 / -400.) as f32).clamp(0., 1.);
        *speed = ((t as f64 * -0.19999999925494194) as f32 as f64 + 0.25) as f32;
    }
    *x -= *speed;
}
pub fn walk_kind(x: &mut f32, progress: &mut f32, speed: f32, chilled: bool, kind: Z) {
    let profile = match kind {
        Z::Gargantuar | Z::GigaGargantuar => ZombieTrackProfile::GargantuarWalk,
        Z::Football => ZombieTrackProfile::FootballWalk,
        Z::JackInTheBox => ZombieTrackProfile::JackBoxWalk,
        Z::Imp => ZombieTrackProfile::ImpWalk,
        _ => {
            *x -= speed * 0.5 * if chilled { 0.5 } else { 1. };
            return;
        }
    };
    let frames = vanilla_track_frames(profile);
    let rate =
        speed * frames.len() as f32 / (frames[frames.len() - 1] - frames[0]) * 47.0 * if chilled { 0.5 } else { 1.0 };
    let index = ((*progress * (frames.len() - 1) as f32 + 1.0) as usize).clamp(1, frames.len() - 1);
    *x -= (frames[index] - frames[index - 1]) * 0.01 * rate;
    *progress = wrap_progress(*progress + rate * 0.01 / frames.len() as f32);
}

/// A constant-work walk step using the same ground keyframes/formula as rsvz stable motion.
/// It is kept stateful here so hammer recovery and future ice can be composed without O(H虏) calls.
pub fn walk_step(x: &mut f32, progress: &mut f32, speed: f32, chilled: bool, giant: bool) {
    let frames = vanilla_track_frames(if giant {
        ZombieTrackProfile::GargantuarWalk
    } else {
        ZombieTrackProfile::FootballWalk
    });
    let rate =
        speed * frames.len() as f32 / (frames[frames.len() - 1] - frames[0]) * 47.0 * if chilled { 0.5 } else { 1.0 };
    walk_at_rate(x, progress, frames, rate);
}
fn walk_at_rate(x: &mut f32, progress: &mut f32, frames: &[f32], rate: f32) {
    let index = ((*progress * (frames.len() - 1) as f32 + 1.0) as usize).clamp(1, frames.len() - 1);
    *x -= (frames[index] - frames[index - 1]) * 0.01 * rate;
    *progress = wrap_progress(*progress + rate * 0.01 / frames.len() as f32);
}
fn wrap_progress(next: f32) -> f32 {
    // Most steps do not cross an animation cycle. Preserve the exact original
    // remainder operation at wrap/exceptional values, rather than calling it every cs.
    if (0.0..1.0).contains(&next) {
        next
    } else {
        next.rem_euclid(1.0)
    }
}

struct WalkTrack {
    // Only the two fixed vanilla profiles used by this predictor. No per-query allocation.
    delta: [f32; 64],
    last: i32,
    len: f32,
    span: f32,
}
impl WalkTrack {
    fn get(giant: bool) -> &'static Self {
        static GIANT: OnceLock<WalkTrack> = OnceLock::new();
        static FOOTBALL: OnceLock<WalkTrack> = OnceLock::new();
        let (cache, profile) = if giant {
            (&GIANT, ZombieTrackProfile::GargantuarWalk)
        } else {
            (&FOOTBALL, ZombieTrackProfile::FootballWalk)
        };
        cache.get_or_init(|| {
            let frames = vanilla_track_frames(profile);
            let mut delta = [0.; 64];
            assert!((2..=delta.len()).contains(&frames.len()));
            for i in 1..frames.len() {
                // Keep the original f32 rounding order: difference, *0.01, then *rate.
                delta[i] = (frames[i] - frames[i - 1]) * 0.01;
            }
            Self {
                delta,
                last: frames.len() as i32 - 1,
                len: frames.len() as f32,
                span: frames[frames.len() - 1] - frames[0],
            }
        })
    }
}
struct WalkRate {
    track: &'static WalkTrack,
    speed: f32,
    chilled: bool,
    rate: f32,
    phase_step: f32,
}
impl WalkRate {
    fn new(giant: bool) -> Self {
        Self {
            track: WalkTrack::get(giant),
            speed: f32::NAN,
            chilled: false,
            rate: 0.,
            phase_step: 0.,
        }
    }
    #[inline(always)]
    fn step(&mut self, x: &mut f32, progress: &mut f32, speed: f32, chilled: bool) {
        if self.speed != speed || self.chilled != chilled {
            self.speed = speed;
            self.chilled = chilled;
            self.rate = speed * self.track.len / self.track.span * 47.0 * if chilled { 0.5 } else { 1.0 };
            self.phase_step = self.rate * 0.01 / self.track.len;
        }
        // Clamping to the small profile range makes i32 and usize casts equivalent,
        // including out-of-range/NaN input, without a 64-bit saturating conversion.
        let index = ((*progress * self.track.last as f32 + 1.0) as i32).clamp(1, self.track.last) as usize;
        *x -= self.track.delta[index] * self.rate;
        *progress = wrap_progress(*progress + self.phase_step);
    }
}

/// Does not branch on future planting times. Existing pads plus at most one concrete new pad.
pub fn simulate(z: Zombie, scene: &Scene, forecast: &Forecast, extra: Option<Pad>) -> Outcome {
    simulate_until(z, scene, forecast, extra, HORIZON)
}

pub fn simulate_action(z: Zombie, scene: &Scene, forecast: &Forecast, extra: Option<Pad>) -> Outcome {
    let extra = extra.map(|mut p| {
        p.complete_outcome = true;
        p.trigger_window = HORIZON;
        p
    });
    simulate_until(z, scene, forecast, extra, HORIZON)
}

fn plant_observer(row: i32) -> Zombie {
    Zombie {
        id: ZombieId::from_raw(0),
        kind: Z::GigaGargantuar,
        row,
        wave: 0,
        carried: false,
        x: 10000.,
        hp: 6000,
        armor: 0,
        phase: ZombiePhase::ZombieNormal,
        progress: 0.,
        speed: 0.,
        frozen: 0,
        chilled: 0,
        object: false,
        eating: false,
        rect: ContactRect::new(10000, 0, 10125, 154),
    }
}

pub(super) fn plant_outcome(scene: &Scene, forecast: &Forecast, pad: Pad, horizon: i32) -> Outcome {
    simulate_until(
        plant_observer(pad.pos.0),
        scene,
        forecast,
        Some(Pad {
            complete_outcome: true,
            ..pad
        }),
        horizon,
    )
}

pub fn simulate_until(z: Zombie, scene: &Scene, forecast: &Forecast, extra: Option<Pad>, horizon: i32) -> Outcome {
    let mut out = simulate_range(z, scene, forecast, extra, horizon, false, None, 0);
    let Some(pad) = extra.filter(|p| p.kind == PuffShroom && p.offset_x == 0) else {
        return out;
    };
    if scene.edge_stats {
        PUFF_EDGE_STATS.with(|s| {
            let mut v = s.get();
            v[0] += 1;
            s.set(v);
        });
    }
    // A failed nominal block needs no extra work. Only an already raised,
    // still pending hammer with an offset-sensitive contact opens this path.
    // This does not enumerate pixels or fork every candidate's trajectory.
    if out.trigger == NEVER || !pending_hammer_edge(scene, z, pad, horizon) {
        return out;
    }
    let started = scene.edge_stats.then(std::time::Instant::now);
    let mut steps = out.steps;
    let mut skipped = out.skipped;
    let mut runs = 0;
    for offset in [-5, 4] {
        let mut edge = pad;
        edge.offset_x = offset;
        // Keep a real full outcome if a peer destroys the pad before contact;
        // otherwise the short-query early exit can misleadingly look safe.
        edge.complete_outcome = true;
        let other = simulate_range(z, scene, forecast, Some(edge), horizon, false, None, 0);
        runs += 1;
        steps += other.steps;
        skipped += other.skipped;
        if puff_edge_risk(other) < puff_edge_risk(out) {
            out = other;
        }
    }
    out.steps = steps;
    out.skipped = skipped;
    out.assumption(scene.track_evidence, 0, ASSUME_PUFF_OFFSET);
    if let Some(started) = started {
        PUFF_EDGE_STATS.with(|s| {
            let mut v = s.get();
            v[1] += 1;
            v[2] += runs;
            v[3] += started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            s.set(v);
        });
    }
    out
}

thread_local! { static PUFF_EDGE_STATS: std::cell::Cell<[u64; 4]> = const { std::cell::Cell::new([0; 4]) }; }
pub fn take_puff_edge_stats() -> [u64; 4] {
    PUFF_EDGE_STATS.with(|s| s.replace([0; 4]))
}
fn puff_edge_risk(out: Outcome) -> (bool, i32, i32, i32) {
    // Return one actual counterfactual, not a synthetic mix of incompatible states.
    (out.trigger != NEVER, out.danger, -out.death, out.ash_hits)
}
fn pending_hammer_edge(scene: &Scene, target: Zombie, pad: Pad, horizon: i32) -> bool {
    let exposed = |z: Zombie| {
        z.giant()
            && z.phase == ZombiePhase::GargantuarSmashing
            && z.frozen < horizon
            && !hammer_already_landed(z.progress, z.chilled > 0)
            && puff_offset_changes_contact(z, z.x, pad)
    };
    exposed(target) || scene.row_zombies(pad.pos.0).any(|z| z.id != target.id && exposed(*z))
}
/// A not-yet-landed Imp: advance the existing environment, but credit no
/// damage or freeze to the future walker before its arrival.
pub fn simulate_arrival(z: Zombie, scene: &Scene, forecast: &Forecast, arrival: i32) -> Outcome {
    simulate_range(z, scene, forecast, None, HORIZON, false, None, arrival)
}
pub(super) fn simulate_imp_arrival(z: Zombie, scene: &Scene, forecast: &Forecast, born: i32, arrival: i32) -> Outcome {
    let arrival = imp_ready_at(z.chilled, forecast, born, arrival);
    simulate_state(
        z, scene, forecast, None, HORIZON, false, None, arrival, born, None, None,
    )
}
pub(super) fn imp_ready_at(chilled: i32, forecast: &Forecast, born: i32, unchilled_arrival: i32) -> i32 {
    let start = (unchilled_arrival - 25).max(0);
    let mut remaining = (unchilled_arrival - start) as f32;
    for dt in start + 1..=HORIZON {
        let slow = chilled > dt
            || forecast
                .events
                .iter()
                .any(|e| e.kind == EffectKind::Ice && e.at >= born && e.at <= dt && dt < e.at + 2000);
        remaining -= if slow { 0.5 } else { 1. };
        if remaining <= 0. {
            return dt;
        }
    }
    NEVER
}
pub fn probe_trigger(z: Zombie, scene: &Scene, forecast: &Forecast, pad: Pad) -> Outcome {
    simulate_range(
        z,
        scene,
        forecast,
        Some(pad),
        pad.trigger_window.min(pad.lifetime),
        true,
        None,
        0,
    )
}
pub fn simulate_captured(z: Zombie, scene: &Scene, forecast: &Forecast, prefix: &mut MotionPrefix) -> Outcome {
    simulate_range(z, scene, forecast, None, HORIZON, false, Some(prefix), 0)
}

/// Tail-Ice query only: stop at first common-range entry, retaining real pads.
pub fn first_common_entry(z: Zombie, scene: &Scene, rects: [ContactRect; 2]) -> (i32, i32) {
    let mut prefix = MotionPrefix {
        entry_rects: Some(rects),
        ..MotionPrefix::default()
    };
    let out = simulate_captured(z, scene, &Forecast::default(), &mut prefix);
    (prefix.entry_at, out.death)
}

pub fn first_left_of(z: Zombie, scene: &Scene, f: &Forecast, right: i32, horizon: i32) -> i32 {
    let edge = ContactRect::new(-10000, 0, right - (z.rect.right - z.rect.left), 10000);
    let mut prefix = MotionPrefix {
        entry_rects: Some([edge; 2]),
        ..MotionPrefix::default()
    };
    let out = simulate_range(z, scene, f, None, horizon, false, Some(&mut prefix), 0);
    prefix.entry_at.min(out.death)
}

/// One forward trajectory per uncovered W20 row-one red, no time search.
/// Pending/planned cold remains active; the nuke being scheduled cannot kill its own witnesses.
pub fn w20_doom_coverage(scene: &Scene, forecast: &Forecast, earliest: i32) -> i32 {
    let shape = grid_explosion_shape(GridExplosionKind::DoomShroom, grid((4, 7)));
    let without = Forecast {
        events: forecast
            .events
            .iter()
            .copied()
            .filter(|e| e.kind != EffectKind::Doom)
            .collect(),
    };
    let mut at = earliest;
    for &observed in scene.row_zombies(1) {
        let mut z = observed;
        if z.kind != Z::GigaGargantuar || z.wave != 20 || z.carried || circle_hits_rect(shape.circle, z.rect) {
            continue;
        }
        // Coverage needs the rightmost bound, opposite to home-risk prediction.
        // The scene deliberately stores minimum freeze duration and maximum speed.
        if z.frozen > 0 {
            z.frozen = scene
                .cold
                .get(&z.id)
                .map_or(z.frozen + 200, |cold| (cold.2 + 200 - scene.now).max(z.frozen));
            z.speed = 0.23;
        }
        let mut prefix = MotionPrefix {
            entry_shape: Some(shape),
            ..MotionPrefix::default()
        };
        let out = simulate_captured(z, scene, &without, &mut prefix);
        if out.death < prefix.entry_at {
            continue;
        }
        // Plants explode before zombies move in the native frame. Entry after
        // N movement updates is coverable by an explosion on update N + 1.
        at = at.max(prefix.entry_at.saturating_add(1));
    }
    at
}

pub fn simulate_projected(z: Zombie, scene: &Scene, forecast: &Forecast, at: i32) -> (Outcome, Option<f32>) {
    let mut prefix = MotionPrefix {
        projection_at: at,
        ..MotionPrefix::default()
    };
    let out = simulate_captured(z, scene, forecast, &mut prefix);
    let x = prefix
        .projection_x
        .or_else(|| (out.danger == at && out.death > at).then_some(out.x_at_end));
    (out, x)
}

/// Resolve already planted Ice/copies once for the whole field, before a
/// target in another row is allowed to borrow that effect. Probes remove only
/// the corresponding event and replace only its existing physical occupant.
pub fn resolve_live_ice(scene: &Scene, forecast: &mut Forecast) {
    let mut index = 0;
    while index < forecast.events.len() {
        let event = forecast.events[index];
        if event.kind != EffectKind::Ice || !event.certain {
            index += 1;
            continue;
        }
        let Some(plant) = scene
            .plants
            .iter()
            .find(|p| p.pos == event.pos && matches!(p.kind, IceShroom | Imitator))
            .copied()
        else {
            index += 1;
            continue;
        };
        let mut pad = Pad::observed(plant).expect("ice body");
        pad.complete_outcome = true;
        pad.trigger_window = event.at;
        pad.replaces_occupant = true;
        pad.lifetime = pad.lifetime.min(external_destruction_except(scene, None, pad));
        let without = Forecast {
            events: forecast
                .events
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, e)| *e)
                .collect(),
        };
        let outcome = plant_outcome(scene, &without, pad, event.at);
        if outcome.activated == NEVER {
            forecast.events.remove(index);
        } else {
            forecast.events[index].at = outcome.activated;
            index += 1;
        }
    }
    forecast.events.sort_by_key(|e| e.at);
}

// One shared plant HP pool. Companions move, stop, freeze and die alongside the
// queried zombie, instead of receiving their own fictional copy of each Puff.
// This is O(NH), with no combinations of future planting times or zombie order.
struct Chewer {
    z: Zombie,
    x: f32,
    progress: f32,
    speed: f32,
    frozen: i32,
    chilled: i32,
    health: i32,
    walker: WalkRate,
    bite: BiteClock,
    eating: bool,
    action: u8,
    action_progress: f32,
    smashed: bool,
    has_imp: bool,
    arrival: i32,
}
impl Chewer {
    // Select against the plants still present when this actor's slot updates.
    // The caller applies this strike before advancing to the next actor.
    fn giant_step(&mut self, pads: &[Option<Pad>], core: Option<Plant>) -> u16 {
        let target = pads
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.filter(|p| contact(self.z, self.x, p.rect())).map(|p| (i, p.order)))
            .min_by_key(|&(_, order)| order);
        let core_contact = core.is_some_and(|p| contact(self.z, self.x, p.rect));
        let target = target.filter(|&(_, order)| !core_contact || core.is_none_or(|p| order < p.id.index()));
        if self.action == 0 {
            if self.has_imp && self.health < if self.z.kind == Z::GigaGargantuar { 3000 } else { 1500 } && self.x > 400.
            {
                self.action = 2;
                self.action_progress = 0.;
            } else if target.is_some() || core_contact {
                self.action = 1;
                self.action_progress = 0.;
                self.smashed = false;
            } else {
                self.walker
                    .step(&mut self.x, &mut self.progress, self.speed, self.chilled > 0);
            }
            return 0;
        }
        let recovered = self.action_progress >= 1.;
        self.action_progress +=
            if self.action == 1 { 0.16 / 33. } else { 0.24 / 34. } * if self.chilled > 0 { 0.5 } else { 1. };
        let mut hit = 0;
        if self.action == 1 && !self.smashed && self.action_progress >= 0.64 {
            self.smashed = true;
            if let Some((i, _)) = target {
                hit = 1 << i;
            }
        }
        if self.action == 2 && self.action_progress >= 0.74 {
            self.has_imp = false;
        }
        if recovered {
            self.action = 0;
            self.progress = 0.;
            self.speed = max_speed(self.z.kind);
        }
        hit
    }
    fn new(z: Zombie) -> Self {
        Self {
            z,
            x: z.x,
            progress: z.progress,
            speed: z.speed,
            frozen: z.frozen,
            chilled: z.chilled,
            health: z.hp + z.armor,
            walker: WalkRate::new(z.giant()),
            bite: BiteClock::default(),
            eating: z.eating,
            action: if z.phase == ZombiePhase::GargantuarSmashing {
                1
            } else if z.phase == ZombiePhase::GargantuarThrowing {
                2
            } else {
                0
            },
            action_progress: z.progress,
            smashed: z.phase == ZombiePhase::GargantuarSmashing && hammer_already_landed(z.progress, z.chilled > 0),
            has_imp: z.object,
            arrival: 0,
        }
    }
    fn alive(&self) -> bool {
        if self.z.giant() {
            self.health > 0
        } else {
            self.health >= 90
        }
    }
    fn effect(&mut self, e: &Effect, dt: i32) {
        if dt < self.arrival {
            if e.kind == EffectKind::Ice {
                self.chilled = 2000;
            }
            return;
        }
        if !self.alive() {
            return;
        }
        if e.kind == EffectKind::Ice {
            self.frozen = if self.chilled > 0 { 300 } else { 400 };
            self.chilled = 2000;
            // Ordinary thaw retains the observed walking speed.
            self.health -= 20;
        } else if e.hits(self.z, self.x) {
            self.health -= 1800;
            if e.kind == EffectKind::Jalapeno {
                self.frozen = 0;
                self.chilled = 0;
            }
        }
    }
}
#[derive(Default)]
struct BiteClock {
    target: Option<usize>,
    next: i32,
    chilled: bool,
}
impl BiteClock {
    fn damage(&mut self, target: usize, dt: i32, chilled: bool) -> i32 {
        // Native: 4HP every 4cs (8cs when chilled). The hidden age modulo is
        // not sampled: allow the first bite immediately, a <=7cs early bound.
        if self.target != Some(target) || self.chilled != chilled {
            self.next = dt;
        }
        self.target = Some(target);
        self.chilled = chilled;
        if dt < self.next {
            return 0;
        }
        self.next = dt + if chilled { 8 } else { 4 };
        4
    }
}
fn chew_target(z: Zombie, x: f32, pads: &[Option<Pad>]) -> Option<usize> {
    pads.iter()
        .enumerate()
        .filter_map(|(i, p)| p.filter(|p| contact(z, x, p.rect())).map(|p| (i, p.order)))
        .min_by_key(|&(_, order)| order)
        .map(|(i, _)| i)
}

fn simulate_range(
    z: Zombie, scene: &Scene, forecast: &Forecast, extra: Option<Pad>, horizon: i32, trigger_only: bool,
    prefix: Option<&mut MotionPrefix>, arrival: i32,
) -> Outcome {
    simulate_state(
        z,
        scene,
        forecast,
        extra,
        horizon,
        trigger_only,
        prefix,
        arrival,
        arrival,
        None,
        None,
    )
}
fn simulate_state(
    z: Zombie, scene: &Scene, forecast: &Forecast, extra: Option<Pad>, horizon: i32, trigger_only: bool,
    mut prefix: Option<&mut MotionPrefix>, arrival: i32, born: i32, final_state: Option<&mut Zombie>,
    final_scene: Option<&mut Scene>,
) -> Outcome {
    let _perf = crate::perf::scope("prediction::simulate_range");
    let adjusted = extra.and_then(|pad| squash::with_intervention(scene, forecast, pad));
    let forecast = adjusted.as_ref().unwrap_or(forecast);
    let mut out = Outcome::default();
    if !scene.track_evidence {
        out.uncertain_at = 0;
        out.assumptions = ASSUME_UNTRACKED;
    }
    if z.frozen > 0 {
        out.assumption(scene.track_evidence, z.frozen, ASSUME_COLD);
    }
    let vehicle = matches!(z.kind, Z::Zomboni | Z::Catapult);
    let mut guns = [Gun {
        row: 0,
        rect: ContactRect::new(0, 0, 0, 0),
        shooting: 0,
        launch: 200,
    }; 8];
    let mut gun_count = 0;
    let mut pads = [None; 16];
    let mut pad_count = 0;
    let mut core: Option<Plant> = None;
    for p in &scene.plants {
        if extra.is_some_and(|pad| pad.replaces_occupant && pad.pos == p.pos && pad.kind == p.kind) {
            continue;
        }
        if p.kind == GloomShroom && (z.row - p.pos.0).abs() <= 1 && gun_count < guns.len() {
            if p.shooting <= 0 {
                let earliest = ((z.rect.left - p.attack.right).max(0) as f32 / max_step(z.kind)).floor() as i32;
                out.assumption(scene.track_evidence, earliest, ASSUME_GUN);
            }
            guns[gun_count] = Gun {
                row: p.pos.0,
                rect: p.attack,
                shooting: p.shooting,
                launch: p.launch.max(1),
            };
            gun_count += 1;
        }
        if p.pos.0 != z.row {
            continue;
        }
        if pad_count < 14 {
            if let Some(mut pad) = Pad::observed(*p) {
                pad.lifetime = pad.lifetime.min(external_destruction_bound(scene, z, pad));
                pads[pad_count] = Some(pad);
                pad_count += 1;
            }
        }
        if (z.giant() || vehicle) && matches!(p.kind, Pumpkin | GloomShroom | TwinSunflower | UmbrellaLeaf) {
            // Pumpkin is the edible top plant at the protected grid.
            if p.kind != Pumpkin
                && scene
                    .plants
                    .iter()
                    .any(|other| other.pos == p.pos && other.kind == Pumpkin)
            {
                continue;
            }
            if core.is_none_or(|old| p.rect.right > old.rect.right) {
                core = Some(*p);
            }
        }
    }
    let planned_body = forecast.events.iter().find_map(|e| {
        e.body_at
            .filter(|_| e.pos.0 == z.row)
            .filter(|_| extra.is_none_or(|p| p.kind != IceShroom))
            .map(|at| {
                let mut p = Pad::new(e.pos, IceShroom);
                p.plant_at = at;
                p.activation_at = e.at;
                p.lifetime = e.at;
                p.order = scene.next_plant_order;
                p
            })
    });
    let body_index = pad_count;
    if let Some(body) = planned_body {
        if body.plant_at == 0 {
            for p in &mut pads[..pad_count] {
                if p.is_some_and(|p| p.pos == body.pos && p.kind == PuffShroom) {
                    *p = None;
                }
            }
        }
        pads[pad_count] = (body.plant_at == 0).then_some(body);
        pad_count += 1;
    }
    let extra_index = pad_count;
    if let Some(pad) = extra {
        pads[pad_count] = (pad.plant_at == 0).then_some(pad);
        pad_count += 1;
    }
    let mut chewers: Vec<Chewer> = if pad_count == 0 || vehicle {
        Vec::new()
    } else {
        scene
            .row_zombies(z.row)
            .copied()
            .filter(|other| {
                other.id != z.id
                    && matches!(
                        other.kind,
                        Z::Football
                            | Z::Imp
                            | Z::Normal
                            | Z::Conehead
                            | Z::Buckethead
                            | Z::Digger
                            | Z::Gargantuar
                            | Z::GigaGargantuar
                    )
                    && !matches!(
                        other.phase,
                        ZombiePhase::ImpGettingThrown
                            | ZombiePhase::ImpLanding
                            | ZombiePhase::DiggerTunneling
                            | ZombiePhase::DiggerRising
                            | ZombiePhase::DiggerStunned
                    )
                    && pads[..pad_count]
                        .iter()
                        .flatten()
                        .chain(extra.as_ref().filter(|p| p.plant_at > 0))
                        .chain(planned_body.as_ref())
                        .any(|p| {
                            could_contact_pad(*other, *p)
                                && other.x - p.rect().right as f32
                                    <= max_step(other.kind)
                                        * (if scene.trim_peers {
                                            horizon.min(p.lifetime)
                                        } else {
                                            horizon
                                        }) as f32
                        })
            })
            .map(Chewer::new)
            .collect()
    };
    if pad_count > 0 && !vehicle {
        for &(child, delay) in scene.airborne.values() {
            if child.id == z.id || child.row != z.row {
                continue;
            }
            let mut other = Chewer::new(child);
            other.arrival = imp_ready_at(child.chilled, forecast, 0, delay);
            chewers.push(other);
        }
    }
    chewers.sort_unstable_by_key(|other| other.z.id.index());
    for other in &chewers {
        if scene.track_evidence && other.frozen > 0 {
            out.assumption(scene.track_evidence, other.frozen, ASSUME_PEER | ASSUME_COLD);
        }
    }
    let coverage_query = prefix.as_ref().is_some_and(|p| p.entry_shape.is_some());
    let mut x = z.x;
    let mut progress = z.progress;
    let mut speed = z.speed;
    let mut walker = WalkRate::new(z.giant());
    let mut bite = BiteClock::default();
    let mut eating = z.eating;
    let mut frozen = z.frozen;
    let mut chilled = z.chilled;
    let mut health = z.hp + z.armor;
    let mut has_imp = z.object;
    let mut action = if z.phase == ZombiePhase::GargantuarSmashing {
        1
    } else if z.phase == ZombiePhase::GargantuarThrowing {
        2
    } else {
        0
    };
    let mut action_progress = if action != 0 { z.progress } else { 0.0 };
    let mut smashed = action == 1 && hammer_already_landed(action_progress, chilled > 0);
    let mut target_pad = None;
    if let Some(body) = planned_body {
        pads[body_index] = Some(body);
    }
    let mut schedule = EffectSchedule::new(forecast, &pads[..pad_count], extra_index);
    if planned_body.is_some_and(|p| p.plant_at > 0) {
        pads[body_index] = None;
    }

    let shots = (!vehicle && pad_count > 0 && scene.projectiles.active(z.row))
        .then(|| scene.projectiles.events(scene, z.row, extra, forecast));
    let impacts = shots.as_deref().unwrap_or(&[]);
    let words = impacts.iter().map(|e| e.shot / 64 + 1).max().unwrap_or(0);
    let mut shot_bits = [0u64; 16];
    let mut overflow = Vec::new();
    let spent: &mut [u64] = if words <= shot_bits.len() {
        &mut shot_bits[..words]
    } else {
        overflow.resize(words, 0);
        &mut overflow
    };
    let mut next_shot = 0;

    let squash = extra.is_some_and(|p| p.kind == Squash);
    let squash_wait = extra.map_or(0, |p| p.squash_wait);
    let mut squash_launched = false;
    let mut next_effect = schedule.next(0);
    let mut next_gun_event = 1;
    let mut last_gun_event = 0;
    let mut contact_col = 9;
    let end = horizon.min(HORIZON);
    let mut dt = 0;
    'timeline: while dt < end {
        // Future normal Ice body belongs to the committed retention plan.
        if let Some(body) = planned_body.filter(|p| p.plant_at == dt + 1) {
            for (i, p) in pads[..pad_count].iter_mut().enumerate() {
                if p.is_some_and(|p| p.pos == body.pos && p.kind == PuffShroom) {
                    *p = None;
                    if i == extra_index {
                        out.impact = dt + 1;
                    }
                }
            }
            pads[body_index] = Some(body);
        }

        if let Some(query) = prefix.as_deref_mut() {
            if query
                .entry_rects
                .is_some_and(|rs| rs.iter().all(|r| overlap(z.rect_at(x), *r) > 0))
                || query
                    .entry_shape
                    .is_some_and(|shape| circle_hits_rect(shape.circle, z.rect_at(x)))
            {
                query.entry_at = dt;
                break;
            }
        }
        dt += 1;
        if let Some(pad) = extra.filter(|p| p.plant_at == dt) {
            pads[extra_index] = Some(pad);
        }
        out.steps += 1;
        if dt == next_effect {
            for (i, e) in forecast.events.iter().enumerate() {
                if dt == schedule.times[i]
                    && !e.certain
                    && (e.kind == EffectKind::Ice
                        || e.kind == EffectKind::Jalapeno && e.pos.0 == z.row
                        || e.shape.is_some_and(|s| (e.pos.0 - z.row).abs() <= s.row_range))
                {
                    out.assumption(scene.track_evidence, dt, ASSUME_PLAN);
                }
                if e.kind == EffectKind::Ice {
                    let effect_time = schedule.times[i];
                    if dt == effect_time {
                        out.first_ice = out.first_ice.min(dt);
                        for other in &mut chewers {
                            other.effect(e, dt);
                            if other.frozen > 0 {
                                out.assumption(scene.track_evidence, dt + other.frozen, ASSUME_PEER | ASSUME_COLD);
                            }
                        }
                        if z.kind == Z::Zomboni || dt < born {
                            continue;
                        }
                        if dt < arrival {
                            // A thrown Imp is already present: Ice chills it but
                            // cannot freeze/damage it until landing finishes.
                            chilled = 2000;
                            continue;
                        }
                        frozen = if coverage_query {
                            if chilled > 0 { 400 } else { 600 }
                        } else if chilled > 0 {
                            300
                        } else {
                            400
                        };
                        chilled = 2000;
                        // Only an actual walking restart redraws speed, not thaw.
                        out.assumption(scene.track_evidence, dt + frozen, ASSUME_COLD);
                        health -= 20;
                    }
                    continue;
                }
                if dt != schedule.times[i] {
                    continue;
                }
                for other in &mut chewers {
                    other.effect(e, dt);
                }
                let hit = e.hits(z, x);
                if hit && dt >= arrival {
                    health -= 1800;
                    out.ash_hits += 1;
                    out.ash_mask |= 1 << i;
                    if e.kind == EffectKind::Jalapeno {
                        frozen = 0;
                        chilled = 0;
                    }
                }
            }
            next_effect = schedule.next(dt);
        }
        // A guaranteed squash victim is supplied only after the separate possible-lock proof.
        // Its plant blocks until takeoff; a hammer/competitor before takeoff cancels the hit.
        if squash && dt == squash_wait + 126 {
            squash_launched = pads[extra_index].is_some_and(|p| p.lifetime >= squash_wait + 126);
        }
        if squash_launched
            && extra.is_some_and(|p| p.squash_victim == Some(z.id))
            && dt == squash_wait + 182
            && health > 0
        {
            health -= 1800;
            out.ash_hits += 1;
            out.squash_hit = true;
        }
        if dt == next_gun_event {
            let elapsed = dt - last_gun_event;
            let mut next_gap = NEVER;
            for gun in &mut guns[..gun_count] {
                gun.launch -= elapsed;
                let starts = gun.launch <= 0
                    && (dt >= arrival && in_gloom(z, x, *gun)
                        || chewers
                            .iter()
                            .any(|other| other.alive() && dt >= other.arrival && in_gloom(other.z, other.x, *gun)));
                if gun.launch <= 0 {
                    gun.launch = 200;
                }
                // A burst starting this frame decrements once, not by the elapsed gap.
                gun.shooting = if starts { 199 } else { (gun.shooting - elapsed).max(0) };
                if dt >= arrival && matches!(gun.shooting, 126 | 98 | 70 | 42) && in_gloom(z, x, *gun) {
                    health -= 20;
                    out.gloom_damage += 20;
                }
                if matches!(gun.shooting, 126 | 98 | 70 | 42) {
                    for other in &mut chewers {
                        if other.alive() && dt >= other.arrival && in_gloom(other.z, other.x, *gun) {
                            other.health -= 20;
                        }
                    }
                }
                next_gap = next_gap.min(gun.launch);
                for shot in [126, 98, 70, 42] {
                    if gun.shooting > shot {
                        next_gap = next_gap.min(gun.shooting - shot);
                    }
                }
            }
            last_gun_event = dt;
            next_gun_event = dt + next_gap;
            if scene.trim_peers {
                chewers.retain(Chewer::alive);
            }
        }
        // A Football's 270 body HP loses its head strictly below 90, not at 90.
        if health <= 0 || !z.giant() && !vehicle && health < 90 {
            out.death = dt;
            break;
        }
        frozen = (frozen - 1).max(0);
        chilled = (chilled - 1).max(0);
        if let Some(prefix) = prefix.as_mut() {
            // A new passive pad cannot affect this trajectory before spatial contact.
            // Monotone leftward movement crosses each column threshold at most once.
            while contact_col > 0 && x as i32 <= contact_col * 80 + 40 {
                if contact(z, x, Pad::new((z.row, contact_col), PuffShroom).rect()) {
                    prefix.first_contact[(contact_col - 1) as usize] = dt;
                }
                contact_col -= 1;
            }
        }
        for (i, slot) in pads[..pad_count].iter_mut().enumerate() {
            if let Some(pad) = slot.as_mut() {
                if pad.kind == Imitator && dt == pad.morph_at {
                    pad.hp = 300;
                }
            }
            if slot.is_some_and(|p| dt >= p.lifetime) {
                let pad = slot.unwrap();
                if i == extra_index && dt == pad.activation_at {
                    out.activated = out.activated.min(dt);
                }
                if dt < pad.activation_at {
                    schedule.destroyed(i, pad, dt, false);
                }

                if let Some(prefix) = prefix.as_mut() {
                    prefix.first_pad_change = prefix.first_pad_change.min(dt);
                }
                *slot = None;
                if target_pad == Some(i) {
                    target_pad = None;
                }
            }
        }
        // Once this candidate expires or misses its own contact window,
        // later damage cannot make the proposed block usable.
        if extra.is_some()
            && !extra.unwrap().complete_outcome
            && !squash
            && out.trigger == NEVER
            && dt >= extra.unwrap().plant_at
            && (pads[extra_index].is_none() || dt > extra.unwrap().trigger_window)
        {
            break;
        }
        // Companions exist to resolve shared pad consumption. Once no pad is
        // left, their long-term motion no longer changes that decision. Natural
        // gun starts after this point are projected from the target alone.
        if scene.trim_peers
            && !chewers.is_empty()
            && pads[..pad_count].iter().all(Option::is_none)
            && extra.is_none_or(|p| p.plant_at <= dt)
            && planned_body.is_none_or(|p| p.plant_at <= dt)
        {
            chewers.clear();
        }
        if scene.track_evidence
            && extra.is_some_and(|p| p.kind == PuffShroom && p.offset_x == 0)
            && out.assumptions & ASSUME_PUFF_OFFSET == 0
        {
            if let Some(pad) = pads[extra_index] {
                if puff_offset_changes_contact(z, x, pad)
                    || chewers
                        .iter()
                        .any(|other| other.alive() && puff_offset_changes_contact(other.z, other.x, pad))
                {
                    out.assumption(scene.track_evidence, dt, ASSUME_PUFF_OFFSET);
                }
            }
        }
        // Native zombies act in pool-slot order. A later actor sees removals
        // performed by an earlier one, including another strike in this frame.
        let target_index = chewers.partition_point(|other| other.z.id.index() < z.id.index());
        for actor in 0..=chewers.len() {
            let mut bites = [0; 16];
            if actor != target_index {
                let other = &mut chewers[if actor < target_index { actor } else { actor - 1 }];
                if !other.alive() {
                    continue;
                }
                other.frozen = (other.frozen - 1).max(0);
                other.chilled = (other.chilled - 1).max(0);
                if other.frozen > 0 || dt < other.arrival {
                    other.bite.target = None;
                    continue;
                }
                if other.z.giant() {
                    let prior_action = other.action;
                    let hit = other.giant_step(&pads[..pad_count], core);
                    if prior_action != 0 && other.action == 0 {
                        out.assumption(scene.track_evidence, dt, ASSUME_PEER | ASSUME_SPEED);
                    }
                    for (i, slot) in pads[..pad_count].iter_mut().enumerate() {
                        if hit & (1 << i) == 0 || slot.is_none_or(|p| !p.smashable(dt)) {
                            continue;
                        }
                        let pad = slot.take().unwrap();
                        schedule.destroyed(i, pad, dt, true);
                        next_effect = next_effect.min(schedule.next(dt));
                        if i == extra_index {
                            out.impact = dt;
                            if pad.activation_at < NEVER && (pad.kind != Imitator || dt >= pad.morph_at) {
                                out.activated = out.activated.min(dt + 1);
                            }
                        }
                        if let Some(prefix) = prefix.as_mut() {
                            prefix.first_pad_change = prefix.first_pad_change.min(dt);
                        }
                        if target_pad == Some(i) {
                            target_pad = None;
                        }
                    }
                    continue;
                }
                if let Some(i) = chew_target(other.z, other.x, &pads[..pad_count]) {
                    other.eating = true;
                    pads[i].as_mut().unwrap().bitten(dt);
                    if pads[i].unwrap().edible(dt) {
                        bites[i] += other.bite.damage(i, dt, other.chilled > 0);
                    }
                } else {
                    other.bite.target = None;
                    if other.eating {
                        other.eating = false;
                        other.progress = 0.;
                        other.speed = max_speed(other.z.kind);
                        out.assumption(scene.track_evidence, dt, ASSUME_PEER | ASSUME_SPEED);
                    }
                    if other.z.kind == Z::Football {
                        other
                            .walker
                            .step(&mut other.x, &mut other.progress, other.speed, other.chilled > 0);
                    } else {
                        walk_kind(
                            &mut other.x,
                            &mut other.progress,
                            other.speed,
                            other.chilled > 0,
                            other.z.kind,
                        );
                    }
                }
            } else {
                if frozen == 0 && dt >= arrival {
                    if vehicle {
                        if z.kind == Z::Zomboni {
                            zomboni_step(&mut x, &mut speed);
                        } else {
                            // Ignore basketball stops: this is a moving-car threat bound,
                            // not an invented remaining-ammunition or animation counter.
                            x -= speed * if chilled > 0 { 0.5 } else { 1. };
                        }
                        if core.is_some_and(|p| x + 10. <= p.rect.right as f32) || x as i32 <= home_edge(z.kind) {
                            out.danger = dt;
                            break 'timeline;
                        }
                    } else if z.giant() {
                        if x < -150.0 {
                            out.danger = dt;
                            break 'timeline;
                        }
                        if action == 0 {
                            if has_imp && health < if z.kind == Z::GigaGargantuar { 3000 } else { 1500 } && x > 400.0 {
                                action = 2;
                                action_progress = 0.0;
                            } else {
                                let target = pads[..pad_count]
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(i, p)| p.filter(|p| contact(z, x, p.rect())).map(|p| (i, p.order)))
                                    .min_by_key(|&(_, order)| order);
                                let core_contact = core.is_some_and(|p| contact(z, x, p.rect));
                                if target.is_some() || core_contact {
                                    action = 1;
                                    action_progress = 0.0;
                                    smashed = false;
                                    target_pad = target
                                        .filter(|&(_, order)| {
                                            !core_contact || core.is_none_or(|p| order < p.id.index())
                                        })
                                        .map(|(i, _)| i);
                                    if target_pad == Some(extra_index) {
                                        out.trigger = out.trigger.min(dt);
                                    }
                                } else {
                                    walker.step(&mut x, &mut progress, speed, chilled > 0);
                                }
                            }
                        } else {
                            // 33 smash frames at 16 fps; 34 throw frames at 24 fps. Full recovery included.
                            // Recovery consumes the animation completed on the preceding update.
                            let recovered = action_progress >= 1.0;
                            action_progress += if action == 1 { 0.16 / 33.0 } else { 0.24 / 34.0 }
                                * if chilled > 0 { 0.5 } else { 1.0 };
                            if action == 1 && !smashed && action_progress >= 0.64 {
                                smashed = true;
                                let target = pads[..pad_count]
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(i, p)| p.filter(|p| contact(z, x, p.rect())).map(|p| (i, p)))
                                    .min_by_key(|&(_, p)| p.order);
                                let core_contact = core.is_some_and(|p| contact(z, x, p.rect));
                                if let Some((i, pad)) = target
                                    .filter(|&(_, pad)| !core_contact || core.is_none_or(|p| pad.order < p.id.index()))
                                {
                                    if i == extra_index {
                                        out.trigger = out.trigger.min(dt);
                                        if pad.activation_at < NEVER && (pad.kind != Imitator || dt >= pad.morph_at) {
                                            out.activated = out.activated.min(dt + 1);
                                        }
                                        if pad.smashable(dt) {
                                            out.impact = dt;
                                        }
                                    }
                                    schedule.destroyed(i, pad, dt, true);
                                    next_effect = next_effect.min(schedule.next(dt));
                                    if pad.smashable(dt) {
                                        if let Some(prefix) = prefix.as_mut() {
                                            prefix.first_pad_change = prefix.first_pad_change.min(dt);
                                        }
                                        pads[i] = None;
                                    }
                                } else if core_contact {
                                    out.danger = dt;
                                    break 'timeline;
                                }
                            }
                            if action == 2 && action_progress >= 0.74 {
                                if has_imp {
                                    out.throw_at = dt;
                                    out.throw_x = x;
                                    out.throw_chilled = chilled;
                                }
                                has_imp = false;
                            }
                            if recovered {
                                action = 0;
                                progress = 0.0;
                                speed = if coverage_query { 0.23 } else { 0.37 };
                                out.assumption(scene.track_evidence, dt, ASSUME_SPEED);
                                target_pad = None;
                            }
                        }
                    } else {
                        let target = chew_target(z, x, &pads[..pad_count]);
                        if let Some(i) = target {
                            eating = true;
                            if i == extra_index {
                                out.trigger = out.trigger.min(dt);
                            }
                            pads[i].as_mut().unwrap().bitten(dt);
                            if pads[i].unwrap().edible(dt) {
                                bites[i] += bite.damage(i, dt, chilled > 0);
                            }
                        } else {
                            bite.target = None;
                            if eating {
                                eating = false;
                                progress = 0.;
                                speed = max_speed(z.kind);
                                out.assumption(scene.track_evidence, dt, ASSUME_SPEED);
                            }
                            if z.kind != Z::Football {
                                walk_kind(&mut x, &mut progress, speed, chilled > 0, z.kind);
                            } else {
                                walker.step(&mut x, &mut progress, speed, chilled > 0);
                            }
                        }
                        if x as i32 <= home_edge(z.kind) {
                            out.danger = dt;
                            break 'timeline;
                        }
                        // At x<4 a new col-one Puff no longer guarantees 20px bite
                        // overlap across its native [-5,+4] horizontal offsets.
                        if z.kind == Z::Imp && x < 4. && out.puff_deadline == NEVER {
                            out.puff_deadline = dt;
                        }
                    }
                } else {
                    bite.target = None;
                }
            }
            for (i, slot) in pads[..pad_count].iter_mut().enumerate() {
                if let Some(pad) = slot {
                    pad.hp -= bites[i];
                    if bites[i] > 0 && pad.hp <= 0 {
                        schedule.destroyed(i, *pad, dt, false);
                        next_effect = next_effect.min(schedule.next(dt));
                        if i == extra_index {
                            out.impact = dt;
                        }
                        if let Some(prefix) = prefix.as_mut() {
                            prefix.first_pad_change = prefix.first_pad_change.min(dt);
                        }
                        *slot = None;
                    }
                }
            }
        }
        while next_shot < impacts.len() && impacts[next_shot].at <= dt {
            let shot = impacts[next_shot];
            next_shot += 1;
            let word = shot.shot / 64;
            let bit = 1u64 << (shot.shot % 64);
            if spent[word] & bit != 0 {
                continue;
            }
            let hit = pads[..pad_count]
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    p.filter(|p| projectiles::hit(shot.left, shot.right, p.rect()))
                        .map(|p| (i, p.order, p.pos))
                })
                .min_by_key(|p| p.1);
            let permanent = scene
                .plants
                .iter()
                .filter(|p| {
                    p.pos.0 == z.row
                        && Pad::observed(**p).is_none()
                        && !matches!(p.kind, LilyPad | FlowerPot | Squash)
                        && projectiles::hit(shot.left, shot.right, p.rect)
                })
                .min_by_key(|p| p.id.index());
            if permanent.is_some_and(|p| hit.is_none_or(|h| p.id.index() < h.1)) {
                spent[word] |= bit;
                continue;
            }
            if let Some((i, _, pos)) = hit {
                spent[word] |= bit;
                let covered = scene
                    .plants
                    .iter()
                    .any(|p| p.kind == UmbrellaLeaf && (p.pos.0 - pos.0).abs() <= 1 && (p.pos.1 - pos.1).abs() <= 1);
                if covered {
                    continue;
                }
                let pad = pads[i].as_mut().unwrap();
                pad.hp -= 75;
                if let Some(prefix) = prefix.as_mut() {
                    prefix.first_pad_change = prefix.first_pad_change.min(dt);
                }
                if pad.hp < 0 {
                    schedule.destroyed(i, *pad, dt, false);
                    next_effect = next_effect.min(schedule.next(dt));
                    if i == extra_index {
                        out.impact = dt;
                    }
                    pads[i] = None;
                }
            }
        }
        if dt == 5 {
            out.x5 = x;
        }
        if dt == 50 {
            out.x50 = x;
            out.hp50 = health;
        }
        if let Some(prefix) = prefix.as_mut() {
            if dt == prefix.projection_at {
                prefix.projection_x = Some(x);
            }
        }
        if dt <= REUSE_LIMIT + 50 && dt % 5 == 0 {
            if let Some(prefix) = prefix.as_mut() {
                prefix.points[(dt / 5) as usize] = Some(MotionPoint {
                    x,
                    progress: if action == 0 { progress } else { action_progress },
                    speed,
                    frozen,
                    chilled,
                    health,
                    phase: match action {
                        1 => ZombiePhase::GargantuarSmashing,
                        2 => ZombiePhase::GargantuarThrowing,
                        _ => ZombiePhase::ZombieNormal,
                    },
                    object: has_imp,
                    gloom_damage: out.gloom_damage,
                });
            }
        }
        if trigger_only && out.trigger != NEVER {
            break;
        }
        if scene.skip_frozen && frozen > 1 && chewers.iter().all(|other| !other.alive() || other.frozen > 1) {
            // Nothing moves while frozen. Preserve every damage, thaw, expiry,
            // launch and observation boundary; only the intervening counters advance.
            let mut next = (dt + frozen).min(next_effect).min(next_gun_event);
            if let Some(shot) = impacts.get(next_shot) {
                next = next.min(shot.at);
            }
            for other in &chewers {
                if other.alive() {
                    next = next.min(dt + other.frozen);
                }
            }
            for point in [5, 50] {
                if point > dt {
                    next = next.min(point);
                }
            }
            if prefix.is_some() && dt < REUSE_LIMIT + 50 {
                next = next.min((dt / 5 + 1) * 5);
            }
            if let Some(p) = prefix.as_ref() {
                if p.projection_at > dt {
                    next = next.min(p.projection_at);
                }
            }
            for pad in pads[..pad_count].iter().flatten() {
                next = next.min(pad.lifetime);
            }
            if squash {
                for point in [squash_wait + 126, squash_wait + 182] {
                    if point > dt {
                        next = next.min(point);
                    }
                }
            } else if extra.is_some() && out.trigger == NEVER {
                next = next.min(extra.unwrap().trigger_window + 1);
            }
            if let Some(pad) = extra.filter(|p| p.plant_at > dt) {
                next = next.min(pad.plant_at);
            }
            if let Some(body) = planned_body.filter(|p| p.plant_at > dt) {
                next = next.min(body.plant_at);
            }
            let skip = (next - dt - 1).min(end - dt).max(0);
            frozen -= skip;
            chilled = (chilled - skip).max(0);
            for other in &mut chewers {
                other.frozen = (other.frozen - skip).max(0);
                other.chilled = (other.chilled - skip).max(0);
            }
            dt += skip;
            out.steps += skip;
            out.skipped += skip;
        } else if scene.batch_walk
            && prefix
                .as_ref()
                .is_none_or(|p| p.entry_rects.is_none() && p.entry_shape.is_none())
            && frozen == 0
            && (action == 0 || scene.batch_actions)
            && !vehicle
            && !eating
            && dt >= arrival
            && chewers.is_empty()
            && pads[..pad_count].iter().all(Option::is_none)
            && !(action == 0
                && z.giant()
                && has_imp
                && health < if z.kind == Z::GigaGargantuar { 3000 } else { 1500 }
                && x > 400.)
        {
            // Between damage/contact events, only the animation walk changes.
            // Advance its compact recurrence without rerunning the full combat
            // state machine. Keep observation points and every event boundary.
            let mut next = next_effect.min(next_gun_event).min(end + 1);
            if let Some(shot) = impacts.get(next_shot) {
                next = next.min(shot.at);
            }
            if chilled > 0 {
                next = next.min(dt + chilled);
            }
            for point in [5, 50] {
                if point > dt {
                    next = next.min(point);
                }
            }
            if prefix.is_some() && dt < REUSE_LIMIT + 50 {
                next = next.min((dt / 5 + 1) * 5);
            }
            if let Some(p) = prefix.as_ref() {
                if p.projection_at > dt {
                    next = next.min(p.projection_at);
                }
            }
            if squash {
                for point in [squash_wait + 126, squash_wait + 182] {
                    if point > dt {
                        next = next.min(point);
                    }
                }
            }
            if let Some(body) = planned_body.filter(|p| p.plant_at > dt) {
                next = next.min(body.plant_at);
            }
            let mut boundary = home_edge(z.kind) as f32;
            if z.kind == Z::Imp {
                boundary = boundary.max(4.);
            }
            if let Some(core) = core {
                boundary = boundary.max((core.rect.right - attack_offsets(z).0 - 20) as f32);
            }
            if prefix.is_some() && contact_col > 0 {
                boundary = boundary.max((contact_col * 80 + 40) as f32);
            }
            let action_step = if action == 1 { 0.16 / 33.0 } else { 0.24 / 34.0 } * if chilled > 0 { 0.5 } else { 1.0 };
            let reach = if action == 0 {
                ((x - boundary - 2.) / max_step(z.kind)).max(0.) as i32
            } else {
                let phase_boundary = if action == 1 && !smashed {
                    0.64
                } else if action == 2 && has_imp {
                    0.74
                } else {
                    1.0
                };
                // Leave the crossing itself to the ordinary action resolver.
                (((phase_boundary - action_progress) / action_step) as i32 - 2).max(0)
            };
            if let Some(pad) = extra.filter(|p| p.plant_at > dt) {
                next = next.min(pad.plant_at);
            }
            let skip = (next - dt - 1).min(reach).max(0);
            for _ in 0..skip {
                if action != 0 {
                    action_progress += action_step;
                } else if !z.giant() && z.kind != Z::Football {
                    walk_kind(&mut x, &mut progress, speed, chilled > 0, z.kind);
                } else {
                    walker.step(&mut x, &mut progress, speed, chilled > 0);
                }
            }
            chilled = (chilled - skip).max(0);
            dt += skip;
            out.steps += skip;
            out.skipped += skip;
        }
    }
    out.hp_at_end = health;
    out.x_at_end = x;
    out.end_at = dt;
    if dt == horizon {
        if let Some(scene) = final_scene {
            scene.plants.retain_mut(|p| {
                if p.pos.0 != z.row || Pad::observed(*p).is_none() {
                    return true;
                }
                let Some(pad) = pads[..pad_count]
                    .iter()
                    .flatten()
                    .find(|pad| pad.pos == p.pos && pad.kind == p.kind)
                else {
                    return false;
                };
                p.hp = pad.hp;
                if pad.activation_at < NEVER {
                    p.effect = (pad.activation_at - dt).max(1);
                } else if p.kind == Blover {
                    p.effect = (pad.lifetime - dt).max(1);
                }
                p.phase_left = (p.phase_left - dt).max(0);
                true
            });
            if let Some(pad) = extra.and_then(|_| pads[extra_index]) {
                if !scene.plants.iter().any(|p| p.pos == pad.pos && p.kind == pad.kind) {
                    scene.plants.push(Plant {
                        id: PlantId::from_raw(0x1fffe),
                        kind: pad.kind,
                        pos: pad.pos,
                        hp: pad.hp,
                        rect: pad.rect(),
                        attack: pad.rect(),
                        shooting: 0,
                        launch: 0,
                        state: 0,
                        phase_left: 0,
                        effect: (pad.activation_at - dt).max(1),
                    });
                }
            }
            for p in &mut scene.plants {
                if p.kind != GloomShroom {
                    continue;
                }
                if let Some(g) = guns[..gun_count]
                    .iter()
                    .find(|g| g.row == p.pos.0 && g.rect == p.attack)
                {
                    let elapsed = dt - last_gun_event;
                    p.launch = (g.launch - elapsed).max(1);
                    p.shooting = (g.shooting - elapsed).max(0);
                }
            }
        }
    }
    if let Some(state) = final_state {
        *state = Zombie {
            x,
            rect: z.rect_at(x),
            hp: health,
            armor: 0,
            speed,
            frozen,
            chilled,
            progress: if action == 0 { progress } else { action_progress },
            phase: if action == 1 {
                ZombiePhase::GargantuarSmashing
            } else if action == 2 {
                ZombiePhase::GargantuarThrowing
            } else {
                ZombiePhase::ZombieNormal
            },
            object: has_imp,
            eating,
            ..z
        };
    }
    out
}

pub fn project_row(scene: &Scene, f: &Forecast, row: i32, extra: Option<Pad>, at: i32) -> Option<(Scene, Forecast)> {
    let mut projected = scene.clone();
    projected.now += at;
    projected.projectiles.advance(at);
    projected.zombies.clear();
    projected.squashes.clear();
    for indices in &mut projected.zombie_rows {
        indices.clear();
    }
    let mut actors: Vec<_> = scene.row_zombies(row).copied().collect();
    actors.sort_by(|a, b| a.x.total_cmp(&b.x));

    projected.airborne.retain(|_, (z, _)| z.row != row);
    for original in actors {
        let (z, arrival) = if let Some(&(child, delay)) = scene.airborne.get(&original.id) {
            let arrival = imp_ready_at(child.chilled, f, 0, delay);
            if arrival > at {
                return None;
            }
            (child, arrival)
        } else {
            (original, 0)
        };
        if z.kind == Z::Balloon && z.phase == ZombiePhase::BalloonFlying
            || z.kind == Z::Catapult && scene.protected_catapult_until(z) >= at
        {
            // Keep special actors in the scene. A flying balloon cannot lock a
            // Squash; an observed protected shooting phase cannot drive yet.
            let mut state = z;
            if z.kind == Z::Balloon {
                state.x -= max_step(z.kind) * at as f32;
                state.rect = z.rect_at(state.x);
            }
            state.frozen = (state.frozen - at).max(0);
            state.chilled = (state.chilled - at).max(0);
            projected.zombie_rows[(row - 1) as usize].push(projected.zombies.len());
            projected.zombies.push(state);
            continue;
        }
        if !matches!(
            z.kind,
            Z::Gargantuar
                | Z::GigaGargantuar
                | Z::Football
                | Z::Imp
                | Z::Normal
                | Z::Conehead
                | Z::Buckethead
                | Z::Zomboni
        ) || matches!(z.phase, ZombiePhase::ImpGettingThrown | ZombiePhase::ImpLanding)
        {
            // No exact short state transition is available for these actors.
            // Decline this wait; the live scene will be observed again next tick.
            return None;
        }
        let mut state = z;
        let out = simulate_state(
            z,
            scene,
            f,
            extra.map(|mut p| {
                p.complete_outcome = true;
                p
            }),
            at,
            false,
            None,
            arrival,
            0,
            Some(&mut state),
            None,
        );
        if out.danger <= at {
            return None;
        }
        if out.death <= at {
            continue;
        }

        projected.zombie_rows[(row - 1) as usize].push(projected.zombies.len());
        projected.zombies.push(state);
    }
    // Resolve the row's temporary bodies in one shared consumption timeline.
    simulate_state(
        plant_observer(row),
        scene,
        f,
        extra.map(|p| Pad {
            complete_outcome: true,
            ..p
        }),
        at,
        false,
        None,
        0,
        0,
        None,
        Some(&mut projected),
    );
    let shifted = Forecast {
        events: f
            .events
            .iter()
            .filter(|e| {
                e.at > at
                    && !(e.certain
                        && e.kind == EffectKind::Ice
                        && scene
                            .plants
                            .iter()
                            .any(|p| p.pos == e.pos && matches!(p.kind, IceShroom | Imitator))
                        && !projected
                            .plants
                            .iter()
                            .any(|p| p.pos == e.pos && matches!(p.kind, IceShroom | Imitator)))
            })
            .map(|e| {
                let mut e = *e;
                e.at -= at;
                e.body_at = e.body_at.map(|t| (t - at).max(0));
                if let Some(landing) = &mut e.landing {
                    landing.launch = (landing.launch - at).max(0);
                }
                e
            })
            .collect(),
    };
    Some((projected, shifted))
}

/// A necessary reach bound only. It never decides whether damage/padding is sufficient.
pub fn home_edge(kind: Z) -> i32 {
    match kind {
        Z::Football | Z::Zomboni | Z::Catapult => -175,
        Z::Gargantuar | Z::GigaGargantuar | Z::PoleVaulting => -150,
        _ => -100,
    }
}
pub fn could_threaten(z: Zombie, scene: &Scene, horizon: i32) -> bool {
    let boundary = if z.giant() {
        scene
            .plants
            .iter()
            .filter(|p| p.pos.0 == z.row && matches!(p.kind, Pumpkin | GloomShroom | TwinSunflower | UmbrellaLeaf))
            .map(|p| (p.rect.right + 10) as f32)
            .fold(-150., f32::max)
    } else {
        home_edge(z.kind) as f32
    };
    z.x - max_step(z.kind) * horizon as f32 <= boundary
}

/// Only current coverage is eligible for retention: never hold a still-too-far-right giant.
pub fn retainable_ash(z: Zombie, forecast: &Forecast) -> u32 {
    let mut mask = 0;
    for (i, e) in forecast.events.iter().enumerate() {
        if e.at > 2000 || !matches!(e.kind, EffectKind::Doom | EffectKind::Cherry) {
            continue;
        }
        let shape = e.shape.expect("ash plan geometry");
        if (z.row - e.pos.0).abs() <= shape.row_range && circle_hits_rect(shape.circle, z.rect) {
            mask |= 1 << i;
        }
    }
    mask
}

/// The user's accepted storage rule for awake Puff in the intact six-flower lineup.
pub fn sheltered_puff_site(pos: Pos) -> bool {
    matches!(pos.0, 2 | 5) && pos.1 == 3
}

/// Fog/pool throw geometry. Evaluate the two range endpoints only; the rightmost
/// landing bounds whether a left-walking child can ever touch a stored plant.
/// This is a storage exposure check, not a claim that every possible child survives.
pub fn future_imp_reaches(out: Outcome, rect: ContactRect, window: i32) -> bool {
    if out.throw_at >= window || !out.throw_x.is_finite() {
        return false;
    }
    let distance = (out.throw_x - 360.).max(40.);
    let distance = if distance > 140. { distance - 100. } else { distance };
    // Native dy starts at 88, vertical acceleration is -0.05 per frame.
    let v = distance / 3. * 0.5 * 0.05;
    let flight = ((v - 0.025 + ((v - 0.025).powi(2) + 0.1 * 88.).sqrt()) / 0.05).ceil() as i32;
    let landing_x = out.throw_x - 133. - 3. * flight as f32;
    let lands = out.throw_at + flight + 30;
    if lands >= window || landing_x + 70. < (rect.left + 20) as f32 {
        return false;
    }
    let walk = ((landing_x + 50. - (rect.right - 20) as f32) / 0.37).max(0.) as i32;
    lands + walk < window
}

/// Conservative storage window using only visible actors and scheduled effects.
/// No visible imp does not prove that every throw has finished.
pub fn prestore_clear(scene: &Scene, pos: Pos, window: i32, forecast: &Forecast) -> bool {
    let rect = Pad::new(pos, PuffShroom).rect();
    scene.row_zombies(pos.0).all(|z| {
        if sheltered_puff_site(pos) && matches!(z.kind, Z::Zomboni | Z::Catapult) {
            return true;
        }
        if z.giant() {
            if z.object {
                // An ash plan is not itself proof that this giant throws. Use
                // its actual predicted damage/action timeline before rejecting.
                let out = simulate_until(*z, scene, forecast, None, window);
                // Even an already visible throw needs its landing animation
                // before a child can eat. Do not reject a shorter window.
                if future_imp_reaches(out, rect, window) {
                    return false;
                }
            }
            true
        } else if z.kind == Z::Imp {
            // Grounded Imps now share the plant HP simulation. Flight/landing
            // remains separate until its transition is represented there.
            !matches!(z.phase, ZombiePhase::ImpGettingThrown | ZombiePhase::ImpLanding)
                || z.rect.right < rect.left + 20
                || z.rect.left as f32 - 3.0 * window as f32 > (rect.right - 20) as f32
        } else if z.kind == Z::Catapult {
            scene.protected_catapult_until(*z) + ((z.x - rect.right as f32) / 0.5).max(0.) as i32 > window
        } else if matches!(z.kind, Z::Balloon | Z::Bungee) {
            true
        } else {
            z.x < (rect.left - 60) as f32 || z.x - max_step(z.kind) * window as f32 > (rect.right + 30) as f32
        }
    })
}

/// Cheap external destruction bound ONLY (vehicles and airborne Imps).
/// A large value is not a survival certificate: hammers and chewing are handled
/// by simulate_range's shared plant state. Callers may reject an expired bound,
/// but must run the shared predictor before crediting a block or a squash hit.
pub fn puff_external_destruction_bound(scene: &Scene, target: Zombie, pos: Pos) -> i32 {
    external_destruction_bound(scene, target, Pad::new(pos, PuffShroom))
}
pub fn external_destruction_bound(scene: &Scene, target: Zombie, pad: Pad) -> i32 {
    let _perf = crate::perf::scope("prediction::external_destruction_bound");
    external_destruction_except(scene, Some(target.id), pad)
}
fn external_destruction_except(scene: &Scene, excluded: Option<ZombieId>, pad: Pad) -> i32 {
    let mut end = HORIZON;
    let rect = pad.rect();
    let vulnerable_bound = |at| {
        if pad.kind == Imitator && at >= pad.morph_at {
            HORIZON
        } else {
            at
        }
    };
    for z in scene.row_zombies(pad.pos.0) {
        if Some(z.id) == excluded {
            continue;
        }
        if matches!(z.kind, Z::Zomboni | Z::Catapult) {
            // Accept storage attrition at this specific site: lineup output plus
            // the awake Puff clears cars, and basketball targets the covered core.
            // Actual plant disappearance is observed on the next scene sample.
            if pad.kind == PuffShroom && sheltered_puff_site(pad.pos) {
                continue;
            }
            // Native ATTACKTYPE_DRIVE_OVER excludes these awake plants.
            // Basketball, hammers and explosions remain separate hazards.
            if matches!(
                pad.kind,
                Blover | IceShroom | DoomShroom | CherryBomb | Jalapeno | Squash
            ) {
                continue;
            }
            if z.kind == Z::Zomboni {
                // A Zamboni only slows down as x falls. Its present visible speed
                // is an upper bound, so this division bounds the earliest crush
                // without repeatedly stepping its entire future trajectory.
                if z.x + 143. >= (rect.left + 20) as f32 {
                    end = end.min(vulnerable_bound(
                        ((z.x - rect.right as f32) / (z.speed + 0.001).clamp(0.1, 0.25)).max(0.) as i32,
                    ));
                }
            } else {
                let phase_left = if matches!(z.phase, ZombiePhase::CatapultLaunching | ZombiePhase::CatapultReloading) {
                    scene
                        .projectiles
                        .cars
                        .iter()
                        .find(|c| c.z.id == z.id)
                        .map_or(0, |c| c.count.max(0))
                } else {
                    0
                };
                let moving_after = scene.protected_catapult_until(*z).max(z.frozen + phase_left);
                let speed = if z.phase == ZombiePhase::ZombieNormal {
                    z.speed
                } else {
                    max_speed(Z::Catapult)
                }
                .max(0.001);
                let work = ((z.x - (rect.right - 30) as f32) / speed).max(0.);
                let slow = (z.chilled - moving_after).max(0) as f32;
                let travel = if work * 2. <= slow {
                    work * 2.
                } else {
                    slow + work - slow * 0.5
                };
                end = end.min(vulnerable_bound(moving_after + travel.floor() as i32));
            }
        }
    }
    // Chewing is accounted for collectively in simulate_range, including later
    // arrivals, cold transitions and deaths. Do not give each eater a fresh 300HP.
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fact_plant(kind: PlantKind, pos: Pos, hp: i32) -> Plant {
        let pad = Pad::new(pos, kind);
        Plant {
            id: PlantId::from_raw(65540),
            kind,
            pos,
            hp,
            rect: pad.rect(),
            attack: pad.rect(),
            shooting: 0,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: 100,
        }
    }
    #[test]
    fn common_bite_geometry_does_not_use_snorkel_offsets() {
        let mut z = tail_red(255.);
        z.kind = Z::Normal;
        let rect = Pad::new((6, 3), PuffShroom).rect();
        assert!(!contact(z, 255., rect));
        assert!(contact(z, 230., rect));
        assert!(!contact(z, 231., rect));
    }
    #[test]
    fn airborne_child_only_bites_after_landing_and_shares_actual_hp() {
        let (mut scene, mut target) = football_group(1, 500., 0);
        target.frozen = 1000;
        scene.zombies.clear();
        scene.zombie_rows[5].clear();
        let child = Zombie {
            id: ZombieId::from_raw(65538),
            kind: Z::Imp,
            x: 100.,
            rect: ContactRect::new(136, 0, 178, 115),
            hp: 270,
            armor: 0,
            ..target
        };
        let child = Zombie { frozen: 0, ..child };
        scene.airborne.insert(child.id, (child, 100));
        let pad = Pad {
            complete_outcome: true,
            ..Pad::new((6, 2), PuffShroom)
        };
        let before = plant_outcome(&scene, &Forecast::default(), pad, 150);
        assert_eq!(before.impact, NEVER);
        let after = plant_outcome(&scene, &Forecast::default(), pad, 600);
        assert!(after.impact >= 390 && after.impact < 450, "{after:?}");
        let mut f = Forecast::default();
        f.add(120, EffectKind::Jalapeno, (6, 2), true);
        assert_eq!(plant_outcome(&scene, &f, pad, 600).impact, NEVER);
    }

    #[test]
    fn replacing_a_dying_puff_removes_its_old_hp_pool() {
        let (mut scene, z) = football_group(1, 100., 0);
        scene.plants.push(fact_plant(PuffShroom, (6, 2), 4));
        let mut pad = Pad::new((6, 2), PuffShroom);
        pad.replaces_occupant = true;
        let renewed = simulate_until(z, &scene, &Forecast::default(), Some(pad), 400);
        scene.plants.clear();
        let fresh = simulate_until(z, &scene, &Forecast::default(), Some(pad), 400);
        assert_eq!(renewed.impact, fresh.impact);
        assert!(renewed.impact > 250);
    }

    #[test]
    fn projected_wait_carries_consumed_and_surviving_body_hp() {
        let (mut scene, z) = football_group(1, 100., 0);
        let pos = (z.row, 2);
        scene.plants.push(fact_plant(PuffShroom, pos, 300));
        let (short, _) = project_row(&scene, &Forecast::default(), z.row, None, 50).unwrap();
        assert!(short.plants[0].hp < 300 && short.plants[0].hp > 0);
        let (long, _) = project_row(&scene, &Forecast::default(), z.row, None, 400).unwrap();
        assert!(long.plants.is_empty());
        let mut zero = Scene::fixture(vec![]);
        zero.plants.push(fact_plant(PuffShroom, pos, 0));
        let (still, _) = project_row(&zero, &Forecast::default(), z.row, None, 50).unwrap();
        assert_eq!(still.plants.len(), 1);
        assert_eq!(still.plants[0].hp, 0);
    }
    #[test]
    fn projected_wait_keeps_a_flying_balloon_without_rejecting_the_row() {
        let z = Zombie {
            kind: Z::Balloon,
            phase: ZombiePhase::BalloonFlying,
            ..tail_red(800.)
        };
        let (future, _) = project_row(&Scene::fixture(vec![z]), &Forecast::default(), 6, None, 100).unwrap();
        assert_eq!(future.zombies[0].phase, ZombiePhase::BalloonFlying);
        assert_eq!(future.zombies[0].id, z.id);
        assert!(future.zombies[0].x < z.x);
    }
    #[test]
    fn projected_wait_never_silently_erases_special_actors() {
        for kind in [Z::Catapult, Z::JackInTheBox] {
            let z = Zombie { kind, ..tail_red(500.) };
            assert!(project_row(&Scene::fixture(vec![z]), &Forecast::default(), z.row, None, 100).is_none());
        }
    }
    #[test]
    fn projected_wait_preserves_new_unconsumed_pad() {
        let scene = Scene::fixture(vec![]);
        let (future, _) = project_row(&scene, &Forecast::default(), 6, Some(Pad::new((6, 2), PuffShroom)), 50).unwrap();
        assert_eq!(future.plants.len(), 1);
        assert_eq!(future.plants[0].hp, 300);
    }
    #[test]
    fn airborne_ice_chills_without_freezing_or_damaging() {
        let z = Zombie {
            kind: Z::Imp,
            hp: 270,
            speed: max_speed(Z::Imp),
            rect: ContactRect::new(103, 0, 145, 115),
            ..tail_red(67.)
        };
        let scene = Scene::default();
        let mut f = Forecast::default();
        f.add(150, EffectKind::Ice, (5, 1), true);
        let before_birth = simulate_imp_arrival(z, &scene, &f, 160, 200);
        let airborne = simulate_imp_arrival(z, &scene, &f, 100, 200);
        assert_eq!(airborne.hp_at_end, 270);
        assert!(airborne.puff_deadline > before_birth.puff_deadline);
        let mut state = z;
        simulate_state(z, &scene, &f, None, 200, false, None, 200, 100, Some(&mut state), None);
        assert_eq!(state.frozen, 0);
        assert_eq!(state.chilled, 1949);
    }
    #[test]
    fn ice_activation_is_distinct_from_body_destruction() {
        let mut scene = Scene::fixture(vec![]);
        let mut ice = Pad::new((6, 2), IceShroom);
        ice.lifetime = 30;
        let out = plant_outcome(&scene, &Forecast::default(), ice, 100);
        assert_eq!(out.activated, NEVER);
        ice.lifetime = 100;
        assert_eq!(plant_outcome(&scene, &Forecast::default(), ice, 100).activated, 100);
        let z = Zombie {
            phase: ZombiePhase::GargantuarSmashing,
            progress: 0.63,
            ..tail_red(150.)
        };
        scene = Scene::fixture(vec![z]);
        let out = plant_outcome(&scene, &Forecast::default(), ice, 100);
        assert!(out.activated < 10, "{out:?}");
    }
    #[test]
    fn live_ice_hit_by_basketball_is_cancelled_not_activated() {
        let mut scene = Scene::fixture(vec![]);
        scene.plants.push(fact_plant(IceShroom, (6, 2), 50));
        scene.projectiles.balls.push(projectiles::Ball {
            row: 6,
            x: 140.,
            vx: 0.,
            height: 59.,
            vz: 2.,
            az: 0.,
            age: 30,
        });
        let mut f = Forecast::default();
        f.add(100, EffectKind::Ice, (6, 2), true);
        resolve_live_ice(&scene, &mut f);
        assert!(f.events.is_empty());
        scene.plants[0].hp = 75;
        let mut f = Forecast::default();
        f.add(100, EffectKind::Ice, (6, 2), true);
        resolve_live_ice(&scene, &mut f);
        assert_eq!(f.events.len(), 1);
        assert_eq!(f.events[0].at, 100);
    }
    #[test]
    fn ice_chill_extends_landing_animation_only_after_birth() {
        let mut f = Forecast::default();
        f.add(150, EffectKind::Ice, (5, 1), true);
        assert_eq!(imp_ready_at(0, &f, 100, 200), 225);
        assert_eq!(imp_ready_at(0, &f, 160, 200), 200);
        assert_eq!(imp_ready_at(2000, &Forecast::default(), 0, 200), 225);
    }

    #[test]
    fn scheduled_ice_body_waits_for_planting_and_preserves_activation_time() {
        let z = Zombie {
            id: ZombieId::from_raw(65537),
            kind: Z::GigaGargantuar,
            row: 6,
            wave: 1,
            carried: false,
            x: 310.,
            hp: 6000,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.37,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(293, 437, 418, 591),
        };
        let mut scene = Scene::fixture(vec![z]);
        let mut f = Forecast::default();
        f.add(150, EffectKind::Ice, (6, 4), false);
        let walking = simulate_until(z, &scene, &f, None, 180);
        f.events[0].body_at = Some(50);
        let stopped = simulate_until(z, &scene, &f, None, 180);
        assert_eq!(stopped.first_ice, 150);
        assert!(stopped.x_at_end > walking.x_at_end);
        scene.batch_walk = false;
        scene.batch_actions = false;
        scene.skip_frozen = false;
        let frames = simulate_until(z, &scene, &f, None, 180);
        assert_eq!(stopped.x_at_end.to_bits(), frames.x_at_end.to_bits());
        assert_eq!(stopped.first_ice, frames.first_ice);
        assert_eq!(stopped.hp_at_end, frames.hp_at_end);
    }
    #[test]
    fn idle_gloom_does_not_invent_a_ready_burst_after_missed_scan() {
        let mut z = tail_red(311.55472);
        z.row = 2;
        z.hp = 300;
        z.speed = 0.342301;
        z.progress = 0.862753;
        z.frozen = 0;
        z.chilled = 0;
        z.object = false;
        z.rect = ContactRect::new(294, 0, 419, 154);
        let mut scene = Scene::fixture(vec![z]);
        for (i, row, col, shooting, launch, left, right) in [
            (1, 2, 1, 0, 67, -40, 200),
            (2, 2, 2, 0, 117, 40, 280),
            (3, 3, 6, 38, 30, 360, 600),
        ] {
            scene.plants.push(Plant {
                id: PlantId::from_raw(65536 + i),
                kind: GloomShroom,
                pos: (row, col),
                hp: 300,
                rect: ContactRect::new(col * 80 - 35, 0, col * 80 + 34, 100),
                attack: ContactRect::new(left, 0, right, 100),
                shooting,
                launch,
                state: 0,
                phase_left: 0,
                effect: 0,
            });
        }
        let observed = simulate_until(z, &scene, &Forecast::default(), None, 700);
        assert!(observed.death > 700);
        assert_eq!(observed.hp_at_end, 20);
        for p in &mut scene.plants {
            p.launch = if p.shooting > 0 { p.shooting } else { 200 };
        }
        let invented = simulate_until(z, &scene, &Forecast::default(), None, 700);
        assert!(invented.death < 700);
    }
    fn observed_w12_hammer_scene() -> (Scene, Zombie, Pad) {
        let mut target = tail_red(289.33716);
        target.id = ZombieId::from_raw(102105092);
        target.progress = 0.39316896;
        target.speed = 0.35907197;
        let mut peer = tail_red(284.8412);
        peer.id = ZombieId::from_raw(103677982);
        peer.phase = ZombiePhase::GargantuarSmashing;
        peer.progress = 0.5527269;
        peer.speed = 0.35173708;
        let scene = Scene::fixture(vec![target, peer]);
        let mut pad = Pad::new((6, 3), PuffShroom);
        pad.order = 32;
        (scene, target, pad)
    }

    #[test]
    fn delayed_plant_preserves_shared_actor_projection_before_it_exists() {
        let (mut scene, z, mut pad) = observed_w12_hammer_scene();
        pad.plant_at = 100;
        pad.complete_outcome = true;
        let f = Forecast::default();
        scene.trim_peers = false;
        let full = simulate_until(z, &scene, &f, Some(pad), 400);
        scene.trim_peers = true;
        let trimmed = simulate_until(z, &scene, &f, Some(pad), 400);
        assert_eq!(full.trigger, trimmed.trigger);
        assert_eq!(full.impact, trimmed.impact);
        assert_eq!(full.x_at_end, trimmed.x_at_end);
    }
    #[test]
    fn delayed_pad_is_absent_until_its_planting_event() {
        let z = tail_red(285.);
        let scene = Scene::fixture(vec![z]);
        let f = Forecast::default();
        let mut pad = Pad::new((6, 3), PuffShroom);
        pad.complete_outcome = true;
        let now = simulate_until(z, &scene, &f, Some(pad), 400);
        pad.plant_at = 100;
        let later = simulate_until(z, &scene, &f, Some(pad), 400);
        assert!(now.trigger < 100, "{now:?}");
        assert!(later.trigger >= 100 && later.trigger < NEVER, "{later:?}");
        assert!(later.impact > now.impact, "now={now:?} later={later:?}");
        let before = simulate_until(z, &scene, &f, Some(pad), 99);
        let absent = simulate_until(z, &scene, &f, None, 99);
        assert_eq!(before.x_at_end, absent.x_at_end);
        assert_eq!(before.hp_at_end, absent.hp_at_end);
    }
    #[test]
    fn future_imp_left_of_column_four_cannot_walk_back_to_it() {
        let out = Outcome {
            throw_at: 100,
            throw_x: 530.,
            ..Outcome::default()
        };
        assert!(future_imp_reaches(out, Pad::new((5, 3), PuffShroom).rect(), 1000));
        assert!(!future_imp_reaches(out, Pad::new((5, 4), PuffShroom).rect(), 1000));
        assert!(!future_imp_reaches(out, Pad::new((5, 3), PuffShroom).rect(), 120));
    }
    #[test]
    fn w12_pending_peer_hammer_rejects_the_nominal_successful_block() {
        let (scene, target, pad) = observed_w12_hammer_scene();
        let f = Forecast::default();
        let nominal = simulate_range(target, &scene, &f, Some(pad), HORIZON, false, None, 0);
        assert!((75..=80).contains(&nominal.trigger), "{nominal:?}");
        let peer = scene.zombies[1];
        assert!(!contact(peer, peer.x, pad.rect()));
        let mut edge = pad;
        edge.offset_x = 4;
        assert!(contact(peer, peer.x, edge.rect()));
        let actual = simulate_until(target, &scene, &f, Some(edge), HORIZON);
        assert!((18..=21).contains(&actual.impact));
        assert_eq!(actual.trigger, NEVER);
        let robust = simulate_until(target, &scene, &f, Some(pad), HORIZON);
        assert_eq!(robust.trigger, NEVER);
        assert_eq!(robust.impact, actual.impact);
        assert!(robust.danger < NEVER, "must finish prediction after the pad disappears");
    }

    #[test]
    fn pending_hammer_check_does_not_reject_a_peer_killed_before_striking() {
        let (mut scene, target, pad) = observed_w12_hammer_scene();
        scene.zombies[1].hp = 100;
        let mut f = Forecast::default();
        f.add(1, EffectKind::Jalapeno, (6, 3), true);
        let after = simulate_until(target, &scene, &f, Some(pad), 400);
        assert!(after.trigger < NEVER, "the peer dies before it can consume the pad");
        scene.zombies[1].progress = 0.8;
        assert!(!pending_hammer_edge(&scene, target, pad, HORIZON));
        scene.zombies[1].phase = ZombiePhase::ZombieNormal;
        assert!(!pending_hammer_edge(&scene, target, pad, HORIZON));
    }

    #[test]
    fn immediate_giant_contact_has_no_interior_only_offset_interval() {
        // A geometry property, not a claim about all subsequent battle histories.
        let pad = Pad::new((6, 3), PuffShroom);
        for x in 100..400 {
            let z = tail_red(x as f32);
            let hit = |offset| {
                let mut p = pad;
                p.offset_x = offset;
                contact(z, z.x, p.rect())
            };
            assert_eq!((-5..=4).any(hit), hit(-5) || hit(4));
        }
    }
    #[test]
    fn disabled_provenance_changes_no_physical_prediction_and_is_explicitly_unknown() {
        let mut z = tail_red(220.);
        z.object = false;
        z.frozen = 0;
        z.chilled = 0;
        let mut scene = Scene::fixture(vec![z]);
        let f = Forecast::default();
        let pad = Some(Pad::new((1, 3), PuffShroom));
        let before = simulate(z, &scene, &f, pad);
        scene.track_evidence = false;
        let after = simulate(z, &scene, &f, pad);
        assert_eq!(
            (
                before.death,
                before.danger,
                before.trigger,
                before.impact,
                before.ash_mask,
                before.gloom_damage
            ),
            (
                after.death,
                after.danger,
                after.trigger,
                after.impact,
                after.ash_mask,
                after.gloom_damage
            )
        );
        assert_eq!(
            (
                before.x_at_end,
                before.hp_at_end,
                before.end_at,
                before.steps,
                before.skipped
            ),
            (
                after.x_at_end,
                after.hp_at_end,
                after.end_at,
                after.steps,
                after.skipped
            )
        );
        assert!(!after.known_through(1));
        assert_ne!(after.assumptions & ASSUME_UNTRACKED, 0);
    }

    #[test]
    fn evidence_marks_recovery_after_a_known_first_block() {
        let mut z = tail_red(220.);
        z.object = false;
        z.frozen = 0;
        z.chilled = 0;
        z.phase = ZombiePhase::ZombieNormal;
        let scene = Scene::fixture(vec![z]);
        let base = simulate_until(z, &scene, &Forecast::default(), None, 300);
        assert!(base.known_through(300));
        let after = simulate_until(z, &scene, &Forecast::default(), Some(Pad::new((1, 3), PuffShroom)), 300);
        assert!(after.trigger < 10);
        assert!(after.known_through(100));
        assert!(!after.known_through(300));
        assert_ne!(after.assumptions & ASSUME_SPEED, 0);
    }

    #[test]
    fn future_puff_contact_marks_random_offset_before_claiming_a_known_trigger() {
        let mut z = tail_red(330.);
        z.object = false;
        z.frozen = 0;
        z.chilled = 0;
        z.phase = ZombiePhase::ZombieNormal;
        let scene = Scene::fixture(vec![z]);
        let after = simulate_until(
            z,
            &scene,
            &Forecast::default(),
            Some(Pad::new((1, 3), PuffShroom)),
            1000,
        );
        assert!(after.trigger < NEVER);
        assert_ne!(after.assumptions & ASSUME_PUFF_OFFSET, 0);
        assert!(after.uncertain_at <= after.trigger);
    }

    #[test]
    fn cold_reach_pruning_never_discards_a_contact_in_the_frame_model() {
        let scene = Scene::default();
        for i in 0..1200 {
            let mut z = tail_red(180. + ((i * 71) % 550) as f32);
            z.object = false;
            z.frozen = (i * 37) % 401;
            z.chilled = (i * 83) % 2001;
            let mut forecast = Forecast::default();
            if i % 3 == 0 {
                forecast.events.push(Effect {
                    at: 120,
                    kind: EffectKind::Jalapeno,
                    pos: (z.row, 2),
                    certain: true,
                    body_at: None,
                    landing: None,
                    shape: None,
                });
            }
            let pad = Pad::new((z.row, i % 7 + 1), [PuffShroom, Blover, IceShroom][i as usize % 3]);
            if !could_reach_pad(z, pad, &forecast) {
                assert_eq!(probe_trigger(z, &scene, &forecast, pad).trigger, NEVER, "case {i}");
            }
        }
    }

    #[test]
    fn cherry_checkpoint_matches_the_same_trajectory_stopped_at_that_frame() {
        let scene = Scene::default();
        for at in [5, 50, 100, 151, 600, 1999, 2500, 5999] {
            let z = tail_red(700.);
            let mut forecast = Forecast::default();
            forecast.events.push(Effect {
                at: 340,
                kind: EffectKind::Ice,
                pos: (1, 1),
                certain: true,
                body_at: None,
                landing: None,
                shape: None,
            });
            let (full, point) = simulate_projected(z, &scene, &forecast, at);
            let stopped = simulate_until(z, &scene, &forecast, None, at);
            if full.death > at && full.danger >= at {
                assert_eq!(point.unwrap().to_bits(), stopped.x_at_end.to_bits());
            } else {
                assert!(point.is_none());
            }
        }
    }

    #[test]
    fn squash_damage_is_bound_to_the_proven_victim() {
        let mut target = tail_red(360.);
        target.frozen = 500;
        let mut left = tail_red(141.);
        left.id = ZombieId::from_raw(65538);
        left.hp = 1460;
        left.frozen = 500;
        let mut scene = Scene::default();
        scene.zombies = vec![target, left];
        scene.zombie_rows[5] = vec![0, 1];
        let forecast = Forecast::default();
        let mut predictor = squash::Predictor::default();
        let choice = predictor
            .evaluate(&scene, &forecast, (6, 4), &[target.id], false)
            .expect("right-side target is covered");
        assert!(predictor.guarantees_hit(choice, target.id));
        assert!(!predictor.guarantees_hit(choice, left.id));
        let mut pad = Pad::new((6, 4), Squash);
        pad.lifetime = 126;
        let unproven = simulate_until(target, &scene, &forecast, Some(pad), 183);
        assert!(!unproven.squash_hit);
        assert_eq!(unproven.hp_at_end, 6000);
        pad.squash_victim = Some(target.id);
        let hit = simulate_until(target, &scene, &forecast, Some(pad), 183);
        assert!(hit.squash_hit);
        assert_eq!(hit.hp_at_end, 4200);
        let missed = simulate_until(left, &scene, &forecast, Some(pad), 183);
        assert!(!missed.squash_hit);
        assert_eq!(missed.hp_at_end, 1460);
        assert_eq!(missed.death, NEVER);
    }

    #[test]
    fn an_unhit_actor_still_receives_the_squash_block() {
        let z = tail_red(260.);
        let scene = Scene::default();
        let forecast = Forecast::default();
        let mut pad = Pad::new((6, 3), Squash);
        pad.lifetime = 126;
        let blocked = simulate_until(z, &scene, &forecast, Some(pad), 183);
        let walking = simulate_until(z, &scene, &forecast, None, 183);
        assert_eq!(blocked.trigger, 1);
        assert!(!blocked.squash_hit);
        assert_eq!(blocked.hp_at_end, walking.hp_at_end);
    }

    #[test]
    fn same_frame_peer_strike_respects_actor_slot_order() {
        let mut target = tail_red(100.);
        target.object = false;
        let mut peer = target;
        peer.id = ZombieId::from_raw(65538);
        peer.phase = ZombiePhase::GargantuarSmashing;
        peer.progress = 0.639;
        let mut scene = Scene::default();
        scene.zombies.push(peer);
        scene.zombie_rows[5].push(0);
        let result = simulate_until(
            target,
            &scene,
            &Forecast::default(),
            Some(Pad::new((6, 2), PuffShroom)),
            300,
        );
        assert_eq!(result.impact, 1);
        assert_eq!(result.trigger, 1, "earlier target starts before the later strike");
        scene.zombies[0].id = ZombieId::from_raw(65536);
        let first = simulate_until(
            target,
            &scene,
            &Forecast::default(),
            Some(Pad::new((6, 2), PuffShroom)),
            300,
        );
        assert_eq!(first.impact, 1);
        assert_eq!(
            first.trigger, NEVER,
            "earlier peer removes the plant before target acts"
        );
    }

    #[test]
    fn recovering_hammer_cannot_restart_before_peer_removes_pad() {
        let mut target = tail_red(100.);
        target.object = false;
        target.phase = ZombiePhase::GargantuarSmashing;
        target.progress = 0.999;
        let mut peer = target;
        peer.id = ZombieId::from_raw(65538);
        peer.progress = 0.632;
        let scene = Scene::fixture(vec![peer]);
        let out = simulate_until(
            target,
            &scene,
            &Forecast::default(),
            Some(Pad::new((6, 2), PuffShroom)),
            300,
        );
        assert_eq!(out.impact, 2);
        assert_eq!(out.trigger, NEVER, "recovery frame is not a fresh attack frame");
    }

    #[test]
    fn walking_peer_can_destroy_a_future_block_before_target_arrives() {
        let mut target = tail_red(450.);
        target.object = false;
        let mut peer = tail_red(100.);
        peer.id = ZombieId::from_raw(65538);
        peer.object = false;
        let mut scene = Scene::default();
        let mut pad = Pad::new((6, 2), PuffShroom);
        pad.trigger_window = 2000;
        let alone = simulate_until(target, &scene, &Forecast::default(), Some(pad), 2000);
        assert!(alone.trigger < NEVER);
        scene.zombie_rows[5].push(scene.zombies.len());
        scene.zombies.push(peer);
        let shared = simulate_until(target, &scene, &Forecast::default(), Some(pad), 2000);
        assert_eq!(shared.trigger, NEVER);
        assert!(shared.impact < alone.trigger);
        scene.zombies[0].frozen = 500;
        let frozen_peer = simulate_until(target, &scene, &Forecast::default(), Some(pad), 2000);
        assert_eq!(frozen_peer.trigger, NEVER);
        assert!(frozen_peer.impact > shared.impact + 400);
    }

    #[test]
    fn a_peer_killed_before_its_hammer_does_not_destroy_the_pad() {
        let mut target = tail_red(450.);
        target.object = false;
        let mut peer = tail_red(100.);
        peer.id = ZombieId::from_raw(65538);
        peer.hp = 100;
        peer.object = false;
        let mut scene = Scene::default();
        scene.zombie_rows[5].push(scene.zombies.len());
        scene.zombies.push(peer);
        let mut pad = Pad::new((6, 2), PuffShroom);
        pad.trigger_window = 2000;
        let mut f = Forecast::default();
        f.events.push(Effect {
            at: 50,
            kind: EffectKind::Jalapeno,
            pos: (6, 2),
            certain: true,
            body_at: None,
            landing: None,
            shape: None,
        });
        assert!(simulate_until(target, &scene, &f, Some(pad), 2000).trigger < NEVER);
    }

    #[test]
    fn simultaneous_strikes_retarget_after_each_removal() {
        let mut target = tail_red(136.28);
        target.phase = ZombiePhase::GargantuarSmashing;
        target.progress = 0.639;
        let mut peer = tail_red(108.33);
        peer.id = ZombieId::from_raw(65538);
        peer.phase = target.phase;
        peer.progress = target.progress;
        let mut scene = Scene::fixture(vec![peer]);
        scene.plants.push(Plant {
            id: PlantId::from_raw(65537),
            kind: PuffShroom,
            pos: (6, 2),
            hp: 300,
            rect: ContactRect::new(127, 0, 187, 80),
            attack: ContactRect::new(0, 0, 0, 0),
            shooting: 0,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: 0,
        });
        let mut pad = Pad::new((6, 1), Blover);
        pad.order = 2;
        let out = simulate_until(target, &scene, &Forecast::default(), Some(pad), 2);
        assert_eq!(out.impact, 1, "target destroys col2; later peer must then hit col1");
        scene.zombies[0].id = ZombieId::from_raw(65536);
        let reversed = simulate_until(target, &scene, &Forecast::default(), Some(pad), 2);
        assert_eq!(
            reversed.impact, NEVER,
            "peer first destroys col2; target cannot reach col1"
        );
    }

    #[test]
    fn squash_uses_its_own_contact_rectangle() {
        let pad = Pad::new((6, 2), Squash);
        assert_eq!((pad.rect().left, pad.rect().right), (130, 190));
        let mut z = tail_red(70.);
        z.kind = Z::Football;
        assert!(contact(z, 90., pad.rect()));
        assert!(contact(z, 80., pad.rect()));
        assert!(!contact(z, 79., pad.rect()));
    }

    #[test]
    fn w20_nuke_waits_only_for_current_row_one_reds() {
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        world.with_backend(|b| {
            b.place_zombie(Z::GigaGargantuar, grid((1, 9))).unwrap();
            PeBackend::scope_backend(b, || {
                let mut scene = Scene::default();
                scene.sample(0, -10000);
                scene.zombies[0].wave = 20;
                let f = Forecast::default();
                let delayed = w20_doom_coverage(&scene, &f, 100);
                assert!(delayed > 100 && delayed < HORIZON, "{delayed}");
                let shape = grid_explosion_shape(GridExplosionKind::DoomShroom, grid((4, 7)));
                let z = scene.zombies[0];
                let too_early = simulate_until(z, &scene, &f, None, delayed - 2);
                assert!(!circle_hits_rect(shape.circle, z.rect_at(too_early.x_at_end)));
                // The explosion observes the preceding frame's movement result.
                let out = simulate_until(z, &scene, &f, None, delayed - 1);
                assert!(circle_hits_rect(shape.circle, z.rect_at(out.x_at_end)));
                scene.zombies[0].frozen = 200;
                assert!(w20_doom_coverage(&scene, &f, 100) > delayed);
                scene.zombies[0].wave = 19;
                assert_eq!(w20_doom_coverage(&scene, &f, 100), 100);
                scene.zombies[0].wave = 20;
                scene.zombies[0].kind = Z::Gargantuar;
                assert_eq!(w20_doom_coverage(&scene, &f, 100), 100);
            });
        });
    }

    fn tail_red(x: f32) -> Zombie {
        Zombie {
            id: ZombieId::from_raw(65537),
            kind: Z::GigaGargantuar,
            row: 6,
            wave: 20,
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
    fn football_group(count: u32, x: f32, chilled: i32) -> (Scene, Zombie) {
        let mut scene = Scene::default();
        for i in 0..count {
            let mut z = tail_red(x);
            z.id = ZombieId::from_raw(65537 + i);
            z.kind = Z::Football;
            z.hp = 270;
            z.armor = 1400;
            z.speed = 0.68;
            z.chilled = chilled;
            scene.zombie_rows[5].push(scene.zombies.len());
            scene.zombies.push(z);
        }
        let z = scene.zombies[0];
        (scene, z)
    }
    #[test]
    fn complete_action_does_not_mislabel_an_expired_unreachable_puff_as_safe() {
        let z = tail_red(400.);
        let scene = Scene::default();
        let f = Forecast::default();
        let base = simulate(z, &scene, &f, None);
        let mut pad = Pad::new((6, 9), PuffShroom);
        pad.lifetime = 10;
        let probe = simulate(z, &scene, &f, Some(pad));
        let after = simulate_action(z, &scene, &f, Some(pad));
        assert!(base.danger > 751 && base.danger < NEVER, "{base:?}");
        assert_eq!(probe.danger, NEVER);
        assert_eq!(probe.steps, 10);
        assert_eq!(after.danger, base.danger);
        assert_eq!(after.death, base.death);
        assert_eq!(after.hp_at_end, base.hp_at_end);
        assert!(after.steps > 751);
    }
    #[test]
    fn complete_ice_outcome_continues_after_nonblocking_activation() {
        let z = tail_red(400.);
        let scene = Scene::default();
        let mut f = Forecast::default();
        f.add(100, EffectKind::Ice, (6, 9), true);
        let base = simulate(z, &scene, &f, None);
        let after = simulate_action(z, &scene, &f, Some(Pad::new((6, 9), IceShroom)));
        assert!(after.danger > 751 && after.danger < NEVER, "{after:?}");
        assert_eq!(after.danger, base.danger);
        assert_eq!(after.death, base.death);
        assert_eq!(after.hp_at_end, base.hp_at_end);
        assert!(after.steps > 751);
    }
    #[test]
    fn landed_imp_walk_matches_native_pe() {
        use rsvz_pvz_emulator_backend::{PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        let id = world.with_backend(|b| {
            b.set_zombie_spawn_stopped(true).unwrap();
            let z = b.place_zombie(Z::Imp, grid((6, 5))).unwrap();
            b.zombie_id(z)
        });
        for _ in 0..5 {
            world.update_world().unwrap();
        }
        let scene = Scene::default();
        let z = world.with_backend(|b| {
            let native = b.zombie(id).unwrap().unwrap();
            Zombie {
                id,
                kind: Z::Imp,
                hp: b.zombie_hp(native),
                phase: b.zombie_phase(native).unwrap(),
                progress: b.zombie_reanim_anim_time(native).unwrap().unwrap(),
                speed: b.zombie_speed_x(native),
                rect: b.zombie_collision(native).unwrap(),
                ..tail_red(b.zombie_pos_x(native))
            }
        });
        assert_eq!(z.phase, ZombiePhase::ZombieNormal);
        assert!(z.speed >= 0.23 && z.speed <= 0.37);
        let predicted = simulate_until(z, &scene, &Forecast::default(), None, 50);
        for _ in 0..50 {
            world.update_world().unwrap();
        }
        let actual = world.with_backend(|b| b.zombie_pos_x(b.zombie(id).unwrap().unwrap()));
        assert!(
            (predicted.x50 - actual).abs() < 0.02,
            "predicted={} actual={actual}",
            predicted.x50
        );
    }
    #[test]
    fn imp_home_risk_is_removed_by_pad_or_timed_ice() {
        let mut z = tail_red(50.);
        z.kind = Z::Imp;
        z.hp = 270;
        z.rect = ContactRect::new(86, 0, 128, 115);
        let mut scene = Scene::default();
        scene.plants.push(Plant {
            id: PlantId::from_raw(65537),
            kind: GloomShroom,
            pos: (5, 2),
            hp: 300,
            rect: ContactRect::new(130, 0, 190, 80),
            attack: ContactRect::new(40, 0, 280, 240),
            shooting: 0,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: 0,
        });
        let f = Forecast::default();
        let before = simulate(z, &scene, &f, None);
        assert!(before.danger < NEVER, "{before:?}");
        let after = simulate(z, &scene, &f, Some(Pad::new((6, 1), PuffShroom)));
        assert!(
            after.trigger < NEVER && after.death < NEVER && after.danger == NEVER,
            "{after:?}"
        );
        let mut ice = Forecast::default();
        ice.add(100, EffectKind::Ice, (5, 1), true);
        let cold = simulate(z, &scene, &ice, None);
        assert!(cold.death < NEVER && cold.danger == NEVER, "{cold:?}");
        assert_eq!(max_speed(Z::Imp), 0.37);
    }
    #[test]
    fn emergency_ash_must_reach_the_left_target_before_home() {
        let cherry = |pos: Pos, pixel| Effect {
            at: 100,
            kind: EffectKind::Cherry,
            pos,
            certain: false,
            body_at: None,
            landing: None,
            shape: Some(rsvz::core::contact::grid_explosion_shape_from_pixel(
                GridExplosionKind::CherryBomb,
                grid(pos),
                pixel,
            )),
        };
        let (_, mut z) = football_group(1, -80., 0);
        let scene = Scene::default();
        z.row = 1;
        z.rect = ContactRect::new(-30, 40, 27, 155);
        let mut miss = Forecast::default();
        miss.events
            .push(cherry((2, 4), rsvz::core::model::PixelPos { x: 280, y: 165 }));
        let old = simulate_until(z, &scene, &miss, None, 400);
        assert!(old.danger < 400 && old.ash_hits == 0, "{old:?}");
        let mut hit = Forecast::default();
        hit.events
            .push(cherry((1, 1), rsvz::core::model::PixelPos { x: 40, y: 80 }));
        let rescued = simulate_until(z, &scene, &hit, None, 400);
        assert_eq!(rescued.death, 100);
        assert_eq!(rescued.danger, NEVER);
        z.x = -174.;
        let late = simulate_until(z, &scene, &hit, None, 400);
        assert!(late.danger < 100 && late.ash_hits == 0, "{late:?}");
    }

    #[test]
    fn future_imp_does_not_receive_ice_before_it_exists() {
        let mut z = tail_red(67.);
        z.kind = Z::Imp;
        z.hp = 270;
        z.rect = ContactRect::new(103, 0, 145, 115);
        let scene = Scene::default();
        let mut f = Forecast::default();
        f.add(100, EffectKind::Ice, (5, 1), false);
        let delayed = simulate_arrival(z, &scene, &f, 200);
        let plain = simulate(z, &scene, &Forecast::default(), None);
        assert_eq!(delayed.hp_at_end, 270);
        assert_eq!(delayed.danger, plain.danger + 199);
        assert!(delayed.puff_deadline > 200 && delayed.puff_deadline < 751);
        let mut parent = tail_red(401.02);
        parent.object = true;
        let mut pepper = Forecast::default();
        pepper.add(100, EffectKind::Jalapeno, (6, 2), false);
        parent.hp = 4000;
        let throw = simulate_until(parent, &scene, &pepper, None, 751);
        // This moving parent can walk left of the throw threshold before damage.
        assert_eq!(throw.throw_at, NEVER);
        parent.phase = ZombiePhase::GargantuarThrowing;
        let throwing = simulate_until(parent, &scene, &Forecast::default(), None, 751);
        assert!(throwing.throw_at > 0 && throwing.throw_at < 150);
        assert_eq!(throwing.throw_x, parent.x);
    }

    #[test]
    fn football_can_die_in_the_last_gloom_after_passing_minus_100() {
        let (mut scene, mut z) = football_group(1, -130., 0);
        z.hp = 100;
        z.armor = 0;
        z.rect = ContactRect::new(-80, 0, -23, 115);
        scene.plants.push(Plant {
            id: PlantId::from_raw(65537),
            kind: GloomShroom,
            pos: (5, 1),
            hp: 300,
            rect: ContactRect::new(50, 0, 110, 80),
            attack: ContactRect::new(-40, 0, 200, 240),
            shooting: 135,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: 0,
        });
        let out = simulate_until(z, &scene, &Forecast::default(), None, 50);
        assert_eq!(out.death, 9);
        assert_eq!(out.danger, NEVER);
        z.x = -175.;
        z.hp = 270;
        scene.plants.clear();
        assert_eq!(simulate_until(z, &scene, &Forecast::default(), None, 5).danger, 1);
    }
    #[test]
    fn vehicle_crush_does_not_remove_blover_or_awake_ice() {
        let (mut scene, z) = football_group(2, 120., 0);
        scene.zombies[1].kind = Z::Zomboni;
        scene.zombies[1].x = 170.;
        scene.zombies[1].speed = 0.1;
        assert_eq!(puff_external_destruction_bound(&scene, z, (6, 2)), 0);
        for kind in [Blover, IceShroom, Jalapeno, Squash] {
            assert_eq!(external_destruction_bound(&scene, z, Pad::new((6, 2), kind)), HORIZON);
        }
    }
    #[test]
    fn just_crossed_hammer_phase_can_still_hit_on_the_next_native_frame() {
        let mut z = tail_red(200.);
        z.phase = ZombiePhase::GargantuarSmashing;
        z.progress = 0.642;
        let pad = Pad::new((6, 2), Blover);
        let out = simulate_until(z, &Scene::default(), &Forecast::default(), Some(pad), 60);
        assert_eq!(out.impact, 1);
        z.progress = 0.8;
        let out = simulate_until(z, &Scene::default(), &Forecast::default(), Some(pad), 20);
        assert_eq!(out.impact, NEVER, "a genuinely completed hit must not be repeated");
    }
    #[test]
    fn footballs_share_one_puff_health_pool_including_later_arrivals() {
        let pad = Pad::new((6, 3), PuffShroom);
        let f = Forecast::default();
        let (scene, z) = football_group(1, 200., 0);
        assert_eq!(simulate_until(z, &scene, &f, Some(pad), 400).impact, 297);
        let (scene, z) = football_group(4, 200., 0);
        assert_eq!(simulate_until(z, &scene, &f, Some(pad), 400).impact, 73);
        let (mut scene, z) = football_group(2, 200., 0);
        scene.zombies[1].x = 240.;
        let arrived = simulate_until(z, &scene, &f, Some(pad), 400).impact;
        assert!(arrived > 149 && arrived < 297, "later peer bite: {arrived}");
        let (scene, z) = football_group(4, 200., 1000);
        assert_eq!(simulate_until(z, &scene, &f, Some(pad), 400).impact, 145);
    }
    #[test]
    fn blover_and_awake_ice_are_not_eaten_by_a_football_group() {
        let (scene, z) = football_group(8, 200., 0);
        for kind in [Blover, IceShroom, Jalapeno] {
            let out = simulate_until(z, &scene, &Forecast::default(), Some(Pad::new((6, 3), kind)), 250);
            assert_eq!(out.trigger, 1);
            assert_eq!(out.impact, NEVER, "{kind:?} was eaten");
            assert_eq!(out.x50, z.x);
        }
    }
    #[test]
    fn dead_or_frozen_companions_do_not_continue_eating() {
        let (mut scene, z) = football_group(4, 200., 0);
        scene.zombies[1].hp = 80;
        scene.zombies[1].armor = 0;
        scene.zombies[2].frozen = 400;
        scene.zombies[3].frozen = 400;
        let out = simulate_until(z, &scene, &Forecast::default(), Some(Pad::new((6, 3), PuffShroom)), 400);
        assert_eq!(out.impact, 297);
    }
    #[test]
    fn shared_bites_do_not_depend_on_peer_order_or_frozen_skip() {
        let (mut scene, z) = football_group(4, 200., 0);
        scene.zombies[1].x = 215.;
        scene.zombies[2].x = 230.;
        scene.zombies[3].x = 245.;
        let mut forecast = Forecast::default();
        forecast.add(50, EffectKind::Ice, (5, 1), true);
        let pad = Some(Pad::new((6, 3), PuffShroom));
        let before = simulate_until(z, &scene, &forecast, pad, 1500);
        scene.zombie_rows[5].reverse();
        scene.skip_frozen = false;
        let after = simulate_until(z, &scene, &forecast, pad, 1500);
        assert!(super::super::planner::same_outcome(before, after));
        assert!(before.impact > 400 && before.impact < 1000);
    }
    #[test]
    fn finished_bite_restarts_football_walk_at_fastest_new_speed() {
        let (scene, mut z) = football_group(1, 200., 0);
        z.progress = 0.8;
        z.speed = 0.66;
        let mut pad = Pad::new((6, 3), PuffShroom);
        pad.hp = 4;
        let out = simulate_until(z, &scene, &Forecast::default(), Some(pad), 5);
        assert_eq!(out.impact, 1);
        let mut expected_x = z.x;
        let mut progress = 0.;
        for _ in 0..4 {
            walk_step(&mut expected_x, &mut progress, 0.68, false, false);
        }
        assert_eq!(out.x5.to_bits(), expected_x.to_bits());
    }
    #[test]
    fn storage_waits_for_pending_throws_and_visible_imps() {
        let mut scene = Scene::default();
        let mut z = tail_red(700.);
        z.object = true;
        z.hp = 3200;
        scene.zombies.push(z);
        scene.zombie_rows[5].push(0);
        let f = Forecast::default();
        assert!(
            prestore_clear(&scene, (6, 3), 751, &f),
            "no damage or throw during the storage window"
        );
        scene.zombies[0].hp = 2900;
        assert!(!prestore_clear(&scene, (6, 3), 751, &f));
        scene.zombies[0].object = false;
        scene.zombies[0].phase = ZombiePhase::GargantuarThrowing;
        assert!(
            prestore_clear(&scene, (6, 3), 751, &f),
            "the child has already been released; visible children are checked separately"
        );
        scene.zombies[0].phase = ZombiePhase::ZombieNormal;
        assert!(prestore_clear(&scene, (6, 3), 751, &f));
        scene.zombies[0].kind = Z::Imp;
        assert!(
            prestore_clear(&scene, (6, 3), 751, &f),
            "grounded Imp damage is handled by the shared simulation"
        );
        scene.zombies[0].phase = ZombiePhase::ImpLanding;
        assert!(!prestore_clear(&scene, (6, 3), 751, &f));
    }

    #[test]
    fn slow_zomboni_does_not_erase_a_useful_free_pad_window() {
        let mut scene = Scene::default();
        let target = tail_red(220.);
        let mut car = tail_red(400.);
        car.id = ZombieId::from_raw(65538);
        car.kind = Z::Zomboni;
        car.speed = 0.1;
        scene.zombies.push(car);
        scene.zombie_rows[5].push(0);
        assert!(puff_external_destruction_bound(&scene, target, (6, 2)) >= 2000);
        scene.zombies[0].x = 0.;
        assert_eq!(puff_external_destruction_bound(&scene, target, (6, 2)), HORIZON);
    }
    #[test]
    fn vehicle_storage_exception_is_only_for_inner_row_col_three_puff() {
        for row in [1, 2, 5, 6] {
            for col in [3, 4, 6] {
                for kind in [Z::Zomboni, Z::Catapult] {
                    let mut scene = Scene::default();
                    let mut target = tail_red(220.);
                    target.row = row;
                    let mut car = target;
                    car.id = ZombieId::from_raw(65538);
                    car.kind = kind;
                    car.x = (col * 80 + 30) as f32;
                    car.speed = 0.25;
                    scene.zombies.push(car);
                    scene.zombie_rows[(row - 1) as usize].push(0);
                    let exempt = matches!(row, 2 | 5) && col == 3;
                    assert_eq!(prestore_clear(&scene, (row, col), 751, &Forecast::default()), exempt);
                    assert_eq!(
                        puff_external_destruction_bound(&scene, target, (row, col)) == HORIZON,
                        exempt
                    );
                    let flower = Pad::new((row, col), Sunflower);
                    assert!(external_destruction_bound(&scene, target, flower) < HORIZON);
                    scene.zombies[0].kind = Z::Imp;
                    scene.zombies[0].phase = ZombiePhase::ImpLanding;
                    scene.zombies[0].rect = ContactRect::new(car.x as i32 + 36, 0, car.x as i32 + 78, 115);
                    assert!(!prestore_clear(&scene, (row, col), 751, &Forecast::default()));
                    assert_eq!(
                        puff_external_destruction_bound(&scene, target, (row, col)),
                        HORIZON,
                        "landing is a future eater, not an unconditional plant deletion"
                    );
                }
            }
        }
    }
    #[test]
    fn zomboni_motion_matches_native_probe_and_ignores_ice() {
        let mut x = 800.;
        let mut speed = 0.25;
        for dt in 1..=2500 {
            zomboni_step(&mut x, &mut speed);
            if dt == 1000 {
                assert!((x - 570.51215).abs() < 0.6);
            }
        }
        assert!((x - 373.50986).abs() < 0.6);
        let mut z = tail_red(500.);
        z.kind = Z::Zomboni;
        z.hp = 1350;
        z.speed = 0.15;
        let scene = Scene::default();
        let plain = simulate_until(z, &scene, &Forecast::default(), None, 500);
        let mut f = Forecast::default();
        f.add(1, EffectKind::Ice, (5, 1), true);
        let iced = simulate_until(z, &scene, &f, None, 500);
        assert_eq!(plain.x50.to_bits(), iced.x50.to_bits());
        assert_eq!(plain.hp_at_end, iced.hp_at_end);
    }
    #[test]
    fn stationary_catapult_requires_observed_birth_and_a_protected_permanent_target() {
        let mut scene = Scene::default();
        scene.now = 2200;
        let mut z = tail_red(650.);
        z.kind = Z::Catapult;
        z.row = 2;
        z.phase = ZombiePhase::CatapultReloading;
        let mut p = Plant {
            id: PlantId::from_raw(65536),
            kind: GloomShroom,
            pos: (2, 1),
            hp: 300,
            rect: ContactRect::new(50, 0, 110, 80),
            attack: ContactRect::new(0, 0, 0, 0),
            shooting: 0,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: 0,
        };
        scene.plants.push(p);
        scene.catapult_seen.insert(z.id, (0, 2200));
        assert_eq!(scene.protected_catapult_until(z), 0);
        p.kind = UmbrellaLeaf;
        p.pos = (3, 2);
        scene.plants.push(p);
        assert_eq!(scene.protected_catapult_until(z), 3500);
        scene.now = 6000;
        assert_eq!(scene.protected_catapult_until(z), 0);
        scene.now = 2200;
        scene.catapult_seen.clear();
        assert_eq!(scene.protected_catapult_until(z), 0);
    }
    #[test]
    fn ash_economy_removes_the_canceled_plan_but_keeps_planted_damage() {
        let mut planner = planner::Planner::default();
        let mut z = tail_red(260.);
        z.hp = 1700;
        z.rect.top = 500;
        z.rect.bottom = 654;
        planner.scene.zombies.push(z);
        planner.scene.zombie_rows[5].push(0);
        let mut f = Forecast::default();
        f.add(100, EffectKind::Jalapeno, (6, 3), false);
        assert!(!planner.ash_unneeded(&f, EffectKind::Jalapeno, &[6]));
        f.events[0].certain = true;
        assert!(planner.ash_unneeded(&f, EffectKind::Jalapeno, &[6]));
        planner.scene.zombies[0].kind = Z::Catapult;
        assert!(!planner.ash_unneeded(&f, EffectKind::Jalapeno, &[6]));
        planner.scene.zombies[0].kind = Z::Zomboni;
        assert!(!planner.ash_unneeded(&f, EffectKind::Jalapeno, &[6]));
        planner.scene.zombies[0].kind = Z::Football;
        assert!(!planner.ash_unneeded(&f, EffectKind::Jalapeno, &[6]));
    }
    #[test]
    fn squash_can_be_planted_before_a_target_is_in_lock_range() {
        let mut scene = Scene::default();
        let z = tail_red(800.);
        scene.zombies.push(z);
        scene.zombie_rows[5].push(0);
        let mut predictor = squash::Predictor::default();
        let forecast = Forecast::default();
        assert!(predictor.evaluate(&scene, &forecast, (6, 8), &[z.id], false).is_none());
        let choice = predictor
            .prestore(&scene, &forecast, (6, 8), &[z.id])
            .expect("safe waiting squash");
        assert!(choice.wait > 0 && choice.wait <= 500);
        assert_eq!(choice.guaranteed, 1);
        let mut changing = Forecast::default();
        changing.add(1, EffectKind::Ice, (1, 1), true);
        assert!(predictor.prestore(&scene, &changing, (6, 8), &[z.id]).is_none());
    }
    #[test]
    fn pepper_disappearance_does_not_cancel_hammer_recovery() {
        let scene = Scene::default();
        let z = tail_red(260.);
        let mut f = Forecast::default();
        f.add(100, EffectKind::Jalapeno, (6, 3), true);
        let pad = Pad::new((6, 3), Jalapeno);
        let mut prefix = MotionPrefix::default();
        let out = simulate_range(z, &scene, &f, Some(pad), 250, false, Some(&mut prefix), 0);
        assert_eq!(out.trigger, 1);
        assert_eq!(out.impact, NEVER);
        assert_eq!(out.ash_hits, 1);
        assert_eq!(prefix.points[40].unwrap().x, 260.);
        assert!(prefix.points[50].unwrap().x < 260.);
    }
    #[test]
    fn prestore_can_wait_beyond_one_seed_cooldown() {
        let scene = Scene::default();
        let z = tail_red(500.);
        let f = Forecast::default();
        let mut pad = Pad::new((6, 3), PuffShroom);
        assert_eq!(probe_trigger(z, &scene, &f, pad).trigger, NEVER);
        pad.trigger_window = 2500;
        let out = probe_trigger(z, &scene, &f, pad);
        assert!((752..=2500).contains(&out.trigger));
        assert_eq!(out.steps, out.trigger);
    }

    #[test]
    fn cached_walk_math_preserves_every_frame_bit_for_bit() {
        let mut state = 187_u32;
        let mut next = || {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            state
        };
        for giant in [false, true] {
            for _ in 0..256 {
                let mut walker = WalkRate::new(giant);
                let mut x = 800.;
                let mut phase = (next() % 10000) as f32 / 10000.;
                let (mut expected_x, mut expected_phase) = (x, phase);
                let mut speed = 0.;
                for dt in 0..3000 {
                    if dt % 211 == 0 {
                        speed = if giant { 0.23 } else { 0.66 } + (next() % 1401) as f32 / 10000.;
                    }
                    let chilled = dt % 1300 < 800;
                    if dt % 499 == 0 {
                        phase = 0.;
                        expected_phase = 0.;
                    }
                    walker.step(&mut x, &mut phase, speed, chilled);
                    walk_step(&mut expected_x, &mut expected_phase, speed, chilled, giant);
                    assert_eq!(x.to_bits(), expected_x.to_bits(), "giant={giant} dt={dt}");
                    assert_eq!(phase.to_bits(), expected_phase.to_bits(), "giant={giant} dt={dt}");
                }
            }
        }
    }

    #[test]
    fn frozen_interval_skips_match_frame_by_frame_predictions() {
        let mut seed = 1741_u32;
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed
        };
        let mut skipped = 0;
        for index in 0..2000 {
            let mut scene = Scene::default();
            // Isolate frozen skipping from the independently selectable walking fast path.
            scene.batch_walk = false;
            for (slot, col) in [2, 4].into_iter().enumerate() {
                scene.plants.push(Plant {
                    id: PlantId::from_raw(65536 + slot as u32),
                    kind: GloomShroom,
                    pos: (5, col),
                    hp: 300,
                    rect: Pad::new((5, col), PuffShroom).rect(),
                    attack: ContactRect::new(col * 80 - 200, 400, col * 80 + 120, 550),
                    shooting: (next() % 201) as i32,
                    launch: 200,
                    state: 0,
                    phase_left: 0,
                    effect: 0,
                });
            }
            if index % 3 == 0 {
                scene.plants.push(Plant {
                    id: PlantId::from_raw(65538),
                    kind: PuffShroom,
                    pos: (5, 3),
                    hp: 120,
                    rect: Pad::new((5, 3), PuffShroom).rect(),
                    attack: ContactRect::new(0, 0, 0, 0),
                    shooting: 0,
                    launch: 200,
                    state: 0,
                    phase_left: 0,
                    effect: 0,
                });
            }
            let mut forecast = Forecast::default();
            for (kind, at, pos) in [
                (EffectKind::Ice, 40 + next() % 400, (5, 1)),
                (EffectKind::Jalapeno, 100 + next() % 1300, (5, 1)),
                (EffectKind::Cherry, 500 + next() % 1500, (5, 4)),
                (EffectKind::Doom, 1000 + next() % 2000, (4, 7)),
            ] {
                let shape = match kind {
                    EffectKind::Doom | EffectKind::Cherry => {
                        Some(rsvz::core::contact::grid_explosion_shape_from_pixel(
                            if kind == EffectKind::Doom {
                                GridExplosionKind::DoomShroom
                            } else {
                                GridExplosionKind::CherryBomb
                            },
                            grid(pos),
                            rsvz::core::model::PixelPos { x: pos.1 * 80, y: 440 },
                        ))
                    }
                    _ => None,
                };
                forecast.events.push(Effect {
                    at: at as i32,
                    kind,
                    pos,
                    certain: false,
                    shape,
                    body_at: None,
                    landing: None,
                });
            }
            let giant = index % 3 != 0;
            let x = 180. + (next() % 36000) as f32 / 100.;
            let z = Zombie {
                id: ZombieId::from_raw(65536),
                kind: if giant { Z::GigaGargantuar } else { Z::Football },
                row: 5,
                wave: 1,
                carried: false,
                x,
                hp: if giant { 100 + (next() % 5901) as i32 } else { 270 },
                armor: if giant { 0 } else { 1400 },
                phase: match index % 4 {
                    0 if giant => ZombiePhase::GargantuarSmashing,
                    1 if giant => ZombiePhase::GargantuarThrowing,
                    _ => ZombiePhase::ZombieNormal,
                },
                progress: (next() % 10000) as f32 / 10000.,
                speed: if giant {
                    0.23 + (next() % 1401) as f32 / 10000.
                } else {
                    0.68
                },
                frozen: if index % 5 == 0 { 160 } else { 0 },
                chilled: if index % 2 == 0 { 900 } else { 0 },
                object: index % 2 == 0,
                eating: false,
                rect: ContactRect::new(x as i32 - 17, 400, x as i32 + 108, 554),
            };
            let mut pad = Pad::new(
                (5, 1 + (next() % 6) as i32),
                [PuffShroom, IceShroom, Imitator][index % 3],
            );
            pad.lifetime = pad.lifetime.min([100, 420, 751, HORIZON][index % 4]);
            for extra in [None, Some(pad)] {
                let horizon = [5, 50, 100, 420, 751, HORIZON][index % 6];
                let trigger_only = extra.is_some() && index % 4 == 0;
                let mut original_prefix = MotionPrefix::default();
                let mut skipped_prefix = MotionPrefix::default();
                scene.skip_frozen = false;
                scene.batch_walk = false;
                let original = simulate_range(
                    z,
                    &scene,
                    &forecast,
                    extra,
                    horizon,
                    trigger_only,
                    Some(&mut original_prefix),
                    0,
                );
                scene.skip_frozen = true;
                scene.batch_walk = true;
                let optimized = simulate_range(
                    z,
                    &scene,
                    &forecast,
                    extra,
                    horizon,
                    trigger_only,
                    Some(&mut skipped_prefix),
                    0,
                );
                assert!(
                    planner::same_outcome(original, optimized),
                    "{index}: {original:?} != {optimized:?}"
                );
                assert_eq!(original.steps, optimized.steps);
                assert_eq!(original_prefix.first_pad_change, skipped_prefix.first_pad_change);
                assert_eq!(original_prefix.first_contact, skipped_prefix.first_contact);
                for (a, b) in original_prefix.points.into_iter().zip(skipped_prefix.points) {
                    assert_eq!(a.is_some(), b.is_some());
                    if let (Some(a), Some(b)) = (a, b) {
                        assert!(a.matches(Zombie {
                            x: b.x,
                            progress: b.progress,
                            speed: b.speed,
                            frozen: b.frozen,
                            chilled: b.chilled,
                            hp: b.health,
                            armor: 0,
                            phase: b.phase,
                            object: b.object,
                            ..z
                        }));
                        assert_eq!(a.gloom_damage, b.gloom_damage);
                    }
                }
                let without_capture = simulate_range(z, &scene, &forecast, extra, horizon, trigger_only, None, 0);
                assert!(planner::same_outcome(original, without_capture));
                assert_eq!(original.steps, without_capture.steps);
                skipped += without_capture.skipped;
            }
        }
        assert!(skipped > 10000);
    }

    #[test]
    fn captured_motion_stops_reuse_at_a_predicted_pad_removal() {
        let mut scene = Scene::default();
        scene.plants.push(Plant {
            id: PlantId::from_raw(65536),
            kind: PuffShroom,
            pos: (5, 3),
            hp: 300,
            rect: ContactRect::new(210, 0, 270, 100),
            attack: ContactRect::new(0, 0, 0, 0),
            shooting: 0,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: 0,
        });
        let z = Zombie {
            id: ZombieId::from_raw(65537),
            kind: Z::GigaGargantuar,
            row: 5,
            wave: 1,
            carried: false,
            x: 230.,
            hp: 6000,
            armor: 0,
            phase: ZombiePhase::GargantuarSmashing,
            progress: 0.63,
            speed: 0.37,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(213, 0, 338, 154),
        };
        let mut prefix = MotionPrefix::default();
        simulate_captured(z, &scene, &Forecast::default(), &mut prefix);
        assert!((1..=5).contains(&prefix.first_pad_change));
        assert!(prefix.points[1].is_some());
        // Matching actor motion alone is insufficient if the observed pad is still present.
        assert!(5 >= prefix.first_pad_change);
    }

    #[test]
    fn rejected_swept_contacts_never_trigger_in_the_frame_predictor() {
        let scene = Scene::default();
        let empty_forecast = Forecast::default();
        let mut forecast = Forecast::default();
        // An unfreeze inside the query must not invalidate the necessary reach bound.
        forecast.events.push(Effect {
            at: 51,
            kind: EffectKind::Jalapeno,
            pos: (5, 2),
            certain: false,
            body_at: None,
            landing: None,
            shape: None,
        });
        let mut state = 17_u32;
        let mut next = || {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            state
        };
        let mut rejected = 0;
        for _ in 0..6000 {
            let giant = next() & 2 == 0;
            let phase = match next() % 3 {
                0 if giant => ZombiePhase::GargantuarSmashing,
                1 if giant => ZombiePhase::GargantuarThrowing,
                _ => ZombiePhase::ZombieNormal,
            };
            let x = (next() % 104000) as f32 * 0.01 - 200.;
            let z = Zombie {
                id: ZombieId::from_raw(65536),
                kind: if giant { Z::GigaGargantuar } else { Z::Football },
                row: 5,
                wave: 1,
                carried: false,
                x,
                hp: if giant { 6000 } else { 270 },
                armor: if giant { 0 } else { 1400 },
                phase,
                progress: (next() % 10000) as f32 * 0.0001,
                speed: if giant { 0.37 } else { 0.68 },
                frozen: (next() % 401) as i32,
                chilled: (next() % 2001) as i32,
                object: true,
                eating: false,
                rect: ContactRect::new(x as i32, 0, x as i32 + 100, 100),
            };
            let mut pad = Pad::new((5, (next() % 9) as i32 + 1), PuffShroom);
            pad.lifetime = [1, 100, 250, 420, 751][(next() % 5) as usize];
            pad.offset_x = (next() % 10) as i32 - 5;
            if !could_contact_pad(z, pad) {
                rejected += 1;
                let plan = if giant { &forecast } else { &empty_forecast };
                assert_eq!(probe_trigger(z, &scene, plan, pad).trigger, NEVER, "{z:?} {pad:?}");
            }
            if giant && pad.offset_x == 0 {
                let mut prefix = MotionPrefix::default();
                simulate_captured(z, &scene, &forecast, &mut prefix);
                if prefix.first_contact[(pad.pos.1 - 1) as usize] > pad.lifetime.min(751) {
                    assert_eq!(probe_trigger(z, &scene, &forecast, pad).trigger, NEVER, "{z:?} {pad:?}");
                }
            }
        }
        assert!(rejected > 1000);
    }
}
