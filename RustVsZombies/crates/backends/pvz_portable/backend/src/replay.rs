use crate::{PortableBackend, PortableBackendError, Result};
use rsvz_backend_api::{NativeCheckpoint, NativeCheckpointBackend};
use std::{
    fs::OpenOptions,
    time::{SystemTime, UNIX_EPOCH},
};

// Immutable executable-artifact metadata, not a cached game query or object address.
// Fixed bytes have no DLL-local heap destructor and are shared by both sources.
static IDENTITY: std::sync::OnceLock<[u8; 78]> = std::sync::OnceLock::new();
fn identity() -> Result<&'static str> {
    if IDENTITY.get().is_none() {
        #[cfg(target_os = "android")]
        let value = {
            // Android's current_exe is app_process, not the game's native library.
            // The Activity hashes the actual APK entry before starting the game.
            let hash = std::env::var("RSVZ_ANDROID_NATIVE_SHA256")
                .map_err(|_| PortableBackendError::OperationRejected("missing Android native artifact identity"))?;
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(PortableBackendError::OperationRejected(
                    "invalid Android native artifact identity",
                ));
            }
            let mut value = [0u8; 78];
            value[..14].copy_from_slice(b"pvzp:replay-2:");
            value[14..].copy_from_slice(hash.as_bytes());
            value
        };
        #[cfg(not(target_os = "android"))]
        let value = {
            use sha2::{Digest, Sha256};
            use std::io::Read;
            let mut file = std::fs::File::open(std::env::current_exe().map_err(rejected)?).map_err(rejected)?;
            let mut hasher = Sha256::new();
            let mut buffer = [0u8; 8192];
            loop {
                let size = file.read(&mut buffer).map_err(rejected)?;
                if size == 0 {
                    break;
                }
                hasher.update(&buffer[..size]);
            }
            let mut value = [0u8; 78];
            value[..14].copy_from_slice(b"pvzp:replay-2:");
            for (index, byte) in hasher.finalize().iter().enumerate() {
                value[14 + index * 2] = b"0123456789abcdef"[usize::from(byte >> 4)];
                value[15 + index * 2] = b"0123456789abcdef"[usize::from(byte & 15)];
            }
            value
        };
        let _ = IDENTITY.set(value);
    }
    Ok(std::str::from_utf8(IDENTITY.get().expect("initialized artifact identity")).expect("ASCII identity"))
}
fn rejected(_: std::io::Error) -> PortableBackendError {
    PortableBackendError::OperationRejected("checkpoint file I/O failed")
}
impl NativeCheckpointBackend for PortableBackend {
    fn replay_input_terminal_clock(&self) -> Result<Option<i32>> {
        // SAFETY: a scalar input-recorder boundary, including a destroyed world's final clock.
        Ok(Some(unsafe { pvzp_rs::raw::pvzp_rs_input_terminal_clock() }))
    }
    fn ordered_replay_input(&self) -> bool {
        true
    }
    fn begin_replay_input(&self, playback: bool) -> Result<()> {
        // SAFETY: the native call validates recorder ownership before mutation.
        if unsafe { pvzp_rs::raw::pvzp_rs_input_begin(u8::from(playback)) } != 0 {
            Ok(())
        } else {
            Err(PortableBackendError::OperationRejected(
                "native demo conflicts with Replay input",
            ))
        }
    }
    fn drain_replay_input(&self, output: &mut [rsvz_backend_api::ReplayInput]) -> Result<usize> {
        use pvzp_rs::raw::PointerInput as Native;
        use rsvz_backend_api::ReplayInput as Event;
        use std::mem::{align_of, offset_of, size_of};
        const {
            assert!(size_of::<Event>() == size_of::<Native>());
            assert!(align_of::<Event>() == align_of::<Native>());
        }
        macro_rules! field {
            ($name:ident) => {
                const {
                    assert!(offset_of!(Event, $name) == offset_of!(Native, $name));
                }
            };
        }
        field!(sequence);
        field!(device);
        field!(finger);
        field!(clock);
        field!(x);
        field!(y);
        field!(button);
        field!(kind);
        let mut count = 0;
        // SAFETY: all scalar layouts are checked above, and output has the supplied capacity.
        if unsafe { pvzp_rs::raw::pvzp_rs_input_drain(output.as_mut_ptr().cast(), output.len() as _, &mut count) } == 0
        {
            return Err(PortableBackendError::OperationRejected("native input journal overflow"));
        }
        Ok(count as usize)
    }
    fn apply_replay_input(&mut self, event: rsvz_backend_api::ReplayInput) -> Result<()> {
        let native = pvzp_rs::raw::PointerInput {
            sequence: event.sequence,
            device: event.device,
            finger: event.finger,
            clock: event.clock,
            x: event.x,
            y: event.y,
            button: event.button,
            kind: event.kind,
        };
        // SAFETY: exclusive backend access; the pointer is borrowed only during this call.
        if unsafe { pvzp_rs::raw::pvzp_rs_input_apply(&native) } != 0 {
            Ok(())
        } else {
            Err(PortableBackendError::OperationRejected(
                "invalid replay input or missing ownership",
            ))
        }
    }
    fn end_replay_input(&self) -> Result<()> {
        // SAFETY: only releases the native recorder owned by this Replay.
        unsafe {
            pvzp_rs::raw::pvzp_rs_input_end();
        }
        Ok(())
    }
    fn hand_back_replay_input(&self) -> Result<()> {
        // SAFETY: this is called at the outer handoff boundary with no entity borrows.
        unsafe {
            pvzp_rs::raw::pvzp_rs_input_hand_back();
        }
        Ok(())
    }
    fn replay_input_sequence(&self) -> Result<u64> {
        // SAFETY: scalar observation on the game thread.
        Ok(unsafe { pvzp_rs::raw::pvzp_rs_input_sequence() })
    }

