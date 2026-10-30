#![cfg(feature = "pvz-emulator")]

use rsvz::cards::{CardErrorKind, card, try_card};
use rsvz_backend_api::{
    BattleEntryBackend, CardAppendSelectionBackend, PlantReadBackend, SeedBankReadBackend, SeedRuleEditBackend,
    SunCostRuleEditBackend,
};
use rsvz_model::{CardSelection, Grid, PlantId, PlantKind, SeedSlot};
use rsvz_pvz_emulator_backend::{PeWorldConfig, runner_internal::PeWorldOwner};

fn grid(id: PlantId) -> Grid {
    rsvz::Plant::from_id(id).grid().expect_live("planted plant")
}

#[test]
fn current_card_adapters_preserve_slots_candidates_and_batch_results() {
    rsvz::reset_runtime_state_preserving_backend();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            rsvz::with_backend(|backend| {
                for kind in [
                    PlantKind::Sunflower,
                    PlantKind::Peashooter,
                    PlantKind::LilyPad,
                    PlantKind::DoomShroom,
                ] {
                    backend
                        .select_card(CardSelection::Plant(kind).checked().unwrap())
                        .unwrap();
                }
                backend.start_battle().unwrap();
                for index in [rsvz_model::MAX_SEED_SLOTS, usize::MAX] {
                    assert!(backend.seed(SeedSlot::from_index_unchecked(index)).unwrap().is_none());
                }
                for index in 0..rsvz_model::MAX_SEED_SLOTS {
                    let slot = SeedSlot::new(index).unwrap();
                    let direct = backend
                        .seed(slot)
                        .unwrap()
                        .map(|seed| backend.seed_selection(seed).unwrap());
                    let scanned = backend
                        .seeds()
                        .unwrap()
                        .find(|seed| backend.seed_slot(*seed) == slot)
                        .map(|seed| backend.seed_selection(seed).unwrap());
                    assert_eq!(direct, scanned);
                }
                backend.set_seed_recharge_ignored(true).unwrap();
            });
            rsvz::cards::set_sun_cost_ignored(true);
            assert!(rsvz::with_backend(|backend| backend.sun_cost_ignored()).unwrap());
            rsvz::cards::set_sun_cost_ignored(false);
            assert!(!rsvz::with_backend(|backend| backend.sun_cost_ignored()).unwrap());
            rsvz::cards::set_sun_cost_ignored(true);
            assert_eq!(
                grid(try_card(PlantKind::Sunflower, 1, 1).unwrap()),
                Grid { row: 0, col: 0 }
            );
            assert_eq!(
                grid(card(CardSelection::Plant(PlantKind::Peashooter), 1, 2).unwrap()),
                Grid { row: 0, col: 1 }
            );
            assert_eq!(
                grid(try_card(SeedSlot::from_index_unchecked(0), 1, 3).unwrap()),
                Grid { row: 0, col: 2 }
            );

            let water = try_card([PlantKind::LilyPad, PlantKind::DoomShroom], 3, 4).unwrap();
            assert!(water.plants.iter().all(Option::is_some));
            assert_eq!(
                grid(try_card(PlantKind::Sunflower, [(1, 1), (1, 4)]).unwrap()),
                Grid { row: 0, col: 3 }
            );
            let candidates = try_card([PlantKind::Sunflower, PlantKind::Peashooter], [(2, 1), (2, 2)]).unwrap();
            assert_eq!(grid(candidates.plants[0].unwrap()), Grid { row: 1, col: 0 });
            assert_eq!(grid(candidates.plants[1].unwrap()), Grid { row: 1, col: 1 });
            let operations = card([(PlantKind::Sunflower, 2, 3), (PlantKind::Peashooter, 2, 4)]);
            assert_eq!(operations.len(), 2);
            assert!(operations.iter().all(Option::is_some));

            assert!(matches!(
                try_card(PlantKind::Pumpkin, 2, 5).unwrap_err().kind(),
                CardErrorKind::NotSelected { .. }
            ));
            let failure = try_card([(PlantKind::Pumpkin, 2, 5), (PlantKind::Peashooter, 2, 6)]).unwrap_err();
            assert_eq!(failure.report.failures[0].input_number, 1);
            assert!(matches!(
                failure.report.failures[0].error.kind(),
                CardErrorKind::NotSelected { .. }
            ));
            let partial = failure.report.plants;
            assert!(partial[0].is_none());
            assert_eq!(grid(partial[1].unwrap()), Grid { row: 1, col: 5 });
            rsvz::__run_script(|| {
                rsvz::at(1, 0, || try_card(PlantKind::Sunflower, 2, 7).map_err(Into::into));
                Ok(())
            })
            .unwrap();
        })
    });
    rsvz::reset_runtime_state_preserving_backend();
}

