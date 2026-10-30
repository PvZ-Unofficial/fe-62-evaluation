use super::*;
use prediction::{Forecast, NEVER, Pad, Scene, Zombie};
use rsvz::core::backend::*;
use rsvz::core::model::{ContactRect, PlantId, ZombieId};
use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};

fn red(row: i32, x: f32) -> Zombie {
    Zombie {
        id: ZombieId::from_raw(65537),
        kind: Z::GigaGargantuar,
        row,
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
#[test]
fn existing_cherry_and_morphing_copy_block_before_activation() {
    let z = red(2, 310.);
    for kind in [PuffShroom, CherryBomb, Imitator, Squash] {
        let mut s = Scene::fixture(vec![z]);
        s.plants.push(prediction::Plant {
            id: PlantId::from_raw(65537),
            kind,
            pos: (2, 4),
            hp: 300,
            rect: ContactRect::new(290, 0, 350, 80),
            attack: ContactRect::new(0, 0, 0, 0),
            shooting: 0,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: if kind == Imitator { 420 } else { 100 },
        });
        assert_eq!(
            prediction::simulate_until(z, &s, &Forecast::default(), None, 99).x_at_end,
            310.,
            "{kind:?}"
        );
        if kind == Squash {
            s.plants[0].state = 5;
            assert!(prediction::simulate_until(z, &s, &Forecast::default(), None, 99).x_at_end < 300.);
        }
    }
}
#[test]
fn destroyed_copy_cancels_only_its_own_ice() {
    let z = red(5, 460.);
    let s = Scene::fixture(vec![z]);
    let mut f = Forecast::default();
    f.add(420, prediction::EffectKind::Ice, (5, 6), false);
    let out = prediction::simulate_action(z, &s, &f, Some(Pad::new((5, 6), Imitator)));
    assert_eq!(out.impact, 134);
    assert_eq!(out.hp_at_end, 6000);
    f.add(500, prediction::EffectKind::Ice, (1, 1), true);
    assert_eq!(
        prediction::simulate_action(z, &s, &f, Some(Pad::new((5, 6), Imitator))).hp_at_end,
        5980
    );
}
#[test]
fn remote_rows_do_not_borrow_a_doomed_live_copy() {
    let z = red(5, 460.);
    let mut s = Scene::fixture(vec![z]);
    s.plants.push(prediction::Plant {
        id: PlantId::from_raw(65537),
        kind: Imitator,
        pos: (5, 6),
        hp: 300,
        rect: ContactRect::new(450, 0, 510, 80),
        attack: ContactRect::new(0, 0, 0, 0),
        shooting: 0,
        launch: 200,
        state: 0,
        phase_left: 200,
        effect: 420,
    });
    let mut f = Forecast::default();
    f.add(420, prediction::EffectKind::Ice, (5, 6), true);
    f.add(800, prediction::EffectKind::Ice, (5, 6), false);
    prediction::resolve_live_ice(&s, &mut f);
    assert_eq!(f.events.len(), 1);
    assert_eq!(f.events[0].at, 800);
    assert_eq!(
        prediction::simulate_until(red(1, 700.), &s, &f, None, 700).hp_at_end,
        6000
    );
}
#[test]
fn frozen_jack_is_ignored_only_with_observed_ice_lower_bound() {
    effects::clear();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Fog,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    let ice = world.with_backend(|b| {
        b.set_zombie_spawn_stopped(true).unwrap();
        let z = b.place_zombie(Z::JackInTheBox, grid((5, 6))).unwrap();
        b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(440.).unwrap())
            .unwrap();
        let p = b.new_plant(sel(IceShroom).checked().unwrap(), grid((1, 1))).unwrap();
        b.plant_id(p)
    });
    world.with_backend(|b| PeBackend::scope_backend(b, || effects::planted(ice, sel(IceShroom), (1, 1), 2)));
    for _ in 0..101 {
        world.update_world().unwrap();
        world.with_backend(|b| PeBackend::scope_backend(b, effects::poll));
    }
    assert!(world.with_backend(|b| PeBackend::scope_backend(b, || combat::jack_safe((5, 6), 182))));
    world.with_backend(|b| {
        let z = b.place_zombie(Z::JackInTheBox, grid((5, 6))).unwrap();
        b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(440.).unwrap())
            .unwrap();
    });
    assert!(!world.with_backend(|b| PeBackend::scope_backend(b, || combat::jack_safe((5, 6), 182))));
    effects::clear();
}
#[test]
fn irrelevant_flyer_and_frozen_throw_do_not_veto_storage() {
    let z = red(5, 810.);
    let mut s = Scene::fixture(vec![z]);
    let mut picker = squash::Predictor::default();
    assert!(picker.prestore(&s, &Forecast::default(), (5, 8), &[z.id]).is_some());
    let balloon = Zombie {
        kind: Z::Balloon,
        phase: ZombiePhase::BalloonFlying,
        id: ZombieId::from_raw(65538),
        ..red(5, 1000.)
    };
    s = Scene::fixture(vec![z, balloon]);
    assert!(picker.prestore(&s, &Forecast::default(), (5, 8), &[z.id]).is_some());
    let frozen = Zombie {
        object: true,
        phase: ZombiePhase::GargantuarThrowing,
        frozen: 400,
        ..red(5, 800.)
    };
    assert!(prediction::prestore_clear(
        &Scene::fixture(vec![frozen]),
        (5, 3),
        100,
        &Forecast::default()
    ));
    let near = Zombie {
        object: true,
        phase: ZombiePhase::GargantuarThrowing,
        progress: 0.73,
        ..red(5, 450.)
    };
    assert!(prediction::prestore_clear(
        &Scene::fixture(vec![near]),
        (5, 3),
        300,
        &Forecast::default()
    ));
    let reachable = Zombie { x: 530., ..near };
    assert!(!prediction::prestore_clear(
        &Scene::fixture(vec![reachable]),
        (5, 3),
        300,
        &Forecast::default()
    ));
    assert!(prediction::prestore_clear(
        &Scene::fixture(vec![reachable]),
        (5, 4),
        300,
        &Forecast::default()
    ));
}
#[test]
fn protected_basketball_assumption_is_removed_when_umbrella_is_threatened() {
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Fog,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|b| {
        b.set_zombie_spawn_stopped(true).unwrap();
        b.new_plant(sel(GloomShroom).checked().unwrap(), grid((2, 1))).unwrap();
        b.new_plant(sel(UmbrellaLeaf).checked().unwrap(), grid((2, 2))).unwrap();
        for _ in 0..8 {
            b.place_zombie(Z::Catapult, grid((2, 8))).unwrap();
        }
    });
    assert!(world.with_backend(|b| PeBackend::scope_backend(b, || rsvz::is_safe_imitator_ice(grid((2, 3))).unwrap())));
    let giant = world.with_backend(|b| {
        let z = b.place_zombie(Z::GigaGargantuar, grid((2, 2))).unwrap();
        b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(160.).unwrap())
            .unwrap();
        b.zombie_id(z)
    });
    assert!(!world.with_backend(|b| PeBackend::scope_backend(b, || rsvz::is_safe_imitator_ice(grid((2, 3))).unwrap())));
    assert!(world.with_backend(|b| b.zombie(giant).unwrap().is_some()));
}
#[test]
fn blover_uses_activation_time_for_lifetime() {
    let (z, s) = (red(6, 210.), Scene::default());
    let mut foot = Zombie {
        kind: Z::Football,
        speed: 0.68,
        hp: 270,
        armor: 1400,
        ..z
    };
    foot.rect = ContactRect::new(246, 0, 288, 115);
    let mut pad = Pad::new((6, 2), Blover);
    pad.complete_outcome = true;
    let out = prediction::simulate_until(foot, &s, &Forecast::default(), Some(pad), 500);
    assert_eq!(pad.lifetime, 251);
    assert!(out.trigger == NEVER || out.trigger < 251);
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Fog,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    let id = world.with_backend(|b| {
        b.set_zombie_spawn_stopped(true).unwrap();
        let p = b.new_plant(sel(Blover).checked().unwrap(), grid((2, 3))).unwrap();
        b.plant_id(p)
    });
    for _ in 0..250 {
        world.update_world().unwrap();
    }
    assert!(world.with_backend(|b| b.plant(id).unwrap().is_some_and(|p| b.plant_is_alive(p))));
    world.update_world().unwrap();
    assert!(!world.with_backend(|b| b.plant(id).unwrap().is_some_and(|p| b.plant_is_alive(p))));
}
#[test]
fn insufficient_combined_sun_does_not_spend_lily() {
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Fog,
        initial_sun: 125,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|b| {
        b.set_zombie_spawn_stopped(true).unwrap();
        b.select_card(sel(DoomShroom).checked().unwrap()).unwrap();
        b.select_card(sel(LilyPad).checked().unwrap()).unwrap();
        b.finish_card_selection().unwrap();
    });
    world.with_backend(|b| {
        PeBackend::scope_backend(b, || {
            assert!(ready(sel(DoomShroom)));
            assert!(ready(sel(LilyPad)));
            assert!(!place(sel(DoomShroom), (3, 7), 3, true));
            assert!(hp(LilyPad, (3, 7)).is_none());
            assert_eq!(rsvz::board::sun(), 125);
        })
    });
}

