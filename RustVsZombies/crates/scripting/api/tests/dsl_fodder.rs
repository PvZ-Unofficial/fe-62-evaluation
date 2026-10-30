#![cfg(feature = "pvz-emulator")]

use rsvz::dsl::{Fodder, FodderConstraint, act, c, pot, scaredy, sunflower};
use rsvz_backend_api::{
    BattleEntryBackend, CardAppendSelectionBackend, ClockBackend, CurrentWaveBackend, PlantReadBackend,
    SunCostRuleEditBackend, ZombieCreateBackend, ZombieRuleEditBackend,
};
use rsvz_model::{CardSelection, GameUi, Grid, PlantKind, Wave, WaveTimingSnapshot, ZombieKind};
use rsvz_pvz_emulator_backend::{
    PeWorldConfig, pe_rs,
    runner_internal::{PeWorldOwner, PeWorldRun},
};
use rsvz_schedule::tick::{TickMeta, TickPhase};

fn scoped<T>(world: &mut PeWorldRun, f: impl FnOnce() -> T) -> T {
    world.with_backend(|backend| rsvz_pvz_emulator_backend::PeBackend::scope_backend(backend, f))
}

fn world(cards: &[CardSelection]) -> PeWorldRun {
    world_in(pe_rs::SceneType::Day, cards)
}

fn world_in(scene: pe_rs::SceneType, cards: &[CardSelection]) -> PeWorldRun {
    rsvz::reset_runtime_state_preserving_backend();
    rsvz_game::state_hook::clear_state_hooks();
    let mut world = PeWorldOwner::new_reset(PeWorldConfig {
        scene,
        ..PeWorldConfig::default()
    })
    .unwrap()
    .install_current()
    .unwrap();
    scoped(&mut world, || {
        rsvz::with_backend(|backend| {
            for &card in cards {
                backend.select_card(card.checked().unwrap()).unwrap();
            }
            backend.start_battle().unwrap();
            backend.set_sun_cost_ignored(true).unwrap();
            backend.set_zombie_spawn_stopped(true).unwrap();
        })
    });
    world
}

fn at(world: &mut PeWorldRun, clock: i32) {
    while world.with_backend(|b| b.clock().unwrap()) < clock {
        world.update_world().unwrap();
    }
    scoped(world, || {
        assert_eq!(
            rsvz_game::timeline::dispatch_timeline_tick(
                WaveTimingSnapshot::minimal(clock, Wave(1)),
                TickMeta {
                    phase: TickPhase::Playing,
                    game_ui: Some(GameUi::Playing),
                    clock: Some(clock),
                    is_new_frame: true
                }
            ),
            rsvz_schedule::TimelineDispatchResult::Continue
        );
    });
}

fn plants(world: &mut PeWorldRun) -> Vec<(PlantKind, i32, i32)> {
    scoped(world, || {
        rsvz::with_backend(|b| {
            b.plants()
                .unwrap()
                .map(|p| (b.plant_kind(p).unwrap(), b.plant_row(p) + 1, b.plant_col(p) + 1))
                .collect()
        })
    })
}

#[test]
fn skips_cooling_candidates_and_cleans_by_position_without_moving_cursor() {
    let mut world = world(&[scaredy.selection(), pot.selection(), sunflower.selection()]);
    let ran = std::rc::Rc::new(std::cell::Cell::new(false));
    scoped(&mut world, || {
        // The first candidate is on cooldown before C runs.
        rsvz::core::card(scaredy.selection(), 5, 1).unwrap();
        c.set_cards([scaredy, pot, sunflower]).unwrap();
        let ran = ran.clone();
        rsvz::__run_script(|| {
            (1, 0) << c(4, 12) + act(move || ran.set(true));
            Ok(())
        })
        .unwrap();
    });
    at(&mut world, 0);
    assert!(ran.get());
    assert!(plants(&mut world).contains(&(PlantKind::FlowerPot, 1, 9)));
    assert!(plants(&mut world).contains(&(PlantKind::Sunflower, 2, 9)));
    scoped(&mut world, || {
        // A replacement of the same kind is removed, matching AFodder rather than retained-ID Card.
        rsvz::core::shovel::shovel_target_at(Grid { row: 1, col: 8 }, rsvz::core::shovel::ShovelTarget::Default)
            .unwrap();
        rsvz::core::cards::new_plant(PlantKind::Sunflower, Grid { row: 1, col: 8 }).unwrap();
    });
    at(&mut world, 3);
    assert_eq!(plants(&mut world).len(), 3);
    at(&mut world, 4);
    assert_eq!(plants(&mut world), vec![(PlantKind::ScaredyShroom, 5, 1)]);
}

