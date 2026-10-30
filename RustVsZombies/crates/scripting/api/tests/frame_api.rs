#![cfg(feature = "pvz-emulator")]
use rsvz_backend_api::{
    BattleEntryBackend, CardAppendSelectionBackend, PlantCreateBackend, PlantStateWriteBackend, SeedRuleEditBackend,
    ZombieCreateBackend, ZombieReadBackend,
};
use rsvz_model::{CardSelection, GameUi, Grid, PlantKind, Wave, WaveTimingSnapshot, ZombieKind};
use rsvz_pvz_emulator_backend::{PeWorldConfig, runner_internal::PeWorldOwner};
use rsvz_schedule::{TickControl, TickMeta, TickOptions, TickPhase, TickTaskState, TimelineDispatchResult};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn reset() {
    rsvz_game::frame::reset_runtime_state_preserving_backend();
    rsvz_game::state_hook::clear_state_hooks();
    rsvz_game::lifecycle::reset_session_resources();
    rsvz_game::session::reset_session_control();
    let _ = rsvz::cob::default_manager();
}
fn meta(clock: i32) -> TickMeta {
    TickMeta {
        phase: TickPhase::Playing,
        game_ui: Some(GameUi::Playing),
        clock: Some(clock),
        is_new_frame: true,
    }
}

#[test]
fn lineup_matures_imitators_before_ladders_without_accelerating_played_cards() {
    use rsvz_backend_api::{GridItemReadBackend, PlantReadBackend, PlantStateBackend};
    use rsvz_model::GridItemKind;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let lineup =
                rsvz::Lineup::from_code("LI5HDH/tAib/1HpXSFw4wch97UF84sI1eRCQNXFEOiEMneRlNHlQFhE4RHbM7lBkGFJU8MFKAFY=")
                    .unwrap();
            rsvz_game::lineup::apply_lineup(&lineup, Default::default()).unwrap();
        });
    });
    let assert_lineup = |backend: &mut rsvz_pvz_emulator_backend::PeBackend| {
        let mut imitated_pumpkins = 0;
        for plant in backend.plants().unwrap() {
            assert_ne!(
                backend.plant_raw_kind(plant).unwrap(),
                PlantKind::Imitator,
                "lineup has no pending morphs"
            );
            if backend.plant_imitater_kind(plant) == 48 {
                assert_eq!(backend.plant_raw_kind(plant).unwrap(), PlantKind::Pumpkin);
                assert_eq!(backend.plant_hp(plant), 4000);
                imitated_pumpkins += 1;
            }
        }
        assert_eq!(imitated_pumpkins, 10);
        assert_eq!(
            backend
                .grid_items()
                .unwrap()
                .filter(|&g| backend.grid_item_kind(g).unwrap() == GridItemKind::Ladder)
                .count(),
            10
        );
    };
    world.with_backend(assert_lineup);
    world.with_backend(|backend| backend.start_battle().unwrap());
    for _ in 0..400 {
        world.update_world().unwrap();
    }
    world.with_backend(assert_lineup);
    let id = world.with_backend(|backend| {
        let placeholder = backend
            .new_plant(
                CardSelection::Imitator(PlantKind::IceShroom).checked().unwrap(),
                Grid { row: 0, col: 8 },
            )
            .unwrap();
        assert_eq!(backend.plant_raw_kind(placeholder).unwrap(), PlantKind::Imitator);
        assert_eq!(backend.plant_state_countdown(placeholder), 200);
        backend.plant_id(placeholder)
    });
    world.update_world().unwrap();
    world.with_backend(|backend| {
        let placeholder = backend.plant(id).unwrap().unwrap();
        assert_eq!(backend.plant_raw_kind(placeholder).unwrap(), PlantKind::Imitator);
        assert_eq!(backend.plant_state_countdown(placeholder), 199);
    });
    reset();
}

#[test]
fn active_time_skips_native_delays_without_interrupting_the_script() {
    use rsvz_backend_api::{PlantEffectCountdownWriteBackend, PlantReadBackend};
    use rsvz_model::NonNegativeI32;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let id = rsvz::with_backend(|backend| {
                backend.start_battle().unwrap();
                let plant = backend
                    .add_plant(
                        CardSelection::Plant(PlantKind::IceShroom).checked().unwrap(),
                        Grid { row: 0, col: 0 },
                    )
                    .unwrap();
                backend.set_plant_state(plant, 2).unwrap();
                backend.plant_id(plant)
            });
            let messages = Rc::new(RefCell::new(Vec::new()));
            let captured = Rc::clone(&messages);
            let logger = rsvz::replace_logger(move |record: &rsvz::LogRecord<'_>| {
                captured.borrow_mut().push((record.level, record.message.to_owned()));
            });
            let snapshot = rsvz_game::timing::wave_timing();
            rsvz_game::timeline::dispatch_timeline_tick(snapshot, meta(snapshot.clock));
            for (before, after) in [(7, 10), (13, 10), (44, 44)] {
                rsvz::with_backend(|backend| {
                    backend
                        .set_plant_effect_countdown(
                            backend.plant(id).unwrap().unwrap(),
                            NonNegativeI32::new(before).unwrap(),
                        )
                        .unwrap();
                });
                rsvz::cards::set_plant_active_time(PlantKind::IceShroom, 10).unwrap();
                assert_eq!(
                    rsvz_game::timeline::dispatch_timeline_tick(snapshot, meta(snapshot.clock)),
                    TimelineDispatchResult::Continue
                );
                rsvz::with_backend(|backend| {
                    assert_eq!(
                        backend.plant_effect_countdown(backend.plant(id).unwrap().unwrap()),
                        after
                    );
                });
                assert!(rsvz::session::take_dispatch_outcome().is_none());
            }
            assert_eq!(messages.borrow().len(), 1);
            assert_eq!(messages.borrow()[0].0, rsvz::LogLevel::Debug);
            assert!(messages.borrow()[0].1.contains("countdown=44"));
            rsvz::restore_logger(logger);
        });
    });
    reset();
}

