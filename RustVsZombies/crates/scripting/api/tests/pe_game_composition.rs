#![cfg(feature = "pvz-emulator")]

//! PE atomic capabilities composed through shared game rules.
use rsvz_backend_api::backend::{
    BoardStateBackend, PlantContactBackend, PlantCreateBackend, PlantReadBackend, ZombieContactBackend,
    ZombieCreateBackend, ZombieRawFactsBackend, ZombieReadBackend,
};
use rsvz_game::logic::{
    ZombieSpawnRequest, ZombieTypeSelection, apply_zombie_spawn_request, zombie_motion_state, zombie_state,
};
use rsvz_game::modifier::spawn_zombie;
use rsvz_model::{CardSelection, DamageRangeFlags, Grid, PlantKind, ZombieKind, ZombieMovementModel, ZombieSpawnMode};
use rsvz_pvz_emulator_backend::{PeWorldConfig, runner_internal::PeWorldOwner};

#[test]
fn optional_native_hints_validate_without_a_pe_world_or_tasks() {
    rsvz::core::caption::hint("Balloon Warning", 2).unwrap();
    rsvz::core::caption::hint("", 0).unwrap();
    assert!(rsvz::core::caption::hint("bad\0text", 2).is_err());
    assert!(rsvz::core::caption::hint("[ADVICE_MORE_ZOMBIES]", 2).is_err());
    assert!(rsvz::core::caption::hint("text", u32::MAX).is_err());
    assert!(rsvz::core::caption::hint(std::str::from_utf8(&[b'x'; 127]).unwrap(), 2).is_err());
}

#[test]
fn jack_warning_requires_the_correct_day_or_night_counter_card() {
    use rsvz_backend_api::{CardAppendSelectionBackend, SceneEditBackend, ZombiePhaseCountdownWriteBackend};
    use rsvz_game::warnings::{self, JackWarningMode};
    use rsvz_model::{NonNegativeI32, SceneKind};
    for (scene, expected) in [(SceneKind::Day, false), (SceneKind::Night, true)] {
        let mut owner = PeWorldOwner::new_reset(PeWorldConfig {
            initial_sun: 9990,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        owner.with_backend(|backend| {
            backend.set_scene(scene).unwrap();
            backend
                .select_card(CardSelection::Plant(PlantKind::DoomShroom).checked().unwrap())
                .unwrap();
            backend.finish_card_selection().unwrap();
            backend
                .add_plant(
                    CardSelection::Plant(PlantKind::WallNut).checked().unwrap(),
                    Grid { row: 0, col: 3 },
                )
                .unwrap();
            let zombie = backend
                .add_zombie_in_row(ZombieKind::JackInTheBox, 0, 0)
                .unwrap()
                .unwrap();
            rsvz_backend_api::ZombieXWriteBackend::set_zombie_x(
                backend,
                zombie,
                rsvz_model::I32RepresentableF32::new(280.0).unwrap(),
            )
            .unwrap();
            backend
                .set_zombie_phase_countdown(zombie, NonNegativeI32::new(1).unwrap())
                .unwrap();
        });
        owner.update_world().unwrap();
        owner.with_backend(|backend| {
            let zombie = backend.zombies().unwrap().next().unwrap();
            assert_eq!(backend.zombie_phase_counter(zombie), 110);
            rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
                assert_eq!(warnings::jack_should_pause(JackWarningMode::All).unwrap(), expected);
                assert_eq!(
                    warnings::jack_should_pause(JackWarningMode::OnlyWhenHit).unwrap(),
                    expected
                );
                assert!(!warnings::jack_should_pause(JackWarningMode::Off).unwrap());
            });
        });
    }
}

