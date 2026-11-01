//! Rolling first-pad decisions, plus the six-flower row-five damage-gap squash.
use super::*;
use diagnostics::Condition as C;
use prediction::{Forecast, NEVER, Outcome, Pad, Scene, Zombie};
use rsvz::core::model::ZombieId;
use std::cell::Cell;
use std::collections::HashMap;
mod blover;
mod catapult;
pub(super) mod cherry;
mod cherry_guard;
mod combination;
mod extension;
mod imp;
mod nuke;
mod pepper;
mod retention;
mod tuning;
mod window;

#[derive(Clone, Copy)]
struct Checked {
    observation: u64,
    forecast: u64,
    outcome: Option<Outcome>,
}
struct Cached {
    row: i32,
    at: i32,
    serial: u32,
    forecast: u64,
    outcome: Outcome,
    environment: u64,
    stable_environment: u64,
    prefix: prediction::MotionPrefix,
    checked: Cell<Option<Checked>>,
}
pub struct Planner {
    pub scene: Scene,
    tuning: tuning::Tuning,
    pub(super) cherry_guard: cherry::State,
    extension: extension::State,
    cache: HashMap<ZombieId, Cached>,
    pub last_blover_wave: i32,
    gap_logged: HashMap<ZombieId, bool>,
    imp_risk_logged: HashMap<ZombieId, ()>,
    imp_padding: bool,
    imp_puff_hold: Option<imp::Reservation>,
    next_imp_reserve: i32,
    next_imp_assess: i32,
    required: Vec<ZombieId>,
    last_ice: i32,
    squash: squash::Predictor,
    reserve_squash5: bool,
    next_ash_pad: i32,
    next_catapult_bait: i32,
    next_support_squash: i32,
    squash5_deferred: bool,
    ash_births: u32,
    event_cache: u32,
    memo_checks: bool,
    pad_base_first: bool,
    cold_reach: bool,
    approximate_reuse: i32,
    pad_retry_gap: i32,
    pad_retry: HashMap<(i32, bool), (i32, u32)>,
    cache_audit_id: u32,
    next_tail: i32,
    tail_five_used: bool,
    tail_two_used: bool,
    next_carry: i32,
    next_combo_check: i32,
    squash_group_wait: Option<(ZombieId, i32)>,
    nuke_hold: Option<nuke::Retention>,
    nuke_debts: Vec<(ZombieId, i32)>,
    next_nuke_retention: i32,
    nuke_exclude47: bool,
    current_nuke: Option<(i32, i32)>,
    next_puff_idle_log: i32,
    puff_window_hold: Option<i32>,
    economy_victims: Vec<(ZombieId, i32)>,
    economy_rejection: [i64; 8],
    red_pressure_cache: Cell<Option<(u64, u64, bool)>>,
    omitted_cherry_births: u32,
    blover_handled_births: u32,
    blover_watch_birth: Option<i32>,
    blover_reserved_birth: Option<i32>,
    blover_wait_logged: [u8; 21],
}
impl Default for Planner {
    fn default() -> Self {
        let extension = extension::State::default();
        let mut scene = Scene::default();
        scene.track_evidence = extension.requires_evidence();
        Self {
            scene,
            tuning: tuning::Tuning::default(),
            cherry_guard: cherry::State::default(),
            extension,
            cache: HashMap::with_capacity(512),
            last_blover_wave: 0,
            gap_logged: HashMap::with_capacity(512),
            imp_risk_logged: HashMap::with_capacity(32),
            imp_padding: setting("SIX_IMP_PAD", 1) != 0,
            imp_puff_hold: None,
            next_imp_reserve: 0,
            next_imp_assess: 0,
            required: Vec::with_capacity(128),
            last_ice: -99999,
            squash: squash::Predictor::default(),
            reserve_squash5: false,
            next_ash_pad: 0,
            next_catapult_bait: 0,
            next_support_squash: 0,
            squash5_deferred: false,
            ash_births: 0,
            event_cache: event_cache_mode(),
            memo_checks: setting("SIX_MEMO_CHECKS", 1) != 0,
            pad_base_first: setting("SIX_PAD_BASE_FIRST", 1) != 0,
            cold_reach: setting("SIX_COLD_REACH", 1) != 0,
            approximate_reuse: setting("SIX_APPROX_REUSE", 200).min(200) as i32,
            pad_retry_gap: setting("SIX_PAD_RETRY", 25) as i32,
            pad_retry: HashMap::new(),
            cache_audit_id: setting("SIX_CACHE_AUDIT_ID", 0),
            next_tail: 0,
            tail_five_used: false,
            tail_two_used: false,
            next_carry: 0,
            next_combo_check: 0,
            squash_group_wait: None,
            nuke_hold: None,
            nuke_debts: Vec::with_capacity(32),
            next_nuke_retention: 0,
            nuke_exclude47: false,
            current_nuke: None,
            next_puff_idle_log: 0,
            puff_window_hold: None,
            economy_victims: Vec::with_capacity(64),
            economy_rejection: [0; 8],
            red_pressure_cache: Cell::new(None),
            omitted_cherry_births: 0,
            blover_handled_births: 0,
            blover_watch_birth: None,
            blover_reserved_birth: None,
            blover_wait_logged: [0; 21],
        }
    }
}
fn columns(row: i32) -> &'static [i32] {
    match row {
        1 => &[2, 1],
        6 => &[3, 2, 1],
        _ => &[3, 4, 5, 6],
    }
}
#[cfg(test)]
mod candidate_tests {
    use super::*;
    #[test]
    fn foot_candidates_match_all_columns_and_both_puff_offsets() {
        use rsvz::core::model::ContactRect;
        for x in -20..850 {
            let z = Zombie {
                age: 0,
                id: ZombieId::from_raw(65537),
                kind: Z::GigaGargantuar,
                row: 2,
                wave: 20,
                carried: false,
                x: x as f32,
                hp: 6000,
                armor: 0,
                phase: ZombiePhase::ZombieNormal,
                progress: 0.,
                speed: 0.3,
                frozen: 0,
                chilled: 0,
                object: false,
                eating: false,
                rect: ContactRect::new(x - 17, 0, x + 108, 154),
            };
            let cols = foot_columns(z);
            for col in 1..=9 {
                let valid = [-5, 4].iter().all(|&offset| {
                    let mut pad = Pad::new((2, col), PuffShroom);
                    pad.offset_x = offset;
                    prediction::contact(z, z.x, pad.rect())
                });
                assert_eq!(cols.contains(&col), valid, "x={x} col={col}");
            }
        }
    }

    #[test]
    fn rejected_best_pad_does_not_hide_an_acceptable_runner_up() {
        use rsvz::core::model::{ContactRect, ZombiePhase};
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        world.with_backend(|b| {
            PeBackend::scope_backend(b, || {
                let z = Zombie {
                    age: 0,
                    id: ZombieId::from_raw(65537),
                    kind: Z::Football,
                    row: 6,
                    wave: 1,
                    carried: false,
                    x: 200.,
                    hp: 270,
                    armor: 1400,
                    phase: ZombiePhase::ZombieNormal,
                    progress: 0.,
                    speed: 0.68,
                    frozen: 0,
                    chilled: 0,
                    object: false,
                    eating: false,
                    rect: ContactRect::new(236, 0, 278, 115),
                };
                let mut p = Planner::default();
                p.scene = Scene::fixture(vec![z]);
                let f = Forecast::default();
                let best = p.choose_pad(PuffShroom, &f).expect("at least one pad").0;
                let next = p
                    .choose_pad_filtered(PuffShroom, &f, None, false, false, |_, c| c.0 != best)
                    .expect("runner-up")
                    .0;
                assert_ne!(next, best);
                assert!(
                    p.choose_pad_filtered(PuffShroom, &f, None, false, false, |_, _| false)
                        .is_none()
                );
            })
        });
    }
}
// At most two grid cells can overlap one attack rectangle by 20px.
// Include the native Puff offset extremes instead of testing nine trajectories.
fn foot_columns(z: Zombie) -> [i32; 2] {
    let (lo, hi) = prediction::attack_offsets(z);
    let left = (((z.x as i32 + lo - 5) as f32 / 80.).ceil() as i32).max(1);
    let right = (((z.x as i32 + hi + 6) as f32 / 80.).floor() as i32).min(9);
    if left > right {
        [0, 0]
    } else {
        [right, if left < right { left } else { 0 }]
    }
}
fn pad_columns(z: Zombie) -> &'static [i32] {
    if z.kind == Z::Imp {
        &[3, 2, 1]
    } else if z.row == 5 && z.kind == Z::Football {
        &[3, 2, 1, 4, 5, 6]
    } else {
        columns(z.row)
    }
}
fn run(z: Zombie, scene: &Scene, forecast: &Forecast, pad: Option<Pad>) -> Outcome {
    let result = prediction::simulate(z, scene, forecast, pad);
    telemetry::work_prediction(
        if pad.is_none() {
            0
        } else if pad.is_some_and(|p| p.kind == Squash) {
            3
        } else {
            1
        },
        result,
    );
    result
}
fn run_until(z: Zombie, scene: &Scene, forecast: &Forecast, pad: Option<Pad>, horizon: i32, kind: usize) -> Outcome {
    let result = prediction::simulate_until(z, scene, forecast, pad, horizon);
    telemetry::work_prediction(kind, result);
    result
}
fn forecast_key(f: &Forecast) -> u64 {
    f.events.iter().fold(0u64, |key, e| {
        key.wrapping_mul(0x100000001b3)
            ^ ((e.at as u64) << 16)
            ^ ((e.kind as u64) << 10)
            ^ ((e.pos.0 * 10 + e.pos.1) as u64) << 1
            ^ u64::from(e.certain)
            ^ e.body_at.map_or(0, |at| (at as u64 + 1).rotate_left(51))
            ^ e.landing.map_or(0, |p| {
                (p.left.to_bits() as u64).rotate_left(7)
                    ^ (p.right.to_bits() as u64).rotate_left(29)
                    ^ (p.launch as u64).rotate_left(41)
            })
    })
}
pub(super) fn same_outcome(a: Outcome, b: Outcome) -> bool {
    let same_float = |a: f32, b: f32| a.to_bits() == b.to_bits() || a.is_nan() && b.is_nan();
    a.throw_at == b.throw_at
        && same_float(a.throw_x, b.throw_x)
        && a.throw_chilled == b.throw_chilled
        && a.puff_deadline == b.puff_deadline
        && a.death == b.death
        && a.danger == b.danger
        && a.hp_at_end == b.hp_at_end
        && a.trigger == b.trigger
        && a.impact == b.impact
        && a.activated == b.activated
        && a.first_ice == b.first_ice
        && a.ash_hits == b.ash_hits
        && a.ash_mask == b.ash_mask
        && a.squash_hit == b.squash_hit
        && a.gloom_damage == b.gloom_damage
        && same_float(a.x5, b.x5)
        && same_float(a.x50, b.x50)
        && a.hp50 == b.hp50
}
fn audit_cache(z: Zombie, scene: &Scene, f: &Forecast, rebuilding: bool) {
    telemetry::record(
        "cache_input",
        &[
            z.id.raw() as i64,
            i64::from(rebuilding),
            z.x.to_bits() as i64,
            z.progress.to_bits() as i64,
            z.speed.to_bits() as i64,
            z.phase as i64,
            z.hp as i64,
            z.armor as i64,
            z.frozen as i64,
            z.chilled as i64,
            i64::from(z.object),
            z.rect.left as i64,
            z.rect.top as i64,
            z.rect.right as i64,
            z.rect.bottom as i64,
        ],
    );
    for p in &scene.plants {
        telemetry::record(
            "cache_plant",
            &[
                p.id.raw() as i64,
                p.kind as i64,
                p.pos.0 as i64,
                p.pos.1 as i64,
                p.hp as i64,
                p.rect.left as i64,
                p.rect.right as i64,
                p.attack.left as i64,
                p.attack.right as i64,
                p.shooting as i64,
                p.effect as i64,
            ],
        );
    }
    for e in &f.events {
        telemetry::record(
            "cache_plan",
            &[e.kind as i64, e.at as i64, e.pos.0 as i64, e.pos.1 as i64],
        );
    }
}
fn environment_key(z: Zombie, scene: &Scene, f: &Forecast) -> u64 {
    let _perf = crate::perf::scope("planner::environment_key");
    let mut key = 0xcbf29ce484222325_u64;
    let mut add = |value: i64| {
        key = (key ^ value as u64).wrapping_mul(0x100000001b3);
    };
    add(z.kind as i64);
    add(z.row as i64);
    add(z.wave as i64);
    add((z.rect.left - z.x as i32) as i64);
    add((z.rect.right - z.x as i32) as i64);
    add(z.rect.top as i64);
    add(z.rect.bottom as i64);
    for e in &f.events {
        add((scene.now + e.at) as i64);
        add(e.kind as i64);
        add(e.pos.0 as i64);
        add(e.pos.1 as i64);
        add(i64::from(e.certain));
        add(e.body_at.map_or(-1, |at| (scene.now + at) as i64));
        if let Some(p) = e.landing {
            add(p.left.to_bits() as i64);
            add(p.right.to_bits() as i64);
            add(if p.launch > 0 {
                (scene.now + p.launch) as i64
            } else {
                -1
            });
        }
    }
    add(scene.projectile_key(z.row) as i64);
    add(-1);
    for p in &scene.plants {
        if p.pos.0 != z.row && !(p.kind == GloomShroom && (p.pos.0 - z.row).abs() <= 1) {
            continue;
        }
        add(p.id.raw() as i64);
        add(p.kind as i64);
        add(p.pos.0 as i64);
        add(p.pos.1 as i64);
        add(p.hp as i64);
        for v in [p.rect.left, p.rect.top, p.rect.right, p.rect.bottom] {
            add(v as i64);
        }
        if p.kind == GloomShroom && z.rect.right > p.attack.left {
            add(p.attack.left as i64);
            add(p.attack.right as i64);
            add((scene.now + p.launch) as i64);
            add(if p.shooting > 0 {
                (scene.now + p.shooting) as i64
            } else {
                -1
            });
        }
        if matches!(p.kind, PuffShroom | IceShroom | Blover | Jalapeno) {
            let life = prediction::external_destruction_bound(scene, z, Pad::new(p.pos, p.kind)).min(match p.kind {
                IceShroom | Jalapeno => p.effect.max(1),
                Blover => p.effect.max(1),
                _ => prediction::HORIZON,
            });
            add(if life == prediction::HORIZON {
                -1
            } else {
                (scene.now + life) as i64
            });
        }
    }
    if scene
        .plants
        .iter()
        .any(|p| p.pos.0 == z.row && matches!(p.kind, PuffShroom | IceShroom | Blover | Jalapeno))
    {
        // A peer's arrival, death or cold transition changes shared plant HP.
        // Do not reuse a giant result while only checking the giant's own motion.
        for other in scene.row_zombies(z.row).filter(|other| other.id != z.id) {
            add(other.id.raw() as i64);
            add(other.x.to_bits() as i64);
            add(other.progress.to_bits() as i64);
            add(other.speed.to_bits() as i64);
            add(other.hp as i64 + other.armor as i64);
            add(other.frozen as i64);
            add(other.chilled as i64);
            add(other.phase as i64);
            add(i64::from(other.object));
        }
    }
    key
}

