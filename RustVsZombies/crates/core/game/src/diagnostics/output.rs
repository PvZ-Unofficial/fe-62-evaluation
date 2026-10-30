//! Configuration, bounded JSONL encoding and presentation policy for one execution.
use super::{LogLevel, LogRecord};
use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::diagnostics::{DiagnosticLaunch, DialogAction, RecordBuffer};
pub use rsvz_backend_api::diagnostics::{DiagnosticLevel, DiagnosticMode, DiagnosticOptions};
use rsvz_current::CurrentBackend;
use serde::Serialize;
use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};

thread_local! {
    static OUTPUT: RefCell<Option<Output>> = const { RefCell::new(None) };
    static DISPATCH: Cell<u64> = const { Cell::new(0) };
    static CLEANING: Cell<bool> = const { Cell::new(false) };
    static CLOCK: Cell<Option<i32>> = const { Cell::new(None) };
    static CONTROL: Cell<u32> = const { Cell::new(0) };
}
struct Repeat {
    key: u64,
    count: u64,
    first_sequence: u64,
    began: Instant,
    first_time: Option<rsvz_model::RelativeTime>,
    last_time: Option<rsvz_model::RelativeTime>,
}
struct Output {
    launch: DiagnosticLaunch,
    script: DiagnosticOptions,
    options: DiagnosticOptions,
    measurement: bool,
    buffer: RecordBuffer,
    sequence: u64,
    last_truncated: bool,
    opened: bool,
    failed: bool,
    mute_run: bool,
    mute_frame: Option<(u64, u64, Option<i32>)>,
    repeat: Option<Repeat>,
    last_message: String,
    progress_at: Option<Instant>,
    finished: bool,
}
impl Output {
    fn new() -> Self {
        let mut launch = CurrentBackend::diagnostic_launch();
        if launch.run_id.is_empty() {
            let defaults = DiagnosticLaunch::standalone();
            launch.run_id = defaults.run_id;
            if launch.default_file.as_os_str().is_empty() {
                launch.default_file = defaults.default_file;
            }
            if launch.source.is_empty() {
                launch.source = defaults.source;
            }
        }
        if let Some(execution) = crate::modules::current_execution(rsvz_schedule::module::current_module()) {
            launch.source = format!("{}-{}", launch.source, execution.generation);
        }
        let options = launch.options.clone();
        Self {
            launch,
            options,
            script: DiagnosticOptions::default(),
            measurement: false,
            buffer: RecordBuffer::default(),
            sequence: 0,
            last_truncated: false,
            opened: false,
            failed: false,
            mute_run: false,
            mute_frame: None,
            repeat: None,
            last_message: String::with_capacity(8192),
            progress_at: None,
            finished: false,
        }
    }
    fn mode(&self) -> DiagnosticMode {
        match self.options.mode.unwrap_or_default() {
            DiagnosticMode::Auto if !self.measurement && CurrentBackend::diagnostic_dialog_supported() => {
                DiagnosticMode::Dialog
            }
            DiagnosticMode::Auto | DiagnosticMode::Cli if !self.launch.following => DiagnosticMode::File,
            DiagnosticMode::Auto => DiagnosticMode::Cli,
            other => other,
        }
    }
    fn write<T: Serialize>(&mut self, kind: &str, payload: &T) {
        if self.finished {
            return;
        }
        if !self.opened {
            self.opened = true;
            let path = self.launch.path(&self.options);
            if let Err(error) =
                std::path::absolute(&path).and_then(|path| CurrentBackend::diagnostic_open(&self.launch, &path))
            {
                self.failed = true;
                CurrentBackend::diagnostic_incomplete();
                CurrentBackend::diagnostic_fallback(&format!(
                    "cannot open diagnostic file {}: {error}",
                    path.display()
                ));
            } else if !self.launch.following {
                CurrentBackend::diagnostic_fallback(&format!("rsvz diagnostic log: {}", path.display()));
            }
        }
        self.sequence += 1;
        self.last_truncated = false;
        self.buffer.clear();
        #[derive(Serialize)]
        struct Record<'a, T> {
            version: u32,
            run_id: &'a str,
            source: &'a str,
            worker: usize,
            sequence: u64,
            kind: &'a str,
            world_epoch: u64,
            logic_clock: Option<i32>,
            game_time: Option<(i32, i32)>,
            payload: &'a T,
        }
        let record = Record {
            version: 1,
            run_id: &self.launch.run_id,
            source: &self.launch.source,
            worker: self.launch.worker,
            sequence: self.sequence,
            kind,
            world_epoch: crate::session::world_epoch(),
            logic_clock: CLOCK.get(),
            game_time: super::runtime_report_time().map(|time| (time.wave.0, time.time)),
            payload,
        };
        if serde_json::to_writer(&mut self.buffer, &record).is_err() {
            self.last_truncated = true;
            CurrentBackend::diagnostic_incomplete();
            self.buffer.clear();
            let lost = Record {
                version: 1,
                run_id: &self.launch.run_id,
                source: &self.launch.source,
                worker: self.launch.worker,
                sequence: self.sequence,
                kind: "diagnostic_truncated",
                world_epoch: crate::session::world_epoch(),
                logic_clock: CLOCK.get(),
                game_time: None,
                payload: &"record exceeded 64 KiB or serialization failed",
            };
            let _ = serde_json::to_writer(&mut self.buffer, &lost);
        }
        if let Err(error) = CurrentBackend::diagnostic_write(self.sequence, self.buffer.bytes()) {
            if !self.failed {
                CurrentBackend::diagnostic_fallback(&format!("diagnostic write failed: {error}"));
            }
            self.failed = true;
        }
    }
    fn flush_repeat(&mut self) {
        let Some(repeat) = self.repeat.take() else {
            return;
        };
        if repeat.count <= 1 {
            return;
        }
        #[derive(Serialize)]
        struct Summary<'a> {
            first_sequence: u64,
            repeat_count: u64,
            first_time: Option<(i32, i32)>,
            last_time: Option<(i32, i32)>,
            last_observation: &'a str,
            summarized: bool,
        }
        // Move the reusable observation string while writing; no per-summary copy.
        let message = std::mem::take(&mut self.last_message);
        self.write(
            "repeat_summary",
            &Summary {
                first_sequence: repeat.first_sequence,
                repeat_count: repeat.count,
                first_time: repeat.first_time.map(|t| (t.wave.0, t.time)),
                last_time: repeat.last_time.map(|t| (t.wave.0, t.time)),
                last_observation: &message,
                summarized: true,
            },
        );
        self.last_message = message;
    }
}
fn with_output<R>(f: impl FnOnce(&mut Output) -> R) -> R {
    OUTPUT.with_borrow_mut(|state| f(state.get_or_insert_with(Output::new)))
}

