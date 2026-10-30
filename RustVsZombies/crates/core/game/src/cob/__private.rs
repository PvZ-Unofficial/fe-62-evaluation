//! Private cross-crate support for column commands and DSL registration.
use super::{
    CobBackend, CobManager, IntoCobTarget, IntoCobTargets, report_fire, report_fire_many, try_fire_many_with_manager,
    try_fire_with_manager,
};
use rsvz_backend_api::{ClockBackend, SceneBackend};
use rsvz_current::CurrentBackend;

pub use super::input::FireInput;

pub use super::impact::{ImpactManager, prepare, prepare_default_pair, prepare_raw};

pub fn prepare_auto_set_list_capacity(manager: CobManager) -> impl FnOnce() + 'static
where
    CurrentBackend: rsvz_backend_api::PlantPoolBackend,
{
    manager.prepare_auto_set_list_capacity()
}

/// Fires with the script's lazily discovered tail-column list.
#[doc(hidden)]
pub fn fire_in_column<C>(column: usize, row: i32, col: C) -> Option<i32>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend
        + 'static,
    (i32, C): IntoCobTarget,
{
    report_fire(try_fire_in_column(column, row, col))
}

pub fn try_fire_in_column<C>(column: usize, row: i32, col: C) -> Result<i32, super::CobError>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend
        + 'static,
    (i32, C): IntoCobTarget,
{
    crate::logic::cob::column_cob_manager(column)
        .map_err(Into::into)
        .and_then(|manager| try_fire_with_manager(&manager, row, col))
}

/// Fires a batch with the same tail-column cursor and per-target results as fire_many.
#[doc(hidden)]
pub fn fire_many_in_column<T>(column: usize, targets: T) -> Vec<Option<i32>>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend
        + 'static,
    T: IntoCobTargets,
{
    report_fire_many(try_fire_many_in_column(column, targets))
}

pub fn try_fire_many_in_column<T>(column: usize, targets: T) -> Result<super::CobBatchReport, super::CobBatchError>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend
        + 'static,
    T: IntoCobTargets,
{
    crate::logic::cob::column_cob_manager(column)
        .map_err(Into::into)
        .and_then(|manager| try_fire_many_with_manager(&manager, targets))
}

/// A single typed/tuple target uses the same executor as the two-coordinate call.
pub fn try_fire_target<T: IntoCobTarget>(
    manager: Option<&CobManager>, target: T, recover: bool, uncorrected: bool,
) -> Result<i32, super::CobError>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend
        + 'static,
{
    let default;
    let manager = match manager {
        Some(manager) => manager,
        None => {
            default = crate::logic::cob::current_cob_manager_for_fire()?;
            &default
        }
    };
    manager.try_fire_target(target, recover, uncorrected)
}

pub fn fire_target<T: IntoCobTarget>(
    manager: Option<&CobManager>, target: T, recover: bool, uncorrected: bool,
) -> Option<i32>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend
        + 'static,
{
    report_fire(try_fire_target(manager, target, recover, uncorrected))
}

pub fn try_fire_target_in_column<T: IntoCobTarget>(column: usize, target: T) -> Result<i32, super::CobError>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend
        + 'static,
{
    crate::logic::cob::column_cob_manager(column)
        .map_err(Into::into)
        .and_then(|manager| manager.try_fire_target(target, false, false))
}

pub fn fire_target_in_column<T: IntoCobTarget>(column: usize, target: T) -> Option<i32>
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend
        + 'static,
{
    report_fire(try_fire_target_in_column(column, target))
}