#[test]
fn canonical_resource_functions_share_access_and_apply_native_policy() {
    use rsvz_backend_api::{BoardStateBackend, DropRuleEditBackend, SunQueryBackend};
    use rsvz_game::modifier::{set_sun, stabilize_natural_sun_drop, stabilize_natural_sun_drop_aging};
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            let set: fn(u32) -> rsvz::RuntimeResult<()> = set_sun;
            let sample: fn() -> rsvz::RuntimeResult<rsvz_game::runtime::RuntimeFrameFacts> =
                rsvz_game::runtime::runtime_frame_facts;
            // A legal token can perform the first Board query directly.
            rsvz_pvz_emulator_backend::PeBackend::with_backend_shared(|backend| {
                backend.natural_sun_generated().unwrap()
            })
            .unwrap();
            rsvz_pvz_emulator_backend::PeBackend::with_backend_shared(|access| {
                let backend = access;
                set(1234).unwrap();
                assert_eq!(backend.sun().unwrap(), 1234);
                stabilize_natural_sun_drop_aging();
                assert_eq!(backend.natural_sun_generated().unwrap(), 52);
                stabilize_natural_sun_drop();
                assert!(backend.natural_sun_drop_disabled().unwrap());
                assert_eq!(backend.natural_sun_countdown().unwrap(), 425);
                sample().unwrap();
            })
            .unwrap();
            // A live exclusive physical borrow still excludes ordinary operations.
            let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rsvz::with_backend(|_| {
                    let _ = set(100);
                    panic!("a conflicting access must not return a recoverable result");
                });
            }))
            .unwrap_err();
            let error = rsvz::__private::into_callback_error(payload).expect("callback-local access failure");
            assert!(error.to_string().contains("active borrow"));
            set(100).unwrap();
        });
    });
}

fn with_selected_cards(
    scene: rsvz_pvz_emulator_backend::pe_rs::SceneType, selections: &[CardSelection], body: impl FnOnce(),
) {
    rsvz::reset_runtime_state_preserving_backend();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene,
        ..PeWorldConfig::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    world.with_backend(|backend| {
        rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, || {
            rsvz::with_backend(|backend| {
                for selection in selections {
                    backend.select_card(selection.checked().unwrap()).unwrap();
                }
                backend.start_battle().unwrap();
            });
            body();
        })
    });
    rsvz::reset_runtime_state_preserving_backend();
}

