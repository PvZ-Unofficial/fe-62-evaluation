//! CLI-only bounded diagnostic following, shared by the three concrete launchers.
use anyhow::{Result, bail};
use rsvz_backend_api::diagnostics::{
    DiagnosticEnd, DiagnosticLaunch, DiagnosticLevel, DiagnosticMode, DiagnosticOptions, MAX_READ_BYTES,
    MAX_RECORD_BYTES,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[derive(clap::Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticArgs {
    #[arg(long, value_parser = mode)]
    diagnostics: Option<DiagnosticMode>,
    #[arg(long)]
    log_file: Option<PathBuf>,
    #[arg(long, value_parser = level)]
    log_level: Option<DiagnosticLevel>,
    #[arg(long)]
    progress_interval_ms: Option<u64>,
    #[arg(long)]
    pub stream_json: bool,
}
fn mode(text: &str) -> Result<DiagnosticMode, String> {
    match text {
        "auto" => Ok(DiagnosticMode::Auto),
        "dialog" => Ok(DiagnosticMode::Dialog),
        "cli" => Ok(DiagnosticMode::Cli),
        "file" => Ok(DiagnosticMode::File),
        _ => Err("expected auto, dialog, cli or file".into()),
    }
}
fn level(text: &str) -> Result<DiagnosticLevel, String> {
    match text {
        "debug" => Ok(DiagnosticLevel::Debug),
        "info" => Ok(DiagnosticLevel::Info),
        "warning" | "warn" => Ok(DiagnosticLevel::Warning),
        "error" => Ok(DiagnosticLevel::Error),
        _ => Err("expected debug, info, warning or error".into()),
    }
}
impl DiagnosticArgs {
    pub fn validate(&self, no_wait: bool, json: bool, dialog_supported: bool) -> Result<()> {
        if self.log_file.as_ref().is_some_and(|path| path.as_os_str().is_empty()) {
            bail!("--log-file must not be empty");
        }
        if no_wait && (self.stream_json || self.diagnostics == Some(DiagnosticMode::Cli)) {
            bail!("--no-wait conflicts with CLI/streaming diagnostics");
        }
        if self.stream_json && (json || self.diagnostics == Some(DiagnosticMode::Dialog)) {
            bail!("--stream-json conflicts with --json and --diagnostics dialog");
        }
        if !dialog_supported && self.diagnostics == Some(DiagnosticMode::Dialog) {
            bail!("Dialog is unavailable on PE; choose cli or file");
        }
        Ok(())
    }
    pub fn launch(&self, run: &str, base: &Path, cwd: &Path, no_wait: bool) -> DiagnosticLaunch {
        DiagnosticLaunch {
            run_id: run.into(),
            source: "script".into(),
            worker: 0,
            following: !no_wait && (self.stream_json || self.diagnostics != Some(DiagnosticMode::File)),
            default_file: self
                .log_file
                .as_ref()
                .map_or_else(|| base.to_owned(), |path| cwd.join(path)),
            options: DiagnosticOptions {
                mode: if self.stream_json {
                    Some(DiagnosticMode::Cli)
                } else {
                    self.diagnostics
                },
                log_file: self.log_file.as_ref().map(|p| cwd.join(p)),
                level: self.log_level,
                progress_interval_ms: self.progress_interval_ms,
            },
        }
    }
}
struct Reader {
    file: File,
    pending: Vec<u8>,
    discard: bool,
    last: u64,
    incomplete: bool,
}
impl Reader {
    fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            file: File::open(path)?,
            pending: Vec::with_capacity(MAX_RECORD_BYTES),
            discard: false,
            last: 0,
            incomplete: false,
        })
    }
    fn poll(&mut self, budget: &mut usize, mut accept: impl FnMut(Value)) -> Result<()> {
        let mut chunk = [0u8; 8192];
        while *budget > 0 {
            let count = self.file.read(&mut chunk[..(*budget).min(8192)])?;
            if count == 0 {
                break;
            }
            *budget -= count;
            for &byte in &chunk[..count] {
                if byte == b'\n' {
                    if !self.discard {
                        match serde_json::from_slice::<Value>(&self.pending) {
                            Ok(record) => {
                                if let Some(sequence) = record["sequence"].as_u64() {
                                    if sequence != self.last + 1 {
                                        self.incomplete = true;
                                    }
                                    self.last = sequence;
                                    accept(record);
                                } else {
                                    self.incomplete = true;
                                }
                            }
                            Err(_) => self.incomplete = true,
                        }
                    }
                    self.pending.clear();
                    self.discard = false;
                } else if !self.discard {
                    if self.pending.len() == MAX_RECORD_BYTES {
                        self.incomplete = true;
                        self.discard = true;
                        self.pending.clear();
                    } else {
                        self.pending.push(byte);
                    }
                }
            }
        }
        Ok(())
    }
}
pub struct Follower {
    launch: DiagnosticLaunch,
    stream: bool,
    readers: BTreeMap<PathBuf, Reader>,
    latest: BTreeMap<(String, u64), Value>,
    last_discovery: Option<Instant>,
    incomplete: bool,
    ended: bool,
}
impl Follower {
    pub fn new(launch: &DiagnosticLaunch, stream: bool) -> Self {
        Self {
            launch: launch.clone(),
            stream,
            readers: BTreeMap::new(),
            latest: BTreeMap::new(),
            last_discovery: None,
            incomplete: false,
            ended: false,
        }
    }
    fn add(&mut self, path: &Path) -> Result<()> {
        if self.readers.contains_key(path) {
            return Ok(());
        }
        if self.readers.len() >= 256 {
            self.incomplete = true;
            bail!("diagnostic source limit (256) exceeded");
        }
        eprintln!("diagnostic log: {}", path.display());
        self.readers.insert(path.to_owned(), Reader::open(path)?);
        Ok(())
    }
    fn discover(&mut self) -> Result<()> {
        if self
            .last_discovery
            .is_some_and(|at| at.elapsed() < Duration::from_millis(500))
        {
            return Ok(());
        }
        self.last_discovery = Some(Instant::now());
        let directory = self.launch.default_file.parent().unwrap_or(Path::new("."));
        if !directory.exists() {
            return Ok(());
        }
        let prefix = format!(
            "{}.{}.",
            self.launch
                .default_file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            self.launch.run_id
        );
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&prefix) && name.ends_with(".location.json") {
                let mut bytes = Vec::new();
                File::open(entry.path())?
                    .take((MAX_RECORD_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > MAX_RECORD_BYTES {
                    self.incomplete = true;
                    continue;
                }
                // Creation may be observable before the single small write completes.
                if let Ok(path) = serde_json::from_slice::<PathBuf>(&bytes) {
                    self.add(&path)?;
                }
            }
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Result<()> {
        self.poll_bytes().map(|_| ())
    }
    fn poll_bytes(&mut self) -> Result<usize> {
        if let Err(error) = self.discover() {
            self.read_failed(&error);
        }
        let per_source = MAX_READ_BYTES / self.readers.len().max(1);
        let mut consumed = 0;
        let mut records = Vec::new();
        // At most MAX_READ_BYTES of records per poll; no unbounded queue/history.
        for reader in self.readers.values_mut() {
            let mut remaining = per_source;
            if let Err(error) = reader.poll(&mut remaining, |record| records.push(record)) {
                if !reader.incomplete {
                    eprintln!("diagnostic read failed: {error}");
                }
                reader.incomplete = true;
            }
            consumed += per_source - remaining;
        }
        for record in records {
            self.accept(record)?;
        }
        Ok(consumed)
    }
    fn read_failed(&mut self, error: &anyhow::Error) {
        if !self.incomplete {
            eprintln!("diagnostic discovery failed: {error}");
        }
        self.incomplete = true;
    }
    fn accept(&mut self, record: Value) -> Result<()> {
        if record["run_id"].as_str() != Some(&self.launch.run_id)
            || record["version"] != 1
            || !record["source"].is_string()
            || !record["worker"].is_u64()
        {
            self.incomplete = true;
            return Ok(());
        }
        if record["kind"] == "diagnostic_truncated" {
            self.incomplete = true;
        }
        if record["kind"] == "progress" {
            let key = (
                record["source"].as_str().unwrap_or_default().to_owned(),
                record["worker"].as_u64().unwrap_or_default(),
            );
            if self.latest.len() >= 256 && !self.latest.contains_key(&key) {
                self.incomplete = true;
                return Ok(());
            }
            self.latest.insert(key, record["payload"].clone());
        }
        if self.stream {
            serde_json::to_writer(std::io::stdout().lock(), &record)?;
            println!();
            if record["kind"] == "progress" {
                for ((source, measurement), total) in self.totals() {
                    serde_json::to_writer(
                        std::io::stdout().lock(),
                        &serde_json::json!({"kind":"progress_total","source":source,"measurement":measurement,"payload":total}),
                    )?;
                    println!();
                }
            }
        } else if self.launch.following {
            if record["kind"] == "progress" {
                for ((source, measurement), total) in self.totals() {
                    eprintln!(
                        "progress {source}/{measurement}: completed={} valid={} invalid={} active={} incomplete={} damage={} ratio={:?}",
                        total.completed,
                        total.valid,
                        total.invalid,
                        total.active,
                        total.incomplete,
                        total.damage,
                        if total.ratio_denominator == 0 {
                            None
                        } else {
                            Some(total.ratio_numerator as f64 / total.ratio_denominator as f64)
                        }
                    );
                }
            } else if let Some(message) = record["payload"]["message"].as_str() {
                eprintln!("{message}");
            } else if record["kind"] == "repeat_summary" {
                eprintln!("repeat summary: {}", record["payload"]);
            }
        }
        Ok(())
    }
    fn totals(&self) -> BTreeMap<(String, String), Totals> {
        let mut totals = BTreeMap::<(String, String), Totals>::new();
        for ((source, _), value) in &self.latest {
            let total = totals
                .entry((
                    source.clone(),
                    value["measurement"].as_str().unwrap_or("custom").to_owned(),
                ))
                .or_default();
            total.completed += u128::from(value["completed"].as_u64().unwrap_or(0));
            total.valid += u128::from(value["valid"].as_u64().unwrap_or(0));
            total.invalid += u128::from(value["invalid"].as_u64().unwrap_or(0));
            total.active += u128::from(value["active"].as_u64().unwrap_or(0));
            total.incomplete += u128::from(value["incomplete"].as_u64().unwrap_or(0));
            total.damage += i128::from(value["damage"].as_i64().unwrap_or(0));
            total.ratio_numerator += u128::from(value["ratio_numerator"].as_u64().unwrap_or(0));
            total.ratio_denominator += u128::from(value["ratio_denominator"].as_u64().unwrap_or(0));
        }
        totals
    }
    pub fn finish(&mut self, ends: &[DiagnosticEnd], result: &impl serde::Serialize) -> Result<()> {
        if ends.is_empty() {
            self.incomplete = true;
        }
        for end in ends {
            if end.path.as_os_str().is_empty() {
                self.incomplete = true;
                continue;
            }
            if let Err(error) = self.add(&end.path) {
                self.incomplete = true;
                eprintln!("final diagnostic file unavailable {}: {error}", end.path.display());
            }
            if !end.complete || end.dropped != 0 {
                self.incomplete = true;
            }
        }
        self.drain(ends, Duration::from_secs(5));
        for (path, reader) in &self.readers {
            if !ends
                .iter()
                .any(|end| end.path == *path && end.final_sequence == reader.last)
            {
                self.incomplete = true;
            }
            self.incomplete |= reader.incomplete || !reader.pending.is_empty() || reader.discard;
        }
        if self.stream {
            serde_json::to_writer(
                std::io::stdout().lock(),
                &serde_json::json!({"kind":"terminal", "diagnostics_complete": !self.incomplete,
                "physical_unload_verified": false, "result":result}),
            )?;
            println!();
        }
        self.ended = true;
        if self.incomplete {
            if !self.stream {
                eprintln!(
                    "business result (diagnostics incomplete): {}",
                    serde_json::to_string(result)?
                );
            }
            bail!("diagnostic evidence is incomplete (business result remains available)");
        }
        Ok(())
    }
    fn drain(&mut self, ends: &[DiagnosticEnd], idle_timeout: Duration) {
        let mut last_progress = Instant::now();
        loop {
            match self.poll_bytes() {
                Ok(0) => {}
                Ok(_) => {
                    // Finished workers may leave much more than one bounded poll of data.
                    // Drain continuously while bytes arrive, including partial JSON records.
                    last_progress = Instant::now();
                    continue;
                }
                Err(error) => {
                    self.incomplete = true;
                    eprintln!("final diagnostic read failed: {error}");
                    break;
                }
            }
            // Verify EOF as well as the declared boundary, so trailing records/fragments
            // are not silently accepted after final_sequence has been reached.
            if ends
                .iter()
                .all(|end| self.readers.get(&end.path).is_none_or(|r| r.last >= end.final_sequence))
            {
                break;
            }
            if last_progress.elapsed() >= idle_timeout {
                self.incomplete = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn path(name: &str) -> PathBuf {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "rsvz-tail-test-{}-{}-{name}",
            std::process::id(),
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }
    #[test]
    fn tailer_keeps_utf8_fragments_and_does_not_treat_temporary_eof_as_terminal() {
        let path = path("utf8");
        let mut file = File::create(&path).unwrap();
        let bytes = "{\"sequence\":1,\"message\":\"寒冰菇\"}\n".as_bytes();
        let split = bytes.iter().position(|b| *b >= 128).unwrap() + 1;
        file.write_all(&bytes[..split]).unwrap();
        file.flush().unwrap();
        let mut reader = Reader::open(&path).unwrap();
        let mut count = 0;
        reader.poll(&mut MAX_READ_BYTES.clone(), |_| count += 1).unwrap();
        assert_eq!(count, 0);
        assert!(!reader.incomplete);
        file.write_all(&bytes[split..]).unwrap();
        file.flush().unwrap();
        reader
            .poll(&mut MAX_READ_BYTES.clone(), |record| {
                assert_eq!(record["message"], "寒冰菇");
                count += 1;
            })
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(reader.last, 1);
        assert!(reader.pending.is_empty());
        drop(reader);
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn malformed_and_oversized_lines_are_bounded_and_mark_incomplete() {
        let path = path("bounds");
        let mut file = File::create(&path).unwrap();
        file.write_all(&vec![b'x'; MAX_RECORD_BYTES + 10]).unwrap();
        file.write_all(b"\nnot json\n{\"sequence\":3}\n").unwrap();
        file.flush().unwrap();
        let mut reader = Reader::open(&path).unwrap();
        reader.poll(&mut MAX_READ_BYTES.clone(), |_| {}).unwrap();
        assert!(reader.incomplete);
        assert_eq!(reader.last, 3);
        assert!(reader.pending.capacity() <= MAX_RECORD_BYTES);
        drop(reader);
        drop(file);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn launch_preserves_unspecified_mode_and_rejects_incompatible_flags() {
        let mut args = DiagnosticArgs::default();
        assert!(
            args.launch("r", Path::new("log"), Path::new("."), false)
                .options
                .mode
                .is_none()
        );
        args.diagnostics = Some(DiagnosticMode::Cli);
        assert!(args.validate(true, false, true).is_err());
        args.diagnostics = Some(DiagnosticMode::Dialog);
        assert!(args.validate(false, false, false).is_err());
        args.stream_json = true;
        assert!(args.validate(false, false, true).is_err());
        args.diagnostics = None;
        assert!(args.validate(false, true, true).is_err());
    }

    #[test]
    fn discovery_io_failure_does_not_abort_business_waiting() {
        let base = path("missing-log");
        let launch = DiagnosticLaunch {
            run_id: "read-failure".into(),
            source: "script".into(),
            default_file: base.clone(),
            ..Default::default()
        };
        let location = launch.path(&Default::default()).with_extension("location.json");
        std::fs::write(&location, serde_json::to_vec(&base).unwrap()).unwrap();
        let mut follower = Follower::new(&launch, false);
        assert!(follower.poll().is_ok());
        assert!(follower.incomplete);
        assert!(follower.finish(&[], &serde_json::json!({"status":"success"})).is_err());
        assert!(follower.ended);
        std::fs::remove_file(location).unwrap();
    }

    #[test]
    fn final_drain_uses_byte_progress_and_does_not_timeout_on_backlog() {
        let base = path("backlog");
        let launch = DiagnosticLaunch {
            run_id: "backlog".into(),
            default_file: base.clone(),
            ..Default::default()
        };
        let mut follower = Follower::new(&launch, false);
        let mut ends = Vec::new();
        // Several readers split the poll budget, including JSON records across polls.
        for worker in 0..14 {
            let log = base.with_extension(format!("{worker}.jsonl"));
            let mut file = File::create(&log).unwrap();
            for sequence in 1..=12 {
                writeln!(
                    file,
                    "{}",
                    serde_json::json!({
                        "version": 1, "run_id": "backlog", "source": "script",
                        "worker": worker, "sequence": sequence, "kind": "log",
                        "payload": {"message": "x".repeat(30000)}
                    })
                )
                .unwrap();
            }
            drop(file);
            follower.add(&log).unwrap();
            ends.push(DiagnosticEnd {
                path: log,
                final_sequence: 12,
                complete: true,
                dropped: 0,
            });
        }
        // Even a zero idle allowance must drain any amount of available data.
        follower.drain(&ends, Duration::ZERO);
        assert!(!follower.incomplete);
        assert!(follower.readers.values().all(|r| r.last == 12 && r.pending.is_empty()));
        follower.finish(&ends, &()).unwrap();
        drop(follower);
        for end in ends {
            std::fs::remove_file(end.path).unwrap();
        }
    }

    #[test]
    fn final_drain_still_rejects_missing_sequences_and_trailing_fragments() {
        for (name, tail, expected) in [("missing", "", 2), ("fragment", "{", 1)] {
            let log = path(name);
            let launch = DiagnosticLaunch {
                run_id: "invalid".into(),
                default_file: log.clone(),
                ..Default::default()
            };
            std::fs::write(
                &log,
                format!(
                    "{}\n{tail}",
                    serde_json::json!({
                        "version": 1, "run_id": "invalid", "source": "script",
                        "worker": 0, "sequence": 1, "kind": "log", "payload": {}
                    })
                ),
            )
            .unwrap();
            let end = DiagnosticEnd {
                path: log.clone(),
                final_sequence: expected,
                complete: true,
                dropped: 0,
            };
            let mut follower = Follower::new(&launch, false);
            follower.add(&log).unwrap();
            follower.drain(std::slice::from_ref(&end), Duration::ZERO);
            if name == "missing" {
                assert!(follower.incomplete);
                follower.ended = true;
            } else {
                assert!(follower.finish(&[end], &()).is_err());
            }
            drop(follower);
            std::fs::remove_file(log).unwrap();
        }
    }
}

impl Drop for Follower {
    fn drop(&mut self) {
        if !self.ended {
            eprintln!("diagnostic evidence incomplete: no verified final sequence boundary was received");
        }
    }
}

#[derive(Default, serde::Serialize)]
struct Totals {
    completed: u128,
    valid: u128,
    invalid: u128,
    active: u128,
    incomplete: u128,
    damage: i128,
    ratio_numerator: u128,
    ratio_denominator: u128,
}

pub fn build_message(stream: bool, message: std::fmt::Arguments<'_>) {
    if stream {
        eprintln!("{message}");
    } else {
        println!("{message}");
    }
}
