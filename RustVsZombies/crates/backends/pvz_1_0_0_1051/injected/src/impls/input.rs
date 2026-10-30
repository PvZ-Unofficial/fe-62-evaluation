use rsvz_backend_api::backend::KeyboardStateBackend;
use rsvz_model::model::KeyCode;

use crate::error::{Pvz1051Error, Result};
use crate::raw::layout as ptrs;
use crate::runtime::Pvz1051Backend;
use rsvz_backend_api::InputBackend;
use rsvz_model::{MouseButton, PixelPos};

use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

impl KeyboardStateBackend for Pvz1051Backend {
    fn set_key_event_sink(&self, sink: Option<fn(u32)>) -> Result<()> {
        if let Some(result) = crate::module_link::key(sink) {
            return result;
        }
        crate::patches::keyboard::set(sink)
    }
    fn key_is_down(&self, key: KeyCode) -> bool {
        // SAFETY: GetAsyncKeyState reads process-independent keyboard state for the copied virtual-key
        // value. It does not retain pointers or references into the game process.
        let state = unsafe { GetAsyncKeyState(i32::from(key.raw_virtual_key())) };
        state < 0
    }
    fn input_focused(&self) -> Result<bool> {
        let app = self.app();

        // SAFETY: `app` is a non-null LawnApp pointer. The mHWnd field is copied and only compared
        // against the current foreground window handle.
        let hwnd = unsafe { ptrs::LawnApp::hwnd(app.as_ptr()) };
        if hwnd.is_null() {
            return Ok(false);
        }

        // SAFETY: GetForegroundWindow returns a borrowed OS handle value. It does not give ownership and
        // no pointer derived from it is dereferenced.
        let foreground = unsafe { GetForegroundWindow() };
        Ok(foreground == hwnd)
    }
}

fn button(button: MouseButton) -> i32 {
    match button {
        MouseButton::Left => 1,
        MouseButton::Right => -1,
    }
}

impl Pvz1051Backend {
    fn mouse_target(&self, pos: PixelPos) -> Result<*mut ptrs::MouseWindow> {
        let app = self.app();
        // SAFETY: active host proves App lifetime; dimensions are native logical coordinates.
        unsafe {
            if pos.x < 0
                || pos.y < 0
                || pos.x >= ptrs::LawnApp::canvas_width(app.as_ptr())
                || pos.y >= ptrs::LawnApp::canvas_height(app.as_ptr())
            {
                return Err(Pvz1051Error::OperationRejected(
                    "mouse position is outside the native canvas",
                ));
            }
            let manager = ptrs::LawnApp::mouse_window(app.as_ptr());
            if manager.is_null() {
                return Err(Pvz1051Error::OperationRejected("native widget manager is unavailable"));
            }
            Ok(manager)
        }
    }
}
impl InputBackend for Pvz1051Backend {
    fn mouse_move(&mut self, pos: PixelPos) -> Result<()> {
        let manager = self.mouse_target(pos)?;
        // SAFETY: exclusive backend borrow; verified native mouse ABI.
        unsafe {
            crate::raw::abi::widget_manager_mouse_move(manager, pos.x, pos.y);
        }
        Ok(())
    }
    fn mouse_down(&mut self, pos: PixelPos, key: MouseButton) -> Result<()> {
        let manager = self.mouse_target(pos)?;
        // SAFETY: no entity borrows survive this exclusive UI operation. The physical
        // ingress defers Rust dispatch/unload during reentrant native modal loops.
        unsafe {
            crate::raw::abi::widget_manager_mouse_down(manager, pos.x, pos.y, button(key));
        }
        Ok(())
    }
    fn mouse_up(&mut self, pos: PixelPos, key: MouseButton) -> Result<()> {
        let manager = self.mouse_target(pos)?;
        // SAFETY: exclusive UI operation with validated coordinates and ABI.
        unsafe {
            crate::raw::abi::widget_manager_mouse_up(manager, pos.x, pos.y, button(key));
        }
        Ok(())
    }
    fn release_mouse(&mut self) -> Result<()> {
        let board = self.board()?;
        // SAFETY: current Board owns the cursor and pools; validate native dereferences
        // before invoking cancellation, which restores an unconsumed held seed.
        unsafe {
            let cursor = ptrs::Board::cursor_object(board.as_ptr());
            if cursor.is_null() {
                return Err(Pvz1051Error::OperationRejected("no native cursor"));
            }
            match ptrs::CursorObject::cursor_type(cursor) {
                1 => {
                    let index = ptrs::CursorObject::seed_bank_index(cursor);
                    let bank = self.seed_bank()?;
                    if index < 0 || index >= ptrs::SeedBank::packet_count(bank.as_ptr()) || index >= 10 {
                        return Err(Pvz1051Error::OperationRejected("held seed slot is no longer active"));
                    }
                }
                2 => {
                    let coins = ptrs::Board::coins(board.as_ptr());
                    if ptrs::DataArray::try_to_get_coin(coins, ptrs::CursorObject::coin_id(cursor)).is_none() {
                        return Err(Pvz1051Error::OperationRejected(
                            "held usable seed coin is no longer present",
                        ));
                    }
                }
                _ => {}
            }
            crate::raw::abi::board_refresh_seed_packet_from_cursor(board.as_ptr());
        }
        Ok(())
    }
}

impl rsvz_backend_api::BoardClickBackend for Pvz1051Backend {
    fn set_board_click_capture(&self, enabled: bool) -> Result<()> {
        if let Some(host) = crate::module_link::services() {
            return crate::module_link::checked((host.click_set)(crate::module_link::source(), enabled));
        }
        crate::patches::board_click::set(enabled)
    }
    fn set_board_click_native_priority(&self, enabled: bool) -> Result<()> {
        if let Some(host) = crate::module_link::services() {
            return crate::module_link::checked((host.click_priority)(enabled));
        }
        crate::patches::board_click::prefer_native(enabled);
        Ok(())
    }
    fn board_click_capture_active(&self) -> Result<bool> {
        if let Some(host) = crate::module_link::services() {
            let value = (host.click_active)();
            crate::module_link::checked(u32::from(value > 1))?;
            return Ok(value == 1);
        }
        Ok(crate::patches::board_click::active())
    }
    fn set_lawn_preview(&self, preview: Option<rsvz_model::LawnPreview>) -> Result<()> {
        if let Some(host) = crate::module_link::services() {
            let (kind, imitate) = match preview {
                None => (-1, false),
                Some(rsvz_model::LawnPreview::Shovel) => (-2, false),
                Some(rsvz_model::LawnPreview::Plant(p)) => (i32::from(p.kind()), p.imitater() >= 0),
            };
            return crate::module_link::checked((host.preview)(crate::module_link::source(), kind, imitate));
        }
        crate::patches::board_click::preview(preview)
    }
    fn take_board_click(&self) -> Result<Option<(i32, i32, i32)>> {
        if let Some(host) = crate::module_link::services() {
            let mut value = [0; 3];
            let status = unsafe { (host.click_take)(crate::module_link::source(), value.as_mut_ptr()) };
            crate::module_link::checked(u32::from(status > 1))?;
            return Ok((status == 1).then_some((value[0], value[1], value[2])));
        }
        crate::patches::board_click::take()
    }
}