#[test]
fn exact_combined_sun_can_plant_lily_and_doom() {
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Fog,
        initial_sun: 150,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|b| {
        b.set_zombie_spawn_stopped(true).unwrap();
        b.select_card(sel(DoomShroom).checked().unwrap()).unwrap();
        b.select_card(sel(LilyPad).checked().unwrap()).unwrap();
        b.finish_card_selection().unwrap();
    });
    world.with_backend(|b| {
        PeBackend::scope_backend(b, || {
            assert!(place(sel(DoomShroom), (3, 7), 3, true));
            assert!(hp(LilyPad, (3, 7)).is_some());
            assert!(hp(DoomShroom, (3, 7)).is_some());
            assert_eq!(rsvz::board::sun(), 0);
        })
    });
}

#[test]
fn w20_football_guard_only_spends_puff_when_needed() {
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Fog,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|b| {
        b.set_zombie_spawn_stopped(true).unwrap();
        b.select_card(sel(PuffShroom).checked().unwrap()).unwrap();
        b.finish_card_selection().unwrap();
        let z = b.place_zombie(Z::Football, grid((6, 2))).unwrap();
        b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(140.).unwrap())
            .unwrap();
    });
    world.with_backend(|b| {
        PeBackend::scope_backend(b, || {
            let mut p = planner::Planner::default();
            p.observe(rsvz::timing::clock(), -99999);
            let mut safe = Forecast::default();
            safe.add(100, prediction::EffectKind::Jalapeno, (6, 2), false);
            assert!(!p.guard_w20_football(&safe));
            assert!(ready(sel(PuffShroom)));
            assert!(p.guard_w20_football(&Forecast::default()));
            assert!(!ready(sel(PuffShroom)));
        })
    });
}

