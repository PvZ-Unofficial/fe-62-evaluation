//! Current cannon operations and explicitly shared cannon lists.

pub use crate::logic::cob::CobManager;
use crate::logic::cob::current_cob_manager;
pub use crate::logic::cob::timing;
pub use crate::logic::cob::{
    CobBackend, CobFireDrop, CobInputColumn, CobListOrder, CobManagerCallError, CobManagerError, CobRecoverInfo,
    CobSequentialMode, IntoCobFireDrop, IntoCobFireDrops, IntoCobTarget, IntoCobTargets, NO_EXIST_RECOVER_TIME,
    cob_recover_time, cob_target_to_pixel as target_to_pixel, current_cob_manager as default_manager,
    find_cob_at as find_cob, fire_cob as fire_at_pixel, try_plant_cob as plant_cob_step,
};
use crate::runtime::{RuntimeError, RuntimeResult};

mod error;
pub use error::{CobBatchError, CobBatchReport, CobError, CobSelectionEvidence};

fn report_fire(result: Result<i32, CobError>) -> Option<i32> {
    match result {
        Ok(index) => Some(index),
        Err(error) => {
            crate::diagnostics::report_operation_error(error);
            None
        }
    }
}

fn report_fire_many(result: Result<CobBatchReport, CobBatchError>) -> Vec<Option<i32>> {
    match result {
        Ok(report) => report.indices,
        Err(error) => {
            crate::diagnostics::report_operation_error_ref(&error);
            error.report.indices
        }
    }
}

mod uncorrected;
pub use uncorrected::*;

mod current;
pub use current::*;

mod impact;
mod input;

/// Cross-crate scripting implementation support, not a user API.
#[doc(hidden)]
pub mod __private;

/// Returns the shared built-in manager for a 1-based cannon tail column (1..=8).
/// First access discovers its list; subsequent access preserves explicit edits.
/// The handle retains its identity across automatic script restarts.
pub fn column_manager(column: usize) -> RuntimeResult<CobManager>
where
    rsvz_current::CurrentBackend: rsvz_backend_api::PlantReadBackend,
{
    crate::logic::cob::column_cob_manager(column)
}
