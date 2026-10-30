//! Narrow diagnostic transport contract. No globals, game access or execution policy.
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const MAX_RECORD_BYTES: usize = 64 * 1024;
pub const MAX_READ_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticMode {
    #[default]
    Auto,
    Dialog,
    Cli,
    File,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    Debug,
    Info,
    Warning,
    Error,
}

/// None means unspecified, including for mode; Some(Auto) is an explicit choice.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticOptions {
    pub mode: Option<DiagnosticMode>,
    pub log_file: Option<PathBuf>,
    pub level: Option<DiagnosticLevel>,
    pub progress_interval_ms: Option<u64>,
}
impl DiagnosticOptions {
    pub fn overlay(&mut self, higher: &Self) {
        if let Some(value) = higher.mode {
            self.mode = Some(value);
        }
        if let Some(value) = &higher.log_file {
            self.log_file = Some(value.clone());
        }
        if let Some(value) = higher.level {
            self.level = Some(value);
        }
        if let Some(value) = higher.progress_interval_ms {
            self.progress_interval_ms = Some(value);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticLaunch {
    pub options: DiagnosticOptions,
    pub run_id: String,
    pub following: bool,
    pub worker: usize,
    pub source: String,
    pub default_file: PathBuf,
}
impl DiagnosticLaunch {
    pub fn standalone() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let run_id = format!("{}-{stamp}", std::process::id());
        Self {
            default_file: std::env::temp_dir().join(format!("rsvz-{run_id}.jsonl")),
            run_id,
            source: "script".into(),
            ..Self::default()
        }
    }
    pub fn path(&self, options: &DiagnosticOptions) -> PathBuf {
        source_path(
            options.log_file.as_deref().unwrap_or(&self.default_file),
            &self.run_id,
            &self.source,
            self.worker,
        )
    }
}

pub fn source_path(base: &Path, run: &str, source: &str, worker: usize) -> PathBuf {
    let name = base.file_name().unwrap_or_default().to_string_lossy();
    base.with_file_name(format!("{name}.{run}.{source}.{worker}.jsonl"))
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticEnd {
    pub path: PathBuf,
    pub final_sequence: u64,
    pub complete: bool,
    pub dropped: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DialogAction {
    Continue = 0,
    MuteFrame = 1,
    MuteRun = 2,
    Stop = 3,
    Exit = 4,
}
impl DialogAction {
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => Self::MuteFrame,
            2 => Self::MuteRun,
            3 => Self::Stop,
            4 => Self::Exit,
            _ => Self::Continue,
        }
    }
}

/// Owned by the concrete backend; writes and flushes complete, already encoded records.
#[derive(Default)]
pub struct DiagnosticFile {
    file: Option<File>,
    end: DiagnosticEnd,
    attempted: bool,
}
impl DiagnosticFile {
    pub fn open(&mut self, path: &Path) -> io::Result<()> {
        if self.attempted {
            return if self.end.path == path && self.file.is_some() {
                Ok(())
            } else {
                Err(io::Error::other(
                    "diagnostic path cannot change after output starts, or opening already failed",
                ))
            };
        }
        self.attempted = true;
        self.end = DiagnosticEnd {
            path: path.to_owned(),
            complete: true,
            ..Default::default()
        };
        let result = (|| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // A source/run path is unique; never append a new sequence to an older execution.
            self.file = Some(OpenOptions::new().write(true).create_new(true).open(path)?);
            Ok(())
        })();
        if result.is_err() {
            self.end.complete = false;
        }
        result
    }
    pub fn write(&mut self, sequence: u64, bytes: &[u8]) -> io::Result<()> {
        let result = (|| {
            if bytes.len() > MAX_RECORD_BYTES || bytes.contains(&b'\n') {
                return Err(io::Error::other("invalid or oversized diagnostic record"));
            }
            let file = self
                .file
                .as_mut()
                .ok_or_else(|| io::Error::other("diagnostic file is not open"))?;
            file.write_all(bytes)?;
            file.write_all(b"\n")?;
            file.flush()?;
            self.end.final_sequence = sequence;
            Ok(())
        })();
        if result.is_err() {
            self.end.complete = false;
            self.end.dropped += 1;
        }
        result
    }
    pub fn mark_incomplete(&mut self) {
        self.end.complete = false;
        self.end.dropped += 1;
    }
    pub fn finish(&mut self) -> DiagnosticEnd {
        if let Some(mut file) = self.file.take() {
            if file.flush().is_err() {
                self.end.complete = false;
            }
        }
        self.end.clone()
    }
}

/// A reusable serializer destination whose allocation and capacity are bounded.
pub struct RecordBuffer(Vec<u8>);
impl Default for RecordBuffer {
    fn default() -> Self {
        Self(Vec::with_capacity(MAX_RECORD_BYTES))
    }
}
impl RecordBuffer {
    pub fn clear(&mut self) {
        self.0.clear();
    }
    pub fn release(&mut self) {
        self.0 = Vec::new();
    }
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}
impl Write for RecordBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_RECORD_BYTES - self.0.len() {
            return Err(io::Error::other("diagnostic record exceeds 64 KiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// One immutable location record per run/source, for scripts choosing their own path.
pub fn publish_location(launch: &DiagnosticLaunch, path: &Path) -> io::Result<()> {
    let base = source_path(&launch.default_file, &launch.run_id, &launch.source, launch.worker);
    let location = base.with_extension("location.json");
    if let Some(parent) = location.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(location)?;
    serde_json::to_writer(&mut file, path)?;
    file.flush()
}

/// Independent, noninteractive fallback; failure of the fallback cannot recurse.
pub fn emergency_output(message: &str) {
    let mut end = message.len().min(8192);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    let message = &message[..end];
    let _ = writeln!(std::io::stderr().lock(), "{message}");
    let path = std::env::temp_dir().join(format!("rsvz-diagnostic-fallback-{}.log", std::process::id()));
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
        let _ = file.flush();
    }
}

#[cfg(all(test, windows))]
mod file_tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;
    #[test]
    fn terminal_flush_closes_the_writer_before_an_execution_can_unload() {
        let launch = DiagnosticLaunch::standalone();
        let path = launch.path(&DiagnosticOptions::default());
        let mut writer = DiagnosticFile::default();
        writer.open(&path).unwrap();
        writer.write(1, br#"{"sequence":1}"#).unwrap();
        let end = writer.finish();
        assert!(end.complete);
        assert_eq!(end.final_sequence, 1);
        // An exclusive write open is rejected while the earlier writer is alive.
        let exclusive = OpenOptions::new().write(true).share_mode(0).open(&path).unwrap();
        drop(exclusive);
        std::fs::remove_file(path).unwrap();
    }
}
