//! Named native snapshots. No replay controller, input playback or recording history.
use crate::{
    replay::{self, Record},
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::{
    BoardStateBackend, ClockBackend, DancerClockWriteBackend, GameUiBackend, MaidCheatsBackend, MouseStateBackend,
    NativeCheckpointBackend, WorldIdentityBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{MaidCheat, ReplayCursor, ReplayRestoreKind, WorldId};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    io::Write,
    path::Path,
    rc::Rc,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub version: u32,
    pub backend: String,
    pub mode: i32,
    pub clock: i32,
    dancer: u32,
    maid: u8,
}
fn error(e: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::new(e.to_string())
}
impl Metadata {
    pub fn read(directory: &Path) -> RuntimeResult<Self> {
        let path = directory.join("save.json");
        if std::fs::metadata(&path).map_err(error)?.len() > 16384 {
            return Err(error("存档信息过大"));
        }
        let value: Self = serde_json::from_slice(&std::fs::read(path).map_err(error)?).map_err(error)?;
        if value.version != 1 || value.backend.is_empty() || value.maid > 3 {
            return Err(error("存档格式不兼容"));
        }
        Ok(value)
    }
}
/// Writes into a new private directory. Publication and naming belong to the caller.
pub fn save(directory: &Path) -> RuntimeResult<Metadata>
where
    CurrentBackend: NativeCheckpointBackend + ClockBackend + BoardStateBackend + MaidCheatsBackend + MouseStateBackend,
{
    let record = replay::capture()?;
    record.checkpoint.export(&directory.join("world.dat")).map_err(error)?;
    let metadata = Metadata {
        version: 1,
        backend: record.checkpoint.identity().into(),
        mode: record.checkpoint.mode(),
        clock: record.clock,
        dancer: record.dancer,
        maid: match record.maid {
            MaidCheat::Stop => 0,
            MaidCheat::CallPartner => 1,
            MaidCheat::Dancing => 2,
            MaidCheat::Move => 3,
        },
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("save.json"))
        .map_err(error)?;
    file.write_all(&serde_json::to_vec(&metadata).map_err(error)?)
        .and_then(|()| file.sync_all())
        .map_err(error)?;
    Ok(metadata)
}
pub struct PreparedSave(Record);
pub fn prepare(directory: &Path) -> RuntimeResult<PreparedSave>
where
    CurrentBackend: NativeCheckpointBackend,
{
    let info = Metadata::read(directory)?;
    let checkpoint = crate::access::with_backend(|backend| {
        backend.import_checkpoint(&directory.join("world.dat"), &info.backend, info.mode)
    })
    .map_err(error)?;
    if checkpoint.clock() != info.clock {
        return Err(error("存档时钟不匹配"));
    }
    Ok(PreparedSave(Record {
        checkpoint,
        clock: info.clock,
        dancer: info.dancer,
        maid: match info.maid {
            0 => MaidCheat::Stop,
            1 => MaidCheat::CallPartner,
            2 => MaidCheat::Dancing,
            _ => MaidCheat::Move,
        },
        cursor: ReplayCursor {
            x: 0,
            y: 0,
            kind: 0,
            buttons: 0,
        },
        file: String::new(),
        input_sequence: 0,
    }))
}
/// Dropping the ticket before the boundary cancels the queued load.
pub struct LoadTicket(Rc<RefCell<Option<RuntimeResult<()>>>>);
impl LoadTicket {
    pub fn take_result(&self) -> Option<RuntimeResult<()>> {
        self.0.borrow_mut().take()
    }
}
struct Pending {
    release_boundary: bool,
    saved: PreparedSave,
    world: WorldId,
    result: Rc<RefCell<Option<RuntimeResult<()>>>>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        if self.release_boundary {
            crate::modules::release_native_boundary();
        }
    }
}
std::thread_local! {
    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
    static POLL: Cell<Option<fn() -> RuntimeResult<bool>>> = const { Cell::new(None) };
    static CLEANUP: Cell<bool> = const { Cell::new(false) };
}
impl PreparedSave {
    pub fn queue(self) -> RuntimeResult<LoadTicket>
    where
        CurrentBackend: NativeCheckpointBackend
            + WorldIdentityBackend
            + GameUiBackend
            + ClockBackend
            + BoardStateBackend
            + MaidCheatsBackend
            + MouseStateBackend
            + DancerClockWriteBackend,
    {
        if PENDING.with_borrow(Option::is_some)
            || replay::pending()
            || replay::status().mode != rsvz_model::ReplayMode::Idle
        {
            return Err(error("请先停止录制或回放，再读取存档"));
        }
        let world = crate::board::world_id()?.ok_or_else(|| error("读取存档需要准备完成的战场"))?;
        // Reuse the host's exclusive boundary slot, without installing a replay
        // controller or touching its retained recording history.
        crate::modules::claim_native_boundary()?;
        let result = Rc::new(RefCell::new(None));
        PENDING.with_borrow_mut(|pending| {
            *pending = Some(Pending {
                release_boundary: !replay::controlled_by_current(),
                saved: self,
                world,
                result: result.clone(),
            })
        });
        POLL.set(Some(poll));
        if !CLEANUP.replace(true) {
            crate::state_hook::register_native_cleanup(i32::MAX, || {
                if let Some(pending) = PENDING.with_borrow_mut(Option::take) {
                    *pending.result.borrow_mut() = Some(Err(error("读取存档已取消")));
                }
                POLL.set(None);
                CLEANUP.set(false);
                Ok(())
            });
        }
        Ok(LoadTicket(result))
    }
}
fn poll() -> RuntimeResult<bool>
where
    CurrentBackend: NativeCheckpointBackend
        + WorldIdentityBackend
        + GameUiBackend
        + ClockBackend
        + BoardStateBackend
        + MaidCheatsBackend
        + MouseStateBackend
        + DancerClockWriteBackend,
{
    let Some(pending) = PENDING.with_borrow_mut(Option::take) else {
        return Ok(false);
    };
    if Rc::strong_count(&pending.result) == 1 {
        return Ok(false);
    }
    let mut started = false;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if crate::board::world_id()? != Some(pending.world) {
            return Err(error("读取前战场已变化"));
        }
        let backup = replay::capture()?;
        started = true;
        if let Err(failure) = replay::restore(&pending.saved.0, ReplayRestoreKind::Seek) {
            return match replay::restore(&backup, ReplayRestoreKind::Recover) {
                Ok(_) => Err(error(format!("读取失败，已恢复原战场：{failure}"))),
                Err(recovery) => Err(error(format!("读取失败：{failure}；恢复原战场失败：{recovery}"))),
            };
        }
        Ok(())
    }))
    .unwrap_or_else(|_| Err(error("存档恢复发生异常；未继续推进游戏")));
    *pending.result.borrow_mut() = Some(result);
    Ok(started)
}
#[doc(hidden)]
pub fn poll_at_boundary(backend: &mut CurrentBackend) -> RuntimeResult<bool> {
    match POLL.get() {
        Some(poll) => CurrentBackend::scope_backend(backend, poll),
        None => Ok(false),
    }
}