#[test]
fn automatic_list_uses_definitions_and_manual_order_survives_enter_fight() {
    use PlantKind::*;
    let mut world = world(&[
        Plant(Sunflower),
        Plant(FlowerPot),
        Plant(PuffShroom),
        CardSelection::Imitator(PuffShroom),
        Plant(GraveBuster),
        Plant(LilyPad),
        Plant(CoffeeBean),
        Plant(CherryBomb),
        Plant(WallNut),
        Plant(PotatoMine),
    ]);
    scoped(&mut world, || {
        let manager = rsvz_game::fodder::Fodder::new();
        manager.auto_set_cards().unwrap();
        assert_eq!(
            manager.cards(),
            [
                Plant(PuffShroom),
                CardSelection::Imitator(PuffShroom),
                Plant(FlowerPot),
                Plant(Sunflower)
            ]
        );
        manager.move_to_list_top([Sunflower, PuffShroom]).unwrap();
        manager.move_to_list_bottom(FlowerPot).unwrap();
        manager.erase_from_list(CardSelection::Imitator(PuffShroom));
        assert_eq!(manager.cards(), [Plant(Sunflower), Plant(PuffShroom), Plant(FlowerPot)]);
        manager.set_cards([Sunflower, FlowerPot]).unwrap();
        assert!(matches!(
            rsvz_game::state_hook::dispatch_state_event(rsvz::state_hook::StateEvent::EnterFight),
            rsvz::state_hook::StateHookDispatchResult::Continue
        ));
        assert_eq!(manager.cards(), [Plant(Sunflower), Plant(FlowerPot)]);
        assert_eq!(manager.available_count(0), 2);
        rsvz::core::card(Plant(Sunflower), 1, 1).unwrap();
        assert_eq!(manager.available_count(0), 1);
        assert_eq!(manager.available_count(751), 2);
    });
}

#[test]
fn trigger_by_filters_rows_and_nonpositive_delay_keeps_plants() {
    let mut world = world(&[pot.selection(), sunflower.selection()]);
    scoped(&mut world, || {
        rsvz::with_backend(|b| {
            let wave = b.current_wave().unwrap().0;
            b.add_zombie_in_row(ZombieKind::Normal, 1, wave).unwrap();
            b.add_zombie_in_row(ZombieKind::Conehead, 3, wave).unwrap();
        });
        let f = Fodder::new();
        f.set_cards([pot, sunflower]).unwrap();
        rsvz::__run_script(|| {
            (1, 0)
                << f.trigger_by([
                    ZombieKind::Normal & rsvz::dsl::curr_wave,
                    FodderConstraint::from(ZombieKind::Conehead) & rsvz::dsl::absc_in(0, 2000),
                ])(0, 12345);
            Ok(())
        })
        .unwrap();
    });
    at(&mut world, 0);
    assert_eq!(
        plants(&mut world),
        [(PlantKind::FlowerPot, 2, 9), (PlantKind::Sunflower, 4, 9)]
    );
    at(&mut world, 10);
    assert_eq!(plants(&mut world).len(), 2);
}

