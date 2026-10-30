//! Native checkpoint replay. Rust script variables and timelines are not rolled back.
//! World-changing requests are drained only after ordinary callbacks have returned.
mod import;
mod input;
mod manifest;
mod writer;
use crate::runtime::{RuntimeError, RuntimeResult};
pub use import::{ImportJob, PreparedReplay, install_import, prepare_import};
use rsvz_backend_api::{
    AdvancedPauseBackend, BoardStateBackend, ClockBackend, DancerClockWriteBackend, DropRuleEditBackend,
    FastForwardBackend, GameUiBackend, InputBackend, MaidCheatsBackend, MouseStateBackend, NativeCheckpoint,
    NativeCheckpointBackend, RefreshCountdownWriteBackend, WorldIdentityBackend, ZombieRuleEditBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{AdvancedPauseOptions, GameUi, MaidCheat, WorldId};
pub use rsvz_model::{ReplayCursor, ReplayMode, ReplayRestoreKind, ReplayStatus, RestoreReceipt};
use rsvz_schedule::module::{ModuleSlot, current_module, with_module};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct RecordOptions {
    pub interval_cs: u32,
    pub directory: PathBuf,
}

pub(crate) struct Record {
    pub(crate) input_sequence: u64,
    pub(crate) checkpoint: NativeCheckpoint,
    pub(crate) clock: i32,
    pub(crate) dancer: u32,
    pub(crate) maid: MaidCheat,
    pub(crate) cursor: ReplayCursor,
    pub(crate) file: String,
}
#[derive(Clone, Copy)]
struct TickInfo {
    input_sequence: u64,
    clock: i32,
    dancer: u32,
    maid: MaidCheat,
    cursor: ReplayCursor,
}
struct Recovery {
    record: Record,
    pause: bool,
    pause_options: AdvancedPauseOptions,
    sun: bool,
    spawn: bool,
}
#[derive(Clone, Copy)]
enum Request {
    Seek(u64, Option<WorldId>),
    Play(WorldId),
    Stop,
}
struct Controller {
    writer: Option<writer::Writer>,
    input: Option<input::InputHistory>,
    owner: ModuleSlot,
    execution: Option<crate::modules::ExecutionId>,
    status: ReplayStatus,
    records: BTreeMap<u64, Record>,
    directory: PathBuf,
    pending: Option<Request>,
    recovery: Option<Recovery>,
    last_clock: Option<i32>,
    playback_ticks: u32,
    interpolate: bool,
    mouse_visible: bool,
    error: Option<String>,
    ticks: BTreeMap<i32, TickInfo>,
    cursor: Option<ReplayCursor>,
    last_buttons: Option<i32>,
    file_sequence: u64,
    start_block: Option<crate::modules::StartBlock>,
    natural_round: u64,
    branch_pending_capture: bool,
    read_only: bool,
}
type Poll = fn() -> RuntimeResult<bool>;
type ReadWorld = fn() -> RuntimeResult<Option<WorldId>>;
thread_local! {
    static REPLAY: RefCell<Option<Controller>> = const { RefCell::new(None) };
    static POLL: Cell<Option<Poll>> = const { Cell::new(None) };
    static READ_WORLD: Cell<Option<ReadWorld>> = const { Cell::new(None) };
    static SKIP: Cell<bool> = const { Cell::new(false) };
}
std::thread_local! {
    static CLEANUP_REGISTERED: Cell<bool> = const { Cell::new(false) };
    static OBSERVED_RESTORE: Cell<u64> = const { Cell::new(0) };
}

/// True once per module for the actual current restored world. An intervening
/// natural replacement cannot consume a stale Replay receipt as its own cause.
pub(crate) fn observe_restore() -> RuntimeResult<bool> {
    let Some(receipt) = last_restore() else {
        return Ok(false);
    };
    if OBSERVED_RESTORE.get() == receipt.sequence {
        return Ok(false);
    }
    OBSERVED_RESTORE.set(receipt.sequence);
    let world = READ_WORLD.get().map(|read| read()).transpose()?.flatten();
    Ok(world == Some(receipt.after))
}

pub fn status() -> ReplayStatus {
    REPLAY.with_borrow(|state| {
        state.as_ref().map_or(ReplayStatus::default(), |state| {
            let mut status = state.status;
            if status.mode == ReplayMode::Playing
                && status.paused
                && !state.interpolate
                && let (Some(input), Some(position)) = (&state.input, status.position)
                && position + 1 == status.end_index
            {
                let record = &state.records[&position];
                status.tail_pending =
                    record.input_sequence < input.last_sequence || record.clock < input.terminal_clock;
            }
            status
        })
    })
}
pub fn last_restore() -> Option<RestoreReceipt> {
    crate::modules::last_restore()
}
pub fn last_error() -> Option<String> {
    REPLAY.with_borrow(|state| state.as_ref().and_then(|state| state.error.clone()))
}
pub fn playback_owned() -> bool {
    status().playback_owned()
}
pub fn controlled_by_current() -> bool {
    REPLAY.with_borrow(|state| state.as_ref().is_some_and(|state| state.owner == current_module()))
}
pub fn pending() -> bool {
    REPLAY.with_borrow(|state| state.as_ref().is_some_and(|state| state.pending.is_some()))
}
/// Product code can transfer its explicitly acquired execution block to Replay.
/// A failed recovery retains it even if that product's service task is dropped.
#[doc(hidden)]
pub fn retain_start_block(block: crate::modules::StartBlock) -> RuntimeResult<()> {
    own(|state| {
        if state.start_block.is_some() {
            return Err(RuntimeError::new("Replay already holds an execution start block"));
        }
        state.start_block = Some(block);
        Ok(())
    })
}
pub fn cursor() -> Option<ReplayCursor> {
    REPLAY.with_borrow(|state| {
        state
            .as_ref()
            .filter(|state| state.mouse_visible)
            .and_then(|state| state.cursor)
    })
}

fn own<T>(operation: impl FnOnce(&mut Controller) -> RuntimeResult<T>) -> RuntimeResult<T> {
    REPLAY.with_borrow_mut(|state| {
        let state = state.as_mut().ok_or_else(|| RuntimeError::new("必须录制才能回档"))?;
        if state.owner != current_module() {
            return Err(RuntimeError::new("another source owns Replay"));
        }
        if state.execution.is_some() && crate::modules::current_execution(state.owner) != state.execution {
            return Err(RuntimeError::new("Replay request belongs to an earlier execution"));
        }
        operation(state)
    })
}

pub fn seek(index: u64) -> RuntimeResult<()> {
    own(|state| {
        if !matches!(state.status.mode, ReplayMode::Recording | ReplayMode::Playing) {
            return Err(RuntimeError::new("start recording or playback before seeking"));
        }
        if index >= state.status.end_index {
            return Err(RuntimeError::new("Replay record is outside the available history"));
        }
        if matches!(state.status.mode, ReplayMode::Recovering | ReplayMode::Fault) {
            return Err(RuntimeError::new("Replay recovery must finish first"));
        }
        let expected = if state.status.mode == ReplayMode::Playing && state.input.is_some() {
            // Ordered playback can seek back after its terminal event removed the Board.
            READ_WORLD
                .get()
                .ok_or_else(|| RuntimeError::new("Replay is not installed"))?()?
        } else {
            Some(request_world()?)
        };
        if state.status.mode == ReplayMode::Playing && expected != state.status.world {
            return Err(RuntimeError::new("Replay world changed outside the controller"));
        }
        state.pending = Some(Request::Seek(index, expected));
        state.status.paused = state.status.mode == ReplayMode::Playing;
        Ok(())
    })
}
pub fn previous(count: u64) -> RuntimeResult<()> {
    if count == 0 {
        return Err(RuntimeError::new("rewind count must be positive"));
    }
    let state = status();
    let origin = if state.mode == ReplayMode::Playing {
        state.position.unwrap_or(0)
    } else {
        state.end_index
    };
    let index = origin
        .checked_sub(count)
        .ok_or_else(|| RuntimeError::new("not enough Replay history"))?;
    seek(index)
}
pub fn first() -> RuntimeResult<()> {
    seek(status().start_index)
}
pub fn last() -> RuntimeResult<()> {
    seek(
        status()
            .end_index
            .checked_sub(1)
            .ok_or_else(|| RuntimeError::new("必须录制才能回档"))?,
    )
}
pub fn next() -> RuntimeResult<()> {
    let index = status()
        .position
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| RuntimeError::new("Replay index exhausted"))?;
    seek(index)
}
pub fn pause(paused: bool) -> RuntimeResult<()> {
    own(|state| {
        if !matches!(state.status.mode, ReplayMode::Recording | ReplayMode::Playing) {
            return Err(RuntimeError::new("Replay is not in a pausable state"));
        }
        if !paused
            && state.status.mode == ReplayMode::Playing
            && state
                .status
                .position
                .is_some_and(|position| position + 1 >= state.status.end_index)
            && !state.input.as_ref().is_some_and(|input| {
                state.interpolate
                    && (input.playback_offset < input.count
                        || state
                            .status
                            .current_clock
                            .is_some_and(|clock| clock < input.terminal_clock))
            })
        {
            return Err(RuntimeError::new("Replay is at its last record; seek before resuming"));
        }
        state.status.paused = paused;
        Ok(())
    })
}
pub fn set_interpolation(enabled: bool) -> RuntimeResult<()> {
    own(|state| {
        state.interpolate = enabled;
        Ok(())
    })
}
pub fn set_mouse_visible(enabled: bool) -> RuntimeResult<()> {
    own(|state| {
        state.mouse_visible = enabled;
        Ok(())
    })
}
pub fn stop() -> RuntimeResult<()> {
    if status().mode == ReplayMode::Idle {
        return Ok(());
    }
    own(|state| {
        state.pending = Some(Request::Stop);
        Ok(())
    })
}

