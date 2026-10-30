//! Keep optional W10 padding from consuming a usable interception window.
use super::*;

impl Planner {
    /// A current request can lose a useful contact cell before the next card,
    /// even when the eventual home/core failure is much farther away.
    pub(super) fn last_pad_window_closes(&self, z: Zombie, base: Outcome, f: &Forecast) -> bool {
        self.puff_operation_deadline(z, base, f) <= 751 + setting("SIX_DECISION_PERIOD", 20).max(1) as i32 - 1
    }
    pub(super) fn puff_operation_deadline(&self, z: Zombie, base: Outcome, f: &Forecast) -> i32 {
        if base.safe() {
            return NEVER;
        }
        let Some(col) = pad_columns(z)
            .iter()
            .copied()
            .filter(|&col| {
                let pos = (z.row, col);
                self.scene.main(pos).is_none_or(|p| p.kind == PuffShroom && p.hp < 300)
                    && combat::candidate(sel(PuffShroom), pos)
                    && prediction::could_contact_pad(z, Pad::new(pos, PuffShroom))
            })
            .min()
        else {
            return base.danger;
        };
        let mut pad = Pad::new((z.row, col), PuffShroom);
        pad.offset_x = 4;
        let edge = pad.rect().left + 20 - prediction::attack_offsets(z).1;
        prediction::first_left_of(z, &self.scene, f, edge + z.rect.right - z.x as i32, 771).min(base.danger)
    }
    /// Keep a concrete post-strike interception in competition with current
    /// consumers. The first pad need not solve the full remaining fight.
    pub(super) fn wait_for_old_hammer(&mut self, f: &Forecast, current: Option<(Zombie, Outcome)>) -> bool {
        if current.is_some_and(|(z, _)| self.confirmed_imp(z.id)) {
            return false;
        }
        let ready_again = 751 + setting("SIX_DECISION_PERIOD", 20).max(1) as i32 - 1;
        for index in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[index];
            if !z.giant()
                || !matches!(z.row, 1 | 2 | 5 | 6)
                || z.phase != ZombiePhase::GargantuarSmashing
                || z.progress >= 0.65
                || z.frozen >= ready_again
                || z.x > 400.
            {
                continue;
            }
            let rate = 0.16 / 33. * if z.chilled > 0 { 0.5 } else { 1. };
            let wait = z.frozen + ((0.65 - z.progress) / rate).ceil() as i32 + 1;
            if wait >= ready_again {
                continue;
            }
            let base = self.assessment(z, f);
            if base.safe() || base.danger <= wait {
                continue;
            }
            let closing = self.last_pad_window_closes(z, base, f);
            if let Some((other, outcome)) = current {
                if other.id == z.id
                    || outcome.danger <= wait
                    || !(closing && !self.last_pad_window_closes(other, outcome, f)
                        || closing == self.last_pad_window_closes(other, outcome, f) && base.danger < outcome.danger)
                {
                    continue;
                }
            }
            for &col in pad_columns(z) {
                let pos = (z.row, col);
                let mut pad = Pad::new(pos, PuffShroom);
                if self.scene.main(pos).is_some()
                    || !legal(sel(PuffShroom), pos)
                    || !prediction::contact(z, z.x, pad.rect())
                    || !combat::jack_safe(pos, 1)
                {
                    continue;
                }
                pad.order = self.scene.next_plant_order;
                pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, z, pos);
                if pad.lifetime <= wait {
                    continue;
                }
                let now = run_until(z, &self.scene, f, Some(pad), wait, 1);
                if now.trigger == NEVER || now.impact > now.trigger {
                    continue;
                }
                pad.plant_at = wait;
                pad.trigger_window = ready_again;
                let later = run_until(z, &self.scene, f, Some(pad), ready_again, 1);
                if later.trigger == NEVER || later.impact <= later.trigger || later.death <= later.trigger {
                    continue;
                }
                self.puff_window_hold = Some(self.scene.now);
                telemetry::record(
                    "puff_waiting_request",
                    &[
                        z.id.raw() as i64,
                        z.row as i64,
                        col as i64,
                        wait as i64,
                        base.danger as i64,
                        i64::from(closing),
                    ],
                );
                return true;
            }
        }
        false
    }
    pub(super) fn reserve_real_pad_window(&mut self, f: &Forecast, recipient: ZombieId) -> bool {
        // Include the normal decision-grid delay after the 751cs card cooldown.
        let ready_again = 751 + setting("SIX_DECISION_PERIOD", 20).max(1) as i32 - 1;
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            if z.id == recipient
                || !matches!(z.row, 1 | 2 | 5 | 6)
                || !(z.giant() || matches!(z.kind, Z::Football | Z::Imp))
            {
                continue;
            }
            // Preserve an earlier useful column as well as the final column:
            // retaining col1 does not compensate for losing a needed col2 block.
            let (_, hi) = prediction::attack_offsets(z);
            let edge_at = |col| {
                let mut pad = Pad::new((z.row, col), PuffShroom);
                pad.offset_x = 4;
                (pad.rect().left + 20 - hi) as f32 + 1.
            };
            let Some(col) = pad_columns(z)
                .iter()
                .copied()
                .filter(|&col| {
                    z.x >= edge_at(col)
                        && self.scene.main((z.row, col)).is_none()
                        && legal(sel(PuffShroom), (z.row, col))
                })
                .max()
            else {
                continue;
            };
            let edge = edge_at(col);
            if z.x < edge || z.x - prediction::max_step(z.kind) * ready_again as f32 >= edge {
                continue;
            }
            let base = self.assessment(z, f);
            if base.safe() {
                continue;
            }
            // The other W10 front-control candidates remain optional, too.
            // Do not let peers in that same optional cohort reserve one another's card.
            if z.kind == Z::GigaGargantuar && z.wave == 10 && !z.carried && matches!(z.row, 2 | 5) && base.danger > 751
            {
                continue;
            }
            // One bounded trajectory for a near, independently unsafe actor.
            // Existing plants, freezing, ash and natural damage remain active.
            let later = run_until(z, &self.scene, f, None, ready_again, 1);
            if later.death <= ready_again {
                continue;
            }
            if later.danger <= ready_again || later.x_at_end < edge {
                self.puff_window_hold = Some(self.scene.now);
                telemetry::record(
                    "real_pad_window_reserved",
                    &[
                        z.id.raw() as i64,
                        z.row as i64,
                        z.wave as i64,
                        col as i64,
                        (z.x * 100.) as i64,
                        (later.x_at_end * 100.) as i64,
                        base.danger as i64,
                        ready_again as i64,
                        recipient.raw() as i64,
                    ],
                );
                return true;
            }
        }
        false
    }
}