/// Applies script options field by field. Explicit launch options take precedence.
/// Once output starts its file cannot change; other options can change during a run.
pub fn configure(options: DiagnosticOptions) -> RuntimeResult<()> {
    with_output(|state| {
        if state.finished {
            return Err(RuntimeError::new("diagnostic execution already finished"));
        }
        if options
            .log_file
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err(RuntimeError::new("diagnostic log_file must not be empty"));
        }
        let mut script = state.script.clone();
        script.overlay(&options);
        let mut merged = script.clone();
        merged.overlay(&state.launch.options);
        if merged.mode == Some(DiagnosticMode::Dialog) && !CurrentBackend::diagnostic_dialog_supported() {
            return Err(RuntimeError::new(
                "Dialog is unavailable on this backend; use Cli or File",
            ));
        }
        if state.opened && state.launch.path(&merged) != state.launch.path(&state.options) {
            return Err(RuntimeError::new(
                "log_file cannot change after diagnostic output starts",
            ));
        }
        state.script = script;
        state.options = merged;
        Ok(())
    })
}
pub(crate) fn begin() {
    CurrentBackend::diagnostic_reset();
    OUTPUT.with_borrow_mut(|state| *state = None);
    CONTROL.set(0);
    CLEANING.set(false);
    DISPATCH.set(0);
}
pub(crate) fn boundary() {
    CLOCK.set(None);
    DISPATCH.set(DISPATCH.get().wrapping_add(1));
    OUTPUT.with_borrow_mut(|state| {
        if let Some(state) = state {
            if state
                .repeat
                .as_ref()
                .is_some_and(|r| r.began.elapsed() >= Duration::from_millis(500))
            {
                state.flush_repeat();
            }
        }
    });
}
pub(crate) fn measurement() {
    with_output(|state| state.measurement = true);
}
pub fn progress_due(force: bool) -> bool {
    with_output(|state| {
        let interval = state.options.progress_interval_ms.unwrap_or(500);
        !state.finished
            && (force
                || interval != 0
                    && state
                        .progress_at
                        .is_none_or(|at| at.elapsed() >= Duration::from_millis(interval)))
    })
}
/// Publish a small cumulative snapshot from the component's existing counters.
pub fn progress(snapshot: &impl Serialize, force: bool) {
    if !progress_due(force) {
        return;
    }
    with_output(|state| {
        state.progress_at = Some(Instant::now());
        state.write("progress", snapshot);
    });
}
pub(crate) fn transport(record: &LogRecord<'_>) {
    with_output(|state| {
        if state.finished {
            return;
        }
        let level = match record.level {
            LogLevel::Debug => DiagnosticLevel::Debug,
            LogLevel::Info => DiagnosticLevel::Info,
            LogLevel::Warning => DiagnosticLevel::Warning,
            LogLevel::Error => DiagnosticLevel::Error,
        };
        if level < state.options.level.unwrap_or(DiagnosticLevel::Info) && record.error.is_none() {
            return;
        }
        let key = super::structured::key(record);
        let now = Instant::now();
        let time = super::runtime_report_time();
        let same = state
            .repeat
            .as_ref()
            .is_some_and(|r| r.key == key && r.began.elapsed() < Duration::from_millis(500));
        if same {
            let repeat = state.repeat.as_mut().unwrap();
            repeat.count += 1;
            repeat.last_time = time;
            state.last_message.clear();
            let mut end = record.message.len().min(8000);
            while !record.message.is_char_boundary(end) {
                end -= 1;
            }
            state.last_message.push_str(&record.message[..end]);
            if end < record.message.len() {
                state.last_message.push_str(" [summary observation truncated]");
            }
            return;
        }
        state.flush_repeat();
        state.write(
            if record.error.is_some() {
                "operation_error"
            } else {
                "log"
            },
            &super::structured::Payload(record),
        );
        let truncated = state.last_truncated;
        if truncated {
            state.write("operation_error_summary", &super::structured::Summary(record));
        }
        if record.message_truncated && !truncated {
            CurrentBackend::diagnostic_incomplete();
        }
        state.repeat = Some(Repeat {
            key,
            count: 1,
            first_sequence: state.sequence,
            began: now,
            first_time: time,
            last_time: time,
        });
    });
}
pub(crate) fn present(record: &LogRecord<'_>) {
    let frame = (
        crate::session::world_epoch(),
        if CLOCK.get().is_some() { 0 } else { DISPATCH.get() },
        CLOCK.get(),
    );
    let dialog = with_output(|state| {
        if state.finished {
            return false;
        }
        let level = match record.level {
            LogLevel::Debug => DiagnosticLevel::Debug,
            LogLevel::Info => DiagnosticLevel::Info,
            LogLevel::Warning => DiagnosticLevel::Warning,
            LogLevel::Error => DiagnosticLevel::Error,
        };
        (record.error.is_some() || level >= state.options.level.unwrap_or(DiagnosticLevel::Info))
            && state.mode() == DiagnosticMode::Dialog
            && !state.mute_run
            && state.mute_frame != Some(frame)
    });
    if !dialog || !matches!(record.level, LogLevel::Warning | LogLevel::Error) {
        return;
    }
    let message = with_output(|state| format!("来源：{}\n{record}", state.launch.source));
    match CurrentBackend::diagnostic_dialog(&message) {
        DialogAction::Continue => {}
        DialogAction::MuteFrame => with_output(|state| state.mute_frame = Some(frame)),
        DialogAction::MuteRun => with_output(|state| state.mute_run = true),
        DialogAction::Stop => control(false),
        DialogAction::Exit => control(true),
    }
}
fn control(exit: bool) {
    CONTROL.set(CONTROL.get().max(if exit { 2 } else { 1 }));
    if exit {
        for source in [
            rsvz_schedule::module::ModuleSlot::Script,
            rsvz_schedule::module::ModuleSlot::Modifier,
        ] {
            if let Some(id) = crate::modules::current_execution(source) {
                let _ = crate::modules::request_stop(id);
            }
        }
    }
    if let Some(id) = crate::modules::current_execution(rsvz_schedule::module::current_module()) {
        let _ = crate::modules::request_unload(id);
    }
    crate::session::fail_script(RuntimeError::from_source(rsvz_backend_api::error::UserCancelled {
        exit_game: exit,
        cause: None,
    }));
    if !CLEANING.get() {
        rsvz_schedule::timeline::terminate_timeline_callback();
    }
}
pub(crate) fn cleaning() {
    CLEANING.set(true);
}
pub(crate) fn cancelled() -> bool {
    CONTROL.get() != 0
}
pub(crate) fn finish() {
    OUTPUT.with_borrow_mut(|state| {
        if let Some(state) = state {
            if state.finished {
                return;
            }
            state.flush_repeat();
            state.write("diagnostics_end", &serde_json::json!({"cancelled": CONTROL.get() != 0}));
            state.finished = true;
            state.buffer.release();
            state.last_message = String::new();
        }
    });
    let _ = CurrentBackend::diagnostic_finish();
    if CONTROL.get() == 2 {
        CurrentBackend::diagnostic_exit();
    }
}