/// Plays this controller's recorded immutable checkpoints. A filesystem import
/// uses prepare_import/install_import; a path or checksum alone never suffices.
pub fn play_recorded() -> RuntimeResult<()> {
    own(|state| {
        if state.records.is_empty() {
            return Err(RuntimeError::new("必须录制才能回档"));
        }
        if state.status.playback_owned() {
            return Err(RuntimeError::new("Replay already owns playback"));
        }
        state.pending = Some(Request::Play(request_world()?));
        Ok(())
    })
}

pub fn start_record(options: RecordOptions) -> RuntimeResult<()>
where
    CurrentBackend: NativeCheckpointBackend
        + WorldIdentityBackend
        + ClockBackend
        + GameUiBackend
        + BoardStateBackend
        + DancerClockWriteBackend
        + MaidCheatsBackend
        + AdvancedPauseBackend
        + DropRuleEditBackend
        + ZombieRuleEditBackend
        + MouseStateBackend
        + InputBackend
        + RefreshCountdownWriteBackend
        + FastForwardBackend,
{
    if matches!(
        crate::lifecycle::with_lifecycle(|state| state.session()),
        crate::lifecycle::SessionState::Finalizing | crate::lifecycle::SessionState::Done
    ) {
        return Err(RuntimeError::new("cannot start Replay during module cleanup"));
    }
    if options.interval_cs == 0 || options.interval_cs > i32::MAX as u32 {
        return Err(RuntimeError::new("Replay interval must be positive"));
    }
    if status().mode != ReplayMode::Idle {
        return Err(RuntimeError::new("stop Replay before starting a recording"));
    }
    if REPLAY.with_borrow(|state| {
        state.as_ref().is_some_and(|state| {
            state.pending.is_some()
                || state.start_block.is_some()
                || (state.owner != current_module() && !state.records.is_empty())
        })
    }) {
        return Err(RuntimeError::new(
            "Replay history or a pending operation belongs to another active owner",
        ));
    }
    let world = ready_world()?;
    // A new recording gets its own directory. No existing recording is overwritten.
    std::fs::create_dir_all(&options.directory).map_err(io)?;
    let clock = crate::access::with_backend(|backend| backend.clock()).map_err(crate::access::operation_error)?;
    let directory = std::fs::canonicalize(&options.directory).map_err(io)?;
    let mut controller = Controller::new(world, directory, options.interval_cs, Some(clock));
    controller.writer = Some(writer::Writer::new().map_err(io)?);
    let ordered = crate::access::with_backend(|backend| backend.ordered_replay_input());
    if ordered {
        let sequence = crate::access::with_backend(|backend| backend.replay_input_sequence())
            .map_err(crate::access::operation_error)?;
        controller.input = Some(input::InputHistory::new(sequence, clock)?);
    }
    crate::modules::claim_native_boundary()?;
    if let Err(error) = crate::access::with_backend(|backend| backend.begin_replay_input(false)) {
        crate::modules::release_native_boundary();
        return Err(crate::access::operation_error(error));
    }
    if ordered {
        // Ordered input needs an initial watermark; sampled replay keeps its
        // fixed, absolute-clock checkpoint cadence (including the first one).
        if let Err(error) = capture().and_then(|record| append(&mut controller, record)) {
            let _ = crate::access::with_backend(|backend| backend.end_replay_input());
            crate::modules::release_native_boundary();
            return Err(error);
        }
        controller.last_clock = Some(clock);
    }
    install_controller(controller);
    Ok(())
}

