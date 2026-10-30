//! External unload owner for standalone runners, including --no-wait launches.
use super::{InjectorError, PvzProcess, Result, inject, own_handle, raw_handle};
use std::{
    fs::OpenOptions,
    os::windows::{io::OwnedHandle, process::CommandExt},
    path::Path,
    process::{Command, Stdio},
};
use windows_sys::Win32::{
    Foundation::{WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{CREATE_NO_WINDOW, CreateMutexW, INFINITE, ReleaseMutex, WaitForSingleObject},
};

/// Only external loader tools take this OS mutex; the game thread never does.
pub(super) struct LoaderLock(OwnedHandle);
impl LoaderLock {
    pub fn acquire(pid: u32) -> Result<Self> {
        let name: Vec<u16> = format!("Local\\rsvz-1051-loader-{pid}\0").encode_utf16().collect();
        let handle = unsafe { own_handle(CreateMutexW(std::ptr::null(), 0, name.as_ptr())) }
            .ok_or_else(|| InjectorError::Windows(std::io::Error::last_os_error()))?;
        let status = unsafe { WaitForSingleObject(raw_handle(&handle), INFINITE) };
        if !matches!(status, WAIT_OBJECT_0 | WAIT_ABANDONED) {
            return Err(InjectorError::Windows(std::io::Error::last_os_error()));
        }
        Ok(Self(handle))
    }
}
impl Drop for LoaderLock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(raw_handle(&self.0));
        }
    }
}

pub(super) fn start(pid: u32, config: &str) -> Result<()> {
    let config: serde_json::Value =
        serde_json::from_str(config).map_err(|e| InjectorError::InjectionFailed(e.to_string()))?;
    let field = |name| {
        config[name]
            .as_str()
            .ok_or_else(|| InjectorError::InjectionFailed(format!("missing {name}")))
    };
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::temp_dir().join(format!("rsvz-1051-unload-{pid}.log")))?;
    Command::new(std::env::current_exe()?)
        .args([
            "--pid",
            &pid.to_string(),
            "--watch-result",
            field("control_result_path")?,
            "--watch-run",
            field("run_id")?,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    Ok(())
}

pub(super) fn follow(pid: u32, path: &Path, run: &str) -> Result<()> {
    let process = PvzProcess::open_for_unload(pid)?;
    loop {
        // The owned handle cannot be confused with a later process reusing this PID.
        match unsafe { WaitForSingleObject(process.handle(), 250) } {
            WAIT_OBJECT_0 => return Ok(()),
            WAIT_TIMEOUT => {}
            _ => return Err(InjectorError::Windows(std::io::Error::last_os_error())),
        }
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(result) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if result["run_id"].as_str() == Some(run) {
                    if result["cleanup_complete"] == true
                        && result["terminal"]["status"] == "cancelled"
                        && result["terminal"]["exit_game"] == false
                    {
                        let _lock = LoaderLock::acquire(pid)?;
                        inject::unload_finished_run(&process, run)?;
                    }
                    return Ok(());
                }
                // An older run may publish while the injector replaces it.
                // Never treat another run's terminal result as ours.
            }
        }
    }
}
