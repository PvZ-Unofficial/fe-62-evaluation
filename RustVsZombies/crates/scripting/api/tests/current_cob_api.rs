#![cfg(feature = "pvz-emulator")]

use rsvz::cob;
use rsvz::cob::{CobManager, fire, try_fire};
use rsvz_backend_api::backend::{ClockBackend, PlantCreateBackend, PlantReadBackend, PlantStateWriteBackend};
use rsvz_backend_api::{PlantStateBackend, PlantStateCountdownWriteBackend};
use rsvz_game::cob::{CobListOrder, CobSequentialMode, target_to_pixel};
use rsvz_model::{CardSelection, CobTarget, Grid, PlantId, PlantKind, Wave, WaveTimingSnapshot};
use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
use rsvz_schedule::tick::{TickMeta, TickPhase};
use rsvz_schedule::timeline::TimelineDispatchResult;

fn reset() {
    rsvz_game::frame::reset_runtime_state_preserving_backend();
    rsvz_game::state_hook::clear_state_hooks();
    rsvz_game::lifecycle::reset_session_resources();
    rsvz_game::session::reset_session_control();
    let _ = rsvz::cob::default_manager();
}

fn ready(backend: &PeBackend, id: PlantId) {
    let plant = backend.plant(id).unwrap().unwrap();
    backend.set_plant_state(plant, 37).unwrap();
}

fn add_cob(backend: &PeBackend, grid: Grid) -> PlantId {
    let plant = backend
        .add_plant(CardSelection::Plant(PlantKind::CobCannon).checked().unwrap(), grid)
        .unwrap();
    let id = backend.plant_id(plant);
    ready(backend, id);
    id
}

#[test]
fn recovery_uses_present_animation_in_both_nonready_animation_states() {
    reset();
    let mut owner = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    owner.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                let id = add_cob(backend, Grid { row: 0, col: 0 });
                let plant = backend.plant(id).unwrap().unwrap();
                assert_eq!(backend.plant_reanim_circulation(plant).unwrap(), Some(0.0));
                backend.set_plant_state(plant, 36).unwrap();
                assert_eq!(rsvz_game::cob::cob_recover_time(id), 126);
                backend.set_plant_state(plant, 38).unwrap();
                assert_eq!(rsvz_game::cob::cob_recover_time(id), 3475);
            })
            .unwrap();
        });
    });
}

fn assert_fired(selected: PlantId, untouched: PlantId) {
    rsvz_current::CurrentBackend::with_backend_shared(|access| {
        let backend = access;

        assert_eq!(backend.plant_state(backend.plant(selected).unwrap().unwrap()), 38);
        assert_eq!(backend.plant_state(backend.plant(untouched).unwrap().unwrap()), 37);
        ready(backend, selected);
    })
    .unwrap();
}

#[test]
fn ordinary_core_functions_and_script_overloads_share_the_manager() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grid = Grid { row: 0, col: 0 };
            let id = rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                add_cob(backend, grid)
            })
            .unwrap();
            rsvz::cob::try_set_cobs([grid]).unwrap();
            let explicit_grid = Grid { row: 1, col: 0 };
            let explicit_id = rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                add_cob(backend, explicit_grid)
            })
            .unwrap();
            let explicit = CobManager::new();
            rsvz::cob::try_set_cobs(&explicit, [explicit_grid]).unwrap();
            let copied = try_fire;
            let core_fn: fn(i32, f32) -> Result<i32, cob::CobError> = rsvz::core::cob::try_fire::<f32>;
            assert_eq!(core_fn(1, 9.0).unwrap(), 0);
            assert_fired(id, explicit_id);
            assert_eq!(copied(1, 9).unwrap(), 0);
            assert_fired(id, explicit_id);
            assert_eq!(try_fire([(1, 9)]).unwrap().indices, [Some(0)]);
            assert_fired(id, explicit_id);
            assert_eq!(try_fire(&explicit, 1, 9.0).ok(), Some(0));
            assert_fired(explicit_id, id);
            assert_eq!(try_fire(&explicit, [(1, 9.0)]).unwrap().indices, [Some(0)]);
            assert_fired(explicit_id, id);
            assert_eq!(fire(1, 9), Some(0));
            assert_fired(id, explicit_id);
            assert_eq!(fire([(1, 9)]), [Some(0)]);
            assert_fired(id, explicit_id);
            assert_eq!(fire(&explicit, 1, 9), Some(0));
            assert_fired(explicit_id, id);
            assert_eq!(fire(&explicit, [(1, 9)]), [Some(0)]);
            assert_fired(explicit_id, id);
            // A callback returning the operation Result uses the unified at output adapter.
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                ready(backend, id)
            })
            .unwrap();
            rsvz::__run_script(|| {
                rsvz::at(1, 0, || try_fire(1, 9).map_err(rsvz::RuntimeError::from));
                Ok(())
            })
            .unwrap();
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(100, Wave(1)), meta(100)),
                TimelineDispatchResult::Continue
            );
        })
    });
    reset();
}

fn meta(clock: i32) -> TickMeta {
    TickMeta {
        clock: Some(clock),
        phase: TickPhase::Playing,
        game_ui: Some(rsvz_model::GameUi::Playing),
        is_new_frame: true,
    }
}