#[test]
fn kind_lookup_keeps_the_first_duplicate_and_can_try_a_different_candidate() {
    use rsvz_game::logic::cards::{find_seed_slot, find_usable_seed, plant_first_available, seed_slot_usable};
    use rsvz_pvz_emulator_backend::pe_rs::SceneType;

    let sunflower = CardSelection::Plant(PlantKind::Sunflower);
    let peashooter = CardSelection::Plant(PlantKind::Peashooter);
    let first = SeedSlot::new(0).unwrap();
    let duplicate = SeedSlot::new(1).unwrap();
    let different = SeedSlot::new(2).unwrap();
    with_selected_cards(SceneType::Day, &[sunflower, sunflower, peashooter], || {
        rsvz_game::modifier::set_sun(1000).unwrap();
        assert_eq!(find_seed_slot(sunflower), Some(first));
        assert_eq!(find_usable_seed([sunflower, peashooter]), Some(first));
        assert_eq!(find_usable_seed([peashooter, sunflower]), Some(different));
        try_card(sunflower, 1, 1).unwrap();
        assert!(!seed_slot_usable(first));
        assert!(seed_slot_usable(duplicate));

        assert_eq!(find_seed_slot(sunflower), Some(first));
        assert_eq!(find_usable_seed([sunflower]), None);
        assert!(matches!(
            try_card(sunflower, 1, 2).unwrap_err().kind(),
            CardErrorKind::Cooldown { .. }
        ));
        assert_eq!(find_usable_seed([sunflower, peashooter]), Some(different));
        assert_eq!(
            plant_first_available([sunflower, peashooter], Grid { row: 0, col: 1 }).unwrap(),
            Some(different)
        );
        assert_eq!(grid(try_card(duplicate, 1, 3).unwrap()), Grid { row: 0, col: 2 });
    });
}

#[test]
fn auto_container_commit_is_kept_when_the_main_card_becomes_unaffordable() {
    use rsvz_pvz_emulator_backend::pe_rs::SceneType;
    with_selected_cards(
        SceneType::Roof,
        &[
            CardSelection::Plant(PlantKind::FlowerPot),
            CardSelection::Plant(PlantKind::Sunflower),
        ],
        || {
            rsvz_game::modifier::set_sun(50).unwrap();
            let first = Grid { row: 0, col: 0 };
            let second = Grid { row: 0, col: 1 };
            let error = rsvz_game::logic::cards::card_any_prepared(
                CardSelection::Plant(PlantKind::Sunflower),
                &[first, second],
            )
            .unwrap_err();
            assert!(matches!(error.kind(), CardErrorKind::NotEnoughSun { .. }), "{error:?}");
            let plants = rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                backend
                    .plants()
                    .unwrap()
                    .map(|plant| {
                        (
                            backend.plant_kind(plant).unwrap(),
                            Grid {
                                row: backend.plant_row(plant),
                                col: backend.plant_col(plant),
                            },
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap();
            assert_eq!(plants, [(PlantKind::FlowerPot, first)]);
            use rsvz_backend_api::SunQueryBackend;
            assert_eq!(
                rsvz_current::CurrentBackend::with_backend_shared(|access| {
                    let backend = access;
                    backend.sun().unwrap()
                })
                .unwrap(),
                25
            );
        },
    );
}

#[test]
fn recorder_runs_after_creation_and_before_seed_post_processing_even_when_the_callback_aborts() {
    use rsvz_game::logic::cards::PlantingComponent;
    use rsvz_model::{GameUi, Wave, WaveTimingSnapshot};
    use rsvz_pvz_emulator_backend::pe_rs::SceneType;
    use rsvz_schedule::{TickMeta, TickPhase, TimelineDispatchResult};
    use std::{cell::RefCell, rc::Rc};
    with_selected_cards(SceneType::Day, &[CardSelection::Plant(PlantKind::Sunflower)], || {
        rsvz_game::modifier::set_sun(100).unwrap();
        let recorded = Rc::new(RefCell::new(Vec::new()));
        let saved = Rc::clone(&recorded);
        rsvz::__run_script(|| {
            rsvz::at(1, 0, move || {
                rsvz_game::logic::cards::card_recording(
                    CardSelection::Plant(PlantKind::Sunflower),
                    1,
                    1,
                    |component, id| {
                        saved.borrow_mut().push((component, id));
                        rsvz_game::diagnostics::abort_operation(rsvz::RuntimeError::new("recorded plant"));
                    },
                )
                .map_err(rsvz::RuntimeError::from)
            });
            Ok(())
        })
        .unwrap();
        let mut errors = Vec::new();
        assert_eq!(
            rsvz_game::timeline::dispatch_timeline_tick_reporting(
                WaveTimingSnapshot::minimal(0, Wave(1)),
                TickMeta {
                    phase: TickPhase::Playing,
                    game_ui: Some(GameUi::Playing),
                    clock: Some(0),
                    is_new_frame: true
                },
                &mut |error| errors.push(error.to_string()),
            ),
            TimelineDispatchResult::Continue
        );
        assert_eq!(errors, ["recorded plant"]);
        let records = recorded.borrow();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, PlantingComponent::Main);
        assert_eq!(grid(records[0].1), Grid { row: 0, col: 0 });
        assert_eq!(
            rsvz_game::logic::cards::card_cd(CardSelection::Plant(PlantKind::Sunflower)),
            Some(0)
        );
    });
}

#[test]
fn gloom_upgrade_restores_the_base_sleep_state_and_pending_coffee_counter() {
    use rsvz_backend_api::PlantSleepBackend;
    use rsvz_pvz_emulator_backend::pe_rs::SceneType;
    for asleep in [false, true] {
        with_selected_cards(SceneType::Day, &[CardSelection::Plant(PlantKind::GloomShroom)], || {
            rsvz_game::modifier::set_sun(1000).unwrap();
            let at = Grid { row: 0, col: 0 };
            let base = rsvz_game::logic::cards::new_plant(PlantKind::FumeShroom, at).unwrap();
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                let plant = backend.plant(base).unwrap().unwrap();
                backend.set_plant_sleeping(plant, asleep).unwrap();
                if asleep {
                    backend.set_plant_wake_up_counter(plant, 37).unwrap();
                }
            })
            .unwrap();
            let upgraded = try_card(PlantKind::GloomShroom, 1, 1).unwrap();
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                assert!(backend.plant(base).unwrap().is_none());
                let plant = backend.plant(upgraded).unwrap().unwrap();
                assert_eq!(backend.plant_is_sleeping(plant), asleep);
                if asleep {
                    assert_eq!(backend.plant_wake_up_counter(plant), 37);
                }
            })
            .unwrap();
        });
    }
}

