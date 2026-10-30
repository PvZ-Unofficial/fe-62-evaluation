use super::*;
#[derive(Clone, Copy)]
pub(super) struct Reservation {
    pub row: i32,
    pub until: i32,
    pub source: ZombieId,
    pub birth: i32,
    pub observed: bool,
    pub committed: bool,
}

// Native non-random throw branch: for x in (400,500], d2y=x-360.
// The Imp starts at x-133 with y=88 and moves 3px each flight update.
// anim_land has six frames at 24fps; no future zombie RNG/slot is read.
fn near_landing(x: f32) -> Option<(f32, i32)> {
    if !(400.0..=500.0).contains(&x) {
        return None;
    }
    let distance = (x - 360.).max(40.);
    let mut velocity = ((distance as f64 / 3.) as f32 as f64 * 0.5 * 0.05000000074505806) as f32;
    let mut height = 88.0f32;
    let mut frames = 0;
    while height > 0. && frames < 200 {
        velocity -= 0.05;
        height += velocity;
        frames += 1;
    }
    Some((x - 133. - 3. * frames as f32, frames + 25))
}

use prediction::imp_landing::observed_landing;

impl Planner {
    pub(super) fn spare_puff(&self) -> bool {
        ready(sel(PuffShroom))
            && self.puff_window_hold != Some(self.scene.now)
            && !self.imp_puff_hold.is_some_and(|r| self.scene.now < r.until)
    }
    pub(super) fn confirmed_imp(&self, id: ZombieId) -> bool {
        self.imp_puff_hold
            .is_some_and(|r| r.observed && r.source == id && self.scene.now < r.until)
    }
    pub(super) fn imp_allows_pad(&self, z: Zombie, base: Outcome, after: Outcome, f: &Forecast) -> bool {
        let Some(r) = self.imp_puff_hold.filter(|r| self.scene.now < r.until) else {
            return true;
        };
        // A projected child can reserve optional stock, not erase an actual
        // useful interception. Landed threatened children compete by identity.
        if r.committed && !r.observed {
            return z.kind == Z::Imp && z.row == r.row
                || self.puff_operation_deadline(z, base, f) < r.until - self.scene.now;
        }
        r.observed
            || z.kind == Z::Imp
            || after.trigger <= setting("SIX_DECISION_PERIOD", 20).max(1) as i32
            || self.last_pad_window_closes(z, base, f)
    }