#[test]
fn left_old_sixth_cars_are_outside_normal_next_nuke_sites() {
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Fog,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|b| {
        b.set_zombie_spawn_stopped(true).unwrap();
        let id = b.zombie_id(b.place_zombie(Z::Zomboni, grid((6, 3))).unwrap());
        for x in [123., 140., 145.] {
            b.set_zombie_x(
                b.zombie(id).unwrap().unwrap(),
                rsvz::core::model::I32RepresentableF32::new(x).unwrap(),
            )
            .unwrap();
            PeBackend::scope_backend(b, || {
                let rect = rsvz::with_frame(|frame| frame.zombie(id).unwrap().collision());
                for pos in [(3, 7), (4, 7), (3, 8), (4, 8)] {
                    let shape = rsvz::core::contact::grid_explosion_shape(
                        rsvz::core::model::GridExplosionKind::DoomShroom,
                        grid(pos),
                    );
                    assert!(
                        !rsvz::core::contact::circle_hits_rect(shape.circle, rect),
                        "{x} {pos:?}"
                    );
                }
            });
        }
    });
}

#[test]
fn ordinary_ice_and_pepper_detect_an_inflight_basketball_barrage() {
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Fog,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|b| {
        b.set_zombie_spawn_stopped(true).unwrap();
        b.new_plant(sel(WallNut).checked().unwrap(), grid((1, 1))).unwrap();
        for _ in 0..8 {
            b.place_zombie(Z::Catapult, grid((1, 8))).unwrap();
        }
    });
    let mut detected = false;
    for _ in 0..1500 {
        world.update_world().unwrap();
        let (ice, pepper) = world.with_backend(|b| {
            PeBackend::scope_backend(b, || {
                (
                    rsvz::is_safe_ice(grid((1, 1))).unwrap(),
                    rsvz::is_safe_jalapeno(grid((1, 1))).unwrap(),
                )
            })
        });
        assert_eq!(ice, pepper);
        if !ice {
            detected = true;
            break;
        }
    }
    assert!(detected, "visible basketballs must veto a lethal100cs placement");
}