#[test]
fn public_cob_lists_queries_next_and_raw_preserve_the_selected_manager_and_arguments() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grids = [
                Grid { row: 0, col: 0 },
                Grid { row: 0, col: 3 },
                Grid { row: 1, col: 0 },
                Grid { row: 1, col: 3 },
            ];
            let ids = rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                grids.map(|grid| add_cob(backend, grid))
            })
            .unwrap();
            let explicit = CobManager::new();
            assert!(rsvz::prelude::cob_list().is_empty());
            assert!(cob::cob_list(&explicit).is_empty());
            cob::try_set_cobs([grids[0], grids[1]]).unwrap();
            cob::try_set_cobs(&explicit, [grids[2], grids[3]]).unwrap();
            assert_eq!(cob::cob_list(), grids[..2]);
            assert_eq!(cob::cob_list(&explicit), grids[2..]);
            assert_eq!(rsvz::core::cob::cob_list(), grids[..2]);
            assert_eq!(rsvz::core::cob::cob_list_with_manager(&explicit), grids[2..]);
            assert_eq!(cob::usable_cob_list().unwrap(), ids[..2]);
            assert_eq!(cob::usable_cob_list(&explicit).unwrap(), ids[2..]);
            assert_eq!(cob::recover_cob(&explicit).unwrap(), Some(ids[2]));
            cob::set_cob_sequential_mode(CobSequentialMode::Time);
            cob::set_cob_sequential_mode(&explicit, CobSequentialMode::Priority);
            assert!(cob::set_next_cob_slot(&explicit, 1).is_err());
            assert!(cob::set_next_cob_slot(1).is_ok());
            cob::set_cob_sequential_mode(&explicit, CobSequentialMode::Time);
            cob::set_next_cob_slot(2).unwrap();
            cob::set_next_cob(&explicit, (2, 4)).unwrap();
            assert_eq!(try_fire(1, 9).ok(), Some(1));
            assert_fired(ids[1], ids[3]);
            assert_eq!(try_fire(&explicit, 1, 9).ok(), Some(1));
            assert_fired(ids[3], ids[1]);

            cob::set_cob_sequential_mode(CobSequentialMode::Priority);
            cob::move_cobs_to_list_top([(1, 4)]).unwrap();
            cob::erase_cobs_from_list(&explicit, [(2, 1)]);
            let mut default_grids = Vec::new();
            let mut explicit_grids = Vec::new();
            cob::for_each_cob_grid(|grid| default_grids.push(grid));
            cob::for_each_cob_grid(&explicit, |grid| explicit_grids.push(grid));
            assert_eq!(default_grids, [grids[1], grids[0]]);
            assert_eq!(explicit_grids, [grids[3]]);
            let mut copied = cob::cob_list();
            assert_eq!(copied, default_grids);
            copied.clear();
            assert_eq!(cob::cob_list(), default_grids);
            assert_eq!(cob::cob_list(&explicit), explicit_grids);
            assert!(cob::try_set_cobs(&explicit, [(6, 8)]).is_err());
            assert_eq!(cob::usable_cob_list(&explicit).unwrap(), [ids[3]]);
            cob::set_cobs(&explicit, [(6, 8)]);
            cob::try_set_cobs(&explicit, [grids[3]]).unwrap();
            assert_eq!(cob::usable_cob(&explicit).unwrap(), Some(ids[3]));
            assert_eq!(fire(0, 9), None);
            assert_eq!(try_fire(0, 9).ok(), None);

            let target = target_to_pixel(CobTarget::from_one_based_row(4, 8.5).unwrap()).unwrap();
            cob::try_raw_fire(2, 4, 4, 8.5).unwrap();
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                let plant = backend.plant(ids[3]).unwrap().unwrap();
                // PE plant_cob_cannon::launch stores x - 47 in cannon.x.
                assert_eq!(
                    [backend.plant_target_x(plant), backend.plant_target_y(plant)],
                    [target.x - 47, target.y]
                );
            })
            .unwrap();
            assert_fired(ids[3], ids[1]);
            cob::try_raw_fire([(1, 1, 4, 8.5)]).unwrap();
            assert_fired(ids[0], ids[3]);

            cob::try_auto_set_cobs().unwrap();
            cob::try_auto_set_cobs(&explicit, CobListOrder::Vertical).unwrap();
            assert_eq!(cob::usable_cob_list().unwrap(), ids);
            assert_eq!(
                cob::usable_cob_list(&explicit).unwrap(),
                [ids[0], ids[2], ids[1], ids[3]]
            );
        })
    });
    reset();
}

#[test]
fn delayed_current_fire_shares_reservations_and_survives_manager_drop() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Roof,
        ..PeWorldConfig::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    let (id, observer) = world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grid = Grid { row: 0, col: 0 };
            let id = rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                add_cob(backend, grid)
            })
            .unwrap();
            let source = CobManager::new();
            let observer = CobManager::new();
            let default_grid = Grid { row: 1, col: 3 };
            let default_id = rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                add_cob(backend, default_grid)
            })
            .unwrap();
            cob::try_set_cobs([default_grid]).unwrap();
            rsvz::cob::try_set_cobs(&source, [grid]).unwrap();
            rsvz::cob::try_set_cobs(&observer, [grid]).unwrap();
            assert_eq!(cob::roof_usable_cob_list(9.0).unwrap(), [default_id]);
            assert_eq!(cob::roof_usable_cob_list(&source, 9.0).unwrap(), [id]);
            assert_eq!(cob::roof_recover_cob(&source, 9.0).unwrap(), Some(id));
            assert_eq!(cob::roof_recover_cob_list(&source, 9.0).unwrap()[0].id, Some(id));
            assert_eq!(cob::roof_cob_fly_time(1, 9.0).unwrap(), 359);
            assert_eq!(try_fire(&source, 1, 9.0).ok(), Some(0));
            assert_eq!(try_fire(&observer, 1, 9.0).ok(), None);
            drop(source);
            (id, observer)
        })
    });
    // Existing roof calibration: first-column cob, column-9 drop delays by 28cs.
    for clock in 0..=28 {
        world.with_backend(|backend| {
            rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
                rsvz_pvz_emulator_backend::PeBackend::with_backend(|backend| {
                    assert_eq!(backend.clock().unwrap(), clock)
                });
                let result = rsvz_game::timeline::dispatch_timeline_tick(
                    WaveTimingSnapshot::minimal(clock, Wave(1)),
                    meta(clock),
                );
                assert_eq!(result, TimelineDispatchResult::Continue);
                if clock < 28 {
                    assert_eq!(try_fire(&observer, 1, 9.0).ok(), None);
                }
            })
        });
        if clock < 28 {
            world.update_world().unwrap();
        }
    }
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                let plant = backend.plant(id).unwrap().unwrap();
                assert_eq!(backend.plant_state(plant), 38);
                ready(backend, id);
            })
            .unwrap();
            assert_eq!(try_fire(&observer, 1, 9.0).ok(), Some(0));
        })
    });
    reset();
}

#[test]
fn shovel_overloads_preserve_layers_and_continue_after_invalid_batch_inputs() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let (main, pumpkin) = rsvz::with_backend(|backend| {
                let grid = Grid { row: 0, col: 0 };
                let main = backend
                    .add_plant(CardSelection::Plant(PlantKind::Sunflower).checked().unwrap(), grid)
                    .unwrap();
                let pumpkin = backend
                    .add_plant(CardSelection::Plant(PlantKind::Pumpkin).checked().unwrap(), grid)
                    .unwrap();
                (backend.plant_id(main), backend.plant_id(pumpkin))
            });
            let alive = |id| {
                rsvz_current::CurrentBackend::with_backend_shared(|access| {
                    let backend = access;
                    backend
                        .plant(id)
                        .unwrap()
                        .is_some_and(|plant| backend.plant_is_alive(plant))
                })
                .unwrap()
            };
            rsvz::shovel::try_shovel(1, 1, true).unwrap();
            assert!(!alive(pumpkin));
            assert!(alive(main));
            assert!(rsvz::shovel::try_shovel([(0, 1), (1, 1)]).is_err());
            assert!(!alive(main));
            rsvz::shovel::shovel(1, 1, PlantKind::CoffeeBean);
            rsvz::shovel::shovel([(1, 1)]);
        })
    });
    reset();
}

