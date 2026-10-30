use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackendKind {
    PvzPortable {},
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableRunConfig {
    pub diagnostics: rsvz_backend_api::diagnostics::DiagnosticLaunch,
    pub schema_version: u32,
    pub run_id: String,
    pub backend: BackendKind,
    pub output_path: Option<PathBuf>,
    pub report_path: PathBuf,
    pub stop_event_name: Option<String>,
}

impl PortableRunConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported Portable wire schema version {}; expected {SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        if self.run_id.is_empty() {
            return Err("Portable run_id must not be empty".to_owned());
        }
        if self.report_path.as_os_str().is_empty() {
            return Err("Portable report path must not be empty".to_owned());
        }
        if self
            .stop_event_name
            .as_ref()
            .is_some_and(|name| name.is_empty() || name.contains('\0'))
        {
            return Err("Portable stop-event name must not be empty or contain NUL".to_owned());
        }
        if self
            .output_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty() || path == &self.report_path)
        {
            return Err("Portable output paths must be nonempty and distinct".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Terminal {
    Success {},
    Cancelled { message: String, exit_game: bool },
    Failure { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableRunResult {
    pub cleanup_error: Option<String>,
    pub cleanup_complete: bool,
    pub diagnostics: Vec<rsvz_backend_api::diagnostics::DiagnosticEnd>,
    pub schema_version: u32,
    pub run_id: String,
    pub backend: BackendKind,
    pub terminal: Terminal,
    pub wall_ns: u128,
    pub frames: u64,
    pub completed_rounds: u64,
}

impl PortableRunResult {
    pub fn validate(&self, expected_run_id: &str) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported Portable result schema version {}; expected {SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        if self.run_id.is_empty() || self.run_id != expected_run_id {
            return Err("Portable result run_id is missing or does not match this invocation".to_owned());
        }
        Ok(())
    }
}