#[test]
fn cherry_ladder_guard_matches_native_destruction_and_blocks_execution() {
    for pos in [(1, 3), (1, 4), (2, 3), (2, 4)] {
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        let safe = pos.1 == 4;
        world.with_backend(|b| {
            b.set_zombie_spawn_stopped(true).unwrap();
            b.add_ladder(grid((2, 2))).unwrap();
            b.select_card(sel(CherryBomb).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            PeBackend::scope_backend(b, || {
                assert_eq!(combat::cherry_preserves_ladders(pos), safe);
                if !safe {
                    assert!(!place(sel(CherryBomb), pos, 9, true));
                }
            });
            b.new_plant(sel(CherryBomb).checked().unwrap(), grid(pos)).unwrap();
        });
        for _ in 0..101 {
            world.update_world().unwrap();
        }
        assert_eq!(
            world.with_backend(|b| b.grid_items().unwrap().any(|item| b.grid_item_is_alive(item)
                && b.grid_item_kind(item).unwrap() == rsvz::core::model::GridItemKind::Ladder)),
            safe
        );
    }
}

#[test]
fn tail_ice_waits_for_both_glooms_and_checks_its_actual_activation() {
    let mut z = red(1, 300.);
    z.kind = Z::Football;
    z.hp = 270;
    z.armor = 1400;
    z.speed = 0.67;
    z.rect = ContactRect::new(300, 0, 350, 100);
    let mut planner = planner::Planner::default();
    planner.scene = Scene::fixture(vec![z]);
    for (col, right) in [(1, 180), (2, 260)] {
        planner.scene.plants.push(prediction::Plant {
            id: PlantId::from_raw(65536 + col as u32),
            kind: GloomShroom,
            pos: (2, col),
            hp: 300,
            rect: ContactRect::new(0, 200, 50, 250),
            attack: ContactRect::new(-100, 0, right, 100),
            shooting: 0,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: 0,
        });
    }
    let due = planner.tail_ice_delay().unwrap();
    assert!(due > 100 && due < NEVER);
    assert!(!planner.tail_ice_inside(sel(IceShroom), None));
    planner.scene.zombies[0].x = 120.;
    planner.scene.zombies[0].rect = ContactRect::new(120, 0, 170, 100);
    assert_eq!(planner.tail_ice_delay(), Some(0));
    assert!(planner.tail_ice_inside(sel(IceShroom), None));
}

#[test]
fn catapult_bait_stops_one_cycle_even_if_target_is_removed() {
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Fog,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|b| {
        b.set_zombie_spawn_stopped(true).unwrap();
        let z = b.place_zombie(Z::Catapult, grid((1, 6))).unwrap();
        b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(400.).unwrap())
            .unwrap();
        b.new_plant(sel(PuffShroom).checked().unwrap(), grid((1, 2))).unwrap();
    });
    world.update_world().unwrap();
    let stopped = world.with_backend(|b| {
        let z = b.zombies().unwrap().next().unwrap();
        assert_eq!(b.zombie_phase(z).unwrap(), ZombiePhase::CatapultLaunching);
        let x = b.zombie_pos_x(z);
        let p = b.plants().unwrap().next().unwrap();
        b.remove_plant(p).unwrap();
        x
    });
    for _ in 0..298 {
        world.update_world().unwrap();
        world.with_backend(|b| {
            let z = b.zombies().unwrap().next().unwrap();
            assert_eq!(
                b.zombie_pos_x(z),
                stopped,
                "target loss must not cancel the current cycle"
            );
        });
    }
    for _ in 0..10 {
        world.update_world().unwrap();
    }
    world.with_backend(|b| {
        let z = b.zombies().unwrap().next().unwrap();
        assert!(
            b.zombie_pos_x(z) < stopped,
            "no permanent parking after bait disappears"
        );
    });
}