#[test]
fn plant_tick_waits_for_resources_then_finishes_with_real_pe_cards() {
    use rsvz_backend_api::{BattleEntryBackend, CardAppendSelectionBackend, SeedRuleEditBackend};
    use rsvz_schedule::TickDispatchResult;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            rsvz::with_backend(|backend| {
                for kind in [PlantKind::KernelPult, PlantKind::CobCannon] {
                    backend
                        .select_card(CardSelection::Plant(kind).checked().unwrap())
                        .unwrap();
                }
                backend.start_battle().unwrap();
                backend.set_seed_recharge_ignored(true).unwrap();
            });
            rsvz_game::modifier::set_sun(200).unwrap();
            rsvz_game::cob::plant_cob((1, 1)).unwrap();
            let meta = |clock| TickMeta {
                phase: TickPhase::Playing,
                game_ui: Some(rsvz_model::GameUi::Playing),
                clock: Some(clock),
                is_new_frame: true,
            };
            assert_eq!(
                rsvz_game::tick::dispatch_scheduler_tick(meta(1)),
                TickDispatchResult::Continue
            );
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                let kinds: Vec<_> = backend
                    .plants()
                    .unwrap()
                    .map(|plant| backend.plant_kind(plant).unwrap())
                    .collect();
                assert_eq!(kinds, [PlantKind::KernelPult, PlantKind::KernelPult]);
            })
            .unwrap();
            rsvz_game::modifier::set_sun(1000).unwrap();
            assert_eq!(
                rsvz_game::tick::dispatch_scheduler_tick(meta(2)),
                TickDispatchResult::Continue
            );
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                let kinds: Vec<_> = backend
                    .plants()
                    .unwrap()
                    .map(|plant| backend.plant_kind(plant).unwrap())
                    .collect();
                assert_eq!(kinds, [PlantKind::CobCannon]);
            })
            .unwrap();
            assert_eq!(
                rsvz_game::tick::dispatch_scheduler_tick(meta(3)),
                TickDispatchResult::Continue
            );
        });
    });
    reset();
}

#[test]
fn batch_abort_keeps_prior_fire_and_releases_borrows_before_the_next_callback() {
    use rsvz_game::cob::{CobManagerError, IntoCobTargets};
    use std::{cell::RefCell, rc::Rc};
    struct AbortAfterFirst;
    impl IntoCobTargets for AbortAfterFirst {
        fn try_for_each_cob_target_result<E>(
            self, visit: &mut impl FnMut(Result<CobTarget, CobManagerError>) -> Result<(), E>,
        ) -> Result<(), E> {
            visit(Ok(CobTarget { row: 0, drop_col: 9.0 }))?;
            rsvz_game::diagnostics::abort_operation(rsvz::RuntimeError::new("second target access failed"));
        }
    }
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grids = [
                Grid { row: 0, col: 0 },
                Grid { row: 1, col: 0 },
                Grid { row: 2, col: 0 },
            ];
            let ids = rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                grids.map(|grid| add_cob(backend, grid))
            })
            .unwrap();
            cob::try_set_cobs(grids).unwrap();
            let reports = Rc::new(RefCell::new(Vec::new()));
            let captured = Rc::clone(&reports);
            let previous = rsvz::replace_logger(move |record: &rsvz::LogRecord<'_>| {
                captured.borrow_mut().push(record.message.to_owned())
            });
            rsvz::__run_script(|| {
                rsvz::at(1, 0, || {
                    rsvz_game::cob::try_fire_many(AbortAfterFirst).map_err(rsvz::RuntimeError::from)
                });
                rsvz::at(1, 0, || {
                    assert_eq!(rsvz_game::cob::try_fire(1, 9).ok(), Some(1));
                });
                Ok(())
            })
            .unwrap();
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick_reporting(
                    WaveTimingSnapshot::minimal(0, Wave(1)),
                    meta(0),
                    &mut rsvz_game::diagnostics::report_runtime_error,
                ),
                TimelineDispatchResult::Continue
            );
            rsvz::restore_logger(previous);
            assert_eq!(reports.borrow().len(), 1);
            assert!(reports.borrow()[0].contains("second target access failed"));
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                assert_eq!(
                    ids.map(|id| backend.plant_state(backend.plant(id).unwrap().unwrap())),
                    [38, 38, 37]
                );
            })
            .unwrap();
        })
    });
    reset();
}

#[test]
fn delayed_failure_and_queue_clear_release_reservations_without_targeting_replacements() {
    use rsvz_backend_api::PlantRemoveBackend;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Roof,
        ..PeWorldConfig::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    let manager = CobManager::new();
    let replacement = world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grid = Grid { row: 0, col: 0 };
            let id = rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                add_cob(backend, grid)
            })
            .unwrap();
            cob::try_set_cobs(&manager, [grid]).unwrap();
            assert_eq!(try_fire(&manager, 1, 9).ok(), Some(0));
            assert_eq!(rsvz_game::timeline::runtime_timeline_diagnostics().pending_count, 1);
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(0, Wave(1)), meta(0)),
                TimelineDispatchResult::Continue
            );
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                backend.remove_plant(backend.plant(id).unwrap().unwrap()).unwrap();
                let replacement = add_cob(backend, grid);
                assert_ne!(id, replacement);
                replacement
            })
            .unwrap()
        })
    });
    for _ in 0..28 {
        world.update_world().unwrap();
    }
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let mut errors = Vec::new();
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick_reporting(
                    WaveTimingSnapshot::minimal(28, Wave(1)),
                    meta(28),
                    &mut |error| errors.push(error),
                ),
                TimelineDispatchResult::Continue
            );
            assert_eq!(errors.len(), 1);
            assert_eq!(rsvz_game::timeline::runtime_timeline_diagnostics().pending_count, 0);
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                assert_eq!(backend.plant_state(backend.plant(replacement).unwrap().unwrap()), 37)
            })
            .unwrap();
            assert_eq!(try_fire(&manager, 1, 9).ok(), Some(0));
            assert_eq!(rsvz_game::timeline::runtime_timeline_diagnostics().pending_count, 1);
            rsvz_schedule::timeline::with_timeline(|timeline| timeline.clear());
            assert_eq!(rsvz_game::timeline::runtime_timeline_diagnostics().pending_count, 0);
            assert_eq!(try_fire(&manager, 1, 9).ok(), Some(0));
        })
    });
    reset();
}