#[test]
fn shovel_preserves_exact_lineage_and_restores_lily_under_a_protected_cattail() {
    use rsvz_pvz_emulator_backend::pe_rs::SceneType;
    with_selected_cards(SceneType::Day, &[], || {
        let normal = rsvz_game::logic::cards::new_plant(PlantKind::IceShroom, Grid { row: 0, col: 0 }).unwrap();
        let imitator = rsvz_game::logic::cards::new_plant_from_selection(
            CardSelection::Imitator(PlantKind::IceShroom),
            Grid { row: 0, col: 1 },
        )
        .unwrap();
        rsvz::shovel::try_shovel(1, 1, CardSelection::Imitator(PlantKind::IceShroom)).unwrap();
        rsvz::shovel::try_shovel(1, 2, PlantKind::IceShroom).unwrap();
        assert!(
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                backend.plant(normal).unwrap().is_some() && backend.plant(imitator).unwrap().is_some()
            })
            .unwrap()
        );
        rsvz::shovel::try_shovel(1, 2, CardSelection::Imitator(PlantKind::IceShroom)).unwrap();
        assert!(
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                backend.plant(normal).unwrap().is_some() && backend.plant(imitator).unwrap().is_none()
            })
            .unwrap()
        );
    });
    with_selected_cards(
        SceneType::Pool,
        &[
            CardSelection::Plant(PlantKind::LilyPad),
            CardSelection::Plant(PlantKind::Cattail),
            CardSelection::Plant(PlantKind::Pumpkin),
        ],
        || {
            rsvz_game::modifier::set_sun(1000).unwrap();
            try_card(PlantKind::LilyPad, 3, 1).unwrap();
            let cattail = try_card(PlantKind::Cattail, 3, 1).unwrap();
            let pumpkin = try_card(PlantKind::Pumpkin, 3, 1).unwrap();
            rsvz::shovel::try_shovel(3, 1, PlantKind::Cattail).unwrap();
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;

                assert!(backend.plant(cattail).unwrap().is_none());
                assert!(backend.plant(pumpkin).unwrap().is_some());
                assert_eq!(
                    backend
                        .plants()
                        .unwrap()
                        .filter(|plant| backend.plant_kind(*plant).unwrap() == PlantKind::LilyPad)
                        .count(),
                    1
                );
            })
            .unwrap();
        },
    );
}

