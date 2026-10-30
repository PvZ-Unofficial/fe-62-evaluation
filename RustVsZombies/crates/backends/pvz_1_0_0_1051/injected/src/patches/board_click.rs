//! Board::MouseDown 0x411f20: ECX=this, stack x/y/count, ret 12.
//! Verified prologue is six bytes; native MouseUp tests Board+0x150 first.
use super::memory::{encode_rel32, read_bytes, restore_owned_bytes, write_bytes};
use crate::{
    error::{Pvz1051Error, Result},
    raw::{
        abi,
        layout::{self, Board, CursorObject, LawnApp},
    },
};
use rsvz_model::{LawnPreview, model::input::lawn_grid};
use std::cell::{Cell, RefCell};
struct ClickQueue {
    entries: [(i32, i32, i32); 32],
    head: usize,
    len: usize,
    overflow: bool,
}
thread_local! {
    static ENABLED: Cell<bool> = const {Cell::new(false)};
    static NATIVE_PRIORITY: Cell<bool> = const {Cell::new(false)};
    static PREVIEW: Cell<Option<LawnPreview>> = const {Cell::new(None)};
    static DRAW_PATCH: Cell<Option<[u8;4]>> = const {Cell::new(None)};
    static PATCH:Cell<Option<[u8;6]>>=const{Cell::new(None)};
    static CLICKS: RefCell<ClickQueue> = const {RefCell::new(ClickQueue {entries:[(0,0,0);32],head:0,len:0,overflow:false})};
}
const SITE: usize = 0x411f20;
const ORIGINAL: [u8; 6] = [0x55, 0x8b, 0xec, 0x83, 0xe4, 0xf8];
pub(crate) fn set(enabled: bool) -> Result<()> {
    if enabled && PATCH.get().is_none() {
        if read_bytes(SITE, 6) != ORIGINAL {
            return Err(Pvz1051Error::OperationRejected("Board mouse entry is already patched"));
        }
        let mut patch = [0x90; 6];
        patch[..5].copy_from_slice(&encode_rel32(
            0xe9,
            SITE,
            mouse_down as *const () as usize,
            "board click",
        )?);
        CLICKS.with_borrow_mut(|q| {
            q.head = 0;
            q.len = 0;
            q.overflow = false;
        });
        write_bytes(SITE, &patch)?;
        PATCH.set(Some(patch));
        ENABLED.set(true);
        NATIVE_PRIORITY.set(false);
    } else if !enabled {
        preview(None)?;
        ENABLED.set(false);
        NATIVE_PRIORITY.set(false);
        if let Some(patch) = PATCH.get() {
            restore_owned_bytes(SITE, &ORIGINAL, &patch, "board click")?;
            PATCH.set(None);
        }
        CLICKS.with_borrow_mut(|q| {
            q.head = 0;
            q.len = 0;
            q.overflow = false;
        });
    }
    Ok(())
}
pub(crate) fn take() -> Result<Option<(i32, i32, i32)>> {
    CLICKS.with_borrow_mut(|q| {
        if q.overflow {
            return Err(Pvz1051Error::OperationRejected("lawn click queue overflow"));
        }
        if q.len == 0 {
            return Ok(None);
        }
        let item = q.entries[q.head];
        q.head = (q.head + 1) % 32;
        q.len -= 1;
        Ok(Some(item))
    })
}
extern "C" fn capture(board: *mut Board, x: i32, y: i32, button: i32) -> bool {
    let _active = crate::runtime::hook::ActiveHookGuard::enter();
    // SAFETY: native Board method supplies its live receiver; only Copy facts
    // are queued. No dispatch, user callback, allocation or world edit occurs here.
    unsafe {
        if !capture_active(board) {
            return false;
        }
        let Ok(scene) = rsvz_model::SceneKind::try_from_code(Board::scene(board)) else {
            return false;
        };
        if rsvz_model::model::input::lawn_grid(scene, x, y).is_none() {
            return false;
        }
        CLICKS.with_borrow_mut(|q| {
            if q.len == 32 {
                q.overflow = true;
            } else {
                let index = (q.head + q.len) % 32;
                q.entries[index] = (x, y, button);
                q.len += 1;
            }
        });
        // Cancel immediately, so another input in the same native batch cannot
        // still use the old tool. The queued right click remains readable.
        if button < 0 && NATIVE_PRIORITY.get() {
            ENABLED.set(false);
        }
        board.cast::<u8>().add(0x150).write(1);
    }
    true
}
#[unsafe(naked)]
unsafe extern "C" fn mouse_down() {
    core::arch::naked_asm!(
        "pushfd", "pushad", "push [esp+48]", "push [esp+48]", "push [esp+48]", "push [esp+36]",
        "call {capture}", "add esp,16", "test al,al", "jz 2f",
        "popad", "popfd", "ret 12",
        "2:", "popad", "popfd", "push ebp", "mov ebp,esp", "and esp,-8", "push 0x411f26", "ret",
        capture=sym capture,
    );
}