fn io(error: std::io::Error) -> RuntimeError {
    RuntimeError::new(error.to_string())
}
fn read_world() -> RuntimeResult<Option<WorldId>>
where
    CurrentBackend: WorldIdentityBackend,
{
    crate::access::with_backend(|backend| backend.world_identity()).map_err(crate::access::operation_error)
}
fn request_world() -> RuntimeResult<WorldId> {
    READ_WORLD
        .get()
        .ok_or_else(|| RuntimeError::new("Replay is not installed"))?()?
    .ok_or_else(|| RuntimeError::new("Replay has no current world"))
}
fn append(state: &mut Controller, mut record: Record) -> RuntimeResult<()> {
    let serial = state.file_sequence;
    state.file_sequence = serial
        .checked_add(1)
        .ok_or_else(|| RuntimeError::new("Replay file identity exhausted"))?;
    record.file = format!("{serial:012}.dat");
    let path = state.directory.join(&record.file);
    if let Some(buffer) = record.checkpoint.buffered() {
        state
            .writer
            .as_ref()
            .ok_or_else(|| RuntimeError::new("recording writer missing"))?
            .append(buffer, path)
            .map_err(io)?;
    } else {
        record.checkpoint.export(&path).map_err(io)?;
    }
    state.records.insert(state.records.len() as u64, record);
    state.status.end_index = state.records.len() as u64;
    state.status.position = state.status.end_index.checked_sub(1);
    Ok(())
}

fn ready_world() -> RuntimeResult<WorldId>
where
    CurrentBackend: WorldIdentityBackend + GameUiBackend,
{
    crate::access::with_backend(|backend| {
        if backend.game_ui().map_err(crate::access::operation_error)? != GameUi::Playing {
            return Err(RuntimeError::new("Replay requires a playing world"));
        }
        backend
            .world_identity()
            .map_err(crate::access::operation_error)?
            .ok_or_else(|| RuntimeError::new("Replay has no world"))
    })
}
pub(crate) fn capture() -> RuntimeResult<Record>
where
    CurrentBackend: NativeCheckpointBackend + ClockBackend + BoardStateBackend + MaidCheatsBackend + MouseStateBackend,
{
    crate::access::with_backend(|backend| {
        Ok(Record {
            input_sequence: backend
                .replay_input_sequence()
                .map_err(crate::access::operation_error)?,
            checkpoint: backend.capture_checkpoint().map_err(crate::access::operation_error)?,
            clock: backend.clock().map_err(crate::access::operation_error)?,
            dancer: backend.dancer_clock().map_err(crate::access::operation_error)?,
            maid: backend.maid_cheat().map_err(crate::access::operation_error)?,
            cursor: ReplayCursor {
                x: backend.mouse_x().map_err(crate::access::operation_error)?,
                y: backend.mouse_y().map_err(crate::access::operation_error)?,
                kind: backend.cursor_image().map_err(crate::access::operation_error)?,
                buttons: backend.mouse_buttons().map_err(crate::access::operation_error)?,
            },
            file: String::new(),
        })
    })
}

pub(crate) fn restore(record: &Record, kind: ReplayRestoreKind) -> RuntimeResult<WorldId>
where
    CurrentBackend:
        NativeCheckpointBackend + WorldIdentityBackend + GameUiBackend + DancerClockWriteBackend + MaidCheatsBackend,
{
    // Named saves can restore without ever installing a Replay controller.
    // Their receipt must still be recognized as a restore, not a new Opening.
    READ_WORLD.set(Some(read_world));
    let before = read_world()?
        .or_else(|| last_restore().map(|receipt| receipt.after))
        .ok_or_else(|| RuntimeError::new("Replay has no previous world identity"))?;
    let sequence = last_restore()
        .map_or(0, |receipt| receipt.sequence)
        .checked_add(1)
        .ok_or_else(|| RuntimeError::new("Replay restore identity exhausted"))?;
    // A failed native load can already have replaced Board. Never advance that
    // physical update, including when this was the owner's final cleanup.
    SKIP.set(true);
    crate::access::with_exclusive_backend(|backend| backend.restore_checkpoint(&record.checkpoint))
        .map_err(crate::access::operation_error)?;
    let after = ready_world()?;
    if before == after {
        return Err(RuntimeError::new(
            "native restore did not invalidate the previous world",
        ));
    }
    crate::access::with_backend(|backend| {
        backend
            .set_dancer_clock(record.dancer)
            .map_err(crate::access::operation_error)?;
        backend
            .set_maid_cheat(record.maid)
            .map_err(crate::access::operation_error)
    })?;
    crate::modules::publish_restore_receipt(RestoreReceipt {
        sequence,
        before,
        after,
        kind,
    });
    crate::game_pause::replay_world_restored(before, after);
    Ok(after)
}

