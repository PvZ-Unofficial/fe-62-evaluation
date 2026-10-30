//! Current cannon operations.

use super::{
    CobManager, CobSequentialMode, RuntimeError, RuntimeResult, current_cob_manager, report_fire, report_fire_many,
};
use crate::logic::cob::{CobBackend, IntoCobTarget, IntoCobTargets};
pub use crate::logic::cob::{
    fire as try_fire_with_manager, fire_many as try_fire_many_with_manager,
    recover_fire as try_recover_fire_with_manager, recover_fire_many as try_recover_fire_many_with_manager,
};
use crate::logic::{
    IntoGrid,
    cards::CardPlantingBackend,
    cob::{CobListOrder, CobRecoverInfo, IntoCobFireDrops},
};
use rsvz_backend_api::CobFireBackend;
use rsvz_backend_api::backend::{ClockBackend, SceneBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{Grid, PlantId};
use rsvz_schedule::tick::with_scheduler;

pub fn try_fire<C>(row: i32, col: C) -> Result<i32, crate::cob::CobError>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    (i32, C): IntoCobTarget,
{
    try_fire_with_manager(&crate::logic::cob::current_cob_manager_for_fire()?, row, col)
}

pub fn fire<C>(row: i32, col: C) -> Option<i32>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    (i32, C): IntoCobTarget,
{
    report_fire(try_fire(row, col))
}

pub fn fire_with_manager<C>(manager: &CobManager, row: i32, col: C) -> Option<i32>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    (i32, C): IntoCobTarget,
{
    report_fire(try_fire_with_manager(manager, row, col))
}

pub fn try_fire_many<T>(targets: T) -> Result<crate::cob::CobBatchReport, crate::cob::CobBatchError>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    T: IntoCobTargets,
{
    try_fire_many_with_manager(&crate::logic::cob::current_cob_manager_for_fire()?, targets)
}

pub fn fire_many<T>(targets: T) -> Vec<Option<i32>>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    T: IntoCobTargets,
{
    report_fire_many(try_fire_many(targets))
}

pub fn fire_many_with_manager<T>(manager: &CobManager, targets: T) -> Vec<Option<i32>>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    T: IntoCobTargets,
{
    report_fire_many(try_fire_many_with_manager(manager, targets))
}

/// Returns an owned copy of the configured list, without discovery or validation.
pub fn cob_list() -> Vec<Grid> {
    cob_list_with_manager(&current_cob_manager())
}

/// Returns configured grids in list order without changing the manager.
pub fn cob_list_with_manager(manager: &CobManager) -> Vec<Grid> {
    let mut grids = Vec::new();
    manager.for_each_grid(|grid| grids.push(grid));
    grids
}

pub fn for_each_cob_grid<F>(visit: F)
where
    F: FnMut(Grid),
{
    current_cob_manager().for_each_grid(visit);
}

pub fn for_each_cob_grid_with_manager<F>(manager: &CobManager, visit: F)
where
    F: FnMut(Grid),
{
    manager.for_each_grid(visit);
}

pub fn set_cob_columns(manager: &CobManager, col: i32)
where
    CurrentBackend: CobFireBackend,
{
    {
        let Ok(grid) = Grid::from_one_based(1, col) else {
            crate::diagnostics::report_operation_error(RuntimeError::new(format!("invalid cob tail column {col}")));
            return;
        };
        if let Err(error) = manager.auto_set_list_in_column(grid.col) {
            crate::diagnostics::report_operation_error(RuntimeError::from_source(error));
        }
    }
}

pub fn set_cob_sequential_mode(mode: CobSequentialMode) {
    current_cob_manager().set_sequential_mode(mode);
}

pub fn set_cob_sequential_mode_with_manager(manager: &CobManager, mode: CobSequentialMode) {
    manager.set_sequential_mode(mode);
}

pub fn set_next_cob_slot(slot: i32) -> RuntimeResult {
    current_cob_manager()
        .set_next_slot(slot)
        .map_err(RuntimeError::from_source)
}

pub fn set_next_cob_slot_with_manager(manager: &CobManager, slot: i32) -> RuntimeResult {
    manager.set_next_slot(slot).map_err(RuntimeError::from_source)
}

pub fn set_next_cob<G>(grid: G) -> RuntimeResult
where
    G: IntoGrid,
{
    current_cob_manager()
        .set_next_grid(grid.try_into_grid().map_err(RuntimeError::from_source)?)
        .map_err(RuntimeError::from_source)
}

pub fn set_next_cob_with_manager<G>(manager: &CobManager, grid: G) -> RuntimeResult
where
    G: IntoGrid,
{
    manager
        .set_next_grid(grid.try_into_grid().map_err(RuntimeError::from_source)?)
        .map_err(RuntimeError::from_source)
}

pub fn skip_cobs(n: i32) -> RuntimeResult {
    current_cob_manager().skip(n).map_err(RuntimeError::from_source)
}

pub fn skip_cobs_with_manager(manager: &CobManager, n: i32) -> RuntimeResult {
    manager.skip(n).map_err(RuntimeError::from_source)
}

pub fn erase_cobs_from_list<I, G>(grids: I)
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    current_cob_manager().erase_from_list(&collect_grids(grids));
}

