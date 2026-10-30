//! Only immutable serialized bytes enter this thread; never game objects or callbacks.
use rsvz_backend_api::backend::replay::{BufferedCheckpoint, CheckpointStorage};
use std::{
    fs::File,
    io::{self, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_BYTES: usize = 32 * 1024 * 1024;
const BATCH_BYTES: usize = 4 * 1024 * 1024;
const MAX_DELAY: Duration = Duration::from_millis(250);

struct Record {
    data: Data,
    bytes: usize,
    pending: Arc<AtomicUsize>,
}
enum Data {
    Checkpoint(BufferedCheckpoint, PathBuf),
    Input(Arc<Mutex<File>>, u64, Arc<Vec<u8>>),
}
impl Drop for Record {
    fn drop(&mut self) {
        self.pending.fetch_sub(self.bytes, Ordering::Release);
    }
}
enum Command {
    Write(Record),
    Flush(mpsc::Sender<()>),
}
pub(super) struct Writer {
    send: Option<SyncSender<Command>>,
    thread: Option<JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
    pending: Arc<AtomicUsize>,
}
impl Writer {
    pub fn new() -> io::Result<Self> {
        let storage = CheckpointStorage::new()?;
        let (send, receive) = mpsc::sync_channel(256);
        let error = Arc::new(Mutex::new(None));
        let errors = error.clone();
        let thread = thread::Builder::new()
            .name("rsvz-replay-writer".into())
            .spawn(move || {
                run(receive, storage, errors);
            })?;
        Ok(Self {
            send: Some(send),
            thread: Some(thread),
            error,
            pending: Arc::new(AtomicUsize::new(0)),
        })
    }
    pub fn check(&self) -> io::Result<()> {
        if let Some(error) = &*self
            .error
            .lock()
            .map_err(|_| io::Error::other("recording writer poisoned"))?
        {
            return Err(io::Error::other(error.clone()));
        }
        if self.thread.as_ref().is_some_and(|thread| thread.is_finished()) {
            return Err(io::Error::other("recording writer stopped unexpectedly"));
        }
        Ok(())
    }
    pub fn append(&self, data: BufferedCheckpoint, path: PathBuf) -> io::Result<()> {
        let bytes = data.resident_bytes()?;
        self.enqueue(Data::Checkpoint(data, path), bytes)
    }
    pub fn input(&self, file: Arc<Mutex<File>>, offset: u64, data: Arc<Vec<u8>>) -> io::Result<()> {
        let bytes = data.capacity();
        self.enqueue(Data::Input(file, offset, data), bytes)
    }
    fn enqueue(&self, data: Data, bytes: usize) -> io::Result<()> {
        self.check()?;
        self.pending
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
                pending.checked_add(bytes).filter(|sum| *sum <= MAX_BYTES)
            })
            .map_err(|_| {
                io::Error::other("recording disk writer cannot keep up (32 MiB pending); recording stopped")
            })?;
        let record = Record {
            data,
            bytes,
            pending: self.pending.clone(),
        };
        self.send
            .as_ref()
            .expect("writer still open")
            .try_send(Command::Write(record))
            .map_err(|_| io::Error::other("recording write queue unavailable or full; recording stopped"))
    }
    pub fn flush(&self) -> io::Result<()> {
        let (done, receive) = mpsc::channel();
        self.send
            .as_ref()
            .expect("writer still open")
            .send(Command::Flush(done))
            .map_err(|_| io::Error::other("recording writer disconnected"))?;
        receive
            .recv()
            .map_err(|_| io::Error::other("recording writer failed during flush"))?;
        self.check()
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.send.take();
        if let Some(thread) = self.thread.take() {
            // DLL code must stay mapped until all disk work, including failures, has ended.
            let _ = thread.join();
        }
    }
}
fn run(receive: Receiver<Command>, storage: CheckpointStorage, error: Arc<Mutex<Option<String>>>) {
    let mut batch = Vec::with_capacity(256);
    let mut bytes = 0;
    let mut deadline = Instant::now();
    let flush = |batch: &mut Vec<Record>| {
        for record in batch.drain(..) {
            let failed = error.lock().expect("writer error lock").is_some();
            if !failed {
                let result = match &record.data {
                    Data::Checkpoint(data, path) => data.persist(&storage, path),
                    Data::Input(file, offset, bytes) => (|| {
                        let mut file = file.lock().map_err(|_| io::Error::other("input storage poisoned"))?;
                        file.seek(SeekFrom::Start(*offset))?;
                        file.write_all(bytes)
                    })(),
                };
                if let Err(failure) = result {
                    *error.lock().expect("writer error lock") = Some(failure.to_string());
                }
            }
        }
    };
    loop {
        let command = if batch.is_empty() {
            receive.recv().map_err(|_| RecvTimeoutError::Disconnected)
        } else {
            receive.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        };
        match command {
            Ok(Command::Write(record)) => {
                if batch.is_empty() {
                    deadline = Instant::now() + MAX_DELAY;
                }
                bytes += record.bytes;
                batch.push(record);
                if bytes >= BATCH_BYTES || batch.len() == 256 {
                    flush(&mut batch);
                    bytes = 0;
                }
            }
            Ok(Command::Flush(done)) => {
                flush(&mut batch);
                bytes = 0;
                let _ = done.send(());
            }
            Err(RecvTimeoutError::Timeout) => {
                flush(&mut batch);
                bytes = 0;
            }
            Err(RecvTimeoutError::Disconnected) => {
                flush(&mut batch);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz_backend_api::NativeCheckpoint;

    #[test]
    fn bounded_writer_preserves_private_history_reports_failure_and_joins() {
        let root = std::env::temp_dir().join(format!(
            "rsvz-writer-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        // SAFETY: this test's certified format is just the supplied immutable byte string.
        let checkpoint = unsafe { NativeCheckpoint::from_validated_bytes(b"checkpoint".to_vec(), "test", 1, 2) };
        let writer = Writer::new().unwrap();
        let first = root.join("first.dat");
        writer.append(checkpoint.buffered().unwrap(), first.clone()).unwrap();
        assert_eq!(checkpoint.read_bytes().unwrap(), b"checkpoint");
        writer.flush().unwrap();
        assert_eq!(writer.pending.load(Ordering::Acquire), 0);
        assert_eq!(checkpoint.buffered().unwrap().resident_bytes().unwrap(), 0);
        std::fs::write(&first, b"external edit").unwrap();
        assert_eq!(checkpoint.read_bytes().unwrap(), b"checkpoint");

        let mut oversized = Vec::with_capacity(MAX_BYTES + 1);
        oversized.push(1);
        // SAFETY: immutable bytes in the local test format.
        let oversized = unsafe { NativeCheckpoint::from_validated_bytes(oversized, "test", 1, 3) };
        assert!(
            writer
                .append(oversized.buffered().unwrap(), root.join("too-large.dat"))
                .is_err()
        );
        assert_eq!(writer.pending.load(Ordering::Acquire), 0);
        assert!(!root.join("too-large.dat").exists());

        // SAFETY: immutable bytes in the local test format.
        let second = unsafe { NativeCheckpoint::from_validated_bytes(b"second".to_vec(), "test", 1, 3) };
        let second_path = root.join("second.dat");
        writer.append(second.buffered().unwrap(), second_path.clone()).unwrap();
        drop(writer);
        assert_eq!(std::fs::read(&second_path).unwrap(), b"second");
        assert_eq!(checkpoint.read_bytes().unwrap(), b"checkpoint");
        assert_eq!(second.read_bytes().unwrap(), b"second");

        let writer = Writer::new().unwrap();
        // SAFETY: immutable bytes in the local test format.
        let failed = unsafe { NativeCheckpoint::from_validated_bytes(b"replacement".to_vec(), "test", 1, 4) };
        writer.append(failed.buffered().unwrap(), first.clone()).unwrap();
        assert!(writer.flush().is_err());
        assert!(writer.check().is_err());
        assert_eq!(std::fs::read(&first).unwrap(), b"external edit");
        assert_eq!(failed.read_bytes().unwrap(), b"replacement");
        drop(writer);
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second_path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
