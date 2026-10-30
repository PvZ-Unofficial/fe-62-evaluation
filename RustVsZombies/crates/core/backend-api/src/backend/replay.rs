//! Native serialization is not a general safe deserialization API.
use super::Backend;
mod buffer;
pub use buffer::{BufferedCheckpoint, CheckpointStorage};
use std::{
    cell::RefCell,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    rc::Rc,
};

/// A checkpoint certified for one native format and inaccessible to external writers.
/// There is intentionally no safe constructor from a path or arbitrary bytes.
pub struct NativeCheckpoint {
    buffered: Option<BufferedCheckpoint>,
    path: PathBuf,
    // The backend keeps the file locked against writes and replacement.
    lease: Option<File>,
    range: Option<(Rc<RefCell<File>>, u64, u64)>,
    identity: &'static str,
    mode: i32,
    clock: i32,
}
impl NativeCheckpoint {
    /// # Safety
    /// `path` must identify a complete native save from `identity`, either produced
    /// natively or fully checked by that backend against its format and resources.
    /// `lease` must exclude writes/replacement for this object's entire lifetime.
    /// No unvalidated external file may be certified by this constructor.
    pub unsafe fn from_validated_save(
        path: PathBuf, lease: File, identity: &'static str, mode: i32, clock: i32,
    ) -> Self {
        Self {
            buffered: None,
            path,
            lease: Some(lease),
            range: None,
            identity,
            mode,
            clock,
        }
    }
    /// # Safety
    /// The complete range must contain a validated native record. The storage is
    /// private: all published ranges remain unchanged until their last reference
    /// is dropped. Appends may only write beyond previously published ranges.
    pub unsafe fn from_validated_range(
        storage: Rc<RefCell<File>>, offset: u64, length: u64, identity: &'static str, mode: i32, clock: i32,
    ) -> Self {
        Self {
            buffered: None,
            path: PathBuf::new(),
            lease: None,
            range: Some((storage, offset, length)),
            identity,
            mode,
            clock,
        }
    }
    /// # Safety
    /// Bytes must be complete native output or fully validated for this backend,
    /// with matching mode and clock. No external mutable storage may alias them.
    pub unsafe fn from_validated_bytes(bytes: Vec<u8>, identity: &'static str, mode: i32, clock: i32) -> Self {
        Self {
            buffered: Some(BufferedCheckpoint::new(bytes)),
            path: PathBuf::new(),
            lease: None,
            range: None,
            identity,
            mode,
            clock,
        }
    }
    pub fn buffered(&self) -> Option<BufferedCheckpoint> {
        self.buffered.clone()
    }
    /// Read only this record; never expose the writable backing file.
    pub fn read_bytes(&self) -> std::io::Result<Vec<u8>> {
        if let Some(buffer) = &self.buffered {
            return buffer.read();
        }
        let mut bytes = Vec::new();
        if let Some((storage, offset, length)) = &self.range {
            let mut file = storage.borrow_mut();
            file.seek(SeekFrom::Start(*offset))?;
            (&mut *file).take(*length).read_to_end(&mut bytes)?;
            if bytes.len() as u64 != *length {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
        } else {
            File::open(&self.path)?.read_to_end(&mut bytes)?;
        }
        Ok(bytes)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn identity(&self) -> &'static str {
        self.identity
    }
    pub fn clock(&self) -> i32 {
        self.clock
    }
    pub fn mode(&self) -> i32 {
        self.mode
    }
    /// Exporting preserves the record, but does not certify a later import.
    pub fn export(&self, destination: &Path) -> std::io::Result<()> {
        if let Some(buffer) = &self.buffered {
            return buffer.export(destination);
        }
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        if let Some((storage, offset, length)) = &self.range {
            let mut file = storage.borrow_mut();
            file.seek(SeekFrom::Start(*offset))?;
            let copied = std::io::copy(&mut (&mut *file).take(*length), &mut output)?;
            if copied != *length {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            Ok(())
        } else {
            std::io::copy(&mut File::open(&self.path)?, &mut output).map(|_| ())
        }
    }
}
impl Drop for NativeCheckpoint {
    fn drop(&mut self) {
        drop(self.lease.take());
        if self.range.is_none() && self.buffered.is_none() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Portable records ordered events; 1051 retains its sampled mouse strategy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct ReplayInput {
    pub sequence: u64,
    pub device: i64,
    pub finger: i64,
    pub clock: i32,
    pub x: i32,
    pub y: i32,
    pub button: i32,
    pub kind: u32,
}

/// Available only where native save/restore really exists. PE does not implement it.
pub trait NativeCheckpointBackend: Backend {
    fn replay_input_terminal_clock(&self) -> Result<Option<i32>, Self::Error> {
        Ok(None)
    }
    fn ordered_replay_input(&self) -> bool {
        false
    }
    // Sampled backends have no native event recorder to acquire or drain.
    fn begin_replay_input(&self, _playback: bool) -> Result<(), Self::Error> {
        Ok(())
    }
    fn drain_replay_input(&self, _output: &mut [ReplayInput]) -> Result<usize, Self::Error> {
        Ok(0)
    }
    fn apply_replay_input(&mut self, input: ReplayInput) -> Result<(), Self::Error>;
    fn end_replay_input(&self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn hand_back_replay_input(&self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn replay_input_sequence(&self) -> Result<u64, Self::Error> {
        Ok(0)
    }

    fn checkpoint_input_sequence(&self, _checkpoint: &NativeCheckpoint) -> Result<u64, Self::Error> {
        Ok(0)
    }
    fn checkpoint_identity(&self) -> Result<&'static str, Self::Error>;
    /// False while native modal UI prevents a complete checkpoint; recording can continue.
    fn checkpoint_ready(&self) -> Result<bool, Self::Error>;
    fn capture_checkpoint(&self) -> Result<NativeCheckpoint, Self::Error>;
    /// Cold preflight: reject incompatible or malformed bytes before world edits,
    /// then retain an immutable private copy for later exclusive restoration.
    fn import_checkpoint(&self, path: &Path, identity: &str, mode: i32) -> Result<NativeCheckpoint, Self::Error>;
    /// Replaces the Board and invalidates all entity identities. The exclusive
    /// receiver prevents outstanding borrowed entities; callers additionally run
    /// this only at their outer update boundary, and skip that native update.
    fn restore_checkpoint(&mut self, checkpoint: &NativeCheckpoint) -> Result<(), Self::Error>;
}

#[cfg(all(test, unix))]
mod range_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn committed_records_share_one_file_and_export_only_their_range() {
        let path = std::env::temp_dir().join(format!("rsvz-checkpoint-ranges-{}", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::fs::remove_file(&path).unwrap();
        let storage = Rc::new(RefCell::new(file));
        let weak = Rc::downgrade(&storage);
        let mut records = Vec::new();
        for index in 0..256u32 {
            storage.borrow_mut().write_all(&index.to_le_bytes()).unwrap();
            // SAFETY: the test format is exactly one LE u32; only unpublished tail is appended.
            records.push(unsafe {
                NativeCheckpoint::from_validated_range(
                    storage.clone(),
                    u64::from(index) * 4,
                    4,
                    "test-u32",
                    0,
                    index as i32,
                )
            });
        }
        #[cfg(target_os = "linux")]
        {
            let descriptors = std::fs::read_dir("/proc/self/fd")
                .unwrap()
                .filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
                .filter(|target| {
                    target
                        .to_string_lossy()
                        .contains(path.file_name().unwrap().to_str().unwrap())
                })
                .count();
            assert_eq!(descriptors, 1);
        }
        assert_eq!(records[0].read_bytes().unwrap(), 0u32.to_le_bytes());
        assert_eq!(records[255].read_bytes().unwrap(), 255u32.to_le_bytes());
        let exported = path.with_extension("export");
        records[127].export(&exported).unwrap();
        assert_eq!(std::fs::read(&exported).unwrap(), 127u32.to_le_bytes());
        std::fs::remove_file(exported).unwrap();
        drop(storage);
        drop(records);
        assert!(weak.upgrade().is_none());
    }
}