#[test]
fn zero_delay_recover_fire_executes_immediately_inside_and_outside_timeline() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grid = Grid { row: 0, col: 0 };
            let id = rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                add_cob(backend, grid)
            })
            .unwrap();
            cob::try_set_cobs([grid]).unwrap();
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(0, Wave(1)), meta(0)),
                TimelineDispatchResult::Continue
            );
            assert_eq!(cob::try_recover_fire(1, 9).ok(), Some(0));
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                assert_eq!(backend.plant_state(backend.plant(id).unwrap().unwrap()), 38);
                ready(backend, id);
            })
            .unwrap();
            rsvz::__run_script(|| {
                rsvz::at(1, 0, move || {
                    assert_eq!(cob::try_recover_fire(1, 9).ok(), Some(0));
                    assert_eq!(rsvz_game::timeline::runtime_timeline_diagnostics().pending_count, 0);
                    rsvz_current::CurrentBackend::with_backend_shared(|backend| {
                        assert_eq!(backend.plant_state(backend.plant(id).unwrap().unwrap()), 38);
                    })
                    .unwrap();
                });
                Ok(())
            })
            .unwrap();
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(0, Wave(1)), meta(0)),
                TimelineDispatchResult::Continue
            );
            assert_eq!(rsvz_game::timeline::runtime_timeline_diagnostics().pending_count, 0);
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                assert_eq!(backend.plant_state(backend.plant(id).unwrap().unwrap()), 38)
            })
            .unwrap();
        })
    });
    reset();
}

#[test]
fn cannon_modes_skip_only_when_allowed_and_batches_keep_logical_failures_in_order() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grids = [
                Grid { row: 0, col: 0 },
                Grid { row: 1, col: 0 },
                Grid { row: 2, col: 0 },
            ];
            let ids = rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                grids.map(|grid| add_cob(backend, grid))
            })
            .unwrap();
            let manager = CobManager::new();
            for (mode, expected) in [
                (CobSequentialMode::Space, None),
                (CobSequentialMode::Time, Some(1)),
                (CobSequentialMode::Priority, Some(1)),
            ] {
                rsvz_current::CurrentBackend::with_backend_shared(|access| {
                    let backend = access;

                    for id in ids {
                        ready(backend, id);
                    }
                    backend
                        .set_plant_state(backend.plant(ids[0]).unwrap().unwrap(), 35)
                        .unwrap();
                })
                .unwrap();
                cob::try_set_cobs(&manager, grids).unwrap();
                cob::set_cob_sequential_mode(&manager, mode);
                assert_eq!(try_fire(&manager, 1, 9).ok(), expected);
                rsvz_current::CurrentBackend::with_backend_shared(|access| {
                    let backend = access;

                    assert_eq!(backend.plant_state(backend.plant(ids[0]).unwrap().unwrap()), 35);
                    assert_eq!(
                        backend.plant_state(backend.plant(ids[1]).unwrap().unwrap()),
                        if expected.is_some() { 38 } else { 37 }
                    );
                })
                .unwrap();
            }
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                for id in ids {
                    ready(backend, id);
                }
            })
            .unwrap();
            cob::set_cob_sequential_mode(&manager, CobSequentialMode::Time);
            cob::try_set_cobs(&manager, grids).unwrap();
            assert_eq!(
                try_fire(&manager, [(0, 9), (1, 9), (1, 9), (1, 9), (1, 9)])
                    .unwrap_err()
                    .report
                    .indices,
                [None, Some(0), Some(1), Some(2), None]
            );
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                assert_eq!(
                    ids.map(|id| backend.plant_state(backend.plant(id).unwrap().unwrap())),
                    [38; 3]
                )
            })
            .unwrap();
        })
    });
    reset();
}

#[test]
fn roof_queries_keep_adjusted_then_raw_then_list_order() {
    use rsvz_backend_api::PlantStateCountdownWriteBackend;
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Roof,
        ..PeWorldConfig::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let grids = [Grid { row: 0, col: 0 }, Grid { row: 1, col: 7 }];
            let ids =
                rsvz_current::CurrentBackend::with_backend_shared(|backend| grids.map(|grid| add_cob(backend, grid)))
                    .unwrap();
            let manager = CobManager::new();
            manager.set_list_unchecked(grids);
            for (raw, expected, usable) in [([20, 10], 1, true), ([50, 36], 1, false), ([10, 10], 0, true)] {
                rsvz_current::CurrentBackend::with_backend_shared(|backend| {
                    for (id, recover) in ids.into_iter().zip(raw) {
                        let plant = backend.plant(id).unwrap().unwrap();
                        backend.set_plant_state(plant, 35).unwrap();
                        backend.set_plant_state_countdown(plant, recover - 125).unwrap();
                    }
                })
                .unwrap();
                assert_eq!(manager.roof_recover_cob(9.0).unwrap(), Some(ids[expected]));
                assert_eq!(manager.roof_usable_cob(9.0).unwrap(), usable.then_some(ids[expected]));
                let expected_list = if usable {
                    vec![ids[expected], ids[1 - expected]]
                } else {
                    vec![]
                };
                assert_eq!(manager.roof_usable_list(9.0).unwrap(), expected_list);
            }
        });
    });
    reset();
}

#[test]
fn column_fire_preserves_its_cursor_and_rediscovers_on_script_restart() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            // Reverse creation order: automatic discovery must still sort by row.
            let (second, first) = rsvz::with_backend(|backend| {
                (
                    add_cob(backend, Grid { row: 2, col: 0 }),
                    add_cob(backend, Grid { row: 0, col: 0 }),
                )
            });
            cob::set_cobs([(1, 1)]);
            assert_eq!(cob::fire_1(2, 9), Some(0));
            assert_fired(first, second);
            assert_eq!(cob::fire_1(4, 9.0), Some(1));
            assert_fired(second, first);
            assert_eq!(cob::default_manager().next_index_raw(), 0);
            assert_eq!(cob::fire_1([(2, 9.0), (4, 9.0), (3, 9.0)]), [Some(0), Some(1), None]);
            assert_eq!(
                try_fire(2, 9).ok(),
                None,
                "default manager sees the same cooling cannon"
            );
            let added = rsvz::with_backend(|backend| {
                ready(backend, first);
                ready(backend, second);
                add_cob(backend, Grid { row: 1, col: 0 })
            });
            assert_eq!(
                rsvz_game::state_hook::dispatch_state_event(rsvz_schedule::state_hook::StateEvent::BeforeScript),
                rsvz_schedule::state_hook::StateHookDispatchResult::Continue
            );
            assert_eq!(cob::fire_1([(2, 9), (4, 9), (3, 9)]), [Some(0), Some(1), Some(2)]);
            rsvz::with_backend(|backend| {
                assert_eq!(backend.plant_state(backend.plant(added).unwrap().unwrap()), 38);
            });
        });
    });
    reset();
}

