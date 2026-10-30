use rsvz::cob::{CobListOrder, CobSequentialMode};
use rsvz::prelude::*;

// This fixture checks source compatibility; execution requires a game scope.
fn main() {
    at(1, 0, || cob::set_next_cob_slot(1));
    at(1, 0, || cob::usable_cob());
    at(1, 0, || -> RuntimeResult<()> {
        cob::set_next_cob_slot(1)?;
        cob::try_auto_set_cobs()?;
        cob::try_fire_1(2, 9)?;
        Ok(())
    });
    let input = target(2, 9.0);
    let manager = CobManager::new();
    macro_rules! check_shapes {
        ($name:ident, $single:ty, $batch:ty) => {{
            let _: $single = cob::$name((2, 9));
            let _: $single = cob::$name((2, 9.0_f32));
            let _: $single = cob::$name(input);
            let _: $batch = cob::$name([input]);
            let _: $batch = cob::$name(vec![(2, 9)]);
            let _: $batch = cob::$name([(2, 9.0)].as_slice());
        }};
    }
    macro_rules! check_manager_shapes {
        ($name:ident, $single:ty, $batch:ty) => {{
            check_shapes!($name, $single, $batch);
            let _: $single = cob::$name(&manager, (2, 9));
            let _: $single = cob::$name(&manager, input);
            let _: $batch = cob::$name(&manager, [input]);
            let _: $batch = cob::$name(&manager, vec![(2, 9)]);
            let _: $batch = cob::$name(&manager, [(2, 9.0)].as_slice());
        }};
    }
    check_manager_shapes!(fire, Option<i32>, Vec<Option<i32>>);
    check_manager_shapes!(try_fire, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_manager_shapes!(recover_fire, Option<i32>, Vec<Option<i32>>);
    check_manager_shapes!(try_recover_fire, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_manager_shapes!(fire_uncorrected, Option<i32>, Vec<Option<i32>>);
    check_manager_shapes!(try_fire_uncorrected, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_manager_shapes!(recover_fire_uncorrected, Option<i32>, Vec<Option<i32>>);
    check_manager_shapes!(try_recover_fire_uncorrected, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_shapes!(fire_1, Option<i32>, Vec<Option<i32>>);
    check_shapes!(try_fire_1, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_shapes!(fire_2, Option<i32>, Vec<Option<i32>>);
    check_shapes!(try_fire_2, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_shapes!(fire_3, Option<i32>, Vec<Option<i32>>);
    check_shapes!(try_fire_3, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_shapes!(fire_4, Option<i32>, Vec<Option<i32>>);
    check_shapes!(try_fire_4, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_shapes!(fire_5, Option<i32>, Vec<Option<i32>>);
    check_shapes!(try_fire_5, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_shapes!(fire_6, Option<i32>, Vec<Option<i32>>);
    check_shapes!(try_fire_6, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_shapes!(fire_7, Option<i32>, Vec<Option<i32>>);
    check_shapes!(try_fire_7, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    check_shapes!(fire_8, Option<i32>, Vec<Option<i32>>);
    check_shapes!(try_fire_8, Result<i32, cob::CobError>, Result<cob::CobBatchReport, cob::CobBatchError>);
    let _ = fire_1(2, 9);
    let _ = fire_1([(2, 9.0), (4, 9.0)]);
    let _ = fire_2(2, 9);
    let _ = fire_2([(2, 9.0), (4, 9.0)]);
    let _ = fire_3(2, 9);
    let _ = fire_3([(2, 9.0), (4, 9.0)]);
    let _ = fire_4(2, 9);
    let _ = fire_4([(2, 9.0), (4, 9.0)]);
    let _ = fire_5(2, 9);
    let _ = fire_5([(2, 9.0), (4, 9.0)]);
    let _ = fire_6(2, 9);
    let _ = fire_6([(2, 9.0), (4, 9.0)]);
    let _ = fire_7(2, 9);
    let _ = fire_7([(2, 9.0), (4, 9.0)]);
    let _ = fire_8(2, 9);
    let _ = fire_8([(2, 9.0), (4, 9.0)]);
    let default = cob::default_manager();
    let _ = cob::find_cob((1, 1));
    let _ = rsvz::plant::find_plant_at_kind((1, 1), PlantKind::CobCannon);
    if let Some(id) = cob::find_cob(grid(1, 1)) {
        let _ = cob::cob_recover_time(id);
        let _ = cob::fire_at_pixel(id, cob::target_to_pixel(target(2, 9.0)).unwrap());
    }
    let _ = cob::plant_cob_step(grid(1, 1));
    let one: fn(i32, f32) -> Option<i32> = rsvz::core::cob::fire::<f32>;
    let explicit: fn(&CobManager, i32, f32) -> Result<i32, cob::CobError> = rsvz::core::cob::try_fire_with_manager::<f32>;
    let _ = (one(2, 9.0), explicit(&default, 2, 9.0));
    let _ = cob::fire_uncorrected(2, 9.0);
    let _ = cob::fire_uncorrected([(2, 9.0), (4, 9.0)]);
    let _ = cob::fire_uncorrected(&default, 2, 9.0);
    let _ = cob::fire_uncorrected(&default, [(2, 9.0), (4, 9.0)]);
    let _ = cob::try_fire_uncorrected(2, 9.0);
    let _ = cob::try_fire_uncorrected([(2, 9.0), (4, 9.0)]);
    let _ = cob::try_fire_uncorrected(&default, 2, 9.0);
    let _ = cob::try_fire_uncorrected(&default, [(2, 9.0), (4, 9.0)]);
    let _ = cob::recover_fire_uncorrected(2, 9.0);
    let _ = cob::recover_fire_uncorrected([(2, 9.0), (4, 9.0)]);
    let _ = cob::recover_fire_uncorrected(&default, 2, 9.0);
    let _ = cob::recover_fire_uncorrected(&default, [(2, 9.0), (4, 9.0)]);
    let _ = cob::try_recover_fire_uncorrected(2, 9.0);
    let _ = cob::try_recover_fire_uncorrected([(2, 9.0), (4, 9.0)]);
    let _ = cob::try_recover_fire_uncorrected(&default, 2, 9.0);
    let _ = cob::try_recover_fire_uncorrected(&default, [(2, 9.0), (4, 9.0)]);
    let _ = cob::raw_fire_uncorrected(1, 1, 2, 9.0);
    let _ = cob::raw_fire_uncorrected([(1, 1, 2, 9.0), (2, 1, 4, 9.0)]);
    let _ = cob::try_raw_fire_uncorrected(1, 1, 2, 9.0);
    let _ = cob::try_raw_fire_uncorrected([(1, 1, 2, 9.0), (2, 1, 4, 9.0)]);
    let wind = CobManager::new();
    for_each_cob_grid(|_| ());
    for_each_cob_grid(&wind, |_| ());
    set_cob_sequential_mode(CobSequentialMode::Time);
    set_cob_sequential_mode(&wind, CobSequentialMode::Space);
    let _ = (set_next_cob_slot(1), set_next_cob_slot(&wind, 1));
    let _ = (set_next_cob((1, 1)), set_next_cob(&wind, (1, 1)));
    let _ = (skip_cobs(1), skip_cobs(&wind, 1));
    erase_cobs_from_list([(1, 1)]);
    erase_cobs_from_list(&wind, [(1, 1)]);
    let _ = (move_cobs_to_list_top([(1, 1)]), move_cobs_to_list_top(&wind, [(1, 1)]));
    let _ = (
        move_cobs_to_list_bottom([(1, 1)]),
        move_cobs_to_list_bottom(&wind, [(1, 1)]),
    );
    let _ = (recover_cob_list(), recover_cob_list(&wind));
    let _ = (usable_cob_list(), usable_cob_list(&wind));
    let _ = (recover_cob(), recover_cob(&wind));
    let _ = (usable_cob(), usable_cob(&wind));
    let _ = (roof_recover_cob_list(9.0), roof_recover_cob_list(&wind, 9.0));
    let _ = (roof_usable_cob_list(9.0), roof_usable_cob_list(&wind, 9.0));
    let _ = (roof_recover_cob(9.0), roof_recover_cob(&wind, 9.0));
    let _ = (roof_usable_cob(9.0), roof_usable_cob(&wind, 9.0));
    let _ = roof_cob_fly_time(1, 9.0);
    let _ = (try_set_cobs([(1, 1)]), try_set_cobs(&wind, [(1, 1)]));
    set_cobs([(1, 1)]);
    set_cobs(&wind, [(1, 1)]);
    let _ = (try_auto_set_cobs(), try_auto_set_cobs(CobListOrder::Horizontal));
    let _ = (
        try_auto_set_cobs(&wind),
        try_auto_set_cobs(&wind, CobListOrder::Vertical),
    );
    auto_set_cobs();
    auto_set_cobs(CobListOrder::Horizontal);
    auto_set_cobs(&wind);
    auto_set_cobs(&wind, CobListOrder::Vertical);
    let _ = (try_recover_fire(1, 9), try_recover_fire([(1, 9)]));
    let _ = (try_recover_fire(&wind, 1, 9), try_recover_fire(&wind, [(1, 9)]));
    recover_fire(1, 9);
    recover_fire([(1, 9)]);
    recover_fire(&wind, 1, 9);
    recover_fire(&wind, [(1, 9)]);
    let _ = (try_raw_fire(1, 1, 1, 9), try_raw_fire([(1, 1, 1, 9)]));
    raw_fire(1, 1, 1, 9);
    raw_fire([(1, 1, 1, 9)]);
    let _ = plant_cob((1, 1));
    let _ = (fix_latest_cob(), fix_latest_cob(&wind));
    let _ = (try_fire(1, 9), try_fire([(1, 9)]));
    let _ = (try_fire(&wind, 1, 9), try_fire(&wind, [(1, 9)]));
    fire(1, 9);
    fire([(1, 9)]);
    fire(&wind, 1, 9);
    fire(&wind, [(1, 9)]);
}