pub fn erase_cobs_from_list_with_manager<I, G>(manager: &CobManager, grids: I)
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    manager.erase_from_list(&collect_grids(grids));
}

pub fn move_cobs_to_list_top<I, G>(grids: I) -> RuntimeResult
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    current_cob_manager()
        .move_to_list_top(&collect_grids(grids))
        .map_err(RuntimeError::from_source)
}

pub fn move_cobs_to_list_top_with_manager<I, G>(manager: &CobManager, grids: I) -> RuntimeResult
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    manager
        .move_to_list_top(&collect_grids(grids))
        .map_err(RuntimeError::from_source)
}

pub fn move_cobs_to_list_bottom<I, G>(grids: I) -> RuntimeResult
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    current_cob_manager()
        .move_to_list_bottom(&collect_grids(grids))
        .map_err(RuntimeError::from_source)
}

pub fn move_cobs_to_list_bottom_with_manager<I, G>(manager: &CobManager, grids: I) -> RuntimeResult
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    manager
        .move_to_list_bottom(&collect_grids(grids))
        .map_err(RuntimeError::from_source)
}

pub fn recover_cob_list() -> RuntimeResult<Vec<CobRecoverInfo>>
where
    CurrentBackend: CobBackend,
{
    current_cob_manager().recover_list().map_err(RuntimeError::from_source)
}

pub fn recover_cob_list_with_manager(manager: &CobManager) -> RuntimeResult<Vec<CobRecoverInfo>>
where
    CurrentBackend: CobBackend,
{
    manager.recover_list().map_err(RuntimeError::from_source)
}

pub fn usable_cob_list() -> RuntimeResult<Vec<PlantId>>
where
    CurrentBackend: CobBackend,
{
    current_cob_manager().usable_list().map_err(RuntimeError::from_source)
}

pub fn usable_cob_list_with_manager(manager: &CobManager) -> RuntimeResult<Vec<PlantId>>
where
    CurrentBackend: CobBackend,
{
    manager.usable_list().map_err(RuntimeError::from_source)
}

pub fn recover_cob() -> RuntimeResult<Option<PlantId>>
where
    CurrentBackend: CobBackend,
{
    current_cob_manager().recover_cob().map_err(RuntimeError::from_source)
}

pub fn recover_cob_with_manager(manager: &CobManager) -> RuntimeResult<Option<PlantId>>
where
    CurrentBackend: CobBackend,
{
    manager.recover_cob().map_err(RuntimeError::from_source)
}

pub fn usable_cob() -> RuntimeResult<Option<PlantId>>
where
    CurrentBackend: CobBackend,
{
    current_cob_manager().usable_cob().map_err(RuntimeError::from_source)
}

pub fn usable_cob_with_manager(manager: &CobManager) -> RuntimeResult<Option<PlantId>>
where
    CurrentBackend: CobBackend,
{
    manager.usable_cob().map_err(RuntimeError::from_source)
}

pub fn roof_recover_cob_list(drop_col: f32) -> RuntimeResult<Vec<CobRecoverInfo>>
where
    CurrentBackend: CobBackend,
{
    current_cob_manager()
        .roof_recover_list(drop_col)
        .map_err(RuntimeError::from_source)
}

pub fn roof_recover_cob_list_with_manager(manager: &CobManager, drop_col: f32) -> RuntimeResult<Vec<CobRecoverInfo>>
where
    CurrentBackend: CobBackend,
{
    manager.roof_recover_list(drop_col).map_err(RuntimeError::from_source)
}