#[test]
fn shovel_skips_airborne_squashes_but_removes_a_grounded_squash() {
    use rsvz_backend_api::PlantStateWriteBackend;
    use rsvz_pvz_emulator_backend::pe_rs::SceneType;
    with_selected_cards(SceneType::Day, &[], || {
        for (col, state) in [5, 6, 7].into_iter().enumerate() {
            let at = Grid {
                row: 0,
                col: col as i32,
            };
            let id = rsvz_game::logic::cards::new_plant(PlantKind::Squash, at).unwrap();
            rsvz::__private::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                let plant = backend.plant(id).unwrap().unwrap();
                backend.set_plant_state(plant, state).unwrap();
                rsvz_game::logic::shovel::shovel_at(at).unwrap();
                rsvz::shovel::try_shovel(1, col as i32 + 1, PlantKind::Squash).unwrap();
                // The crushed state is excluded by live-ID lookup; restoring only the state
                // verifies that neither shovel path marked the held object dead.
                backend.set_plant_state(plant, 4).unwrap();
                assert!(backend.plant(id).unwrap().is_some());
                rsvz::shovel::try_shovel(1, col as i32 + 1, PlantKind::Squash).unwrap();
                assert!(backend.plant(id).unwrap().is_none());
            })
            .unwrap();
        }
    });
}

#[test]
fn cooldown_rejection_keeps_native_remaining_time_and_target_diagnostics() {
    use rsvz_pvz_emulator_backend::pe_rs::SceneType;
    with_selected_cards(SceneType::Day, &[CardSelection::Plant(PlantKind::Sunflower)], || {
        rsvz_game::modifier::set_sun(1000).unwrap();
        try_card(PlantKind::Sunflower, 1, 1).unwrap();
        let remaining = rsvz_game::logic::cards::card_cd(CardSelection::Plant(PlantKind::Sunflower)).unwrap();
        assert!(remaining > 0);
        let error = try_card(PlantKind::Sunflower, 1, 2).unwrap_err();
        assert!(matches!(error.kind(), CardErrorKind::Cooldown { remaining: value, .. } if value == remaining));
        assert!(error.message().contains("(1, 2)"));
        assert!(error.message().contains("向日葵"));
        assert_eq!(
            rsvz_current::CurrentBackend::with_backend_shared(|access| {
                let backend = access;
                backend.plants().unwrap().count()
            })
            .unwrap(),
            1
        );
    });
}

#[test]
fn shovel_uses_pool_order_for_overlapping_plants_and_cob_right_half() {
    use rsvz_backend_api::PlantCreateBackend;
    use rsvz_pvz_emulator_backend::pe_rs::SceneType;
    with_selected_cards(SceneType::Day, &[], || {
        let at = Grid { row: 0, col: 0 };
        let create = |kind, at| {
            rsvz::with_backend(|backend| {
                let plant = backend
                    .new_plant(CardSelection::Plant(kind).checked().unwrap(), at)
                    .unwrap();
                backend.plant_id(plant)
            })
        };
        let first = create(PlantKind::Peashooter, at);
        let second = create(PlantKind::Sunflower, at);
        rsvz_game::logic::shovel::shovel_at(at).unwrap();
        rsvz::with_backend(|backend| {
            assert!(backend.plant(first).unwrap().is_none());
            assert!(backend.plant(second).unwrap().is_some());
        });
        let cob = create(PlantKind::CobCannon, Grid { row: 1, col: 2 });
        rsvz_game::logic::shovel::shovel_at(Grid { row: 1, col: 3 }).unwrap();
        rsvz::with_backend(|backend| assert!(backend.plant(cob).unwrap().is_none()));
        rsvz_game::logic::shovel::shovel_at(Grid { row: 0, col: 9 }).unwrap();
    });
}

