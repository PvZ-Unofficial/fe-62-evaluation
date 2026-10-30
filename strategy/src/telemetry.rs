//! Buffered operation and outcome ledger. No per-frame field dump.
use super::*;
use prediction::{Scene, Zombie, contact};
use rsvz::core::model::{PlantEffectEvent, PlantId, ZombieId};
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Entry {
    clock: i32,
    wave: i32,
    time: i32,
    tag: &'static str,
    values: [i64; 16],
    len: usize,
}
struct Watch {
    pos: Pos,
    kind: PlantKind,
    planted: i32,
    trigger: i32,
    contacts: u32,
}
struct History {
    round: u32,
    z: Zombie,
    seen: i32,
    ash_hits: i32,
    squashed: bool,
    mask: u64,
    mutual: i32,
    natural: i32,
}
struct Ledger {
    hit_effects: [Option<HitEffect>; 16],
    round: u32,
    seed: u32,
    entries: Vec<Entry>,
    plants: HashMap<PlantId, Watch>,
    zombies: HashMap<ZombieId, History>,
    ash: bool,
    last_effect: Option<ActivationKind>,
    serial: u32,
    pub simulations: u64,
    pub predicted_frames: u64,
    work_by_kind: [[u64; 2]; 6],
    cache_counts: [u64; 10],
    skipped_frames: u64,
}
impl Default for Ledger {
    fn default() -> Self {
        Self {
            hit_effects: [None; 16],
            round: 0,
            seed: 0,
            entries: Vec::with_capacity(256),
            plants: HashMap::with_capacity(128),
            zombies: HashMap::with_capacity(1024),
            ash: false,
            last_effect: None,
            serial: 0,
            simulations: 0,
            predicted_frames: 0,
            work_by_kind: [[0; 2]; 6],
            cache_counts: [0; 10],
            skipped_frames: 0,
        }
    }
}
#[derive(Clone, Copy)]
struct HitEffect {
    kind: ActivationKind,
    row: i32,
    x: i32,
}
fn damage_account(old: Zombie, new: Zombie, elapsed: i32, effects: &[Option<HitEffect>]) -> (i32, bool) {
    let damage = (old.hp + old.armor - new.hp - new.armor).max(0);
    let relevant = |e: &HitEffect| match e.kind {
        ActivationKind::Cherry => (e.row - new.row).abs() <= 1,
        ActivationKind::Doom => (e.row - new.row).abs() <= 3,
        ActivationKind::Jalapeno | ActivationKind::Squash => e.row == new.row,
        _ => false,
    };
    let events = effects.iter().flatten().filter(|e| relevant(e));
    // Within50cs this formation's natural fire cannot account for1800 HP.
    // Closing intervals can be much longer: cap their count by witnessed ash.
    let hits = if elapsed <= 50 {
        damage / 1800
    } else {
        (damage / 1800).min(events.clone().count() as i32)
    };
    let other = events.clone().filter(|e| e.kind != ActivationKind::Squash).count() as i32;
    let squashed = hits > other
        && events.filter(|e| e.kind == ActivationKind::Squash).any(|e| {
            let margin = if new.kind == Z::Football { 20 } else { 0 };
            let covers = |z: Zombie| e.x + 65 + margin > z.rect.left && e.x + 20 - margin < z.rect.right;
            covers(old) && covers(new)
        });
    (hits, squashed)
}
#[cfg(test)]
mod damage_tests {
    use super::*;
    fn z(row: i32, hp: i32) -> Zombie {
        Zombie {
            id: ZombieId::from_raw(65537),
            kind: Z::GigaGargantuar,
            row,
            wave: 1,
            carried: false,
            x: 300.,
            hp,
            armor: 0,
            phase: rsvz::core::model::ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.37,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: rsvz::core::model::ContactRect::new(283, 0, 408, 154),
        }
    }
    #[test]
    fn multiple_hits_and_natural_fire_are_separated() {
        assert_eq!(damage_account(z(2, 6000), z(2, 2340), 20, &[]), (2, false));
        assert_eq!(damage_account(z(2, 6000), z(2, 5920), 20, &[]), (0, false));
        assert_eq!(damage_account(z(2, 6000), z(2, 4200), 500, &[]), (0, false));
    }
    #[test]
    fn squash_credit_requires_its_row_geometry_and_unexplained_ash_damage() {
        let events = [
            Some(HitEffect {
                kind: ActivationKind::Cherry,
                row: 2,
                x: 280,
            }),
            Some(HitEffect {
                kind: ActivationKind::Squash,
                row: 5,
                x: 280,
            }),
        ];
        assert_eq!(damage_account(z(2, 6000), z(2, 4200), 20, &events), (1, false));
        let squash = [Some(HitEffect {
            kind: ActivationKind::Squash,
            row: 2,
            x: 280,
        })];
        assert_eq!(damage_account(z(2, 6000), z(2, 4200), 20, &squash), (1, true));
        let remote = [Some(HitEffect {
            kind: ActivationKind::Squash,
            row: 2,
            x: 700,
        })];
        assert_eq!(damage_account(z(2, 6000), z(2, 4200), 20, &remote), (1, false));
        let mixed = [events[0], squash[0]];
        assert_eq!(damage_account(z(2, 6000), z(2, 2400), 20, &mixed), (2, true));
    }
}
thread_local! { static LEDGER: RefCell<Ledger> = RefCell::new(Ledger::default()); }
pub fn clear() {
    flush();
    let seed = diagnostics::seed();
    audit::begin(seed);
    LEDGER.with_borrow_mut(|l| {
        l.seed = seed;
        l.round = 0;
        l.plants.clear();
        l.zombies.clear();
        l.ash = false;
        l.last_effect = None;
        l.hit_effects.fill(None);
        l.serial = 0;
        l.simulations = 0;
        l.predicted_frames = 0;
        l.work_by_kind.fill([0; 2]);
        l.cache_counts.fill(0);
        l.skipped_frames = 0;
    });
}
pub fn next_round() {
    audit::next_round();
    flush();
    LEDGER.with_borrow_mut(|l| {
        l.round += 1;
        l.simulations = 0;
        l.predicted_frames = 0;
        l.work_by_kind.fill([0; 2]);
        l.cache_counts.fill(0);
        l.skipped_frames = 0;
        l.serial = l.serial.wrapping_add(1);
        l.ash = false;
        l.last_effect = None;
        l.hit_effects.fill(None);
    });
}
pub fn carried(id: ZombieId) -> bool {
    LEDGER.with_borrow(|l| l.zombies.get(&id).is_some_and(|h| h.round < l.round))
}
fn flush_inner(l: &mut Ledger) {
    if setting("SIX_DETAIL", 1) == 0 {
        l.entries.clear();
        return;
    }
    let seed = l.seed;
    for e in l.entries.drain(..) {
        rsvz::log(
            rsvz::LogLevel::Info,
            format_args!(
                "six_two_detail seed={} clock={} wave={} time={} tag={} values={:?}",
                seed,
                e.clock,
                e.wave,
                e.time,
                e.tag,
                &e.values[..e.len]
            ),
        );
    }
}
fn append(l: &mut Ledger, tag: &'static str, values: &[i64]) {
    if !diagnostics::enabled() {
        return;
    }
    if l.entries.len() == 256 {
        flush_inner(l);
    }
    let (clock, wave, time) = time();
    let mut data = [0; 16];
    data[..values.len()].copy_from_slice(values);
    l.entries.push(Entry {
        clock,
        wave,
        time,
        tag,
        values: data,
        len: values.len(),
    });
}
pub fn record(tag: &'static str, values: &[i64]) {
    if !diagnostics::enabled() {
        return;
    }
    LEDGER.with_borrow_mut(|l| append(l, tag, values));
}
pub fn flush() {
    LEDGER.with_borrow_mut(flush_inner);
}
pub fn serial() -> u32 {
    LEDGER.with_borrow(|l| l.serial)
}
pub fn damage_history(id: ZombieId) -> (i32, bool) {
    LEDGER.with_borrow(|l| l.zombies.get(&id).map_or((0, false), |h| (h.ash_hits, h.squashed)))
}
pub fn work(frames: i32) {
    work_kind(4, frames);
}
pub fn work_kind(kind: usize, frames: i32) {
    record_work(kind, frames, 0);
}
pub fn work_prediction(kind: usize, result: prediction::Outcome) {
    record_work(kind, result.steps - result.skipped, result.skipped);
}
fn record_work(kind: usize, frames: i32, skipped: i32) {
    if !diagnostics::enabled() {
        return;
    }
    LEDGER.with_borrow_mut(|l| {
        l.simulations += 1;
        l.predicted_frames += frames as u64;
        l.skipped_frames += skipped as u64;
        l.work_by_kind[kind][0] += 1;
        l.work_by_kind[kind][1] += frames as u64;
    });
}
pub fn record_work_kinds() {
    LEDGER.with_borrow_mut(|l| {
        for kind in 0..l.work_by_kind.len() {
            let [calls, frames] = l.work_by_kind[kind];
            append(l, "prediction_cost", &[kind as i64, calls as i64, frames as i64]);
        }
        let counts = l.cache_counts.map(|v| v as i64);
        append(l, "event_cache", &counts);
        append(l, "prediction_skip", &[l.skipped_frames as i64]);
    });
}
pub fn cache_count(index: usize) {
    if !diagnostics::enabled() {
        return;
    }
    LEDGER.with_borrow_mut(|l| l.cache_counts[index] += 1);
}
pub fn cache_mismatch(id: ZombieId, age: i32, a: prediction::Outcome, b: prediction::Outcome) {
    LEDGER.with_borrow_mut(|l| {
        if l.cache_counts[5] <= 20 {
            append(
                l,
                "event_cache_mismatch",
                &[
                    id.raw() as i64,
                    age as i64,
                    a.death as i64,
                    b.death as i64,
                    a.danger as i64,
                    b.danger as i64,
                    a.hp_at_end as i64,
                    b.hp_at_end as i64,
                    a.gloom_damage as i64,
                    b.gloom_damage as i64,
                    a.x5.to_bits() as i64,
                    b.x5.to_bits() as i64,
                    a.x50.to_bits() as i64,
                    b.x50.to_bits() as i64,
                ],
            );
        }
    });
}
pub fn totals() -> (u64, u64) {
    LEDGER.with_borrow(|l| (l.simulations, l.predicted_frames))
}
/// The caller refreshes the actual boundary, including the closing interval without Playing ticks.
pub fn end_snapshot() {
    if !diagnostics::enabled() {
        return;
    }
    LEDGER.with_borrow_mut(|l| {
        let zombies = std::mem::take(&mut l.zombies);
        for (id, h) in &zombies {
            let z = h.z;
            if !z.giant() {
                continue;
            }
            let budget = match z.row {
                1 | 2 | 5 | 6 => 2400,
                _ => 0,
            };
            append(
                l,
                "end_giant",
                &[
                    id.raw() as i64,
                    z.kind as i64,
                    z.row as i64,
                    z.wave as i64,
                    (z.x * 100.) as i64,
                    z.hp as i64,
                    h.ash_hits as i64,
                    i64::from(h.squashed),
                    h.seen as i64,
                    budget as i64,
                    i64::from(z.carried),
                ],
            );
        }
        l.zombies = zombies;
    });
}
pub fn planted(id: PlantId, card: CardSelection, pos: Pos, reason: i32) {
    let kind = match card {
        CardSelection::Plant(k) => k,
        CardSelection::Imitator(_) => Imitator,
    };
    LEDGER.with_borrow_mut(|l| {
        l.serial += 1;
        if !diagnostics::enabled() {
            return;
        }
        append(
            l,
            "plant",
            &[
                id.raw() as i64,
                kind as i64,
                pos.0 as i64,
                pos.1 as i64,
                reason as i64,
                rsvz::board::sun() as i64,
            ],
        );
        if matches!(kind, PuffShroom | Blover | IceShroom | Imitator | Jalapeno) {
            if let Some(rect) = rsvz::with_frame(|frame| frame.plant(id).map(|p| p.hit_box())) {
                append(
                    l,
                    "plant_geometry",
                    &[
                        id.raw() as i64,
                        rect.left as i64,
                        rect.right as i64,
                        rect.top as i64,
                        rect.bottom as i64,
                    ],
                );
            }
            l.plants.insert(
                id,
                Watch {
                    pos,
                    kind,
                    planted: rsvz::timing::clock(),
                    trigger: -1,
                    contacts: 0,
                },
            );
        }
    });
}
pub fn activation(a: &rsvz::core::model::ActivationFact) {
    LEDGER.with_borrow_mut(|l| {
        l.serial += 1;
        l.ash = true;
        l.last_effect = Some(a.kind);
        if matches!(
            a.kind,
            ActivationKind::Cherry | ActivationKind::Doom | ActivationKind::Jalapeno | ActivationKind::Squash
        ) {
            if let Some(slot) = l.hit_effects.iter_mut().find(|e| e.is_none()) {
                *slot = Some(HitEffect {
                    kind: a.kind,
                    row: a.grid.row + 1,
                    x: a.position.x,
                });
            }
        }
        append(
            l,
            "effect",
            &[
                a.kind as i64,
                (a.grid.row + 1) as i64,
                (a.grid.col + 1) as i64,
                a.target_x as i64,
                rsvz::board::sun() as i64,
            ],
        );
    });
}
pub fn plant_effect(e: &PlantEffectEvent) {
    if !matches!(
        e.outcome,
        PlantEffectOutcome::Killed
            | PlantEffectOutcome::Squished
            | PlantEffectOutcome::Stolen
            | PlantEffectOutcome::Activated
    ) {
        return;
    }
    if !diagnostics::enabled() {
        LEDGER.with_borrow_mut(|l| l.serial += 1);
        return;
    }
    let outcome = match e.outcome {
        PlantEffectOutcome::Killed => 1,
        PlantEffectOutcome::Squished => 2,
        PlantEffectOutcome::Stolen => 3,
        PlantEffectOutcome::Activated => 4,
        _ => 0,
    };
    LEDGER.with_borrow_mut(|l| {
        if let Some(p) = l.plants.remove(&e.plant_id) {
            if matches!(e.source.as_str(), "zombie_chew" | "gargantuar_smash") {
                rsvz::with_frame(|frame| {
                    for z in frame.zombies().filter(|z| z.is_alive() && z.row() == e.grid.row) {
                        let relevant = if e.source.as_str() == "gargantuar_smash" {
                            z.phase() == ZombiePhase::GargantuarSmashing
                                && z.animation_progress().is_some_and(|p| (0.63..=0.67).contains(&p))
                        } else {
                            z.is_eating()
                        };
                        if !relevant {
                            continue;
                        }
                        if let Ok(Some(bounds)) = rsvz::core::contact::zombie_attack_bounds(z.id()) {
                            let left = p.pos.1 * 80 - 40 + 5;
                            let right = p.pos.1 * 80 - 40 + 74;
                            if bounds.range.right < left || bounds.range.left > right {
                                continue;
                            }
                            append(
                                l,
                                "pad_consumer_candidate",
                                &[
                                    e.plant_id.raw() as i64,
                                    z.id().raw() as i64,
                                    z.kind() as i64,
                                    (z.row() + 1) as i64,
                                    (z.from_wave() + 1) as i64,
                                    (z.x() * 100.) as i64,
                                    z.hp() as i64,
                                    i64::from(z.is_eating()),
                                ],
                            );
                        }
                    }
                });
            }
            append(
                l,
                "pad_consumed",
                &[
                    e.plant_id.raw() as i64,
                    p.kind as i64,
                    p.pos.0 as i64,
                    p.pos.1 as i64,
                    p.planted as i64,
                    p.trigger as i64,
                    p.contacts as i64,
                    e.hp_before as i64,
                    outcome,
                ],
            );
        }
        append(
            l,
            e.source.as_str(),
            &[
                e.plant_id.raw() as i64,
                e.effective_kind as i64,
                (e.grid.row + 1) as i64,
                (e.grid.col + 1) as i64,
                e.hp_before as i64,
                outcome,
                match e.source {
                    rsvz::core::model::PlantEffectSource::Basketball(id) => id.raw() as i64,
                    rsvz::core::model::PlantEffectSource::Jack(id)
                    | rsvz::core::model::PlantEffectSource::Bite(id)
                    | rsvz::core::model::PlantEffectSource::Gargantuar(id)
                    | rsvz::core::model::PlantEffectSource::ZomboniCrush(id)
                    | rsvz::core::model::PlantEffectSource::CatapultCrush(id)
                    | rsvz::core::model::PlantEffectSource::Bungee(id) => id.raw() as i64,
                },
            ],
        );
        l.serial += 1;
    });
}
pub fn observe(scene: &Scene) {
    if !diagnostics::enabled() {
        LEDGER.with_borrow_mut(|l| {
            for z in scene
                .zombies
                .iter()
                .filter(|z| z.giant() || matches!(z.kind, Z::Football | Z::Imp | Z::Balloon | Z::Zomboni | Z::Catapult))
            {
                // These fields feed strategy decisions even when diagnostics are off.
                let witnessed = l.hit_effects;
                let history = l.zombies.entry(z.id).or_insert(History {
                    round: l.round,
                    z: *z,
                    seen: scene.now,
                    ash_hits: 0,
                    squashed: false,
                    mask: 0,
                    mutual: 0,
                    natural: 0,
                });
                let (hits, squashed) = damage_account(history.z, *z, scene.now - history.seen, &witnessed);
                history.ash_hits += hits;
                history.squashed |= squashed;
                history.z = *z;
                history.seen = scene.now;
            }
            l.zombies.retain(|_, h| h.seen == scene.now);
            l.ash = false;
            l.hit_effects.fill(None);
        });
        return;
    }
    LEDGER.with_borrow_mut(|l| {
        for z in scene
            .zombies
            .iter()
            .filter(|z| z.giant() || matches!(z.kind, Z::Football | Z::Imp | Z::Balloon | Z::Zomboni | Z::Catapult))
        {
            let born_round = l.zombies.get(&z.id).map_or(l.round, |h| h.round);
            let mut mask = 0u64;
            for p in &scene.plants {
                if p.kind == GloomShroom
                    && (p.pos.0 - z.row).abs() <= 1
                    && p.attack.right.min(z.rect.right) - p.attack.left.max(z.rect.left) > 0
                {
                    mask |= 1u64 << ((p.pos.0 - 1) * 9 + p.pos.1 - 1);
                }
            }
            let previous = l
                .zombies
                .get(&z.id)
                .map(|h| (h.z, h.ash_hits, h.squashed, h.mask, h.mutual, h.natural, h.seen));
            let mut hits = previous.map_or(0, |(_, hits, _, _, _, _, _)| hits);
            let mut squashed = previous.is_some_and(|(_, _, s, _, _, _, _)| s);
            let mutual = previous.map_or(0, |(_, _, _, old_mask, spent, _, seen)| {
                spent
                    + if old_mask.count_ones() >= 2 {
                        scene.now - seen
                    } else {
                        0
                    }
            });
            let mut natural = previous.map_or(0, |(_, _, _, _, _, damage, _)| damage);
            if let Some((old, _, _, old_mask, _, _, _)) = previous {
                let delta = old.hp + old.armor - z.hp - z.armor;
                let elapsed = scene.now - previous.unwrap().6;
                let (added, squash) = damage_account(old, *z, elapsed, &l.hit_effects);
                natural += (delta - added * 1800).max(0);
                hits += added;
                squashed |= squash;
                if old.phase != z.phase || old.frozen == 0 && z.frozen > 0 || old.frozen > 0 && z.frozen == 0 {
                    append(
                        l,
                        "zombie_state",
                        &[
                            z.id.raw() as i64,
                            z.kind as i64,
                            z.row as i64,
                            z.wave as i64,
                            (z.x * 100.) as i64,
                            z.hp as i64,
                            z.phase as i64,
                            (z.progress * 100000.) as i64,
                            z.frozen as i64,
                            z.chilled as i64,
                        ],
                    );
                }
                if (z.phase == ZombiePhase::GargantuarSmashing && old.phase != z.phase)
                    || matches!(z.kind, Z::Football | Z::Imp) && z.eating && !old.eating
                {
                    let target = scene
                        .plants
                        .iter()
                        .filter(|p| p.pos.0 == z.row && contact(*z, z.x, p.rect))
                        .filter(|p| {
                            p.kind == Pumpkin || !scene.plants.iter().any(|q| q.pos == p.pos && q.kind == Pumpkin)
                        })
                        .min_by_key(|p| p.id.index());
                    if let Some(p) = target {
                        if let Some(watch) = l.plants.get_mut(&p.id) {
                            if watch.trigger < 0 {
                                watch.trigger = scene.now;
                            }
                            watch.contacts += 1;
                            append(
                                l,
                                "pad_trigger",
                                &[
                                    p.id.raw() as i64,
                                    z.id.raw() as i64,
                                    z.row as i64,
                                    (z.x * 100.) as i64,
                                    z.hp as i64,
                                    p.pos.1 as i64,
                                ],
                            );
                        }
                    }
                }
                if z.giant() && (mask != old_mask || z.phase == ZombiePhase::GargantuarSmashing && old.phase != z.phase)
                {
                    append(
                        l,
                        "gloom_residence",
                        &[
                            z.id.raw() as i64,
                            z.row as i64,
                            z.wave as i64,
                            (z.x * 100.) as i64,
                            z.hp as i64,
                            mask as i64,
                            mutual as i64,
                            natural as i64,
                            hits as i64,
                        ],
                    );
                }
                if delta >= 1700 || l.ash {
                    append(
                        l,
                        "ash_health",
                        &[
                            z.id.raw() as i64,
                            z.row as i64,
                            z.wave as i64,
                            (z.x * 100.) as i64,
                            z.hp as i64,
                            z.armor as i64,
                            delta as i64,
                            hits as i64,
                        ],
                    );
                }
            } else {
                append(
                    l,
                    "zombie_seen",
                    &[
                        z.id.raw() as i64,
                        z.kind as i64,
                        z.row as i64,
                        z.wave as i64,
                        (z.x * 100.) as i64,
                        z.hp as i64,
                        z.armor as i64,
                    ],
                );
            }
            l.zombies.insert(
                z.id,
                History {
                    round: born_round,
                    z: *z,
                    seen: scene.now,
                    ash_hits: hits,
                    squashed,
                    mask,
                    mutual,
                    natural,
                },
            );
        }
        // Move only the small map handles out so the logger can be borrowed during retention.
        let mut zombies = std::mem::take(&mut l.zombies);
        zombies.retain(|id, h| {
            if h.seen == scene.now {
                true
            } else {
                append(
                    l,
                    "zombie_gone",
                    &[
                        id.raw() as i64,
                        h.z.kind as i64,
                        h.z.row as i64,
                        h.z.wave as i64,
                        (h.z.x * 100.) as i64,
                        h.z.hp as i64,
                        h.ash_hits as i64,
                    ],
                );
                append(
                    l,
                    "final_residence",
                    &[
                        id.raw() as i64,
                        h.z.row as i64,
                        h.mutual as i64,
                        h.natural as i64,
                        h.ash_hits as i64,
                    ],
                );
                false
            }
        });
        l.zombies = zombies;
        let mut plants = std::mem::take(&mut l.plants);
        plants.retain(|id, p| {
            if scene.plants.iter().any(|plant| plant.id == *id) {
                true
            } else {
                append(
                    l,
                    "pad_gone",
                    &[
                        id.raw() as i64,
                        p.kind as i64,
                        p.pos.0 as i64,
                        p.pos.1 as i64,
                        p.planted as i64,
                        p.trigger as i64,
                        p.contacts as i64,
                    ],
                );
                false
            }
        });
        l.plants = plants;
        l.ash = false;
        l.hit_effects.fill(None);
    });
}