#[test]
fn smart_removal_disable_suppresses_legacy_tick_without_losing_its_options() {
    use rsvz_backend_api::{PlantPresenceBackend, ZombieXWriteBackend};
    use rsvz_game::{logic::contact, smart_remove};
    use rsvz_model::{I32RepresentableF32, PlantDefenseKind};

    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        let grid = Grid { row: 0, col: 3 };
        let pea = backend
            .add_plant(CardSelection::Plant(PlantKind::Peashooter).checked().unwrap(), grid)
            .unwrap();
        let pumpkin = backend
            .add_plant(CardSelection::Plant(PlantKind::Pumpkin).checked().unwrap(), grid)
            .unwrap();
        let zombie = backend.add_zombie_in_row(ZombieKind::Zomboni, 0, 0).unwrap().unwrap();
        let (pea_id, pumpkin_id, zombie_id) = (
            backend.plant_id(pea),
            backend.plant_id(pumpkin),
            backend.zombie_id(zombie),
        );
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let own = contact::plant_defense_bounds(pumpkin_id, PlantDefenseKind::ChewCrushSmash).unwrap();
            let protected = contact::plant_defense_bounds(pea_id, PlantDefenseKind::ChewCrushSmash).unwrap();
            // Construct a real contact edge: the vehicle reaches the protective
            // shell but not its contents. This tests coordination, not geometry parity.
            let found = (200..500).any(|x| {
                rsvz::with_backend(|backend| {
                    let zombie = backend.zombie(zombie_id).unwrap().unwrap();
                    backend
                        .set_zombie_x(zombie, I32RepresentableF32::new(x as f32).unwrap())
                        .unwrap();
                });
                let attack = contact::zombie_attack_bounds(zombie_id).unwrap().unwrap();
                contact::same_row_and_attack_reaches_plant_geometry(attack, own)
                    && !contact::same_row_and_attack_reaches_plant_geometry(attack, protected)
            });
            assert!(found, "fixture needs a shell-only contact edge");
            smart_remove::start().unwrap();
            smart_remove::request(None).unwrap();
            smart_remove::tick().unwrap();
            assert!(rsvz::with_backend(|backend| backend
                .present_plants()
                .unwrap()
                .any(|plant| backend.plant_id(plant) == pumpkin_id)));
            smart_remove::release_request().unwrap();
            smart_remove::tick().unwrap();
            assert!(!rsvz::with_backend(|backend| backend
                .present_plants()
                .unwrap()
                .any(|plant| backend.plant_id(plant) == pumpkin_id)));
            assert!(rsvz::with_backend(|backend| backend
                .present_plants()
                .unwrap()
                .any(|plant| backend.plant_id(plant) == pea_id)));
            smart_remove::stop();
        });
    });
}

#[test]
fn legacy_capture_preserves_same_cell_objects_imitation_and_ladder() {
    use rsvz_backend_api::{GridItemCreateBackend, PlantStateBackend};
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    owner.with_backend(|backend| {
        let grid = Grid { row: 2, col: 0 };
        for kind in [PlantKind::LilyPad, PlantKind::Peashooter, PlantKind::Peashooter] {
            backend
                .add_plant(CardSelection::Plant(kind).checked().unwrap(), grid)
                .unwrap();
        }
        let imitator = backend
            .add_plant(CardSelection::Imitator(PlantKind::Pumpkin).checked().unwrap(), grid)
            .unwrap();
        let pumpkin = backend.morph_imitator(imitator).unwrap();
        assert_eq!(backend.plant_imitater_kind(pumpkin), 48);
        backend.add_ladder(grid).unwrap();
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let text = rsvz_game::lineup::capture_text().unwrap();
            assert_eq!(text, "0,10 3 1 0 0 0,0 3 1 0 0 0,0 3 1 0 0 0,1E 3 1 0 0 1,30 3 1 0 0 0");
            let document = rsvz_game::lineup::LineupDocument::from_text(&text).unwrap();
            assert_eq!(document.objects().len(), 5);
            assert_eq!(document.to_text().unwrap(), text);
            assert!(
                document.to_code().is_err(),
                "compact conversion must not discard the second pea"
            );
        });
    });
}

#[test]
fn average_rows_balances_each_fresh_kind_without_moving_old_zombies_or_integer_y() {
    use rsvz_backend_api::{GridGeometryBackend, ZombieVerticalPositionBackend};
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    let old = owner.with_backend(|backend| {
        let zombie = backend.add_zombie_in_row(ZombieKind::Conehead, 0, 0).unwrap().unwrap();
        backend.zombie_id(zombie)
    });
    owner.update_world().unwrap();
    owner.with_backend(|backend| {
        let mut fresh = Vec::new();
        for kind in [ZombieKind::Conehead, ZombieKind::Buckethead] {
            for _ in 0..11 {
                let zombie = backend.add_zombie_in_row(kind, 0, 0).unwrap().unwrap();
                fresh.push((
                    backend.zombie_id(zombie),
                    backend.zombie_pos_y(zombie),
                    backend.zombie_int_y(zombie),
                ));
            }
        }
        let old_row = backend.zombie_row(backend.zombie(old).unwrap().unwrap());
        assert!(backend.zombie_age(backend.zombie(old).unwrap().unwrap()) > 0);
        let height = backend.grid_to_pixel_y(0, 1).unwrap() - backend.grid_to_pixel_y(0, 0).unwrap();
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            assert_eq!(rsvz_game::spawn_rows::apply(&[], 5489).unwrap(), 22);
        });
        assert_eq!(backend.zombie_row(backend.zombie(old).unwrap().unwrap()), old_row);
        for kind in [ZombieKind::Conehead, ZombieKind::Buckethead] {
            let mut counts = [0; 6];
            for &(id, y, int_y) in &fresh {
                let zombie = backend.zombie(id).unwrap().unwrap();
                if backend.zombie_kind(zombie).unwrap() != kind {
                    continue;
                }
                let row = backend.zombie_row(zombie);
                counts[row as usize] += 1;
                assert_eq!(backend.zombie_int_y(zombie), int_y);
                assert_eq!(backend.zombie_pos_y(zombie), y + (height * row) as f32);
            }
            assert_eq!(counts[2], 0);
            assert_eq!(counts[3], 0);
            let lanes = [counts[0], counts[1], counts[4], counts[5]];
            assert_eq!(lanes.iter().sum::<i32>(), 11);
            assert_eq!(lanes.iter().max().unwrap() - lanes.iter().min().unwrap(), 1);
        }
    });
}

