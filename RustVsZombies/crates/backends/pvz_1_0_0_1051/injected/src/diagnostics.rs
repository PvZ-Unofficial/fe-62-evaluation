//! Backend-local diagnostic output and launch state.

use std::{cell::RefCell, io, path::Path};

use rsvz_backend_api::diagnostics::{DiagnosticEnd, DiagnosticFile, DiagnosticLaunch, DialogAction};

use crate::Pvz1051Backend;

thread_local! {
    static ENDS: RefCell<[Option<DiagnosticEnd>; 2]> = const { RefCell::new([None, None]) };
    static FILE: RefCell<DiagnosticFile> = RefCell::new(DiagnosticFile::default());
}

pub(crate) fn local_launch() -> DiagnosticLaunch {
    crate::host::diagnostic_launch().unwrap_or_else(DiagnosticLaunch::standalone)
}

impl Pvz1051Backend {
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

thread_local! { static OWNER_CLOSE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
thread_local! { static MODAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

pub(crate) fn modal_active() -> bool {
    MODAL.get()
}

struct ModalGuard;

impl Drop for ModalGuard {
    fn drop(&mut self) {
        MODAL.set(false);
    }
}

fn window() -> windows_sys::Win32::Foundation::HWND {
    // Existing, verified HWND accessor; no new raw game call or layout assumption.
    unsafe {
        let app = crate::raw::layout::lawn_app();
        if app.is_null() {
            std::ptr::null_mut()
        } else {
            crate::raw::layout::LawnApp::hwnd(app)
        }
    }
}

pub(crate) fn post_exit() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let hwnd = window();
    if !hwnd.is_null() {
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }
}

struct OwnerGuard {
    hwnd: windows_sys::Win32::Foundation::HWND,
    previous: i32,
}
impl OwnerGuard {
    fn install(hwnd: windows_sys::Win32::Foundation::HWND) -> Option<Self> {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        if hwnd.is_null() {
            return None;
        }
        // Standard Win32 subclassing on the owning window thread; no game ABI.
        let previous = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, owner_proc as *const () as i32) };
        if previous == 0 {
            None
        } else {
            Some(Self { hwnd, previous })
        }
    }
}
impl Drop for OwnerGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_WNDPROC, self.previous);
        }
    }
}
unsafe extern "system" fn owner_proc(
    hwnd: windows_sys::Win32::Foundation::HWND, message: u32, wparam: usize, lparam: isize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    if message == WM_CLOSE || message == WM_SYSCOMMAND && (wparam & 0xfff0) == SC_CLOSE as usize {
        // Do not destroy the world while a suspended script owns borrows. The
        // user still dismisses the open dialog; its returned action then exits safely.
        OWNER_CLOSE.set(true);
        return 0;
    }
    // The OS may service the owner, but none of the game's WndProc is reentered.
    // Native updates, paint callbacks and other source dispatch remain excluded.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

pub(crate) fn show_dialog(message: &str) -> DialogAction {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    if MODAL.replace(true) {
        return DialogAction::Continue;
    }
    let _guard = ModalGuard;
    let Some(_owner) = OwnerGuard::install(window()) else {
        Pvz1051Backend::diagnostic_fallback("cannot guard the game window for a diagnostic dialog");
        return DialogAction::Continue;
    };
    OWNER_CLOSE.set(false);
    fn dword(out: &mut Vec<u16>, value: u32) {
        out.extend([value as u16, (value >> 16) as u16]);
    }
    fn string(out: &mut Vec<u16>, text: &str) {
        out.extend(text.encode_utf16());
        out.push(0);
    }
    let mut template = Vec::<u16>::new();
    dword(
        &mut template,
        WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME as u32 | DS_SETFONT as u32 | DS_CENTER as u32,
    );
    dword(&mut template, 0);
    template.extend([6, 0, 0, 390, 228, 0, 0]);
    string(&mut template, "RustVsZombies — 运行诊断");
    template.push(9);
    string(&mut template, "Segoe UI");
    let controls = [
        (
            200,
            8,
            8,
            374,
            154,
            0x81,
            WS_VSCROLL | ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32,
            "",
        ),
        (100, 8, 170, 116, 23, 0x80, BS_DEFPUSHBUTTON as u32, "继续"),
        (101, 137, 170, 116, 23, 0x80, 0, "本帧不弹窗"),
        (102, 266, 170, 116, 23, 0x80, 0, "本次不弹窗"),
        (103, 73, 199, 116, 23, 0x80, 0, "卸载脚本"),
        (104, 202, 199, 116, 23, 0x80, 0, "退出游戏"),
    ];
    for (id, x, y, w, h, class, style, title) in controls {
        if template.len() % 2 != 0 {
            template.push(0);
        }
        dword(&mut template, WS_CHILD | WS_VISIBLE | WS_TABSTOP | style);
        dword(&mut template, 0);
        template.extend([x, y, w, h, id, 0xffff, class]);
        string(&mut template, title);
        template.push(0);
    }
    let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    // Template and text stay alive throughout the synchronous modal loop.
    let result = unsafe {
        DialogBoxIndirectParamW(
            std::ptr::null_mut(),
            template.as_ptr().cast(),
            window(),
            Some(dialog_proc),
            text.as_ptr() as isize,
        )
    };
    if result == -1 {
        Pvz1051Backend::diagnostic_fallback(&format!(
            "dialog creation failed: {}\n{message}",
            io::Error::last_os_error()
        ));
    }
    if OWNER_CLOSE.replace(false) {
        DialogAction::Exit
    } else {
        DialogAction::from_code(result.max(0) as u32)
    }
}

unsafe extern "system" fn dialog_proc(
    hwnd: windows_sys::Win32::Foundation::HWND, message: u32, wparam: usize, lparam: isize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    // No allocation, Rust callbacks or unwinding in this window procedure.
    match message {
        WM_INITDIALOG => {
            unsafe {
                SetDlgItemTextW(hwnd, 200, lparam as *const u16);
            }
            1
        }
        WM_COMMAND => {
            let id = (wparam & 0xffff) as u32;
            if (100..=104).contains(&id) || id == IDCANCEL as u32 {
                unsafe {
                    EndDialog(hwnd, if id == IDCANCEL as u32 { 0 } else { (id - 100) as isize });
                }
                1
            } else {
                0
            }
        }
        WM_CLOSE => {
            unsafe {
                EndDialog(hwnd, 0);
            }
            1
        }
        _ => 0,
    }
}

thread_local! { static EXIT_PENDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub(crate) fn request_exit() {
    EXIT_PENDING.set(true);
}
impl Pvz1051Backend {
    pub fn diagnostic_drive_exit(clean: bool) {
        if clean && EXIT_PENDING.replace(false) {
            post_exit();
        }
    }
}
