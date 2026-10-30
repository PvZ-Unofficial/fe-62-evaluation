//! Use the already scheduled pepper as a short-lived block, without moving its effect.
use super::*;
use prediction::EffectKind;

#[derive(Debug)]
struct Choice {
    pos: Pos,
    target: ZombieId,
    contact: i32,
    danger: i32,
}

impl Planner {
    pub fn jalapeno_pad(&mut self, forecast: &Forecast) -> bool {
        if !ready(sel(Jalapeno)) {
            return false;
        }
        // Nine cheap spatial tests; full simulation only for cells that a red
        // can reach during the pepper's100cs lifetime. No pixel enumeration.
        let mut legal = [false; 9];
        for col in 1..=9 {
            let pos = (6, col);
            let pad = Pad::new(pos, Jalapeno);
            if !self
                .scene
                .row_zombies(6)
                .any(|z| z.kind == Z::GigaGargantuar && prediction::could_contact_pad(*z, pad))
            {
                continue;
            }
            legal[(col - 1) as usize] = self.scene.main(pos).is_none()
                && combat::candidate(sel(Jalapeno), pos)
                && combat::jack_safe(pos, 100)
                && rsvz::is_safe_jalapeno(grid(pos)).unwrap_or(false);
        }
        let Some(choice) = choose(&self.scene, forecast, legal) else {
            return false;
        };
        if !play(sel(Jalapeno), choice.pos, 19) {
            return false;
        }
        telemetry::record(
            "jalapeno_pad",
            &[
                choice.target.raw() as i64,
                choice.pos.1 as i64,
                choice.contact as i64,
                choice.danger as i64,
            ],
        );
        true
    }
}