pub fn roof_usable_cob_list(drop_col: f32) -> RuntimeResult<Vec<PlantId>>
where
    CurrentBackend: CobBackend,
{
    current_cob_manager()
        .roof_usable_list(drop_col)
        .map_err(RuntimeError::from_source)
}

pub fn roof_usable_cob_list_with_manager(manager: &CobManager, drop_col: f32) -> RuntimeResult<Vec<PlantId>>
where
    CurrentBackend: CobBackend,
{
    manager.roof_usable_list(drop_col).map_err(RuntimeError::from_source)
}

pub fn roof_recover_cob(drop_col: f32) -> RuntimeResult<Option<PlantId>>
where
    CurrentBackend: CobBackend,
{
    current_cob_manager()
        .roof_recover_cob(drop_col)
        .map_err(RuntimeError::from_source)
}

pub fn roof_recover_cob_with_manager(manager: &CobManager, drop_col: f32) -> RuntimeResult<Option<PlantId>>
where
    CurrentBackend: CobBackend,
{
    manager.roof_recover_cob(drop_col).map_err(RuntimeError::from_source)
}

pub fn roof_usable_cob(drop_col: f32) -> RuntimeResult<Option<PlantId>>
where
    CurrentBackend: CobBackend,
{
    current_cob_manager()
        .roof_usable_cob(drop_col)
        .map_err(RuntimeError::from_source)
}

pub fn roof_usable_cob_with_manager(manager: &CobManager, drop_col: f32) -> RuntimeResult<Option<PlantId>>
where
    CurrentBackend: CobBackend,
{
    manager.roof_usable_cob(drop_col).map_err(RuntimeError::from_source)
}

pub fn roof_cob_fly_time(cob_col: i32, drop_col: f32) -> RuntimeResult<i32> {
    crate::logic::cob::classic_roof_cob_fly_time(cob_col, drop_col).map_err(RuntimeError::from_source)
}

pub fn try_set_cobs<I, G>(grids: I) -> RuntimeResult<()>
where
    CurrentBackend: CobFireBackend,
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    {
        current_cob_manager()
            .set_checked_list_validated(grids)
            .map_err(|error| RuntimeError::from_source(error))
    }
}

pub fn try_set_cobs_with_manager<I, G>(manager: &CobManager, grids: I) -> RuntimeResult<()>
where
    CurrentBackend: CobFireBackend,
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    {
        manager
            .set_checked_list_validated(grids)
            .map_err(|error| RuntimeError::from_source(error))
    }
}

pub fn set_cobs<I, G>(grids: I)
where
    CurrentBackend: CobFireBackend,
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    if let Err(error) = try_set_cobs(grids) {
        crate::diagnostics::report_operation_error(error);
    }
}

pub fn set_cobs_with_manager<I, G>(manager: &CobManager, grids: I)
where
    CurrentBackend: CobFireBackend,
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    if let Err(error) = try_set_cobs_with_manager(manager, grids) {
        crate::diagnostics::report_operation_error(error);
    }
}

/// Refresh the default and all eight built-in column lists in one board scan.
pub fn try_auto_set_cobs() -> RuntimeResult<()>
where
    CurrentBackend: CobFireBackend,
{
    try_auto_set_cobs_by(CobListOrder::Horizontal)
}

pub fn try_auto_set_cobs_by(order: CobListOrder) -> RuntimeResult<()>
where
    CurrentBackend: CobFireBackend,
{
    crate::logic::cob::refresh_builtin_cob_lists(order)
}

pub fn try_auto_set_cobs_with_manager(manager: &CobManager) -> RuntimeResult<()>
where
    CurrentBackend: CobFireBackend,
{
    {
        manager
            .auto_set_list()
            .map_err(|error| RuntimeError::from_source(error))
    }
}

pub fn try_auto_set_cobs_by_with_manager(manager: &CobManager, order: CobListOrder) -> RuntimeResult<()>
where
    CurrentBackend: CobFireBackend,
{
    {
        manager
            .auto_set_list_by(order)
            .map_err(|error| RuntimeError::from_source(error))
    }
}

pub fn auto_set_cobs()
where
    CurrentBackend: CobFireBackend,
{
    if let Err(error) = try_auto_set_cobs() {
        crate::diagnostics::report_operation_error(error);
    }
}