    fn checkpoint_input_sequence(&self, checkpoint: &NativeCheckpoint) -> Result<u64> {
        crate::replay_format::input_sequence(&checkpoint.read_bytes().map_err(rejected)?)
            .map_err(PortableBackendError::OperationRejected)
    }
    fn checkpoint_identity(&self) -> Result<&'static str> {
        identity()
    }
    fn checkpoint_ready(&self) -> Result<bool> {
        // SAFETY: scalar readiness query on the game thread; no mutation.
        Ok(unsafe { pvzp_rs::raw::pvzp_rs_checkpoint_ready() } != 0)
    }
    fn capture_checkpoint(&self) -> Result<NativeCheckpoint> {
        let mode = pvzp_rs::native_game_mode()?;
        let clock = rsvz_backend_api::ClockBackend::clock(self)?;
        // SAFETY: this backend call runs in the current game-thread access scope.
        let mut bytes = unsafe { pvzp_rs::replay::capture_checkpoint()? };
        bytes.extend_from_slice(&checkpoint_footer(mode, clock));
        // SAFETY: native serialization produced these owned bytes on the game thread.
        Ok(unsafe { NativeCheckpoint::from_validated_bytes(bytes, identity()?, mode, clock) })
    }
    fn import_checkpoint(&self, source: &std::path::Path, expected: &str, mode: i32) -> Result<NativeCheckpoint> {
        if expected != identity()? || !(0..73).contains(&mode) {
            return Err(PortableBackendError::OperationRejected(
                "incompatible checkpoint identity or mode",
            ));
        }
        #[cfg(unix)]
        {
            return unix_storage::import(self, source, mode);
        }
        #[cfg(windows)]
        {
            use std::{
                io::{Read, Write},
                os::windows::fs::OpenOptionsExt,
            };
            if rsvz_backend_api::GameUiBackend::game_ui(self)? == rsvz_model::GameUi::Loading {
                return Err(PortableBackendError::OperationRejected(
                    "game resources are still loading",
                ));
            }
            let mut input = OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(source)
                .map_err(rejected)?;
            if input.metadata().map_err(rejected)?.len() > i32::MAX as u64 / 8 {
                return Err(PortableBackendError::OperationRejected(
                    "checkpoint exceeds native Buffer capacity",
                ));
            }
            let mut bytes = Vec::new();
            input.read_to_end(&mut bytes).map_err(rejected)?;
            let native_length = bytes
                .len()
                .checked_sub(16)
                .ok_or(PortableBackendError::OperationRejected("missing checkpoint metadata"))?;
            let (stored_mode, clock) = read_footer(&bytes[native_length..])?;
            if stored_mode != mode {
                return Err(PortableBackendError::OperationRejected("checkpoint mode mismatch"));
            }
            let native_clock = crate::replay_format::validate(&bytes[..native_length], emitter_count, animation_counts)
                .map_err(PortableBackendError::OperationRejected)?;
            if native_clock != clock {
                return Err(PortableBackendError::OperationRejected("checkpoint clock mismatch"));
            }
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| PortableBackendError::OperationRejected("invalid system clock"))?
                .as_nanos();
            let path = std::env::temp_dir().join(format!("rsvz-replay-{}-{nonce}.dat", std::process::id()));
            let mut lease = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .share_mode(1)
                .open(&path)
                .map_err(rejected)?;
            if let Err(error) = lease.write_all(&bytes) {
                drop(lease);
                let _ = std::fs::remove_file(&path);
                return Err(rejected(error));
            }
            // SAFETY: complete SAVE4 framing/pools/resources were checked before
            // this immutable private copy was certified; later loads use only it.
            Ok(unsafe { NativeCheckpoint::from_validated_save(path, lease, identity()?, mode, clock) })
        }
    }
    fn restore_checkpoint(&mut self, checkpoint: &NativeCheckpoint) -> Result<()> {
        if checkpoint.identity() != identity()? {
            return Err(PortableBackendError::OperationRejected(
                "incompatible checkpoint backend",
            ));
        }
        let bytes = checkpoint.read_bytes().map_err(rejected)?;
        // SAFETY: exclusive backend token and an immutable certified native record.
        if unsafe { pvzp_rs::raw::pvzp_rs_replay_restore_bytes(bytes.as_ptr(), bytes.len() as _, checkpoint.mode()) }
            == 0
        {
            return Err(PortableBackendError::OperationRejected("checkpoint restore failed"));
        }
        Ok(())
    }
}
impl rsvz_backend_api::MouseStateBackend for PortableBackend {
    fn mouse_x(&self) -> Result<i32> {
        read_input(pvzp_rs::raw::pvzp_rs_replay_mouse_x)
    }
    fn mouse_y(&self) -> Result<i32> {
        read_input(pvzp_rs::raw::pvzp_rs_replay_mouse_y)
    }
    fn mouse_buttons(&self) -> Result<i32> {
        read_input(pvzp_rs::raw::pvzp_rs_replay_mouse_buttons)
    }
    fn cursor_image(&self) -> Result<i32> {
        read_input(pvzp_rs::raw::pvzp_rs_replay_cursor_kind)
    }
}
impl rsvz_backend_api::RefreshCountdownWriteBackend for PortableBackend {
    fn decrement_refresh_countdown(&self) -> Result<()> {
        // SAFETY: bridge checks current Board and changes one native scalar.
        if unsafe { pvzp_rs::raw::pvzp_rs_replay_refresh() } == 0 {
            return Err(PortableBackendError::BoardUnavailable);
        }
        Ok(())
    }
}
impl rsvz_backend_api::TransientClearBackend for PortableBackend {
    fn clear_native_projectiles(&mut self) -> Result<()> {
        // SAFETY: exclusive backend lifetime; native checks current Board.
        if unsafe { pvzp_rs::raw::pvzp_rs_clear_projectiles() } == 0 {
            return Err(PortableBackendError::OperationRejected("projectile cleanup failed"));
        }
        Ok(())
    }
    fn clear_native_items(&mut self) -> Result<()> {
        // SAFETY: exclusive backend lifetime; caller has released its held cursor.
        if unsafe { pvzp_rs::raw::pvzp_rs_clear_items() } == 0 {
            return Err(PortableBackendError::OperationRejected("item cleanup failed"));
        }
        Ok(())
    }
}