fn choose(scene: &Scene, forecast: &Forecast, legal: [bool; 9]) -> Option<Choice> {
    if !legal.iter().any(|&v| v) {
        return None;
    }
    let current = forecast
        .events
        .iter()
        .position(|e| !e.certain && e.kind == EffectKind::Jalapeno && e.pos.0 == 6 && e.at <= 100)?;
    let mut f = Forecast {
        events: forecast.events.clone(),
    };
    // A card planted now activates in100cs even if its original schedule is late.
    f.events[current].at = 100;
    // One no-new-block forecast per reachable red, including this pepper's
    // damage. A finite safe death is proof to skip; a truncated horizon is not.
    let mut threats: Vec<_> = scene
        .row_zombies(6)
        .copied()
        .filter_map(|z| {
            if z.kind != Z::GigaGargantuar
                || !(1..=9).any(|col| {
                    legal[(col - 1) as usize] && prediction::could_contact_pad(z, Pad::new((6, col), Jalapeno))
                })
            {
                return None;
            }
            let base = run_until(z, scene, &f, None, prediction::HORIZON, 2);
            requires_block(base).then_some((z, base.danger))
        })
        .collect();
    threats.sort_by(|(a, at), (b, bt)| at.cmp(bt).then_with(|| a.x.total_cmp(&b.x)));
    for (z, danger) in threats {
        for col in (1..=9).rev().filter(|col| legal[(col - 1) as usize]) {
            let pos = (6, col);
            let mut pad = Pad::new(pos, Jalapeno);
            pad.order = scene.next_plant_order;
            pad.complete_outcome = true;
            if !prediction::could_contact_pad(z, pad) || prediction::external_destruction_bound(scene, z, pad) < 100 {
                continue;
            }
            // Only the physical100cs window is checked for candidate locations.
            // Shared hammers are included. Impact at99 activates at100, on time.
            let short = run_until(z, scene, &f, Some(pad), 99, 1);
            if short.trigger < 100 && short.impact >= 99 && short.death > 99 && short.danger > 99 {
                return Some(Choice {
                    pos,
                    target: z.id,
                    contact: short.trigger,
                    danger,
                });
            }
        }
    }
    None
}
fn requires_block(out: Outcome) -> bool {
    !(out.death < NEVER && out.death <= out.danger)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz::core::model::ContactRect;
    fn red(hp: i32) -> Zombie {
        Zombie {
            id: ZombieId::from_raw(65537),
            kind: Z::GigaGargantuar,
            row: 6,
            wave: 5,
            carried: true,
            x: 260.,
            hp,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.37,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(243, 0, 368, 154),
        }
    }
    fn forecast() -> Forecast {
        Forecast {
            events: vec![prediction::Effect {
                at: 100,
                kind: EffectKind::Jalapeno,
                pos: (6, 2),
                certain: false,
                body_at: None,
                landing: None,
                shape: None,
            }],
        }
    }
    #[test]
    fn risk_not_a_blood_band_controls_ordinary_carried_reds() {
        assert!(choose(&Scene::fixture(vec![red(6000)]), &forecast(), [true; 9]).is_some());
        assert!(choose(&Scene::fixture(vec![red(2400)]), &forecast(), [true; 9]).is_some());
        assert!(choose(&Scene::fixture(vec![red(1800)]), &forecast(), [true; 9]).is_none());
    }
    #[test]
    fn frozen_red_and_unavailable_cells_do_not_delay_the_pepper() {
        let mut z = red(6000);
        z.frozen = 150;
        assert!(choose(&Scene::fixture(vec![z]), &forecast(), [true; 9]).is_none());
        assert!(choose(&Scene::fixture(vec![red(6000)]), &forecast(), [false; 9]).is_none());
    }
    #[test]
    fn pending_hammer_does_not_move_activation_earlier() {
        let mut z = red(6000);
        z.phase = ZombiePhase::GargantuarSmashing;
        z.progress = 0.63;
        let mut legal = [false; 9];
        legal[2] = true;
        assert!(choose(&Scene::fixture(vec![z]), &forecast(), legal).is_none());
        z.id = ZombieId::from_raw(65538);
        assert!(choose(&Scene::fixture(vec![red(6000), z]), &forecast(), legal).is_none());
    }
    #[test]
    fn ordinary_native_pepper_blocks_and_still_fires_on_schedule() {
        use rsvz::core::backend::*;
        use rsvz::core::model::I32RepresentableF32;
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            initial_sun: 8000,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        let zombie = world.with_backend(|b| {
            b.set_zombie_spawn_stopped(true).unwrap();
            b.select_card(sel(Jalapeno).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            let z = b.place_zombie(Z::GigaGargantuar, grid((6, 4))).unwrap();
            b.set_zombie_x(z, I32RepresentableF32::new(260.).unwrap()).unwrap();
            let id = b.zombie_id(z);
            PeBackend::scope_backend(b, || {
                let mut planner = Planner::default();
                planner.scene.sample(rsvz::timing::clock(), 0);
                assert!(planner.jalapeno_pad(&forecast()));
            });
            id
        });
        let mut effect_at = None;
        let mut saw_hammer = false;
        for frame in 1..=110 {
            world.update_world().unwrap();
            world.with_backend(|b| {
                let zombie = b.zombie(zombie).unwrap().unwrap();
                saw_hammer |= b.zombie_phase(zombie).unwrap() == ZombiePhase::GargantuarSmashing;
                if b.zombie_hp(zombie) < 6000 && effect_at.is_none() {
                    effect_at = Some(frame);
                }
            });
        }
        assert!(saw_hammer);
        assert_eq!(effect_at, Some(100));
    }

    #[test]
    fn no_long_term_nuke_coverage_veto_on_a_safe_short_block() {
        use rsvz::core::model::{ContactCircle, PlantThreatShape};
        let z = red(6000);
        let scene = Scene::fixture(vec![z]);
        let mut f = forecast();
        let base = run_until(z, &scene, &f, None, 400, 2);
        let mut pad = Pad::new((6, 3), Jalapeno);
        pad.complete_outcome = true;
        let after = run_until(z, &scene, &f, Some(pad), 400, 2);
        assert!(after.x_at_end > base.x_at_end + 10.);
        let mut legal = [false; 9];
        legal[2] = true;
        f.events.push(prediction::Effect {
            at: 400,
            kind: EffectKind::Doom,
            pos: (4, 7),
            certain: false,
            body_at: None,
            landing: None,
            shape: Some(PlantThreatShape {
                kind: rsvz::core::model::PlantThreatKind::DoomShroom,
                center_row: 3,
                damage_flags: rsvz::core::model::DamageRangeFlags::ALL_NON_MIND_CONTROLLED,
                circle: ContactCircle::new(z.rect_at(base.x_at_end).left - 249, 40, 250),
                row_range: 3,
            }),
        });
        assert_ne!(run_until(z, &scene, &f, None, 400, 2).ash_mask & 2, 0);
        assert_eq!(run_until(z, &scene, &f, Some(pad), 400, 2).ash_mask & 2, 0);
        assert!(
            choose(&scene, &f, legal).is_some(),
            "a risky red needs a short block, not an additional long-term comparison"
        );
        f.events[1].shape.as_mut().unwrap().circle = ContactCircle::new(z.rect_at(after.x_at_end).right + 249, 40, 250);
        assert!(choose(&scene, &f, legal).is_some());
    }
    #[test]
    fn natural_damage_and_the_scheduled_pepper_can_prove_no_block_needed() {
        use rsvz::core::model::PlantId;
        let z = red(2000);
        let mut scene = Scene::fixture(vec![z]);
        scene.plants.push(prediction::Plant {
            id: PlantId::from_raw(65537),
            kind: GloomShroom,
            pos: (5, 1),
            hp: 300,
            rect: ContactRect::new(50, 0, 110, 80),
            attack: ContactRect::new(-200, 0, 1000, 240),
            shooting: 135,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: 0,
        });
        let outcome = run_until(z, &scene, &forecast(), None, prediction::HORIZON, 2);
        assert!(outcome.death > 100 && outcome.death < outcome.danger);
        assert!(choose(&scene, &forecast(), [true; 9]).is_none());
    }
    #[test]
    fn an_unfinished_horizon_is_not_a_safety_certificate() {
        assert!(requires_block(Outcome::default()));
        assert!(!requires_block(Outcome {
            death: 200,
            ..Outcome::default()
        }));
        assert!(requires_block(Outcome {
            danger: 100,
            death: 200,
            ..Outcome::default()
        }));
    }
}