fn poll() -> RuntimeResult<bool>
where
    CurrentBackend: NativeCheckpointBackend
        + WorldIdentityBackend
        + ClockBackend
        + GameUiBackend
        + BoardStateBackend
        + DancerClockWriteBackend
        + MaidCheatsBackend
        + AdvancedPauseBackend
        + DropRuleEditBackend
        + ZombieRuleEditBackend
        + MouseStateBackend
        + InputBackend
        + RefreshCountdownWriteBackend
        + FastForwardBackend,
{
    // Taking ownership releases TLS borrow before native calls and lifecycle facts.
    let Some(mut state) = REPLAY.with_borrow_mut(Option::take) else {
        return Ok(false);
    };
    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_module(state.owner, || advance(&mut state))
    })) {
        Ok(result) => result,
        Err(payload) => Err(rsvz_schedule::callback::into_error(payload)
            .unwrap_or_else(|_| RuntimeError::new("Replay native operation panicked; recovery retained"))),
    };
    if let Err(error) = &result {
        state.error = Some(error.to_string());
        // A failed drain can precede taking the request. Do not retry that
        // intent next boundary and let a successful Stop erase the failure.
        state.pending = None;
        state.status.paused = true;
        state.status.mode = if state.recovery.is_some() {
            ReplayMode::Fault
        } else {
            ReplayMode::Idle
        };
        if state.recovery.is_none() {
            let _ = crate::access::with_backend(|backend| backend.end_replay_input());
            state.start_block = None;
        }
    }
    let skip = state.status.playback_owned() && (!state.interpolate || state.status.paused);
    REPLAY.with_borrow_mut(|slot| *slot = Some(state));
    result.map(|changed| changed || skip)
}