#[test]
fn ice3_schedules_normalization_ten_cs_before_the_requested_effect_time() {
    use rsvz_backend_api::{PlantEffectCountdownWriteBackend, PlantReadBackend};
    use rsvz_model::NonNegativeI32;
    for delay in [298, 299] {
        reset();
        let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
            .unwrap()
            .install_current()
            .unwrap();
        world.with_backend(|backend| {
            rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
                let id = rsvz::with_backend(|backend| {
                    backend.start_battle().unwrap();
                    let plant = backend
                        .add_plant(
                            CardSelection::Plant(PlantKind::IceShroom).checked().unwrap(),
                            Grid { row: 0, col: 0 },
                        )
                        .unwrap();
                    backend.set_plant_state(plant, 2).unwrap();
                    backend
                        .set_plant_effect_countdown(plant, NonNegativeI32::new(12).unwrap())
                        .unwrap();
                    backend.plant_id(plant)
                });
                let snapshot = rsvz_game::timing::wave_timing();
                rsvz_game::timeline::dispatch_timeline_tick(snapshot, meta(snapshot.clock));
                assert!(rsvz::cards::ice3(9).is_err());
                rsvz::cards::ice3(delay).unwrap();
                // Advance scheduler time without updating the plant: distinguish the exact
                // dispatch boundary from native countdown progression.
                for (offset, expected) in [(delay - 11, 12), (delay - 10, 10)] {
                    let later = WaveTimingSnapshot {
                        clock: snapshot.clock + offset,
                        ..snapshot
                    };
                    rsvz_game::timeline::dispatch_timeline_tick(later, meta(later.clock));
                    rsvz::with_backend(|backend| {
                        assert_eq!(
                            backend.plant_effect_countdown(backend.plant(id).unwrap().unwrap()),
                            expected
                        );
                    });
                }
            });
        });
        reset();
    }
}

#[test]
fn frame_actions_reenter_current_core_without_invalidating_borrowed_entities() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let zombie_id = rsvz::with_backend(|backend| {
                backend
                    .select_card(CardSelection::Plant(PlantKind::Sunflower).checked().unwrap())
                    .unwrap();
                backend.start_battle().unwrap();
                backend.set_seed_recharge_ignored(true).unwrap();
                let cob = backend
                    .add_plant(
                        CardSelection::Plant(PlantKind::CobCannon).checked().unwrap(),
                        Grid { row: 0, col: 0 },
                    )
                    .unwrap();
                backend.set_plant_state(cob, 37).unwrap();
                let zombie = backend.add_zombie_in_row(ZombieKind::Normal, 1, 0).unwrap().unwrap();
                backend.zombie_id(zombie)
            });
            rsvz_game::cob::try_set_cobs([(1, 1)]).unwrap();
            let expected_motion =
                rsvz_game::logic::zombie_stable_x_trace(zombie_id, 8).map_err(|error| error.to_string());
            let ran = Rc::new(Cell::new(0));
            let called = Rc::clone(&ran);
            rsvz::__run_script(|| {
                rsvz::at_frame(1, 0, move |frame| {
                    let zombie = frame.zombie(zombie_id).unwrap();
                    assert_eq!(zombie.id(), zombie_id);
                    assert_eq!(zombie.hp(), 270);
                    assert!(zombie.x().is_finite());
                    assert_eq!(zombie.phase(), rsvz_model::ZombiePhase::ZombieNormal);
                    assert!(zombie.animation_progress().is_some());
                    assert_eq!(
                        zombie.stable_x_trace(8).map_err(|error| error.to_string()),
                        expected_motion
                    );
                    if let Ok(trace) = &expected_motion {
                        assert_eq!(zombie.stable_x_at(8).unwrap(), trace[8]);
                    }
                    zombie.remove();
                    assert!(zombie.motion_state().is_none());
                    assert!(rsvz_game::logic::zombie_motion_state(zombie_id).is_none());
                    assert!(matches!(
                        zombie.stable_x_trace(8),
                        Err(rsvz_model::ZombieMotionCallError::ObjectUnavailable)
                    ));
                    assert!(!zombie.is_alive());
                    assert_eq!(zombie.hp(), 270);
                    assert!(frame.zombie(zombie_id).is_none());
                    let mut original_plants = frame.plants();
                    let cob = original_plants.next().unwrap();
                    assert_eq!(cob.kind(), PlantKind::CobCannon);
                    rsvz::cob::fire(2, 9);
                    let sunflower = rsvz::cards::card(PlantKind::Sunflower, 2, 5).unwrap();
                    assert_eq!(cob.kind(), PlantKind::CobCannon);
                    assert_eq!(zombie.hp(), 270);
                    assert!(!zombie.is_alive());
                    assert_eq!(
                        original_plants.count(),
                        0,
                        "the old iterator retains its creation bound"
                    );
                    assert_eq!(frame.plants().count(), 2);
                    assert_eq!(frame.plant(sunflower).unwrap().grid(), Grid { row: 1, col: 4 });
                    let at = frame
                        .plant_at(Grid { row: 1, col: 4 }, |p| p.raw_kind() == PlantKind::Sunflower)
                        .unwrap();
                    assert_eq!(at.id(), sunflower);
                    assert!(!at.is_sleeping());
                    at.remove();
                    assert!(
                        frame.plant_at(Grid { row: 1, col: 4 }, |_| true).is_none(),
                        "grid reads observe same-frame removal"
                    );
                    called.set(called.get() + 1);
                    Some(sunflower)
                });
                Ok(())
            })
            .unwrap();
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(100, Wave(1)), meta(100)),
                TimelineDispatchResult::Continue
            );
            assert_eq!(ran.get(), 1);
            // Same-clock Immediate uses a fresh execution borrow and the same adapter.
            rsvz::timeline::try_at_frame(1, 0, |frame| frame.plants().map(|plant| plant.id()).collect::<Vec<_>>())
                .unwrap();
        })
    });
    reset();
}

