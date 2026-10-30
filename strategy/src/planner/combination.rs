//! Two-step, event-bounded alternatives. No per-pixel or unbounded rollout search.
use super::*;

impl Planner {
    pub(super) fn storage_imp_exposure(&self, pos: Pos, at: i32, target: Zombie, base: Outcome) -> usize {
        let rect = Pad::new(pos, PuffShroom).rect();
        self.scene
            .row_zombies(pos.0)
            .filter(|z| z.giant() && z.object)
            .filter(|z| {
                let out = if z.id == target.id {
                    Some(base)
                } else {
                    self.cache
                        .get(&z.id)
                        .filter(|c| c.serial == telemetry::serial() && self.scene.now - c.at <= 200)
                        .map(|c| {
                            let mut o = c.outcome;
                            o.throw_at -= self.scene.now - c.at;
                            o
                        })
                };
                out.is_some_and(|o| prediction::future_imp_reaches(o, rect, at))
            })
            .count()
    }
    pub(super) fn later_pad_better(&self, z: Zombie, pos: Pos, f: &Forecast, now: Outcome) -> bool {
        // Compare two interventions under the same prediction assumptions. This
        // never rejects padding because the no-intervention deadline looked later.
        let mut waits = Vec::new();
        // A stored row-one Puff can lose HP to basketballs before any useful
        // contact. Compare planting just before the first visible consumer
        // arrives; do not assume an unobserved future attacker or spare HP.
        let basketball_wait = if z.row == 1 && z.kind == Z::Football && self.scene.basketball_active(1) {
            let pad = Pad::new(pos, PuffShroom);
            let mut first = now.trigger;
            for actor in self.scene.row_zombies(1).copied().filter(|a| {
                (a.giant() || matches!(a.kind, Z::Football | Z::Imp)) && prediction::could_contact_pad(*a, pad)
            }) {
                let edge = pad.rect().right - 20 - prediction::attack_offsets(actor).0;
                first = first.min(prediction::first_left_of(
                    actor,
                    &self.scene,
                    f,
                    edge + actor.rect.right - actor.x as i32,
                    751,
                ));
            }
            (first > 25 && first < 751).then_some(first - 20)
        } else {
            None
        };

        if z.frozen > 0 {
            waits.push(z.frozen);
        }
        if z.phase == ZombiePhase::GargantuarSmashing {
            waits.push(
                z.frozen + ((1. - z.progress) / (0.16 / 33. * if z.chilled > 0 { 0.5 } else { 1. })).ceil() as i32 + 1,
            );
        }
        let pad = Pad::new(pos, PuffShroom);
        for &other in self.scene.row_zombies(z.row) {
            if other.giant()
                && other.phase == ZombiePhase::GargantuarSmashing
                && other.progress < 0.64
                && prediction::contact(other, other.x, pad.rect())
            {
                waits.push(
                    other.frozen
                        + ((0.64 - other.progress) / (0.16 / 33. * if other.chilled > 0 { 0.5 } else { 1. })).ceil()
                            as i32
                        + 2,
                );
            }
        }
        waits.retain(|&wait| (1..=300).contains(&wait));
        if let Some(wait) = basketball_wait {
            waits.push(wait);
        }

        waits.sort_unstable();
        waits.dedup();
        let deadline = if waits.is_empty() {
            return false;
        } else {
            self.puff_operation_deadline(z, run(z, &self.scene, f, None), f)
        };
        for wait in waits.into_iter().take(4) {
            if wait >= deadline {
                continue;
            }
            let mut pad = pad;
            pad.plant_at = wait;
            pad.order = self.scene.next_plant_order;
            pad.complete_outcome = true;
            pad.trigger_window = self.tuning.horizon;
            pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, z, pos);
            let later = prediction::simulate(z, &self.scene, f, Some(pad));
            if later.trigger == NEVER || later.trigger >= pad.lifetime || later.danger <= wait {
                continue;
            }
            let cleared = |o: Outcome| o.death < NEVER && o.death <= o.danger;
            let better = (cleared(later) && !cleared(now))
                || cleared(later) == cleared(now) && later.ash_hits >= now.ash_hits && later.danger > now.danger + 20;
            if better {
                telemetry::record(
                    "pad_wait_event",
                    &[z.id.raw() as i64, wait as i64, now.danger as i64, later.danger as i64],
                );
                return true;
            }
        }
        false
    }
    pub(super) fn keep_squash_group_wait(&mut self, f: &Forecast) -> bool {
        let Some((id, until)) = self.squash_group_wait else {
            return false;
        };
        let target = self.scene.zombies.iter().find(|z| z.id == id).copied();
        if self.scene.now >= until || target.is_none() {
            self.squash_group_wait = None;
            return false;
        }
        if self.assessment(target.unwrap(), f).danger <= 182 {
            self.squash_group_wait = None;
            return false;
        }
        self.reserve_squash5 = true;
        true
    }
    pub(super) fn prepare_squash_group(
        &mut self, (id, deadline): (ZombieId, i32), f: &Forecast, covered: usize, current: Option<Pos>,
    ) -> bool {
        if self.scene.now < self.next_combo_check {
            return false;
        }
        self.next_combo_check = self.scene.now + 100;
        let Some(z) = self.scene.zombies.iter().find(|z| z.id == id).copied() else {
            return false;
        };
        let horizon = (deadline - 183).min(900);
        // Include the search boundary: a better overlap can occur between named
        // animation events. A short wait may also expose the next useful event.
        let mut events = vec![100, z.frozen, horizon.min(600), horizon, cd(sel(PuffShroom))];
        for &other in self.scene.row_zombies(z.row) {
            if other.x > z.x + 150. {
                continue;
            }
            if other.hp + other.armor <= 1800 {
                let death = prediction::simulate_until(other, &self.scene, f, None, horizon.max(1)).death;
                if death < horizon {
                    events.push(death + 1);
                }
            }
            if matches!(
                other.phase,
                ZombiePhase::GargantuarSmashing | ZombiePhase::GargantuarThrowing
            ) {
                let rate = if other.phase == ZombiePhase::GargantuarSmashing {
                    0.16 / 33.
                } else {
                    0.24 / 34.
                };
                let end = other.frozen
                    + ((1. - other.progress) / (rate * if other.chilled > 0 { 0.5 } else { 1. })).ceil() as i32
                    + 1;
                events.push(end);
                if other.phase == ZombiePhase::GargantuarSmashing && other.progress < 0.64 {
                    events.push(
                        other.frozen
                            + ((0.64 - other.progress) / (rate * if other.chilled > 0 { 0.5 } else { 1. })).ceil()
                                as i32
                            + 2,
                    );
                }
            }
            if other.kind == Z::Football {
                let col = current.map_or(4, |p| p.1);
                events.push(prediction::first_left_of(other, &self.scene, f, col * 80 - 80, horizon.max(1)) + 1);
            }
        }
        events.retain(|&at| at > 0 && at <= horizon);
        events.sort_unstable();
        events.dedup();
        if events.len() > 6 {
            let last = *events.last().unwrap();
            events.truncate(5);
            events.push(last);
        }
        for wait in events {
            let Some((scene, future)) = prediction::project_row(&self.scene, f, z.row, None, wait) else {
                continue;
            };
            if Self::squash_subset(&scene, &future, &self.required, id).is_some_and(|(_, ids)| ids.len() > covered) {
                self.squash_group_wait = Some((id, self.scene.now + wait));
                telemetry::record("squash_wait_group", &[id.raw() as i64, wait as i64, covered as i64]);
                return true;
            }
        }
        if !self.spare_puff() || self.reserve_real_pad_window(f, id) {
            return false;
        }
        for col in [3, 4] {
            let pos = (z.row, col);
            if self.scene.main(pos).is_some() || !combat::candidate(sel(PuffShroom), pos) {
                continue;
            }
            let mut pad = Pad::new(pos, PuffShroom);
            pad.order = self.scene.next_plant_order;
            pad.complete_outcome = true;
            pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, z, pos);
            let first = prediction::simulate_until(z, &self.scene, f, Some(pad), 400);
            let wait = first.impact + 1;
            if first.trigger == NEVER
                || first.impact <= first.trigger
                || !(1..=400).contains(&wait)
                || wait + 182 >= deadline
                || !combat::jack_safe(pos, wait.min(126))
            {
                continue;
            }
            if prediction::project_row(&self.scene, f, z.row, Some(pad), wait).is_some_and(|(scene, future)| {
                Self::squash_subset(&scene, &future, &self.required, id).is_some_and(|(_, ids)| ids.len() > covered)
            }) && play(sel(PuffShroom), pos, 18)
            {
                self.squash_group_wait = Some((id, self.scene.now + wait));
                telemetry::record(
                    "puff_then_squash",
                    &[id.raw() as i64, col as i64, wait as i64, self.required.len() as i64],
                );
                self.recheck_planted(id, pos, f);
                return true;
            }
        }
        false
    }
    pub(super) fn two_pad_retention_event(
        &self, z: Zombie, first: Pad, f: &Forecast, missed: u32, after: Outcome,
    ) -> Option<usize> {
        // Reuse this candidate's first outcome. Only a concrete second card at
        // cooldown recovery is added, never an unlimited chain of free plants.
        let at = 751.max(after.impact.saturating_add(1));
        if at > 1500 || after.trigger == NEVER {
            return None;
        }
        let (scene, future) = prediction::project_row(&self.scene, f, z.row, Some(first), at)?;
        let target = *scene.zombies.iter().find(|actor| actor.id == z.id)?;
        for (index, event) in f.events.iter().enumerate() {
            if missed & (1 << index) == 0 || event.at <= at + 1 {
                continue;
            }
            let future_index = future
                .events
                .iter()
                .position(|e| e.kind == event.kind && e.pos == event.pos && e.at == event.at - at)?;
            for col in foot_columns(target).into_iter().filter(|&c| c > 0) {
                let pos = (target.row, col);
                // Keep occupied permanent cells and current ice paths out; no
                // speculative shoveling or vehicle removal is credited.
                if scene.main(pos).is_some() || !combat::candidate(sel(PuffShroom), pos) {
                    continue;
                }
                let mut pad = Pad::new(pos, PuffShroom);
                pad.order = scene.next_plant_order;
                pad.complete_outcome = true;
                let next = prediction::simulate_until(target, &scene, &future, Some(pad), event.at - at);
                if next.trigger < NEVER && next.ash_mask & (1 << future_index) != 0 {
                    return Some(index);
                }
            }
        }
        None
    }
    pub(super) fn squash_subset(
        scene: &Scene, f: &Forecast, required: &[ZombieId], anchor: ZombieId,
    ) -> Option<(Pos, Vec<ZombieId>)> {
        let target = scene.zombies.iter().find(|z| z.id == anchor)?;
        let mut predictor = squash::Predictor::default();
        let mut best: Option<(Pos, Vec<ZombieId>)> = None;
        for col in (1..=9).rev() {
            let pos = (target.row, col);
            if !combat::candidate(sel(Squash), pos) || !combat::jack_safe(pos, 182) {
                continue;
            }
            if !squash::possible_landing(scene, pos).is_some_and(|window| squash::could_hit(*target, window)) {
                continue;
            }
            let Some(choice) = predictor.evaluate(scene, f, pos, &[anchor], false) else {
                continue;
            };
            let (_, wave, t) = time();
            let future_time = t + (scene.now - rsvz::timing::clock()).max(0);
            if !predictor.preserves_early_refresh(choice, wave, future_time) {
                continue;
            }
            let ids: Vec<_> = required
                .iter()
                .copied()
                .filter(|&id| id == anchor || predictor.guarantees_hit(choice, id))
                .collect();
            if best.as_ref().is_none_or(|(_, old)| ids.len() > old.len()) {
                best = Some((pos, ids));
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz::core::backend::*;
    use rsvz::core::model::ContactRect;
    use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
    #[test]
    fn row_five_outside_400_retains_demand_when_no_card_can_be_planted() {
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            initial_sun: 100000,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        world.with_backend(|b| {
            b.set_zombie_spawn_stopped(true).unwrap();
            // No ready Squash: admission must still preserve deferred demand.
            b.select_card(sel(CherryBomb).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            PeBackend::scope_backend(b, || {
                let z = Zombie {
                    age: 0,
                    id: ZombieId::from_raw(65537),
                    kind: Z::GigaGargantuar,
                    row: 5,
                    wave: 2,
                    carried: false,
                    x: 410.545,
                    hp: 3100,
                    armor: 0,
                    phase: ZombiePhase::ZombieNormal,
                    progress: 0.31665,
                    speed: 0.3477,
                    frozen: 0,
                    chilled: 659,
                    object: true,
                    eating: false,
                    rect: ContactRect::new(393, 352, 518, 506),
                };
                let mut p = Planner::default();
                p.scene = Scene::fixture(vec![z]);
                p.scene.plants.push(prediction::Plant {
                    id: rsvz::core::model::PlantId::from_raw(65538),
                    kind: GloomShroom,
                    pos: (5, 2),
                    hp: 300,
                    rect: ContactRect::new(130, 352, 190, 432),
                    attack: ContactRect::new(40, 340, 280, 580),
                    shooting: 0,
                    launch: 200,
                    state: 0,
                    phase_left: 0,
                    effect: 0,
                });
                let f = Forecast::default();
                assert!(!p.required_squash(3, 10, &f));
                assert!(p.squash5_deferred);
                assert!(
                    p.deferred_squash5_needed(&f),
                    "row-five threat remains reserved across x=400 gate"
                );
                p.scene.zombies[0].x = 900.;
                p.scene.zombies[0].rect = ContactRect::new(883, 352, 1008, 506);
                assert!(
                    !p.deferred_squash5_needed(&f),
                    "do not reserve against threats beyond card recovery"
                );
            });
        });
    }
    #[test]
    fn frozen_pair_waits_for_overlap_without_spending_another_puff() {
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            initial_sun: 8000,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        world.with_backend(|b| {
            b.set_zombie_spawn_stopped(true).unwrap();
            b.select_card(sel(Squash).checked().unwrap()).unwrap();
            b.select_card(sel(PuffShroom).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            PeBackend::scope_backend(b, || {
                let first = Zombie {
                    age: 0,
                    id: ZombieId::from_raw(65537),
                    kind: Z::GigaGargantuar,
                    row: 5,
                    wave: 3,
                    carried: false,
                    x: 279.5,
                    hp: 1960,
                    armor: 0,
                    phase: ZombiePhase::GargantuarSmashing,
                    progress: 0.358787,
                    speed: 0.301166,
                    frozen: 389,
                    chilled: 1988,
                    object: true,
                    eating: false,
                    rect: ContactRect::new(262, 390, 387, 544),
                };
                let second = Zombie {
                    id: ZombieId::from_raw(65538),
                    x: 378.35,
                    hp: 2800,
                    phase: ZombiePhase::ZombieNormal,
                    progress: 0.304688,
                    speed: 0.317987,
                    rect: ContactRect::new(361, 390, 486, 544),
                    ..first
                };
                let mut p = Planner::default();
                p.scene = Scene::fixture(vec![first, second]);
                p.required = vec![first.id, second.id];
                let f = Forecast::default();
                let now = Planner::squash_subset(&p.scene, &f, &p.required, first.id).unwrap();
                assert_eq!(now.1.len(), 1);
                let prepared = p.prepare_squash_group((first.id, 1287), &f, 1, Some(now.0));
                assert!(prepared);
                assert!(p.squash_group_wait.is_some_and(|(_, t)| (500..=900).contains(&t)));
                assert!(ready(sel(Squash)));
                assert!(p.scene.plants.is_empty());
            });
        });
    }
    #[test]
    fn nearby_pair_is_not_rejected_because_a_distant_third_cannot_be_hit() {
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            initial_sun: 8000,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        world.with_backend(|b| {
            b.set_zombie_spawn_stopped(true).unwrap();
            b.select_card(sel(Squash).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            PeBackend::scope_backend(b, || {
                let actors: Vec<_> = [320., 350., 650.]
                    .into_iter()
                    .enumerate()
                    .map(|(i, x)| Zombie {
                        age: 0,
                        id: ZombieId::from_raw(65537 + i as u32),
                        kind: Z::GigaGargantuar,
                        row: 5,
                        wave: 2,
                        carried: false,
                        x,
                        hp: 2980,
                        armor: 0,
                        phase: ZombiePhase::ZombieNormal,
                        progress: 0.,
                        speed: 0.3,
                        frozen: 0,
                        chilled: 0,
                        object: false,
                        eating: false,
                        rect: ContactRect::new(x as i32 - 17, 0, x as i32 + 108, 154),
                    })
                    .collect();
                let ids: Vec<_> = actors.iter().map(|z| z.id).collect();
                let scene = Scene::fixture(actors);
                let (_, hits) = Planner::squash_subset(&scene, &Forecast::default(), &ids, ids[0]).unwrap();
                assert!(hits.contains(&ids[0]) && hits.contains(&ids[1]));
                assert!(!hits.contains(&ids[2]));
            });
        });
    }
}