pub(crate) fn note_clock(clock: Option<i32>) {
    CLOCK.set(clock);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn script_relative_paths_are_published_as_absolute_paths() {
        begin();
        configure(DiagnosticOptions {
            mode: Some(DiagnosticMode::File),
            log_file: Some("../../../target/logs/plan38-review-relative".into()),
            ..Default::default()
        })
        .unwrap();
        super::super::error("relative path probe");
        finish();
        let end = CurrentBackend::diagnostic_finish();
        assert!(end.path.is_absolute());
        with_output(|state| {
            let location = rsvz_backend_api::diagnostics::source_path(
                &state.launch.default_file,
                &state.launch.run_id,
                &state.launch.source,
                state.launch.worker,
            )
            .with_extension("location.json");
            let published: std::path::PathBuf = serde_json::from_slice(&std::fs::read(location).unwrap()).unwrap();
            assert_eq!(published, end.path);
        });
        assert!(
            std::fs::read_to_string(&end.path)
                .unwrap()
                .contains("relative path probe")
        );
    }
    #[test]
    fn explicit_launch_fields_override_only_the_corresponding_script_fields() {
        begin();
        with_output(|state| {
            state.launch.options.mode = Some(DiagnosticMode::Auto);
            state.launch.options.progress_interval_ms = Some(42);
        });
        configure(DiagnosticOptions {
            mode: Some(DiagnosticMode::File),
            level: Some(DiagnosticLevel::Debug),
            progress_interval_ms: Some(3),
            ..Default::default()
        })
        .unwrap();
        with_output(|state| {
            assert_eq!(state.options.mode, Some(DiagnosticMode::Auto));
            assert_eq!(state.options.level, Some(DiagnosticLevel::Debug));
            assert_eq!(state.options.progress_interval_ms, Some(42));
        });
    }
    #[test]
    fn stop_unwinds_to_the_execution_boundary_but_cleanup_merges_requests() {
        begin();
        crate::session::reset_session_control();
        let reached = Cell::new(false);
        let signal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            control(false);
            reached.set(true);
        }))
        .unwrap_err();
        assert!(rsvz_schedule::timeline::is_timeline_termination(&*signal));
        assert!(!reached.get());
        assert!(cancelled());
        cleaning();
        control(false);
        control(true);
        control(false);
        assert_eq!(CONTROL.get(), 2);
        assert!(crate::session::script_stop_requested());
        crate::session::reset_session_control();
    }
    #[test]
    fn logger_replacement_preserves_transport_and_repeats_are_counted_once() {
        begin();
        configure(DiagnosticOptions {
            mode: Some(DiagnosticMode::File),
            ..Default::default()
        })
        .unwrap();
        let calls = std::rc::Rc::new(Cell::new(0));
        let captured = calls.clone();
        let saved = super::super::replace_logger(move |_: &LogRecord<'_>| captured.set(captured.get() + 1));
        for _ in 0..5 {
            super::super::report_operation_error(RuntimeError::new("repeat probe"));
        }
        super::super::restore_logger(saved);
        finish();
        let end = CurrentBackend::diagnostic_finish();
        let text = std::fs::read_to_string(&end.path).unwrap();
        let records: Vec<serde_json::Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(records.iter().filter(|r| r["kind"] == "operation_error").count(), 1);
        assert_eq!(
            records.iter().find(|r| r["kind"] == "repeat_summary").unwrap()["payload"]["repeat_count"],
            5
        );
        assert_eq!(calls.get(), 5);
        assert_eq!(records.last().unwrap()["sequence"], end.final_sequence);
        assert!(end.complete);
    }
    #[test]
    fn zero_interval_disables_only_periodic_progress() {
        begin();
        configure(DiagnosticOptions {
            progress_interval_ms: Some(0),
            ..Default::default()
        })
        .unwrap();
        assert!(!progress_due(false));
        assert!(progress_due(true));
    }
}