#[test]
fn frame_ticks_preserve_options_output_control_and_retry_after_error() {
    reset();
    assert!(rsvz::tick::spawn_frame(TickOptions::any_dispatch(), |_| ()).is_err());
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let calls = Rc::new(Cell::new(0));
            let count = Rc::clone(&calls);
            let task = rsvz::tick::on_frame(move |frame| -> rsvz::RuntimeResult<TickControl> {
                assert_eq!(frame.zombies().count(), 0);
                count.set(count.get() + 1);
                if count.get() == 1 {
                    Err(rsvz::RuntimeError::new("retry"))
                } else {
                    Ok(TickControl::Stop)
                }
            });
            let intro_calls = Rc::new(Cell::new(0));
            let intro = Rc::clone(&intro_calls);
            let intro_task = rsvz::tick::spawn_frame(TickOptions::active_phase(), move |frame| {
                assert_eq!(frame.plants().count(), 0);
                intro.set(intro.get() + 1);
                TickControl::Pause
            })
            .unwrap();
            let intro_meta = TickMeta {
                phase: TickPhase::LevelIntro,
                game_ui: Some(GameUi::LevelIntro),
                clock: None,
                is_new_frame: false,
            };
            let mut errors = Vec::new();
            rsvz_game::tick::dispatch_scheduler_tick_reporting(intro_meta, &mut |error| errors.push(error.to_string()));
            assert_eq!(intro_calls.get(), 1);
            assert_eq!(calls.get(), 0);
            for clock in [1, 2, 3] {
                rsvz_game::tick::dispatch_scheduler_tick_reporting(meta(clock), &mut |error| {
                    errors.push(error.to_string())
                });
            }
            assert_eq!(calls.get(), 2);
            assert_eq!(errors, ["retry"]);
            rsvz_schedule::tick::with_scheduler(|scheduler| {
                assert_eq!(scheduler.state(task), TickTaskState::Stopped);
                assert_eq!(scheduler.state(intro_task), TickTaskState::Paused);
            });
        })
    });
    reset();
}

#[test]
fn frame_acquisition_failure_is_local_and_does_not_use_tick_meta_as_proof() {
    reset();
    let reached = Rc::new(Cell::new(false));
    let callback = Rc::clone(&reached);
    let after_read = Rc::new(Cell::new(false));
    let after_read_callback = Rc::clone(&after_read);
    let logs = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&logs);
    let logger =
        rsvz::replace_logger(move |record: &rsvz::LogRecord<'_>| captured.borrow_mut().push(record.message.to_owned()));
    rsvz::__run_script(|| {
        rsvz::try_at_frame(1, 0, |_| -> () { panic!("missing Board must not invoke the callback") })?;
        rsvz::at(1, 0, move || {
            let _ = rsvz_game::logic::zombie_motion_state(rsvz_model::ZombieId::from_raw(1));
            after_read_callback.set(true);
        });
        rsvz::at(1, 0, || -> () {
            let _ = rsvz::refresh_countdown();
            panic!("missing Board must end the scalar read callback");
        });
        rsvz::at(1, 0, move || callback.set(true));
        Ok(())
    })
    .unwrap();
    assert_eq!(
        rsvz_game::timeline::dispatch_timeline_tick_reporting(
            WaveTimingSnapshot::minimal(100, Wave(1)),
            meta(100),
            &mut rsvz_game::diagnostics::report_runtime_error
        ),
        TimelineDispatchResult::Continue
    );
    assert!(reached.get());
    assert!(!after_read.get());
    assert_eq!(logs.borrow().len(), 3);
    assert!(!rsvz_game::session::script_stop_requested());
    rsvz::restore_logger(logger);
    reset();
}

#[test]
fn ordinary_callbacks_allow_exclusive_access_but_frames_reject_it_locally() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    // Exclusive access advances the sampling epoch. Keep the real world at
    // the supplied wave boundary so later callbacks remain eligible on resample.
    world.with_backend(|backend| rsvz_backend_api::BattleEntryBackend::start_battle(backend).unwrap());
    for _ in 0..601 {
        world.update_world().unwrap();
    }
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let current_clock = rsvz_game::runtime::runtime_frame_facts().unwrap().clock.unwrap();
            rsvz::with_backend(|backend| backend.start_battle().unwrap());
            let ran = Rc::new(Cell::new(0));
            let later = Rc::clone(&ran);
            rsvz::__run_script(|| {
                rsvz::at(1, 0, || rsvz::with_backend(|_| ()));
                rsvz::at_frame(1, 0, |_frame| -> () {
                    rsvz::with_backend(|_| panic!("exclusive callback must not execute"));
                    panic!("access failure must end the Frame callback");
                });
                rsvz::at(1, 0, move || {
                    later.set(later.get() + 1);
                });
                Ok(())
            })
            .unwrap();
            let mut errors = Vec::new();
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick_reporting(
                    WaveTimingSnapshot::minimal(current_clock, Wave(1)),
                    meta(current_clock),
                    &mut |error| errors.push(error)
                ),
                TimelineDispatchResult::Continue
            );
            assert_eq!(ran.get(), 1);
            assert_eq!(errors.len(), 1);
            assert!(errors[0].to_string().contains("active borrow"));
            rsvz::with_backend(|_| ());
        });
    });
    reset();
}