#[test]
fn spawn_statistics_reads_native_counts_and_stops_at_each_wave_sentinel() {
    use rsvz_backend_api::SpawnScheduleBackend;
    use rsvz_model::SpawnWaveSlot;
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    owner.with_backend(|backend| {
        for wave in 0..20 {
            backend
                .set_spawn_slot(wave, SpawnWaveSlot::new(0).unwrap(), None)
                .unwrap();
        }
        backend
            .set_spawn_slot(0, SpawnWaveSlot::new(0).unwrap(), Some(ZombieKind::Conehead))
            .unwrap();
        backend.set_spawn_slot(0, SpawnWaveSlot::new(1).unwrap(), None).unwrap();
        backend
            .set_spawn_slot(0, SpawnWaveSlot::new(2).unwrap(), Some(ZombieKind::GigaGargantuar))
            .unwrap();
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let stats = rsvz_game::spawn_update::statistics().unwrap();
            assert_eq!(stats.waves.len(), 20);
            assert_eq!(stats.waves[0].counts[2], 1);
            assert_eq!(stats.waves[0].counts[32], 0);
            assert_eq!(stats.waves.iter().flat_map(|wave| wave.counts).sum::<u16>(), 1);
            for (i, wave) in stats.waves.iter().enumerate() {
                assert_eq!(wave.huge, (i + 1) % 10 == 0);
            }
        });
    });
}

#[test]
fn native_rule_last_write_persists_through_cleanup_and_explicit_release_restores() {
    use rsvz_game::modifier::{request_sun_cost_ignored, set_sun_cost_ignored, sun_cost_ignored};
    use rsvz_schedule::state_hook::{StateEvent, StateHookDispatchResult};
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    owner.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let baseline = sun_cost_ignored();
            set_sun_cost_ignored(true);
            set_sun_cost_ignored(false);
            assert!(!sun_cost_ignored());
            request_sun_cost_ignored(None).unwrap();
            assert_eq!(sun_cost_ignored(), baseline);
            set_sun_cost_ignored(true);
            assert_eq!(
                rsvz_game::state_hook::dispatch_state_event(StateEvent::BeforeExit),
                StateHookDispatchResult::Continue
            );
            rsvz_game::state_hook::clear_state_hooks();
            assert!(sun_cost_ignored(), "automatic cleanup retains ordinary native settings");
        })
    });
}

#[test]
fn raw_atoms_compose_through_core() {
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .expect("PE world")
        .install_current()
        .expect("current world");
    owner.with_backend(|backend| {
        let plant = backend
            .add_plant(
                CardSelection::Plant(PlantKind::Peashooter)
                    .checked()
                    .expect("selection"),
                Grid { row: 0, col: 4 },
            )
            .expect("plant");
        let zombie = backend
            .add_zombie_in_row(ZombieKind::Normal, 0, 0)
            .expect("zombie atom")
            .expect("zombie");
        assert!(backend.plant_hit_box(plant).expect("plant rect").is_valid());
        backend.zombie_can_attack_plant(zombie, plant, 0).expect("contact atom");
        backend
            .zombie_effected_by_damage(zombie, DamageRangeFlags::GROUND)
            .expect("damage atom");
        assert!(backend.zombie_reanim_anim_time(zombie).expect("reanimation").is_some());

        let plant_id = backend.plant_id(plant);
        let zombie_id = backend.zombie_id(zombie);
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            use rsvz_game::logic::contact;
            assert!(!matches!(
                zombie_motion_state(zombie_id).expect("live").model,
                ZombieMovementModel::Unsupported(_)
            ));

            assert!(contact::zombie_defense_bounds(zombie_id).expect("defense").is_some());
            assert!(contact::zombie_attack_bounds(zombie_id).expect("attack").is_some());
            assert_eq!(zombie_state(zombie_id).expect("live").kind, ZombieKind::Normal);
            assert!(contact::plant_contact_rect(plant_id).is_some());
        });
    });
}

