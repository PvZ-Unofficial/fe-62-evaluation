//! Native-produced, write-locked checkpoints. No arbitrary native-file import.
use crate::{
    Pvz1051Backend,
    error::{Pvz1051Error, Result},
    raw::abi,
};
use rsvz_backend_api::{GameUiBackend, NativeCheckpoint, NativeCheckpointBackend};
use rsvz_model::GameUi;
use std::{
    fs::OpenOptions,
    io::Write,
    os::windows::fs::OpenOptionsExt,
    time::{SystemTime, UNIX_EPOCH},
};

struct SaveContext([u32; 9]);
impl Drop for SaveContext {
    fn drop(&mut self) {
        // SAFETY: native Buffer was initialized once and still owns its buffer.
        unsafe {
            abi::replay_buffer_drop(self.0.as_mut_ptr());
        }
    }
}

const IDENTITY: &str = "pvz-1051:f9669af338964787a3785a7895791297d599295b8bb669b0db49443f736a1322:replay-2";
fn io(error: std::io::Error) -> Pvz1051Error {
    Pvz1051Error::ScriptConfig(error.to_string())
}

/// Borrowed 1051 MSVC string, read-only for the duration of a native call.
fn path_string(path: &std::path::Path) -> Result<(std::ffi::CString, [u32; 7])> {
    let path = path
        .to_str()
        .filter(|path| path.is_ascii())
        .ok_or(Pvz1051Error::OperationRejected(
            "1051 checkpoint staging path must be ASCII",
        ))?;
    let text = std::ffi::CString::new(path).map_err(|_| Pvz1051Error::OperationRejected("invalid checkpoint path"))?;
    let mut native = [0; 7];
    native[1] = text.as_ptr() as u32;
    native[5] = path.len() as u32;
    native[6] = (path.len() + 1).max(16) as u32;
    Ok((text, native))
}