#[test]
fn unbound_reset_rechecks_remaining_task_availability_and_pre_hook_samples() {
    use rsvz_backend_api::WorldResetBackend;
    use rsvz_model::WorldResetConfig;
    use rsvz_schedule::TickDispatchResult;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            rsvz::with_backend(|backend| backend.start_battle().unwrap());
            let sample = rsvz_game::frame::sample_current_frame().unwrap();
            assert_eq!(sample.phase(), TickPhase::Playing);
            rsvz::tick::spawn(TickOptions::any_dispatch(), |_| {
                rsvz::with_backend(|backend| {
                    backend.reset_world(WorldResetConfig {
                        completed_rounds: 63,
                        ..WorldResetConfig::default()
                    })
                })?;
                Ok(TickControl::Stop)
            });
            let active = Rc::new(Cell::new(0));
            let called = Rc::clone(&active);
            rsvz::tick::spawn(TickOptions::active_phase(), move |meta| {
                assert_eq!(meta.phase, TickPhase::LevelIntro);
                called.set(called.get() + 1);
                Ok(TickControl::Continue)
            });
            let playing = rsvz::tick::spawn(TickOptions::playing_frame(), |_| panic!("stale Playing eligibility"));
            assert_eq!(
                rsvz_game::tick::dispatch_scheduler_tick(sample.meta),
                TickDispatchResult::Continue
            );
            assert_eq!(active.get(), 1);
            assert_eq!(
                rsvz_game::frame::dispatch_sample_scheduler(&sample),
                rsvz_game::runtime::RuntimeFrameDispatch::Continue
            );
            assert_eq!(active.get(), 2);
            rsvz_schedule::tick::with_scheduler(|scheduler| {
                assert_eq!(scheduler.state(playing), TickTaskState::Running)
            });
        });
    });
    reset();
}

#[test]
fn required_cooldown_read_has_no_fallback_but_native_action_rejection_is_recoverable() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    let reached = Rc::new(Cell::new(false));
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let after = Rc::clone(&reached);
            rsvz::__run_script(|| {
                rsvz::at(1, 0, || -> () {
                    let _ = rsvz::cards::card_cd(PlantKind::Imitator);
                    panic!("invalid required cooldown must not return zero");
                });
                rsvz::at(1, 0, move || {
                    assert!(rsvz_game::modifier::set_sun(u32::MAX).is_err());
                    assert!(matches!(
                        rsvz_game::modifier::spawn_zombie(ZombieKind::Normal, Grid { row: 99, col: 0 }),
                        Err(rsvz_game::modifier::SpawnZombieError::Rejected(_))
                    ));
                    after.set(true);
                });
                Ok(())
            })
            .unwrap();
            let mut errors = Vec::new();
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick_reporting(
                    WaveTimingSnapshot::minimal(100, Wave(1)),
                    meta(100),
                    &mut |error| errors.push(error)
                ),
                TimelineDispatchResult::Continue
            );
            assert_eq!(errors.len(), 1);
            assert!(errors[0].to_string().contains("invalid cooldown card selection"));
        });
    });
    assert!(reached.get());
    reset();
}

#[test]
fn dancing_and_runtime_cob_scan_binding_use_current_core_without_exclusive_reentry() {
    use rsvz_backend_api::MaidCheatsBackend;
    use rsvz_model::MaidCheat;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            rsvz::with_backend(|backend| {
                for grid in [Grid { row: 0, col: 0 }, Grid { row: 1, col: 2 }] {
                    backend
                        .add_plant(CardSelection::Plant(PlantKind::CobCannon).checked().unwrap(), grid)
                        .unwrap();
                }
            });
            let selected = rsvz::cob::CobManager::new();
            let scan = selected.clone();
            rsvz::__run_script(|| {
                (1, 0) << rsvz::dsl::dancing();
                rsvz::at(1, 0, move || {
                    (1, 1) << rsvz::dsl::auto_cobs();
                    (1, 1) << rsvz::dsl::auto_cobs_col(&scan, 1);
                });
                Ok(())
            })
            .unwrap();
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(100, Wave(1)), meta(100)),
                TimelineDispatchResult::Continue
            );
            assert_eq!(
                rsvz_current::CurrentBackend::with_backend_shared(|backend| backend.maid_cheat())
                    .unwrap()
                    .unwrap(),
                MaidCheat::Dancing
            );
            let mut before = Vec::new();
            rsvz::cob::for_each_cob_grid(|grid| before.push(grid));
            assert!(
                before.is_empty(),
                "preparation reserves capacity; the scan stays scheduled"
            );
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(101, Wave(1)), meta(101)),
                TimelineDispatchResult::Continue
            );
            let mut all = Vec::new();
            let mut column = Vec::new();
            rsvz::cob::for_each_cob_grid(|grid| all.push(grid));
            rsvz::cob::for_each_cob_grid(&selected, |grid| column.push(grid));
            assert_eq!(all, [Grid { row: 0, col: 0 }, Grid { row: 1, col: 2 }]);
            assert_eq!(column, [Grid { row: 0, col: 0 }]);
        });
    });
    reset();
}

#[test]
fn pure_immediate_can_run_during_exclusive_access_but_getters_and_frames_cannot() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(100, Wave(1)), meta(100)),
                TimelineDispatchResult::Continue
            );
            let pure = Rc::new(Cell::new(false));
            let called = Rc::clone(&pure);
            let errors = Rc::new(RefCell::new(Vec::new()));
            let reported = Rc::clone(&errors);
            let logger = rsvz::replace_logger(move |record: &rsvz::LogRecord<'_>| {
                reported.borrow_mut().push(record.message.to_owned())
            });
            rsvz::with_backend(|_| {
                rsvz::try_at(1, 0, move || called.set(true)).unwrap();
                assert!(pure.get());
                rsvz::try_at(1, 0, || -> () {
                    let _ = rsvz::refresh_countdown();
                    panic!("required getter must not pass an exclusive borrow");
                })
                .unwrap();
                rsvz::try_at_frame(1, 0, |_| -> () {
                    panic!("Frame callback must not acquire a conflicting borrow");
                })
                .unwrap();
            });
            rsvz::restore_logger(logger);
            assert_eq!(errors.borrow().len(), 2);
            assert!(errors.borrow().iter().all(|message| message.contains("active borrow")));
            assert!(!rsvz_game::session::script_stop_requested());
            // Both guards were released after the failing callbacks.
            rsvz::try_at_frame(1, 0, |frame| assert_eq!(frame.zombies().count(), 0)).unwrap();
        });
    });
    reset();
}