#[test]
fn unsupported_paired_explosion_rule_does_not_modify_the_supported_half() {
    use rsvz_backend_api::ZombieRuleEditBackend;
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    owner.with_backend(|backend| {
        let before = backend.jack_explosions_disabled().unwrap();
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            assert!(rsvz_game::modifier::set_zombie_explosion_rule(rsvz_model::ZombieExplosionRule::Never).is_err());
        });
        assert_eq!(backend.jack_explosions_disabled().unwrap(), before);
    });
}

#[test]
fn mushroom_rule_repeated_declarations_apply_native_waking_side_effects() {
    use rsvz_backend_api::{PlantEffectRuleEditBackend, PlantSleepBackend};
    use rsvz_schedule::state_hook::{StateEvent, StateHookDispatchResult};
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    owner.with_backend(|backend| {
        let plant = backend
            .add_plant(
                CardSelection::Plant(PlantKind::PuffShroom).checked().unwrap(),
                Grid { row: 0, col: 4 },
            )
            .unwrap();
        backend.set_plant_sleeping(plant, true).unwrap();
        let id = backend.plant_id(plant);
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let asleep = || rsvz::with_backend(|native| native.plant_is_sleeping(native.plant(id).unwrap().unwrap()));
            assert!(asleep());
            rsvz_game::modifier::set_mushrooms_awake(false);
            rsvz_game::modifier::set_mushrooms_awake(true);
            assert!(!asleep(), "enabling must wake a plant already on the board");
            rsvz_game::modifier::set_mushrooms_awake(false);
            assert!(!asleep(), "disabling must not put awakened plants back to sleep");
            rsvz::with_backend(|native| {
                native
                    .set_plant_sleeping(native.plant(id).unwrap().unwrap(), true)
                    .unwrap()
            });
            rsvz_game::modifier::set_mushrooms_awake(true);
            assert!(
                !asleep(),
                "a repeated enabled declaration must wake existing plants again"
            );
            assert_eq!(
                rsvz_game::state_hook::dispatch_state_event(StateEvent::BeforeExit),
                StateHookDispatchResult::Continue
            );
            rsvz_game::state_hook::clear_state_hooks();
            assert!(rsvz::with_backend(|native| native.mushrooms_awake().unwrap()));
        });
    });
}

#[test]
fn placed_zombie_returns_its_id_directly() {
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .expect("PE world")
        .install_current()
        .expect("current world");
    owner.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let id = spawn_zombie(ZombieKind::Normal, Grid { row: 0, col: 7 }).expect("place zombie");
            let zombie = rsvz::Zombie::from_id(id);
            assert_eq!(zombie.kind(), ZombieKind::Normal);
            assert_eq!(zombie.row(), 0);
        });
    });
}
#[test]
fn exact_spawn_writes_native_empty_tail() {
    let mut owner = PeWorldOwner::new_reset_deferred_spawn(PeWorldConfig::default())
        .expect("deferred PE reset should succeed")
        .install_current()
        .expect("current world");
    owner.with_backend(|backend| {
        let request = ZombieSpawnRequest::new(
            ZombieTypeSelection::Exact(vec![
                ZombieKind::JackInTheBox,
                ZombieKind::Ladder,
                ZombieKind::Football,
                ZombieKind::Catapult,
            ]),
            ZombieSpawnMode::Exact,
        )
        .expect("exact request");
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || apply_zombie_spawn_request(&request, 0))
            .expect("apply exact request");

        for (slot, kind) in [
            ZombieKind::JackInTheBox,
            ZombieKind::Ladder,
            ZombieKind::Football,
            ZombieKind::Catapult,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(backend.spawn_entry(0, slot as u32).expect("spawn entry"), kind.code());
        }
        for slot in 4..50 {
            assert_eq!(backend.spawn_entry(0, slot).expect("spawn entry"), -1, "slot {slot}");
        }
    });
}