#[test]
fn cleanup_removes_existing_layers_and_native_card_does_not_add_containers() {
    let mut world = world(&[sunflower.selection()]);
    scoped(&mut world, || {
        rsvz::core::cards::new_plant(PlantKind::FlowerPot, Grid { row: 0, col: 8 }).unwrap();
        rsvz::core::cards::new_plant(PlantKind::Pumpkin, Grid { row: 0, col: 8 }).unwrap();
        c.set_cards(sunflower).unwrap();
        rsvz::__run_script(|| {
            (1, 0) << c(2, 1);
            Ok(())
        })
        .unwrap();
    });
    at(&mut world, 0);
    assert_eq!(plants(&mut world).len(), 3);
    at(&mut world, 2);
    assert!(plants(&mut world).is_empty());
}

use CardSelection::Plant;

#[test]
fn imitator_cleanup_handles_placeholder_and_morphed_plant() {
    for delay in [10, 330] {
        let selection = CardSelection::Imitator(PlantKind::FlowerPot);
        let mut world = world(&[selection]);
        scoped(&mut world, || {
            c.set_cards(selection).unwrap();
            rsvz::__run_script(|| {
                (1, 0) << c(delay, 1);
                Ok(())
            })
            .unwrap();
        });
        at(&mut world, 0);
        assert_eq!(plants(&mut world).len(), 1);
        at(&mut world, delay);
        assert!(plants(&mut world).is_empty(), "cleanup after {delay}cs");
    }
}

#[test]
fn auto_list_is_installed_on_enter_fight_and_failed_plant_still_records_cleanup_target() {
    let mut world = world(&[pot.selection(), sunflower.selection()]);
    scoped(&mut world, || {
        let f = Fodder::new();
        // Native ACard fails on an occupied cell, but AFodder still records the selected type.
        rsvz::core::cards::new_plant(PlantKind::Sunflower, Grid { row: 0, col: 8 }).unwrap();
        rsvz::__run_script(|| {
            (1, 0) << f(2, 1);
            Ok(())
        })
        .unwrap();
        assert!(matches!(
            rsvz_game::state_hook::dispatch_state_event(rsvz::state_hook::StateEvent::EnterFight),
            rsvz::state_hook::StateHookDispatchResult::Continue
        ));
        // Explicit list update after expression construction must be visible at execution time.
        f.set_cards(sunflower).unwrap();
    });
    at(&mut world, 0);
    assert_eq!(plants(&mut world).len(), 1);
    at(&mut world, 2);
    assert!(plants(&mut world).is_empty());
}

#[test]
fn water_placement_does_not_automatically_use_a_carried_lily() {
    let mut world = world_in(
        pe_rs::SceneType::Pool,
        &[sunflower.selection(), Plant(PlantKind::LilyPad)],
    );
    scoped(&mut world, || {
        c.set_cards(sunflower).unwrap();
        rsvz::__run_script(|| {
            (1, 0) << c(-1, 3);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            rsvz_game::fodder::land_positions()
                .iter()
                .map(|g| g.row + 1)
                .collect::<Vec<_>>(),
            [1, 2, 5, 6]
        );
    });
    at(&mut world, 0);
    assert!(plants(&mut world).is_empty());
}

#[test]
fn expressions_reused_in_two_waves_have_independent_cleanup_records() {
    let mut world = world(&[sunflower.selection()]);
    scoped(&mut world, || {
        c.set_cards(sunflower).unwrap();
        rsvz::with_backend(|b| rsvz_backend_api::SeedRuleEditBackend::set_seed_recharge_ignored(b, true)).unwrap();
        let expression = c(2, 1);
        rsvz::__run_script(|| {
            (1, 0) << &expression;
            (1, 4) << &expression;
            Ok(())
        })
        .unwrap();
    });
    at(&mut world, 0);
    assert_eq!(plants(&mut world).len(), 1);
    at(&mut world, 2);
    assert!(plants(&mut world).is_empty());
    at(&mut world, 4);
    assert_eq!(plants(&mut world).len(), 1);
    at(&mut world, 6);
    assert!(plants(&mut world).is_empty());
}
