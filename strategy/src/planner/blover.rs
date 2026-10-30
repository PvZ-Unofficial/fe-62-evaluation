//! A real balloon-clearing use can also hold footballs omitted or missed by Cherry.
use super::*;

fn wave_bit(wave: i32) -> u32 {
    if (1..=20).contains(&wave) { 1 << wave } else { 0 }
}

// Spawn is observed through the current wave, never through the future spawn list.
// The five-frame fallback allows a next wave which actually contains no balloons.
fn next_spawn_observed(wave: i32, time: i32, birth: i32, flying: &[Zombie]) -> bool {
    wave > birth && (time >= 5 || flying.iter().any(|z| z.kind == Z::Balloon && z.wave > birth))
}

impl Planner {
    pub fn cherry_omitted(&mut self, wave: i32) {
        self.omitted_cherry_births |= wave_bit(wave);
        telemetry::record("blover_birth_plan", &[wave as i64]);
    }

    /// Called only after a real Cherry planting succeeds. Use its actual position,
    /// not a provisional future Cherry, and retain the missed actors' birth waves.
    pub fn register_cherry_misses(&mut self, forecast: &Forecast, pos: Pos) {
        let mut plan = Self::without_ash(forecast, prediction::EffectKind::Cherry);
        let event = plan.events.len();
        plan.add(100, prediction::EffectKind::Cherry, pos, true);
        // The successful planting can replace a Puff that is still in this snapshot.
        let replaced = self
            .scene
            .plants
            .iter()
            .position(|p| p.pos == pos && p.kind == PuffShroom)
            .map(|index| (index, self.scene.plants.remove(index)));
        let mut births = 0;
        for z in self
            .scene
            .row_zombies(1)
            .copied()
            .filter(|z| z.kind == Z::Football && !z.carried)
        {
            let outcome = run(z, &self.scene, &plan, None);
            if outcome.ash_mask & (1 << event) == 0 && !outcome.safe() {
                births |= wave_bit(z.wave);
                telemetry::record(
                    "cherry_missed_football",
                    &[z.id.raw() as i64, z.wave as i64, pos.0 as i64, pos.1 as i64],
                );
            }
        }
        if let Some((index, plant)) = replaced {
            self.scene.plants.insert(index, plant);
        }
        self.omitted_cherry_births |= births;
        self.blover_handled_births &= !births;
    }

    fn pending_football(&self) -> Option<Zombie> {
        self.scene
            .row_zombies(1)
            .copied()
            .filter(|z| {
                z.kind == Z::Football
                    && !z.carried
                    && self.omitted_cherry_births & wave_bit(z.wave) != 0
                    && self.blover_handled_births & wave_bit(z.wave) == 0
            })
            .min_by(|a, b| a.x.total_cmp(&b.x))
    }

    pub(super) fn puff_reserved_for_blover(&self, z: Zombie) -> bool {
        z.kind == Z::Football && z.row == 1 && !z.carried && self.blover_reserved_birth == Some(z.wave)
    }

    /// Only a cheap contact query runs between the usual five-frame observations.
    pub fn blover_contact_due(&self, wave: i32, time: i32) -> bool {
        let Some(birth) = self.blover_watch_birth else {
            return false;
        };
        if wave <= birth || time < 1 {
            return false;
        }
        rsvz::with_frame(|board| {
            board.zombies().any(|z| {
                z.is_alive()
                    && z.has_head()
                    && !z.is_frozen()
                    && z.row() == 0
                    && z.kind() == Z::Football
                    && z.from_wave() + 1 == birth
                    && (80..=120).contains(&(z.x() as i32))
            })
        })
    }