#[test]
fn lineup_validates_before_clear_and_applies_layers_and_ready_cannon() {
    use rsvz_backend_api::SceneBackend;
    use rsvz_game::lineup::*;
    use rsvz_model::SceneKind;
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: rsvz_pvz_emulator_backend::pe_rs::SceneType::Day,
        ..PeWorldConfig::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let old = rsvz_game::logic::cards::new_plant(PlantKind::Sunflower, Grid { row: 0, col: 7 }).unwrap();
            let mut cells = vec![LineupCell::empty(); SceneKind::Pool.row_count() * 9];
            cells[0].main = Some(LineupPlant::plain(CardSelection::Plant(PlantKind::CobCannon)));
            cells[18].base = LineupBase::LilyPad { imitator: false };
            cells[18].main = Some(LineupPlant::plain(CardSelection::Plant(PlantKind::Peashooter)));
            cells[18].pumpkin = Some(LineupPlant::plain(CardSelection::Plant(PlantKind::Pumpkin)));
            let lineup = Lineup::new(SceneKind::Pool, cells, None).unwrap();
            assert!(matches!(
                apply_lineup(
                    &lineup,
                    LineupApplyOptions {
                        allow_scene_switch: false,
                        ..Default::default()
                    }
                ),
                Err(ApplyLineupError::SceneMismatch { .. })
            ));
            assert!(rsvz::Plant::from_id(old).is_alive());
            let rake = Lineup::new(
                SceneKind::Day,
                vec![LineupCell::empty(); 45],
                Some(Grid { row: 0, col: 7 }),
            )
            .unwrap();
            assert!(matches!(
                apply_lineup(&rake, LineupApplyOptions::default()),
                Err(ApplyLineupError::UnsupportedGridItem(_))
            ));
            assert!(rsvz::Plant::from_id(old).is_alive());
            apply_lineup(&lineup, LineupApplyOptions::default()).unwrap();
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let native = access;
                assert_eq!(native.scene().unwrap(), SceneKind::Pool);
                assert!(native.plant(old).unwrap().is_none());
                let kinds: Vec<_> = native
                    .plants()
                    .unwrap()
                    .map(|p| native.plant_kind(p).unwrap())
                    .collect();
                assert_eq!(
                    kinds,
                    [
                        PlantKind::LilyPad,
                        PlantKind::CobCannon,
                        PlantKind::Peashooter,
                        PlantKind::Pumpkin
                    ]
                );
                let cannon = native
                    .plants()
                    .unwrap()
                    .find(|p| native.plant_kind(*p).unwrap() == PlantKind::CobCannon)
                    .unwrap();
                assert_eq!(native.plant_state(cannon), 37);
                assert_eq!(native.plant_state_countdown(cannon), 0);
                assert_eq!(native.plant_effect_countdown(cannon), 0);
                assert_eq!(rsvz_game::modifier::clear_plants(), 4);
                assert_eq!(rsvz_game::modifier::clear_plants(), 0);
            })
            .unwrap();
            let mut cells = vec![LineupCell::empty(); 45];
            cells[0].main = Some(LineupPlant {
                selection: CardSelection::Plant(PlantKind::PuffShroom),
                awake: Some(false),
                grown: false,
            });
            apply_lineup(
                &Lineup::new(SceneKind::Night, cells, None).unwrap(),
                LineupApplyOptions::default(),
            )
            .unwrap();
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let native = access;
                let plant = native.plants().unwrap().next().unwrap();
                assert!(!native.plant_is_sleeping(plant));
            })
            .unwrap();
        })
    });
}

#[test]
fn opening_keeps_initial_lineup_history_and_ordered_card_prefix() {
    use rsvz_backend_api::CardAppendSelectionBackend;
    use rsvz_game::{lineup::*, setup::*};
    use rsvz_model::SceneKind;
    let mut world = PeWorldOwner::new_reset_deferred_spawn(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let desired = vec![
                CardSelection::Plant(PlantKind::Peashooter),
                CardSelection::Imitator(PlantKind::Pumpkin),
            ];
            let mut cells = vec![LineupCell::empty(); 54];
            cells[0].main = Some(LineupPlant::plain(CardSelection::Plant(PlantKind::Sunflower)));
            let setup = ScriptSetup {
                lineup: Some(PendingLineup::new(
                    Lineup::new(SceneKind::Pool, cells, None).unwrap(),
                    LineupApplyOptions::default(),
                    LineupReloadPolicy::InitialOnly,
                    "test",
                )),
                desired_cards: Some(desired.clone()),
                ..ScriptSetup::default()
            };
            let mut state = OpeningState::default();
            prepare_script_opening(&setup, 7, &mut state).unwrap();
            let first =
                rsvz_current::CurrentBackend::with_backend_shared(|a| a.plant_id(a.plants().unwrap().next().unwrap()))
                    .unwrap();
            prepare_script_opening(&setup, 7, &mut state).unwrap();
            assert!(rsvz::Plant::from_id(first).is_alive());
            state.reset_world();
            prepare_script_opening(&setup, 7, &mut state).unwrap();
            assert!(!rsvz::Plant::from_id(first).is_alive());
            rsvz_current::CurrentBackend::with_backend_shared(|b| b.select_card(desired[0].checked().unwrap()))
                .unwrap()
                .unwrap();
            assert!(matches!(
                verify_selected_cards(&desired),
                Err(VerifySelectedCardsError::TooFew)
            ));
            finish_script_opening(&setup).unwrap();
            verify_selected_cards(&desired).unwrap();
            finish_script_opening(&setup).unwrap();
            let mismatch = ScriptSetup {
                desired_cards: Some(vec![CardSelection::Plant(PlantKind::Sunflower)]),
                ..ScriptSetup::default()
            };
            assert!(matches!(
                finish_script_opening(&mismatch),
                Err(ApplyScriptOpeningError::CardMismatch { index: 0, .. })
            ));
            rsvz_current::CurrentBackend::with_backend_shared(|b| {
                b.select_card(CardSelection::Plant(PlantKind::Sunflower).checked().unwrap())
            })
            .unwrap()
            .unwrap();
            finish_script_opening(&setup).unwrap();
            finish_script_opening(&ScriptSetup::default()).unwrap();
            assert!(matches!(
                verify_selected_cards(&desired),
                Err(VerifySelectedCardsError::TooMany)
            ));
        })
    });
}