pub fn auto_set_cobs_by(order: CobListOrder)
where
    CurrentBackend: CobFireBackend,
{
    if let Err(error) = try_auto_set_cobs_by(order) {
        crate::diagnostics::report_operation_error(error);
    }
}

pub fn auto_set_cobs_with_manager(manager: &CobManager)
where
    CurrentBackend: CobFireBackend,
{
    if let Err(error) = try_auto_set_cobs_with_manager(manager) {
        crate::diagnostics::report_operation_error(error);
    }
}

pub fn auto_set_cobs_by_with_manager(manager: &CobManager, order: CobListOrder)
where
    CurrentBackend: CobFireBackend,
{
    if let Err(error) = try_auto_set_cobs_by_with_manager(manager, order) {
        crate::diagnostics::report_operation_error(error);
    }
}

pub fn try_recover_fire_many<T>(targets: T) -> Result<crate::cob::CobBatchReport, crate::cob::CobBatchError>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    T: IntoCobTargets,
{
    crate::logic::cob::recover_fire_many(&crate::logic::cob::current_cob_manager_for_fire()?, targets)
}

pub fn try_recover_fire<C>(row: i32, col: C) -> Result<i32, crate::cob::CobError>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    (i32, C): IntoCobTarget,
{
    crate::logic::cob::recover_fire(&crate::logic::cob::current_cob_manager_for_fire()?, row, col)
}

pub fn recover_fire_many<T>(targets: T) -> Vec<Option<i32>>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    T: IntoCobTargets,
{
    report_fire_many(try_recover_fire_many(targets))
}

pub fn recover_fire<C>(row: i32, col: C) -> Option<i32>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    (i32, C): IntoCobTarget,
{
    report_fire(try_recover_fire(row, col))
}

pub fn recover_fire_many_with_manager<T>(manager: &CobManager, targets: T) -> Vec<Option<i32>>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    T: IntoCobTargets,
{
    report_fire_many(try_recover_fire_many_with_manager(manager, targets))
}

pub fn recover_fire_with_manager<C>(manager: &CobManager, row: i32, col: C) -> Option<i32>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    (i32, C): IntoCobTarget,
{
    report_fire(try_recover_fire_with_manager(manager, row, col))
}

pub fn try_raw_fire<D>(drops: D) -> Result<(), crate::cob::CobBatchError>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    D: IntoCobFireDrops,
{
    crate::logic::cob::raw_fire(&current_cob_manager(), drops)
}

pub fn try_raw_fire_coordinates<C>(
    cob_row: i32, cob_col: i32, row: i32, col: C,
) -> Result<(), crate::cob::CobBatchError>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    (i32, i32, i32, C): IntoCobFireDrops,
{
    crate::logic::cob::raw_fire(&current_cob_manager(), (cob_row, cob_col, row, col))
}

pub fn raw_fire<D>(drops: D)
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    D: IntoCobFireDrops,
{
    if let Err(error) = try_raw_fire(drops) {
        crate::diagnostics::report_operation_error(error);
    }
}

pub fn raw_fire_coordinates<C>(cob_row: i32, cob_col: i32, row: i32, col: C)
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
    (i32, i32, i32, C): IntoCobFireDrops,
{
    if let Err(error) = try_raw_fire_coordinates(cob_row, cob_col, row, col) {
        crate::diagnostics::report_operation_error(error);
    }
}

pub fn plant_cob<G>(grid: G) -> RuntimeResult
where
    CurrentBackend: CardPlantingBackend + 'static,
    G: IntoGrid,
{
    with_scheduler(|scheduler| {
        current_cob_manager().plant(scheduler, grid);
        Ok(())
    })
}

pub fn fix_latest_cob() -> RuntimeResult
where
    CurrentBackend: ClockBackend + CardPlantingBackend + 'static,
{
    with_scheduler(|scheduler| {
        current_cob_manager()
            .fix_latest(scheduler)
            .map_err(RuntimeError::from_source)
    })
}

pub fn fix_latest_cob_with_manager(manager: &CobManager) -> RuntimeResult
where
    CurrentBackend: ClockBackend + CardPlantingBackend + 'static,
{
    with_scheduler(|scheduler| manager.fix_latest(scheduler).map_err(RuntimeError::from_source))
}

fn collect_grids<I, G>(grids: I) -> Vec<Grid>
where
    I: IntoIterator<Item = G>,
    G: IntoGrid,
{
    grids.into_iter().map(IntoGrid::into_grid).collect()
}