pub(crate) fn exit_requested() -> bool {
    CONTROL.get() == 2
}

#[cfg(test)]
mod size_tests {
    use super::*;

    #[cfg(all(test, feature = "backend-tests"))]
    #[test]
    fn oversized_card_and_shovel_batches_keep_structured_summaries() {
        use crate::logic::shovel::{ShovelBatchError, ShovelError, ShovelFailure};
        use crate::model::{CardSelection, GridError, PlantKind};
        begin();
        configure(DiagnosticOptions {
            mode: Some(DiagnosticMode::File),
            ..Default::default()
        })
        .unwrap();
        let cards = crate::logic::cards::cards(std::iter::repeat_n(
            (CardSelection::Plant(PlantKind::IceShroom), 0, 1),
            1000,
        ))
        .unwrap_err();
        super::super::report_operation_error(RuntimeError::from(cards).context("card batch"));
        let shovel = ShovelBatchError {
            failures: (1..=2000)
                .map(|input_number| ShovelFailure {
                    input_number,
                    error: ShovelError::InvalidGrid {
                        coordinates: Some((0, 1)),
                        cause: GridError::NegativeCoordinate,
                    },
                })
                .collect(),
        };
        super::super::report_operation_error(RuntimeError::from(shovel).context("shovel batch"));
        finish();
        let end = CurrentBackend::diagnostic_finish();
        assert!(!end.complete);
        assert_eq!(end.dropped, 2);
        let text = std::fs::read_to_string(&end.path).unwrap();
        assert!(
            text.lines()
                .all(|line| line.len() <= rsvz_backend_api::diagnostics::MAX_RECORD_BYTES)
        );
        let records: Vec<serde_json::Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        let summaries: Vec<_> = records
            .iter()
            .filter(|record| record["kind"] == "operation_error_summary")
            .map(|record| &record["payload"])
            .collect();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0]["details_truncated"], true);
        assert_eq!(summaries[0]["card_processed"], 1000);
        assert_eq!(summaries[0]["card_failure_count"], 1000);
        assert_eq!(summaries[0]["card_first_failure"][0], 1);
        assert_eq!(summaries[0]["card_last_failure"][0], 1000);
        assert_eq!(summaries[0]["card_last_failure"][1]["code"], "invalid_grid");
        assert_eq!(summaries[1]["shovel_failure_count"], 2000);
        assert_eq!(summaries[1]["shovel_first_failure"][0], 1);
        assert_eq!(summaries[1]["shovel_last_failure"][0], 2000);
    }

    #[test]
    fn oversized_batch_retains_counts_and_an_honest_prefix() {
        begin();
        configure(DiagnosticOptions {
            mode: Some(DiagnosticMode::File),
            ..Default::default()
        })
        .unwrap();
        let failures = (0..1000)
            .map(|index| {
                let mut error = crate::cob::CobError::new("发炮", crate::cob::CobManagerError::EmptyList);
                error.input_number = index + 1;
                error
            })
            .collect();
        let error = crate::cob::CobBatchError {
            report: crate::cob::CobBatchReport {
                indices: vec![None; 1000],
                processed: 1000,
                accepted: 0,
                total: Some(1000),
                complete: true,
                failures,
                stopped_at: None,
            },
        };
        super::super::report_operation_error(error);
        finish();
        let end = CurrentBackend::diagnostic_finish();
        assert!(!end.complete);
        assert_eq!(end.dropped, 1);
        let text = std::fs::read_to_string(&end.path).unwrap();
        assert!(
            text.lines()
                .all(|line| line.len() <= rsvz_backend_api::diagnostics::MAX_RECORD_BYTES)
        );
        let records: Vec<serde_json::Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        let summary = &records
            .iter()
            .find(|record| record["kind"] == "operation_error_summary")
            .unwrap()["payload"];
        assert_eq!(summary["processed"], 1000);
        assert_eq!(summary["failure_count"], 1000);
        assert_eq!(summary["indices_prefix"].as_array().unwrap().len(), 64);
        assert_eq!(summary["indices_truncated"], true);
    }
}