    /// Avoid spending the only short-CD blocker immediately before a visible
    /// near-left throw (including a throw caused by already scheduled ash).
    /// The normal landed-Imp predictor still decides whether/where to plant.
    pub fn refresh_imp_reservation(&mut self, f: &Forecast) {
        if !self.imp_padding || !ready(sel(PuffShroom)) || self.scene.now < self.next_imp_reserve {
            return;
        }
        self.next_imp_reserve = self.scene.now + 25;
        let old = self.imp_puff_hold;
        self.imp_puff_hold = old.filter(|r| {
            !r.observed && self.scene.now < r.until
                && self.scene.row_zombies(r.row).any(|z| {
                    z.kind == Z::Imp && z.wave == r.birth && matches!(z.phase, ZombiePhase::ImpGettingThrown | ZombiePhase::ImpLanding)
                        // Once a visible throw is underway, keep the short
                        // reservation until the real child can be assessed.
                        // A moving next-wave ice estimate must not spend it
                        // in the gap between starting the throw and spawning.
                        || z.id == r.source && z.giant() && z.object && z.phase == ZombiePhase::GargantuarThrowing
                })
        }).map(|mut r| { r.committed = true; r });
        for z in self.scene.zombies.iter().copied() {
            if !matches!(z.row, 1 | 6) || !legal(sel(PuffShroom), (z.row, 1)) {
                continue;
            }
            let airborne =
                z.kind == Z::Imp && matches!(z.phase, ZombiePhase::ImpGettingThrown | ZombiePhase::ImpLanding);
            let out = if airborne {
                let Some((child, arrival)) = observed_landing(z) else {
                    continue;
                };
                prediction::simulate_imp_arrival(child, &self.scene, f, 0, arrival)
            } else if z.kind == Z::Imp && z.phase == ZombiePhase::ZombieNormal {
                if z.x < 4. {
                    continue;
                }
                run(z, &self.scene, f, None)
            } else if z.giant() && z.object && z.x - prediction::max_step(z.kind) * 751. <= 500. {
                let parent = run_until(z, &self.scene, f, None, 751, 2);
                if parent.throw_at >= 751 {
                    continue;
                }
                let Some((x, delay)) = near_landing(parent.throw_x) else {
                    continue;
                };
                let arrival = parent.throw_at + delay;
                let imp = Zombie {
                    kind: Z::Imp,
                    x,
                    hp: 270,
                    armor: 0,
                    phase: ZombiePhase::ZombieNormal,
                    progress: 0.,
                    speed: prediction::max_speed(Z::Imp),
                    frozen: 0,
                    // simulate_arrival advances this countdown from now, so
                    // at landing it equals the inherited throw value - flight.
                    chilled: parent.throw_chilled + parent.throw_at,
                    object: false,
                    eating: false,
                    rect: rsvz::core::model::ContactRect::new(
                        x as i32 + 36,
                        z.rect.top + 38,
                        x as i32 + 78,
                        z.rect.top + 153,
                    ),
                    ..z
                };
                prediction::simulate_imp_arrival(imp, &self.scene, f, parent.throw_at, arrival)
            } else {
                continue;
            };
            if !out.safe() && out.puff_deadline > 0 && out.puff_deadline < 751 {
                let until = self.scene.now + out.puff_deadline;
                let observed = z.kind == Z::Imp && z.phase == ZombiePhase::ZombieNormal;
                let committed = observed || airborne || z.phase == ZombiePhase::GargantuarThrowing;
                if self.imp_puff_hold.is_none_or(|r| {
                    (observed, committed) > (r.observed, r.committed)
                        || (observed, committed) == (r.observed, r.committed) && until < r.until
                }) {
                    self.imp_puff_hold = Some(Reservation {
                        row: z.row,
                        until,
                        source: z.id,
                        birth: z.wave,
                        observed,
                        committed,
                    });
                    if old.is_none() {
                        telemetry::record(
                            "imp_puff_reserve",
                            &[z.id.raw() as i64, z.row as i64, until as i64, out.danger as i64],
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_airborne_child_creates_reservation_without_parent_history() {
        use rsvz::core::backend::*;
        use rsvz::core::model::{I32RepresentableF32, PositiveHp};
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        for chilled in [false, true] {
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
                let z = b.place_zombie(Z::GigaGargantuar, grid((6, 6))).unwrap();
                b.set_zombie_x(z, I32RepresentableF32::new(401.02).unwrap()).unwrap();
                b.set_zombie_body_hp(z, PositiveHp::new(2900).unwrap()).unwrap();
            });
            let mut expected = None;
            let mut landing_expected = None;
            let mut ice_used = false;
            let mut actual = None;
            for frame in 1..=450 {
                world.update_world().unwrap();
                world.with_backend(|b| {
                    if chilled && !ice_used && b.zombies().unwrap().any(|z| b.zombie_kind(z).ok() == Some(Z::Imp)) {
                        b.set_instant_ice_and_ash_effects(true).unwrap();
                        b.new_plant(sel(IceShroom).checked().unwrap(), grid((5, 1))).unwrap();
                        ice_used = true;
                    }
                    PeBackend::scope_backend(b, || {
                        let mut planner = Planner::default();
                        planner.imp_padding = true;
                        planner.scene.sample(frame, -99999);
                        let Some(z) = planner.scene.zombies.iter().find(|z| z.kind == Z::Imp).copied() else {
                            return;
                        };
                        if expected.is_none() && z.phase == ZombiePhase::ImpGettingThrown && (!chilled || z.chilled > 0)
                        {
                            assert_eq!(z.hp, 270);
                            assert_eq!(z.frozen, 0);
                            let (child, delay) = observed_landing(z).unwrap();
                            expected = Some((
                                child.x,
                                frame + prediction::imp_ready_at(child.chilled, &Forecast::default(), 0, delay),
                            ));
                            assert!(planner.imp_puff_hold.is_none());
                            planner.refresh_imp_reservation(&Forecast::default());
                            if !chilled {
                                let hold = planner
                                    .imp_puff_hold
                                    .expect("ready puff must discover airborne child without an old hold");
                                assert!(hold.committed);
                                assert_eq!(hold.source, z.id);
                            }
                        }
                        if landing_expected.is_none() && z.phase == ZombiePhase::ImpLanding {
                            let (child, delay) = observed_landing(z).unwrap();
                            landing_expected =
                                Some(frame + prediction::imp_ready_at(child.chilled, &Forecast::default(), 0, delay));
                        }
                        if actual.is_none() && z.phase == ZombiePhase::ZombieNormal {
                            actual = Some((z.x, frame));
                        }
                    })
                });
                if actual.is_some() {
                    break;
                }
            }
            let (x, at) = expected.expect("observed flight");
            let (actual_x, actual_at) = actual.expect("observed landing complete");
            assert!((actual_x - x).abs() < 1., "{actual_x} vs {x}");
            assert!((actual_at - at).abs() <= 1, "chilled={chilled}: {actual_at} vs {at}");
            assert!(
                (actual_at - landing_expected.unwrap()).abs() <= 2,
                "landing: {actual_at} vs {landing_expected:?}"
            );
        }
    }
    #[test]
    fn near_throw_lands_at_observed_left_site_without_random_draws() {
        let (x, delay) = near_landing(401.02).unwrap();
        assert!((x - 67.02).abs() < 0.1, "{x} {delay}");
        assert_eq!(delay, 92);
        assert!(near_landing(501.).is_none());
    }
}
