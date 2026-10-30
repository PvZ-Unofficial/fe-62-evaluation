//! Backend-local diagnostic output and launch state.
use crate::PortableBackend;
use rsvz_backend_api::diagnostics::{DiagnosticEnd, DiagnosticFile, DiagnosticLaunch, DialogAction};
use std::{cell::RefCell, io, path::Path};
thread_local! {
    static LAUNCH: RefCell<Option<DiagnosticLaunch>> = const { RefCell::new(None) };
    static ENDS: RefCell<[Option<DiagnosticEnd>; 2]> = const { RefCell::new([None, None]) };
    static FILE: RefCell<DiagnosticFile> = RefCell::new(DiagnosticFile::default());
}
pub(crate) fn install(launch: DiagnosticLaunch) {
    LAUNCH.with_borrow_mut(|slot| *slot = Some(launch));
    FILE.with_borrow_mut(|file| *file = DiagnosticFile::default());
}
pub(crate) fn local_launch() -> DiagnosticLaunch {
    LAUNCH.with_borrow(|slot| slot.clone().unwrap_or_else(DiagnosticLaunch::standalone))
}
impl PortableBackend {
    pub const fn diagnostic_dialog_supported() -> bool {
        true
    }
    pub fn diagnostic_launch() -> DiagnosticLaunch {
        crate::module_link::diagnostic_launch().unwrap_or_else(local_launch)
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
    pub fn diagnostic_dialog(message: &str) -> DialogAction {
        crate::module_link::diagnostic_dialog(message)
    }
    pub fn diagnostic_exit() {
        crate::module_link::diagnostic_exit();
    }
}

thread_local! { static MODAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub(crate) fn modal_active() -> bool {
    MODAL.get()
}
pub(crate) fn show_dialog(message: &str) -> DialogAction {
    if MODAL.replace(true) {
        return DialogAction::Continue;
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            MODAL.set(false);
        }
    }
    let _guard = Guard;
    let code = pvzp_rs::diagnostic_dialog(message);
    if code < 0 {
        PortableBackend::diagnostic_fallback(&format!("SDL diagnostic dialog failed\n{message}"));
    }
    DialogAction::from_code(code.max(0) as u32)
}
pub(crate) fn post_exit() {
    pvzp_rs::diagnostic_exit();
}

thread_local! { static EXIT_PENDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub(crate) fn request_exit() {
    EXIT_PENDING.set(true);
}
impl PortableBackend {
    pub fn diagnostic_drive_exit(clean: bool) {
        if clean && EXIT_PENDING.replace(false) {
            post_exit();
        }
    }
}
