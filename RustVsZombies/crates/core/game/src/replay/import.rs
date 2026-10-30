//! Incremental file preflight. No script stop, Board replacement or native load.
use super::*;

pub struct ImportJob {
    input: Option<input::InputHistory>,
    manifest: manifest::Manifest,
    directory: PathBuf,
    owner: ModuleSlot,
    execution: Option<crate::modules::ExecutionId>,
    records: BTreeMap<u64, Record>,
    index: usize,
}
pub struct PreparedReplay {
    input: Option<input::InputHistory>,
    directory: PathBuf,
    owner: ModuleSlot,
    execution: Option<crate::modules::ExecutionId>,
    interval_cs: u32,
    records: BTreeMap<u64, Record>,
    ticks: BTreeMap<i32, TickInfo>,
}

pub fn prepare_import(path: &Path) -> RuntimeResult<ImportJob>
where
    CurrentBackend: NativeCheckpointBackend,
{
    let manifest = manifest::Manifest::read(path)?;
    let identity =
        crate::access::with_backend(|backend| backend.checkpoint_identity()).map_err(crate::access::operation_error)?;
    if manifest.backend != identity {
        return Err(RuntimeError::new("回放后端或游戏版本不兼容；尚未修改世界"));
    }
    let path = std::fs::canonicalize(path).map_err(io)?;
    let directory = path
        .parent()
        .ok_or_else(|| RuntimeError::new("回放没有有效目录"))?
        .to_path_buf();
    Ok(ImportJob {
        input: None,
        manifest,
        directory,
        owner: current_module(),
        execution: crate::modules::current_execution(current_module()),
        records: BTreeMap::new(),
        index: 0,
    })
}
fn maid(value: u8) -> RuntimeResult<MaidCheat> {
    match value {
        0 => Ok(MaidCheat::Stop),
        1 => Ok(MaidCheat::CallPartner),
        2 => Ok(MaidCheat::Dancing),
        3 => Ok(MaidCheat::Move),
        _ => Err(RuntimeError::new("invalid recorded maid phase")),
    }
}
fn cursor(value: [i32; 4]) -> ReplayCursor {
    ReplayCursor {
        x: value[0],
        y: value[1],
        kind: value[2],
        buttons: value[3],
    }
}
fn owner_matches(owner: ModuleSlot, execution: Option<crate::modules::ExecutionId>) -> RuntimeResult<()> {
    if owner != current_module() || execution != crate::modules::current_execution(owner) {
        return Err(RuntimeError::new("回放预检属于已结束的执行来源"));
    }
    Ok(())
}
impl ImportJob {
    /// Validate at most one native checkpoint per call, keeping control dispatch alive.
    /// An error does not advance the index or silently omit a failed record.
    pub fn advance(&mut self) -> RuntimeResult<bool>
    where
        CurrentBackend: NativeCheckpointBackend,
    {
        owner_matches(self.owner, self.execution)?;
        let Some(record) = self.manifest.records.get_mut(self.index) else {
            return Ok(true);
        };
        let path = self.directory.join(&record.file);
        let checkpoint = crate::access::with_backend(|backend| {
            backend.import_checkpoint(&path, &self.manifest.backend, record.mode)
        })
        .map_err(|error| RuntimeError::new(format!("回放记录 {} 校验失败：{error}", self.index + 1)))?;
        let sequence = crate::access::with_backend(|backend| backend.checkpoint_input_sequence(&checkpoint))
            .map_err(crate::access::operation_error)?;
        if sequence != record.input_sequence {
            return Err(RuntimeError::new("checkpoint input sequence differs from manifest"));
        }
        if checkpoint.clock() != record.clock {
            return Err(RuntimeError::new("回放记录时钟与清单不一致"));
        }
        if self.index == 0
            && let Some(input) = &self.manifest.input
        {
            let mut history = input::InputHistory::import(
                &self.directory.join(&input.file),
                input.base_sequence,
                (input.terminal_clock, input.terminal_sequence),
            )?;
            history.world_ended = input.world_ended;
            history.terminal_snapshot = input.terminal_snapshot;
            self.input = Some(history);
        }
        self.records.insert(
            self.index as u64,
            Record {
                input_sequence: record.input_sequence,
                checkpoint,
                clock: record.clock,
                dancer: record.dancer,
                maid: maid(record.maid)?,
                cursor: cursor(record.cursor),
                file: std::mem::take(&mut record.file),
            },
        );
        self.index += 1;
        Ok(self.index == self.manifest.records.len())
    }
    pub fn finish(self) -> RuntimeResult<PreparedReplay> {
        owner_matches(self.owner, self.execution)?;
        if self.index != self.manifest.records.len() {
            return Err(RuntimeError::new("回放预检尚未结束"));
        }
        let input = self.input;
        if let Some(history) = &input {
            for &(clock, sequence) in &self.manifest.input.as_ref().unwrap().ticks {
                if sequence > history.base_sequence && history.read(sequence - history.base_sequence - 1)?.clock > clock
                {
                    return Err(RuntimeError::new("tick watermark includes future input"));
                }
            }
            for record in self.records.values() {
                // Contiguous sequences index the final event at this watermark directly.
                if record.input_sequence > history.base_sequence
                    && history.read(record.input_sequence - history.base_sequence - 1)?.clock > record.clock
                {
                    return Err(RuntimeError::new("checkpoint includes future input"));
                }
            }
        }
        let input_ticks = self.manifest.input.as_ref().map(|input| &input.ticks);
        let ticks = self
            .manifest
            .ticks
            .into_iter()
            .enumerate()
            .map(|(index, (clock, dancer, phase, pointer))| {
                Ok((
                    clock,
                    TickInfo {
                        input_sequence: input_ticks.map(|ticks| &ticks[index]).map_or(0, |tick| tick.1),
                        clock,
                        dancer,
                        maid: maid(phase)?,
                        cursor: cursor(pointer),
                    },
                ))
            })
            .collect::<RuntimeResult<_>>()?;
        Ok(PreparedReplay {
            input,
            directory: self.directory,
            owner: self.owner,
            execution: self.execution,
            interval_cs: self.manifest.interval_cs,
            records: self.records,
            ticks,
        })
    }
}