#[test]
fn all_eight_column_fire_functions_select_the_named_tail_column() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            let ids: Vec<_> =
                rsvz::with_backend(|backend| (0..8).map(|col| add_cob(backend, Grid { row: col % 5, col })).collect());
            let calls: [fn(i32, f32) -> Option<i32>; 8] = [
                |r, c| cob::fire_1(r, c),
                |r, c| cob::fire_2(r, c),
                |r, c| cob::fire_3(r, c),
                |r, c| cob::fire_4(r, c),
                |r, c| cob::fire_5(r, c),
                |r, c| cob::fire_6(r, c),
                |r, c| cob::fire_7(r, c),
                |r, c| cob::fire_8(r, c),
            ];
            for (column, call) in calls.into_iter().enumerate() {
                assert_eq!(call(2, 9.0), Some(0));
                rsvz::with_backend(|backend| {
                    for (index, id) in ids.iter().enumerate() {
                        assert_eq!(
                            backend.plant_state(backend.plant(*id).unwrap().unwrap()),
                            if index <= column { 38 } else { 37 }
                        );
                    }
                });
            }
        });
    });
    reset();
}

#[test]
fn all_eight_try_column_functions_preserve_partial_results_without_reporting() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            let ids: Vec<_> =
                rsvz::with_backend(|backend| (0..8).map(|col| add_cob(backend, Grid { row: col % 5, col })).collect());
            let calls: [fn(i32, f32) -> Result<i32, cob::CobError>; 8] = [
                |r, c| rsvz::prelude::try_fire_1(r, c),
                |r, c| rsvz::prelude::try_fire_2(r, c),
                |r, c| rsvz::prelude::try_fire_3(r, c),
                |r, c| rsvz::prelude::try_fire_4(r, c),
                |r, c| rsvz::prelude::try_fire_5(r, c),
                |r, c| rsvz::prelude::try_fire_6(r, c),
                |r, c| rsvz::prelude::try_fire_7(r, c),
                |r, c| rsvz::prelude::try_fire_8(r, c),
            ];
            let batches: [fn([(i32, i32); 2]) -> Result<cob::CobBatchReport, cob::CobBatchError>; 8] = [
                |targets| rsvz::dsl::prelude::try_fire_1(targets),
                |targets| rsvz::dsl::prelude::try_fire_2(targets),
                |targets| rsvz::dsl::prelude::try_fire_3(targets),
                |targets| rsvz::dsl::prelude::try_fire_4(targets),
                |targets| rsvz::dsl::prelude::try_fire_5(targets),
                |targets| rsvz::dsl::prelude::try_fire_6(targets),
                |targets| rsvz::dsl::prelude::try_fire_7(targets),
                |targets| rsvz::dsl::prelude::try_fire_8(targets),
            ];
            let tuples: [fn((i32, i32)) -> Result<i32, cob::CobError>; 8] = [
                |input| cob::try_fire_1(input),
                |input| cob::try_fire_2(input),
                |input| cob::try_fire_3(input),
                |input| cob::try_fire_4(input),
                |input| cob::try_fire_5(input),
                |input| cob::try_fire_6(input),
                |input| cob::try_fire_7(input),
                |input| cob::try_fire_8(input),
            ];
            let typed: [fn(CobTarget) -> Result<i32, cob::CobError>; 8] = [
                |input| cob::try_fire_1(input),
                |input| cob::try_fire_2(input),
                |input| cob::try_fire_3(input),
                |input| cob::try_fire_4(input),
                |input| cob::try_fire_5(input),
                |input| cob::try_fire_6(input),
                |input| cob::try_fire_7(input),
                |input| cob::try_fire_8(input),
            ];
            let ordinary_tuples: [fn((i32, i32)) -> Option<i32>; 8] = [
                |input| cob::fire_1(input),
                |input| cob::fire_2(input),
                |input| cob::fire_3(input),
                |input| cob::fire_4(input),
                |input| cob::fire_5(input),
                |input| cob::fire_6(input),
                |input| cob::fire_7(input),
                |input| cob::fire_8(input),
            ];
            let ordinary_typed: [fn(CobTarget) -> Option<i32>; 8] = [
                |input| cob::fire_1(input),
                |input| cob::fire_2(input),
                |input| cob::fire_3(input),
                |input| cob::fire_4(input),
                |input| cob::fire_5(input),
                |input| cob::fire_6(input),
                |input| cob::fire_7(input),
                |input| cob::fire_8(input),
            ];
            let seen = std::rc::Rc::new(std::cell::Cell::new(0));
            let count = seen.clone();
            let previous = rsvz::replace_logger(move |_: &rsvz::LogRecord<'_>| count.set(count.get() + 1));
            for (column, call) in calls.into_iter().enumerate() {
                assert_eq!(call(0, 9.0).unwrap_err().code(), "invalid_input");
                let target = rsvz::grid::target(2, 9.0);
                assert_eq!(tuples[column]((2, 9)).unwrap(), 0);
                rsvz::with_backend(|backend| ready(backend, ids[column]));
                assert_eq!(typed[column](target).unwrap(), 0);
                rsvz::with_backend(|backend| ready(backend, ids[column]));
                assert_eq!(ordinary_tuples[column]((2, 9)), Some(0));
                rsvz::with_backend(|backend| ready(backend, ids[column]));
                assert_eq!(ordinary_typed[column](target), Some(0));
                rsvz::with_backend(|backend| ready(backend, ids[column]));
                let report = batches[column]([(0, 9), (2, 9)]).unwrap_err().report;
                assert_eq!(report.indices, [None, Some(0)]);
                assert_eq!(report.accepted, 1);
                assert!(report.complete);
                assert_eq!(report.failures[0].code(), "invalid_input");
                assert_eq!(call(2, 9.0).unwrap_err().code(), "no_ready_cannon");
                rsvz::with_backend(|backend| {
                    for (index, id) in ids.iter().enumerate() {
                        assert_eq!(
                            backend.plant_state(backend.plant(*id).unwrap().unwrap()),
                            if index <= column { 38 } else { 37 }
                        );
                    }
                });
            }
            rsvz::restore_logger(previous);
            assert_eq!(seen.get(), 0);
            assert_eq!(rsvz_game::session::take_dispatch_outcome(), None);
        });
    });
    reset();
}