fn read_input(read: unsafe extern "C" fn(*mut i32) -> u8) -> Result<i32> {
    let mut value = 0;
    // SAFETY: fixed native scalar getter and live stack output; it checks App.
    if unsafe { read(&mut value) } == 0 {
        return Err(PortableBackendError::OperationRejected("native input unavailable"));
    }
    Ok(value)
}

fn checkpoint_footer(mode: i32, clock: i32) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(b"RSVZPVZP");
    bytes[8..12].copy_from_slice(&mode.to_le_bytes());
    bytes[12..16].copy_from_slice(&clock.to_le_bytes());
    bytes
}
fn read_footer(bytes: &[u8]) -> Result<(i32, i32)> {
    if bytes.len() != 16 || &bytes[..8] != b"RSVZPVZP" {
        return Err(PortableBackendError::OperationRejected("invalid checkpoint trailer"));
    }
    Ok((
        i32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        i32::from_le_bytes(bytes[12..16].try_into().unwrap()),
    ))
}
fn emitter_count(kind: i32) -> std::result::Result<usize, &'static str> {
    // SAFETY: only a scalar kind enters the native resource query, which validates it.
    usize::try_from(unsafe { pvzp_rs::raw::pvzp_rs_replay_emitter_count(kind) })
        .map_err(|_| "particle definition unavailable")
}
fn animation_counts(kind: i32) -> std::result::Result<(usize, usize), &'static str> {
    // SAFETY: these load only a bounded built-in animation definition, never a Board.
    let tracks = usize::try_from(unsafe { pvzp_rs::raw::pvzp_rs_replay_track_count(kind) })
        .map_err(|_| "animation definition unavailable")?;
    let frames = usize::try_from(unsafe { pvzp_rs::raw::pvzp_rs_replay_frame_count(kind) })
        .map_err(|_| "animation frames unavailable")?;
    Ok((tracks, frames))
}

