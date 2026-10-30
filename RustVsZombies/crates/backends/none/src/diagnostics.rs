//! Backend-local diagnostic output and launch state.
use crate::NoBackend;
use rsvz_backend_api::diagnostics::{DiagnosticEnd, DiagnosticFile, DiagnosticLaunch, DialogAction};
use std::{cell::RefCell, io, path::Path};
thread_local! {
    static ENDS: RefCell<[Option<DiagnosticEnd>; 2]> = const { RefCell::new([None, None]) };
    static FILE: RefCell<DiagnosticFile> = RefCell::new(DiagnosticFile::default());
}
pub(crate) fn local_launch() -> DiagnosticLaunch {
    DiagnosticLaunch::standalone()
}
impl NoBackend {
    pub const fn diagnostic_dialog_supported() -> bool {
        false
    }
    pub fn diagnostic_launch() -> DiagnosticLaunch {
        local_launch()
    }
    pub fn diagnostic_open(launch: &DiagnosticLaunch, path: &Path) -> io::Result<()> {
        FILE.with_borrow_mut(|file| file.open(path))?;
        rsvz_backend_api::diagnostics::publish_location(launch, path)
    }
    pub fn diagnostic_reset() {
        FILE.with_borrow_mut(|file| *file = DiagnosticFile::default());
    }
    pub fn diagnostic_write(sequence: u64, bytes: &[u8]) -> io::Result<()> {
        FILE.with_borrow_mut(|file| file.write(sequence, bytes))
    }
    pub fn diagnostic_incomplete() {
        FILE.with_borrow_mut(DiagnosticFile::mark_incomplete);
    }
    pub fn diagnostic_clear_end(source: usize) {
        ENDS.with_borrow_mut(|ends| {
            if let Some(slot) = ends.get_mut(source) {
                *slot = None;
            }
        });
    }
    pub fn diagnostic_publish_end(source: usize, end: DiagnosticEnd) {
        ENDS.with_borrow_mut(|ends| {
            if let Some(slot) = ends.get_mut(source) {
                *slot = Some(end);
            }
        });
    }
    pub fn diagnostic_ends() -> Vec<DiagnosticEnd> {
        let own = Self::diagnostic_finish();
        let mut ends: Vec<_> = ENDS.with_borrow(|ends| ends.iter().flatten().cloned().collect());
        if !own.path.as_os_str().is_empty() {
            ends.push(own);
        }
        ends
    }
    pub fn diagnostic_finish() -> DiagnosticEnd {
        FILE.with_borrow_mut(DiagnosticFile::finish)
    }
    pub fn diagnostic_fallback(message: &str) {
        rsvz_backend_api::diagnostics::emergency_output(message);
    }
    pub fn diagnostic_dialog(_message: &str) -> DialogAction {
        DialogAction::Continue
    }
    pub fn diagnostic_exit() {}
    pub fn diagnostic_drive_exit(_clean: bool) {}
}