fn advance(state: &mut Controller) -> RuntimeResult<bool>
where
    CurrentBackend: NativeCheckpointBackend
        + WorldIdentityBackend
        + ClockBackend
        + GameUiBackend
        + BoardStateBackend
        + DancerClockWriteBackend
        + MaidCheatsBackend
        + AdvancedPauseBackend
        + DropRuleEditBackend
        + ZombieRuleEditBackend
        + MouseStateBackend
        + InputBackend
        + RefreshCountdownWriteBackend
        + FastForwardBackend,
{
    if state.status.mode == ReplayMode::Recording {
        if !matches!(state.pending, Some(Request::Stop)) {
            if let Some(writer) = &state.writer {
                writer.check().map_err(io)?;
            }
        }
        if let Some(input) = &mut state.input {
            let count = crate::access::with_backend(|backend| backend.drain_replay_input(&mut input.batch))
                .map_err(crate::access::operation_error)?;
            input.append_batch(count, state.writer.as_ref())?;
            for event in &input.batch[..count] {
                if let Some(tick) = state.ticks.get_mut(&event.clock) {
                    tick.input_sequence = event.sequence;
                }
            }
            if let Some(clock) = crate::access::with_backend(|backend| backend.replay_input_terminal_clock())
                .map_err(crate::access::operation_error)?
            {
                input.terminal_clock = clock;
            }
        }
    }
    if let Some(request) = state.pending.take() {
        // Reject failed persistence before playback takeover; branches must also
        // follow queued input writes before truncating their private history.
        if matches!(request, Request::Play(_))
            || (state.status.mode == ReplayMode::Recording
                && state.input.is_some()
                && matches!(request, Request::Seek(..)))
        {
            if let Some(writer) = &state.writer {
                writer.flush().map_err(io)?;
            }
        }
        let unchanged = match &request {
            Request::Play(expected) => ready_world()? == *expected,
            Request::Seek(_, expected) if state.status.mode == ReplayMode::Playing && state.input.is_some() => {
                read_world()? == *expected
            }
            Request::Seek(_, expected) => Some(ready_world()?) == *expected,
            _ => true,
        };
        if !unchanged {
            return Err(RuntimeError::new("world changed before Replay safe point"));
        }
        match request {
            Request::Play(_) => {
                crate::access::with_backend(|backend| backend.begin_replay_input(true))
                    .map_err(crate::access::operation_error)?;
                seal_recording(state)?;
                crate::single_step::cancel_for_declaration()?;
                crate::access::with_backend(|backend| {
                    if backend.fast_forward_active() {
                        backend
                            .stop_fast_forward(rsvz_model::FastForwardStopReason::LeftFight)
                            .map_err(crate::access::operation_error)?;
                    }
                    Ok::<_, RuntimeError>(())
                })?;
                // Capture recovery before changing any world or native controls.
                state.recovery = Some(capture_recovery()?);
                state.status.mode = ReplayMode::Playing;
                state.status.paused = false;
                state.status.world = Some(restore(&state.records[&0], ReplayRestoreKind::Seek)?);
                state.status.position = Some(0);
                state.status.current_clock = Some(state.records[&0].clock);
                state.status.paused = state.input.is_none() && state.status.end_index == 1;
                if let Some(input) = &mut state.input {
                    input.seek_sequence(state.records[&0].input_sequence)?;
                }
                state.cursor = Some(state.records[&0].cursor);
                state.last_buttons = None;
                state.playback_ticks = 0;
                playback_controls()?;
                state.error = None;
                return Ok(true);
            }
            Request::Seek(index, _) => {
                let recording = state.status.mode == ReplayMode::Recording;
                crate::access::with_backend(|backend| backend.begin_replay_input(true))
                    .map_err(crate::access::operation_error)?;
                if recording {
                    state.recovery = Some(capture_recovery()?);
                }
                let record = state
                    .records
                    .get(&index)
                    .ok_or_else(|| RuntimeError::new("Replay record disappeared"))?;
                let restored = restore(record, ReplayRestoreKind::Seek);
                if let Err(error) = restored {
                    if recording {
                        let backup = state.recovery.as_ref().expect("recording recovery retained");
                        match restore(&backup.record, ReplayRestoreKind::Recover) {
                            Ok(world) => {
                                state.status.world = Some(world);
                                state.recovery = None;
                            }
                            Err(recovery) => {
                                return Err(RuntimeError::new(format!(
                                    "Replay restore failed: {error}; recovery failed: {recovery}"
                                )));
                            }
                        }
                    }
                    return Err(error);
                }
                state.status.world = Some(restored.expect("successful restore"));
                state.status.current_clock = Some(record.clock);
                state.status.position = Some(index);
                state.last_clock = Some(record.clock);
                if let Some(input) = &mut state.input {
                    input.seek_sequence(record.input_sequence)?;
                }
                state.cursor = Some(record.cursor);
                state.last_buttons = None;
                state.playback_ticks = 0;
                if state.status.mode == ReplayMode::Recording {
                    state.recovery = None;
                    if let Some(input) = &mut state.input {
                        input.branch(record.input_sequence, record.clock)?;
                    }
                    crate::access::with_backend(|backend| backend.begin_replay_input(false))
                        .map_err(crate::access::operation_error)?;
                    crate::access::with_backend(|backend| backend.hand_back_replay_input())
                        .map_err(crate::access::operation_error)?;
                    // A recording rewind branches here; future native records are
                    // not a rollback of user Rust variables or completed tasks.
                    state.records.retain(|saved, _| *saved < index);
                    state
                        .ticks
                        .retain(|clock, _| *clock < state.status.current_clock.unwrap_or(0));
                    state.status.end_index = index;
                    state.start_block = None;
                    state.branch_pending_capture = true;
                }
                state.error = None;
                return Ok(true);
            }
            Request::Stop => {
                let returning_to_player = state.recovery.is_some() || state.status.playback_owned();
                let seal_error = seal_recording(state).err();
                // File publication must never prevent the necessary native
                // recovery. Report it after releasing successful takeover.
                let export_error = write_manifest(state).err().or(seal_error);
                if let Some(recovery) = &state.recovery {
                    state.status.mode = ReplayMode::Recovering;
                    let world = restore(&recovery.record, ReplayRestoreKind::Recover)?;
                    crate::access::with_backend(|backend| {
                        backend
                            .set_natural_sun_drop_disabled(recovery.sun)
                            .map_err(crate::access::operation_error)?;
                        backend
                            .set_zombie_spawn_stopped(recovery.spawn)
                            .map_err(crate::access::operation_error)?;
                        backend
                            .set_advanced_pause_with_options(recovery.pause, recovery.pause_options)
                            .map_err(crate::access::operation_error)
                    })?;
                    state.status.world = Some(world);
                    state.recovery = None;
                }
                crate::access::with_backend(|backend| backend.end_replay_input())
                    .map_err(crate::access::operation_error)?;
                if returning_to_player {
                    crate::access::with_backend(|backend| backend.hand_back_replay_input())
                        .map_err(crate::access::operation_error)?;
                }
                state.status.mode = ReplayMode::Idle;
                state.error = export_error.map(|error| error.to_string());
                state.start_block = None;
                state.status.paused = false;
                state.status.current_clock = None;
                state.status.world = None;
                return Ok(SKIP.get());
            }
        }
    }
    if matches!(
        state.status.mode,
        ReplayMode::Idle | ReplayMode::Fault | ReplayMode::Recovering
    ) {
        return Ok(state.status.playback_owned());
    }
    if state.status.mode == ReplayMode::Playing
        && let Some(input) = &state.input
    {
        let left = read_world()? != state.status.world
            || crate::access::with_backend(|backend| backend.game_ui()).map_err(crate::access::operation_error)?
                != GameUi::Playing;
        if left && read_world()? == state.status.world && input.world_ended && input.playback_offset == input.count {
            let clock = crate::access::with_backend(|backend| backend.replay_input_terminal_clock())
                .map_err(crate::access::operation_error)?;
            if clock.is_some_and(|clock| clock >= input.terminal_clock) {
                state.status.paused = true;
                state.status.current_clock = Some(input.terminal_clock);
                return Ok(true);
            }
        }
    }
    let world = if state.status.mode == ReplayMode::Recording {
        let current = read_world()?;
        if current != state.status.world
            || state.natural_round != crate::session::completed_rounds()
            || crate::access::with_backend(|backend| backend.game_ui()).map_err(crate::access::operation_error)?
                != GameUi::Playing
        {
            if let Some(input) = &mut state.input {
                input.world_ended = true;
            }
            write_manifest(state)?;
            crate::access::with_backend(|backend| backend.end_replay_input())
                .map_err(crate::access::operation_error)?;
            state.status.mode = ReplayMode::Idle;
            state.status.world = current;
            return Ok(false);
        }
        ready_world()?
    } else {
        ready_world()?
    };
    if state.status.world != Some(world) {
        return Err(RuntimeError::new("Replay world changed outside the controller"));
    }
    if state.status.mode == ReplayMode::Playing {
        playback_controls()?;
        if state.status.paused {
            return Ok(true);
        }
        if state.input.is_some() {
            return advance_ordered(state);
        }
        if state
            .status
            .position
            .is_some_and(|position| position + 1 >= state.status.end_index)
        {
            state.status.paused = true;
            return Ok(true);
        }
        state.playback_ticks += 1;
        if state.playback_ticks >= state.status.interval_cs {
            state.playback_ticks = 0;
            let index = state.status.position.unwrap_or(0) + 1;
            if index >= state.status.end_index {
                state.status.paused = true;
                return Ok(true);
            }
            let record = &state.records[&index];
            state.status.world = Some(restore(record, ReplayRestoreKind::Seek)?);
            state.status.position = Some(index);
            state.status.paused = index + 1 == state.status.end_index;
            state.status.current_clock = Some(record.clock);
            state.cursor = Some(record.cursor);
            return Ok(true);
        }
        if state.interpolate {
            let clock =
                crate::access::with_backend(|backend| backend.clock()).map_err(crate::access::operation_error)?;
            if let Some(&tick) = state.ticks.get(&clock) {
                state.cursor = Some(tick.cursor);
                crate::access::with_backend(|backend| {
                    backend
                        .set_dancer_clock(tick.dancer)
                        .map_err(crate::access::operation_error)?;
                    backend
                        .set_maid_cheat(tick.maid)
                        .map_err(crate::access::operation_error)
                })?;
                let position = rsvz_model::PixelPos {
                    x: tick.cursor.x,
                    y: tick.cursor.y,
                };
                if (0..800).contains(&position.x) && (0..600).contains(&position.y) {
                    crate::access::with_exclusive_backend(|backend| {
                        let ordered = backend.ordered_replay_input();
                        let event = |kind, button| rsvz_backend_api::ReplayInput {
                            clock,
                            x: position.x,
                            y: position.y,
                            kind,
                            button,
                            ..Default::default()
                        };
                        if ordered {
                            backend
                                .apply_replay_input(event(0, 0))
                                .map_err(crate::access::operation_error)?;
                        } else {
                            backend.mouse_move(position).map_err(crate::access::operation_error)?;
                        }
                        for (mask, button) in [(1, rsvz_model::MouseButton::Left), (2, rsvz_model::MouseButton::Right)]
                        {
                            if let Some(previous) = state.last_buttons {
                                if previous & mask == 0 && tick.cursor.buttons & mask != 0 {
                                    if ordered {
                                        backend
                                            .apply_replay_input(event(1, if mask == 1 { 1 } else { -1 }))
                                            .map_err(crate::access::operation_error)?;
                                    } else {
                                        backend
                                            .mouse_down(position, button)
                                            .map_err(crate::access::operation_error)?;
                                    }
                                }
                                if previous & mask != 0 && tick.cursor.buttons & mask == 0 {
                                    if ordered {
                                        backend
                                            .apply_replay_input(event(2, if mask == 1 { 1 } else { -1 }))
                                            .map_err(crate::access::operation_error)?;
                                    } else {
                                        backend
                                            .mouse_up(position, button)
                                            .map_err(crate::access::operation_error)?;
                                    }
                                }
                            }
                        }
                        Ok::<_, RuntimeError>(())
                    })?;
                }
                state.last_buttons = Some(tick.cursor.buttons);
            }
            crate::access::with_backend(|backend| backend.decrement_refresh_countdown())
                .map_err(crate::access::operation_error)?;
        }
        return Ok(!state.interpolate);
    }
    let clock = crate::access::with_backend(|backend| backend.clock()).map_err(crate::access::operation_error)?;
    state.status.current_clock = Some(clock);
    let can_capture =
        crate::access::with_backend(|backend| backend.checkpoint_ready()).map_err(crate::access::operation_error)?;
    if state.branch_pending_capture
        && can_capture
        && !crate::access::with_backend(|backend| backend.advanced_pause_active())
    {
        append(state, capture()?)?;
        state.branch_pending_capture = false;
        state.last_clock = Some(clock);
    }
    if !state.status.paused && (state.input.is_some() || state.last_clock != Some(clock)) {
        let tick = crate::access::with_backend(|backend| {
            Ok::<_, RuntimeError>(TickInfo {
                input_sequence: backend
                    .replay_input_sequence()
                    .map_err(crate::access::operation_error)?,
                clock,
                dancer: backend.dancer_clock().map_err(crate::access::operation_error)?,
                maid: backend.maid_cheat().map_err(crate::access::operation_error)?,
                cursor: ReplayCursor {
                    x: backend.mouse_x().map_err(crate::access::operation_error)?,
                    y: backend.mouse_y().map_err(crate::access::operation_error)?,
                    kind: backend.cursor_image().map_err(crate::access::operation_error)?,
                    buttons: backend.mouse_buttons().map_err(crate::access::operation_error)?,
                },
            })
        })?;
        state.cursor = Some(tick.cursor);
        state.ticks.insert(clock, tick);
    }
    // A rewind discards its branch checkpoint until recording can resume.
    // Do not let periodic capture bypass that wait, even when older records remain.
    let due = if state.branch_pending_capture {
        false
    } else if state.input.is_some() {
        clock.saturating_sub(state.records.last_key_value().unwrap().1.clock) >= state.status.interval_cs as i32
    } else {
        state.last_clock != Some(clock) && clock.rem_euclid(state.status.interval_cs as i32) == 0
    };
    if !state.status.paused && can_capture && due {
        let record = capture()?;
        append(state, record)?;
    }
    state.last_clock = Some(clock);
    Ok(false)
}