#[test]
fn public_shared_queries_preserve_epoch_and_native_grid_order() {
    use rsvz_backend_api::PlantReadBackend;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grid = Grid { row: 2, col: 0 };
            let first = rsvz::with_backend(|backend| {
                let mut first = None;
                for kind in [PlantKind::LilyPad, PlantKind::Peashooter, PlantKind::Pumpkin] {
                    let plant = backend
                        .new_plant(CardSelection::Plant(kind).checked().unwrap(), grid)
                        .unwrap();
                    first.get_or_insert(backend.plant_id(plant));
                }
                first.unwrap()
            });
            let epoch = rsvz_current::CurrentBackend::backend_access_epoch();
            rsvz::with_frame(|frame| {
                assert_eq!(rsvz::board::sun_if_available(), Some(rsvz::board::sun()));
                assert_eq!(rsvz::timing::clock_if_available(), Some(rsvz::timing::clock()));
                assert_eq!(frame.plant_at(grid, |_| true).unwrap().id(), first);
                let mut kinds = Vec::new();
                let mut last = None;
                frame.for_each_plant_at_anchor_grid(grid, |plant| {
                    assert!(!plant.is_squished());
                    kinds.push(plant.raw_kind());
                    last = Some(plant.raw_kind());
                });
                assert_eq!(kinds, [PlantKind::Pumpkin, PlantKind::LilyPad, PlantKind::Peashooter]);
                assert_eq!(last, Some(PlantKind::Peashooter));
                assert_eq!(rsvz::with_frame(|inner| inner.plants().count()), 3);
            });
            assert_eq!(rsvz_current::CurrentBackend::backend_access_epoch(), epoch);
        });
    });
    reset();
}

#[test]
fn placement_query_preserves_card_configuration_and_native_rejection() {
    use rsvz_backend_api::PlantPlacementBackend;
    use rsvz_model::Plantability;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grid = Grid { row: 0, col: 0 };
            let card = CardSelection::Plant(PlantKind::Sunflower);
            let imitated = CardSelection::Imitator(PlantKind::IceShroom);
            assert_eq!(rsvz::cards::plantability(card, grid).unwrap(), Plantability::Allowed);
            assert_eq!(
                rsvz::cards::plantability(imitated, grid).unwrap(),
                Plantability::Allowed
            );
            for invalid in [
                CardSelection::Plant(PlantKind::Imitator),
                CardSelection::Imitator(PlantKind::Imitator),
            ] {
                assert_eq!(
                    rsvz::cards::plantability(invalid, grid).unwrap_err(),
                    invalid.checked().unwrap_err()
                );
            }
            let native = rsvz::with_backend(|backend| {
                backend.new_plant(card.checked().unwrap(), grid).unwrap();
                backend.can_plant_at(card.checked().unwrap(), grid).unwrap()
            });
            assert!(matches!(native, Plantability::Rejected(_)));
            assert_eq!(rsvz::cards::plantability(card, grid).unwrap(), native);
            let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rsvz::with_backend(|_| rsvz::cards::plantability(card, grid))
            }))
            .unwrap_err();
            assert!(rsvz::__private::into_callback_error(payload).is_ok());
        });
    });
    reset();
}

#[test]
fn spawn_query_rejects_invalid_indices_without_truncation() {
    use rsvz_game::logic::zombies::{spawn_entry, spawn_wave_count};
    use rsvz_model::{SPAWN_SLOTS_PER_WAVE, SpawnWaveSlot};
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let slot = SpawnWaveSlot::new(0).unwrap();
            let count = spawn_wave_count();
            assert_eq!(count, 20);
            assert!(SpawnWaveSlot::new(SPAWN_SLOTS_PER_WAVE).is_err());
            for wave in [count, usize::MAX] {
                let payload = std::panic::catch_unwind(|| spawn_entry(wave, slot)).unwrap_err();
                assert!(rsvz::__private::into_callback_error(payload).is_ok());
            }
            assert!(spawn_entry(0, slot).is_some());
        });
    });
    reset();
}

