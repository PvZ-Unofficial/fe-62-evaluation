pub(crate) mod lifecycle;

mod output;
mod readiness;
pub mod wire;

use std::ffi::c_void;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use rsvz_backend_api::error::RuntimeError;

use crate::runtime::hook;
use crate::{DispatchEntry, DispatchInput, DispatchResult};

const CONFIG_FILE_PREFIX: &str = "rsvz-1051-script-";

static CONFIG: OnceLock<wire::RunConfig> = OnceLock::new();
static FINISHED: AtomicBool = AtomicBool::new(false);
static SHARED_MODULE_HOST: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeUpdate {
    Idle,
    Continue,
    Skip,
}

pub fn initialize(_context: *mut c_void, dispatch: DispatchEntry) -> u32 {
    initialize_mode(dispatch, false)
}

pub fn initialize_shared(_context: *mut c_void, dispatch: DispatchEntry) -> u32 {
    initialize_mode(dispatch, true)
}

fn initialize_mode(dispatch: DispatchEntry, shared: bool) -> u32 {
    match panic::catch_unwind(AssertUnwindSafe(|| initialize_inner(dispatch, shared))) {
        Ok(Ok(())) => 1,
        Ok(Err(_)) | Err(_) => {
            hook::mark_unowned_initialization_failure();
            0
        }
    }
}

fn initialize_inner(dispatch: DispatchEntry, shared: bool) -> Result<(), String> {
    let config = load_config()?;
    CONFIG
        .set(config)
        .map_err(|_config| "1051 host config is already installed".to_owned())?;
    SHARED_MODULE_HOST.store(shared, Ordering::Release);
    hook::install_dispatch_entry(dispatch).map_err(|error| error.to_string())?;
    hook::initialize().map_err(|error| error.to_string())
}

pub fn request_unload(context: *mut c_void) -> u32 {
    // The injector may supply its run id as a remote NUL-terminated UTF-8 string.
    // A late watcher must never stop a replacement runner at the same DLL address.
    if !context.is_null() {
        let expected = unsafe { std::ffi::CStr::from_ptr(context.cast()) }.to_bytes();
        if CONFIG.get().is_none_or(|config| config.run_id.as_bytes() != expected) {
            return 0;
        }
    }
    u32::from(panic::catch_unwind(hook::request_unload_and_wait).unwrap_or(false))
}

pub(crate) fn dispatch_frame() -> NativeUpdate {
    panic::catch_unwind(AssertUnwindSafe(dispatch_frame_inner)).unwrap_or_else(|_panic| {
        finish(
            None,
            Some(RuntimeError::new("1051 backend host dispatch panicked")),
            false,
            false,
        )
    })
}

fn dispatch_frame_inner() -> NativeUpdate {
    // A retained DLL is not permission to invoke the stopped script again.
    // Native gameplay may continue while an unsafe unload remains refused.
    if FINISHED.load(Ordering::Acquire) {
        return NativeUpdate::Idle;
    }
    let stop_requested = hook::unload_requested();
    let result = crate::runtime::hook::dispatch_entry::with_installed(|backend, dispatch| {
        let (opening_ready, script_dispatch_allowed) = match readiness::before_dispatch(backend, stop_requested) {
            Ok(readiness::BeforeDispatch::AdvanceNative) if SHARED_MODULE_HOST.load(Ordering::Acquire) => {
                (false, false)
            }
            Ok(readiness::BeforeDispatch::AdvanceNative) => return DispatchResult::Continue,
            Ok(readiness::BeforeDispatch::Dispatch { opening_ready }) => (opening_ready, true),
            Err(error) => {
                return DispatchResult::Stop {
                    cleanup_complete: false,
                    artifact: None,
                    error: Some(RuntimeError::new(error.to_string())),
                };
            }
        };
        let result = dispatch(
            backend,
            DispatchInput {
                stop_requested,
                completed_rounds: if script_dispatch_allowed {
                    readiness::take_completed_rounds()
                } else {
                    0
                },
                opening_ready,
                script_dispatch_allowed,
            },
        );
        if matches!(result, DispatchResult::EnterBattle)
            && let Err(error) = readiness::after_dispatch(backend)
        {
            return DispatchResult::Stop {
                cleanup_complete: false,
                artifact: None,
                error: Some(RuntimeError::new(error.to_string())),
            };
        }
        result
    })
    .unwrap_or(DispatchResult::Continue);

    match result {
        DispatchResult::Continue => NativeUpdate::Continue,
        DispatchResult::Idle => NativeUpdate::Idle,
        DispatchResult::SkipUpdate | DispatchResult::EnterBattle => NativeUpdate::Skip,
        DispatchResult::Stop {
            artifact,
            error,
            cleanup_complete,
        } => finish(artifact, error, stop_requested, cleanup_complete),
    }
}