impl NativeCheckpointBackend for Pvz1051Backend {
    fn apply_replay_input(&mut self, _input: rsvz_backend_api::ReplayInput) -> Result<()> {
        Err(Pvz1051Error::UnsupportedToolMode(
            "1051 replay uses sampled mouse input",
        ))
    }
    fn checkpoint_identity(&self) -> Result<&'static str> {
        Ok(IDENTITY)
    }
    fn checkpoint_ready(&self) -> Result<bool> {
        if self.game_ui()? != GameUi::Playing {
            return Ok(false);
        }
        let board = self.board()?;
        // SAFETY: the current Board is borrowed on the game thread.
        Ok(!unsafe { crate::raw::layout::Board::paused(board.as_ptr()) })
    }
    fn capture_checkpoint(&self) -> Result<NativeCheckpoint> {
        if self.game_ui()? != GameUi::Playing {
            return Err(Pvz1051Error::GameNotReady);
        }
        let board = self.board()?;
        let clock = rsvz_backend_api::ClockBackend::clock(self)?;
        let mut keys = [0; 12];
        for (index, key) in keys.iter_mut().enumerate() {
            *key = unsafe { std::ptr::read_unaligned(self.replay_allocator_header(index)?.add(20).cast::<u32>()) };
        }
        // Native modal pause owns widgets outside the serialized Board.
        if unsafe { crate::raw::layout::Board::paused(board.as_ptr()) } {
            return Err(Pvz1051Error::OperationRejected(
                "close the native pause dialog before recording",
            ));
        }
        let mode = rsvz_backend_api::NativeGameModeBackend::native_game_mode(self)?;
        let mut context = SaveContext([0; 9]);
        // SAFETY: exact 24h SaveGameContext, Buffer at +0, flags +20/+21.
        // This is the same constructor/header/SyncBoard sequence as LawnSaveGame.
        unsafe {
            abi::replay_buffer_new(context.0.as_mut_ptr());
        }
        let mut header = [0xfeeddead, 2, unsafe {
            std::ptr::read_unaligned(0x6aa7ec as *const u32)
        }];
        unsafe {
            abi::replay_sync_header(context.0.as_mut_ptr(), header.as_mut_ptr());
            abi::replay_sync_board(context.0.as_mut_ptr(), board.as_ptr());
        }
        // Native Buffer GetDataPtr/GetDataLen (5D6990/5D69C0) use +8/+14;
        // +C is vector end. Never trust a byte count beyond the owned allocation.
        let length = (u64::from(context.0[5]) + 7) / 8;
        let capacity = context.0[3].checked_sub(context.0[2]).unwrap_or(0);
        if context.0[8] & 0xff != 0 || context.0[2] == 0 || length == 0 || length > u64::from(capacity) {
            return Err(Pvz1051Error::OperationRejected(
                "native checkpoint serialization failed",
            ));
        }
        // SAFETY: this native-produced buffer is owned until context drops.
        let bytes = unsafe { std::slice::from_raw_parts(context.0[2] as *const u8, length as usize) };
        let footer = checkpoint_footer(mode, clock, keys);
        let mut owned = Vec::with_capacity(bytes.len() + footer.len());
        owned.extend_from_slice(bytes);
        owned.extend_from_slice(&footer);
        // SAFETY: complete native output and its allocator generations are owned.
        Ok(unsafe { NativeCheckpoint::from_validated_bytes(owned, IDENTITY, mode, clock) })
    }
    fn import_checkpoint(&self, source: &std::path::Path, identity: &str, mode: i32) -> Result<NativeCheckpoint> {
        if identity != IDENTITY || !(0..73).contains(&mode) {
            return Err(Pvz1051Error::OperationRejected(
                "incompatible checkpoint identity or game mode",
            ));
        }
        use std::io::Read;
        // Exclude writes/replacement while reading; restore never reopens this external path.
        let mut input = OpenOptions::new().read(true).share_mode(1).open(source).map_err(io)?;
        // Native Buffer stores bit positions in signed 32-bit fields (4813DF..481432).
        if input.metadata().map_err(io)?.len() > (i32::MAX as u64 / 8) {
            return Err(Pvz1051Error::OperationRejected(
                "checkpoint exceeds native Buffer capacity",
            ));
        }
        let mut bytes = Vec::new();
        input.read_to_end(&mut bytes).map_err(io)?;
        let (native, footer) = split_checkpoint(&bytes)?;
        let (stored_mode, clock, _) = read_checkpoint_footer(footer)?;
        if stored_mode != mode {
            return Err(Pvz1051Error::OperationRejected(
                "checkpoint mode does not match manifest",
            ));
        }
        self.validate_imported_checkpoint(native)?;
        if i32::from_le_bytes(native[21528..21532].try_into().unwrap()) != clock {
            return Err(Pvz1051Error::OperationRejected(
                "checkpoint clock does not match native state",
            ));
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Pvz1051Error::OperationRejected("invalid system clock"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rsvz-replay-{}-{nonce}.dat", std::process::id()));
        path_string(&path)?;
        let mut lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .share_mode(1)
            .open(&path)
            .map_err(io)?;
        if let Err(error) = lease.write_all(&bytes) {
            drop(lease);
            let _ = std::fs::remove_file(&path);
            return Err(io(error));
        }
        // SAFETY: framed pools, ID lists, native resource counts, scalar fields and
        // effect graphs were checked before writing this continuously protected copy.
        Ok(unsafe { NativeCheckpoint::from_validated_save(path, lease, IDENTITY, mode, clock) })
    }
    fn restore_checkpoint(&mut self, checkpoint: &NativeCheckpoint) -> Result<()> {
        if checkpoint.identity() != IDENTITY {
            return Err(Pvz1051Error::OperationRejected("incompatible checkpoint backend"));
        }
        if self.game_ui()? != GameUi::Playing {
            return Err(Pvz1051Error::GameNotReady);
        }
        // The native loader accepts paths. Materialize only the selected record,
        // not every captured checkpoint; its lease lives through the native load.
        let staged = if checkpoint.path().as_os_str().is_empty() {
            Some(stage_checkpoint(checkpoint)?)
        } else {
            None
        };
        let checkpoint = staged.as_ref().unwrap_or(checkpoint);
        if unsafe { crate::raw::layout::Board::paused(self.board()?.as_ptr()) } {
            return Err(Pvz1051Error::OperationRejected(
                "close the native pause dialog before restoring",
            ));
        }
        let (text, native) = path_string(checkpoint.path())?;
        let footer = read_protected_footer(&text)?;
        let (mode, clock, keys) = read_checkpoint_footer(&footer)?;
        if mode != checkpoint.mode() || clock != checkpoint.clock() {
            return Err(Pvz1051Error::OperationRejected("checkpoint metadata changed"));
        }
        let app = self.app();
        // SAFETY: exclusive token, only certified immutable native output enters
        // the loader. Recreating Board gives every native pool its full capacity.
        unsafe {
            crate::raw::layout::LawnApp::set_replay_game_mode(app.as_ptr(), checkpoint.mode());
            abi::replay_new_board(app.as_ptr());
            abi::replay_safe_delete(app.as_ptr());
        }
        crate::patches::world_lifetime::note_mutation();
        let board = self.board()?;
        // SAFETY: the checkpoint's write lease remains live through deserialization.
        if unsafe { abi::replay_load(board.as_ptr(), native.as_ptr()) } == 0 {
            return Err(Pvz1051Error::OperationRejected(
                "native checkpoint restore failed after Board replacement",
            ));
        }
        for (index, key) in keys.into_iter().enumerate() {
            // SAFETY: fresh native pool header, exclusive restore, bounded saved generation key.
            unsafe {
                std::ptr::write_unaligned(self.replay_allocator_header(index)?.add(20).cast::<u32>(), key);
            }
        }
        Ok(())
    }
}

fn stage_checkpoint(checkpoint: &NativeCheckpoint) -> Result<NativeCheckpoint> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Pvz1051Error::OperationRejected("invalid system clock"))?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("rsvz-replay-{}-{nonce}.dat", std::process::id()));
    path_string(&path)?;
    let mut lease = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .share_mode(1)
        .open(&path)
        .map_err(io)?;
    let result = checkpoint.read_bytes().and_then(|bytes| lease.write_all(&bytes));
    if let Err(error) = result {
        drop(lease);
        let _ = std::fs::remove_file(&path);
        return Err(io(error));
    }
    // SAFETY: copied from an immutable certified record into an exclusive lease.
    Ok(unsafe { NativeCheckpoint::from_validated_save(path, lease, IDENTITY, checkpoint.mode(), checkpoint.clock()) })
}