pub fn core_loss(e: &PlantEffectEvent) {
    if !diagnostics::enabled() {
        return;
    }
    if e.source.as_str() != "gargantuar_smash" {
        return;
    }
    LEDGER.with_borrow_mut(|l| {
        rsvz::with_frame(|frame| {
            for z in frame.zombies().filter(|z| {
                z.is_alive()
                    && z.row() == e.grid.row
                    && matches!(z.kind(), Z::Gargantuar | Z::GigaGargantuar)
                    && z.phase() == ZombiePhase::GargantuarSmashing
            }) {
                let progress = z.animation_progress().unwrap_or(-1.);
                if !(0.63..=0.67).contains(&progress) {
                    continue;
                }
                append(
                    l,
                    "core_smash_candidate",
                    &[
                        e.plant_id.raw() as i64,
                        z.id().raw() as i64,
                        (z.row() + 1) as i64,
                        (z.from_wave() + 1) as i64,
                        (z.x() * 100.) as i64,
                        z.hp() as i64,
                        (progress * 100000.) as i64,
                    ],
                );
                if let Some(h) = l.zombies.get(&z.id()) {
                    let values = [
                        z.id().raw() as i64,
                        h.mutual as i64,
                        h.natural as i64,
                        h.ash_hits as i64,
                        h.mask as i64,
                    ];
                    append(l, "core_smash_history", &values);
                }
            }
        });
        append(
            l,
            "core_smash_cards",
            &[
                cd(sel(PuffShroom)) as i64,
                cd(sel(IceShroom)) as i64,
                cd(COPY) as i64,
                cd(sel(Squash)) as i64,
                cd(sel(DoomShroom)) as i64,
                cd(sel(CherryBomb)) as i64,
                cd(sel(Jalapeno)) as i64,
            ],
        );
    });
}
