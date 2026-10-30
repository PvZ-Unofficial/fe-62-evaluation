//! Backend-neutral logging state for the current runtime thread.

use rsvz_current::CurrentBackend;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use rsvz_model::RelativeTime;

/// Presentation severity for one runtime log record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Debug => "调试",
            Self::Info => "信息",
            Self::Warning => "警告",
            Self::Error => "错误",
        })
    }
}

/// Where a runtime log record originated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogContext {
    Registration,
    Runtime(RelativeTime),
    Unscoped,
}

/// A structured message passed to the installed [`Logger`].
#[derive(Clone, Copy, Debug)]
pub struct LogRecord<'a> {
    pub level: LogLevel,
    pub message: &'a str,
    pub message_truncated: bool,
    pub context: LogContext,
    /// Original operation error, available only while delivering this record.
    pub error: Option<&'a (dyn std::error::Error + 'static)>,
}

impl<'a> LogRecord<'a> {
    #[must_use]
    pub const fn new(level: LogLevel, message: &'a str, context: LogContext) -> Self {
        Self {
            level,
            message,
            message_truncated: false,
            context,
            error: None,
        }
    }
}

impl fmt::Display for LogRecord<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.context {
            LogContext::Registration => write!(formatter, "[注册期][{}] {}", self.level, self.message),
            LogContext::Runtime(time) => {
                write!(
                    formatter,
                    "[第 {} 波, {}cs][{}] {}",
                    time.wave.0, time.time, self.level, self.message
                )
            }
            LogContext::Unscoped => write!(formatter, "[{}] {}", self.level, self.message),
        }
    }
}

/// Destination for structured runtime log records.
pub trait Logger: 'static {
    fn log(&self, record: &LogRecord<'_>);
}

impl<F> Logger for F
where
    F: for<'a> Fn(&LogRecord<'a>) + 'static,
{
    fn log(&self, record: &LogRecord<'_>) {
        self(record);
    }
}

/// Opaque ownership of a previously installed logger.
pub struct LoggerHandle(Rc<dyn Logger>);

impl LoggerHandle {
    fn new(logger: impl Logger) -> Self {
        Self(Rc::new(logger))
    }
}

struct DefaultLogger;

impl Logger for DefaultLogger {
    fn log(&self, record: &LogRecord<'_>) {
        output::present(record);
    }
}

std::thread_local! {
    static LOGGER: RefCell<LoggerHandle> = RefCell::new(LoggerHandle::new(DefaultLogger));
    static LOGGER_FAULTED: Cell<bool> = const { Cell::new(false) };
    static REPORTING: Cell<bool> = const { Cell::new(false) };
}

/// Installs one logger for the current runtime thread and discards the previous logger.
pub fn set_logger(logger: impl Logger) {
    let _previous = replace_logger(logger);
}

/// Restores the backend's default logger for the current runtime thread.
pub fn reset_logger() {
    let _previous = replace_logger(DefaultLogger);
}

/// Installs one logger and returns ownership of the previous logger.
#[must_use]
pub fn replace_logger(logger: impl Logger) -> LoggerHandle {
    LOGGER_FAULTED.set(false);
    LOGGER.with_borrow_mut(|slot| std::mem::replace(slot, LoggerHandle::new(logger)))
}

/// Restores a logger returned by [`replace_logger`].
/// If thread teardown has already destroyed the logger slot, releases the saved logger instead.
pub fn restore_logger(logger: LoggerHandle) {
    let _ = LOGGER.try_with(|slot| {
        *slot.borrow_mut() = logger;
        LOGGER_FAULTED.set(false);
    });
}

/// Delivers one record synchronously without changing runtime failure state.
pub fn emit_log(record: &LogRecord<'_>) {
    emit_log_inner(record, false);
}

fn emit_log_inner(record: &LogRecord<'_>, reporting_failure: bool) {
    let nested = REPORTING.with(|reporting| reporting.replace(true));
    if nested {
        emergency_output(&record.to_string());
        return;
    }
    let _guard = ReportingGuard;
    output::transport(record);
    let logger = LOGGER.with_borrow(|slot| Rc::clone(&slot.0));
    if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| logger.log(record))) {
        LOGGER_FAULTED.set(true);
        if rsvz_schedule::callback::is_abort(&*payload) {
            if !reporting_failure {
                panic::resume_unwind(payload);
            }
            // Preserve the original failure when its logger encounters another failure.
            emergency_output(&record.to_string());
            return;
        }
        rsvz_schedule::timeline::resume_timeline_termination(payload);
        emergency_output(&record.to_string());
    }
}

/// Emits one unscoped error message through the installed logger.
#[doc(hidden)]
pub fn report_message(message: &str) {
    emit_log(&LogRecord::new(LogLevel::Error, message, LogContext::Unscoped));
}