#[test]
fn native_timing_fault_preserves_framework_errors_and_user_callback_abort() {
    use rsvz_game::runtime::RuntimeFrameDispatch;
    use rsvz_schedule::TickDispatchResult;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    let spawn = world.with_native_fixture(|world| world.scene().spawn_data().as_ptr());
    // SAFETY: this test owns the world until all callbacks are cleared. No update,
    // scene replacement or simultaneous native borrow occurs while writing this scalar.
    let write_countdown = move |value| unsafe { (*spawn).countdown.next_wave = value };
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            rsvz::with_backend(|backend| backend.start_battle().unwrap());
            let sample = rsvz_game::frame::sample_current_frame().unwrap();
            write_countdown(u32::MAX);
            let error = rsvz_game::frame::sample_current_frame().unwrap_err();
            assert!(error.to_string().contains("next_wave_countdown"));
            let payload = std::panic::catch_unwind(rsvz::timing::wave_timing).unwrap_err();
            assert!(
                rsvz::__private::into_callback_error(payload)
                    .unwrap()
                    .to_string()
                    .contains("next_wave_countdown")
            );
            rsvz::with_backend(|_| ());
            assert!(matches!(
                rsvz_game::frame::dispatch_sample_timeline(&sample),
                RuntimeFrameDispatch::TimingBackendError(_)
            ));
            assert!(matches!(
                rsvz_game::frame::dispatch_sample_scheduler(&sample),
                RuntimeFrameDispatch::FrameCallbackError(_)
            ));

            write_countdown(600);
            rsvz::__run_script(|| {
                rsvz::at(1, 0, move || {
                    rsvz::with_backend(|_| write_countdown(u32::MAX));
                });
                Ok(())
            })
            .unwrap();
            let result =
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(100, Wave(1)), meta(100));
            let TimelineDispatchResult::BackendError(error) = result else {
                panic!("wrong resample result: {result:?}")
            };
            assert!(error.to_string().contains("next_wave_countdown"));

            reset();
            write_countdown(600);
            rsvz::tick::spawn(TickOptions::any_dispatch(), move |_| {
                rsvz::with_backend(|_| write_countdown(u32::MAX));
                Ok(TickControl::Stop)
            });
            let result = rsvz_game::tick::dispatch_scheduler_tick(meta(100));
            let TickDispatchResult::TaskError(error) = result else {
                panic!("wrong resample result: {result:?}")
            };
            assert!(error.to_string().contains("next_wave_countdown"));

            reset();
            let task = rsvz::tick::spawn(TickOptions::any_dispatch(), |_| {
                let _ = rsvz::timing::wave_timing();
                panic!("invalid timing must not return a default snapshot");
                #[allow(unreachable_code)]
                Ok(TickControl::Continue)
            });
            let calls = Rc::new(Cell::new(0));
            let after = Rc::clone(&calls);
            rsvz::tick::spawn(TickOptions::any_dispatch(), move |_| {
                after.set(after.get() + 1);
                Ok(TickControl::Continue)
            });
            let mut errors = Vec::new();
            for clock in [100, 101] {
                assert_eq!(
                    rsvz_game::tick::dispatch_scheduler_tick_reporting(meta(clock), &mut |error| errors.push(error)),
                    TickDispatchResult::Continue
                );
            }
            assert_eq!(errors.len(), 2, "one error per attempted callback");
            assert_eq!(calls.get(), 2);
            rsvz::with_scheduler(|scheduler| assert_eq!(scheduler.state(task), TickTaskState::Running));
            write_countdown(600);
        });
    });
    reset();
}

#[test]
fn native_empty_spawn_is_distinct_from_unknown_kind() {
    use rsvz_model::SpawnWaveSlot;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    let spawn = world.with_native_fixture(|world| world.scene().spawn_data().as_ptr());
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            // SAFETY: owned native fixture, no world updates or overlapping references.
            unsafe {
                (*spawn).spawn_list.0[0] = (-1i32) as u32;
            }
            assert_eq!(
                rsvz_game::logic::zombies::spawn_entry(0, SpawnWaveSlot::new(0).unwrap()),
                None
            );
            unsafe {
                (*spawn).spawn_list.0[0] = 999;
            }
            let payload =
                std::panic::catch_unwind(|| rsvz_game::logic::zombies::spawn_entry(0, SpawnWaveSlot::new(0).unwrap()))
                    .unwrap_err();
            assert!(rsvz::__private::into_callback_error(payload).is_ok());
        });
    });
    reset();
}

#[test]
fn optional_board_reads_require_owner_and_preserve_signed_clock() {
    for read in [
        || {
            let _ = rsvz::board::sun_if_available();
        },
        || {
            let _ = rsvz::timing::clock_if_available();
        },
    ] {
        let payload = std::panic::catch_unwind(read).unwrap_err();
        assert!(rsvz::__private::into_callback_error(payload).is_ok());
    }
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_native_fixture(|world| world.scene().set_main_counter(u32::MAX).unwrap());
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            // PE exposes this wrapping native counter as a signed clock.
            assert_eq!(rsvz::timing::clock_if_available(), Some(-1));
        });
    });
}

#[test]
fn safety_queries_abort_native_faults_but_keep_model_errors() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: rsvz_pvz_emulator_backend::pe_rs::SceneType::Night,
        ..Default::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    let id = world.with_backend(|backend| {
        let z = backend
            .place_zombie(ZombieKind::Normal, Grid { row: 0, col: 8 })
            .unwrap();
        backend.zombie_id(z)
    });
    let raw = world.with_native_fixture(|world| world.scene().zombies().try_to_get(id.raw()).unwrap().as_ptr());
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grid = Grid { row: 0, col: 0 };
            let queries = [rsvz::is_safe_blover, rsvz::is_safe_imitator_ice];
            for query in queries {
                assert!(query(Grid { row: 5, col: 0 }).is_err(), "invalid grid is a model error");
                // SAFETY: this fixture owns the occupied slot; no native update or
                // simultaneous reference accesses it. Restore before native code runs.
                let old = unsafe { (*raw).type_ };
                unsafe {
                    (*raw).type_ = 999;
                }
                let outcome = std::panic::catch_unwind(|| query(grid));
                unsafe {
                    (*raw).type_ = old;
                }
                let payload = outcome.expect_err("native fault must not become a recoverable safety result");
                let error = rsvz::__private::into_callback_error(payload).unwrap();
                assert!(error.to_string().contains("zombie_kind"));
                assert!(query(grid).is_ok(), "unwind releases the shared borrow");
            }
        });
    });
    reset();
}

