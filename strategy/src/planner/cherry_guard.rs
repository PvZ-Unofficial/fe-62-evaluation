//! Upper-lane damage obligations and the last usable Cherry coverage window.
use super::*;
use rsvz::core::contact::{circle_hits_rect, grid_explosion_shape};
use rsvz::core::model::{ContactRect, GridExplosionKind};

fn recovery_improves(now: &[ZombieId], future: &[ZombieId], no_current: bool, at: i32, deadline: i32) -> bool {
    at < deadline && now.iter().all(|id| future.contains(id)) && (no_current || future.len() > now.len())
}

impl Planner {
    pub fn cherry_site_available(&self, pos: Pos, at: i32) -> bool {
        if !combat::cherry_preserves_ladders(pos)
            || !self.cherry_jack_window(pos, at)
            || main_kind(pos).is_some_and(|k| k != PuffShroom)
        {
            return false;
        }
        let row = (pos.0 - 1) as u32;
        let ice_left = rsvz::board::ice_path_x(row);
        let ice_time = rsvz::board::ice_path_countdown(row) as i32;
        let iced = ice_time > 0 && ice_left <= 750 && pos.1 - 1 >= ((ice_left + 12 - 40) / 80).clamp(0, 8);
        if !iced {
            return combat::candidate(sel(CherryBomb), pos);
        }
        // Only credit an observed expiry, never speculative destruction of a car.
        at > 100
            && ice_time < at - 100
            && !self.scene.row_zombies(pos.0).any(|z| z.kind == Z::Zomboni)
            && rsvz::with_frame(|b| {
                !b.grid_items().any(|g| {
                    g.is_alive() && g.grid() == grid(pos) && g.kind() != rsvz::core::model::GridItemKind::Ladder
                })
            })
    }
    fn cherry_jack_window(&self, pos: Pos, effect: i32) -> bool {
        if effect <= 100 {
            return combat::jack_safe(pos, 100);
        }
        let planting = effect - 100;
        let px = (pos.1 * 80 - 40) as f32;
        // An opened box that explodes before planting cannot destroy the future plant.
        // Do not invent a fuse for unopened boxes; revalidate when planting is due.
        rsvz::with_frame(|b| {
            !b.zombies().any(|z| {
                z.is_alive()
                    && z.kind() == Z::JackInTheBox
                    && (z.row() + 1 - pos.0).abs() <= 1
                    && z.phase() == ZombiePhase::JackInTheBoxPopping
                    && z.phase_counter() >= planting
                    && z.phase_counter() <= effect + 2
                    && z.x() >= px - 135.
                    && z.x() <= px + 130.
            })
        })
    }
    fn cherry_recovery_events(&self, earliest: i32) -> Vec<i32> {
        // Reconsider only visible expiry events, not every intervening frame.
        let mut events = Vec::new();
        rsvz::with_frame(|b| {
            for z in b.zombies() {
                if z.is_alive() && z.kind() == Z::JackInTheBox && z.phase() == ZombiePhase::JackInTheBoxPopping {
                    events.push(z.phase_counter() + 101);
                }
            }
        });
        for row in [0, 1] {
            let remaining = rsvz::board::ice_path_countdown(row) as i32;
            if remaining > 0 {
                events.push(remaining + 101);
            }
        }
        events.sort_unstable();
        events.dedup();
        events
            .into_iter()
            .filter(|&at| at > earliest && at <= prediction::HORIZON)
            .collect()
    }
    /// Compare known obstacle expiries even when an irrelevant right-hand site
    /// is legal now. Hypothetical selection must not mutate the live debt ledger.
    pub fn cherry_recovery_plan(
        &mut self, f: &Forecast, at: i32, flag: bool, current: Option<Pos>,
    ) -> Option<(i32, Pos)> {
        if at > 100 && current.is_some() {
            return None;
        }
        if current.is_some() && self.cherry_guard.debts.is_empty() {
            return None;
        }
        let events = self.cherry_recovery_events(at);
        if events.is_empty() {
            return None;
        }
        let without = Forecast {
            events: f
                .events
                .iter()
                .filter(|e| e.kind != prediction::EffectKind::Cherry || e.certain)
                .copied()
                .collect(),
        };
        let mut obligations = Vec::new();
        let mut deadline = NEVER;
        for &z in self.scene.zombies.iter().filter(|z| {
            matches!(z.row, 1..=3) && (z.giant() || matches!(z.kind, Z::Football | Z::Zomboni | Z::Catapult))
        }) {
            let out = run(z, &self.scene, &without, None);
            deadline = deadline.min(out.danger);
            if self.cherry_guard.required(z.id) && out.death > at {
                obligations.push(z);
            }
        }
        let covered = |pos: Pos, when: i32| -> Vec<ZombieId> {
            let shape = grid_explosion_shape(GridExplosionKind::CherryBomb, grid(pos));
            obligations
                .iter()
                .filter_map(|&z| {
                    let (out, x) = prediction::simulate_projected(z, &self.scene, &without, when);
                    // Already cleared by independent damage no longer needs this Cherry.
                    (out.death <= when
                        || x.is_some_and(|x| {
                            (z.row - pos.0).abs() <= shape.row_range && circle_hits_rect(shape.circle, z.rect_at(x))
                        }))
                    .then_some(z.id)
                })
                .collect()
        };
        let now = current.map_or_else(Vec::new, |pos| covered(pos, at));
        if current.is_some() && now.len() == obligations.len() {
            return None;
        }
        let mut best: Option<(i32, Pos, Vec<ZombieId>, cherry::State)> = None;
        let now_sites = self.cherry_sites(at);
        for later in events {
            if later >= deadline {
                continue;
            }
            let sites = self.cherry_sites(later);
            if !sites.iter().zip(now_sites).any(|(&future, present)| future && !present) {
                continue;
            }
            let mut ledger = self.cherry_guard.clone();
            let Some(pos) = cherry::select(&self.scene, &without, later, sites, flag, &mut ledger) else {
                continue;
            };
            let future = covered(pos, later);
            if !recovery_improves(&now, &future, current.is_none(), later, deadline) {
                continue;
            }
            if best.as_ref().is_none_or(|(_, _, ids, _)| future.len() > ids.len()) {
                best = Some((later, pos, future, ledger));
            }
        }
        let (later, pos, ids, mut ledger) = best?;
        ledger.alternative = None;
        ledger.alternative_targets.clear();
        self.cherry_guard = ledger;
        telemetry::record(
            "cherry_wait_recovery",
            &[
                at as i64,
                later as i64,
                pos.0 as i64,
                pos.1 as i64,
                now.len() as i64,
                ids.len() as i64,
                deadline as i64,
            ],
        );
        Some((later, pos))
    }
    pub(super) fn cherry_sites(&self, at: i32) -> [bool; cherry::CANDIDATES] {
        std::array::from_fn(|i| self.cherry_site_available(cherry::position(i), at))
    }
    pub fn cherry_damage_required(&mut self) -> bool {
        self.cherry_guard.refresh(&self.scene);
        !self.cherry_guard.debts.is_empty()
    }
    pub(super) fn retain_cherry_targets(&mut self, f: &Forecast) -> bool {
        if !self.spare_puff() || !self.cherry_damage_required() {
            return false;
        }
        let Some(event) = f
            .events
            .iter()
            .filter(|e| !e.certain && e.kind == prediction::EffectKind::Cherry && e.at > 100 && e.at <= 2000)
            .min_by_key(|e| e.at)
            .copied()
        else {
            return false;
        };
        // Preserve coverage until either the Cherry or the next usable card.
        // A multi-pad obligation stays live; one pad need not solve the whole horizon.
        let horizon = event.at.min(751 + setting("SIX_DECISION_PERIOD", 20).max(1) as i32 - 1);
        let sites = self.cherry_sites(event.at);
        let shapes: [_; cherry::CANDIDATES] =
            std::array::from_fn(|i| grid_explosion_shape(GridExplosionKind::CherryBomb, grid(cherry::position(i))));
        let covers = |row: i32, rect: ContactRect, excluded: Option<Pos>| {
            (0..cherry::CANDIDATES).any(|i| {
                sites[i]
                    && Some(cherry::position(i)) != excluded
                    && (row - cherry::position(i).0).abs() <= shapes[i].row_range
                    && circle_hits_rect(shapes[i].circle, rect)
            })
        };
        let without = Forecast {
            events: f
                .events
                .iter()
                .copied()
                .filter(|e| e.kind != prediction::EffectKind::Cherry || e.certain)
                .collect(),
        };
        for n in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[n];
            if !self.cherry_guard.required(z.id) || !covers(z.row, z.rect, None) {
                continue;
            }
            let before = run_until(z, &self.scene, &without, None, horizon, 1);
            if before.death <= horizon || covers(z.row, z.rect_at(before.x_at_end), None) {
                continue;
            }
            let feet = foot_columns(z);
            for (i, col) in feet.into_iter().enumerate() {
                if !(1..=9).contains(&col) || i == 1 && col == feet[0] {
                    continue;
                }
                let pos = (z.row, col);
                if self.scene.main(pos).is_some() || !legal(sel(PuffShroom), pos) || !combat::jack_safe(pos, 126) {
                    continue;
                }
                let mut pad = Pad::new(pos, PuffShroom);
                pad.order = self.scene.next_plant_order;
                pad.complete_outcome = true;
                pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, z, pos);
                let after = run_until(z, &self.scene, &without, Some(pad), horizon, 1);
                // Exclude the pad cell: planting the Cherry there would shovel its protection.
                if after.trigger >= pad.lifetime
                    || after.trigger >= horizon
                    || after.danger <= horizon
                    || !(after.death <= horizon || covers(z.row, z.rect_at(after.x_at_end), Some(pos)))
                {
                    continue;
                }
                if self.cherry_pad_conflict(z.id, pad, f) || !self.puff_spend_allowed(pos, f) {
                    continue;
                }
                if play(sel(PuffShroom), pos, 30) {
                    telemetry::record(
                        "cherry_retention_pad",
                        &[z.id.raw() as i64, pos.0 as i64, pos.1 as i64, event.at as i64],
                    );
                    self.recheck_planted(z.id, pos, f);
                    return true;
                }
            }
        }
        false
    }
    // Only a successful retention candidate pays for checking competing windows.
    // Same-row actors get credit for the proposed shared pad.
    fn cherry_pad_conflict(&mut self, recipient: ZombieId, pad: Pad, f: &Forecast) -> bool {
        let until = 751 + setting("SIX_DECISION_PERIOD", 20).max(1) as i32 - 1;
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            if z.id == recipient
                || !(z.giant() || matches!(z.kind, Z::Football | Z::Imp))
                || !matches!(z.row, 1 | 2 | 5 | 6)
            {
                continue;
            }
            let (_, hi) = prediction::attack_offsets(z);
            let edge = pad_columns(z)
                .iter()
                .copied()
                .filter_map(|col| {
                    let pos = (z.row, col);
                    let mut p = Pad::new(pos, PuffShroom);
                    p.offset_x = 4;
                    let edge = (p.rect().left + 20 - hi) as f32 + 1.;
                    (z.x >= edge && self.scene.main(pos).is_none() && legal(sel(PuffShroom), pos)).then_some(edge)
                })
                .reduce(f32::max);
            let Some(edge) = edge else {
                continue;
            };
            if z.x - prediction::max_step(z.kind) * until as f32 >= edge {
                continue;
            }
            let base = self.assessment(z, f);
            if base.safe()
                || z.kind == Z::GigaGargantuar
                    && z.wave == 10
                    && !z.carried
                    && matches!(z.row, 2 | 5)
                    && base.danger > 751
            {
                continue;
            }
            let extra = (z.row == pad.pos.0).then_some(pad);
            let after = run_until(z, &self.scene, f, extra, until, 1);
            if after.death > until && (after.danger <= until || after.x_at_end < edge) {
                return true;
            }
        }
        false
    }
    pub fn cherry_with_squash(&mut self, f: &Forecast) -> Option<Pos> {
        let pos = self.cherry_guard.alternative?;
        if self.cherry_guard.alternative_targets.is_empty()
            || !ready(sel(Squash))
            || rsvz::board::sun() < 200
            || self.reserve_squash5
            || self.deferred_squash5_needed(f)
            || !self.cherry_site_available(pos, 100)
        {
            return None;
        }
        // Flag-wave allocation may not have run required_squash yet. Check the
        // current row-five gap here too, using its existing predicted ash plan.
        if self.scene.row_zombies(5).copied().any(|z| {
            z.kind == Z::GigaGargantuar && z.hp > 3600 && {
                let out = run(z, &self.scene, f, None);
                out.death == NEVER || out.danger < out.death
            }
        }) {
            return None;
        }
        let row = self
            .scene
            .zombies
            .iter()
            .find(|z| z.id == self.cherry_guard.alternative_targets[0])?
            .row;
        if self
            .cherry_guard
            .alternative_targets
            .iter()
            .any(|id| !self.scene.row_zombies(row).any(|z| z.id == *id))
        {
            return None;
        }
        self.required.clear();
        self.required.extend_from_slice(&self.cherry_guard.alternative_targets);
        let mut other = Forecast {
            events: f.events.clone(),
        };
        let event = other
            .events
            .iter_mut()
            .find(|e| !e.certain && e.kind == prediction::EffectKind::Cherry && e.at <= 100)?;
        event.pos = pos;
        event.shape = Some(grid_explosion_shape(GridExplosionKind::CherryBomb, grid(pos)));
        // The existing Squash survival and coverage proof is shared, with just
        // the imminent Cherry cell excluded from this one allocation.
        self.place_squash_except(row, &other, 31, Some(pos)).then_some(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz::core::backend::*;
    use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
    #[test]
    fn recovery_requires_more_coverage_without_losing_existing_rescue_or_deadline() {
        let a = ZombieId::from_raw(65537);
        let b = ZombieId::from_raw(65538);
        assert!(recovery_improves(&[], &[a, b], false, 170, 883));
        assert!(!recovery_improves(&[a], &[b], false, 170, 883));
        assert!(!recovery_improves(&[a], &[a], false, 170, 883));
        assert!(!recovery_improves(&[], &[a, b], false, 170, 160));
        assert!(recovery_improves(&[], &[], true, 170, 883));
    }
    #[test]
    fn popping_jack_recovers_useful_left_cherry_despite_legal_right_site() {
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
            b.select_card(sel(CherryBomb).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            let z = b.place_zombie(Z::JackInTheBox, grid((1, 5))).unwrap();
            b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(375.84).unwrap())
                .unwrap();
        });
        let mut checked = false;
        for tick in 1..=5000 {
            world.with_backend(|b| {
                for z in b.zombies().unwrap() {
                    b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(375.84).unwrap())
                        .unwrap();
                }
            });
            world.update_world().unwrap();
            world.with_backend(|b| {
                if !b.zombies().unwrap().any(|z| {
                    b.zombie_phase(z).ok() == Some(ZombiePhase::JackInTheBoxPopping) && b.zombie_phase_counter(z) < 100
                }) {
                    return;
                }
                PeBackend::scope_backend(b, || {
                    let z = Zombie {
                        age: 0,
                        id: ZombieId::from_raw(65539),
                        kind: Z::GigaGargantuar,
                        row: 2,
                        wave: 10,
                        carried: false,
                        x: 300.,
                        hp: 2040,
                        armor: 0,
                        phase: ZombiePhase::ZombieNormal,
                        progress: 0.,
                        speed: 0.3,
                        frozen: 0,
                        chilled: 628,
                        object: false,
                        eating: false,
                        rect: ContactRect::new(283, 97, 408, 251),
                    };
                    let mut p = Planner::default();
                    p.scene = Scene::fixture(vec![z]);
                    p.scene.now = tick;
                    p.cherry_guard.debts.push(cherry::Debt {
                        id: z.id,
                        hits: 1,
                        planned: true,
                    });
                    let f = Forecast::default();
                    assert!(!p.cherry_site_available((2, 4), 100));
                    assert!(p.cherry_site_available((2, 7), 100));
                    let (later, pos) = p
                        .cherry_recovery_plan(&f, 100, false, Some((2, 7)))
                        .expect("wait for useful left site");
                    assert!(later > 100 && later < 202);
                    assert!(pos.1 <= 6);
                    assert!(p.cherry_guard.required(z.id));
                    assert!(p.cherry_guard.alternative.is_none());
                    checked = true;
                });
            });
            if checked {
                break;
            }
        }
        assert!(checked, "native Jack must open for recovery fixture");
    }
    #[test]
    fn promised_clearance_is_kept_until_another_real_hit_clears_it() {
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
            b.select_card(sel(CherryBomb).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            PeBackend::scope_backend(b, || {
                let z = Zombie {
                    age: 0,
                    id: ZombieId::from_raw(65537),
                    kind: Z::GigaGargantuar,
                    row: 2,
                    wave: 9,
                    carried: false,
                    x: 400.,
                    hp: 2640,
                    armor: 0,
                    phase: ZombiePhase::ZombieNormal,
                    progress: 0.,
                    speed: 0.3,
                    frozen: 0,
                    chilled: 0,
                    object: false,
                    eating: false,
                    rect: ContactRect::new(383, 97, 508, 251),
                };
                let mut scene = Scene::fixture(vec![z]);
                scene.plants.push(prediction::Plant {
                    id: rsvz::core::model::PlantId::from_raw(65537),
                    kind: Pumpkin,
                    pos: (2, 2),
                    hp: 4000,
                    rect: ContactRect::new(100, 165, 220, 245),
                    attack: ContactRect::new(0, 0, 0, 0),
                    shooting: 0,
                    launch: 200,
                    state: 0,
                    phase_left: 0,
                    effect: 0,
                });
                let mut state = cherry::State::default();
                assert!(cherry::select(&scene, &Forecast::default(), 1000, [true; 12], false, &mut state).is_some());
                assert!(state.debts.iter().any(|d| d.id == z.id && d.planned));
                scene.zombies[0].hp = 1000;
                let mut f = Forecast::default();
                f.add(1, prediction::EffectKind::Cherry, (2, 4), true);
                cherry::select(&scene, &f, 1000, [true; 12], false, &mut state);
                assert!(!state.required(z.id));
                scene.zombies[0].hp = 2640;
                scene.plants.push(prediction::Plant {
                    kind: IceShroom,
                    pos: (2, 5),
                    effect: 40,
                    ..scene.plants[0]
                });
                let mut ice = Forecast::default();
                ice.add(40, prediction::EffectKind::Ice, (2, 5), true);
                let (future, _) = prediction::project_row(&scene, &ice, 2, None, 50).unwrap();
                assert!(future.zombies[0].frozen > 0);
                assert!(!future.plants.iter().any(|p| p.kind == IceShroom));
            });
        });
    }
    #[test]
    fn pad_retains_a_left_red_but_does_not_pad_a_frozen_one() {
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
            b.select_card(sel(PuffShroom).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            PeBackend::scope_backend(b, || {
                let mut z = Zombie {
                    age: 0,
                    id: ZombieId::from_raw(65537),
                    kind: Z::GigaGargantuar,
                    row: 1,
                    wave: 7,
                    carried: false,
                    x: 130.,
                    hp: 6000,
                    armor: 0,
                    phase: ZombiePhase::ZombieNormal,
                    progress: 0.,
                    speed: 0.3,
                    frozen: 1000,
                    chilled: 2000,
                    object: false,
                    eating: false,
                    rect: ContactRect::new(113, 12, 238, 166),
                };
                let mut p = Planner::default();
                p.scene = Scene::fixture(vec![z]);
                p.cherry_guard.debts.push(cherry::Debt {
                    id: z.id,
                    hits: 0,
                    planned: false,
                });
                let mut f = Forecast::default();
                f.add(600, prediction::EffectKind::Cherry, (2, 4), false);
                assert!(!p.retain_cherry_targets(&f), "already frozen inside coverage");
                z.frozen = 0;
                p.scene = Scene::fixture(vec![z]);
                p.invalidate();
                let mut competing = z;
                competing.id = ZombieId::from_raw(65538);
                competing.row = 6;
                competing.x = 70.;
                competing.rect = z.rect_at(70.);
                p.scene = Scene::fixture(vec![z, competing]);
                p.invalidate();
                assert!(
                    !p.retain_cherry_targets(&f),
                    "must not spend another lane's last usable pad window"
                );
                p.scene = Scene::fixture(vec![z]);
                p.invalidate();
                assert!(p.retain_cherry_targets(&f), "a real foot pad preserves the left target");
            });
        });
    }
}
