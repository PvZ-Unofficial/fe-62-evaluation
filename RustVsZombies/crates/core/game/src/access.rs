//! Shared primitive access used by current game operations.
use rsvz_current::CurrentBackend;

pub(crate) fn with_backend<R>(f: impl for<'a> FnOnce(&'a CurrentBackend) -> R) -> R {
    CurrentBackend::with_backend_shared(f).unwrap_or_else(|error| {
        crate::diagnostics::abort_operation(crate::runtime::RuntimeError::new(error.to_string()))
    })
}

/// Exclusive physical access; conflicting callback borrows abort only that callback.
/// Use ordinary game queries or `with_frame` for shared reads. This entry is for
/// low-level probes, fixtures and operations that can invalidate entity borrows.
pub fn with_exclusive_backend<R>(f: impl for<'a> FnOnce(&'a mut CurrentBackend) -> R) -> R {
    CurrentBackend::try_with_backend(f).unwrap_or_else(|error| {
        crate::diagnostics::abort_operation(crate::runtime::RuntimeError::new(error.to_string()))
    })
}

/// Preserve domain/action rejection; access and native read faults cannot be
/// recovered by a user action retry within the current callback.
pub(crate) fn operation_error(error: rsvz_current::CurrentBackendError) -> crate::runtime::RuntimeError {
    rejected_error(error).into()
}

/// Keep the typed native rejection while composing an action in core.
pub(crate) fn rejected_error(error: rsvz_current::CurrentBackendError) -> rsvz_current::CurrentBackendError {
    if error.is_operation_rejection() {
        error
    } else {
        crate::diagnostics::abort_operation(error.into())
    }
}