#[test]
fn current_spawn_configuration_validates_before_writes_and_resets_flags() {
    use rsvz_game::logic::{ApplySpawnListError, apply_spawn_list};
    use rsvz_model::{DEFAULT_SPAWN_WAVES, SpawnList};
    let mut world = PeWorldOwner::new_reset_deferred_spawn(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let requested = [ZombieKind::JackInTheBox, ZombieKind::JackInTheBox, ZombieKind::Ladder];
            let exact = ZombieSpawnRequest::new(ZombieTypeSelection::Exact(requested.to_vec()), ZombieSpawnMode::Exact)
                .unwrap();
            apply_zombie_spawn_request(&exact, 7).unwrap();
            assert!(matches!(
                apply_spawn_list(&SpawnList::new()),
                Err(ApplySpawnListError::Empty)
            ));
            let mut incomplete = SpawnList::new();
            incomplete.set_wave(0, [ZombieKind::Normal]);
            assert!(matches!(
                apply_spawn_list(&incomplete),
                Err(ApplySpawnListError::MissingWaves { .. })
            ));
            rsvz_current::CurrentBackend::with_backend_shared(|a| {
                let native = a;
                for wave in 0..DEFAULT_SPAWN_WAVES {
                    for slot in 0..50 {
                        assert_eq!(
                            native.spawn_entry(wave as u32, slot).unwrap(),
                            requested.get(slot as usize).map_or(-1, |k| k.code())
                        );
                    }
                }
                for kind in 0..33 {
                    assert_eq!(
                        native.spawn_allowed(kind).unwrap(),
                        kind == ZombieKind::JackInTheBox.code() as u32 || kind == ZombieKind::Ladder.code() as u32
                    );
                }
            })
            .unwrap();
            let natural = ZombieSpawnRequest::new(
                ZombieTypeSelection::Exact(vec![ZombieKind::Gargantuar]),
                ZombieSpawnMode::Natural,
            )
            .unwrap();
            apply_zombie_spawn_request(&natural, 7).unwrap();
            rsvz_current::CurrentBackend::with_backend_shared(|a| {
                let native = a;
                for kind in 0..33 {
                    assert_eq!(
                        native.spawn_allowed(kind).unwrap(),
                        kind == ZombieKind::Normal.code() as u32 || kind == ZombieKind::Gargantuar.code() as u32
                    );
                }
                assert!((0..50).all(|slot| native.spawn_entry(0, slot).unwrap() >= 0));
            })
            .unwrap();
            let mut explicit = SpawnList::new();
            for wave in 0..DEFAULT_SPAWN_WAVES {
                explicit.set_wave(wave, [ZombieKind::Normal, ZombieKind::Buckethead]);
            }
            apply_spawn_list(&explicit).unwrap();
            rsvz_current::CurrentBackend::with_backend_shared(|a| {
                let native = a;
                for kind in 0..33 {
                    assert_eq!(
                        native.spawn_allowed(kind).unwrap(),
                        kind == ZombieKind::Normal.code() as u32 || kind == ZombieKind::Buckethead.code() as u32
                    );
                }
            })
            .unwrap();
        })
    });
}