#[test]
fn immediate_aliases_select_normal_and_imitator_cards_without_changing_their_state() {
    use rsvz::dsl::{ice, m_ice};
    use rsvz_pvz_emulator_backend::pe_rs::SceneType;
    with_selected_cards(SceneType::Pool, &[ice.selection(), m_ice.selection()], || {
        rsvz::cards::set_sun_cost_ignored(true);
        let normal = card(ice, 1, 1).unwrap();
        // Candidate fallback exercises the same alias adapter as direct coordinates.
        let imitated = try_card(m_ice, [(1, 1), (1, 2)]).unwrap();
        assert_eq!(grid(imitated), Grid { row: 0, col: 1 });
        rsvz::with_backend(|backend| {
            assert_eq!(
                backend.plant_raw_kind(backend.plant(normal).unwrap().unwrap()).unwrap(),
                PlantKind::IceShroom
            );
            assert_eq!(
                backend
                    .plant_raw_kind(backend.plant(imitated).unwrap().unwrap())
                    .unwrap(),
                PlantKind::Imitator
            );
        });
    });
}

#[test]
fn domain_errors_keep_sources_and_batches_keep_all_rejections() {
    use rsvz::cards::{CardBatchError, CardError};
    use rsvz::ice_filler::IceFillerError;
    use rsvz::plant_fixer::PlantFixerError;
    use rsvz::shovel::{ShovelBatchError, ShovelError};
    with_selected_cards(rsvz_pvz_emulator_backend::pe_rs::SceneType::Day, &[], || {
        let error: rsvz::RuntimeError = try_card(PlantKind::IceShroom, 1, 1).unwrap_err().into();
        assert_eq!(error.find_source::<CardError>().unwrap().code(), "not_selected");
        assert_eq!(
            try_card(PlantKind::Imitator, 1, 1).unwrap_err().kind(),
            CardErrorKind::InvalidSelection
        );
        let slot = SeedSlot::from_index_unchecked(rsvz_model::MAX_SEED_SLOTS);
        assert_eq!(try_card(slot, 0, 1).unwrap_err().coordinates, Some((0, 1)));
        assert_eq!(try_card(slot, 1, 1).unwrap_err().coordinates, Some((1, 1)));
        let error = rsvz::ice_filler::start([(0, 1)]).unwrap_err();
        assert!(error.find_source::<IceFillerError>().is_some());
        let error = rsvz::plant_fixer::set_run_interval(0).unwrap_err();
        assert!(error.find_source::<PlantFixerError>().is_some());
        let error = rsvz::shovel::try_shovel(0, 1).unwrap_err();
        assert!(error.find_source::<ShovelError>().is_some());
        let error = rsvz::shovel::try_shovel([(0, 1), (1, 1), (1, 0)]).unwrap_err();
        let batch = error.find_source::<ShovelBatchError>().unwrap();
        assert_eq!(
            batch.failures.iter().map(|f| f.input_number).collect::<Vec<_>>(),
            [1, 3]
        );
        let error = try_card([PlantKind::IceShroom, PlantKind::CherryBomb], [(0, 1)]).unwrap_err();
        assert_eq!(error.report.plants, [None, None]);
        assert!(
            error
                .report
                .failures
                .iter()
                .all(|f| f.error.kind() == CardErrorKind::InvalidGrid)
        );
        let error: rsvz::RuntimeError = error.into();
        assert_eq!(error.find_source::<CardBatchError>().unwrap().report.failures.len(), 2);
        assert_eq!(rsvz_game::session::take_dispatch_outcome(), None);
        assert_eq!(rsvz::ice_filler::coffee().unwrap(), None);
    });
}

#[test]
fn ordinary_card_batch_reports_detailed_errors_once_and_continues() {
    let reports = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    with_selected_cards(rsvz_pvz_emulator_backend::pe_rs::SceneType::Day, &[], || {
        let output = reports.clone();
        let logger = rsvz::replace_logger(move |record: &rsvz::LogRecord<'_>| {
            output.borrow_mut().push(record.message.to_owned());
        });
        assert_eq!(card([PlantKind::IceShroom, PlantKind::Pumpkin], 1, 1), [None, None]);
        assert_eq!(reports.borrow().len(), 1);
        assert!(reports.borrow()[0].contains("寒冰菇"));
        assert!(reports.borrow()[0].contains("南瓜头"));
        assert!(!rsvz_game::session::script_stop_requested());
        rsvz::restore_logger(logger);
    });
}