pub(crate) fn prefer_native(value: bool) {
    NATIVE_PRIORITY.set(value);
}

pub(crate) fn active() -> bool {
    unsafe {
        let app = layout::lawn_app();
        if app.is_null() {
            return false;
        }
        let board = LawnApp::board(app);
        !board.is_null() && capture_active(board)
    }
}

unsafe fn capture_active(board: *mut Board) -> bool {
    if ENABLED.get() && NATIVE_PRIORITY.get() {
        let cursor = unsafe { Board::cursor_object(board) };
        if !cursor.is_null() && unsafe { CursorObject::cursor_type(cursor) } != 0 {
            ENABLED.set(false);
        }
    }
    ENABLED.get()
}

// Board's Draw virtual slot (vtable 0x656CA8 +0x68). Original 0x41ACF0
// takes ECX=Board, stack Graphics*, RET 4. The hook only draws native resources.
const DRAW_SLOT: usize = 0x656d10;
const DRAW_ORIGINAL: [u8; 4] = 0x41acf0u32.to_le_bytes();
pub(crate) fn preview(value: Option<LawnPreview>) -> Result<()> {
    if value.is_some() && DRAW_PATCH.get().is_none() {
        if read_bytes(DRAW_SLOT, 4) != DRAW_ORIGINAL {
            return Err(Pvz1051Error::OperationRejected("Board draw entry is already patched"));
        }
        let patch = (draw as *const () as u32).to_le_bytes();
        write_bytes(DRAW_SLOT, &patch)?;
        DRAW_PATCH.set(Some(patch));
    } else if value.is_none() {
        if let Some(patch) = DRAW_PATCH.get() {
            restore_owned_bytes(DRAW_SLOT, &DRAW_ORIGINAL, &patch, "lawn preview")?;
            DRAW_PATCH.set(None);
        }
    }
    PREVIEW.set(value);
    Ok(())
}

unsafe extern "fastcall" fn draw(board: *mut Board, _edx: usize, graphics: *mut u8) {
    let _active = crate::runtime::hook::ActiveHookGuard::enter();
    unsafe {
        abi::board_draw(board, graphics);
        if !capture_active(board) {
            return;
        }
        let Some(preview) = PREVIEW.get() else {
            return;
        };
        let app = layout::lawn_app();
        let mouse = LawnApp::mouse_window(app);
        if LawnApp::game_ui(app) != 3 || mouse.is_null() || *mouse.cast::<u8>().add(0xdc) == 0 {
            return;
        }
        let Ok(scene) = rsvz_model::SceneKind::try_from_code(Board::scene(board)) else {
            return;
        };
        let (mx, my) = layout::MouseWindow::position(mouse);
        let (bx, by) = Board::view_position(board);
        let Some(grid) = lawn_grid(scene, mx - bx, my - by) else {
            return;
        };
        // Graphics::SetColor 0x586CC0 and SetColorizeImages 0x586D10 verify
        // these offsets. Restore only the fields used for this scoped drawing.
        let color = graphics.add(0x30).cast::<[i32; 4]>();
        let saved_color = color.read_unaligned();
        let colorize = graphics.add(0x48);
        let saved_colorize = *colorize;
        color.write_unaligned([255, 255, 255, 100]);
        *colorize = 1;
        match preview {
            LawnPreview::Plant(p) => {
                let x = abi::board_grid_to_pixel_x(grid.row, grid.col) as f32;
                let y = abi::board_grid_to_pixel_y(grid.row, grid.col) as f32
                    + abi::plant_draw_height_offset(board, i32::from(p.kind()), grid.col, grid.row);
                let (kind, imitater) = if p.imitater() >= 0 {
                    (48, i32::from(p.kind()))
                } else {
                    (i32::from(p.kind()), -1)
                };
                abi::plant_draw_seed(graphics, kind, imitater, x, y);
            }
            LawnPreview::Shovel => {
                // Same shovel resource and mouse offsets as CursorObject::Draw.
                abi::draw_shovel(graphics, mx - bx - 15, my - by - 65);
            }
        }
        color.write_unaligned(saved_color);
        *colorize = saved_colorize;
    }
}