impl rsvz_backend_api::MouseStateBackend for Pvz1051Backend {
    fn mouse_x(&self) -> Result<i32> {
        let manager = self.replay_manager()?;
        Ok(unsafe { crate::raw::layout::MouseWindow::x(manager) })
    }
    fn mouse_y(&self) -> Result<i32> {
        let manager = self.replay_manager()?;
        Ok(unsafe { crate::raw::layout::MouseWindow::y(manager) })
    }
    fn mouse_buttons(&self) -> Result<i32> {
        let manager = self.replay_manager()?;
        Ok(unsafe { crate::raw::layout::MouseWindow::buttons(manager) })
    }
    fn cursor_image(&self) -> Result<i32> {
        Ok(unsafe { crate::raw::layout::LawnApp::cursor_num(self.app().as_ptr()) })
    }
}
impl rsvz_backend_api::RefreshCountdownWriteBackend for Pvz1051Backend {
    fn decrement_refresh_countdown(&self) -> Result<()> {
        use crate::raw::layout::Board;
        let board = self.board()?;
        // SAFETY: existing verified refresh field, no object lifetime change.
        unsafe {
            Board::set_refresh_countdown(board.as_ptr(), Board::refresh_countdown(board.as_ptr()).wrapping_sub(1));
        }
        Ok(())
    }
}

impl Pvz1051Backend {
    fn replay_manager(&self) -> Result<*mut crate::raw::layout::MouseWindow> {
        // SAFETY: current App owns the native manager. Each accessor copies one
        // scalar, with offsets evidenced at 539505/53950B/5394A5.
        let manager = unsafe { crate::raw::layout::LawnApp::mouse_window(self.app().as_ptr()) };
        if manager.is_null() {
            Err(Pvz1051Error::OperationRejected("no cursor manager"))
        } else {
            Ok(manager)
        }
    }
}