pub(super) fn wasted_old_hammer(z: Zombie, base: Outcome, after: Outcome) -> bool {
    z.giant()
        && z.phase == ZombiePhase::GargantuarSmashing
        && after.trigger < NEVER
        && after.impact <= after.trigger
        && base.danger == after.danger
        && base.death == after.death
        && base.ash_mask == after.ash_mask
        && after.gloom_damage <= base.gloom_damage
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz::core::backend::*;
    use rsvz::core::model::ContactRect;
    use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};

    #[test]
    fn first_ice_waits_for_old_football_output_band_only() {
        let z = Zombie {
            id: ZombieId::from_raw(65537),
            kind: Z::Football,
            row: 1,
            wave: 1,
            carried: false,
            x: 300.,
            hp: 270,
            armor: 10000,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.68,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(350, 0, 407, 115),
        };
        let mut p = Planner::default();
        p.scene = Scene::fixture(vec![z]);
        for col in [1, 2] {
            p.scene.plants.push(prediction::Plant {
                id: rsvz::core::model::PlantId::from_raw(65536 + col as u32),
                kind: GloomShroom,
                pos: (2, col),
                hp: 300,
                rect: ContactRect::new(0, 100, 60, 180),
                attack: ContactRect::new(col * 80 - 120, 0, col * 80 + 120, 240),
                shooting: 0,
                launch: 200,
                state: 0,
                phase_left: 0,
                effect: 0,
            });
        }
        let f = Forecast::default();
        assert!(p.first_ice_wait(2, sel(IceShroom), None, &f) > 0);
        assert_eq!(p.first_ice_wait(1, sel(IceShroom), None, &f), 0);
        p.scene.zombies[0].x = 90.;
        p.scene.zombies[0].rect = z.rect_at(90.);
        assert_eq!(p.first_ice_wait(2, sel(IceShroom), None, &f), 0);
    }

    #[test]
    fn future_home_risk_reserves_lost_pad_window_but_not_frozen_or_safe_actor() {
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
                let z = Zombie {
                    id: ZombieId::from_raw(65537),
                    kind: Z::GigaGargantuar,
                    row: 1,
                    wave: 9,
                    carried: false,
                    x: 70.,
                    hp: 6000,
                    armor: 0,
                    phase: ZombiePhase::ZombieNormal,
                    progress: 0.,
                    speed: 0.3,
                    frozen: 0,
                    chilled: 2000,
                    object: false,
                    eating: false,
                    rect: ContactRect::new(53, 0, 178, 154),
                };
                let other = ZombieId::from_raw(65538);
                let mut p = Planner::default();
                p.scene = Scene::fixture(vec![z]);
                let f = Forecast::default();
                p.imp_puff_hold = Some(imp::Reservation {
                    row: 6,
                    until: 700,
                    source: other,
                    birth: 9,
                    observed: false,
                    committed: false,
                });
                let demand = Outcome {
                    danger: 3000,
                    ..Outcome::default()
                };
                assert!(p.imp_allows_pad(
                    z,
                    demand,
                    Outcome {
                        trigger: 1,
                        impact: 266,
                        ..demand
                    },
                    &f
                ));
                let remote = Zombie { x: 500., ..z };
                assert!(!p.imp_allows_pad(
                    remote,
                    demand,
                    Outcome {
                        trigger: 1500,
                        ..demand
                    },
                    &f
                ));
                assert!(!p.confirmed_imp(other));
                p.imp_puff_hold.as_mut().unwrap().committed = true;
                // This giant loses its last plant cell before the child's deadline,
                // even though home entry is later. Both compare operation windows.
                assert!(p.imp_allows_pad(z, demand, Outcome { trigger: 1, ..demand }, &f));
                p.imp_puff_hold.as_mut().unwrap().until = 10;
                assert!(!p.imp_allows_pad(z, demand, Outcome { trigger: 1, ..demand }, &f));
                p.imp_puff_hold.as_mut().unwrap().until = 700;
                assert!(p.imp_allows_pad(
                    z,
                    Outcome { danger: 10, ..demand },
                    Outcome { trigger: 1, ..demand },
                    &f
                ));
                p.imp_puff_hold.as_mut().unwrap().observed = true;
                assert!(p.confirmed_imp(other));
                p.imp_puff_hold = None;
                assert!(p.assessment(z, &f).danger > 751);
                let base = p.assessment(z, &f);
                assert!(p.last_pad_window_closes(z, base, &f));
                let hammer = Zombie {
                    phase: ZombiePhase::GargantuarSmashing,
                    ..z
                };
                let spent = Outcome {
                    trigger: 100,
                    impact: 100,
                    ..base
                };
                assert!(wasted_old_hammer(hammer, base, spent));
                assert!(!wasted_old_hammer(
                    hammer,
                    base,
                    Outcome {
                        danger: base.danger + 100,
                        ..spent
                    }
                ));
                assert!(p.reserve_real_pad_window(&f, other));
                assert!(!p.spare_puff()); // Later optional consumers cannot bypass reservation.
                p.scene.revision += 1;
                assert!(
                    !p.spare_puff(),
                    "resampling in the same decision must not release reservation"
                );
                p.scene.now += 1;
                assert!(p.spare_puff());
                assert!(!p.reserve_real_pad_window(&f, z.id)); // The candidate protects this actor itself.
                let waiting = Zombie {
                    phase: ZombiePhase::GargantuarSmashing,
                    progress: 0.48,
                    frozen: 150,
                    ..z
                };
                p.scene = Scene::fixture(vec![waiting]);
                p.invalidate();
                assert!(p.wait_for_old_hammer(&f, None));
                assert!(!p.spare_puff());
                p.puff_window_hold = None;
                let urgent = Zombie { id: other, row: 6, ..z };
                assert!(!p.wait_for_old_hammer(
                    &f,
                    Some((
                        urgent,
                        Outcome {
                            danger: 5,
                            ..Outcome::default()
                        }
                    ))
                ));
                p.scene.now += 1;
                let mut earlier_column = z;
                earlier_column.x = 143.;
                earlier_column.rect = ContactRect::new(126, 0, 251, 154);
                earlier_column.frozen = 82;
                p.scene = Scene::fixture(vec![earlier_column]);
                p.invalidate();
                assert!(
                    p.reserve_real_pad_window(&f, other),
                    "col2 closes even though col1 remains usable"
                );
                let mut optional_peer = z;
                optional_peer.row = 5;
                optional_peer.wave = 10;
                optional_peer.x = 350.;
                optional_peer.rect = ContactRect::new(333, 0, 458, 154);
                p.scene = Scene::fixture(vec![optional_peer]);
                p.invalidate();
                assert!(
                    !p.reserve_real_pad_window(&f, other),
                    "optional W10 peers must not block each other"
                );
                optional_peer.wave = 9;
                p.scene = Scene::fixture(vec![optional_peer]);
                p.invalidate();
                assert!(p.reserve_real_pad_window(&f, other), "old red remains a real demand");
                let mut frozen = z;
                frozen.frozen = 1000;
                p.scene = Scene::fixture(vec![frozen]);
                let base = run(frozen, &p.scene, &f, None);
                assert!(!p.last_pad_window_closes(frozen, base, &f));
                p.invalidate();
                assert!(!p.reserve_real_pad_window(&f, other));
                p.scene = Scene::fixture(vec![z]);
                p.invalidate();
                let mut killed = Forecast::default();
                killed.add(100, prediction::EffectKind::Jalapeno, (1, 1), true);
                let mut low = z;
                low.hp = 1000;
                p.scene = Scene::fixture(vec![low]);
                assert!(!p.reserve_real_pad_window(&killed, other));
            });
        });
    }
}