#[test]
fn lazy_column_initialization_preserves_existing_roof_reservations() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Roof,
        ..PeWorldConfig::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            rsvz::with_backend(|backend| {
                add_cob(backend, Grid { row: 0, col: 0 });
                add_cob(backend, Grid { row: 0, col: 6 });
            });
            cob::set_cobs([(1, 1)]);
            let explicit = CobManager::new();
            cob::set_cobs(&explicit, [(1, 7)]);
            assert_eq!(try_fire(2, 9.0).ok(), Some(0));
            // The first column call must not clear the default manager's reservation.
            assert_eq!(cob::fire_1(4, 9.0), None);
            assert_eq!(cob::fire_7(4, 9.0), Some(0));
            cob::try_auto_set_cobs().unwrap();
            assert_eq!(cob::fire_1(4, 9.0), None, "refresh preserves pending roof reservation");
            assert_eq!(cob::fire_7(4, 9.0), None);
            assert_eq!(try_fire(&explicit, 2, 9.0).ok(), None);
        });
    });
    reset();
}

#[test]
fn default_fire_initializes_once_but_preserves_explicit_empty_lists() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            let first = rsvz::with_backend(|backend| add_cob(backend, Grid { row: 0, col: 0 }));
            assert_eq!(try_fire(2, 9).ok(), Some(0));
            let second = rsvz::with_backend(|backend| add_cob(backend, Grid { row: 1, col: 0 }));
            assert_eq!(try_fire(2, 9).ok(), None, "no rescan on every shot");
            rsvz_game::state_hook::dispatch_state_event(rsvz_schedule::state_hook::StateEvent::BeforeScript);
            assert_eq!(
                try_fire(2, 9).ok(),
                Some(1),
                "new generation rediscovers automatic lists"
            );
            rsvz::with_backend(|backend| {
                ready(backend, first);
                ready(backend, second);
            });
            cob::set_cobs([] as [(i32, i32); 0]);
            rsvz_game::state_hook::dispatch_state_event(rsvz_schedule::state_hook::StateEvent::BeforeScript);
            assert_eq!(try_fire(2, 9).ok(), None, "explicit empty remains empty after restart");
            assert!(cob::try_set_cobs([(6, 8)]).is_err());
            assert_eq!(
                try_fire(2, 9).ok(),
                None,
                "failed configuration does not erase explicit state"
            );
            cob::try_auto_set_cobs().unwrap();
            assert_eq!(
                try_fire(2, 9).ok(),
                Some(0),
                "explicit refresh intentionally replaces empty list"
            );
        })
    });
    reset();
}

#[test]
fn global_scan_refreshes_all_columns_and_preserves_explicit_managers() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            let first = rsvz::with_backend(|backend| add_cob(backend, Grid { row: 2, col: 0 }));
            let custom = CobManager::new();
            cob::set_cobs(&custom, [(3, 1)]);
            cob::set_cobs([(3, 1)]);
            cob::try_auto_set_cobs(CobListOrder::Vertical).unwrap();
            assert_eq!(cob::fire_1(2, 9), Some(0));
            rsvz::with_backend(|backend| {
                ready(backend, first);
                add_cob(backend, Grid { row: 0, col: 0 });
                add_cob(backend, Grid { row: 0, col: 7 });
            });
            assert_eq!(
                cob::fire_8(2, 9),
                None,
                "even unused columns were initialized by global scan"
            );
            cob::try_auto_set_cobs().unwrap();
            assert_eq!(cob::fire_1([(2, 9), (4, 9)]), [Some(0), Some(1)]);
            assert_eq!(cob::fire_8(2, 9), Some(0));
            let mut grids = Vec::new();
            custom.for_each_grid(|grid| grids.push(grid));
            assert_eq!(grids, [Grid { row: 2, col: 0 }]);
            grids.clear();
            cob::for_each_cob_grid(|grid| grids.push(grid));
            assert_eq!(grids.len(), 3);
            // Explicit manager scans must not refresh any built-in column.
            rsvz::with_backend(|backend| {
                add_cob(backend, Grid { row: 1, col: 7 });
            });
            cob::try_auto_set_cobs(&custom).unwrap();
            assert_eq!(cob::fire_8(2, 9), None);
        })
    });
    reset();
}

#[test]
fn uncorrected_roof_commands_keep_selection_and_reservations_without_roof_delay() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene: pe_rs::SceneType::Roof,
        ..PeWorldConfig::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    let ids = world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            rsvz_current::CurrentBackend::with_backend_shared(|backend| {
                let ids = std::array::from_fn::<_, 4, _>(|row| {
                    add_cob(
                        backend,
                        Grid {
                            row: row as i32,
                            col: 0,
                        },
                    )
                });
                cob::try_auto_set_cobs().unwrap();
                assert_eq!(cob::find_cob((2, 1)), Some(ids[1]));
                assert_eq!(cob::find_cob((0, 1)), None);
                assert_eq!(
                    rsvz::plant::find_plant_at_kind((2, 1), PlantKind::CobCannon),
                    Some(ids[1])
                );
                assert_eq!(cob::cob_recover_time(ids[1]), 0);

                // Ordinary roof fire reserves the first cannon until clock 28.
                assert_eq!(try_fire(2, 9.0).ok(), Some(0));
                let same = cob::default_manager();
                assert_eq!(same.next_index_raw(), 1);
                assert_eq!(
                    cob::try_fire_uncorrected(&same, [(2, 9.0), (0, 9.0)])
                        .unwrap_err()
                        .report
                        .indices,
                    [Some(1), None]
                );
                assert_eq!(backend.plant_state(backend.plant(ids[0]).unwrap().unwrap()), 37);
                assert_eq!(backend.plant_state(backend.plant(ids[1]).unwrap().unwrap()), 38);
                assert_eq!(same.next_index_raw(), 2);
                assert!(cob::cob_recover_time(ids[1]) > 0);

                // Raw batches reject a reserved cannon and still fire subsequent valid entries.
                assert!(cob::try_raw_fire_uncorrected([(1, 1, 2, 9.0), (3, 1, 2, 9.0)]).is_err());
                assert_eq!(backend.plant_state(backend.plant(ids[2]).unwrap().unwrap()), 38);
                assert_eq!(same.next_index_raw(), 2);

                // A separate list cannot steal the first manager's reserved cannon.
                let observer = CobManager::new();
                cob::try_set_cobs(&observer, [(1, 1)]).unwrap();
                assert_eq!(cob::try_fire_uncorrected(&observer, 2, 9.0).ok(), None);

                // Recovery waits for actual cooldown, without adding roof correction.
                let recovering = backend.plant(ids[3]).unwrap().unwrap();
                backend.set_plant_state(recovering, 35).unwrap();
                backend.set_plant_state_countdown(recovering, 0).unwrap();
                cob::try_set_cobs(&observer, [(4, 1)]).unwrap();
                assert_eq!(cob::cob_recover_time(ids[3]), 125);
                assert_eq!(
                    rsvz::core::cob::try_recover_fire_uncorrected_with_manager(&observer, 2, 9.0).ok(),
                    Some(0)
                );
                assert!(cob::try_raw_fire_uncorrected(4, 1, 2, 9.0).is_err());
                ids
            })
            .unwrap()
        })
    });
    for clock in 0..=125 {
        world.with_backend(|backend| {
            PeBackend::scope_backend(backend, || {
                rsvz_current::CurrentBackend::with_backend_shared(|backend| {
                    // The fixture uses a hand-set cooldown state. Become ready just before
                    // its deadline to isolate command timing from native animation progression.
                    if clock == 124 {
                        ready(backend, ids[3]);
                    }
                    assert_eq!(
                        rsvz_game::timeline::dispatch_timeline_tick(
                            WaveTimingSnapshot::minimal(clock, Wave(1)),
                            meta(clock)
                        ),
                        TimelineDispatchResult::Continue,
                        "clock={clock}"
                    );
                    if clock == 27 {
                        assert_eq!(backend.plant_state(backend.plant(ids[0]).unwrap().unwrap()), 37);
                    }
                    if clock == 28 {
                        assert_eq!(backend.plant_state(backend.plant(ids[0]).unwrap().unwrap()), 38);
                    }
                    if clock < 125 {
                        assert_ne!(backend.plant_state(backend.plant(ids[3]).unwrap().unwrap()), 38);
                    } else {
                        assert_eq!(backend.plant_state(backend.plant(ids[3]).unwrap().unwrap()), 38);
                    }
                })
                .unwrap()
            })
        });
        if clock < 125 {
            world.update_world().unwrap();
        }
    }
    reset();
}