impl Pvz1051Backend {
    /// Cold preflight only: parses imported bytes and resolves known resource
    /// definitions without replacing a Board or running native load/update.
    pub(crate) fn validate_imported_checkpoint(&self, bytes: &[u8]) -> Result<()> {
        if self.game_ui()? == GameUi::Loading {
            return Err(Pvz1051Error::GameNotReady);
        }
        super::replay_format::parse(bytes, checkpoint_emitter_count, checkpoint_track_count, 1)
            .map(|_| ())
            .map_err(|error| Pvz1051Error::ScriptConfig(error.into()))
    }
}

fn checkpoint_emitter_count(kind: i32) -> std::result::Result<usize, &'static str> {
    if !(0..106).contains(&kind) {
        return Err("invalid particle definition");
    }
    // SAFETY: 481591..48159F bounds kinds to 0..105 and indexes the table
    // at 6A9F0C in eight-byte definitions. TodParticleDefinition count is +4.
    unsafe {
        let definitions = std::ptr::read_unaligned(0x6a9f0c as *const *const u8);
        if definitions.is_null() {
            return Err("particle resources are not loaded");
        }
        let count = std::ptr::read_unaligned(definitions.add(kind as usize * 8 + 4).cast::<i32>());
        usize::try_from(count).map_err(|_| "invalid native emitter definition count")
    }
}
fn checkpoint_track_count(kind: i32, record: &[u8]) -> std::result::Result<usize, &'static str> {
    if !(0..143).contains(&kind) {
        return Err("invalid reanimation definition");
    }
    let field = |offset| {
        record
            .get(offset..offset + 4)
            .map(|bytes| i32::from_le_bytes(bytes.try_into().unwrap()))
            .ok_or("truncated reanimation")
    };
    if field(0)? != kind {
        return Err("reanimation kind/definition mismatch");
    }
    // SAFETY: exact SyncReanimationDef call convention is recorded at 4814F9;
    // only a bounded built-in resource kind enters the native resource loader.
    unsafe {
        abi::replay_ensure_animation(kind, 1);
    }
    // SAFETY: 481501..481507 uses 16-byte definitions at 6A9EE8; SyncReanimation
    // 481925 reads track count at +4. Track records contain pointer,pointer,count.
    unsafe {
        let definitions = std::ptr::read_unaligned(0x6a9ee8 as *const *const u8);
        if definitions.is_null() {
            return Err("animation resources are not loaded");
        }
        let definition = definitions.add(kind as usize * 16);
        let count = std::ptr::read_unaligned(definition.add(4).cast::<i32>());
        if count < 0 || count > i32::MAX / 96 {
            return Err("invalid native track count");
        }
        let tracks = std::ptr::read_unaligned(definition.cast::<*const u8>());
        let mut frames = i32::MAX;
        if count > 0 && tracks.is_null() {
            return Err("animation tracks are not loaded");
        }
        for index in 0..count as usize {
            let track = tracks.add(index * 12);
            let transforms = std::ptr::read_unaligned(track.add(4).cast::<*const u8>());
            let length = std::ptr::read_unaligned(track.add(8).cast::<i32>());
            if length <= 0 || transforms.is_null() {
                return Err("invalid native animation frames");
            }
            frames = frames.min(length);
        }
        let start = field(24)?;
        let length = field(28)?;
        let pose = field(32)?;
        if count == 0 {
            if start != 0 || length != 0 {
                return Err("frames without native tracks");
            }
        } else if start < 0
            || start >= frames
            || length < 0
            || i64::from(start) + i64::from(length) > i64::from(frames)
            || pose < -2
            || pose >= frames
        {
            return Err("animation frame range exceeds its definition");
        }
        Ok(count as usize)
    }
}

