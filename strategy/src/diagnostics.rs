//! Counters and a fixed ring only on the hot path. Serialization is at trial/session boundaries.
use super::*;
use rsvz::core::model::{ActivationFact, HomeEntryEvent, PlantEffectEvent, WorldResetConfig};
use rsvz::tick::{TickLane, TickLifetime, TickPhase};
use std::io::{self, Write};

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Condition {
    LateIce,
    IceNoGrid,
    IceCd,
    IceLost,
    LilyUnavailable,
    DoomNoGrid,
    DoomLeftMiss,
    CherryBlocked,
    SquashAmbiguous,
    SquashMiss,
    SquashLost,
    PadConflict,
    PuffUnavailable,
    OldFootball5,
    PuffPrestore5,
    ImpAtIce,
    BasketballRisk,
    DolphinAtIce,
    FrozenModel,
    PumpkinLost,
    PumpkinRepaired,
    PumpkinWaiting,
    RedAbsent,
    AshSaved,
    EmergencyAsh,
    W8Critical,
    W10Early,
    ShortWave,
    TailResidual,
    BalloonWaiting,
    PlantRejected,
    PadDestroyed,
    LowSun,
    LateJalapeno,
    Timeout,
    HighHealthGap,
    NeedsAnotherPad,
    RequiredSquash5,
    BalloonQuota,
}
const N: usize = 39;
const NAMES: [&str; N] = [
    "late_ice",
    "ice_no_safe_grid",
    "ice_cooldown_delay",
    "ice_destroyed",
    "lily_unavailable",
    "doom_no_grid",
    "doom_left_targets",
    "cherry_blocked",
    "squash_competing_targets",
    "squash_required_fast_remains",
    "squash_destroyed",
    "simultaneous_pad_demands",
    "puff_unavailable",
    "old_row5_football",
    "row5_col3_prestore",
    "imp_at_ice_grid",
    "basketball_candidate_risk",
    "dolphin_at_water_ice",
    "ice_model_frozen_domain",
    "pumpkin_destroyed",
    "pumpkin_repaired",
    "pumpkin_repair_waiting",
    "no_live_red",
    "ash_saved",
    "emergency_ash",
    "wave8_2725_to_2969",
    "wave10_early_nuke",
    "short_wave",
    "wave20_old_residual",
    "blover_unavailable",
    "plant_rejected",
    "fodder_destroyed",
    "low_sun",
    "jalapeno_crossed_wave",
    "trial_timeout",
    "three_hit_red_final_ash_gap",
    "needs_another_intervention_after_pad",
    "required_row5_squash",
    "balloon_near_home_but_two_wave_quota_used",
];
#[derive(Clone, Copy, Default)]
struct Record {
    clock: i32,
    wave: i32,
    time: i32,
    tag: i32,
    a: i32,
    b: i32,
    c: i32,
    d: i32,
}
#[derive(Clone, Copy)]
struct Failure {
    cause: &'static str,
    kind: i32,
    row: i32,
    col: i32,
    wave: i32,
    time: i32,
}
struct Trial {
    active: bool,
    flags: u64,
    wave_flags: [u32; N],
    first: [i32; N],
    terminal: Option<Failure>,
    ring: [Record; 128],
    cursor: usize,
    records: u32,
    rounds: u64,
    min_sun: u32,
    start: i32,
    max_wave: i32,
    rerolls: u32,
    actions: u32,
}
impl Default for Trial {
    fn default() -> Self {
        Self {
            active: false,
            flags: 0,
            wave_flags: [0; N],
            first: [-1; N],
            terminal: None,
            ring: [Record::default(); 128],
            cursor: 0,
            records: 0,
            rounds: 0,
            min_sun: u32::MAX,
            start: 0,
            max_wave: 0,
            rerolls: 0,
            actions: 0,
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Counts {
    trials: u64,
    failed: u64,
    succeeded: u64,
    wave_occurrences: u64,
}
struct Report {
    chains: u64,
    chain_failed: u64,
    chain_capped: u64,
    roots: Vec<String>,
    reached: Vec<u64>,
    passed: Vec<u64>,
    w20_reached: Vec<u64>,
    attempted: u64,
    success: u64,
    failed: u64,
    incomplete: u64,
    conditions: [Counts; N],
    failure_wave: [u64; 21],
    failure_row: [u64; 6],
    core_loss: u64,
    home: u64,
    timeouts: u64,
}
impl Default for Report {
    fn default() -> Self {
        Self {
            chains: 0,
            chain_failed: 0,
            chain_capped: 0,
            roots: Vec::new(),
            reached: Vec::new(),
            passed: Vec::new(),
            w20_reached: Vec::new(),
            attempted: 0,
            success: 0,
            failed: 0,
            incomplete: 0,
            conditions: [Counts::default(); N],
            failure_wave: [0; 21],
            failure_row: [0; 6],
            core_loss: 0,
            home: 0,
            timeouts: 0,
        }
    }
}
struct Run {
    dynamic_roots: bool,
    root_count: u32,
    chain_limit: u32,
    round_timeout_frames: u32,
    passed: u32,
    chain_frames: u64,
    continuing: bool,
    installed: bool,
    published: bool,
    done: u32,
    quota: u32,
    seed: u32,
    trace_limit: u32,
    trial: Trial,
    report: Report,
    pending_rerolls: u32,
}
impl Default for Run {
    fn default() -> Self {
        Self {
            dynamic_roots: false,
            root_count: 0,
            chain_limit: 1,
            round_timeout_frames: 0,
            passed: 0,
            chain_frames: 0,
            continuing: false,
            installed: false,
            published: false,
            done: 0,
            quota: 0,
            seed: 0,
            trace_limit: 0,
            trial: Trial::default(),
            report: Report::default(),
            pending_rerolls: 0,
        }
    }
}
thread_local! {static RUN:RefCell<Run>=RefCell::new(Run::default());}
// One runner process owns one batch. Workers claim independent roots only
// between complete games; game state and strategy remain worker-local.
static NEXT_ROOT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
// Explicit randomized cohorts belong to the evaluation harness, not the policy.
// Loaded once per process; subsequent root assignments are constant-time reads.
fn root_seed(base: u32, index: u32) -> u32 {
    static SEEDS: std::sync::OnceLock<Option<Vec<u32>>> = std::sync::OnceLock::new();
    let seeds = SEEDS.get_or_init(|| {
        std::env::var("SIX_ROOT_SEEDS_FILE").ok().map(|path| {
            assert!(setting("SIX_DYNAMIC_ROOTS", 0) != 0, "seed files require dynamic roots");
            let text = std::fs::read_to_string(path).expect("read seed file");
            let seeds: Vec<u32> = text
                .split_whitespace()
                .map(|s| s.parse().expect("invalid root seed"))
                .collect();
            assert_eq!(seeds.len(), setting("SIX_TRIALS", 200) as usize, "seed count mismatch");
            let mut unique = seeds.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), seeds.len(), "duplicate root seeds");
            seeds
        })
    });
    seeds
        .as_ref()
        .map_or_else(|| base.wrapping_add(index), |seeds| seeds[index as usize])
}
pub fn active() -> bool {
    RUN.with_borrow(|r| r.trial.active && r.trial.terminal.is_none())
}
/// Cached per worker; the switch changes diagnostics, never trial lifecycle.
pub fn enabled() -> bool {
    thread_local! { static ENABLED: bool = setting("SIX_DIAGNOSTICS", 1) != 0; }
    ENABLED.with(|value| *value)
}
pub fn seed() -> u32 {
    RUN.with_borrow(|r| r.seed)
}
pub fn rerolls(n: u32) {
    RUN.with_borrow_mut(|r| r.pending_rerolls = n);
}
pub fn begin() -> bool {
    RUN.with_borrow_mut(|r| {
        let continuing = std::mem::take(&mut r.continuing);
        if !continuing {
            policy::replay_begin(r.seed, rsvz::timing::clock());
            r.passed = 0;
            r.chain_frames = 0;
            r.report.chains += 1;
        }
        r.trial = Trial::default();
        r.trial.active = true;
        r.trial.rounds = rsvz::session::completed_rounds();
        r.trial.start = rsvz::timing::clock();
        r.trial.rerolls = r.pending_rerolls;
        r.report.attempted += 1;
        let n = r.report.reached.len().max(r.passed as usize + 1);
        r.report.reached.resize(n, 0);
        r.report.passed.resize(n, 0);
        r.report.w20_reached.resize(n, 0);
        r.report.reached[r.passed as usize] += 1;
        rsvz::log(
            rsvz::LogLevel::Info,
            format_args!(
                "six_two_round_begin seed={} round={} clock={} sun={} continuing={}",
                r.seed,
                r.passed + 1,
                r.trial.start,
                rsvz::board::sun(),
                continuing
            ),
        );
        continuing
    })
}
fn record(tag: i32, a: i32, b: i32, c: i32, d: i32) {
    if !enabled() {
        return;
    }
    let (clock, wave, t) = time();
    RUN.with_borrow_mut(|r| {
        let tr = &mut r.trial;
        tr.ring[tr.cursor] = Record {
            clock,
            wave,
            time: t,
            tag,
            a,
            b,
            c,
            d,
        };
        tr.cursor = (tr.cursor + 1) % 128;
        tr.records += 1;
    });
}
pub fn mark(condition: Condition) {
    if !enabled() {
        return;
    }
    let (clock, w, _) = time();
    let i = condition as usize;
    let first_wave = RUN.with_borrow_mut(|r| {
        let tr = &mut r.trial;
        let flag = 1u32 << w.clamp(0, 20);
        let new = tr.wave_flags[i] & flag == 0;
        tr.wave_flags[i] |= flag;
        tr.flags |= 1u64 << i;
        if tr.first[i] < 0 {
            tr.first[i] = clock;
        }
        new
    });
    if first_wave {
        record(3, i as i32, 0, 0, 0);
    }
}
pub fn action(c: CardSelection, p: Pos, reason: i32) {
    let code = match c {
        CardSelection::Plant(k) => k as i32,
        CardSelection::Imitator(_) => 48,
    };
    RUN.with_borrow_mut(|r| r.trial.actions += 1);
    record(1, code, p.0, p.1, reason);
}
pub fn activation(a: &ActivationFact) {
    telemetry::activation(a);
    if !enabled() {
        return;
    }
    if a.kind == ActivationKind::Ice {
        let (alive, frozen) = rsvz::with_frame(|frame| {
            let mut counts = (0, 0);
            for zombie in frame.zombies().filter(|z| z.is_alive()) {
                counts.0 += 1;
                counts.1 += i32::from(zombie.is_frozen());
            }
            counts
        });
        record(6, alive, frozen, 0, 0);
    }
    record(2, a.kind as i32, a.grid.row + 1, a.grid.col + 1, a.target_x);
}
pub fn wave(w: i32, t: i32) {
    RUN.with_borrow_mut(|r| {
        if w == 20 && r.trial.max_wave < 20 {
            r.report.w20_reached[r.passed as usize] += 1;
        }
        r.trial.max_wave = r.trial.max_wave.max(w);
    });
    record(4, w, t, 0, 0);
}
pub fn balance(s: u32) {
    RUN.with_borrow_mut(|r| r.trial.min_sun = r.trial.min_sun.min(s));
    if rsvz::timing::clock() % 500 == 0 {
        telemetry::record("sun", &[s as i64]);
    }
}
pub fn loss(e: &PlantEffectEvent) {
    let source_code = match e.source.as_str() {
        "jack_explosion" => 0,
        "zombie_chew" => 1,
        "gargantuar_smash" => 2,
        "catapult_basket" => 3,
        "zomboni_crush" => 4,
        "catapult_crush" => 5,
        "bungee_steal" => 6,
        _ => 7,
    };
    record(7, source_code, e.raw_kind as i32, e.grid.row + 1, e.grid.col + 1);
    record(5, e.raw_kind as i32, e.grid.row + 1, e.grid.col + 1, e.hp_before);
    match e.effective_kind {
        Pumpkin => mark(Condition::PumpkinLost),
        IceShroom => mark(Condition::IceLost),
        Squash => mark(Condition::SquashLost),
        PuffShroom | Blover => mark(Condition::PadDestroyed),
        _ => {}
    }
    // These are observed actual losses, with no measurement suppression or replacement.
    if matches!(e.effective_kind, GloomShroom | TwinSunflower | UmbrellaLeaf) {
        telemetry::core_loss(e);
        let (_, w, t) = time();
        RUN.with_borrow_mut(|r| {
            r.trial.terminal.get_or_insert(Failure {
                cause: e.source.as_str(),
                kind: e.effective_kind as i32,
                row: e.grid.row + 1,
                col: e.grid.col + 1,
                wave: w,
                time: t,
            });
        });
    }
}
pub fn home(e: &HomeEntryEvent) {
    telemetry::record(
        "home",
        &[
            e.zombie_id.raw() as i64,
            e.zombie_kind as i64,
            (e.row + 1) as i64,
            i64::from(telemetry::carried(e.zombie_id)),
        ],
    );
    let (_, w, t) = time();
    RUN.with_borrow_mut(|r| {
        r.trial.terminal.get_or_insert(Failure {
            cause: "home",
            kind: e.zombie_kind as i32,
            row: e.row + 1,
            col: 0,
            wave: w,
            time: t,
        });
    });
}
fn reset(seed: u32) -> WorldResetConfig {
    WorldResetConfig {
        completed_rounds: 63,
        initial_sun: setting("SIX_SUN", 8000),
        seed,
        ..Default::default()
    }
}
pub fn install() -> RuntimeResult<()> {
    let install = RUN.with_borrow_mut(|r| {
        if r.installed {
            return false;
        }
        r.installed = true;
        r.chain_limit = setting("SIX_CHAIN_ROUNDS", 0);
        r.round_timeout_frames = setting("SIX_ROUND_TIMEOUT_FRAMES", 0);
        let sh = rsvz::session::session_shard();
        let n = setting("SIX_TRIALS", 200);
        r.root_count = n;
        r.dynamic_roots = setting("SIX_DYNAMIC_ROOTS", 0) != 0;
        if r.dynamic_roots {
            let index = NEXT_ROOT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            r.quota = if index < n { u32::MAX } else { 0 };
            r.seed = if index < n {
                root_seed(sh.seed_base, index)
            } else {
                sh.seed_base
            };
        } else {
            r.quota = n / sh.count + u32::from(sh.index < n % sh.count);
            r.seed = sh.seed_base.wrapping_add(sh.index);
        }
        r.trace_limit = setting("SIX_TRACE_SAMPLES", 6);
        true
    });
    if !install {
        return Ok(());
    }
    rsvz::tick::spawn(
        TickOptions::any_dispatch()
            .lifetime(TickLifetime::Session)
            .lane(TickLane::After)
            .idle_neutral(),
        |meta| {
            let mut next = None;
            let mut stop = false;
            RUN.with_borrow_mut(|r| {
                if r.done >= r.quota {
                    stop = true;
                    return;
                }
                if !r.trial.active {
                    return;
                }
                if r.round_timeout_frames > 0
                    && meta.clock.is_some_and(|now| {
                        i64::from(now) - i64::from(r.trial.start) > i64::from(r.round_timeout_frames)
                    })
                    && r.trial.terminal.is_none()
                {
                    let (_, w, t) = time();
                    r.trial.terminal = Some(Failure {
                        cause: "timeout",
                        kind: -1,
                        row: 0,
                        col: 0,
                        wave: w,
                        time: t,
                    });
                    r.trial.flags |= 1u64 << (Condition::Timeout as usize);
                }
                let succeeded =
                    meta.phase == TickPhase::RoundComplete || rsvz::session::completed_rounds() > r.trial.rounds;
                if !succeeded && r.trial.terminal.is_none() {
                    return;
                }
                finish(r, succeeded);
                r.trial.active = false;
                if succeeded && r.trial.terminal.is_none() {
                    r.report.passed[r.passed as usize] += 1;
                    r.passed += 1;
                    if r.chain_limit == 0 || r.passed < r.chain_limit {
                        r.continuing = true;
                        return;
                    }
                    r.report.chain_capped += 1;
                } else if r.trial.terminal.is_some_and(|f| f.cause != "timeout") {
                    r.report.chain_failed += 1;
                }
                let replay_end = if r.trial.terminal.is_some_and(|f| f.cause != "timeout") {
                    1
                } else {
                    2
                };
                if replay_end == 2 {
                    CONTROL.with_borrow_mut(|c| c.replay_endpoint(r.seed, 2));
                }
                policy::replay_end(rsvz::timing::clock(), replay_end);
                record_root(
                    r,
                    if r.trial.terminal.is_some_and(|f| f.cause == "timeout") {
                        "censored"
                    } else if r.trial.terminal.is_some() {
                        "failed"
                    } else {
                        "capped"
                    },
                );
                r.done += 1;
                if r.dynamic_roots {
                    let index = NEXT_ROOT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if index >= r.root_count {
                        stop = true;
                    } else {
                        r.seed = root_seed(rsvz::session::session_shard().seed_base, index);
                        next = Some(reset(r.seed));
                    }
                } else if r.done >= r.quota {
                    stop = true;
                } else {
                    r.seed = r.seed.wrapping_add(rsvz::session::session_shard().count);
                    next = Some(reset(r.seed));
                }
            });
            if stop {
                publish()?;
                rsvz::session::stop_script();
                return Ok(TickControl::Stop);
            }
            if let Some(config) = next {
                rsvz::session::request_world_reset(config)?;
            }
            Ok(TickControl::Continue)
        },
    );
    let seed = RUN.with_borrow(|r| r.seed);
    rsvz::session::request_world_reset(reset(seed))?;
    Ok(())
}
fn finish(r: &mut Run, succeeded: bool) {
    crate::perf::flush();
    // Playing callbacks are paused during the native 500cs closing transition.
    // Read the actual boundary, not the last 5cs observation from before that pause.
    CONTROL.with_borrow_mut(|control| control.sample_boundary());
    for (kind, pos) in [
        (GloomShroom, (2, 1)),
        (GloomShroom, (2, 2)),
        (GloomShroom, (3, 6)),
        (GloomShroom, (4, 6)),
        (GloomShroom, (5, 2)),
        (TwinSunflower, (3, 1)),
        (TwinSunflower, (3, 3)),
        (TwinSunflower, (3, 4)),
        (TwinSunflower, (4, 1)),
        (TwinSunflower, (4, 3)),
        (TwinSunflower, (4, 4)),
        (UmbrellaLeaf, (3, 2)),
        (UmbrellaLeaf, (4, 2)),
    ] {
        if hp(kind, pos).is_none_or(|health| health <= 0) {
            let (_, wave, time) = time();
            r.trial.terminal.get_or_insert(Failure {
                cause: "boundary_missing_core",
                kind: kind as i32,
                row: pos.0,
                col: pos.1,
                wave,
                time,
            });
        }
    }
    telemetry::end_snapshot();
    let (simulations, frames) = telemetry::totals();
    telemetry::record("prediction_work", &[simulations as i64, frames as i64]);
    telemetry::record_work_kinds();
    telemetry::flush();
    r.chain_frames += (rsvz::timing::clock() - r.trial.start).max(0) as u64;
    let timeout = r.trial.terminal.is_some_and(|f| f.cause == "timeout");
    let failed = r.trial.terminal.is_some() && !timeout;
    let success = succeeded && r.trial.terminal.is_none();
    if success {
        policy::replay_reward(r.passed + 1, rsvz::timing::clock());
    }
    r.report.timeouts += u64::from(timeout);
    r.report.incomplete += u64::from(timeout);
    r.report.success += u64::from(success);
    r.report.failed += u64::from(failed);
    for (i, c) in r.report.conditions.iter_mut().enumerate() {
        if r.trial.flags & (1u64 << i) != 0 {
            c.trials += 1;
            c.failed += u64::from(failed);
            c.succeeded += u64::from(success);
            c.wave_occurrences += r.trial.wave_flags[i].count_ones() as u64;
        }
    }
    let f = r.trial.terminal.unwrap_or(Failure {
        cause: "success",
        kind: -1,
        row: 0,
        col: 0,
        wave: 20,
        time: 0,
    });
    if failed {
        r.report.failure_wave[f.wave.clamp(0, 20) as usize] += 1;
        if f.row > 0 {
            r.report.failure_row[(f.row - 1) as usize] += 1;
        }
        match f.cause {
            "home" => r.report.home += 1,
            "timeout" => r.report.timeouts += 1,
            _ => r.report.core_loss += 1,
        }
    }
    rsvz::log(
        rsvz::LogLevel::Info,
        format_args!(
            "six_two_trial seed={} outcome={} wave={} time={} row={} col={} kind={} flags={} min_sun={} end_sun={} actions={} rerolls={} max_wave={} records={} round={}",
            r.seed,
            f.cause,
            f.wave,
            f.time,
            f.row,
            f.col,
            f.kind,
            r.trial.flags,
            r.trial.min_sun,
            rsvz::board::sun(),
            r.trial.actions,
            r.trial.rerolls,
            r.trial.max_wave,
            r.trial.records,
            r.passed + 1
        ),
    );
    rsvz::log(
        rsvz::LogLevel::Info,
        format_args!(
            "six_two_condition_times seed={} terminal_clock={} first={:?}",
            r.seed,
            rsvz::timing::clock(),
            r.trial.first
        ),
    );
    if failed && f.cause != "home" && r.report.failed <= u64::from(r.trace_limit) {
        rsvz::log(
            rsvz::LogLevel::Info,
            format_args!(
                "six_two_terminal_resources seed={} ice={} copy={} doom={} cherry={} jalapeno={} squash={} puff={} blover={} pumpkin={} lily={}",
                r.seed,
                cd(sel(IceShroom)),
                cd(COPY),
                cd(sel(DoomShroom)),
                cd(sel(CherryBomb)),
                cd(sel(Jalapeno)),
                cd(sel(Squash)),
                cd(sel(PuffShroom)),
                cd(sel(Blover)),
                cd(sel(Pumpkin)),
                cd(sel(LilyPad))
            ),
        );
        for pos in [(2, 3), (5, 3), (5, 1), (2, 4), (2, 6), (3, 7), (4, 7), (3, 8), (4, 8)] {
            rsvz::log(
                rsvz::LogLevel::Info,
                format_args!(
                    "six_two_terminal_grid seed={} row={} col={} puff={:?} ice={:?} doom={:?}",
                    r.seed,
                    pos.0,
                    pos.1,
                    rsvz::core::cards::plantability(sel(PuffShroom), grid(pos)),
                    rsvz::core::cards::plantability(sel(IceShroom), grid(pos)),
                    rsvz::core::cards::plantability(sel(DoomShroom), grid(pos))
                ),
            );
        }
        let n = (r.trial.records as usize).min(128);
        let start = (r.trial.cursor + 128 - n) % 128;
        for i in 0..n {
            let x = r.trial.ring[(start + i) % 128];
            rsvz::log(
                rsvz::LogLevel::Info,
                format_args!(
                    "six_two_trace seed={} clock={} wave={} time={} tag={} a={} b={} c={} d={}",
                    r.seed, x.clock, x.wave, x.time, x.tag, x.a, x.b, x.c, x.d
                ),
            );
        }
        rsvz::with_frame(|b| {
            for plant in b.plants().filter(|p| p.is_alive()) {
                rsvz::log(
                    rsvz::LogLevel::Info,
                    format_args!(
                        "six_two_terminal_plant seed={} kind={:?} grid={:?} hp={} state={}",
                        r.seed,
                        plant.raw_kind(),
                        plant.grid(),
                        plant.hp(),
                        plant.state()
                    ),
                );
            }
            for z in b.zombies().filter(|z| z.is_alive() && z.x() < 650.0) {
                rsvz::log(
                    rsvz::LogLevel::Info,
                    format_args!(
                        "six_two_terminal_zombie seed={} kind={:?} row={} wave={} x={:.2} hp={} armor={} phase={:?} frozen={}",
                        r.seed,
                        z.kind(),
                        z.row() + 1,
                        z.from_wave() + 1,
                        z.x(),
                        z.hp(),
                        z.accessory_1_hp(),
                        z.phase(),
                        z.is_frozen()
                    ),
                );
            }
        });
    }
}
// One row per independent root; caps and infrastructure exits are not game deaths.
fn record_root(r: &mut Run, status: &str) {
    audit::finish();
    let edge_stats = prediction::take_puff_edge_stats();
    if edge_stats[0] != 0 {
        rsvz::log(
            rsvz::LogLevel::Info,
            format_args!(
                "six_two_puff_edges seed={} calls={} risky={} reruns={} ns={}",
                r.seed, edge_stats[0], edge_stats[1], edge_stats[2], edge_stats[3]
            ),
        );
    }
    let f = r.trial.terminal;
    let (_, wave, time) = time();
    let row = format!(
        "{{\"seed\":{},\"status\":\"{}\",\"n\":{},\"round\":{},\"cause\":\"{}\",\"wave\":{},\"time\":{},\"row\":{},\"col\":{},\"kind\":{},\"sun\":{},\"frames\":{},\"clock\":{}}}",
        r.seed,
        status,
        r.passed,
        if status == "capped" { r.passed } else { r.passed + 1 },
        f.map_or(
            if status == "capped" {
                "round_cap"
            } else {
                "infrastructure_or_stop"
            },
            |f| f.cause
        ),
        f.map_or(wave, |f| f.wave),
        f.map_or(time, |f| f.time),
        f.map_or(0, |f| f.row),
        f.map_or(0, |f| f.col),
        f.map_or(-1, |f| f.kind),
        rsvz::board::sun(),
        r.chain_frames,
        rsvz::timing::clock()
    );
    rsvz::log(rsvz::LogLevel::Info, format_args!("six_two_root {}", row));
    r.report.roots.push(row);
}
fn merge(a: &mut Report, b: Report) -> RuntimeResult<()> {
    a.chains += b.chains;
    a.chain_failed += b.chain_failed;
    a.chain_capped += b.chain_capped;
    let n = a.reached.len().max(b.reached.len());
    a.reached.resize(n, 0);
    a.passed.resize(n, 0);
    a.w20_reached.resize(n, 0);
    a.roots.extend(b.roots);
    for i in 0..b.reached.len() {
        a.reached[i] += b.reached[i];
        a.passed[i] += b.passed[i];
        a.w20_reached[i] += b.w20_reached[i];
    }
    a.attempted += b.attempted;
    a.success += b.success;
    a.failed += b.failed;
    a.incomplete += b.incomplete;
    a.core_loss += b.core_loss;
    a.home += b.home;
    a.timeouts += b.timeouts;
    for (a, b) in a.conditions.iter_mut().zip(b.conditions) {
        a.trials += b.trials;
        a.failed += b.failed;
        a.succeeded += b.succeeded;
        a.wave_occurrences += b.wave_occurrences;
    }
    for (a, b) in a.failure_wave.iter_mut().zip(b.failure_wave) {
        *a += b;
    }
    for (a, b) in a.failure_row.iter_mut().zip(b.failure_row) {
        *a += b;
    }
    Ok(())
}
fn encode(r: &Report, w: &mut dyn Write) -> io::Result<()> {
    write!(
        w,
        "{{\"kind\":\"six_two_autonomous\",\"attempted\":{},\"success\":{},\"failed\":{},\"incomplete\":{},\"core_loss\":{},\"home\":{},\"timeouts\":{},\"failure_wave\":{:?},\"failure_row\":{:?},\"conditions\":[",
        r.attempted, r.success, r.failed, r.incomplete, r.core_loss, r.home, r.timeouts, r.failure_wave, r.failure_row
    )?;
    for (i, c) in r.conditions.iter().enumerate() {
        if i != 0 {
            write!(w, ",")?;
        }
        write!(
            w,
            "{{\"name\":\"{}\",\"trials\":{},\"subsequent_failed_trials\":{},\"succeeded_trials\":{},\"wave_occurrences\":{}}}",
            NAMES[i], c.trials, c.failed, c.succeeded, c.wave_occurrences
        )?;
    }
    write!(
        w,
        "],\"chains\":{},\"chain_failed\":{},\"chain_capped\":{},\"round_reached\":{:?},\"round_passed\":{:?},\"w20_reached\":{:?},\"roots\":[{}]}}",
        r.chains,
        r.chain_failed,
        r.chain_capped,
        r.reached,
        r.passed,
        r.w20_reached,
        r.roots.join(",")
    )
}
pub fn publish() -> RuntimeResult<()> {
    let report = RUN.with_borrow_mut(|r| {
        if !r.installed || r.published {
            return None;
        }
        r.published = true;
        if r.trial.active {
            r.report.incomplete += 1;
            r.chain_frames += (rsvz::timing::clock() - r.trial.start).max(0) as u64;
            record_root(r, "censored");
        } else if r.continuing {
            r.report.incomplete += 1;
            record_root(r, "censored");
        }
        Some(std::mem::take(&mut r.report))
    });
    if let Some(report) = report {
        rsvz::session::publish_artifact(rsvz::SessionArtifact::new(report, merge, encode))?;
    }
    Ok(())
}