struct ReportingGuard;

impl Drop for ReportingGuard {
    fn drop(&mut self) {
        REPORTING.set(false);
    }
}

fn emergency_output(message: &str) {
    CurrentBackend::diagnostic_fallback(message);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use rsvz_model::{RelativeTime, Wave};

    use super::*;

    #[test]
    fn logger_restore_survives_tls_destruction() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };

        struct RestoreOnDrop(LoggerHandle, Arc<AtomicBool>, Arc<AtomicBool>);
        impl Drop for RestoreOnDrop {
            fn drop(&mut self) {
                self.1.store(LOGGER.try_with(|_| ()).is_err(), Ordering::SeqCst);
                let saved = LoggerHandle(Rc::clone(&self.0.0));
                let restored = panic::catch_unwind(AssertUnwindSafe(|| restore_logger(saved))).is_ok();
                self.2.store(restored, Ordering::SeqCst);
            }
        }
        thread_local! {
            static SAVED: RefCell<Option<RestoreOnDrop>> = const { RefCell::new(None) };
        }
        let destroyed = Arc::new(AtomicBool::new(false));
        let restored = Arc::new(AtomicBool::new(false));
        let flags = (Arc::clone(&destroyed), Arc::clone(&restored));
        std::thread::spawn(move || {
            // Like the state-hook registry, this owner is initialized before LOGGER.
            SAVED.with(|_| ());
            let saved = replace_logger(DefaultLogger);
            SAVED.with_borrow_mut(|slot| *slot = Some(RestoreOnDrop(saved, flags.0, flags.1)));
        })
        .join()
        .unwrap();
        assert!(
            destroyed.load(Ordering::SeqCst),
            "the test must restore after LOGGER destruction"
        );
        assert!(
            restored.load(Ordering::SeqCst),
            "restoring a dropped logger must not panic"
        );
    }

    thread_local! {
        static RECORDS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    fn capture(record: &LogRecord<'_>) {
        RECORDS.with_borrow_mut(|records| records.push(record.to_string()));
    }

    #[test]
    fn logger_receives_structured_context_and_level() {
        RECORDS.with_borrow_mut(Vec::clear);
        let previous = replace_logger(capture);
        emit_log(&LogRecord::new(
            LogLevel::Warning,
            "probe",
            LogContext::Runtime(RelativeTime::new(Wave(3), 125)),
        ));
        restore_logger(previous);

        RECORDS.with_borrow(|records| assert_eq!(records.as_slice(), ["[第 3 波, 125cs][警告] probe"]));
    }

    #[test]
    fn closures_can_capture_logger_state() {
        let captured = Rc::new(RefCell::new(Vec::new()));
        let target = Rc::clone(&captured);
        let previous = replace_logger(move |record: &LogRecord<'_>| {
            target.borrow_mut().push(record.message.to_owned());
        });
        emit_log(&LogRecord::new(LogLevel::Info, "captured", LogContext::Unscoped));
        restore_logger(previous);

        assert_eq!(captured.borrow().as_slice(), ["captured"]);
    }

    #[test]
    fn boxed_callbacks_are_loggers_too() {
        let captured = Rc::new(RefCell::new(Vec::new()));
        let target = Rc::clone(&captured);
        let logger: Box<dyn for<'a> Fn(&LogRecord<'a>)> = Box::new(move |record| {
            target.borrow_mut().push(record.level);
        });
        let previous = replace_logger(logger);
        emit_log(&LogRecord::new(LogLevel::Debug, "boxed", LogContext::Unscoped));
        restore_logger(previous);

        assert_eq!(captured.borrow().as_slice(), [LogLevel::Debug]);
    }
}

std::thread_local! {
    static REPORT_TIME: Cell<Option<RelativeTime>> = const { Cell::new(None) };
}

/// Returns the current battle time used to annotate recoverable script errors.
///
/// The value is cached before callbacks run, so reporting never re-borrows the
/// backend or Timeline from inside a callback.
#[doc(hidden)]
#[must_use]
pub fn runtime_report_time() -> Option<RelativeTime> {
    REPORT_TIME.get()
}

pub fn report_runtime_error(error: impl Into<crate::runtime::RuntimeError>) {
    let error = error.into();
    let (message, truncated) = bounded_message(&error);
    let context = runtime_report_time().map_or(LogContext::Unscoped, LogContext::Runtime);
    let mut record = LogRecord::new(LogLevel::Error, &message, context);
    record.error = Some(&error);
    record.message_truncated = truncated;
    emit_log_inner(&record, true);
}

struct RuntimeReportTimeGuard(Option<RelativeTime>);