fn checkpoint_footer(mode: i32, clock: i32, keys: [u32; 12]) -> [u8; 64] {
    let mut out = [0; 64];
    out[..8].copy_from_slice(b"RSVZ1051");
    out[8..12].copy_from_slice(&mode.to_le_bytes());
    out[12..16].copy_from_slice(&clock.to_le_bytes());
    for (index, key) in keys.into_iter().enumerate() {
        out[16 + 4 * index..20 + 4 * index].copy_from_slice(&key.to_le_bytes());
    }
    out
}
fn split_checkpoint(bytes: &[u8]) -> Result<(&[u8], &[u8])> {
    let end = bytes
        .len()
        .checked_sub(64)
        .ok_or(Pvz1051Error::OperationRejected("missing rsvz checkpoint trailer"))?;
    Ok(bytes.split_at(end))
}
fn read_checkpoint_footer(bytes: &[u8]) -> Result<(i32, i32, [u32; 12])> {
    if bytes.len() != 64 || &bytes[..8] != b"RSVZ1051" {
        return Err(Pvz1051Error::OperationRejected("invalid rsvz checkpoint trailer"));
    }
    let mode = i32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let clock = i32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let mut keys = [0; 12];
    for (index, key) in keys.iter_mut().enumerate() {
        *key = u32::from_le_bytes(bytes[16 + index * 4..20 + index * 4].try_into().unwrap());
    }
    if !(0..73).contains(&mode) || keys.iter().any(|key| !(1..65536).contains(key)) {
        return Err(Pvz1051Error::OperationRejected(
            "invalid checkpoint mode or allocator generation",
        ));
    }
    Ok((mode, clock, keys))
}
fn read_protected_footer(path: &std::ffi::CStr) -> Result<[u8; 64]> {
    use std::io::{Read, Seek, SeekFrom};
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileA, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
    };
    // Reuse the already encoded native path; no second pathname allocation in playback.
    let handle = unsafe {
        CreateFileA(
            path.as_ptr().cast(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io(std::io::Error::last_os_error()));
    }
    // SAFETY: this call owns the newly opened handle, closed by File on all exits.
    let mut file = unsafe { std::fs::File::from_raw_handle(handle) };
    file.seek(SeekFrom::End(-64)).map_err(io)?;
    let mut footer = [0; 64];
    file.read_exact(&mut footer).map_err(io)?;
    Ok(footer)
}
impl Pvz1051Backend {
    fn replay_allocator_header(&self, index: usize) -> Result<*mut u8> {
        // SyncBoard 4819EC..481B10 enumerates these six Board DataArrays and
        // the EffectSystem holders. DataArray is 0x1c bytes, next-key at +0x14.
        let board = self.board()?.as_ptr().cast::<u8>();
        unsafe {
            if index < 6 {
                return Ok(board.add(0x90 + index * 0x1c));
            }
            let effects = crate::raw::layout::LawnApp::effect_system(self.app().as_ptr()).cast::<u8>();
            if effects.is_null() {
                return Err(Pvz1051Error::GameNotReady);
            }
            let (holder_offset, array_offset) = match index {
                6 => (0, 0),
                7 => (0, 0x1c),
                8 => (0, 0x38),
                9 => (8, 0),
                10 => (4, 0),
                11 => (12, 0),
                _ => return Err(Pvz1051Error::OperationRejected("invalid replay pool")),
            };
            let holder = std::ptr::read_unaligned(effects.add(holder_offset).cast::<*mut u8>());
            if holder.is_null() {
                return Err(Pvz1051Error::GameNotReady);
            }
            Ok(holder.add(array_offset))
        }
    }
}

#[cfg(test)]
mod checkpoint_metadata_tests {
    use super::*;
    #[test]
    fn trailer_preserves_allocator_generations_and_rejects_invalid_keys() {
        let mut keys = [1001; 12];
        keys[2] = 65535;
        let footer = checkpoint_footer(13, 12345, keys);
        assert_eq!(read_checkpoint_footer(&footer).unwrap(), (13, 12345, keys));
        let mut corrupt = footer;
        corrupt[16..20].copy_from_slice(&0u32.to_le_bytes());
        assert!(read_checkpoint_footer(&corrupt).is_err());
        assert!(read_checkpoint_footer(&footer[..63]).is_err());
    }
}
