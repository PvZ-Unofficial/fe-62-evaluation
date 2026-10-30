#![cfg(feature = "pvz-emulator")]
use rsvz::dsl::{auto_cobs, p};
use rsvz_backend_api::{PlantCreateBackend, PlantReadBackend, PlantStateWriteBackend};
use rsvz_model::{CardSelection, Grid, PlantKind, Wave, WaveTimingSnapshot};
use rsvz_pvz_emulator_backend::{PeWorldConfig, runner_internal::PeWorldOwner};
use rsvz_schedule::tick::{TickMeta, TickPhase};

#[test]
fn auto_cobs_runs_before_p_at_the_same_actual_due_time() {
    rsvz::reset_runtime_state_preserving_backend();
    rsvz::cob::default_manager().set_list_unchecked(std::iter::empty::<Grid>());
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let id = rsvz::with_backend(|backend| {
                let plant = backend
                    .add_plant(
                        CardSelection::Plant(PlantKind::CobCannon).checked().unwrap(),
                        Grid { row: 0, col: 0 },
                    )
                    .unwrap();
                backend.set_plant_state(plant, 37).unwrap();
                backend.plant_id(plant)
            });
            rsvz::__run_script(|| {
                (1, 0) << auto_cobs();
                (1, 373) << p(1, 8.0);
                Ok(())
            })
            .unwrap();
            let result = rsvz_game::timeline::dispatch_timeline_tick(
                WaveTimingSnapshot::minimal(100, Wave(1)),
                TickMeta {
                    clock: Some(100),
                    phase: TickPhase::Playing,
                    game_ui: Some(rsvz_model::GameUi::Playing),
                    is_new_frame: true,
                },
            );
            assert_eq!(result, rsvz_schedule::TimelineDispatchResult::Continue);
            assert_eq!(
                rsvz_current::CurrentBackend::with_backend_shared(|access| {
                    let backend = access;
                    backend.plant_state(backend.plant(id).unwrap().unwrap())
                })
                .unwrap(),
                38
            );
        })
    });
}

#[test]
fn column_p_resolves_at_execution_and_shares_fire_cursor_with_scene_timing() {
    use rsvz_backend_api::PlantStateBackend;
    use rsvz_pvz_emulator_backend::{PeBackend, pe_rs::SceneType};
    for (scene, fire_clock) in [(SceneType::Day, 27), (SceneType::Pool, 22), (SceneType::Roof, 41)] {
        rsvz::reset_runtime_state_preserving_backend();
        rsvz_game::state_hook::clear_state_hooks();
        // Construct before installing any backend; discover only after registration.
        let expression = rsvz::dsl::p1([(3, 9.0)]);
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene,
            ..PeWorldConfig::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        let first = world.with_backend(|backend| {
            PeBackend::scope_backend(backend, || {
                rsvz::__run_script(|| {
                    (1, 400) << expression;
                    Ok(())
                })
                .unwrap();
                rsvz::with_backend(|backend| {
                    let mut ids = Vec::new();
                    for row in [0, 1] {
                        let plant = backend
                            .add_plant(
                                CardSelection::Plant(PlantKind::CobCannon).checked().unwrap(),
                                Grid { row, col: 0 },
                            )
                            .unwrap();
                        backend.set_plant_state(plant, 37).unwrap();
                        ids.push(backend.plant_id(plant));
                    }
                    ids[0]
                })
            })
        });
        for clock in 0..=fire_clock {
            world.with_backend(|backend| {
                PeBackend::scope_backend(backend, || {
                    assert_eq!(
                        rsvz_game::timeline::dispatch_timeline_tick(
                            WaveTimingSnapshot::minimal(clock, Wave(1)),
                            TickMeta {
                                clock: Some(clock),
                                phase: TickPhase::Playing,
                                game_ui: Some(rsvz_model::GameUi::Playing),
                                is_new_frame: true
                            },
                        ),
                        rsvz_schedule::TimelineDispatchResult::Continue
                    );
                    rsvz::with_backend(|backend| {
                        assert_eq!(
                            backend.plant_state(backend.plant(first).unwrap().unwrap()),
                            if clock < fire_clock { 37 } else { 38 }
                        );
                    });
                });
            });
            if clock < fire_clock {
                world.update_world().unwrap();
            }
        }
        world.with_backend(|backend| {
            PeBackend::scope_backend(backend, || {
                rsvz::with_backend(|backend| {
                    backend
                        .set_plant_state(backend.plant(first).unwrap().unwrap(), 37)
                        .unwrap()
                });
                assert_eq!(
                    rsvz::cob::fire_1(3, 9.0),
                    Some(1),
                    "p1 advanced the shared column cursor"
                );
            });
        });
        rsvz::reset_runtime_state_preserving_backend();
        rsvz_game::state_hook::clear_state_hooks();
    }
}

