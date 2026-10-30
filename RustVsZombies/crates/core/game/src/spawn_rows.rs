//! AvZ per-kind averaging of newly born zombies, using actual row terrain.
use crate::{
    access::operation_error,
    runtime::{RuntimeError, RuntimeResult},
    spawn_distribution::Mt19937,
};
use rsvz_backend_api::ZombieReadBackend;
use rsvz_backend_api::{
    BoardReadinessBackend, CurrentWaveBackend, GameUiBackend, GridGeometryBackend, GridTerrainBackend, SceneBackend,
    WorldIdentityBackend, ZombieRowTranslationBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{FiniteF32, GridRowKind, ZombieKind};
use rsvz_schedule::{
    module::{ModuleSlot, current_module},
    tick::{TickControl, TickHandle, TickLifetime, TickOptions, TickTaskState, with_scheduler},
};

#[derive(Clone, Copy)]
struct Rows {
    values: [u8; 6],
    len: usize,
}
impl Rows {
    const fn empty() -> Self {
        Self { values: [0; 6], len: 0 }
    }
    fn push(&mut self, row: u8) {
        self.values[self.len] = row;
        self.len += 1;
    }
}
fn possible_rows(
    kind: ZombieKind, terrain: [GridRowKind; 6], active: [bool; 6], height: i32, has_pool: bool, wave: i32,
) -> Rows {
    let mut lands = Rows::empty();
    let mut pools = Rows::empty();
    // Match AFieldInfo's active-row count and Aquarium sixth-row correction.
    let count = active.iter().filter(|&&enabled| enabled).count();
    for (row, &kind) in terrain.iter().take(count).enumerate() {
        if kind == GridRowKind::Pool && !(height == 100 && row == 5) {
            pools.push(row as u8);
        } else {
            lands.push(row as u8);
        }
    }
    if pools.len == 0 {
        pools.push(5);
    }
    if matches!(kind, ZombieKind::Snorkel | ZombieKind::DolphinRider) {
        return pools;
    }
    if kind == ZombieKind::Dancing && !(has_pool && height == 85) {
        let mut interior = Rows::empty();
        for &row in &lands.values[..lands.len] {
            if row != 0 && usize::from(row) + 1 != count {
                interior.push(row);
            }
        }
        return interior;
    }
    if has_pool
        && wave > 5
        && matches!(
            kind,
            ZombieKind::Normal | ZombieKind::Conehead | ZombieKind::Buckethead | ZombieKind::Balloon
        )
    {
        return Rows {
            values: [0, 1, 2, 3, 4, 5],
            len: 6,
        };
    }
    lands
}

/// Reorders only live age-zero zombies; Bungees and older zombies are untouched.
/// An empty type slice means all kinds. Randomness is independent of game RNG.
pub fn apply(types: &[ZombieKind], seed: u32) -> RuntimeResult<usize>
where
    CurrentBackend:
        GridTerrainBackend + GridGeometryBackend + SceneBackend + CurrentWaveBackend + ZombieRowTranslationBackend,
{
    crate::access::with_backend(|backend| {
        let height = backend.grid_to_pixel_y(0, 1).map_err(operation_error)?
            - backend.grid_to_pixel_y(0, 0).map_err(operation_error)?;
        let mut terrain = [GridRowKind::Dirt; 6];
        let mut active = [false; 6];
        for row in 0..6 {
            terrain[row] = backend.grid_row_kind(row as u8).map_err(operation_error)?;
            active[row] = terrain[row] != GridRowKind::Dirt
                || backend.row_can_have_zombies(row as i32).map_err(operation_error)?;
        }
        let has_pool = backend.scene().map_err(operation_error)?.has_pool();
        let wave = backend.current_wave().map_err(operation_error)?.0;
        let mut rows = [Rows::empty(); 33];
        let mut counts = [0_usize; 33];
        let mut rng = Mt19937::new(seed);
        for kind in ZombieKind::ALL {
            let choices = &mut rows[kind.code() as usize];
            *choices = possible_rows(kind, terrain, active, height, has_pool, wave);
            for i in (1..choices.len).rev() {
                let other = rng.below((i + 1) as u64) as usize;
                choices.values.swap(i, other);
            }
        }
        let mut changed = 0;
        for zombie in backend.zombies().map_err(operation_error)? {
            if backend.zombie_age(zombie) != 0 {
                continue;
            }
            let kind = backend.zombie_kind(zombie).map_err(operation_error)?;
            if kind == ZombieKind::Bungee || (!types.is_empty() && !types.contains(&kind)) {
                continue;
            }
            let index = kind.code() as usize;
            let choices = rows[index];
            if choices.len == 0 {
                return Err(RuntimeError::new("no legal row for fresh zombie"));
            }
            let row = choices.values[counts[index] % choices.len];
            counts[index] += 1;
            let delta = i32::from(row)
                .checked_sub(backend.zombie_row(zombie))
                .and_then(|d| d.checked_mul(height))
                .ok_or_else(|| RuntimeError::new("zombie row translation overflow"))?;
            backend
                .translate_zombie_row(
                    zombie,
                    row,
                    FiniteF32::new(delta as f32).map_err(|e| RuntimeError::new(e.to_string()))?,
                )
                .map_err(operation_error)?;
            changed += 1;
        }
        Ok(changed)
    })
}

/// Owns one birth observer in the registering source. It reads the actual wave
/// transition rather than a predicted timeline clock: user refresh controls can
/// change a countdown after a timeline has already predicted that wave's time.
pub struct AverageRows {
    source: ModuleSlot,
    handle: TickHandle,
}
impl AverageRows {
    pub fn active(&self) -> bool {
        self.source == current_module() && with_scheduler(|tasks| tasks.state(self.handle) != TickTaskState::Stopped)
    }
    pub fn cancel(&mut self) -> RuntimeResult<()> {
        if self.source != current_module() {
            return Err(RuntimeError::new("average-row registration belongs to another source"));
        }
        with_scheduler(|tasks| {
            let _ = tasks.stop(self.handle);
        });
        Ok(())
    }
}

pub fn schedule(seed: u32) -> RuntimeResult<AverageRows>
where
    CurrentBackend: GridTerrainBackend
        + GridGeometryBackend
        + SceneBackend
        + CurrentWaveBackend
        + ZombieRowTranslationBackend
        + WorldIdentityBackend
        + GameUiBackend
        + BoardReadinessBackend,
{
    let (world, mut last_wave) = crate::access::with_backend(|backend| {
        Ok::<_, RuntimeError>((
            backend
                .world_identity()
                .map_err(operation_error)?
                .ok_or_else(|| RuntimeError::new("no average-row world"))?,
            backend.current_wave().map_err(operation_error)?.0,
        ))
    })?;
    let handle = crate::tick::spawn(
        TickOptions::playing_frame()
            .lifetime(TickLifetime::Fight)
            .name("average_spawn_rows"),
        move |_| {
            if crate::board::world_id()? != Some(world) {
                return Ok(TickControl::Stop);
            }
            let wave = crate::access::with_backend(|backend| backend.current_wave())
                .map_err(operation_error)?
                .0;
            if wave != last_wave {
                let forward = wave > last_wave;
                last_wave = wave;
                if forward {
                    apply(&[], seed.wrapping_add(wave as u32))?;
                }
            }
            Ok(TickControl::Continue)
        },
    );
    Ok(AverageRows {
        source: current_module(),
        handle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn values(rows: Rows) -> Vec<u8> {
        rows.values[..rows.len].to_vec()
    }
    #[test]
    fn avz_legal_rows_cover_pool_wave_boundary_and_dry_field_fallback() {
        let pool = [
            GridRowKind::Grass,
            GridRowKind::Grass,
            GridRowKind::Pool,
            GridRowKind::Pool,
            GridRowKind::Grass,
            GridRowKind::Grass,
        ];
        assert_eq!(
            values(possible_rows(ZombieKind::Dancing, pool, [true; 6], 85, true, 1)),
            [0, 1, 4, 5]
        );
        assert_eq!(
            values(possible_rows(ZombieKind::Normal, pool, [true; 6], 85, true, 5)),
            [0, 1, 4, 5]
        );
        assert_eq!(
            values(possible_rows(ZombieKind::Normal, pool, [true; 6], 85, true, 6)),
            [0, 1, 2, 3, 4, 5]
        );
        let dry = [GridRowKind::Grass; 6];
        let active = [true, true, true, true, true, false];
        assert_eq!(
            values(possible_rows(ZombieKind::Dancing, dry, active, 100, false, 1)),
            [1, 2, 3]
        );
        assert_eq!(
            values(possible_rows(ZombieKind::DolphinRider, dry, active, 100, false, 1)),
            [5]
        );
        assert_eq!(
            values(possible_rows(ZombieKind::Buckethead, dry, active, 100, false, 6)),
            [0, 1, 2, 3, 4]
        );
    }
}