#[test]
fn refresh_capture_reads_current_health_only_when_due_and_records_once() {
    use rsvz_backend_api::WaveHealthBackend;
    use rsvz_game::measure::{MeasurementState, capture_refresh_after_update};
    use rsvz_model::{Wave, WaveClockState, WaveTimingSnapshot, WavelengthDeclaration};
    let mut state = MeasurementState::new_refresh();
    state.start_trial();
    let mut clocks = WaveClockState::new();
    clocks
        .declare_wavelength_unchecked_bounds(WavelengthDeclaration::assumed(Wave(1), 1000))
        .unwrap();
    clocks.record_refresh_clock(Wave(1), 0);
    // A not-due sample has no backend or world installed at all.
    assert!(
        capture_refresh_after_update(&mut state, WaveTimingSnapshot::minimal(799, Wave(1)), &clocks)
            .unwrap()
            .is_none()
    );
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let native = access;
                let sample =
                    capture_refresh_after_update(&mut state, WaveTimingSnapshot::minimal(800, Wave(1)), &clocks)
                        .unwrap()
                        .unwrap();
                assert_eq!(sample.initial_hp, native.zombie_health_wave_start().unwrap() as u32);
                assert_eq!(
                    sample.current_hp,
                    native.total_zombies_health_in_wave(0).unwrap() as u32
                );
                state.record_refresh_sample(sample, false).unwrap();
            })
            .unwrap();
        })
    });
    assert!(
        capture_refresh_after_update(&mut state, WaveTimingSnapshot::minimal(801, Wave(1)), &clocks)
            .unwrap()
            .is_none()
    );
}

#[test]
fn refresh_observer_returns_access_failure_to_its_session_failure_handler() {
    use rsvz_game::measure::{ObserveRefreshError, RefreshTask};
    use rsvz_model::{MeasureLimit, RefreshMeasureConfig, SessionShard, WaveClockState};
    let mut task = RefreshTask::new(
        MeasureLimit::trials(1).unwrap(),
        RefreshMeasureConfig::default(),
        63,
        SessionShard {
            index: 0,
            count: 1,
            seed_base: 0,
        },
        0,
    );
    let clocks = WaveClockState::new();
    task.observe_after_update(&clocks).unwrap();
    task.tick(0, 0, None).unwrap();
    assert!(matches!(
        task.observe_after_update(&clocks),
        Err(ObserveRefreshError::Backend(_))
    ));
}

#[test]
fn optional_seed_presentation_needs_no_pe_world() {
    for enabled in [true, false] {
        rsvz::core::modifier::request_fog_revealed(Some(enabled)).unwrap();
        rsvz::core::seed_visibility::set_visible(enabled).unwrap();
        rsvz::core::modifier::set_seed_bank_topmost(enabled).unwrap();
        rsvz::core::modifier::set_seed_recharge_shading_hidden(enabled).unwrap();
    }
}

#[test]
fn releasing_one_rule_preserves_other_local_declarations() {
    use rsvz::core::modifier::*;
    use rsvz_schedule::state_hook::StateEvent;
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    owner.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let before_sun = sun_cost_ignored();
            let before_recharge = rsvz::with_backend(|native| {
                rsvz_backend_api::SeedRuleEditBackend::seed_recharge_ignored(native).unwrap()
            });
            set_sun_cost_ignored(!before_sun);
            set_seed_recharge_ignored(!before_recharge);
            request_sun_cost_ignored(None).unwrap();
            assert_eq!(sun_cost_ignored(), before_sun);
            assert_eq!(
                rsvz::with_backend(
                    |native| rsvz_backend_api::SeedRuleEditBackend::seed_recharge_ignored(native).unwrap()
                ),
                !before_recharge
            );
            request_seed_recharge_ignored(None).unwrap();
            assert_eq!(
                rsvz::with_backend(
                    |native| rsvz_backend_api::SeedRuleEditBackend::seed_recharge_ignored(native).unwrap()
                ),
                before_recharge
            );
            let _ = rsvz_game::state_hook::dispatch_state_event(StateEvent::BeforeExit);
            rsvz_game::state_hook::clear_state_hooks();
        })
    });
}

#[test]
fn world_identity_does_not_reuse_a_previous_owner_or_reset() {
    use rsvz_backend_api::{WorldIdentityBackend, WorldResetBackend};
    let first = {
        let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
            .unwrap()
            .install_current()
            .unwrap();
        world.with_backend(|backend| {
            let id = backend.world_identity().unwrap().unwrap();
            assert_eq!(backend.world_identity().unwrap(), Some(id));
            backend
                .reset_world(rsvz_model::WorldResetConfig {
                    completed_rounds: 63,
                    ..Default::default()
                })
                .unwrap();
            assert_ne!(backend.world_identity().unwrap(), Some(id));
            id
        })
    };
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    assert_ne!(
        world.with_backend(|backend| backend.world_identity().unwrap()),
        Some(first)
    );
}