fn advance_ordered(state: &mut Controller) -> RuntimeResult<bool>
where
    CurrentBackend: NativeCheckpointBackend
        + ClockBackend
        + WorldIdentityBackend
        + GameUiBackend
        + DancerClockWriteBackend
        + MaidCheatsBackend
        + RefreshCountdownWriteBackend,
{
    let clock = crate::access::with_backend(|backend| backend.clock()).map_err(crate::access::operation_error)?;
    state.status.current_clock = Some(clock);
    let next = state.status.position.unwrap_or(0) + 1;
    if !state.interpolate {
        if next >= state.status.end_index {
            state.status.paused = true;
            return Ok(true);
        }
        state.playback_ticks += 1;
        let current_clock = state.records[&state.status.position.unwrap_or(0)].clock;
        if state.playback_ticks < (state.records[&next].clock - current_clock).max(1) as u32 {
            return Ok(true);
        }
        let record = &state.records[&next];
        state.status.world = Some(restore(record, ReplayRestoreKind::Seek)?);
        state.input.as_mut().unwrap().seek_sequence(record.input_sequence)?;
        state.status.position = Some(next);
        state.status.current_clock = Some(record.clock);
        state.cursor = Some(record.cursor);
        state.playback_ticks = 0;
        return Ok(true);
    }
    let limit = state
        .records
        .get(&next)
        .map(|record| record.input_sequence)
        .unwrap_or(u64::MAX);
    let input = state.input.as_mut().unwrap();
    while input.playback_offset < input.count {
        let event = input.read(input.playback_offset)?;
        if event.clock > clock || event.sequence > limit {
            break;
        }
        crate::access::with_exclusive_backend(|backend| backend.apply_replay_input(event))
            .map_err(crate::access::operation_error)?;
        input.playback_offset += 1;
        let current_world = read_world()?;
        if current_world != state.status.world {
            if !input.world_ended || input.playback_offset != input.count {
                return Err(RuntimeError::new(
                    "replay input left its world before the terminal event",
                ));
            }
            state.status.world = current_world;
            state.status.paused = true;
            return Ok(true);
        }
    }
    if let Some(record) = state.records.get(&next) {
        if clock >= record.clock {
            state.status.world = Some(restore(record, ReplayRestoreKind::Seek)?);
            state.input.as_mut().unwrap().seek_sequence(record.input_sequence)?;
            state.status.position = Some(next);
            state.status.current_clock = Some(record.clock);
            state.cursor = Some(record.cursor);
            return Ok(true);
        }
    } else if clock >= input.terminal_clock && input.playback_offset == input.count {
        state.status.paused = true;
        state.status.current_clock = Some(clock);
        return Ok(true);
    }
    if let Some(tick) = state.ticks.get(&clock) {
        state.cursor = Some(tick.cursor);
        crate::access::with_backend(|backend| {
            backend
                .set_dancer_clock(tick.dancer)
                .map_err(crate::access::operation_error)?;
            backend
                .set_maid_cheat(tick.maid)
                .map_err(crate::access::operation_error)
        })?;
    }
    crate::access::with_backend(|backend| backend.decrement_refresh_countdown())
        .map_err(crate::access::operation_error)?;
    Ok(false)
}