fn finish(
    artifact: Option<rsvz_backend_api::artifact::SessionArtifact>, error: Option<RuntimeError>,
    externally_stopped: bool, mut cleanup_complete: bool,
) -> NativeUpdate {
    if FINISHED.swap(true, Ordering::AcqRel) {
        return NativeUpdate::Skip;
    }
    let config = config();
    let exit_game = error
        .as_ref()
        .and_then(|error| error.find_source::<rsvz_backend_api::error::UserCancelled>())
        .is_some_and(|cancel| cancel.exit_game);
    let cancelled = error
        .as_ref()
        .is_some_and(|error| error.find_source::<rsvz_backend_api::error::UserCancelled>().is_some());
    let mut cleanup_error = error
        .as_ref()
        .and_then(|e| e.find_source::<rsvz_backend_api::error::CleanupFailure>())
        .map(|e| e.cleanup.to_string());
    let mut failure = error.map(|error| error.to_string());
    if externally_stopped && failure.is_none() {
        failure = Some("runner unloaded before script completion".to_owned());
    }
    let baseline_error = crate::impls::reset::finish_session()
        .err()
        .map(|error| format!("restore 1051 reset session failed: {error}"));
    readiness::clear();
    let physical_error = hook::prepare_runner_cleanup()
        .err()
        .map(|error| format!("prepare 1051 cleanup failed: {error}"));
    for message in [baseline_error, physical_error].into_iter().flatten() {
        cleanup_error = Some(match cleanup_error {
            Some(first) => format!("{first}; {message}"),
            None => message,
        });
    }
    if let Some(message) = &cleanup_error {
        cleanup_complete = false;
        failure.get_or_insert_with(|| message.clone());
    }
    if let (Some(path), Some(artifact)) = (config.output_path.as_deref(), artifact.as_ref())
        && let Err(error) = output::write_artifact(path, &config.run_id, artifact)
    {
        failure = Some(format!("write 1051 artifact failed: {error}"));
    }
    let result = wire::RunResult {
        cleanup_complete,
        cleanup_error,
        diagnostics: crate::Pvz1051Backend::diagnostic_ends(),
        schema_version: wire::SCHEMA_VERSION,
        run_id: config.run_id.clone(),
        terminal: if cancelled {
            wire::Terminal::Cancelled {
                exit_game,
                message: failure.clone().unwrap_or_else(|| "cancelled".into()),
            }
        } else {
            failure
                .as_ref()
                .map_or(wire::Terminal::Success {}, |message| wire::Terminal::Failure {
                    message: message.clone(),
                })
        },
    };
    if let Err(error) = result
        .validate(Some(&config.run_id))
        .and_then(|()| output::write_control(&config.control_result_path, &config.run_id, &result))
    {
        crate::Pvz1051Backend::diagnostic_fallback(&format!("write 1051 control result failed: {error}"));
    }

    if cleanup_complete {
        crate::Pvz1051Backend::diagnostic_drive_exit(true);
        hook::mark_runner_cleanup_complete();
        hook::request_unload();
    }
    NativeUpdate::Skip
}

fn load_config() -> Result<wire::RunConfig, String> {
    let path = std::env::temp_dir().join(format!("{CONFIG_FILE_PREFIX}{}.json", std::process::id()));
    let text =
        fs::read_to_string(&path).map_err(|error| format!("read host config {} failed: {error}", path.display()))?;
    let config: wire::RunConfig =
        serde_json::from_str(&text).map_err(|error| format!("parse 1051 host config failed: {error}"))?;
    config.validate(None)?;
    fs::remove_file(&path).map_err(|error| format!("remove consumed config {} failed: {error}", path.display()))?;
    Ok(config)
}

fn config() -> &'static wire::RunConfig {
    CONFIG
        .get()
        .expect("1051 host config must be installed before dispatch")
}

pub(crate) fn diagnostic_launch() -> Option<rsvz_backend_api::diagnostics::DiagnosticLaunch> {
    CONFIG.get().map(|config| config.diagnostics.clone())
}
