//! Immutable serialized bytes can be spilled without accessing the game.
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

enum Storage {
    Memory(Arc<Vec<u8>>),
    File(Arc<Mutex<File>>, u64, u64),
}

#[derive(Clone)]
pub struct BufferedCheckpoint(Arc<Mutex<Storage>>);

/// Private append-only history shared by completed records; no writable path escapes.
pub struct CheckpointStorage(Arc<Mutex<File>>);

fn lock<T>(value: &Mutex<T>) -> io::Result<std::sync::MutexGuard<'_, T>> {
    value
        .lock()
        .map_err(|_| io::Error::other("checkpoint storage poisoned"))
}

impl CheckpointStorage {
    pub fn new() -> io::Result<Self> {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        loop {
            let path = std::env::temp_dir().join(format!(
                "rsvz-checkpoints-{}-{nonce}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.share_mode(5); // Allow unlink, exclude foreign writers.
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(file) => {
                    std::fs::remove_file(path)?;
                    return Ok(Self(Arc::new(Mutex::new(file))));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

impl BufferedCheckpoint {
    pub(super) fn new(bytes: Vec<u8>) -> Self {
        Self(Arc::new(Mutex::new(Storage::Memory(Arc::new(bytes)))))
    }
    pub fn resident_bytes(&self) -> io::Result<usize> {
        Ok(match &*lock(&self.0)? {
            Storage::Memory(bytes) => bytes.capacity(),
            Storage::File(..) => 0,
        })
    }
    pub fn read(&self) -> io::Result<Vec<u8>> {
        match &*lock(&self.0)? {
            Storage::Memory(bytes) => Ok((**bytes).clone()),
            Storage::File(file, offset, length) => {
                let mut file = lock(file)?;
                file.seek(SeekFrom::Start(*offset))?;
                let mut bytes = Vec::new();
                (&mut *file).take(*length).read_to_end(&mut bytes)?;
                if bytes.len() as u64 != *length {
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                Ok(bytes)
            }
        }
    }
    pub fn export(&self, destination: &Path) -> io::Result<()> {
        let mut output = OpenOptions::new().write(true).create_new(true).open(destination)?;
        match &*lock(&self.0)? {
            Storage::Memory(bytes) => output.write_all(bytes),
            Storage::File(file, offset, length) => {
                let mut file = lock(file)?;
                file.seek(SeekFrom::Start(*offset))?;
                let copied = io::copy(&mut (&mut *file).take(*length), &mut output)?;
                if copied != *length {
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                Ok(())
            }
        }
    }
    /// The public export is not trusted as the backing store: users may modify it.
    pub fn persist(&self, storage: &CheckpointStorage, destination: &Path) -> io::Result<()> {
        let bytes = match &*lock(&self.0)? {
            Storage::Memory(bytes) => bytes.clone(),
            Storage::File(..) => return Err(io::Error::other("checkpoint already persisted")),
        };
        let mut output = OpenOptions::new().write(true).create_new(true).open(destination)?;
        output.write_all(&bytes)?;
        let mut file = lock(&storage.0)?;
        let offset = file.seek(SeekFrom::End(0))?;
        file.write_all(&bytes)?;
        let length = bytes.len() as u64;
        drop(file);
        *lock(&self.0)? = Storage::File(storage.0.clone(), offset, length);
        Ok(())
    }
}
