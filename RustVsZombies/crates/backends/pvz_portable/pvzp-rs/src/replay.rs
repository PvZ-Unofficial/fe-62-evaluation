//! Exclusive native serialization bridge; arbitrary paths are never safe loads.
use crate::{Error, Result, raw};
use std::ffi::CStr;

/// Synchronous game-thread serialization; the returned allocation contains no game pointers.
///
/// # Safety
/// Caller must hold a live backend access scope on the game's thread.
pub unsafe fn capture_checkpoint() -> Result<Vec<u8>> {
    unsafe extern "C" fn receive(context: *mut std::ffi::c_void, bytes: *const u8, length: u64) -> u8 {
        if bytes.is_null() || length == 0 || length > i32::MAX as u64 / 8 {
            return 0;
        }
        let length = length as usize;
        // SAFETY: native borrows the buffer for this callback; context is our live Vec.
        let output = unsafe { &mut *context.cast::<Vec<u8>>() };
        // Reserve the backend's fixed 16-byte footer without reallocating the snapshot.
        if output.try_reserve_exact(length + 16).is_err() {
            return 0;
        }
        output.extend_from_slice(unsafe { std::slice::from_raw_parts(bytes, length) });
        1
    }
    let mut bytes = Vec::new();
    // SAFETY: native invokes receive synchronously and retains neither pointer.
    if unsafe { raw::pvzp_rs_replay_capture((&mut bytes as *mut Vec<u8>).cast(), Some(receive)) } != 0 {
        Ok(bytes)
    } else {
        Err(Error::InvalidArgument { detail: 0 })
    }
}

/// # Safety
/// `file` is a live writable Windows file handle borrowed for this call.
pub unsafe fn save_checkpoint(file: usize) -> Result<()> {
    // SAFETY: caller retains the handle, native validates the current battle.
    if unsafe { raw::pvzp_rs_replay_save(file as u64) } != 0 {
        Ok(())
    } else {
        Err(Error::InvalidArgument { detail: 0 })
    }
}

/// # Safety
/// Caller must own exclusive App/Board access and provide a trusted immutable
/// native save from this exact SDK. All previous entity references are invalid.
pub unsafe fn restore_checkpoint(path: &CStr, mode: i32) -> Result<()> {
    // SAFETY: caller provides the serialization and exclusive-access guarantees.
    if unsafe { raw::pvzp_rs_replay_restore(path.as_ptr(), mode) } != 0 {
        Ok(())
    } else {
        Err(Error::InvalidArgument { detail: 1 })
    }
}