fn stable_environment_key(z: Zombie, scene: &Scene, f: &Forecast) -> u64 {
    let mut key = 0xcbf29ce484222325_u64;
    let mut add = |v: i64| {
        key = (key ^ v as u64).wrapping_mul(0x100000001b3);
    };
    add(scene.projectile_key(z.row) as i64);
    for e in &f.events {
        for v in [scene.now + e.at, e.kind as i32, e.pos.0, e.pos.1, e.certain as i32] {
            add(v as i64);
        }
        add(e.body_at.map_or(-1, |at| (scene.now + at) as i64));
        if let Some(p) = e.landing {
            add(p.left.to_bits() as i64);
            add(p.right.to_bits() as i64);
            add(if p.launch > 0 {
                (scene.now + p.launch) as i64
            } else {
                -1
            });
        }
    }
    add(-1);
    for p in &scene.plants {
        if p.pos.0 != z.row && !(p.kind == GloomShroom && (p.pos.0 - z.row).abs() <= 1) {
            continue;
        }
        for v in [
            p.id.raw() as i64,
            p.kind as i64,
            p.hp as i64,
            p.pos.0 as i64,
            p.pos.1 as i64,
        ] {
            add(v);
        }
        if p.kind == GloomShroom {
            add((scene.now + p.launch) as i64);
            add(if p.shooting > 0 {
                (scene.now + p.shooting) as i64
            } else {
                -1
            });
        }
        if matches!(p.kind, IceShroom | Blover | Jalapeno) {
            add((scene.now + p.effect) as i64);
        }
    }
    add(-2);
    for peer in scene.row_zombies(z.row) {
        add(peer.id.raw() as i64);
        if peer.id != z.id {
            add(((peer.hp + peer.armor) / 180) as i64);
        }
    }
    key
}
fn log_prediction(z: Zombie, base: Outcome, after: Outcome, pos: Pos, kind: PlantKind, forecast: &Forecast) {
    telemetry::record(
        "prediction_initial",
        &[
            z.id.raw() as i64,
            (z.speed * 1000000.) as i64,
            (z.progress * 1000000.) as i64,
            z.phase as i64,
            z.frozen as i64,
            z.chilled as i64,
            i64::from(z.object),
            (after.x5 * 100.) as i64,
            (after.x50 * 100.) as i64,
            after.hp50 as i64,
        ],
    );
    telemetry::record(
        "pad_prediction",
        &[
            z.id.raw() as i64,
            z.row as i64,
            z.wave as i64,
            (z.x * 100.) as i64,
            z.hp as i64,
            base.death as i64,
            base.danger as i64,
            pos.1 as i64,
            kind as i64,
            after.death as i64,
            after.danger as i64,
            after.trigger as i64,
            after.impact as i64,
            base.ash_hits as i64,
            after.ash_hits as i64,
            after.gloom_damage as i64,
        ],
    );
    for e in &forecast.events {
        telemetry::record(
            "prediction_effect",
            &[
                z.id.raw() as i64,
                e.at as i64,
                e.kind as i64,
                e.pos.0 as i64,
                e.pos.1 as i64,
                i64::from(e.certain),
            ],
        );
    }
}
impl Planner {
    pub fn add_squash_forecast(&mut self, f: &mut Forecast) {
        squash::add_committed(&self.scene, f, &mut self.squash);
    }
    pub fn policy_baselines(&mut self, forecast: &Forecast) -> Vec<(Zombie, Outcome)> {
        let mut targets: Vec<_> = self
            .scene
            .zombies
            .iter()
            .copied()
            .filter(|z| {
                matches!(
                    z.kind,
                    Z::Normal
                        | Z::Football
                        | Z::Gargantuar
                        | Z::GigaGargantuar
                        | Z::Imp
                        | Z::Zomboni
                        | Z::Catapult
                        | Z::JackInTheBox
                )
            })
            .collect();
        targets.sort_by(|a, b| {
            a.row
                .cmp(&b.row)
                .then(a.x.total_cmp(&b.x))
                .then((a.kind as i32).cmp(&(b.kind as i32)))
                .then(a.hp.cmp(&b.hp))
                .then(a.armor.cmp(&b.armor))
                .then((a.phase as i32).cmp(&(b.phase as i32)))
                .then(a.progress.total_cmp(&b.progress))
        });
        targets.into_iter().map(|z| (z, self.assessment(z, forecast))).collect()
    }