    /// Return actual useful lifetime, not a generic Puff's vehicle-crush bound.
    fn blover_lifetime(&self, target: Zombie, f: &Forecast, pos: Pos, horizon: i32) -> i32 {
        let mut pad = Pad::new(pos, Blover);
        pad.lifetime = horizon;
        pad.order = self.scene.next_plant_order;
        let mut lifetime = pad
            .lifetime
            .min(prediction::external_destruction_bound(&self.scene, target, pad));
        if !rsvz::is_safe_blover(grid(pad.pos)).unwrap_or(false) || !combat::jack_safe(pad.pos, lifetime) {
            return 0;
        }
        // The library safety check above already projects hostile projectile damage.
        for giant in self.scene.row_zombies(pos.0).copied().filter(|z| z.giant()) {
            if prediction::could_contact_pad(giant, pad) {
                let out = run_until(giant, &self.scene, f, Some(pad), lifetime + 1, 1);
                lifetime = lifetime.min(out.impact);
            }
        }
        lifetime
    }

    fn blover_wait(&mut self, z: Zombie, reason: i64, wave: i32) {
        if reason == 1 && !prediction::contact(z, z.x, Pad::new((1, 2), Blover).rect()) {
            return;
        }
        let logged = &mut self.blover_wait_logged[z.wave as usize];
        if *logged & (1 << reason) != 0 {
            return;
        }
        *logged |= 1 << reason;
        telemetry::record(
            "blover_wait",
            &[
                z.id.raw() as i64,
                z.wave as i64,
                wave as i64,
                reason,
                cd(sel(Blover)) as i64,
                (z.x * 100.) as i64,
            ],
        );
        if reason == 3 {
            for actor in self.scene.row_zombies(1).filter(|a| a.giant() || a.kind == Z::Catapult) {
                telemetry::record(
                    "blover_blocker",
                    &[
                        z.wave as i64,
                        actor.id.raw() as i64,
                        actor.kind as i64,
                        (actor.x * 100.) as i64,
                        actor.phase as i64,
                        (actor.progress * 100000.) as i64,
                        actor.frozen as i64,
                        actor.hp as i64,
                    ],
                );
            }
        }
    }