fn seal_recording(state: &mut Controller) -> RuntimeResult<()>
where
    CurrentBackend: NativeCheckpointBackend
        + ClockBackend
        + BoardStateBackend
        + MaidCheatsBackend
        + MouseStateBackend
        + WorldIdentityBackend
        + GameUiBackend,
{
    if state.status.mode == ReplayMode::Recording && state.input.is_some() && ready_world().ok() == state.status.world {
        // A final snapshot also retains edits made without advancing the clock.
        // If the UI cannot be captured, keep the explicit input tail instead.
        if let Ok(record) = capture() {
            append(state, record)?;
            state.input.as_mut().unwrap().terminal_snapshot = true;
        }
    }
    Ok(())
}

fn capture_recovery() -> RuntimeResult<Recovery>
where
    CurrentBackend: NativeCheckpointBackend
        + ClockBackend
        + BoardStateBackend
        + MaidCheatsBackend
        + MouseStateBackend
        + AdvancedPauseBackend
        + DropRuleEditBackend
        + ZombieRuleEditBackend,
{
    let record = capture()?;
    crate::access::with_backend(|backend| {
        Ok(Recovery {
            record,
            pause: backend.advanced_pause_active(),
            pause_options: backend.advanced_pause_options(),
            sun: backend
                .natural_sun_drop_disabled()
                .map_err(crate::access::operation_error)?,
            spawn: backend.zombie_spawn_stopped().map_err(crate::access::operation_error)?,
        })
    })
}

fn manifest_bytes(state: &Controller) -> RuntimeResult<Vec<u8>> {
    let maid = |value: MaidCheat| match value {
        MaidCheat::Stop => 0,
        MaidCheat::CallPartner => 1,
        MaidCheat::Dancing => 2,
        MaidCheat::Move => 3,
    };
    let cursor = |value: ReplayCursor| [value.x, value.y, value.kind, value.buttons];
    let records: Vec<_> = state.records.values().map(|record| serde_json::json!({"file":record.file,"mode":record.checkpoint.mode(),"clock":record.clock,"dancer":record.dancer,"maid":maid(record.maid),"cursor":cursor(record.cursor),"input_sequence":record.input_sequence})).collect();
    let ticks: Vec<_> = state
        .ticks
        .values()
        .map(|tick| (tick.clock, tick.dancer, maid(tick.maid), cursor(tick.cursor)))
        .collect();
    let input = state.input.as_ref().map(|input| {
        serde_json::json!({"file":"input.bin", "base_sequence":input.base_sequence,
        "terminal_clock":input.terminal_clock, "terminal_sequence":input.last_sequence,"world_ended":input.world_ended,"terminal_snapshot":input.terminal_snapshot,
        "ticks":state.ticks.values().map(|tick|(tick.clock,tick.input_sequence)).collect::<Vec<_>>()})
    });
    let manifest = serde_json::json!({"format":"rsvz-replay","version":2,"input":input,"backend":state.records.first_key_value().map(|(_,record)|record.checkpoint.identity()),"interval_cs":state.status.interval_cs,"records":records,"ticks":ticks});
    serde_json::to_vec(&manifest).map_err(|error| RuntimeError::new(error.to_string()))
}
fn write_manifest(state: &Controller) -> RuntimeResult<()> {
    if let Some(writer) = &state.writer {
        writer.flush().map_err(io)?;
    }
    if state.read_only {
        return Ok(());
    }
    if let Some(input) = &state.input {
        let pending = state.directory.join("input.bin.pending");
        input.export(&pending)?;
        std::fs::rename(pending, state.directory.join("input.bin")).map_err(io)?;
    }
    let pending = state.directory.join("replay.json.pending");
    std::fs::write(&pending, manifest_bytes(state)?).map_err(io)?;
    std::fs::rename(pending, state.directory.join("replay.json")).map_err(io)
}