#[test]
fn wave_health_queries_use_one_based_waves_and_shared_access() {
    use rsvz_backend_api::{SeedRuleEditBackend, WaveHealthBackend};
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_native_fixture(|world| {
        // SAFETY: exclusive fixture access to a live native world, before callback installation.
        unsafe {
            (*world.scene().spawn_data().as_ptr()).hp.initial = 1234;
        }
    });
    let first = world.with_backend(|backend| {
        let z = backend
            .place_zombie(ZombieKind::Normal, Grid { row: 0, col: 8 })
            .unwrap();
        backend.zombie_id(z)
    });
    world.with_native_fixture(|world| {
        let z = world.scene().zombies().try_to_get(first.raw()).unwrap().as_ptr();
        // SAFETY: exclusive fixture access, integer spawn tag only; no update or recycling.
        unsafe {
            (*z).spawn_wave = 1;
        }
    });
    world.with_backend(|backend| {
        let native = (
            backend.total_zombies_health_in_wave(0).unwrap(),
            backend.total_zombies_health_in_wave(1).unwrap(),
        );
        assert_ne!(native.0, native.1);
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let epoch = rsvz_current::CurrentBackend::backend_access_epoch();
            rsvz::with_frame(|_| {
                assert_eq!(rsvz::timing::zombie_health_wave_start(), 1234);
                assert_eq!(rsvz::timing::total_zombies_health_in_wave(Wave(1)), native.0);
                assert_eq!(rsvz::timing::total_zombies_health_in_wave(Wave(2)), native.1);
                for wave in [Wave(0), Wave(i32::MIN)] {
                    let payload =
                        std::panic::catch_unwind(|| rsvz::timing::total_zombies_health_in_wave(wave)).unwrap_err();
                    assert!(rsvz::__private::into_callback_error(payload).is_ok());
                }
                rsvz::core::modifier::set_seed_recharge_ignored(true);
            });
            assert_eq!(rsvz_current::CurrentBackend::backend_access_epoch(), epoch);
        });
        assert!(backend.seed_recharge_ignored().unwrap());
    });
    reset();
}

#[test]
fn complete_entity_queries_preserve_native_values_and_shared_borrows() {
    use rsvz_backend_api::{PlantContactBackend, PlantReadBackend};
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    let id = world.with_backend(|b| {
        let plant = b
            .new_plant(
                CardSelection::Imitator(PlantKind::IceShroom).checked().unwrap(),
                Grid { row: 0, col: 3 },
            )
            .unwrap();
        b.plant_id(plant)
    });
    let zombie_id = world.with_backend(|b| {
        let z = b.place_zombie(ZombieKind::Buckethead, Grid { row: 0, col: 8 }).unwrap();
        b.zombie_id(z)
    });
    world.with_native_fixture(|w| {
        let z = w.scene().zombies().try_to_get(zombie_id.raw()).unwrap().as_ptr();
        // SAFETY: scalar fixture mutation before any update, under exclusive world access.
        unsafe {
            (*z).max_hp = 701;
            (*z).accessory_1.max_hp = 702;
            (*z).accessory_2.max_hp = 703;
            (*z).dy = 12.5;
            (*z).countdown.butter = 37;
            (*z).is_eating = true;
            (*z).is_hypno = true;
        }
        let p = w.scene().plants().try_to_get(id.raw()).unwrap().as_ptr();
        // SAFETY: exclusive fixture access; distinct scalar values, no concurrent update.
        unsafe {
            (*p).x = 317;
            (*p).y = 91;
            (*p).max_hp = 432;
            (*p).countdown.awake = 27;
            (*p).countdown.status = 28;
            (*p).countdown.effect = 29;
            (*p).countdown.dead = 30;
            (*p).countdown.eaten = 31;
            (*p).reanim.progress = 0.375;
        }
    });
    world.with_backend(|b| {
        let hit_box = b.plant_hit_box(b.plant(id).unwrap().unwrap()).unwrap();
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(b, || {
            let epoch = rsvz_current::CurrentBackend::backend_access_epoch();
            rsvz::with_frame(|f| {
                let p = f.plant(id).unwrap();
                let z = f.zombie(zombie_id).unwrap();
                assert_eq!(
                    (z.max_hp(), z.accessory_1_max_hp(), z.accessory_2_max_hp()),
                    (701, 702, 703)
                );
                assert_eq!((z.altitude(), z.buttered_countdown()), (12.5, 37));
                assert!(z.is_eating() && z.is_mind_controlled() && z.has_head());
                assert!(!z.is_dead_or_dying());
                assert_eq!(p.raw_kind(), PlantKind::Imitator);
                assert_eq!(p.kind(), PlantKind::IceShroom);
                assert_eq!((p.x(), p.y(), p.max_hp()), (317, 91, 432));
                assert_eq!(
                    (
                        p.wake_up_counter(),
                        p.state_countdown(),
                        p.effect_countdown(),
                        p.disappear_countdown(),
                        p.recently_eaten_countdown()
                    ),
                    (27, 28, 29, 30, 31)
                );
                assert_eq!(p.animation_progress(), Some(0.375));
                assert_eq!(p.hit_box(), hit_box);
                assert_eq!(f.items().count(), 0);
                assert_eq!(rsvz::board::sun(), f.sun());
            });
            assert_eq!(rsvz_current::CurrentBackend::backend_access_epoch(), epoch);
        });
    });
    reset();
}

#[test]
fn grid_item_creation_rejection_and_same_frame_removal() {
    use rsvz::core::modifier::{add_crater, add_gravestone, add_ladder};
    use rsvz_model::GridItemKind;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|b| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(b, || {
            let ids = [
                add_ladder(Grid { row: 0, col: 1 }).unwrap(),
                add_crater(Grid { row: 1, col: 2 }).unwrap(),
                add_gravestone(Grid { row: 4, col: 3 }).unwrap(),
            ];
            rsvz::with_frame(|f| {
                let actual: Vec<_> = f
                    .grid_items()
                    .map(|g| (g.id(), g.kind(), g.grid(), g.countdown()))
                    .collect();
                assert_eq!(actual.iter().map(|x| x.0).collect::<Vec<_>>(), ids);
                assert_eq!(
                    actual.iter().map(|x| x.1).collect::<Vec<_>>(),
                    [GridItemKind::Ladder, GridItemKind::Crater, GridItemKind::Grave]
                );
                assert_eq!(actual[1].3, 0); // Native AddCrater creates the permanent (zero countdown) form.
                for g in f.grid_items() {
                    g.remove();
                    assert!(!g.is_alive());
                    assert!(ids.contains(&g.id()));
                }
                assert_eq!(f.grid_items().count(), 0);
            });
            assert!(add_ladder(Grid { row: 99, col: 0 }).is_err());
            // Fill the actual pool; native rejection must never fabricate a successful ID.
            let mut rejected = false;
            for _ in 0..4096 {
                if add_ladder(Grid { row: 0, col: 0 }).is_err() {
                    rejected = true;
                    break;
                }
            }
            assert!(rejected, "native pool must reject once full");
        })
    });
    reset();
}