#[test]
fn auto_cobs_refreshes_existing_columns_and_default_p_initializes_without_a_scan_expression() {
    use rsvz::dsl::{act, p1, pp, recover_p};
    use rsvz_backend_api::PlantStateBackend;
    use rsvz_pvz_emulator_backend::PeBackend;
    rsvz::reset_runtime_state_preserving_backend();
    rsvz_game::state_hook::clear_state_hooks();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            rsvz::__run_script(|| {
                // Default p executes before auto_cobs; p1 must share the reservation/cooldown.
                (1, 373) << p(2, 9);
                (1, 1)
                    << act(|| {
                        assert_eq!(rsvz::cob::fire_1(2, 9), None);
                    });
                (1, 2)
                    << act(|| {
                        rsvz::with_backend(|backend| {
                            let plant = backend
                                .add_plant(
                                    CardSelection::Plant(PlantKind::CobCannon).checked().unwrap(),
                                    Grid { row: 1, col: 0 },
                                )
                                .unwrap();
                            backend.set_plant_state(plant, 37).unwrap();
                        });
                    });
                (1, 3) << auto_cobs();
                (1, 377) << p1(2, 9);
                Ok(())
            })
            .unwrap();
            let first = rsvz::with_backend(|backend| {
                let plant = backend
                    .add_plant(
                        CardSelection::Plant(PlantKind::CobCannon).checked().unwrap(),
                        Grid { row: 0, col: 0 },
                    )
                    .unwrap();
                backend.set_plant_state(plant, 37).unwrap();
                backend.plant_id(plant)
            });
            for clock in 100..=104 {
                rsvz_game::timeline::dispatch_timeline_tick(
                    WaveTimingSnapshot::minimal(clock, Wave(1)),
                    TickMeta {
                        clock: Some(clock),
                        phase: TickPhase::Playing,
                        game_ui: Some(rsvz_model::GameUi::Playing),
                        is_new_frame: true,
                    },
                );
            }
            rsvz::with_backend(|backend| {
                assert_eq!(backend.plant_state(backend.plant(first).unwrap().unwrap()), 38);
                let plants: Vec<_> = backend.plants().unwrap().collect();
                assert_eq!(plants.len(), 2);
                assert!(plants.iter().all(|plant| backend.plant_state(*plant) == 38));
            });
            // Begin another generation with fresh timeline clocks.
            rsvz::reset_runtime_state_preserving_backend();
            rsvz_game::state_hook::dispatch_state_event(rsvz_schedule::state_hook::StateEvent::BeforeScript);
            // Re-register with no auto_cobs; pp and recovery also resolve the default lazily.
            rsvz::__run_script(|| {
                (1, 373) << pp();
                (1, 374) << recover_p(2, 9);
                Ok(())
            })
            .unwrap();
            rsvz::with_backend(|backend| {
                for plant in backend.plants().unwrap() {
                    backend.set_plant_state(plant, 37).unwrap();
                }
            });
            rsvz_game::timeline::dispatch_timeline_tick(
                WaveTimingSnapshot::minimal(100, Wave(1)),
                TickMeta {
                    clock: Some(100),
                    phase: TickPhase::Playing,
                    game_ui: Some(rsvz_model::GameUi::Playing),
                    is_new_frame: true,
                },
            );
            rsvz::with_backend(|backend| {
                assert!(backend.plants().unwrap().all(|plant| backend.plant_state(plant) == 38))
            });
        })
    });
    rsvz::reset_runtime_state_preserving_backend();
    rsvz_game::state_hook::clear_state_hooks();
}
