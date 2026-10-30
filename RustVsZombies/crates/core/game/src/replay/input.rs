//! Ordered pointer events live in one private, unlinked file, not per-tick vectors.
use super::{RuntimeError, RuntimeResult, io};
use rsvz_backend_api::ReplayInput;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const WIDTH: u64 = 44;
static FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct InputHistory {
    file: Arc<Mutex<File>>,
    tail: Vec<u8>,
    last_flush: Instant,
    pub count: u64,
    pub base_sequence: u64,
    pub last_sequence: u64,
    pub terminal_clock: i32,
    pub world_ended: bool,
    pub terminal_snapshot: bool,
    pub playback_offset: u64,
    pub batch: Box<[ReplayInput]>,
}

impl InputHistory {
    pub fn new(sequence: u64, clock: i32) -> RuntimeResult<Self> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| RuntimeError::new(error.to_string()))?
            .as_nanos();
        let file = loop {
            let serial = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("rsvz-input-{}-{nonce}-{serial}", std::process::id()));
            match OpenOptions::new().read(true).write(true).create_new(true).open(&path) {
                Ok(file) => {
                    std::fs::remove_file(path).map_err(io)?;
                    break file;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io(error)),
            }
        };
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            tail: Vec::with_capacity(4096 * WIDTH as usize),
            last_flush: Instant::now(),
            count: 0,
            base_sequence: sequence,
            last_sequence: sequence,
            terminal_clock: clock,
            world_ended: false,
            terminal_snapshot: false,
            playback_offset: 0,
            batch: vec![ReplayInput::default(); 4096].into_boxed_slice(),
        })
    }
    pub fn append_batch(&mut self, count: usize, writer: Option<&super::writer::Writer>) -> RuntimeResult<()> {
        for event in &self.batch[..count] {
            if Some(event.sequence) != self.last_sequence.checked_add(1)
                || event.clock < self.terminal_clock
                || event.kind > 8
            {
                return Err(RuntimeError::new("unordered native input journal"));
            }
            self.tail.extend_from_slice(&encode(*event));
            self.last_sequence = event.sequence;
            self.terminal_clock = event.clock;
            self.count += 1;
        }
        if !self.tail.is_empty()
            && (self.tail.len() >= 64 * 1024 || self.last_flush.elapsed() >= Duration::from_millis(250))
        {
            let offset = self.count * WIDTH - self.tail.len() as u64;
            if let Some(writer) = writer {
                let bytes = Arc::new(std::mem::take(&mut self.tail));
                if let Err(error) = writer.input(self.file.clone(), offset, bytes.clone()) {
                    self.tail = Arc::unwrap_or_clone(bytes);
                    return Err(io(error));
                }
                self.tail = Vec::with_capacity(4096 * WIDTH as usize);
            } else {
                let mut file = self
                    .file
                    .lock()
                    .map_err(|_| RuntimeError::new("input storage poisoned"))?;
                file.seek(SeekFrom::Start(offset)).map_err(io)?;
                file.write_all(&self.tail).map_err(io)?;
                self.tail.clear();
            }
            self.last_flush = Instant::now();
        }
        Ok(())
    }
    pub fn read(&self, index: u64) -> RuntimeResult<ReplayInput> {
        if index >= self.count {
            return Err(RuntimeError::new("input index outside recording"));
        }
        let mut bytes = [0; WIDTH as usize];
        let tail_start = self.count * WIDTH - self.tail.len() as u64;
        if index * WIDTH >= tail_start {
            let offset = (index * WIDTH - tail_start) as usize;
            bytes.copy_from_slice(&self.tail[offset..offset + WIDTH as usize]);
            return decode(bytes);
        }
        let mut file = self
            .file
            .lock()
            .map_err(|_| RuntimeError::new("input storage poisoned"))?;
        file.seek(SeekFrom::Start(index * WIDTH)).map_err(io)?;
        file.read_exact(&mut bytes).map_err(io)?;
        decode(bytes)
    }
    pub fn seek_sequence(&mut self, sequence: u64) -> RuntimeResult<()> {
        if sequence < self.base_sequence || sequence > self.last_sequence {
            return Err(RuntimeError::new("input watermark outside history"));
        }
        // Native sequences are contiguous: the watermark also indexes the sidecar.
        self.playback_offset = sequence - self.base_sequence;
        Ok(())
    }
    pub fn branch(&mut self, sequence: u64, clock: i32) -> RuntimeResult<()> {
        self.seek_sequence(sequence)?;
        let tail_start = self.count * WIDTH - self.tail.len() as u64;
        self.count = self.playback_offset;
        if self.count * WIDTH >= tail_start {
            self.tail.truncate((self.count * WIDTH - tail_start) as usize);
        } else {
            self.tail.clear();
            self.file
                .lock()
                .map_err(|_| RuntimeError::new("input storage poisoned"))?
                .set_len(self.count * WIDTH)
                .map_err(io)?;
        }
        self.last_sequence = sequence;
        self.terminal_clock = clock;
        self.world_ended = false;
        self.terminal_snapshot = false;
        Ok(())
    }
    pub fn export(&self, path: &Path) -> RuntimeResult<()> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| RuntimeError::new("input storage poisoned"))?;
        file.seek(SeekFrom::Start(0)).map_err(io)?;
        let mut output = File::create(path).map_err(io)?;
        let length = self.count * WIDTH - self.tail.len() as u64;
        if std::io::copy(&mut (&mut *file).take(length), &mut output).map_err(io)? != length {
            return Err(RuntimeError::new("incomplete input journal"));
        }
        output.write_all(&self.tail).map_err(io)?;
        Ok(())
    }
    pub fn import(path: &Path, base: u64, terminal: (i32, u64)) -> RuntimeResult<Self> {
        let mut source = File::open(path).map_err(io)?;
        let length = source.metadata().map_err(io)?.len();
        if length % WIDTH != 0 {
            return Err(RuntimeError::new("truncated input sidecar"));
        }
        let mut history = Self::new(base, i32::MIN)?;
        let mut bytes = [0; WIDTH as usize];
        for _ in 0..length / WIDTH {
            source.read_exact(&mut bytes).map_err(io)?;
            history.batch[0] = decode(bytes)?;
            history.append_batch(1, None)?;
        }
        if history.last_sequence != terminal.1 || history.terminal_clock > terminal.0 {
            return Err(RuntimeError::new("input terminal position mismatch"));
        }
        history.terminal_clock = terminal.0;
        Ok(history)
    }
}
fn encode(event: ReplayInput) -> [u8; WIDTH as usize] {
    let mut bytes = [0; WIDTH as usize];
    bytes[0..8].copy_from_slice(&event.sequence.to_le_bytes());
    bytes[8..16].copy_from_slice(&event.device.to_le_bytes());
    bytes[16..24].copy_from_slice(&event.finger.to_le_bytes());
    for (chunk, value) in
        bytes[24..]
            .chunks_exact_mut(4)
            .zip([event.clock, event.x, event.y, event.button, event.kind as i32])
    {
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    bytes
}
fn decode(bytes: [u8; WIDTH as usize]) -> RuntimeResult<ReplayInput> {
    let word = |offset| i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let event = ReplayInput {
        sequence: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
        device: i64::from_le_bytes(bytes[8..16].try_into().unwrap()),
        finger: i64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        clock: word(24),
        x: word(28),
        y: word(32),
        button: word(36),
        kind: word(40) as u32,
    };
    if event.kind > 8
        || !(-1048576..=1048576).contains(&event.x)
        || !(-1048576..=1048576).contains(&event.y)
        || (matches!(event.kind, 1 | 2) && ![-2, -1, 1, 2, 3].contains(&event.button))
    {
        return Err(RuntimeError::new("invalid pointer event"));
    }
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asynchronous_chunks_and_resident_tail_branch_without_old_future() {
        let writer = super::super::writer::Writer::new().unwrap();
        let mut history = InputHistory::new(0, 20).unwrap();
        for (index, event) in history.batch.iter_mut().enumerate() {
            *event = ReplayInput {
                sequence: index as u64 + 1,
                clock: 20,
                kind: 3,
                ..Default::default()
            };
        }
        history.append_batch(4096, Some(&writer)).unwrap();
        writer.flush().unwrap();
        assert_eq!(history.read(4095).unwrap().sequence, 4096);
        history.branch(2048, 20).unwrap();
        history.batch[0] = ReplayInput {
            sequence: 2049,
            clock: 20,
            kind: 6,
            ..Default::default()
        };
        history.append_batch(1, Some(&writer)).unwrap();
        writer.flush().unwrap();
        assert_eq!(history.read(2048).unwrap().kind, 6);
        assert!(history.read(2049).is_err());
        let path = std::env::temp_dir().join(format!("rsvz-input-branch-test-{}", std::process::id()));
        history.export(&path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 2049 * WIDTH);
        let imported = InputHistory::import(&path, 0, (20, 2049)).unwrap();
        assert_eq!(imported.read(2048).unwrap().kind, 6);
        assert!(imported.read(2049).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn sequence_gaps_and_clock_reversal_are_rejected() {
        let mut history = InputHistory::new(4, 20).unwrap();
        history.batch[0] = ReplayInput {
            sequence: 6,
            clock: 20,
            kind: 3,
            ..Default::default()
        };
        assert!(history.append_batch(1, None).is_err());
        history.batch[0] = ReplayInput {
            sequence: 5,
            clock: 19,
            kind: 3,
            ..Default::default()
        };
        assert!(history.append_batch(1, None).is_err());
        assert_eq!(history.count, 0);
    }
    #[test]
    fn preserves_same_clock_tail_and_branch_order() {
        let mut history = InputHistory::new(10, 20).unwrap();
        for (index, kind) in [3, 4, 5].into_iter().enumerate() {
            history.batch[index] = ReplayInput {
                sequence: 11 + index as u64,
                clock: 20,
                kind,
                ..Default::default()
            };
        }
        history.append_batch(3, None).unwrap();
        history.seek_sequence(11).unwrap();
        assert_eq!(history.playback_offset, 1);
        assert_eq!(history.read(2).unwrap().kind, 5);
        history.branch(11, 20).unwrap();
        history.batch[0] = ReplayInput {
            sequence: 12,
            clock: 20,
            kind: 6,
            ..Default::default()
        };
        history.append_batch(1, None).unwrap();
        assert_eq!(history.count, 2);
        assert_eq!(history.read(1).unwrap().kind, 6);
        assert!(decode([255; WIDTH as usize]).is_err());
    }
}