#[test]
fn seed_pickup_distinguishes_cooldown_sun_and_selection_and_rules_restore() {
    use rsvz::core::{cards, modifier as m};
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|b| {
        b.select_card(CardSelection::Plant(PlantKind::Peashooter).checked().unwrap())
            .unwrap();
        b.select_card(CardSelection::Imitator(PlantKind::IceShroom).checked().unwrap())
            .unwrap();
        b.start_battle().unwrap();
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(b, || {
            assert_eq!(
                cards::selected_card(1),
                Some(CardSelection::Imitator(PlantKind::IceShroom))
            );
            assert_eq!(cards::selected_card(cards::selected_card_count()), None);
            assert_eq!(
                cards::seed_base_cost(CardSelection::Plant(PlantKind::Peashooter)).unwrap(),
                100
            );
            assert!(cards::seed_base_recharge(CardSelection::Plant(PlantKind::Peashooter)).unwrap() > 0);
            m::set_sun(0).unwrap();
            m::set_seed_recharge_ignored(true);
            rsvz::with_frame(|f| {
                let seed = f.seeds().next().unwrap();
                assert_eq!(seed.slot(), rsvz_model::SeedSlot::from_index_unchecked(0));
                assert!(!seed.can_pick_up());
            });
            m::set_sun_cost_ignored(true);
            rsvz::with_frame(|f| assert!(f.seeds().next().unwrap().can_pick_up()));
            m::set_sun_cost_ignored(false);
            m::set_seed_recharge_ignored(false);
            assert!(!m::sun_cost_ignored());
            assert!(!m::seed_recharge_ignored());
            m::set_sun(1000).unwrap();
            rsvz::core::card(CardSelection::Plant(PlantKind::Peashooter), 1, 1).unwrap();
            rsvz::with_frame(|f| {
                let seed = f.seeds().next().unwrap();
                assert!(seed.cooldown_remaining() > 0);
                assert!(!seed.can_pick_up(), "positive sun cannot bypass cooldown");
            });

            m::set_item_drop_disabled(true).unwrap();
            assert!(m::item_drop_disabled());
            assert!(m::set_item_drop_disabled(false).is_err());
            assert!(m::pepper_explosions_disabled().is_err());
            assert!(m::set_pepper_explosions_disabled(true).is_err());
            m::set_zombie_spawn_stopped(true);
            assert!(m::zombie_spawn_stopped());
            m::set_zombie_spawn_stopped(false);
            assert!(!m::zombie_spawn_stopped());
        });
    });
    reset();
}

#[test]
fn frame_reads_actual_flying_projectiles_and_occupancy() {
    use rsvz_backend_api::ProjectileReadBackend;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|b| {
        b.add_plant(
            CardSelection::Plant(PlantKind::Peashooter).checked().unwrap(),
            Grid { row: 0, col: 1 },
        )
        .unwrap();
        b.add_plant(
            CardSelection::Plant(PlantKind::CobCannon).checked().unwrap(),
            Grid { row: 1, col: 1 },
        )
        .unwrap();
        b.place_zombie(ZombieKind::Normal, Grid { row: 0, col: 8 }).unwrap();
        b.start_battle().unwrap();
    });
    let mut found = false;
    for _ in 0..500 {
        world.update_world().unwrap();
        found = world.with_backend(|b| b.projectiles().unwrap().next().is_some());
        if found {
            break;
        }
    }
    assert!(found, "native peashooter must generate a real projectile");
    let projectile_id = world.with_backend(|b| b.projectile_id(b.projectiles().unwrap().next().unwrap()));
    world.with_native_fixture(|w| {
        let p = w
            .scene()
            .projectiles()
            .try_to_get(projectile_id.raw())
            .unwrap()
            .as_ptr();
        // SAFETY: modify scalar fixture fields between updates, after real projectile creation.
        unsafe {
            (*p).dx = 1.25;
            (*p).dy1 = 2.5;
            (*p).dy2 = 3.75;
            (*p).ddy = 4.5;
            (*p).dddy = 5.25;
        }
    });
    world.with_backend(|b| {
        let expected: Vec<_> = b
            .projectiles()
            .unwrap()
            .map(|p| {
                (
                    b.projectile_id(p),
                    b.projectile_kind(p),
                    b.projectile_pos_x(p),
                    b.projectile_vel_x(p),
                    b.projectile_age(p),
                )
            })
            .collect();
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(b, || {
            rsvz::with_frame(|f| {
                let actual: Vec<_> = f
                    .projectiles()
                    .map(|p| (p.id(), p.kind_code(), p.x(), p.velocity_x(), p.age()))
                    .collect();
                for p in f.projectiles() {
                    assert_eq!(
                        (p.velocity_x(), p.z(), p.velocity_y(), p.velocity_z(), p.acceleration()),
                        (1.25, 2.5, 3.75, 4.5, 5.25)
                    );
                }
                assert_eq!(actual, expected);
                assert_ne!(actual[0].2, 0.0);
                assert_ne!(actual[0].3, 0.0);
                let mut occupied = 0;
                f.for_each_plant_at_grid(Grid { row: 1, col: 2 }, |p| {
                    assert_eq!(p.kind(), PlantKind::CobCannon);
                    occupied += 1;
                });
                assert_eq!(occupied, 1);
                assert!(f.plant_at(Grid { row: 1, col: 2 }, |_| true).is_none());
            })
        });
    });
    reset();
}
