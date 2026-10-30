//! Manager operations that retain cannon selection and reservations without roof timing correction.
use super::{CobBackend, CobManager, IntoCobFireDrops, IntoCobTarget, IntoCobTargets, report_fire, report_fire_many};
use crate::logic::cob::{current_cob_manager, current_cob_manager_for_fire};
use rsvz_backend_api::{BoardReadinessBackend, ClockBackend, GameUiBackend, SceneBackend};
use rsvz_current::CurrentBackend;

/// Same selection, cursor and error contract as the corresponding ordinary command, without roof timing correction.
pub fn try_fire_uncorrected<C>(row: i32, col: C) -> Result<i32, crate::cob::CobError>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    (i32, C): IntoCobTarget,
{
    try_fire_uncorrected_with_manager(&current_cob_manager_for_fire()?, row, col)
}

/// Executes against an explicitly configured list without roof timing correction.
pub fn try_fire_uncorrected_with_manager<C>(manager: &CobManager, row: i32, col: C) -> Result<i32, crate::cob::CobError>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    (i32, C): IntoCobTarget,
{
    manager.try_fire_uncorrected(row, col, false)
}

/// Reports rejected commands; accepted recovery commands may still be queued.
pub fn fire_uncorrected<C>(row: i32, col: C) -> Option<i32>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    (i32, C): IntoCobTarget,
{
    report_fire(try_fire_uncorrected(row, col))
}

/// Reports rejected commands against an explicit manager.
pub fn fire_uncorrected_with_manager<C>(manager: &CobManager, row: i32, col: C) -> Option<i32>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    (i32, C): IntoCobTarget,
{
    report_fire(try_fire_uncorrected_with_manager(manager, row, col))
}

/// Same selection, cursor and error contract as the corresponding ordinary command, without roof timing correction.
pub fn try_fire_uncorrected_many<T>(targets: T) -> Result<crate::cob::CobBatchReport, crate::cob::CobBatchError>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    T: IntoCobTargets,
{
    try_fire_uncorrected_many_with_manager(&current_cob_manager_for_fire()?, targets)
}

/// Executes against an explicitly configured list without roof timing correction.
pub fn try_fire_uncorrected_many_with_manager<T>(
    manager: &CobManager, targets: T,
) -> Result<crate::cob::CobBatchReport, crate::cob::CobBatchError>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    T: IntoCobTargets,
{
    manager.try_fire_uncorrected_many(targets, false)
}

/// Reports rejected commands; accepted recovery commands may still be queued.
pub fn fire_uncorrected_many<T>(targets: T) -> Vec<Option<i32>>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    T: IntoCobTargets,
{
    report_fire_many(try_fire_uncorrected_many(targets))
}

/// Reports rejected commands against an explicit manager.
pub fn fire_uncorrected_many_with_manager<T>(manager: &CobManager, targets: T) -> Vec<Option<i32>>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    T: IntoCobTargets,
{
    report_fire_many(try_fire_uncorrected_many_with_manager(manager, targets))
}

/// Same selection, cursor and error contract as the corresponding ordinary command, without roof timing correction.
pub fn try_recover_fire_uncorrected<C>(row: i32, col: C) -> Result<i32, crate::cob::CobError>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    (i32, C): IntoCobTarget,
{
    try_recover_fire_uncorrected_with_manager(&current_cob_manager_for_fire()?, row, col)
}

/// Executes against an explicitly configured list without roof timing correction.
pub fn try_recover_fire_uncorrected_with_manager<C>(
    manager: &CobManager, row: i32, col: C,
) -> Result<i32, crate::cob::CobError>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    (i32, C): IntoCobTarget,
{
    manager.try_fire_uncorrected(row, col, true)
}

/// Reports rejected commands; accepted recovery commands may still be queued.
pub fn recover_fire_uncorrected<C>(row: i32, col: C) -> Option<i32>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    (i32, C): IntoCobTarget,
{
    report_fire(try_recover_fire_uncorrected(row, col))
}

/// Reports rejected commands against an explicit manager.
pub fn recover_fire_uncorrected_with_manager<C>(manager: &CobManager, row: i32, col: C) -> Option<i32>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    (i32, C): IntoCobTarget,
{
    report_fire(try_recover_fire_uncorrected_with_manager(manager, row, col))
}

/// Same selection, cursor and error contract as the corresponding ordinary command, without roof timing correction.
pub fn try_recover_fire_uncorrected_many<T>(targets: T) -> Result<crate::cob::CobBatchReport, crate::cob::CobBatchError>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    T: IntoCobTargets,
{
    try_recover_fire_uncorrected_many_with_manager(&current_cob_manager_for_fire()?, targets)
}

/// Executes against an explicitly configured list without roof timing correction.
pub fn try_recover_fire_uncorrected_many_with_manager<T>(
    manager: &CobManager, targets: T,
) -> Result<crate::cob::CobBatchReport, crate::cob::CobBatchError>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    T: IntoCobTargets,
{
    manager.try_fire_uncorrected_many(targets, true)
}

/// Reports rejected commands; accepted recovery commands may still be queued.
pub fn recover_fire_uncorrected_many<T>(targets: T) -> Vec<Option<i32>>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    T: IntoCobTargets,
{
    report_fire_many(try_recover_fire_uncorrected_many(targets))
}

/// Reports rejected commands against an explicit manager.
pub fn recover_fire_uncorrected_many_with_manager<T>(manager: &CobManager, targets: T) -> Vec<Option<i32>>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    T: IntoCobTargets,
{
    report_fire_many(try_recover_fire_uncorrected_many_with_manager(manager, targets))
}

/// Fires specified cannons without roof timing correction. Checks the shared reservations, but does not advance any list.
pub fn try_raw_fire_uncorrected<D>(drops: D) -> Result<(), crate::cob::CobBatchError>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    D: IntoCobFireDrops,
{
    current_cob_manager().try_raw_fire_uncorrected(drops)
}

/// Reports errors from the corresponding uncorrected raw operation.
pub fn raw_fire_uncorrected<D>(drops: D)
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    D: IntoCobFireDrops,
{
    if let Err(error) = try_raw_fire_uncorrected(drops) {
        crate::diagnostics::report_operation_error(error);
    }
}

/// Fires specified cannons without roof timing correction. Checks the shared reservations, but does not advance any list.
pub fn try_raw_fire_uncorrected_coordinates<C>(
    cob_row: i32, cob_col: i32, row: i32, col: C,
) -> Result<(), crate::cob::CobBatchError>
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    (i32, i32, i32, C): IntoCobFireDrops,
{
    current_cob_manager().try_raw_fire_uncorrected((cob_row, cob_col, row, col))
}

/// Reports errors from the corresponding uncorrected raw operation.
pub fn raw_fire_uncorrected_coordinates<C>(cob_row: i32, cob_col: i32, row: i32, col: C)
where
    CurrentBackend: ClockBackend + CobBackend + SceneBackend + GameUiBackend + BoardReadinessBackend + 'static,
    (i32, i32, i32, C): IntoCobFireDrops,
{
    if let Err(error) = try_raw_fire_uncorrected((cob_row, cob_col, row, col)) {
        crate::diagnostics::report_operation_error(error);
    }
}