    pub fn blover(&mut self, wave: i32, time: i32, f: &Forecast) {
        self.blover_watch_birth = None;
        self.blover_reserved_birth = None;
        if wave < 2 {
            return;
        }
        let balloon_x = self
            .scene
            .zombies
            .iter()
            .filter(|z| z.kind == Z::Balloon && z.phase == ZombiePhase::BalloonFlying)
            .map(|z| z.x)
            .fold(f32::INFINITY, f32::min);
        if !balloon_x.is_finite() {
            return;
        }
        // Worst speed .37, 50cs activation + at most 5cs polling + 25cs margin.
        // This is a fallback, not the normal time at which to spend the card.
        let insurance = balloon_x + 100. <= 0.37 * 80.;
        // Wait for the actual W20 nuke while balloons still have time. Ground
        // losses within one Puff cooldown keep the existing dual-purpose cast.
        // Once Doom has activated it disappears from this forecast, so remaining
        // balloons immediately return to the ordinary clearing path.
        if wave == 20
            && self.pending_football().is_none()
            && !insurance
            && f.events.iter().any(|e| e.kind == prediction::EffectKind::Doom)
            && !self.scene.zombies.iter().copied().any(|z| {
                (z.giant() || matches!(z.kind, Z::Football | Z::Imp))
                    && run_until(z, &self.scene, f, None, 751, 2).danger <= 751
            })
        {
            return;
        }
        let tail = matches!(wave, 9 | 19 | 20);
        let quota = wave >= self.last_blover_wave + 2 || tail || insurance;
        if let Some(target) = self.pending_football() {
            // The birth-group duty competes with an earlier concrete loss in
            // another lane; the same cast still clears the visible balloons.
            if quota && ready(sel(Blover)) {
                let pending = self.assessment(target, f);
                if let Some((pos, z, before, after)) =
                    self.choose_pad_filtered(Blover, f, None, false, false, |planner, &(pos, z, before, after)| {
                        z.row != 1
                            && before.danger < pending.danger
                            && (after.safe() || after.danger > before.danger)
                            && after.impact >= 50
                            && planner.blover_lifetime(z, f, pos, 50) >= 50
                    })
                {
                    self.record_choice(z, before, after, pos, Blover, f);
                    if play(sel(Blover), pos, 8) {
                        self.last_blover_wave = wave;
                        self.recheck_planted(z.id, pos, f);
                        return;
                    }
                }
            }
            if target.x < 80. {
                self.blover_handled_births |= wave_bit(target.wave);
                telemetry::record(
                    "blover_window_missed",
                    &[target.id.raw() as i64, target.wave as i64, (target.x * 100.) as i64],
                );
            } else {
                let spawned = next_spawn_observed(wave, time, target.wave, &self.scene.zombies);
                if !spawned {
                    self.blover_wait(target, 1, wave);
                } else if !quota || !ready(sel(Blover)) {
                    self.blover_wait(target, 2, wave);
                } else if self.scene.main((1, 2)).is_none() && legal(sel(Blover), (1, 2)) {
                    self.blover_watch_birth = Some(target.wave);
                    let mut pad = Pad::new((1, 2), Blover);
                    pad.order = self.scene.next_plant_order;
                    // Reserve this group's first block only while a timely real
                    // Blover is available. Other birth groups can still use Puff.
                    if target.frozen == 0 && prediction::could_contact_pad(target, pad) {
                        pad.lifetime = self.blover_lifetime(target, f, (1, 2), 200);
                        if pad.lifetime >= 50 {
                            self.blover_reserved_birth = Some(target.wave);
                            if prediction::contact(target, target.x, pad.rect()) {
                                let before = self.assessment(target, f);
                                let after = run(target, &self.scene, f, Some(pad));
                                self.record_choice(target, before, after, pad.pos, Blover, f);
                                if play(sel(Blover), pad.pos, 8) {
                                    self.last_blover_wave = wave;
                                    self.blover_handled_births |= wave_bit(target.wave);
                                    self.blover_reserved_birth = None;
                                    self.blover_watch_birth = None;
                                    telemetry::record(
                                        "blover_birth_group",
                                        &[
                                            target.id.raw() as i64,
                                            target.wave as i64,
                                            wave as i64,
                                            (target.x * 100.) as i64,
                                            pad.lifetime as i64,
                                            before.danger as i64,
                                            after.danger as i64,
                                        ],
                                    );
                                    telemetry::record(
                                        "blover_football",
                                        &[target.id.raw() as i64, 2, after.trigger as i64],
                                    );
                                    self.recheck_planted(target.id, pad.pos, f);
                                    for index in 0..self.scene.zombies.len() {
                                        let member = self.scene.zombies[index];
                                        if member.kind == Z::Football && member.row == 1 && member.wave == target.wave {
                                            let remaining = self.assessment(member, f);
                                            telemetry::record(
                                                "blover_group_target",
                                                &[
                                                    member.id.raw() as i64,
                                                    member.wave as i64,
                                                    (member.x * 100.) as i64,
                                                    member.hp as i64,
                                                    member.armor as i64,
                                                    remaining.death as i64,
                                                    remaining.danger as i64,
                                                ],
                                            );
                                        }
                                    }
                                    return;
                                }
                            }
                        } else {
                            self.blover_wait(target, 3, wave);
                        }
                    }
                } else {
                    self.blover_wait(target, 4, wave);
                }
                // Preserve this actual clearing use for the group's contact.
                // No test is made for a future nuke hypothetically killing balloons.
                if !insurance {
                    return;
                }
            }
        }
        if !quota {
            diagnostics::mark(C::BalloonQuota);
            return;
        }
        let old = self
            .scene
            .zombies
            .iter()
            .any(|z| z.kind == Z::Balloon && z.phase == ZombiePhase::BalloonFlying && (z.carried || z.wave < wave));
        if !old && !tail && !insurance {
            return;
        }
        if !ready(sel(Blover)) {
            diagnostics::mark(C::BalloonWaiting);
            return;
        }
        // Ordinary duties, including the user's easier nuke-born footballs.
        if let Some((pos, z, before, after)) =
            self.choose_pad_filtered(Blover, f, None, false, false, |planner, &(pos, z, _, after)| {
                after.impact >= 50 && planner.blover_lifetime(z, f, pos, 50) >= 50
            })
        {
            {
                self.record_choice(z, before, after, pos, Blover, f);
                if play(sel(Blover), pos, 8) {
                    self.last_blover_wave = wave;
                    if insurance {
                        telemetry::record("blover_insurance", &[wave as i64, (balloon_x * 100.) as i64]);
                    }
                    return;
                }
            }
        }
        if time >= 600 || insurance {
            for (index, pos) in [
                (5, 1),
                (2, 3),
                (5, 3),
                (1, 1),
                (6, 1),
                (6, 9),
                (1, 9),
                (2, 9),
                (5, 9),
                (1, 3),
                (6, 3),
                (3, 1),
                (4, 1),
                (3, 2),
                (4, 2),
                (3, 4),
                (4, 4),
                (3, 5),
                (4, 5),
            ]
            .into_iter()
            .enumerate()
            {
                // Do not occupy the ordinary Ice fallback across its planned planting.
                if pos == (5, 1)
                    && f.events.iter().any(|e| {
                        e.kind == prediction::EffectKind::Ice && e.at <= 355 && cd(sel(IceShroom)) + 100 <= e.at
                    })
                {
                    continue;
                }
                if combat::candidate(sel(Blover), pos)
                    && self.blover_lifetime(self.scene.zombies[0], f, pos, 50) >= 50
                    && place(sel(Blover), pos, 8, false)
                {
                    self.last_blover_wave = wave;
                    if insurance {
                        telemetry::record("blover_insurance", &[wave as i64, (balloon_x * 100.) as i64]);
                    }
                    if index >= 5 {
                        telemetry::record(
                            "blover_fallback_grid",
                            &[pos.0 as i64, pos.1 as i64, i64::from(insurance)],
                        );
                    }
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn football(birth: i32, x: f32) -> Zombie {
        Zombie {
            id: ZombieId::from_raw(65536 + birth as u32),
            kind: Z::Football,
            row: 1,
            wave: birth,
            carried: false,
            x,
            hp: 270,
            armor: 1400,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.35,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: rsvz::core::model::ContactRect::new(x as i32, 80, x as i32 + 50, 160),
        }
    }

    #[test]
    fn cherry_misses_register_actual_births_and_reopen_handled_groups() {
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
            .unwrap()
            .install_current()
            .unwrap();
        world.with_backend(|backend| {
            PeBackend::scope_backend(backend, || {
                let mut planner = Planner::default();
                planner.scene = Scene::fixture(vec![football(10, 200.), football(11, 220.), football(12, 680.)]);
                planner.blover_handled_births = wave_bit(10);
                let mut forecast = Forecast::default();
                // A stale planned left Cherry must not hide the real right Cherry's misses.
                forecast.add(100, prediction::EffectKind::Cherry, (1, 4), false);
                planner.register_cherry_misses(&forecast, (1, 9));
                assert_eq!(planner.omitted_cherry_births, wave_bit(10) | wave_bit(11));
                assert_eq!(planner.blover_handled_births & wave_bit(10), 0);
                assert_eq!(planner.pending_football().unwrap().wave, 10);
            })
        });
    }

    #[test]
    fn missed_football_with_certain_followup_clearance_needs_no_blover() {
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
            .unwrap()
            .install_current()
            .unwrap();
        world.with_backend(|backend| {
            PeBackend::scope_backend(backend, || {
                let mut planner = Planner::default();
                planner.scene = Scene::fixture(vec![football(10, 200.)]);
                let mut forecast = Forecast::default();
                forecast.add(200, prediction::EffectKind::Jalapeno, (1, 1), true);
                planner.register_cherry_misses(&forecast, (1, 9));
                assert_eq!(planner.omitted_cherry_births, 0);
            })
        });
    }

    #[test]
    fn birth_group_does_not_blow_before_the_next_spawn() {
        assert!(!next_spawn_observed(18, 3099, 18, &[]));
        assert!(!next_spawn_observed(19, 0, 18, &[]));
        assert!(next_spawn_observed(19, 5, 18, &[]));
        assert_eq!(wave_bit(0), 0);
        assert_eq!(wave_bit(21), 0);
    }
}