#[test]
fn manager_reset_stops_old_tasks_and_failed_lists_preserve_configuration() {
    use rsvz::ice_filler::IceFiller;
    use rsvz::plant_fixer::PlantFixer;
    use rsvz_schedule::{TickTaskState, tick::with_scheduler};
    let cards = [PlantKind::IceShroom, PlantKind::CoffeeBean, PlantKind::Sunflower].map(CardSelection::Plant);
    with_selected_cards(rsvz_pvz_emulator_backend::pe_rs::SceneType::Day, &cards, || {
        rsvz::ice_filler::start([(1, 1)]).unwrap();
        let ice_task = rsvz_game::ice_filler::with_ice_filler(|r| r.task.unwrap());
        rsvz::ice_filler::reset();
        assert_eq!(with_scheduler(|s| s.state(ice_task)), TickTaskState::Stopped);
        rsvz::plant_fixer::start_plant_fixer(PlantKind::Sunflower, [(1, 1)]).unwrap();
        let fixer_task = rsvz_game::plant_fixer::with_plant_fixer(|r| r.task.unwrap());
        rsvz::plant_fixer::reset();
        assert_eq!(with_scheduler(|s| s.state(fixer_task)), TickTaskState::Stopped);
        let mut ice = IceFiller::new();
        assert!(ice.set_list([(0, 1)]).is_err());
        assert!(ice.set_temp_positions([Grid { row: -1, col: 0 }]).is_err());
        assert!(matches!(
            ice.coffee_at(Grid { row: -1, col: 0 }),
            Err(rsvz::ice_filler::IceFillerError::InvalidGrid)
        ));
        let mut fixer = PlantFixer::new();
        fixer.set_list([(1, 1)]).unwrap();
        assert!(fixer.move_to_list_top([(2, 1), (0, 1)]).is_err());
        assert_eq!(fixer.list(), &[Grid { row: 0, col: 0 }]);
    });
}

#[test]
fn seed_outcomes_keep_normal_failure_separate_from_required_access_abort() {
    use rsvz_game::logic::cards::{PlantSeedOutcome, try_plant_seed};
    with_selected_cards(
        rsvz_pvz_emulator_backend::pe_rs::SceneType::Day,
        &[CardSelection::Plant(PlantKind::Sunflower)],
        || {
            let slot = SeedSlot::new(0).unwrap();
            let target = Grid { row: 0, col: 0 };
            rsvz_game::modifier::set_sun(1000).unwrap();
            assert!(matches!(
                try_plant_seed(slot, target).unwrap(),
                PlantSeedOutcome::Planted(_)
            ));
            assert_eq!(try_plant_seed(slot, target).unwrap(), PlantSeedOutcome::Unusable);
            rsvz::with_backend(|backend| backend.set_seed_recharge_ignored(true).unwrap());
            assert!(matches!(
                try_plant_seed(slot, target).unwrap(),
                PlantSeedOutcome::Rejected(_)
            ));
            let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rsvz::with_backend(|_| {
                    let _ = try_plant_seed(slot, target);
                });
            }))
            .expect_err("required access still aborts instead of returning Err");
            assert!(rsvz::__private::into_callback_error(payload).is_ok());
            assert!(!rsvz_game::session::script_stop_requested());
            assert_eq!(rsvz_game::session::take_dispatch_outcome(), None);
        },
    );
}