/// Install preflighted history after the caller has coordinated any conflicting
/// execution. This retains no policy about which other module should be stopped.
/// Finish any active recording or playback through `stop` before installing.
pub fn install_import(prepared: PreparedReplay) -> RuntimeResult<()>
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
    owner_matches(prepared.owner, prepared.execution)?;
    if matches!(
        crate::lifecycle::with_lifecycle(|state| state.session()),
        crate::lifecycle::SessionState::Finalizing | crate::lifecycle::SessionState::Done
    ) {
        return Err(RuntimeError::new("cannot import Replay during module cleanup"));
    }
    let world = ready_world()?;
    REPLAY.with_borrow(|state| -> RuntimeResult<()> {
        if let Some(state) = state {
            if state.pending.is_some()
                || state.status.mode != ReplayMode::Idle
                || state.start_block.is_some()
                || state.owner != current_module()
            {
                return Err(RuntimeError::new("请先结束当前回放控制器"));
            }
            write_manifest(state)?;
        }
        Ok(())
    })?;
    crate::modules::claim_native_boundary()?;
    let mut controller = Controller::new(world, prepared.directory, prepared.interval_cs, None);
    controller.status.mode = ReplayMode::Idle;
    controller.status.end_index = prepared.records.len() as u64;
    controller.input = prepared.input;
    controller.records = prepared.records;
    controller.ticks = prepared.ticks;
    controller.read_only = true;
    install_controller(controller);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_import_cannot_publish_history() {
        let manifest:manifest::Manifest=serde_json::from_value(serde_json::json!({"format":"rsvz-replay","version":1,"backend":"fixture","interval_cs":10,
          "records":[{"file":"000000000000.dat","mode":13,"clock":0,"dancer":0,"maid":0,"cursor":[0,0,0,0]}],"ticks":[]})).unwrap();
        let job = ImportJob {
            input: None,
            manifest,
            directory: PathBuf::from("never-written"),
            owner: current_module(),
            execution: crate::modules::current_execution(current_module()),
            records: BTreeMap::new(),
            index: 0,
        };
        assert!(job.finish().is_err());
        assert_eq!(status().mode, ReplayMode::Idle);
    }
    #[test]
    fn imported_history_never_rewrites_its_source_manifest() {
        let directory = std::env::temp_dir().join(format!("rsvz-readonly-import-test-{}", std::process::id()));
        assert!(!directory.exists());
        let mut state = Controller::new(WorldId::from_raw(1), directory.clone(), 10, None);
        state.read_only = true;
        write_manifest(&state).unwrap();
        assert!(!directory.exists());
    }
}