impl Drop for RuntimeReportTimeGuard {
    fn drop(&mut self) {
        REPORT_TIME.set(self.0);
    }
}

pub fn with_runtime_report_time<R>(time: Option<RelativeTime>, f: impl FnOnce() -> R) -> R {
    let _guard = RuntimeReportTimeGuard(REPORT_TIME.replace(time));
    f()
}

#[doc(hidden)]
pub fn clear_report_time() {
    REPORT_TIME.set(None);
}

/// Reports an operation failure and records it for dispatch/measurement policy.
pub fn report_operation_error(error: impl Into<crate::runtime::RuntimeError>) {
    report_operation_error_ref(&error.into());
}

pub(crate) fn report_operation_error_ref(error: &(dyn std::error::Error + 'static)) {
    crate::session::record_dispatch_outcome(crate::session::DispatchOutcome::RecoverableError);

    let (message, truncated) = bounded_message(&error);
    let mut record = LogRecord::new(LogLevel::Error, &message, current_context());
    record.error = Some(error);
    record.message_truncated = truncated;
    emit_log_inner(&record, true);
}

/// Stops the nearest callback on an operation failure that cannot return a value.
/// Its execution boundary reports the error after access and component guards unwind.
#[doc(hidden)]
pub fn abort_operation(error: crate::runtime::RuntimeError) -> ! {
    rsvz_schedule::callback::abort(error)
}

/// Writes an Error-level diagnostic without changing callback or measurement state.
pub fn error(message: impl fmt::Display) {
    log(LogLevel::Error, message);
}

pub fn log(level: LogLevel, message: impl fmt::Display) {
    let (message, truncated) = bounded_message(&message);
    let mut record = LogRecord::new(level, &message, current_context());
    record.message_truncated = truncated;
    emit_log(&record);
}

pub fn replace_error_reporter(reporter: fn(&str)) -> LoggerHandle {
    replace_logger(move |record: &LogRecord<'_>| reporter(&record.to_string()))
}

fn current_context() -> LogContext {
    if let Some(time) = runtime_report_time() {
        return LogContext::Runtime(time);
    }

    if crate::registration::is_active() {
        LogContext::Registration
    } else {
        LogContext::Unscoped
    }
}

mod output;
mod structured;
pub use output::{DiagnosticLevel, DiagnosticMode, DiagnosticOptions, configure, progress, progress_due};
pub(crate) use output::{begin, boundary, cancelled, cleaning, exit_requested, finish, measurement, note_clock};

/// Cumulative completed counts; the active sample is reported separately.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct MeasurementProgress {
    pub measurement: &'static str,
    pub completed: u64,
    pub attempted: u64,
    pub valid: u64,
    pub invalid: u64,
    pub incomplete: u64,
    pub active: u64,
    pub remaining: Option<u64>,
    pub successes: u64,
    pub ratio_name: &'static str,
    pub ratio_numerator: u64,
    pub ratio_denominator: u64,
    pub damage: i64,
    pub completed_rounds: u64,
}

fn bounded_message(value: &impl fmt::Display) -> (String, bool) {
    struct Message {
        text: String,
        truncated: bool,
    }
    impl fmt::Write for Message {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            let remaining = 8192usize.saturating_sub(self.text.len());
            if text.len() <= remaining {
                self.text.push_str(text);
                return Ok(());
            }
            let mut end = remaining;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push_str(&text[..end]);
            self.truncated = true;
            Err(fmt::Error)
        }
    }
    let mut output = Message {
        text: String::with_capacity(256),
        truncated: false,
    };
    let _ = fmt::write(&mut output, format_args!("{value}"));
    (output.text, output.truncated)
}

pub(crate) fn report_terminal_error(error: &crate::runtime::RuntimeError) {
    if LOGGER_FAULTED.get() {
        let (message, truncated) = bounded_message(error);
        let mut record = LogRecord::new(LogLevel::Error, &message, current_context());
        record.error = Some(error);
        record.message_truncated = truncated;
        output::transport(&record);
        return;
    }
    // A faulty presentation callback cannot interrupt already-started teardown.
    if panic::catch_unwind(AssertUnwindSafe(|| report_runtime_error(error))).is_err() {
        emergency_output(
            "terminal diagnostic presentation failed; preserving the original failure and continuing cleanup",
        );
    }
}

/// Teardown diagnostics must not reopen a modal dialog or invoke user code.
pub(crate) fn report_cleanup_error(error: &crate::runtime::RuntimeError) {
    let (message, truncated) = bounded_message(error);
    let mut record = LogRecord::new(LogLevel::Error, &message, current_context());
    record.error = Some(error);
    record.message_truncated = truncated;
    output::transport(&record);
}