#[test]
fn handled_cannon_errors_preserve_sources_and_only_reporting_marks_failure() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            let manager = CobManager::new();
            manager.set_list_unchecked([] as [Grid; 0]);
            let error = try_fire(&manager, 2, 9).unwrap_err();
            assert_eq!(error.code(), "empty_list");
            assert_eq!(error.target, Some(CobTarget { row: 1, drop_col: 9.0 }));
            assert_eq!(rsvz_game::session::take_dispatch_outcome(), None);
            let wrapped = rsvz::RuntimeError::from(error).context("outer context");
            assert_eq!(wrapped.find_source::<cob::CobError>().unwrap().code(), "empty_list");
            let registered = rsvz_game::registration::run_script(|| Err(wrapped.clone())).unwrap_err();
            assert_eq!(registered.find_source::<cob::CobError>().unwrap().code(), "empty_list");
            let seen = std::rc::Rc::new(std::cell::Cell::new(0));
            let count = seen.clone();
            let previous = rsvz::replace_logger(move |record: &rsvz::LogRecord<'_>| {
                let error = record.error.unwrap().downcast_ref::<rsvz::RuntimeError>().unwrap();
                assert_eq!(error.find_source::<cob::CobError>().unwrap().code(), "empty_list");
                count.set(count.get() + 1);
            });
            rsvz::diagnostics::report_operation_error(wrapped);
            rsvz::restore_logger(previous);
            assert_eq!(seen.get(), 1);
            assert_eq!(
                rsvz_game::session::take_dispatch_outcome(),
                Some(rsvz_game::session::DispatchOutcome::RecoverableError)
            );

            let report = try_fire(&manager, [(0, 9), (1, 9)]).unwrap_err().report;
            assert_eq!(report.total, Some(2));
            assert_eq!(report.processed, 2);
            assert_eq!(report.accepted, 0);
            assert!(report.complete);
            assert_eq!(report.indices, [None, None]);
            assert_eq!(report.failures[0].code(), "invalid_input");
            assert_eq!(report.failures[1].code(), "empty_list");
            assert_eq!(rsvz_game::session::take_dispatch_outcome(), None);
            let raw = cob::try_raw_fire([(0, 1, 1, 9), (1, 1, 1, 9)]).unwrap_err().report;
            assert_eq!(raw.processed, 2);
            assert!(raw.indices.is_empty());
            assert_eq!(raw.failures.len(), 2);
        })
    });
    reset();
}

#[test]
fn column_handle_survives_restart_and_global_refresh_in_place() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            let own = Grid { row: 0, col: 0 };
            let other = Grid { row: 1, col: 6 };
            PeBackend::with_backend_shared(|backend| {
                add_cob(backend, own);
                add_cob(backend, other);
            })
            .unwrap();
            let handle = cob::column_manager(1).unwrap();
            handle.set_list_unchecked([other]);
            handle.set_sequential_mode(CobSequentialMode::Space);
            rsvz_game::state_hook::clear_state_hooks();
            let _ = cob::default_manager();
            let again = cob::column_manager(1).unwrap();
            let mut grids = Vec::new();
            again.for_each_grid(|grid| grids.push(grid));
            assert_eq!(grids, [other]);
            assert_eq!(cob::fire_1(2, 9), Some(0));
            assert!(try_fire(&handle, 2, 9).is_err());
            cob::try_auto_set_cobs().unwrap();
            grids.clear();
            handle.for_each_grid(|grid| grids.push(grid));
            assert_eq!(grids, [own]);
            assert!(cob::column_manager(0).is_err());
            assert!(cob::column_manager(9).is_err());
        })
    });
    reset();
}

#[test]
fn unknown_batch_tail_is_not_converted_after_a_structural_stop() {
    struct Input;
    impl cob::IntoCobTargets for Input {
        fn try_for_each_cob_target_result<E>(
            self, visit: &mut impl FnMut(Result<CobTarget, cob::CobManagerError>) -> Result<(), E>,
        ) -> Result<(), E> {
            visit(Err(cob::CobManagerError::InvalidNext))?;
            panic!("the unprocessed tail must not be evaluated for counting");
        }
    }
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            let manager = CobManager::new();
            let report = cob::try_fire(&manager, Input).unwrap_err().report;
            assert_eq!(report.total, None);
            assert_eq!(report.processed, 1);
            assert_eq!(report.indices, [None]);
            assert_eq!(report.stopped_at, Some(0));
            assert!(!report.complete);
        })
    });
    reset();
}

#[test]
fn saved_automatic_column_handle_rediscovers_without_reacquiring_it() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            let first = rsvz::with_backend(|backend| add_cob(backend, Grid { row: 0, col: 0 }));
            rsvz::with_backend(|backend| add_cob(backend, Grid { row: 1, col: 6 }));
            let saved = cob::column_manager(1).unwrap();
            assert_eq!(try_fire(&saved, 2, 9).unwrap(), 0);
            rsvz::with_backend(|backend| ready(backend, first));
            let added = rsvz::with_backend(|backend| add_cob(backend, Grid { row: 2, col: 0 }));
            rsvz_game::state_hook::dispatch_state_event(rsvz_schedule::state_hook::StateEvent::BeforeScript);
            assert_eq!(try_fire(&saved, [(2, 9), (4, 9)]).unwrap().indices, [Some(0), Some(1)]);
            rsvz::with_backend(|backend| assert_eq!(backend.plant_state(backend.plant(added).unwrap().unwrap()), 38));
            assert_eq!(try_fire(&CobManager::new(), 2, 9).unwrap_err().code(), "empty_list");
        })
    });
    reset();
}

