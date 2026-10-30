//! Shared callback and backend boundary error.

use std::borrow::Cow;
use std::{error::Error, fmt, sync::Arc};

/// Result shorthand used by backend-free runtime callbacks.
pub type RuntimeResult<T = ()> = Result<T, RuntimeError>;

/// Backend-neutral runtime error for callbacks that no longer carry a backend error type.
#[derive(Clone, Debug)]
pub struct RuntimeError {
    message: Cow<'static, str>,
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl RuntimeError {
    #[must_use]
    pub fn new(message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    #[must_use]
    pub fn message(&self) -> Cow<'_, str> {
        if self.source.is_some() {
            Cow::Owned(self.to_string())
        } else {
            Cow::Borrowed(&self.message)
        }
    }

    /// Preserves a concrete error without formatting it during propagation.
    pub fn from_source(error: impl Error + Send + Sync + 'static) -> Self {
        Self {
            message: Cow::Borrowed(""),
            source: Some(Arc::new(error)),
        }
    }

    /// Adds context while retaining the complete source chain.
    pub fn context(self, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            message: message.into(),
            source: Some(Arc::new(self)),
        }
    }

    /// Finds a concrete cause, including through intermediate runtime wrappers.
    pub fn find_source<E: Error + 'static>(&self) -> Option<&E> {
        let mut cause: Option<&(dyn Error + 'static)> = Some(self);
        while let Some(error) = cause {
            if let Some(error) = error.downcast_ref::<E>() {
                return Some(error);
            }
            cause = error.source();
        }
        None
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        if let Some(source) = &self.source {
            if !self.message.is_empty() {
                f.write_str(": ")?;
            }
            fmt::Display::fmt(source, f)?;
        }
        Ok(())
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|error| error as _)
    }
}

// Text errors compare by value; opaque causes compare by identity, never by
// their rendered text. Cloning an error preserves that identity.
impl PartialEq for RuntimeError {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message
            && match (&self.source, &other.source) {
                (None, None) => true,
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                _ => false,
            }
    }
}
impl Eq for RuntimeError {}

impl From<Cow<'static, str>> for RuntimeError {
    fn from(message: Cow<'static, str>) -> Self {
        Self::new(message)
    }
}

impl From<&'static str> for RuntimeError {
    fn from(message: &'static str) -> Self {
        Self::new(message)
    }
}

impl From<String> for RuntimeError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&RuntimeError> for RuntimeError {
    fn from(error: &RuntimeError) -> Self {
        error.clone()
    }
}

impl From<rsvz_model::ZombieMotionCallError> for RuntimeError {
    fn from(error: rsvz_model::ZombieMotionCallError) -> Self {
        Self::from_source(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct Probe(Arc<AtomicUsize>);
    impl fmt::Display for Probe {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fetch_add(1, Ordering::Relaxed);
            f.write_str("concrete cause")
        }
    }
    impl Error for Probe {}

    #[test]
    fn propagation_and_context_preserve_the_source_without_formatting() {
        let formatted = Arc::new(AtomicUsize::new(0));
        let error = RuntimeError::from_source(Probe(formatted.clone())).context("outer");
        let saved = error.clone();
        assert!(std::ptr::eq(
            error.find_source::<Probe>().unwrap(),
            saved.find_source::<Probe>().unwrap()
        ));
        assert_eq!(formatted.load(Ordering::Relaxed), 0);
        assert_eq!(error.to_string(), "outer: concrete cause");
        assert_eq!(formatted.load(Ordering::Relaxed), 1);
    }
}

/// Explicit user cancellation, distinct from a callback read abort or panic.
#[derive(Debug)]
pub struct UserCancelled {
    pub exit_game: bool,
    pub cause: Option<RuntimeError>,
}

impl fmt::Display for UserCancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cancelled by user ({})",
            if self.exit_game { "exit game" } else { "unload script" }
        )?;
        if let Some(cause) = &self.cause {
            write!(f, ": {cause}")?;
        }
        Ok(())
    }
}
impl Error for UserCancelled {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_ref().map(|error| error as _)
    }
}

/// A terminal cleanup failure alongside, rather than instead of, the original error.
#[derive(Debug)]
pub struct CleanupFailure {
    pub primary: Option<RuntimeError>,
    pub cleanup: RuntimeError,
}

impl fmt::Display for CleanupFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(primary) = &self.primary {
            write!(f, "{primary}; ")?;
        }
        write!(f, "cleanup failed: {}", self.cleanup)
    }
}
impl Error for CleanupFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.primary.as_ref().unwrap_or(&self.cleanup))
    }
}