#[cfg(unix)]
mod unix_storage {
    use super::*;
    use std::{
        cell::RefCell,
        fs::File,
        io::{Read, Seek, SeekFrom, Write},
        rc::{Rc, Weak},
    };
    thread_local! { static STORAGE: RefCell<Weak<RefCell<File>>> = const { RefCell::new(Weak::new()) }; }

    fn storage() -> Result<Rc<RefCell<File>>> {
        STORAGE.with_borrow_mut(|weak| {
            if let Some(storage) = weak.upgrade() {
                return Ok(storage);
            }
            use std::os::unix::fs::OpenOptionsExt;
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| PortableBackendError::OperationRejected("invalid system clock"))?
                .as_nanos();
            let path = std::env::temp_dir().join(format!("rsvz-replay-{}-{nonce}", std::process::id()));
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .map_err(rejected)?;
            std::fs::remove_file(&path).map_err(rejected)?;
            let storage = Rc::new(RefCell::new(file));
            *weak = Rc::downgrade(&storage);
            Ok(storage)
        })
    }

    fn append(mode: i32, clock: i32, write: impl FnOnce(&mut File) -> Result<()>) -> Result<NativeCheckpoint> {
        let storage = storage()?;
        let (offset, length) = {
            let mut file = storage.borrow_mut();
            let offset = file.seek(SeekFrom::End(0)).map_err(rejected)?;
            // A failed write leaves only an unpublished tail; older ranges never change.
            write(&mut file)?;
            let end = file.stream_position().map_err(rejected)?;
            (offset, end - offset)
        };
        // SAFETY: only append can write the private anonymous storage; committed ranges
        // are immutable and every reader/export is bounded by this completed record.
        Ok(unsafe { NativeCheckpoint::from_validated_range(storage, offset, length, identity()?, mode, clock) })
    }

    pub fn import(backend: &PortableBackend, source: &std::path::Path, mode: i32) -> Result<NativeCheckpoint> {
        if rsvz_backend_api::GameUiBackend::game_ui(backend)? == rsvz_model::GameUi::Loading {
            return Err(PortableBackendError::OperationRejected(
                "game resources are still loading",
            ));
        }
        let file = File::open(source).map_err(rejected)?;
        let maximum = i32::MAX as u64 / 8;
        if file.metadata().map_err(rejected)?.len() > maximum {
            return Err(PortableBackendError::OperationRejected(
                "checkpoint exceeds native Buffer capacity",
            ));
        }
        let mut bytes = Vec::new();
        file.take(maximum + 1).read_to_end(&mut bytes).map_err(rejected)?;
        if bytes.len() as u64 > maximum {
            return Err(PortableBackendError::OperationRejected(
                "checkpoint exceeds native Buffer capacity",
            ));
        }
        let native_length = bytes
            .len()
            .checked_sub(16)
            .ok_or(PortableBackendError::OperationRejected("missing checkpoint metadata"))?;
        let (stored_mode, clock) = read_footer(&bytes[native_length..])?;
        let native_clock = crate::replay_format::validate(&bytes[..native_length], emitter_count, animation_counts)
            .map_err(PortableBackendError::OperationRejected)?;
        if stored_mode != mode || native_clock != clock {
            return Err(PortableBackendError::OperationRejected("checkpoint metadata mismatch"));
        }
        append(mode, clock, |file| file.write_all(&bytes).map_err(rejected))
    }
}
