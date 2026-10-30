//! Applying explicit spawn requests to an existing world, without Opening,
//! card selection, battle start, script-stop policy or tool persistence.
use crate::{
    access::operation_error,
    runtime::{RuntimeError, RuntimeResult},
    spawn_distribution::{self, Distribution},
};
use rsvz_backend_api::{
    BoardReadinessBackend, CurrentNaturalSpawnBackend, GameUiBackend, SpawnScheduleBackend, SpawnScheduleReadBackend,
    WorldIdentityBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{GameUi, SpawnList, WorldId, ZombieKind};

pub struct WaveStatistics {
    pub huge: bool,
    pub counts: [u16; 33],
}

pub struct SpawnStatistics {
    pub world: WorldId,
    pub allowed: [bool; 33],
    pub waves: Vec<WaveStatistics>,
}

/// Read the actual native table on demand. Each wave ends at its first empty
/// slot; stale storage after that sentinel is not part of the spawn schedule.
pub fn statistics() -> RuntimeResult<SpawnStatistics>
where
    CurrentBackend: SpawnScheduleReadBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    ready()?;
    crate::access::with_backend(|backend| {
        let world = backend
            .world_identity()
            .map_err(operation_error)?
            .ok_or_else(|| RuntimeError::new("no spawn world"))?;
        let count = backend.spawn_wave_count().map_err(operation_error)?;
        let mut allowed = [false; 33];
        for kind in ZombieKind::ALL {
            allowed[kind.code() as usize] = backend.spawn_type_allowed(kind).map_err(operation_error)?;
        }
        let mut waves = Vec::with_capacity(count);
        for wave in 0..count {
            let mut counts = [0; 33];
            for slot in 0..rsvz_model::SPAWN_SLOTS_PER_WAVE {
                let slot = rsvz_model::SpawnWaveSlot::new(slot).expect("bounded spawn slot");
                let Some(kind) = backend.spawn_slot(wave, slot).map_err(operation_error)? else {
                    break;
                };
                counts[kind.code() as usize] += 1;
            }
            waves.push(WaveStatistics {
                huge: backend.spawn_huge_wave(wave).map_err(operation_error)?,
                counts,
            });
        }
        Ok(SpawnStatistics { world, allowed, waves })
    })
}

fn ready() -> RuntimeResult<()>
where
    CurrentBackend: GameUiBackend + BoardReadinessBackend,
{
    crate::access::with_backend(|backend| {
        let ready = match backend.game_ui().map_err(operation_error)? {
            GameUi::LevelIntro => backend.level_intro_board_ready(),
            GameUi::Playing => backend.playing_board_ready(),
            _ => return Err(RuntimeError::new("spawn configuration requires a chooser or battle")),
        }
        .map_err(operation_error)?;
        if ready {
            Ok(())
        } else {
            Err(RuntimeError::new("spawn board is not ready"))
        }
    })
}

pub struct SpawnTablePlan {
    world: WorldId,
    table: SpawnList,
    started: bool,
}
impl SpawnTablePlan {
    pub fn table(&self) -> &SpawnList {
        &self.table
    }
    pub fn started(&self) -> bool {
        self.started
    }
    pub fn apply(&mut self) -> RuntimeResult<()>
    where
        CurrentBackend: SpawnScheduleBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
    {
        if self.started {
            return Err(RuntimeError::new("spawn plan was already submitted"));
        }
        ready()?;
        crate::access::with_backend(|backend| {
            if backend.world_identity().map_err(operation_error)? != Some(self.world)
                || backend.spawn_wave_count().map_err(operation_error)? != self.table.wave_count()
            {
                return Err(RuntimeError::new("world or wave count changed after spawn preflight"));
            }
            Ok(())
        })?;
        self.started = true;
        crate::logic::zombies::apply_spawn_list(&self.table).map_err(|error| RuntimeError::new(error.to_string()))?;
        crate::access::with_backend(|backend| backend.refresh_spawn_preview()).map_err(operation_error)
    }
}

/// Samples actual native huge-wave rules, including Adventure's short levels.
/// The caller selects types, weights, Giga mask and independent generation seed.
pub fn prepare_limited(types: &[ZombieKind], distribution: Distribution<'_>, seed: u32) -> RuntimeResult<SpawnTablePlan>
where
    CurrentBackend: SpawnScheduleReadBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    ready()?;
    let (world, flags) = crate::access::with_backend(|backend| {
        let world = backend
            .world_identity()
            .map_err(operation_error)?
            .ok_or_else(|| RuntimeError::new("no spawn world"))?;
        let count = backend.spawn_wave_count().map_err(operation_error)?;
        let flags = (0..count)
            .map(|wave| backend.spawn_huge_wave(wave).map_err(operation_error))
            .collect::<RuntimeResult<Vec<_>>>()?;
        Ok::<_, RuntimeError>((world, flags))
    })?;
    let table = spawn_distribution::limited_schedule(types, &flags, distribution, seed)
        .map_err(|error| RuntimeError::new(error.to_string()))?;
    Ok(SpawnTablePlan {
        world,
        table,
        started: false,
    })
}