#[test]
fn placement_helpers_preserve_pt_occupancy_ladder_filter_and_duplicate_semantics() {
    use rsvz_backend_api::{GridItemCreateBackend, GridItemReadBackend, PlantPresenceBackend, PlantStateBackend};
    use rsvz_game::placement::{self, BaseKind};
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    owner.with_backend(|backend| {
        for grid in [
            Grid { row: 0, col: 0 },
            Grid { row: 0, col: 1 },
            Grid { row: 0, col: 1 },
        ] {
            backend
                .add_plant(CardSelection::Plant(PlantKind::Pumpkin).checked().unwrap(), grid)
                .unwrap();
        }
        let imitation = backend
            .add_plant(
                CardSelection::Imitator(PlantKind::Pumpkin).checked().unwrap(),
                Grid { row: 1, col: 1 },
            )
            .unwrap();
        let transformed = backend.morph_imitator(imitation).unwrap();
        assert_eq!(backend.plant_imitater_kind(transformed), 48);
        backend
            .add_plant(
                CardSelection::Plant(PlantKind::TangleKelp).checked().unwrap(),
                Grid { row: 2, col: 0 },
            )
            .unwrap();
        backend.add_ladder(Grid { row: 0, col: 8 }).unwrap();
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            assert_eq!(placement::ladder_pumpkins(true).unwrap(), 1);
            assert_eq!(rsvz::with_backend(|native| native.grid_items().unwrap().count()), 1);
            assert_eq!(placement::ladder_pumpkins(false).unwrap(), 3);
            assert_eq!(
                rsvz::with_backend(|native| native
                    .grid_items()
                    .unwrap()
                    .filter(|item| native.grid_item_row(*item) == 0 && native.grid_item_col(*item) == 1)
                    .count()),
                2
            );
            assert_eq!(placement::fill_empty_bases(BaseKind::WaterLily, 2).unwrap(), 3);
            assert_eq!(placement::fill_empty_bases(BaseKind::WaterLily, 2).unwrap(), 0);
            assert_eq!(placement::fill_empty_bases(BaseKind::RoofPot, 2).unwrap(), 0);
            assert_eq!(
                rsvz::with_backend(|native| native
                    .present_plants()
                    .unwrap()
                    .filter(|plant| native.plant_kind(*plant).unwrap() == PlantKind::LilyPad)
                    .count()),
                3
            );
            assert_eq!(placement::place_ladders(None, Some(8)).unwrap(), 6);
            let count = rsvz::with_backend(|native| native.grid_items().unwrap().count());
            assert!(placement::place_ladders(Some(6), Some(8)).is_err());
            assert_eq!(rsvz::with_backend(|native| native.grid_items().unwrap().count()), count);
            rsvz::core::modifier::cleanup::clear_present_plants().unwrap();
            assert_eq!(placement::ladder_pumpkins(false).unwrap(), 0);
            assert_eq!(
                rsvz::with_backend(|native| native.grid_items().unwrap().count()),
                0,
                "no qualifying pumpkins must still clear ladders"
            );
        });
    });
}

#[test]
fn roof_base_fill_skips_occupied_cells_and_stays_in_the_requested_columns() {
    use rsvz_backend_api::PlantPresenceBackend;
    use rsvz_game::placement::{self, BaseKind};
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig {
        scene: rsvz_pvz_emulator_backend::pe_rs::SceneType::Roof,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    owner.with_backend(|backend| {
        backend
            .add_plant(
                CardSelection::Plant(PlantKind::FlowerPot).checked().unwrap(),
                Grid { row: 0, col: 0 },
            )
            .unwrap();
        backend
            .add_plant(
                CardSelection::Plant(PlantKind::Peashooter).checked().unwrap(),
                Grid { row: 0, col: 0 },
            )
            .unwrap();
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            assert!(placement::fill_empty_bases(BaseKind::RoofPot, 0).is_err());
            assert_eq!(placement::fill_empty_bases(BaseKind::RoofPot, 2).unwrap(), 9);
            assert_eq!(placement::fill_empty_bases(BaseKind::RoofPot, 2).unwrap(), 0);
            rsvz::with_backend(|native| {
                let plants: Vec<_> = native.present_plants().unwrap().collect();
                assert_eq!(plants.len(), 11);
                assert!(plants.into_iter().all(|plant| native.plant_col(plant) < 2));
            });
        });
    });
}