#[test]
fn plant_fixer_replaces_only_its_target_and_keeps_other_layers() {
    use rsvz_game::logic::cards::new_plant;
    with_selected_cards(
        rsvz_pvz_emulator_backend::pe_rs::SceneType::Day,
        &[CardSelection::Plant(PlantKind::Sunflower)],
        || {
            rsvz_game::modifier::set_sun(1000).unwrap();
            let at = Grid { row: 0, col: 0 };
            let old = new_plant(PlantKind::Sunflower, at).unwrap();
            let cover = new_plant(PlantKind::Pumpkin, at).unwrap();
            rsvz_game::modifier::set_plant_hp(old, 10).unwrap();
            let mut fixer = rsvz::plant_fixer::PlantFixer::new();
            fixer.start(PlantKind::Sunflower, [(1, 1)]).unwrap();
            fixer.tick();
            rsvz::with_backend(|backend| {
                assert!(backend.plant(old).unwrap().is_none());
                assert!(backend.plant(cover).unwrap().is_some());
                let repaired = backend
                    .plants()
                    .unwrap()
                    .filter(|plant| backend.plant_kind(*plant).unwrap() == PlantKind::Sunflower)
                    .map(|plant| backend.plant_hp(plant))
                    .collect::<Vec<_>>();
                assert_eq!(repaired, [300]);
            });
            assert_eq!(rsvz_game::session::take_dispatch_outcome(), None);
        },
    );
}

#[test]
fn manager_reset_and_restart_inside_a_tick_do_not_revive_old_tasks() {
    use rsvz_schedule::{TickControl, TickMeta, TickOptions, TickPhase, TickTaskState, tick::with_scheduler};
    let cards = [PlantKind::IceShroom, PlantKind::CoffeeBean, PlantKind::Sunflower].map(CardSelection::Plant);
    with_selected_cards(rsvz_pvz_emulator_backend::pe_rs::SceneType::Day, &cards, || {
        rsvz::ice_filler::start([] as [(i32, i32); 0]).unwrap();
        rsvz::plant_fixer::start_plant_fixer(PlantKind::Sunflower, [] as [(i32, i32); 0]).unwrap();
        let old_ice = rsvz_game::ice_filler::with_ice_filler(|r| r.task.unwrap());
        let old_fixer = rsvz_game::plant_fixer::with_plant_fixer(|r| r.task.unwrap());
        let reset_task = rsvz_game::tick::spawn(TickOptions::playing_frame(), |_| {
            rsvz::ice_filler::reset();
            rsvz::plant_fixer::reset();
            rsvz::ice_filler::start([] as [(i32, i32); 0])?;
            rsvz::plant_fixer::start_plant_fixer(PlantKind::Sunflower, [] as [(i32, i32); 0])?;
            Ok(TickControl::Stop)
        });
        assert_eq!(
            rsvz_game::tick::dispatch_scheduler_tick(TickMeta {
                clock: Some(0),
                phase: TickPhase::Playing,
                game_ui: Some(rsvz_model::GameUi::Playing),
                is_new_frame: true,
            }),
            rsvz_schedule::TickDispatchResult::Continue
        );
        let new_ice = rsvz_game::ice_filler::with_ice_filler(|r| r.task.unwrap());
        let new_fixer = rsvz_game::plant_fixer::with_plant_fixer(|r| r.task.unwrap());
        with_scheduler(|scheduler| {
            for handle in [old_ice, old_fixer, reset_task] {
                assert_eq!(scheduler.state(handle), TickTaskState::Stopped);
            }
            for handle in [new_ice, new_fixer] {
                assert_ne!(scheduler.state(handle), TickTaskState::Stopped);
            }
        });
    });
}

#[test]
fn shovel_required_access_failure_is_a_local_abort_not_a_returned_error() {
    with_selected_cards(rsvz_pvz_emulator_backend::pe_rs::SceneType::Day, &[], || {
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rsvz::with_backend(|_| {
                let _ = rsvz::shovel::try_shovel(1, 1);
                panic!("required access failure must not return");
            });
        }))
        .unwrap_err();
        assert!(rsvz::__private::into_callback_error(payload).is_ok());
        assert!(!rsvz_game::session::script_stop_requested());
        rsvz::shovel::try_shovel(1, 1).unwrap();
    });
}