pub struct NaturalSpawnPlan {
    world: WorldId,
    count: usize,
    types: [bool; 33],
    seed: u32,
    started: bool,
}
impl NaturalSpawnPlan {
    pub fn started(&self) -> bool {
        self.started
    }
    pub fn apply(&mut self) -> RuntimeResult<()>
    where
        CurrentBackend: CurrentNaturalSpawnBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
    {
        if self.started {
            return Err(RuntimeError::new("natural spawn plan was already submitted"));
        }
        ready()?;
        crate::access::with_backend(|backend| {
            if backend.world_identity().map_err(operation_error)? != Some(self.world)
                || backend.spawn_wave_count().map_err(operation_error)? != self.count
            {
                return Err(RuntimeError::new("world or wave count changed after spawn preflight"));
            }
            self.started = true;
            for kind in ZombieKind::ALL {
                backend
                    .set_spawn_type_allowed(kind, self.types[kind.code() as usize])
                    .map_err(operation_error)?;
            }
            backend
                .generate_current_natural_spawn(self.seed)
                .map_err(operation_error)?;
            backend.refresh_spawn_preview().map_err(operation_error)
        })
    }
}

/// `seed` is the native board seed. Native level/profile/stage derivation is
/// retained while only this generation gets its isolated random stream.
pub fn prepare_natural(types: &[ZombieKind], seed: u32) -> RuntimeResult<NaturalSpawnPlan>
where
    CurrentBackend: CurrentNaturalSpawnBackend + WorldIdentityBackend + GameUiBackend + BoardReadinessBackend,
{
    let mut selected = [false; 33];
    for kind in types {
        selected[kind.code() as usize] = true;
    }
    // The native point picker needs a cheap fallback; neither tool nor core
    // silently changes the caller's selection to satisfy this precondition.
    if !selected[0] && !selected[26] {
        return Err(RuntimeError::new(
            "native natural generation requires normal or peashooter zombie",
        ));
    }
    ready()?;
    crate::access::with_backend(|backend| {
        Ok(NaturalSpawnPlan {
            world: backend
                .world_identity()
                .map_err(operation_error)?
                .ok_or_else(|| RuntimeError::new("no spawn world"))?,
            count: backend.spawn_wave_count().map_err(operation_error)?,
            types: selected,
            seed,
            started: false,
        })
    })
}

// Restoration history, deliberately retained by the shared game library across
// a modifier reconnect. This is not a backend query cache or a generated table.
thread_local! {static ORIGINAL_SEED:std::cell::Cell<Option<(WorldId,i32,u32)>>=const{std::cell::Cell::new(None)};}
/// Capture this round's seed before its first explicit tool mutation. Chooser to
/// battle keeps the key; the native survival stage survives module reconnects.
pub fn original_seed_for_round() -> RuntimeResult<u32>
where
    CurrentBackend: WorldIdentityBackend + CurrentNaturalSpawnBackend + rsvz_backend_api::NativeLevelFlowBackend,
{
    let world = crate::board::world_id()?.ok_or_else(|| RuntimeError::new("no spawn world"))?;
    use rsvz_backend_api::NativeLevelFlowBackend;
    let round = crate::access::with_backend(|b| b.survival_stage()).map_err(operation_error)?;
    if let Some(seed) = stored_original_seed(world, round) {
        return Ok(seed);
    }
    let seed = crate::access::with_backend(|b| b.board_spawn_seed()).map_err(operation_error)?;
    ORIGINAL_SEED.set(Some((world, round, seed)));
    Ok(seed)
}
/// A successful native restore changes allocation identity without making a new
/// spawn baseline. The caller supplies its previously captured restoration value.
pub fn restore_seed_baseline(world: WorldId, seed: u32) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::NativeLevelFlowBackend,
{
    use rsvz_backend_api::NativeLevelFlowBackend;
    let stage = crate::access::with_backend(|b| b.survival_stage()).map_err(operation_error)?;
    ORIGINAL_SEED.set(Some((world, stage, seed)));
    Ok(())
}

fn stored_original_seed(world: WorldId, stage: i32) -> Option<u32> {
    ORIGINAL_SEED
        .get()
        .filter(|&(w, s, _)| w == world && s == stage)
        .map(|(_, _, seed)| seed)
}
#[cfg(test)]
mod baseline_tests {
    use super::*;
    #[test]
    fn seed_restoration_history_survives_source_reconnect_but_not_native_round_change() {
        let world = WorldId::from_raw(7);
        ORIGINAL_SEED.set(Some((world, 3, 12345)));
        assert_eq!(stored_original_seed(world, 3), Some(12345));
        assert_eq!(stored_original_seed(world, 4), None);
        assert_eq!(stored_original_seed(WorldId::from_raw(8), 3), None);
        ORIGINAL_SEED.set(None);
    }
}