    pub fn next_round(&mut self) {
        self.invalidate();
        self.nuke_hold = None;
        self.next_nuke_retention = 0;
        self.nuke_exclude47 = false;
        self.current_nuke = None;
        self.imp_puff_hold = None;
        self.next_imp_reserve = 0;
        self.next_imp_assess = 0;
        self.last_blover_wave -= 20;
        self.reserve_squash5 = false;
        self.squash5_deferred = false;
        self.ash_births = 0;
        self.next_tail = 0;
        self.tail_five_used = false;
        self.tail_two_used = false;
        self.required.clear();
        self.omitted_cherry_births = 0;
        self.blover_handled_births = 0;
        self.blover_watch_birth = None;
        self.blover_reserved_birth = None;
        self.blover_wait_logged.fill(0);
    }
    pub fn has_carried_tail(&self) -> bool {
        self.scene
            .zombies
            .iter()
            .any(|z| z.carried && z.row == 5 && z.kind == Z::GigaGargantuar)
    }
    /// Spend spare Puff on one useful future obstruction, after immediate duties.
    pub fn carry_pad(&mut self, wave: i32, forecast: &Forecast) -> bool {
        let _perf = crate::perf::scope("planner::carry_pad");
        self.begin_order(extension::Source::Carry);
        if !self.spare_puff() || self.scene.now < self.next_carry {
            return false;
        }
        self.next_carry = self.scene.now + 25;
        if !self.extension.collecting && self.pool_active() {
            return false;
        }
        let deadline = self.carry_horizon(wave);
        let mut considered = 0i64;
        let mut legal = 0i64;
        let mut surviving = 0i64;
        let mut best: Option<(Pos, Zombie, Outcome, Outcome)> = None;
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            if !(z.giant() || z.kind == Z::Football)
                || !matches!(z.row, 1 | 2 | 5 | 6)
                || matches!(wave, 10..=12) && (z.kind != Z::GigaGargantuar || z.wave != 10 || !matches!(z.row, 2 | 5))
            {
                continue;
            }
            let base = self.assessment(z, forecast);
            if base.safe() {
                continue;
            }
            if !self.tuning.pad_mode
                && !self.changed_carry(wave)
                && best.as_ref().is_some_and(|(_, _, old, _)| base.danger >= old.danger)
            {
                continue;
            }
            considered += 1;
            let feet = foot_columns(z);
            for &col in if wave == 20 {
                &feet[..]
            } else {
                &self.carry_columns(wave)[..]
            } {
                if col == 0 {
                    continue;
                }
                let pos = (z.row, col);
                if self.scene.main(pos).is_some()
                    || !combat::candidate(sel(PuffShroom), pos)
                    || !combat::jack_safe(pos, 100)
                {
                    continue;
                }
                let mut pad = Pad::new(pos, PuffShroom);
                pad.order = self.scene.next_plant_order;
                if !self.group_pad_keeps_jacks_in_gloom(pad) {
                    continue;
                }
                pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, z, pos);
                legal += 1;
                pad.trigger_window = deadline;
                if !prediction::could_contact_pad(z, pad) {
                    continue;
                }
                if self.cold_reach && !prediction::could_reach_pad(z, pad, forecast) {
                    continue;
                }
                let probe = prediction::probe_trigger(z, &self.scene, forecast, pad);
                telemetry::work_prediction(1, probe);
                if probe.trigger >= pad.lifetime
                    || probe.trigger > pad.trigger_window
                    || wave != 20 && !prediction::prestore_clear(&self.scene, pos, probe.trigger, forecast)
                {
                    continue;
                }
                surviving += 1;
                let after = run(z, &self.scene, forecast, Some(pad));
                if window::wasted_old_hammer(z, base, after)
                    || after.trigger >= pad.lifetime
                    || after.ash_hits < base.ash_hits && !after.safe()
                {
                    continue;
                }
                let offer = extension::Offer {
                    pos,
                    target: Some(z),
                    base,
                    after,
                    reason: 20,
                    source: extension::Source::Carry,
                };
                if self.extension.collecting {
                    self.offer_puff(offer);
                    continue;
                }
                if self.extension_waits(offer, forecast) {
                    continue;
                }
                if best.as_ref().is_none_or(|(old_pos, old_z, old_base, old_after)| {
                    if self.changed_preferences(z, base) || self.changed_preferences(*old_z, *old_base) {
                        self.prefer_extended(
                            offer,
                            extension::Offer {
                                pos: *old_pos,
                                target: Some(*old_z),
                                base: *old_base,
                                after: *old_after,
                                reason: 20,
                                source: extension::Source::Carry,
                            },
                            forecast,
                        )
                    } else if self.tuning.pad_mode {
                        self.tuning.prefer_pad(base, after, *old_base, *old_after, forecast)
                    } else {
                        base.danger < old_base.danger
                    }
                }) {
                    best = Some((pos, z, base, after));
                    if !self.tuning.pad_mode && !self.changed_carry(wave) {
                        break;
                    }
                }
            }
        }
        if let Some((pos, z, base, after)) = best {
            if self.wait_for_old_hammer(forecast, Some((z, base))) {
                return false;
            }
            if matches!(wave, 10..=12) && self.reserve_real_pad_window(forecast, z.id) {
                return false;
            }
            self.record_choice(z, base, after, pos, PuffShroom, forecast);
            if play_puff(pos, 20) {
                telemetry::record(
                    "carry_prestore",
                    &[
                        z.id.raw() as i64,
                        z.row as i64,
                        pos.1 as i64,
                        after.trigger as i64,
                        base.danger as i64,
                        after.danger as i64,
                    ],
                );
                self.recheck_planted(z.id, pos, forecast);
                return true;
            }
        }
        // W19 can bank a real plant for the next flag wave, even without a current
        // left-side emergency. Never infer "throws finished" merely from no visible imp.
        if matches!(wave, 9 | 19) {
            for &row in self.bank_rows() {
                for col in [3, 4, 5] {
                    let pos = (row, col);
                    if self.scene.main(pos).is_none()
                        && combat::candidate(sel(PuffShroom), pos)
                        && combat::jack_safe(pos, 751)
                        && prediction::prestore_clear(&self.scene, pos, 751, forecast)
                        && self.bank_preserves_safety(pos, forecast)
                    {
                        if self.extension.collecting {
                            self.offer_puff(extension::Offer {
                                pos,
                                target: None,
                                base: Outcome::default(),
                                after: Outcome::default(),
                                reason: 21,
                                source: extension::Source::Bank,
                            });
                            return false;
                        }
                        if !play_puff(pos, 21) {
                            continue;
                        }
                        telemetry::record("tail_bank", &[row as i64, col as i64]);
                        self.observe(rsvz::timing::clock(), self.last_ice);
                        return true;
                    }
                }
            }
        }
        if self.scene.now >= self.next_puff_idle_log {
            self.next_puff_idle_log = self.scene.now + 500;
            telemetry::record("puff_ready_wait", &[considered, legal, surviving]);
        }
        false
    }
    fn bank_preserves_safety(&mut self, pos: Pos, forecast: &Forecast) -> bool {
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            if z.row != pos.0 || !z.giant() {
                continue;
            }
            let mut pad = Pad::new(pos, PuffShroom);
            pad.order = self.scene.next_plant_order;
            pad.trigger_window = 2500;
            if !prediction::could_contact_pad(z, pad) {
                continue;
            }
            let base = self.assessment(z, forecast);
            let after = run(z, &self.scene, forecast, Some(pad));
            // Banking for a future wave must not reposition an already-safe old giant.
            if after.trigger != NEVER {
                return false;
            }
            if after.ash_hits < base.ash_hits && !after.safe() {
                return false;
            }
        }
        true
    }
    /// Maintain a complete row-five group; no partial-group squash fallback here.
    pub fn tail_group(&mut self, wave: i32, forecast: &Forecast) -> bool {
        let reserve = self.reserve_squash5;
        let deferred = self.squash5_deferred;
        let used = self.tail_group_inner(wave, forecast);
        if (1..20).contains(&wave) {
            self.reserve_squash5 |= reserve;
            self.squash5_deferred |= deferred;
        }
        used
    }
    fn tail_group_inner(&mut self, wave: i32, forecast: &Forecast) -> bool {
        let _perf = crate::perf::scope("planner::tail_group");
        if self.scene.now < self.next_tail {
            return false;
        }
        self.next_tail = self.scene.now + 25;
        if wave == 20 && self.tail_five_used && !self.tail_two_used && ready(sel(Squash)) {
            let reserved = self.reserve_squash5;
            let used = self.tail_row_group(2, wave, forecast);
            self.reserve_squash5 |= reserved;
            if used {
                self.tail_two_used = true;
                return true;
            }
        }
        if self.tail_row_group(5, wave, forecast) {
            return true;
        }
        if wave == 20 && !self.tail_two_used && ready(sel(Squash)) {
            let reserved = self.reserve_squash5;
            let used = self.tail_row_group(2, wave, forecast);
            self.reserve_squash5 |= reserved;
            if used {
                self.tail_two_used = true;
            }
            return used;
        }
        false
    }
    fn tail_row_group(&mut self, row: i32, wave: i32, forecast: &Forecast) -> bool {
        self.reserve_squash5 = false;
        self.squash5_deferred = false;
        self.required.clear();
        if self.scene.plants.iter().any(|p| p.kind == Squash && p.pos.0 == row) {
            self.reserve_squash5 = true;
            return false;
        }
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            if z.row != row || z.kind != Z::GigaGargantuar || !(z.carried || wave == 20 && z.wave == 20) {
                continue;
            }
            let (hits, squashed) = telemetry::damage_history(z.id);
            if z.x > 650. {
                if hits < 2 && !squashed {
                    self.required.push(z.id);
                }
                continue;
            }
            let base = self.assessment(z, forecast);
            if base.death < NEVER && base.death <= base.danger {
                continue;
            }
            if hits < 2 && !squashed || base.danger < NEVER {
                self.required.push(z.id);
            }
        }
        self.reserve_squash5 = !self.required.is_empty();
        if self.required.is_empty() {
            return false;
        }
        if self.place_squash(row, forecast, 15) {
            if row == 5 && wave == 20 {
                self.tail_five_used = true;
            }
            telemetry::record("tail_group_squash", &[self.required.len() as i64, row as i64]);
            return true;
        }
        if row == 2 && wave != 20 || !self.spare_puff() || self.required.len() < 2 {
            return false;
        }
        let Some(leader) = self
            .scene
            .row_zombies(row)
            .filter(|z| self.required.contains(&z.id))
            .min_by(|a, b| a.x.total_cmp(&b.x))
            .copied()
        else {
            return false;
        };
        // A real imminent competing need takes precedence over optional grouping.
        for &other in &self.scene.zombies {
            if other.row != row
                && (other.giant() || other.kind == Z::Football)
                && prediction::could_threaten(other, &self.scene, 751)
                && run_until(other, &self.scene, forecast, None, 751, 2).danger <= 751
            {
                return false;
            }
        }
        let feet = foot_columns(leader);
        for &col in if wave == 20 { &feet[..] } else { &[4, 5][..] } {
            if col == 0 {
                continue;
            }
            let pos = (row, col);
            if self.scene.main(pos).is_some()
                || !combat::candidate(sel(PuffShroom), pos)
                || !combat::jack_safe(pos, 100)
            {
                continue;
            }
            let mut pad = Pad::new(pos, PuffShroom);
            pad.order = self.scene.next_plant_order;
            pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, leader, pos);
            if wave == 20 && !self.group_pad_keeps_jacks_in_gloom(pad) {
                continue;
            }
            if !prediction::could_contact_pad(leader, pad) {
                continue;
            }
            let probe = prediction::probe_trigger(leader, &self.scene, forecast, pad);
            telemetry::work_prediction(1, probe);
            if probe.trigger >= pad.lifetime || probe.trigger > 751 || !combat::jack_safe(pos, probe.trigger.min(126)) {
                continue;
            }
            // A pre-existing strike is not a new grouping pause.
            let before = run_until(leader, &self.scene, forecast, None, 751, 1);
            let after = run_until(leader, &self.scene, forecast, Some(pad), 751, 1);
            if after.impact <= after.trigger || after.x_at_end <= before.x_at_end + 1. {
                continue;
            }
            if play_puff(pos, 18) {
                telemetry::record(
                    "squash_group_pad",
                    &[
                        leader.id.raw() as i64,
                        self.required.len() as i64,
                        col as i64,
                        probe.trigger as i64,
                    ],
                );
                self.recheck_planted(leader.id, pos, forecast);
                return true;
            }
        }
        false
    }
    fn record_choice(&self, z: Zombie, base: Outcome, after: Outcome, pos: Pos, kind: PlantKind, f: &Forecast) {
        log_prediction(z, base, after, pos, kind, f);
        if kind == PuffShroom && matches!(time().1, 10 | 11) && matches!(z.row, 2 | 5) && z.kind == Z::GigaGargantuar {
            telemetry::record(
                "retention_need",
                &[
                    z.id.raw() as i64,
                    retention::classify(base, f) as i64,
                    base.death as i64,
                    base.danger as i64,
                    base.ash_mask as i64,
                    after.ash_mask as i64,
                    after.trigger as i64,
                    after.impact as i64,
                ],
            );
        }
        telemetry::record(
            "pad_scene",
            &[
                z.id.raw() as i64,
                self.scene.next_plant_order as i64,
                prediction::puff_external_destruction_bound(&self.scene, z, pos) as i64,
            ],
        );
        for p in self
            .scene
            .plants
            .iter()
            .filter(|p| p.pos.0 == z.row || p.kind == GloomShroom && (p.pos.0 - z.row).abs() <= 1)
        {
            telemetry::record(
                "prediction_plant",
                &[
                    z.id.raw() as i64,
                    p.id.raw() as i64,
                    p.kind as i64,
                    p.pos.0 as i64,
                    p.pos.1 as i64,
                    p.rect.left as i64,
                    p.rect.right as i64,
                    p.shooting as i64,
                ],
            );
        }
        for other in &self.scene.zombies {
            if let Some(c) = self.cache.get(&other.id) {
                if !c.outcome.safe() {
                    telemetry::record(
                        "simultaneous_demand",
                        &[
                            other.id.raw() as i64,
                            other.row as i64,
                            other.wave as i64,
                            (other.x * 100.) as i64,
                            other.hp as i64,
                            (c.at + c.outcome.danger - self.scene.now) as i64,
                            (self.scene.now - c.at) as i64,
                        ],
                    );
                }
            }
        }
    }
    pub fn invalidate(&mut self) {
        self.invalidate_extension();
        self.puff_window_hold = None;
        self.cache.clear();
        self.pad_retry.clear();
    }
    pub fn motion_changed(&mut self, event: &rsvz::event::GargantuarMotionFact) {
        telemetry::cache_count(2);
        if let Some(old) = self.cache.remove(&event.zombie_id) {
            telemetry::cache_count(3);
            telemetry::record(
                "motion_invalidated",
                &[
                    event.zombie_id.raw() as i64,
                    event.main_counter as i64,
                    event.reasons as i64,
                    old.at as i64,
                ],
            );
        }
    }
    fn invalidate_row(&mut self, row: i32) {
        self.cache.retain(|_, c| c.row != row);
    }
    pub fn observe(&mut self, now: i32, last_ice: i32) {
        let _perf = crate::perf::scope("planner::observe");
        self.last_ice = last_ice;
        self.scene.sample(now, last_ice);
        telemetry::observe(&self.scene);
        self.cache.retain(|id, _| self.scene.contains_zombie(*id));
        self.imp_risk_logged.retain(|id, _| self.scene.contains_zombie(*id));
    }
    fn cached_outcome(&self, z: Zombie, f: &Forecast) -> Option<Outcome> {
        let c = self.cache.get(&z.id)?;
        if c.serial != telemetry::serial() {
            return None;
        }
        let forecast = forecast_key(f);
        if self.memo_checks {
            if let Some(checked) = c.checked.get() {
                if checked.observation == self.scene.revision && checked.forecast == forecast {
                    telemetry::cache_count(9);
                    return checked.outcome;
                }
            }
        }
        let outcome = self.check_cached_outcome(c, z, f, forecast);
        if self.memo_checks {
            c.checked.set(Some(Checked {
                observation: self.scene.revision,
                forecast,
                outcome,
            }));
        }
        outcome
    }
    fn check_cached_outcome(&self, c: &Cached, z: Zombie, f: &Forecast, forecast: u64) -> Option<Outcome> {
        let _perf = crate::perf::scope("planner::check_cached_outcome");
        if c.at == self.scene.now {
            return (c.forecast == forecast).then_some(c.outcome);
        }
        if self.event_cache == 0 || !z.giant() {
            return None;
        }
        telemetry::cache_count(0);
        let age = self.scene.now - c.at;
        if !(5..=prediction::REUSE_LIMIT).contains(&age)
            || age % 5 != 0
            || c.outcome.death.min(c.outcome.danger) >= NEVER
            || age >= c.prefix.first_pad_change
        {
            return None;
        }
        let point = c.prefix.points[(age / 5) as usize]?;
        let approximate = self.approximate_reuse > 0
            && age <= self.approximate_reuse
            && c.outcome.danger > age + 200
            && point.near(z)
            && c.stable_environment == stable_environment_key(z, &self.scene, f);
        if !approximate && !point.matches(z) {
            telemetry::cache_count(6);
            return None;
        }
        if !approximate && c.environment != environment_key(z, &self.scene, f) {
            telemetry::cache_count(7);
            return None;
        }
        let mut out = c.outcome;
        out.uncertain_at = if out.uncertain_at == NEVER {
            NEVER
        } else {
            (out.uncertain_at - age).max(0)
        };
        out.end_at = (out.end_at - age).max(0);
        if approximate {
            out.uncertain_at = 0;
            out.assumptions |= prediction::ASSUME_CACHE;
        }
        if out.throw_at < NEVER && out.throw_at > age {
            out.throw_at -= age;
        } else {
            out.throw_at = NEVER;
            out.throw_x = f32::NAN;
            out.throw_chilled = 0;
        }
        out.puff_deadline = if out.puff_deadline == NEVER {
            NEVER
        } else {
            out.puff_deadline - age
        };
        out.death = if out.death == NEVER { NEVER } else { out.death - age };
        out.first_ice = if out.first_ice <= age || out.first_ice == NEVER {
            NEVER
        } else {
            out.first_ice - age
        };
        out.danger = if out.danger == NEVER { NEVER } else { out.danger - age };
        if out.death.min(out.danger) <= 0 {
            return None;
        }
        out.gloom_damage -= point.gloom_damage;
        out.x5 = c.prefix.points[((age + 5) / 5) as usize].map_or(f32::NAN, |p| p.x);
        out.x50 = c.prefix.points[((age + 50) / 5) as usize].map_or(f32::NAN, |p| p.x);
        out.hp50 = c.prefix.points[((age + 50) / 5) as usize].map_or(0, |p| p.health);
        out.steps -= age;
        if self.event_cache == 2 {
            telemetry::cache_count(4);
            let fresh = run(z, &self.scene, f, None);
            if !same_outcome(out, fresh) {
                telemetry::cache_count(5);
                telemetry::cache_mismatch(z.id, age, out, fresh);
                if self.cache_audit_id == z.id.raw() {
                    audit_cache(z, &self.scene, f, false);
                }
                return None;
            }
            // Shadow mode never lets a reuse discrepancy affect actual actions.
            return Some(fresh);
        }
        telemetry::cache_count(1);
        Some(out)
    }
    fn assessment(&mut self, z: Zombie, f: &Forecast) -> Outcome {
        let _perf = crate::perf::scope("planner::assessment");
        if let Some(out) = self.cached_outcome(z, f) {
            return out;
        }
        let mut prefix = prediction::MotionPrefix::default();
        if self.cache_audit_id == z.id.raw() {
            audit_cache(z, &self.scene, f, true);
        }
        let out = if self.event_cache != 0 && z.giant() {
            let out = prediction::simulate_captured(z, &self.scene, f, &mut prefix);
            telemetry::work_prediction(0, out);
            out
        } else {
            run(z, &self.scene, f, None)
        };
        let environment = if self.event_cache != 0 && z.giant() {
            environment_key(z, &self.scene, f)
        } else {
            0
        };
        self.keep_nuke_dependency(z, out, f);
        self.cache.insert(
            z.id,
            Cached {
                row: z.row,
                at: self.scene.now,
                serial: telemetry::serial(),
                forecast: forecast_key(f),
                outcome: out,
                environment,
                stable_environment: if self.approximate_reuse > 0 {
                    stable_environment_key(z, &self.scene, f)
                } else {
                    0
                },
                prefix,
                checked: Cell::new(None),
            },
        );
        if z.kind == Z::GigaGargantuar
            && matches!(z.row, 2 | 5)
            && z.x < 300.
            && z.hp > 3600
            && out.ash_hits == 0
            && !self.gap_logged.contains_key(&z.id)
        {
            self.gap_logged.insert(z.id, true);
            diagnostics::mark(C::HighHealthGap);
            telemetry::record(
                "three_hit_red_gap",
                &[
                    z.id.raw() as i64,
                    z.row as i64,
                    z.wave as i64,
                    (z.x * 100.) as i64,
                    z.hp as i64,
                    out.death as i64,
                    out.danger as i64,
                ],
            );
        }
        if z.kind == Z::Imp && out.danger < NEVER && self.imp_risk_logged.insert(z.id, ()).is_none() {
            telemetry::record(
                "imp_home_risk",
                &[
                    z.id.raw() as i64,
                    z.row as i64,
                    (z.x * 100.) as i64,
                    z.hp as i64,
                    (z.speed * 100000.) as i64,
                    (z.progress * 100000.) as i64,
                    z.frozen as i64,
                    z.chilled as i64,
                    out.danger as i64,
                    out.death as i64,
                    out.hp_at_end as i64,
                    cd(sel(PuffShroom)) as i64,
                ],
            );
        }
        out
    }
    pub fn choose_pad(&mut self, kind: PlantKind, f: &Forecast) -> Option<(Pos, Zombie, Outcome, Outcome)> {
        self.choose_pad_for(kind, f, None)
    }
    fn choose_pad_for(
        &mut self, kind: PlantKind, f: &Forecast, only: Option<(i32, Z)>,
    ) -> Option<(Pos, Zombie, Outcome, Outcome)> {
        self.choose_pad_pass(kind, f, only, false)
    }
    fn choose_pad_pass(
        &mut self, kind: PlantKind, f: &Forecast, only: Option<(i32, Z)>, early_only: bool,
    ) -> Option<(Pos, Zombie, Outcome, Outcome)> {
        self.choose_pad_filtered(kind, f, only, early_only, true, |_, _| true)
    }
    fn choose_pad_filtered(
        &mut self, kind: PlantKind, f: &Forecast, only: Option<(i32, Z)>, early_only: bool, cache_negative: bool,
        mut accepts: impl FnMut(&Self, &(Pos, Zombie, Outcome, Outcome)) -> bool,
    ) -> Option<(Pos, Zombie, Outcome, Outcome)> {
        let _perf = crate::perf::scope("planner::choose_pad_pass");
        if kind == PuffShroom {
            self.begin_order(if early_only {
                extension::Source::Front
            } else {
                extension::Source::Ordinary
            });
        }
        let retry_key = (kind as i32, early_only);
        if cache_negative && self.pad_retry_gap > 0 && only.is_none() {
            if self
                .pad_retry
                .get(&retry_key)
                .is_some_and(|&(at, serial)| self.scene.now < at && telemetry::serial() == serial)
            {
                return None;
            }
        }
        let mut best: Option<(Pos, Zombie, Outcome, Outcome)> = None;
        let mut best_closing = false;
        let mut best_imp = false;
        // All visible relevant individuals, not the leftmost/highest-HP representative of a row.
        for index in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[index];
            if only.is_some_and(|(row, wanted)| z.row != row || z.kind != wanted) {
                continue;
            }
            if kind == PuffShroom && self.puff_reserved_for_blover(z) {
                continue;
            }
            let landed_imp = self.imp_padding
                && matches!(kind, PuffShroom | Blover | IceShroom | Imitator)
                && z.kind == Z::Imp
                && z.phase == ZombiePhase::ZombieNormal;
            if !matches!(z.row, 1 | 2 | 5 | 6) || !(z.giant() || z.kind == Z::Football || landed_imp) || z.x > 650. {
                continue;
            }
            let forward_retention =
                matches!(time().1, 10 | 11) && matches!(z.row, 2 | 5) && z.kind == Z::GigaGargantuar;
            let eligible_early = forward_retention && kind == PuffShroom && z.wave == 10 && !z.carried;
            if early_only && !eligible_early {
                continue;
            }
            let known = if eligible_early || self.pad_base_first {
                Some(self.assessment(z, f))
            } else {
                self.cached_outcome(z, f)
            };
            if eligible_early && early_only && known.unwrap().danger <= 751 {
                continue;
            }
            let local_retention = eligible_early && early_only;
            if known.is_some_and(|out| {
                if local_retention {
                    retention::classify(out, f) == retention::Need::Clear
                } else {
                    out.safe()
                }
            }) {
                continue;
            }
            let mut baseline = known;
            let mut closing = None;
            let w20_foot = time().1 == 20 && z.giant();
            let feet = foot_columns(z);
            let candidates = if w20_foot {
                &feet[..]
            } else if kind == PuffShroom
                && z.giant()
                && matches!(z.row, 1 | 6)
                && f.events
                    .iter()
                    .any(|e| e.kind == prediction::EffectKind::Doom && e.at <= 2000)
            {
                &[1, 2, 3, 4, 5, 6, 7][..]
            } else if forward_retention || kind == PuffShroom && self.extended_sites(z) {
                &[3, 4, 5, 6, 7][..]
            } else {
                pad_columns(z)
            };
            for &col in candidates {
                if col == 0 {
                    continue;
                }
                let pos = (z.row, col);
                let mut pad = Pad::new(pos, kind);
                if kind == PuffShroom {
                    pad.trigger_window = self.tuning.horizon;
                }
                if !prediction::could_contact_pad(z, pad) {
                    continue;
                }
                if self.cold_reach && !prediction::could_reach_pad(z, pad, f) {
                    continue;
                }
                let replace =
                    kind == PuffShroom && self.scene.main(pos).is_some_and(|p| p.kind == PuffShroom && p.hp < 300);
                pad.replaces_occupant = replace;
                if kind == PuffShroom && !self.group_pad_keeps_jacks_in_gloom(pad) {
                    continue;
                }
                if self.scene.main(pos).is_some() && !replace
                    || !(if replace {
                        combat::candidate(sel(kind), pos)
                    } else {
                        legal(if kind == Imitator { COPY } else { sel(kind) }, pos)
                    })
                    || !combat::jack_safe(pos, 1)
                {
                    continue;
                }
                pad.order = self.scene.next_plant_order;
                pad.lifetime = pad
                    .lifetime
                    .min(prediction::external_destruction_bound(&self.scene, z, pad));
                if pad.lifetime <= 0 || z.frozen >= pad.lifetime || !prediction::could_contact_pad(z, pad) {
                    continue;
                }
                if !replace && self.event_cache != 0 && z.giant() && baseline.is_some() {
                    if let Some(cached) = self.cache.get(&z.id) {
                        let contact = cached.prefix.first_contact[(col - 1) as usize];
                        if contact == NEVER
                            || contact - (self.scene.now - cached.at) > pad.lifetime.min(pad.trigger_window)
                        {
                            if kind != PuffShroom
                                || !self.fresh_contact_needed(cached.at)
                                || prediction::probe_trigger(z, &self.scene, f, pad).trigger
                                    > pad.lifetime.min(pad.trigger_window)
                            {
                                telemetry::cache_count(8);
                                continue;
                            }
                        }
                    }
                }
                if !self.pad_base_first {
                    let probe = prediction::probe_trigger(z, &self.scene, f, pad);
                    telemetry::work_prediction(1, probe);
                    if probe.trigger == NEVER
                        || probe.trigger >= pad.lifetime
                        || !combat::jack_safe(pos, probe.trigger.min(126))
                    {
                        continue;
                    }
                }
                let base = *baseline.get_or_insert_with(|| self.assessment(z, f));
                if if local_retention {
                    retention::classify(base, f) == retention::Need::Clear
                } else {
                    base.safe()
                } {
                    break;
                }
                let after = run(z, &self.scene, f, Some(pad));
                if kind == PuffShroom && !combination::useful(z, base, after) {
                    continue;
                }
                if replace && !(after.safe() && !base.safe() || after.death < base.death || after.danger > base.danger)
                {
                    continue;
                }
                if after.trigger == NEVER || after.trigger >= pad.lifetime {
                    continue;
                }
                if after.trigger > pad.trigger_window || !combat::jack_safe(pos, after.trigger.min(126)) {
                    continue;
                }
                if local_retention && after.impact <= after.trigger
                    || kind == PuffShroom && window::wasted_old_hammer(z, base, after)
                {
                    continue;
                }
                // A late block must not push this concrete target out of a near scheduled ash hit.
                if local_retention && retention::protected_hits(base, f) & !after.ash_mask != 0 {
                    continue;
                }
                if !local_retention && after.ash_hits < base.ash_hits && !after.safe() {
                    continue;
                }
                if !accepts(self, &(pos, z, base, after)) {
                    continue;
                }
                if kind == PuffShroom && !self.imp_allows_pad(z, base, after, f) {
                    continue;
                }
                let source = if early_only {
                    extension::Source::Front
                } else {
                    extension::Source::Ordinary
                };
                let reason = if z.giant() {
                    10
                } else if z.kind == Z::Imp {
                    23
                } else {
                    11
                };
                let offer = extension::Offer {
                    pos,
                    target: Some(z),
                    base,
                    after,
                    reason,
                    source,
                };
                if kind == PuffShroom && self.extension.collecting {
                    self.offer_puff(offer);
                    continue;
                }
                if kind == PuffShroom && self.extension_waits(offer, f) {
                    continue;
                }
                let closes = kind == PuffShroom
                    && !early_only
                    && *closing.get_or_insert_with(|| self.last_pad_window_closes(z, base, f));
                let confirmed_imp = kind == PuffShroom && self.confirmed_imp(z.id);
                if best.as_ref().is_none_or(|(old_pos, old_z, old_base, old_after)| {
                    if confirmed_imp != best_imp {
                        let window = self.imp_puff_hold.map_or(NEVER, |r| r.until - self.scene.now);
                        let other_loss = if confirmed_imp {
                            self.puff_operation_deadline(*old_z, *old_base, f)
                        } else {
                            self.puff_operation_deadline(z, base, f)
                        };
                        return if other_loss < window {
                            !confirmed_imp
                        } else {
                            confirmed_imp
                        };
                    }
                    if closes != best_closing {
                        return closes;
                    }
                    if kind == PuffShroom && base.danger > 751 && old_base.danger > 751 {
                        let retained = after.ash_hits > base.ash_hits;
                        let old_retained = old_after.ash_hits > old_base.ash_hits;
                        if retained != old_retained {
                            return retained;
                        }
                    }
                    if kind == PuffShroom && z.id == old_z.id && after.safe() == old_after.safe() {
                        let loss = self.storage_imp_exposure(pos, after.trigger, z, base);
                        let old_loss = self.storage_imp_exposure(*old_pos, old_after.trigger, *old_z, *old_base);
                        if loss != old_loss {
                            return loss < old_loss;
                        }
                    }
                    if kind == PuffShroom
                        && (self.changed_preferences(z, base) || self.changed_preferences(*old_z, *old_base))
                    {
                        self.prefer_extended(
                            offer,
                            extension::Offer {
                                pos: *old_pos,
                                target: Some(*old_z),
                                base: *old_base,
                                after: *old_after,
                                reason,
                                source,
                            },
                            f,
                        )
                    } else if kind == PuffShroom && self.tuning.pad_mode {
                        self.tuning.prefer_pad(base, after, *old_base, *old_after, f)
                    } else {
                        base.danger < old_base.danger
                            || base.danger == old_base.danger
                                && (after.safe() && !old_after.safe() || after.danger > old_after.danger)
                    }
                }) {
                    best = Some((pos, z, base, after));
                    best_closing = closes;
                    best_imp = confirmed_imp;
                }
            }
        }
        if kind == PuffShroom
            && !early_only
            && only.is_none()
            && self.wait_for_old_hammer(f, best.as_ref().map(|(_, z, base, _)| (*z, *base)))
        {
            return None;
        }
        if cache_negative && best.is_none() && self.pad_retry_gap > 0 && only.is_none() {
            self.pad_retry
                .insert(retry_key, (self.scene.now + self.pad_retry_gap, telemetry::serial()));
        }
        best
    }
    pub fn ice_timing_safe(&self, pos: Pos, f: &Forecast, earliest: i32) -> bool {
        let mut scene = self.scene.clone();
        scene.plants.retain(|p| p.pos != pos || p.kind != PuffShroom);
        let mut pad = Pad::new(pos, IceShroom);
        pad.order = scene.next_plant_order;
        let out = prediction::plant_outcome(&scene, f, pad, 100);
        out.activated >= earliest && out.activated <= 100
    }
    pub fn ice_pad(
        &mut self, card: CardSelection, f: &Forecast, earliest_effect: i32, tail_football: bool,
    ) -> Option<Pos> {
        let mut safe = [None; 54];
        let normal = self.choose_pad_filtered(
            if card == COPY { Imitator } else { IceShroom },
            f,
            None,
            false,
            false,
            |planner, &(pos, _, _, after)| {
                if tail_football && !planner.tail_ice_inside(card, Some(pos)) {
                    return false;
                }
                if card == COPY && (after.impact < earliest_effect || after.impact < 320)
                    || card != COPY && !planner.ice_timing_safe(pos, f, earliest_effect)
                {
                    return false;
                }
                *safe[((pos.0 - 1) * 9 + pos.1 - 1) as usize].get_or_insert_with(|| {
                    (if card == COPY {
                        rsvz::is_safe_imitator_ice(grid(pos))
                    } else {
                        rsvz::is_safe_ice(grid(pos))
                    })
                    .unwrap_or(false)
                        && combat::jack_safe(pos, if card == COPY { 420 } else { 100 })
                })
            },
        );
        let retention = self.retention_ice_pad(card, f, earliest_effect, tail_football);
        let chosen = match (normal, retention) {
            (Some(current), Some(hold))
                if current.1.row != hold.1.row && current.2.danger <= cd(sel(PuffShroom)).max(100) =>
            {
                Some(current)
            }
            (_, Some(hold)) => Some(hold),
            (current, None) => current,
        };
        let (pos, z, base, after) = chosen?;
        self.record_choice(z, base, after, pos, if card == COPY { Imitator } else { IceShroom }, f);
        Some(pos)
    }
    fn tail_guns(&self) -> Option<[rsvz::core::model::ContactRect; 2]> {
        let mut guns = self
            .scene
            .plants
            .iter()
            .filter(|p| p.kind == GloomShroom && p.pos.0 == 2 && p.pos.1 <= 2);
        Some([guns.next()?.attack, guns.next()?.attack])
    }
    pub fn first_ice_wait(&self, wave: i32, card: CardSelection, pos: Option<Pos>, f: &Forecast) -> i32 {
        let Some(guns) = self.tail_guns() else {
            return 0;
        };
        let lead = if card == COPY { 420 } else { 100 };
        let without = Forecast {
            events: f
                .events
                .iter()
                .copied()
                .filter(|e| e.kind != prediction::EffectKind::Ice || e.certain || e.at > lead)
                .collect(),
        };
        let extra = pos.map(|p| Pad::new(p, if card == COPY { Imitator } else { IceShroom }));
        let mut wait = 0;
        for &z in self
            .scene
            .row_zombies(1)
            .filter(|z| z.kind == Z::Football && (z.carried || z.wave < wave))
        {
            let base = run(z, &self.scene, &without, None);
            if base.safe() {
                continue;
            }
            let out = prediction::simulate_until(z, &self.scene, &without, extra, lead);
            if out.death <= lead {
                continue;
            }
            let rect = z.rect_at(out.x_at_end);
            if guns.iter().all(|g| rect.left < g.right && rect.right > g.left) {
                continue;
            }
            // Once left of the common band, waiting cannot restore coverage.
            if rect.right <= guns.iter().map(|g| g.left).max().unwrap() {
                continue;
            }
            let (entry, death) = prediction::first_common_entry(z, &self.scene, guns);
            if death < entry || entry >= NEVER {
                continue;
            }
            wait = wait.max(entry.max(lead + 1));
        }
        if wait == 0 {
            return 0;
        }
        // Preserve an earlier real loss elsewhere instead of waiting blindly
        // for one lane's preferred damage position.
        let emergency = self.scene.zombies.iter().copied().any(|z| {
            if z.kind == Z::Imp && matches!(z.phase, ZombiePhase::ImpGettingThrown | ZombiePhase::ImpLanding) {
                return prediction::imp_landing::observed_landing(z).is_some_and(|(child, arrival)| {
                    prediction::simulate_imp_arrival(child, &self.scene, &without, 0, arrival).danger <= wait
                });
            }
            z.phase == ZombiePhase::JackInTheBoxPopping
                || (z.giant() || matches!(z.kind, Z::Football | Z::Imp))
                    && run_until(z, &self.scene, &without, None, wait, 2).danger <= wait
        });
        if emergency { 0 } else { (wait - lead).max(1) }
    }
    pub fn first_ice_inside(&self, wave: i32, card: CardSelection, pos: Option<Pos>, f: &Forecast) -> bool {
        self.first_ice_wait(wave, card, pos, f) == 0
    }
    pub fn tail_ice_delay(&self) -> Option<i32> {
        let guns = self.tail_guns()?;
        let mut result = None;
        for &z in self.scene.row_zombies(1).filter(|z| z.kind == Z::Football) {
            let (entry, death) = prediction::first_common_entry(z, &self.scene, guns);
            if death < entry {
                continue;
            }
            result = Some(result.unwrap_or(0).max(entry));
        }
        result
    }
    /// Verify the proposed activation, including an optional Ice body used as a pad.
    pub fn tail_ice_inside(&self, card: CardSelection, pos: Option<Pos>) -> bool {
        let Some(guns) = self.tail_guns() else {
            return true;
        };
        let lead = if card == COPY { 420 } else { 100 };
        let extra = pos.map(|p| Pad::new(p, if card == COPY { Imitator } else { IceShroom }));
        for &z in self.scene.row_zombies(1).filter(|z| z.kind == Z::Football) {
            let out = prediction::simulate_until(z, &self.scene, &Forecast::default(), extra, lead);
            if out.death <= lead {
                continue;
            }
            let rect = z.rect_at(out.x_at_end);
            if !guns.iter().all(|g| rect.left < g.right && rect.right > g.left) {
                return false;
            }
        }
        true
    }
    pub fn guard_w20_football(&mut self, f: &Forecast) -> bool {
        let Some((pos, z, base, after)) = self.choose_pad_for(PuffShroom, f, Some((6, Z::Football))) else {
            return false;
        };
        self.record_choice(z, base, after, pos, PuffShroom, f);
        if play_puff(pos, 11) {
            telemetry::record(
                "w20_football_guard",
                &[z.id.raw() as i64, pos.1 as i64, base.danger as i64, after.danger as i64],
            );
            self.recheck_planted(z.id, pos, f);
            return true;
        }
        false
    }
    pub fn pads(&mut self, w: i32, t: i32, f: &Forecast) {
        // Grounded Imp risk is independent of whether Puff is currently ready.
        // Recheck cold/ash changes, but log each individual risk only once.
        if self.scene.now >= self.next_imp_assess {
            self.next_imp_assess = self.scene.now + 25;
            for index in 0..self.scene.zombies.len() {
                let z = self.scene.zombies[index];
                if z.kind == Z::Imp && z.phase == ZombiePhase::ZombieNormal && matches!(z.row, 1 | 2 | 5 | 6) {
                    self.assessment(z, f);
                }
            }
        }
        if self.pool_active() {
            self.pooled_puff(w, t, f);
            return;
        }
        if ready(sel(PuffShroom)) {
            if self.retain_cherry_targets(f) {
                return;
            }
            let ordinary = self.choose_pad(PuffShroom, f);
            if self.wait_for_football_contact(f, ordinary.as_ref()) {
                return;
            }
            if self.try_nuke_retention(w, f, ordinary.as_ref()) {
                return;
            }
            // Compare a concrete one/two-Puff ash window before optional stock.
            // Immediate loss and expiring interception requests keep precedence.
            if ordinary
                .as_ref()
                .is_none_or(|(_, z, base, _)| base.danger > 751 && !self.last_pad_window_closes(*z, *base, f))
                && self.spare_puff()
                && self.scene.now >= self.next_ash_pad
            {
                self.next_ash_pad = self.scene.now + 25;
                if let Some((pos, z, base, after, event)) = self.choose_ash_pad(f) {
                    self.record_choice(z, base, after, pos, PuffShroom, f);
                    if play_puff(pos, 16) {
                        telemetry::record("priority_ash_retention", &[z.id.raw() as i64, event as i64]);
                        self.recheck_planted(z.id, pos, f);
                        return;
                    }
                }
            }
            if let Some((pos, z, base, after)) = ordinary {
                if self.scene.main(pos).is_none() && self.later_pad_better(z, pos, f, after) {
                    self.puff_window_hold = Some(self.scene.now);
                    return;
                }
                self.record_choice(z, base, after, pos, PuffShroom, f);
                if let Some(r) = self.imp_puff_hold.filter(|r| !r.observed && self.scene.now < r.until) {
                    telemetry::record(
                        "imp_reservation_yield",
                        &[
                            r.source.raw() as i64,
                            z.id.raw() as i64,
                            r.until as i64,
                            after.trigger as i64,
                        ],
                    );
                }
                if play_puff(
                    pos,
                    if z.giant() {
                        10
                    } else if z.kind == Z::Imp {
                        23
                    } else {
                        11
                    },
                ) {
                    if z.kind == Z::Imp {
                        telemetry::record(
                            "imp_pad",
                            &[
                                z.id.raw() as i64,
                                z.row as i64,
                                pos.1 as i64,
                                base.danger as i64,
                                after.danger as i64,
                                after.death as i64,
                                after.trigger as i64,
                                after.impact as i64,
                            ],
                        );
                    }
                    self.recheck_planted(z.id, pos, f);
                    return;
                }
            }
            if !self.spare_puff() {
                return;
            }
            // Optional retention uses spare Puff only; normal ice/blover duties are unchanged.
            if self.imp_puff_hold.is_some_and(|r| self.scene.now < r.until) {
                return;
            }
            if self.scene.now >= self.next_ash_pad {
                self.next_ash_pad = self.scene.now + 25;
                if let Some((pos, z, base, after, event)) = self.choose_ash_pad(f) {
                    self.record_choice(z, base, after, pos, PuffShroom, f);
                    let e = f.events[event];
                    if play_puff(pos, 16) {
                        telemetry::record(
                            "ash_retention_pad",
                            &[
                                z.id.raw() as i64,
                                z.row as i64,
                                z.wave as i64,
                                pos.1 as i64,
                                e.kind as i64,
                                e.pos.0 as i64,
                                e.pos.1 as i64,
                                e.at as i64,
                                base.ash_mask as i64,
                                after.ash_mask as i64,
                                base.danger as i64,
                                after.danger as i64,
                            ],
                        );
                        self.recheck_planted(z.id, pos, f);
                        return;
                    }
                }
            }
            // Early W10 control is the last consumer, never a competitor in normal selection.
            if matches!(w, 10 | 11) {
                let mut imminent = false;
                for i in 0..self.scene.zombies.len() {
                    let z = self.scene.zombies[i];
                    if matches!(z.row, 1 | 2 | 5 | 6)
                        && (z.giant() || matches!(z.kind, Z::Football | Z::Imp))
                        && prediction::could_threaten(z, &self.scene, 751)
                        && self.assessment(z, f).danger <= 751
                    {
                        imminent = true;
                        break;
                    }
                }
                if !imminent {
                    if let Some((pos, z, base, after)) = self.choose_pad_pass(PuffShroom, f, None, true) {
                        if self.reserve_real_pad_window(f, z.id) {
                            return;
                        }
                        self.record_choice(z, base, after, pos, PuffShroom, f);
                        if play_puff(pos, 10) {
                            telemetry::record(
                                "early_relief_pad",
                                &[
                                    z.id.raw() as i64,
                                    pos.0 as i64,
                                    pos.1 as i64,
                                    base.danger as i64,
                                    after.trigger as i64,
                                ],
                            );
                            self.recheck_planted(z.id, pos, f);
                            return;
                        }
                    }
                }
            }
        } else {
            let mut demands = 0;
            for index in 0..self.scene.zombies.len() {
                let z = self.scene.zombies[index];
                if matches!(z.row, 1 | 2 | 5 | 6) && (z.giant() || z.kind == Z::Football) && z.x < 350. {
                    // Diagnostics consume already-known demands; they never start a prediction.
                    if self
                        .cached_outcome(z, f)
                        .is_some_and(|out| !out.safe() && out.danger < 751)
                    {
                        demands += 1;
                    }
                }
            }
            if demands > 0 {
                diagnostics::mark(C::PuffUnavailable);
            }
            if demands > 1 {
                diagnostics::mark(C::PadConflict);
            }
        }
        // Prestore at the user's row-five col-three site when no current genuine request used it.
        if w > 1
            && w != 20
            && t > 500
            && self.spare_puff()
            && self.scene.main((5, 3)).is_none()
            && combat::jack_safe((5, 3), 100)
        {
            let foot = self
                .scene
                .zombies
                .iter()
                .find(|z| z.kind == Z::Football && z.row == 5 && z.x > 210. && z.x < 600.)
                .copied();
            if let Some(z) = foot {
                let base = self.assessment(z, f);
                let mut pad = Pad::new((5, 3), PuffShroom);
                pad.order = self.scene.next_plant_order;
                pad.trigger_window = 2500;
                let after = if !base.safe() {
                    Some(run(z, &self.scene, f, Some(pad)))
                } else {
                    None
                };
                if after.is_some_and(|out| out.trigger < NEVER && out.impact > out.trigger && out.danger >= base.danger)
                    && play_puff((5, 3), 12)
                {
                    diagnostics::mark(C::PuffPrestore5);
                    self.recheck_planted(z.id, (5, 3), f);
                }
            }
        }
    }
    fn choose_ash_pad(&mut self, f: &Forecast) -> Option<(Pos, Zombie, Outcome, Outcome, usize)> {
        let _perf = crate::perf::scope("planner::choose_ash_pad");
        self.begin_order(extension::Source::Ash);
        if !f
            .events
            .iter()
            .any(|e| e.at <= 2000 && matches!(e.kind, prediction::EffectKind::Doom | prediction::EffectKind::Cherry))
        {
            return None;
        }
        if !self.candidate_request_guard() {
            // Do not spend the next Puff cooldown while an already-known imminent demand remains.
            for i in 0..self.scene.zombies.len() {
                let z = self.scene.zombies[i];
                if matches!(z.row, 1 | 2 | 5 | 6)
                    && (z.giant() || z.kind == Z::Football)
                    && prediction::could_threaten(z, &self.scene, 751)
                    && self
                        .cached_outcome(z, f)
                        .unwrap_or_else(|| run_until(z, &self.scene, f, None, 751, 2))
                        .danger
                        <= 751
                {
                    return None;
                }
            }
        }
        let mut best: Option<(Pos, Zombie, Outcome, Outcome, usize)> = None;
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            if !z.giant() || !matches!(z.row, 1 | 2 | 5 | 6) {
                continue;
            }
            let eligible = prediction::retainable_ash(z, f);
            if eligible == 0 {
                continue;
            }
            // Optional ash retention must serve a real unresolved threat.
            if self.assessment(z, f).safe() {
                continue;
            }
            let horizon = f
                .events
                .iter()
                .enumerate()
                .filter(|(i, _)| eligible & (1 << i) != 0)
                .map(|(_, e)| e.at)
                .max()
                .unwrap();
            let mut baseline = None;
            for col in 1..=9 {
                let pos = (z.row, col);
                let mut pad = Pad::new(pos, PuffShroom);
                pad.offset_x = -5;
                let left = prediction::contact(z, z.x, pad.rect());
                pad.offset_x = 4;
                let right = prediction::contact(z, z.x, pad.rect());
                pad.offset_x = 0;
                if !left || !right || self.scene.main(pos).is_some() || !legal(sel(PuffShroom), pos) {
                    continue;
                }
                pad.order = self.scene.next_plant_order;
                pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, z, pos);
                if pad.lifetime <= 0 || !combat::jack_safe(pos, 1) {
                    continue;
                }
                // Fresh matching event indices for this comparison, not a cached older forecast.
                let base = *baseline.get_or_insert_with(|| run_until(z, &self.scene, f, None, horizon, 2));
                let missed = eligible & !base.ash_mask;
                if missed == 0 {
                    break;
                }
                let after = run_until(z, &self.scene, f, Some(pad), horizon, 2);
                if after.trigger >= pad.lifetime || after.trigger > 751 || !combat::jack_safe(pos, after.trigger + 1) {
                    continue;
                }
                let event = f
                    .events
                    .iter()
                    .enumerate()
                    .filter(|(j, e)| missed & after.ash_mask & (1 << j) != 0 && base.death > e.at)
                    .min_by_key(|(_, e)| e.at)
                    .map(|(j, _)| j);
                let event = event.or_else(|| self.two_pad_retention_event(z, pad, f, missed, after));
                let Some(event) = event else { continue };
                // A one- or two-card plan has useful coverage. The full outcome
                // evaluates remaining pressure; it is not an earlier-danger veto.
                let full_base = self.assessment(z, f);
                let full_after = run_until(z, &self.scene, f, Some(pad), prediction::HORIZON, 2);
                let offer = extension::Offer {
                    pos,
                    target: Some(z),
                    base: full_base,
                    after: full_after,
                    reason: 16,
                    source: extension::Source::Ash,
                };
                if self.extension.collecting {
                    self.offer_puff(offer);
                    continue;
                }
                if self.extension_waits(offer, f) {
                    continue;
                }
                if best.as_ref().is_none_or(|(old_pos, old_z, old_base, old_after, old)| {
                    if self.changed_preferences(z, full_base) || self.changed_preferences(*old_z, *old_base) {
                        self.prefer_extended(
                            offer,
                            extension::Offer {
                                pos: *old_pos,
                                target: Some(*old_z),
                                base: *old_base,
                                after: *old_after,
                                reason: 16,
                                source: extension::Source::Ash,
                            },
                            f,
                        )
                    } else if self.tuning.ash_mode {
                        self.tuning
                            .prefer_ash(full_base, full_after, event, *old_base, *old_after, *old, f)
                    } else {
                        f.events[event].at < f.events[*old].at
                    }
                }) {
                    best = Some((pos, z, full_base, full_after, event));
                }
            }
        }
        best
    }
    fn recheck_planted(&mut self, id: ZombieId, pos: Pos, f: &Forecast) {
        self.observe(rsvz::timing::clock(), self.last_ice);
        self.cache.remove(&id);
        if let Some(z) = self.scene.zombies.iter().find(|z| z.id == id).copied() {
            let actual = self.assessment(z, f);
            telemetry::record(
                "pad_after_plant",
                &[
                    id.raw() as i64,
                    pos.0 as i64,
                    pos.1 as i64,
                    actual.death as i64,
                    actual.danger as i64,
                    actual.ash_hits as i64,
                    actual.gloom_damage as i64,
                ],
            );
            if !actual.safe() {
                diagnostics::mark(C::NeedsAnotherPad);
            }
        }
    }
    pub fn required_squash(&mut self, w: i32, ash_births: u32, forecast: &Forecast) -> bool {
        let _perf = crate::perf::scope("planner::required_squash");
        self.ash_births = ash_births;
        // Placement admission is not a permission to lend this card away.
        // Keep a lazy demand check even when no immediate placement is useful.
        self.squash5_deferred = self.scene.row_zombies(5).any(|z| z.giant());
        self.reserve_squash5 = self.scene.plants.iter().any(|p| p.kind == Squash && p.pos.0 == 5);
        if !ready(sel(Squash)) || self.reserve_squash5 {
            return false;
        }
        self.required.clear();
        if !self.scene.row_zombies(5).any(|z| {
            if !z.giant() || !(z.carried || z.wave <= w) {
                return false;
            }
            let (hits, squashed) = telemetry::damage_history(z.id);
            !squashed
                && (z.kind == Z::Gargantuar && z.x < 400.
                    || z.kind == Z::GigaGargantuar && (ash_births & (1 << z.wave) != 0 || hits < 2))
        }) {
            return false;
        }
        if self.keep_squash_group_wait(forecast) {
            return false;
        }
        let windows = self.squash_windows(5, true);
        if windows.iter().all(Option::is_none) {
            self.squash5_deferred = true;
            return false;
        }
        self.required.clear();
        let mut first_deadline = None;
        for index in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[index];
            let (hits, squashed) = telemetry::damage_history(z.id);
            if z.row == 5
                && z.giant()
                && !squashed
                && (z.kind == Z::Gargantuar && z.x < 400.
                    || z.kind == Z::GigaGargantuar && (ash_births & (1 << z.wave) != 0 || hits < 2))
                && (z.carried || z.wave <= w)
            {
                // No immediate row-five placement is considered this far right.
                // Evaluate the reserve only if another row is actually about to use the card.
                if z.x >= 620. || !windows.iter().flatten().any(|&window| squash::could_hit(z, window)) {
                    self.squash5_deferred = true;
                    continue;
                }
                let outcome = self.assessment(z, forecast);
                if outcome.safe() {
                    continue;
                }
                self.reserve_squash5 |= outcome.danger <= 3183;
                self.required.push(z.id);
                if first_deadline.is_none_or(|(_, deadline)| outcome.danger < deadline) {
                    first_deadline = Some((z.id, outcome.danger));
                }
            }
        }
        self.reserve_squash5 |= !self.required.is_empty();
        // A missing historical hit does not require Squash when one real shared
        // Puff plus the known ice/ash plan resolves the entire group. Check reds
        // as well as whites, retaining the concrete cross-lane conflict checks.
        // Do not defer on a distant deadline alone: it can lose a usable damage
        // window before later targets arrive or the card is needed again.
        if let Some((id, _)) = first_deadline {
            let z = *self.scene.zombies.iter().find(|z| z.id == id).unwrap();
            if self.try_economy_puff(z, forecast, true) {
                return true;
            }
        }
        let Some(first) = first_deadline else {
            return false;
        };
        let original = self.required.clone();
        let now = Self::squash_subset(&self.scene, forecast, &original, first.0);
        let covered = now.as_ref().map_or(0, |(_, ids)| ids.len());
        if covered < original.len()
            && self.prepare_squash_group(first, forecast, covered, now.as_ref().map(|(p, _)| *p))
        {
            return true;
        }
        self.required = now.map_or_else(|| vec![first.0], |(_, ids)| ids);
        if self.place_squash(5, forecast, 13) {
            if self.required.len() < original.len() {
                telemetry::record(
                    "squash_partial_group",
                    &[
                        first.0.raw() as i64,
                        first.1 as i64,
                        original.len() as i64,
                        self.required.len() as i64,
                    ],
                );
            }
            return true;
        }
        false
    }
    fn squash_windows(&self, row: i32, clear_puff: bool) -> [Option<(f32, f32)>; 9] {
        let mut windows = [None; 9];
        for col in 1..=9 {
            let pos = (row, col);
            if (clear_puff || self.scene.main(pos).is_none())
                && combat::candidate(sel(Squash), pos)
                && combat::jack_safe(pos, 182)
            {
                windows[(col - 1) as usize] = squash::possible_landing(&self.scene, pos);
            }
        }
        windows
    }
    fn deferred_squash5_needed(&self, f: &Forecast) -> bool {
        if !self.squash5_deferred {
            return false;
        }
        self.scene.zombies.iter().any(|&z| {
            if z.row != 5 || !z.giant() {
                return false;
            }
            // Validate on the current scene only when another row is about to spend the card.
            let out = run_until(z, &self.scene, f, None, 3183, 0);
            out.danger <= 3183
        })
    }
    pub fn support_squash(&mut self, w: i32, forecast: &Forecast) -> bool {
        let _perf = crate::perf::scope("planner::support_squash");
        if !ready(sel(Squash)) || self.reserve_squash5 || self.scene.now < self.next_support_squash {
            return false;
        }
        // Preserve the card for the upcoming row-five tail group.
        if w == 19
            && forecast
                .events
                .iter()
                .any(|e| e.kind == prediction::EffectKind::Jalapeno && e.at < 3001)
        {
            return false;
        }
        self.next_support_squash = self.scene.now + 25;
        let windows = [
            self.squash_windows(1, false),
            self.squash_windows(2, false),
            self.squash_windows(5, false),
            self.squash_windows(6, false),
        ];
        let mut targets: [Option<(Zombie, Outcome)>; 4] = [None; 4];
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            let index = match z.row {
                1 => 0,
                2 => 1,
                5 if z.kind == Z::Football => 2,
                6 => 3,
                _ => continue,
            };
            if !(z.giant() || z.kind == Z::Football)
                || self.puff_reserved_for_blover(z)
                || z.x > 650.
                || !windows[index]
                    .iter()
                    .flatten()
                    .any(|&window| squash::could_hit(z, window))
            {
                continue;
            }
            // Existing ash coverage is not a reason to spend an extra Squash.
            let base = self.assessment(z, forecast);
            if base.safe() || base.danger < 182 {
                continue;
            }
            if targets[index].is_none_or(|(_, old)| base.danger < old.danger) {
                targets[index] = Some((z, base));
            }
        }
        // Prioritize the earliest remaining danger under the shared plan.
        targets.sort_by_key(|target| target.map_or(NEVER, |(_, base)| base.danger));
        let quiet = targets.iter().any(Option::is_some)
            && !targets.iter().flatten().any(|(z, _)| z.kind == Z::GigaGargantuar)
            && !self.red_pressure(forecast);
        for (z, base) in targets.into_iter().flatten() {
            // Spend a free Puff only when this concrete current request wins the
            // normal planner and is resolved, with no other demand before its CD.
            if quiet && self.try_economy_puff(z, forecast, false) {
                return true;
            }
            let early = base.danger > 751 + 182;
            if early && !(z.kind == Z::GigaGargantuar && z.x <= 500.) {
                continue;
            }
            // Preserve all planned ash when evaluating relief.
            let plan = forecast;
            if self.spare_puff() {
                if let Some((pos, target, before, after)) = self.choose_pad(PuffShroom, plan) {
                    if target.id == z.id && after.safe() && after.death < NEVER {
                        let conflict = self.scene.zombies.iter().copied().any(|other| {
                            other.id != z.id
                                && (other.giant() || other.kind == Z::Football)
                                && prediction::could_threaten(other, &self.scene, 751)
                                && run_until(other, &self.scene, forecast, None, 751, 2).danger <= 751
                        });
                        if !conflict && self.puff_spend_allowed(pos, forecast) {
                            self.record_choice(target, before, after, pos, PuffShroom, plan);
                            if play_puff(pos, if target.giant() { 10 } else { 11 }) {
                                telemetry::record("economy_puff", &[z.id.raw() as i64, z.row as i64]);
                                self.recheck_planted(z.id, pos, forecast);
                                return true;
                            }
                        }
                    }
                }
            }
            // 2009: an imminent Cherry stopped a giant before squash takeoff. Until
            // that plant exists, the short lock projection cannot assume free walking.
            // Wait only for a concrete same-row planting/contact window, then observe it.
            if forecast.events.iter().any(|e| {
                !e.certain
                    && e.pos.0 == z.row
                    && (100..=282).contains(&e.at)
                    && matches!(
                        e.kind,
                        prediction::EffectKind::Cherry | prediction::EffectKind::Jalapeno
                    )
                    && self.scene.zombies.iter().any(|actor| {
                        let rect = Pad::new(e.pos, CherryBomb).rect();
                        actor.row == z.row
                            && actor.giant()
                            && actor.x >= (rect.left - 39) as f32
                            && actor.x - prediction::max_step(actor.kind) * 182. <= (rect.right + 10) as f32
                    })
            }) {
                continue;
            }
            self.required.clear();
            self.required.push(z.id);
            for col in (1..=9).rev() {
                let pos = (z.row, col);
                if self.scene.main(pos).is_some()
                    || !combat::candidate(sel(Squash), pos)
                    || !combat::jack_safe(pos, 182)
                    || prediction::external_destruction_bound(&self.scene, z, Pad::new(pos, Squash)) <= 126
                {
                    continue;
                }
                let Some(choice) = self.squash.evaluate(&self.scene, plan, pos, &self.required, false) else {
                    continue;
                };
                if !self.squash.preserves_early_refresh(choice, w, time().2) {
                    continue;
                }
                let mut pad = Pad::new(pos, Squash);
                pad.order = self.scene.next_plant_order;
                pad.lifetime = 126;
                pad.complete_outcome = true;
                pad.squash_victim = Some(z.id);
                let after = run(z, &self.scene, plan, Some(pad));
                if !after.squash_hit || early && (!after.safe() || after.death == NEVER) {
                    continue;
                }
                if self.deferred_squash5_needed(forecast) {
                    self.reserve_squash5 = true;
                    return false;
                }
                telemetry::record(
                    "squash_prediction",
                    &[
                        z.row as i64,
                        col as i64,
                        choice.locks as i64,
                        (choice.left * 100.) as i64,
                        (choice.right * 100.) as i64,
                        choice.guaranteed as i64,
                    ],
                );
                self.squash.log_choice();
                telemetry::record("squash_required", &[z.id.raw() as i64, z.row as i64]);
                if place(sel(Squash), pos, 17, false) {
                    if early {
                        telemetry::record(
                            "early_support_squash",
                            &[
                                z.id.raw() as i64,
                                z.row as i64,
                                base.danger as i64,
                                after.death as i64,
                                0,
                            ],
                        );
                    }
                    telemetry::record(
                        "support_squash",
                        &[
                            z.id.raw() as i64,
                            z.row as i64,
                            z.wave as i64,
                            z.kind as i64,
                            base.death as i64,
                            base.danger as i64,
                            after.death as i64,
                            after.danger as i64,
                            base.hp_at_end as i64,
                            after.hp_at_end as i64,
                        ],
                    );
                    self.invalidate_row(z.row);
                    return true;
                }
            }
        }
        false
    }
    fn place_squash(&mut self, row: i32, forecast: &Forecast, reason: i32) -> bool {
        self.place_squash_except(row, forecast, reason, None)
    }
    fn place_squash_except(&mut self, row: i32, forecast: &Forecast, reason: i32, excluded: Option<Pos>) -> bool {
        if self.required.is_empty() || !ready(sel(Squash)) {
            return false;
        }
        for col in (1..=9).rev() {
            let pos = (row, col);
            if Some(pos) == excluded || !combat::candidate(sel(Squash), pos) || !combat::jack_safe(pos, 182) {
                continue;
            }
            let choice = self
                .squash
                .evaluate(&self.scene, forecast, pos, &self.required, reason == 6);
            let choice = choice.or_else(|| {
                if reason == 15 && time().1 == 20 && self.scene.main(pos).is_none() {
                    self.squash.prestore(&self.scene, forecast, pos, &self.required)
                } else {
                    None
                }
            });
            let Some(choice) = choice else {
                continue;
            };
            // Emergency rescue (reason 9) may deliberately change the wave;
            // ordinary required/grouping uses retain the pre-2300 constraint.
            if reason != 9 && !self.squash.preserves_early_refresh(choice, time().1, time().2) {
                continue;
            }
            // Lock coverage is not proof the plant survives to takeoff.
            let Some(victim) = self
                .scene
                .row_zombies(row)
                .copied()
                .find(|z| self.required.contains(&z.id))
            else {
                continue;
            };
            let mut pad = Pad::new(pos, Squash);
            pad.order = self.scene.next_plant_order;
            pad.squash_wait = choice.wait;
            pad.lifetime = choice.wait + 126;
            pad.complete_outcome = true;
            pad.squash_victim = Some(victim.id);
            // place(..., clear_puff=true) shovels this occupant before planting.
            let replaced = self
                .scene
                .plants
                .iter()
                .position(|p| p.pos == pos && p.kind == PuffShroom)
                .map(|i| (i, self.scene.plants.remove(i)));
            let proof = run_until(victim, &self.scene, forecast, Some(pad), choice.wait + 183, 3);
            if let Some((i, plant)) = replaced {
                self.scene.plants.insert(i, plant);
            }
            if !proof.squash_hit {
                continue;
            }
            if choice.wait > 0 && !combat::jack_safe(pos, choice.wait + 182) {
                continue;
            }
            if choice.wait > 0 {
                telemetry::record(
                    "squash_prestore",
                    &[row as i64, col as i64, choice.wait as i64, self.required.len() as i64],
                );
            }
            telemetry::record(
                "squash_prediction",
                &[
                    row as i64,
                    col as i64,
                    choice.locks as i64,
                    (choice.left * 100.) as i64,
                    (choice.right * 100.) as i64,
                    choice.guaranteed as i64,
                ],
            );
            self.squash.log_choice();
            for id in &self.required {
                telemetry::record("squash_required", &[id.raw() as i64, row as i64]);
            }
            if place(sel(Squash), choice.pos, reason, true) {
                if reason == 13 {
                    diagnostics::mark(C::RequiredSquash5);
                }
                self.invalidate_row(row);
                return true;
            }
        }
        diagnostics::mark(C::SquashAmbiguous);
        false
    }
    fn without_ash(forecast: &Forecast, kind: prediction::EffectKind) -> Forecast {
        let mut without = Forecast::default();
        let canceled = forecast.events.iter().position(|e| !e.certain && e.kind == kind);
        without.events.extend(
            forecast
                .events
                .iter()
                .enumerate()
                .filter(|(index, _)| Some(*index) != canceled)
                .map(|(_, e)| *e),
        );
        without
    }
    fn red_pressure(&self, f: &Forecast) -> bool {
        let key = forecast_key(f);
        if let Some((revision, forecast, result)) = self.red_pressure_cache.get() {
            if revision == self.scene.revision && forecast == key {
                return result;
            }
        }
        let result = self.scene.zombies.iter().copied().any(|z| {
            z.kind == Z::GigaGargantuar
                && !self
                    .cached_outcome(z, f)
                    .unwrap_or_else(|| run(z, &self.scene, f, None))
                    .safe()
        });
        self.red_pressure_cache.set(Some((self.scene.revision, key, result)));
        result
    }
    fn economy_puff_choice(&mut self, z: Zombie, f: &Forecast) -> Option<(Pos, Outcome)> {
        if !self.spare_puff() || self.puff_reserved_for_blover(z) {
            return None;
        }
        let base = self.assessment(z, f);
        if self.wait_for_old_hammer(f, Some((z, base))) {
            return None;
        }
        if matches!(z.row, 1 | 6)
            && (self.scene.row_zombies(z.row).any(|actor| actor.kind == Z::Catapult)
                || rsvz::with_frame(|board| {
                    board
                        .projectiles()
                        .any(|shot| shot.kind_code() == 9 && shot.row() + 1 == z.row)
                }))
        {
            return None;
        }
        let forward_retention = matches!(time().1, 10 | 11) && matches!(z.row, 2 | 5) && z.kind == Z::GigaGargantuar;
        let candidates = if forward_retention {
            &[3, 4, 5, 6, 7][..]
        } else {
            pad_columns(z)
        };
        for &col in candidates {
            let pos = (z.row, col);
            if self.scene.main(pos).is_some() || !legal(sel(PuffShroom), pos) {
                continue;
            }
            let mut pad = Pad::new(pos, PuffShroom);
            pad.order = self.scene.next_plant_order;
            pad.trigger_window = if z.kind == Z::Football { 2500 } else { 751 };
            pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, z, pos);
            if !prediction::could_contact_pad(z, pad) {
                continue;
            }
            let out = run(z, &self.scene, f, Some(pad));
            if out.trigger >= pad.lifetime
                || out.trigger > pad.trigger_window
                || !out.safe()
                || out.death == NEVER
                || !combat::jack_safe(pos, out.trigger.min(126))
            {
                continue;
            }
            if [-5, 4].into_iter().any(|offset| {
                let mut edge = pad;
                edge.offset_x = offset;
                let edge_out = run(z, &self.scene, f, Some(edge));
                !edge_out.safe() || edge_out.death == NEVER || edge_out.trigger >= edge.lifetime
            }) {
                continue;
            }
            // One planted Puff must cover the promised group, not one fictional
            // fresh plant per zombie. Other lanes retain their immediate budget.
            let conflict = self.scene.zombies.iter().copied().any(|other| {
                if other.id == z.id || !(other.giant() || other.kind == Z::Football) {
                    return false;
                }
                if other.row == z.row {
                    if !self.required.contains(&other.id) {
                        return false;
                    }
                    let mut shared = pad;
                    shared.lifetime =
                        shared
                            .lifetime
                            .min(prediction::puff_external_destruction_bound(&self.scene, other, pos));
                    let after = run(other, &self.scene, f, Some(shared));
                    !after.safe() || after.death == NEVER
                } else {
                    prediction::could_threaten(other, &self.scene, 751)
                        && run_until(other, &self.scene, f, None, 751, 2).danger <= 751
                }
            });
            if !conflict && self.puff_spend_allowed(pos, f) {
                return Some((pos, out));
            }
        }
        None
    }
    fn try_economy_puff(&mut self, z: Zombie, f: &Forecast, group: bool) -> bool {
        if !group {
            self.required.clear();
            self.required.push(z.id);
        }
        let Some((pos, after)) = self.economy_puff_choice(z, f) else {
            return false;
        };
        let before = run(z, &self.scene, f, None);
        self.record_choice(z, before, after, pos, PuffShroom, f);
        if play_puff(pos, if z.giant() { 10 } else { 11 }) {
            telemetry::record("economy_puff", &[z.id.raw() as i64, z.row as i64, after.trigger as i64]);
            self.recheck_planted(z.id, pos, f);
            return true;
        }
        false
    }
    /// Remove only this use, leaving later real card opportunities intact.
    /// Row-six traffic is never waived using the upper lanes' natural output.
    pub fn ash_unneeded(&mut self, forecast: &Forecast, kind: prediction::EffectKind, rows: &[i32]) -> bool {
        let _perf = crate::perf::scope("planner::ash_unneeded");
        self.economy_rejection = [kind as i64, 0, 0, 0, 0, 0, 0, 0];
        let canceled = forecast.events.iter().position(|e| !e.certain && e.kind == kind);
        let without = Self::without_ash(forecast, kind);
        // The user strategy deliberately budgets future row-one blocks here.
        // It does not require the whole football group to die from ONE Puff now.
        // No red quota/turn-white counter: use the current visible threat proof.
        let quiet_cherry = kind == prediction::EffectKind::Cherry && !self.red_pressure(&without);
        self.required.clear();
        let mut pad_target: Option<(Zombie, i32)> = None;
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            if !rows.contains(&z.row) {
                continue;
            }
            if !(z.giant() || matches!(z.kind, Z::Football | Z::Zomboni | Z::Catapult)) {
                continue;
            }
            if quiet_cherry && z.row == 1 && z.kind == Z::Football {
                continue;
            }
            if z.row == 6 && matches!(z.kind, Z::Football | Z::Zomboni | Z::Catapult) {
                self.economy_rejection = [
                    kind as i64,
                    1,
                    z.id.raw() as i64,
                    z.kind as i64,
                    z.row as i64,
                    z.hp as i64,
                    0,
                    0,
                ];
                return false;
            }
            if z.kind == Z::Catapult
                && without.events.iter().any(|e| {
                    e.at <= self.scene.protected_catapult_until(z)
                        && e.shape.is_some_and(|shape| {
                            (z.row - e.pos.0).abs() <= shape.row_range
                                && rsvz::core::contact::circle_hits_rect(shape.circle, z.rect)
                        })
                })
            {
                continue;
            }
            let out = run(z, &self.scene, &without, None);
            if out.safe() && out.death < NEVER {
                self.keep_nuke_dependency(z, out, &without);
                continue;
            }
            // An unsafe giant that this Cherry cannot hit is not a reason to
            // spend it. If saving requires Puff, the separate whole-field red
            // pressure guard below still protects that contested card budget.
            if kind == prediction::EffectKind::Cherry
                && z.giant()
                && canceled.is_some_and(|index| run(z, &self.scene, forecast, None).ash_mask & (1 << index) == 0)
            {
                continue;
            }
            if !(z.kind == Z::Football && z.row == 1 || z.kind == Z::Gargantuar) {
                self.economy_rejection = [
                    kind as i64,
                    2,
                    z.id.raw() as i64,
                    z.kind as i64,
                    z.row as i64,
                    z.hp as i64,
                    out.death as i64,
                    out.danger as i64,
                ];
                return false;
            }
            self.required.push(z.id);
            if pad_target.is_none_or(|(_, danger)| out.danger < danger) {
                pad_target = Some((z, out.danger));
            }
        }
        if let Some((z, _)) = pad_target {
            if self.red_pressure(&without) || !self.try_economy_puff(z, &without, true) {
                self.economy_rejection = [
                    kind as i64,
                    3,
                    z.id.raw() as i64,
                    z.kind as i64,
                    z.row as i64,
                    z.hp as i64,
                    cd(sel(PuffShroom)) as i64,
                    0,
                ];
                return false;
            }
        }
        true
    }
    pub fn log_economy_rejection(&self) {
        telemetry::record("economy_rejected", &self.economy_rejection);
        if self.economy_rejection[0] == 3 && self.economy_rejection[1] == 6 {
            for z in self.scene.zombies.iter().filter(|z| self.required.contains(&z.id)) {
                telemetry::record(
                    "economy_uncovered",
                    &[
                        z.id.raw() as i64,
                        z.kind as i64,
                        z.row as i64,
                        (z.x * 100.) as i64,
                        z.hp as i64,
                        z.armor as i64,
                        z.phase as i64,
                        z.frozen as i64,
                    ],
                );
            }
        }
    }
    pub fn cherry_position(&mut self, forecast: &Forecast, at: i32, flag: bool) -> Option<Pos> {
        let legal = self.cherry_sites(at);
        cherry::select(&self.scene, forecast, at, legal, flag, &mut self.cherry_guard)
    }

    pub fn economy_jalapeno(&mut self, forecast: &Forecast) -> bool {
        let _perf = crate::perf::scope("planner::economy_jalapeno");
        let wave = time().1;
        if self
            .scene
            .row_zombies(6)
            .any(|z| z.kind == Z::Football && (z.carried || z.wave < wave) && z.x < 200.)
        {
            self.economy_rejection = [3, 7, 0, 0, 6, 0, 0, 0];
            return false;
        }
        // An older white that still lacks its first ash hit is the fallback
        // responsibility of this pepper, not another deferred future nuke.
        if self.scene.row_zombies(6).any(|z| {
            z.kind == Z::Gargantuar
                && (z.carried || z.wave < wave)
                && z.hp > 1200
                && telemetry::damage_history(z.id).0 == 0
        }) {
            self.economy_rejection = [3, 8, 0, 0, 6, 0, 0, 0];
            return false;
        }
        if self.ash_unneeded(forecast, prediction::EffectKind::Jalapeno, &[6]) {
            telemetry::record("economy_jalapeno_empty", &[]);
            return true;
        }
        if !ready(sel(Squash)) || self.reserve_squash5 || self.deferred_squash5_needed(forecast) {
            self.economy_rejection = [
                3,
                4,
                cd(sel(Squash)) as i64,
                self.reserve_squash5 as i64,
                self.squash5_deferred as i64,
                0,
                0,
                0,
            ];
            return false;
        }
        let without = Self::without_ash(forecast, prediction::EffectKind::Jalapeno);
        // Keep deferred whites in the scene: they can still steal the lock.
        // Only waive the damage obligation, with short-term danger still checked.
        let deferred_white = |z: Zombie| {
            z.kind == Z::Gargantuar
                && z.x > 400.
                && (z.hp <= 1200
                    || telemetry::damage_history(z.id).0 > 0
                    || !z.carried
                        && z.wave == wave
                        && without.events.iter().any(|e| e.kind == prediction::EffectKind::Doom))
                && run_until(z, &self.scene, &without, None, 751, 2).danger > 751
        };
        self.required.clear();
        let mut naturally_safe = 0;
        for z in &self.scene.zombies {
            if z.row != 6 || !(z.giant() || matches!(z.kind, Z::Football | Z::Zomboni)) {
                continue;
            }
            if deferred_white(*z) {
                naturally_safe += 1;
                continue;
            }
            if z.giant() {
                let base = run(*z, &self.scene, &without, None);
                if base.safe() && base.death < NEVER {
                    // It remains an actor in the squash lock projection. Only the
                    // obligation to hit it is removed, not its ability to steal a lock.
                    naturally_safe += 1;
                    continue;
                }
            }
            if z.hp + z.armor > 1800 && !z.giant() {
                return false;
            }
            self.required.push(z.id);
        }
        if self.required.is_empty() {
            return false;
        }
        // Do not use up the shared Squash while another visible red is likely
        // to need emergency damage before this card recovers.
        if self.scene.zombies.iter().copied().any(|z| {
            z.row != 6
                && z.kind == Z::GigaGargantuar
                && run_until(z, &self.scene, &without, None, 3183, 2).danger <= 3183
        }) {
            self.economy_rejection[1] = 5;
            return false;
        }
        self.economy_rejection[1] = 6;
        for col in (1..=9).rev() {
            let pos = (6, col);
            if !combat::candidate(sel(Squash), pos) || !combat::jack_safe(pos, 182) {
                continue;
            }
            let choice = self
                .squash
                .evaluate(&self.scene, &without, pos, &self.required, true)
                .or_else(|| self.squash.prestore(&self.scene, &without, pos, &self.required));
            let Some(choice) = choice else { continue };
            if !self.squash.preserves_early_refresh(choice, time().1, time().2) {
                continue;
            }
            if !combat::jack_safe(pos, choice.wait + 182) {
                continue;
            }
            let mut pad = Pad::new(pos, Squash);
            pad.order = self.scene.next_plant_order;
            pad.squash_wait = choice.wait;
            pad.lifetime = choice.wait + 126;
            if self.scene.zombies.iter().filter(|z| z.row == 6 && z.giant()).any(|z| {
                if deferred_white(*z) {
                    return false;
                }

                let mut target_pad = pad;
                target_pad.squash_victim = self.squash.guarantees_hit(choice, z.id).then_some(z.id);
                let after = run(*z, &self.scene, &without, Some(target_pad));
                self.required.contains(&z.id) && !after.squash_hit || !after.safe() || after.death == NEVER
            }) {
                continue;
            }
            telemetry::record(
                "squash_prediction",
                &[
                    6,
                    col as i64,
                    choice.locks as i64,
                    (choice.left * 100.) as i64,
                    (choice.right * 100.) as i64,
                    choice.guaranteed as i64,
                ],
            );
            self.squash.log_choice();
            for id in &self.required {
                telemetry::record("squash_required", &[id.raw() as i64, 6]);
            }
            if place(sel(Squash), pos, 22, true) {
                self.economy_victims.clear();
                self.economy_victims.extend(
                    self.scene
                        .zombies
                        .iter()
                        .filter(|z| self.required.contains(&z.id))
                        .map(|z| (z.id, z.hp + z.armor)),
                );
                telemetry::record("economy_required", &[self.required.len() as i64, naturally_safe]);
                self.invalidate_row(6);
                return true;
            }
        }
        false
    }
    /// Only an already planted, proven replacement can discharge clearance.
    pub fn sixth_clearance_pending(&self) -> bool {
        self.scene.plants.iter().any(|p| p.pos.0 == 6 && p.kind == Jalapeno)
            || self.scene.plants.iter().any(|p| p.pos.0 == 6 && p.kind == Squash)
                && self
                    .scene
                    .row_zombies(6)
                    .filter(|z| z.kind == Z::Zomboni)
                    .all(|z| self.economy_victims.iter().any(|(id, _)| *id == z.id))
    }
    pub fn economy_squash_missed(&mut self) -> bool {
        let mut missed = false;
        for (id, before) in self.economy_victims.drain(..) {
            if let Some(z) = self.scene.zombies.iter().find(|z| z.id == id) {
                let damage = before - z.hp - z.armor;
                telemetry::record(
                    "economy_squash_result",
                    &[
                        id.raw() as i64,
                        before as i64,
                        z.hp as i64,
                        z.armor as i64,
                        damage as i64,
                    ],
                );
                missed |= damage < 1800;
            } else {
                telemetry::record("economy_squash_result", &[id.raw() as i64, before as i64, -1, 0, 1800]);
            }
        }
        missed
    }
    pub fn fast_squash(&mut self, row: i32, forecast: &Forecast) -> bool {
        if !ready(sel(Squash)) || row != 5 && self.reserve_squash5 {
            return false;
        }
        self.required.clear();
        for z in &self.scene.zombies {
            if z.row == row && matches!(z.kind, Z::Football | Z::Zomboni) {
                self.required.push(z.id);
            }
        }
        self.place_squash(row, forecast, 6)
    }
    pub fn rescue_squash(&mut self, row: i32, forecast: &Forecast) -> bool {
        if !ready(sel(Squash)) {
            return false;
        }
        let mut target = None;
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            if z.row != row || !(z.giant() || z.kind == Z::Football) || z.x > 400. {
                continue;
            }
            let out = self.assessment(z, forecast);
            if (182..=400).contains(&out.danger) && target.is_none_or(|(_, old)| out.danger < old) {
                target = Some((z.id, out.danger));
            }
        }
        self.required.clear();
        if let Some((id, _)) = target {
            self.required.push(id);
        }
        self.place_squash(row, forecast, 9)
    }
    pub fn late_ice_floor(&self, wave: i32, time: i32, include_football: bool) -> i32 {
        let _perf = crate::perf::scope("planner::late_ice_floor");
        let mut result = time + 100;
        for z in &self.scene.zombies {
            if z.carried
                || z.wave != wave
                || !matches!(z.row, 2 | 5)
                || z.frozen > 0
                || !(z.kind == Z::JackInTheBox && z.phase == ZombiePhase::JackInTheBoxRunning
                    || include_football && z.row == 5 && z.kind == Z::Football)
            {
                continue;
            }
            let Some(gun) = self
                .scene
                .plants
                .iter()
                .find(|p| p.kind == GloomShroom && p.pos.1 == 6 && (p.pos.0 - z.row).abs() <= 1)
            else {
                continue;
            };
            let mut x = z.x;
            let mut phase = z.progress;
            for dt in 0..=1800 {
                if z.rect_at(x).left < gun.attack.right {
                    result = result.max(time + dt + 2);
                    break;
                }
                prediction::walk_kind(&mut x, &mut phase, z.speed, z.chilled > dt, z.kind);
            }
        }
        result
    }
    pub fn old_football_needs_ice(&mut self, wave: i32, f: &Forecast) -> bool {
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            if z.row == 1 && z.kind == Z::Football && (z.carried || z.wave < wave) && !self.assessment(z, f).safe() {
                return true;
            }
        }
        false
    }
    pub fn unobstructed_ice_floor(&self, wave: i32, time: i32, mut floor: i32) -> i32 {
        for &z in &self.scene.zombies {
            if z.wave != wave
                || z.carried
                || z.kind != Z::JackInTheBox
                || !matches!(z.row, 2 | 5)
                || z.phase != ZombiePhase::JackInTheBoxRunning
                || !self
                    .scene
                    .plants
                    .iter()
                    .any(|p| p.pos.0 == z.row && p.pos.1 >= 7 && !matches!(p.kind, LilyPad | FlowerPot))
            {
                continue;
            }
            if let Some(gun) = self
                .scene
                .plants
                .iter()
                .find(|p| p.kind == GloomShroom && p.pos.1 == 6 && (p.pos.0 - z.row).abs() <= 1)
            {
                let (entry, death) = prediction::first_common_entry(z, &self.scene, [gun.attack; 2]);
                if death >= entry {
                    floor = floor.max(time + entry + 2);
                }
            }
        }
        floor
    }
    pub fn emergency(&mut self, f: &Forecast, allow_cherry: bool) -> [bool; 6] {
        let _perf = crate::perf::scope("planner::emergency");
        let mut rows = [false; 6];
        let squash = ready(sel(Squash));
        let cherry = allow_cherry && ready(sel(CherryBomb));
        for index in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[index];
            if !matches!(z.row, 1 | 2 | 5 | 6)
                || !(z.giant() || z.kind == Z::Football || z.kind == Z::Imp && z.phase == ZombiePhase::ZombieNormal)
                || !(squash || cherry && matches!(z.row, 1 | 2))
                || !prediction::could_threaten(z, &self.scene, 400)
            {
                continue;
            }
            let out = self
                .cached_outcome(z, f)
                .unwrap_or_else(|| run_until(z, &self.scene, f, None, 400, 5));
            if out.danger <= 400 {
                rows[(z.row - 1) as usize] = true;
                if self.scene.now % 100 == 0 {
                    telemetry::record(
                        "urgent_demand",
                        &[
                            z.id.raw() as i64,
                            z.row as i64,
                            z.hp as i64,
                            out.danger as i64,
                            out.death as i64,
                        ],
                    );
                }
            }
        }
        rows
    }

    /// Advance this wave's Cherry only; the controller owns the200cs time gate.
    pub fn advance_cherry(&mut self, row: i32, forecast: &Forecast) -> bool {
        if !matches!(row, 1 | 2) || !ready(sel(CherryBomb)) {
            return false;
        }
        let target = self
            .scene
            .row_zombies(row)
            .copied()
            .filter(|z| z.giant() || z.kind == Z::Football || z.kind == Z::Imp && z.phase == ZombiePhase::ZombieNormal)
            .filter(|z| prediction::could_threaten(*z, &self.scene, 400))
            .filter_map(|z| {
                let before = run_until(z, &self.scene, forecast, None, 400, 5);
                (before.danger < NEVER).then_some((z, before))
            })
            .min_by_key(|(_, before)| before.danger);
        let Some((z, before)) = target else {
            return false;
        };
        for i in (0..cherry::CANDIDATES).rev() {
            let pos = cherry::position(i);
            if !combat::candidate(sel(CherryBomb), pos)
                || !combat::cherry_preserves_ladders(pos)
                || !combat::jack_safe(pos, 100)
            {
                continue;
            }
            let mut after_plan = Self::without_ash(forecast, prediction::EffectKind::Cherry);
            let event = after_plan.events.len();
            after_plan.add(100, prediction::EffectKind::Cherry, pos, false);
            let replaced = self
                .scene
                .plants
                .iter()
                .position(|p| p.pos == pos && p.kind == PuffShroom)
                .map(|index| (index, self.scene.plants.remove(index)));
            let after = run_until(z, &self.scene, &after_plan, None, 400, 5);
            if let Some((index, plant)) = replaced {
                self.scene.plants.insert(index, plant);
            }
            if after.ash_mask & (1 << event) == 0 || after.danger <= before.danger {
                continue;
            }
            if place(sel(CherryBomb), pos, 9, true) {
                self.register_cherry_misses(forecast, pos);
                telemetry::record(
                    "early_cherry_target",
                    &[
                        z.id.raw() as i64,
                        row as i64,
                        pos.0 as i64,
                        pos.1 as i64,
                        before.danger as i64,
                        after.danger as i64,
                    ],
                );
                return true;
            }
        }
        false
    }
}