fn playback_controls() -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend
        + DropRuleEditBackend
        + ZombieRuleEditBackend
        + MouseStateBackend
        + InputBackend
        + RefreshCountdownWriteBackend
        + FastForwardBackend,
{
    crate::access::with_backend(|backend| {
        if backend.fast_forward_active() {
            backend
                .stop_fast_forward(rsvz_model::FastForwardStopReason::LeftFight)
                .map_err(crate::access::operation_error)?;
        }
        backend
            .set_natural_sun_drop_disabled(true)
            .map_err(crate::access::operation_error)?;
        backend
            .set_zombie_spawn_stopped(true)
            .map_err(crate::access::operation_error)?;
        backend
            .set_advanced_pause_with_options(false, AdvancedPauseOptions::default())
            .map_err(crate::access::operation_error)
    })
}

fn cleanup() -> RuntimeResult<()>
where
    CurrentBackend: NativeCheckpointBackend
        + WorldIdentityBackend
        + ClockBackend
        + GameUiBackend
        + BoardStateBackend
        + DancerClockWriteBackend
        + MaidCheatsBackend
        + AdvancedPauseBackend
        + DropRuleEditBackend
        + ZombieRuleEditBackend
        + MouseStateBackend
        + InputBackend
        + RefreshCountdownWriteBackend
        + FastForwardBackend,
{
    if REPLAY.with_borrow(|state| state.as_ref().is_none_or(|state| state.owner != current_module())) {
        return Ok(());
    }
    stop()?;
    poll()?;
    if playback_owned() {
        return Err(RuntimeError::new(
            "Replay recovery incomplete; module must remain mapped",
        ));
    }
    REPLAY.with_borrow_mut(|state| *state = None);
    POLL.set(None);
    crate::modules::release_native_boundary();
    Ok(())
}

/// The shared dispatcher calls this once after both modules have returned.
/// A single-script host invokes it after that script's outer callback boundary.
#[doc(hidden)]
pub fn poll_at_boundary(backend: &mut CurrentBackend) -> RuntimeResult<bool> {
    let result = match POLL.get() {
        Some(poll) => CurrentBackend::scope_backend(backend, poll),
        None => Ok(false),
    };
    let skip = SKIP.replace(false);
    result.map(|value| value || skip)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_terminal_without_board_can_seek_but_unrelated_world_cannot() {
        let mut state = Controller::new(WorldId::from_raw(1), PathBuf::new(), 10, Some(0));
        state.input = Some(input::InputHistory::new(0, 0).unwrap());
        state.status.mode = ReplayMode::Playing;
        state.status.world = None;
        state.status.end_index = 2;
        let previous = REPLAY.replace(Some(state));
        let reader = READ_WORLD.replace(Some(|| Ok(None)));
        assert!(first().is_ok());
        REPLAY.with_borrow(|state| assert!(matches!(state.as_ref().unwrap().pending, Some(Request::Seek(0, None)))));
        READ_WORLD.set(Some(|| Ok(Some(WorldId::from_raw(2)))));
        assert!(first().is_err());
        READ_WORLD.set(Some(|| Ok(None)));
        REPLAY.with_borrow_mut(|state| state.as_mut().unwrap().status.mode = ReplayMode::Recording);
        assert!(first().is_err());
        READ_WORLD.set(reader);
        REPLAY.replace(previous);
    }
    #[test]
    fn recover_and_fault_keep_playback_ownership() {
        for mode in [ReplayMode::Playing, ReplayMode::Recovering, ReplayMode::Fault] {
            assert!(
                ReplayStatus {
                    mode,
                    ..Default::default()
                }
                .playback_owned()
            );
        }
        assert!(
            !ReplayStatus {
                mode: ReplayMode::Recording,
                ..Default::default()
            }
            .playback_owned()
        );
        assert!(previous(0).is_err());
    }
}

impl Controller {
    fn new(world: WorldId, directory: PathBuf, interval_cs: u32, clock: Option<i32>) -> Self {
        Self {
            writer: None,
            input: None,
            owner: current_module(),
            execution: crate::modules::current_execution(current_module()),
            status: ReplayStatus {
                mode: ReplayMode::Recording,
                paused: false,
                tail_pending: false,
                start_index: 0,
                end_index: 0,
                position: None,
                interval_cs,
                current_clock: clock,
                world: Some(world),
            },
            records: BTreeMap::new(),
            directory,
            pending: None,
            recovery: None,
            last_clock: None,
            playback_ticks: 0,
            interpolate: true,
            mouse_visible: true,
            error: None,
            ticks: BTreeMap::new(),
            cursor: None,
            last_buttons: None,
            file_sequence: 0,
            start_block: None,
            natural_round: crate::session::completed_rounds(),
            branch_pending_capture: false,
            read_only: false,
        }
    }
}

fn install_controller(controller: Controller)
where
    CurrentBackend: NativeCheckpointBackend
        + WorldIdentityBackend
        + ClockBackend
        + GameUiBackend
        + BoardStateBackend
        + DancerClockWriteBackend
        + MaidCheatsBackend
        + AdvancedPauseBackend
        + DropRuleEditBackend
        + ZombieRuleEditBackend
        + MouseStateBackend
        + InputBackend
        + RefreshCountdownWriteBackend
        + FastForwardBackend,
{
    REPLAY.with_borrow_mut(|state| *state = Some(controller));
    POLL.set(Some(poll));
    READ_WORLD.set(Some(read_world));
    if !CLEANUP_REGISTERED.replace(true) {
        crate::state_hook::register_native_cleanup(i32::MIN, || {
            cleanup()?;
            CLEANUP_REGISTERED.set(false);
            Ok(())
        });
    }
}