#[test]
fn single_target_overloads_keep_single_results_and_collection_results_stay_batches() {
    reset();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        PeBackend::scope_backend(backend, || {
            let id = rsvz::with_backend(|backend| add_cob(backend, Grid { row: 0, col: 0 }));
            cob::set_cobs([(1, 1)]);
            let manager = CobManager::new();
            cob::set_cobs(&manager, [(1, 1)]);
            let target = rsvz::grid::target(2, 9.0);
            assert_eq!(
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(0, Wave(1)), meta(0)),
                TimelineDispatchResult::Continue
            );
            let restore = || {
                rsvz::with_backend(|backend| {
                    assert_eq!(backend.plant_state(backend.plant(id).unwrap().unwrap()), 38);
                    ready(backend, id);
                })
            };
            macro_rules! check {
                ($fire:ident, $try_fire:ident) => {{
                    let result: Result<i32, cob::CobError> = cob::$try_fire((2, 9));
                    assert_eq!(result.unwrap(), 0);
                    restore();
                    let result: Result<i32, cob::CobError> = cob::$try_fire(target);
                    assert_eq!(result.unwrap(), 0);
                    restore();
                    let result: Result<i32, cob::CobError> = cob::$try_fire(&manager, (2, 9.0));
                    assert_eq!(result.unwrap(), 0);
                    restore();
                    let result: Result<i32, cob::CobError> = cob::$try_fire(&manager, target);
                    assert_eq!(result.unwrap(), 0);
                    restore();
                    let result: Option<i32> = cob::$fire((2, 9.0));
                    assert_eq!(result, Some(0));
                    restore();
                    let result: Option<i32> = cob::$fire(target);
                    assert_eq!(result, Some(0));
                    restore();
                    let result: Option<i32> = cob::$fire(&manager, (2, 9));
                    assert_eq!(result, Some(0));
                    restore();
                    let result: Option<i32> = cob::$fire(&manager, target);
                    assert_eq!(result, Some(0));
                    restore();
                    let batch: Result<cob::CobBatchReport, cob::CobBatchError> = cob::$try_fire([target]);
                    assert_eq!(batch.unwrap().indices, [Some(0)]);
                    restore();
                    let batch: Result<cob::CobBatchReport, cob::CobBatchError> = cob::$try_fire(&manager, vec![(2, 9)]);
                    assert_eq!(batch.unwrap().indices, [Some(0)]);
                    restore();
                    let batch: Vec<Option<i32>> = cob::$fire([(2, 9)].as_slice());
                    assert_eq!(batch, [Some(0)]);
                    restore();
                    let batch: Vec<Option<i32>> = cob::$fire(&manager, [target]);
                    assert_eq!(batch, [Some(0)]);
                    restore();
                }};
            }
            check!(fire, try_fire);
            check!(recover_fire, try_recover_fire);
            check!(fire_uncorrected, try_fire_uncorrected);
            check!(recover_fire_uncorrected, try_recover_fire_uncorrected);

            // Invalid input retains its original coordinates and typed reason; it is not pre-converted to f32.
            let tuple = cob::try_fire_1((0, i32::MAX)).unwrap_err();
            let separate = cob::try_fire_1(0, i32::MAX).unwrap_err();
            assert_eq!(tuple.cause, separate.cause);
            assert_eq!(tuple.input_number, 1);
            let invalid = CobTarget {
                row: -1,
                drop_col: f32::NAN,
            };
            let single = cob::try_fire_1(invalid).unwrap_err();
            let batch = cob::try_fire_1([invalid]).unwrap_err();
            assert_eq!(single.cause, batch.report.failures[0].cause);
            assert_eq!(batch.report.indices, [None]);
            assert_eq!(rsvz_game::session::take_dispatch_outcome(), None);
        });
    });
    reset();
}

#[test]
fn tuple_and_typed_targets_preserve_roof_correction_and_recovery_waits() {
    use rsvz::grid::target;
    type Shot = fn(&CobManager) -> Result<i32, cob::CobError>;
    let cases: [(Shot, bool, i32); 8] = [
        (|m| cob::try_fire(m, (2, 9)), false, 28),
        (|m| cob::try_fire(m, target(2, 9.0)), false, 28),
        (|m| cob::try_fire_uncorrected(m, (2, 9)), false, 0),
        (|m| cob::try_fire_uncorrected(m, target(2, 9.0)), false, 0),
        (|m| cob::try_recover_fire(m, (2, 9)), true, 125),
        (|m| cob::try_recover_fire(m, target(2, 9.0)), true, 125),
        (|m| cob::try_recover_fire_uncorrected(m, (2, 9)), true, 125),
        (|m| cob::try_recover_fire_uncorrected(m, target(2, 9.0)), true, 125),
    ];
    for (shot, cooling, due) in cases {
        reset();
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Roof,
            ..PeWorldConfig::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        world.with_backend(|backend| {
            PeBackend::scope_backend(backend, || {
                let id = rsvz::with_backend(|backend| {
                    let id = add_cob(backend, Grid { row: 0, col: 0 });
                    if cooling {
                        let plant = backend.plant(id).unwrap().unwrap();
                        backend.set_plant_state(plant, 35).unwrap();
                        backend.set_plant_state_countdown(plant, 0).unwrap();
                    }
                    id
                });
                let manager = CobManager::new();
                cob::try_set_cobs(&manager, [(1, 1)]).unwrap();
                rsvz_game::timeline::dispatch_timeline_tick(WaveTimingSnapshot::minimal(0, Wave(1)), meta(0));
                assert_eq!(shot(&manager).unwrap(), 0);
                if due > 0 {
                    rsvz_game::timeline::dispatch_timeline_tick(
                        WaveTimingSnapshot::minimal(due - 1, Wave(1)),
                        meta(due - 1),
                    );
                    // Use a controlled cooldown fixture; only the scheduled deadline is under test.
                    rsvz::with_backend(|backend| {
                        assert_ne!(backend.plant_state(backend.plant(id).unwrap().unwrap()), 38);
                        ready(backend, id);
                    });
                    assert_eq!(
                        rsvz_game::timeline::dispatch_timeline_tick(
                            WaveTimingSnapshot::minimal(due, Wave(1)),
                            meta(due)
                        ),
                        TimelineDispatchResult::Continue
                    );
                }
                rsvz::with_backend(|backend| assert_eq!(backend.plant_state(backend.plant(id).unwrap().unwrap()), 38));
            })
        });
    }
    reset();
}
