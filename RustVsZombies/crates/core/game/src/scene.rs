//! Explicit destructive scene change. Flags, clocks, wave tables and preferences
//! are not reset. The caller owns any policy for stopping a different execution.
use crate::{
    access::operation_error,
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::IceTrailClearBackend;
use rsvz_backend_api::{
    DeadObjectReclaimBackend, GridItemEditBackend, InputBackend, PlantPresenceBackend, PlantRemoveBackend,
    SceneLayoutBackend, TransientClearBackend, WorldIdentityBackend, ZombiePresenceBackend, ZombieRemoveBackend,
};
use rsvz_backend_api::{GridItemReadBackend, LawnMowerClearBackend, LawnMowerReadBackend, LawnMowerRestoreBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{SceneKind, WorldId};

pub struct SceneChange {
    world: WorldId,
    scene: SceneKind,
    started: bool,
}
impl SceneChange {
    pub fn prepare(scene: SceneKind) -> RuntimeResult<Self>
    where
        CurrentBackend: SceneLayoutBackend + WorldIdentityBackend,
    {
        if scene == SceneKind::Greenhouse {
            return Err(RuntimeError::new("Greenhouse is outside the configured scene choices"));
        }
        crate::access::with_backend(|backend| backend.validate_scene_layout(scene)).map_err(operation_error)?;
        Ok(Self {
            world: crate::board::world_id()?.ok_or_else(|| RuntimeError::new("no current world"))?,
            scene,
            started: false,
        })
    }
    pub fn world(&self) -> WorldId {
        self.world
    }
    pub fn started(&self) -> bool {
        self.started
    }
    pub fn validate(&self) -> RuntimeResult<()>
    where
        CurrentBackend: SceneLayoutBackend + WorldIdentityBackend,
    {
        if self.started || crate::board::world_id()? != Some(self.world) {
            return Err(RuntimeError::new("scene plan is stale or already applied"));
        }
        crate::access::with_backend(|backend| backend.validate_scene_layout(self.scene)).map_err(operation_error)
    }
    pub fn apply(&mut self) -> RuntimeResult<()>
    where
        CurrentBackend: SceneLayoutBackend
            + WorldIdentityBackend
            + PlantPresenceBackend
            + PlantRemoveBackend
            + ZombiePresenceBackend
            + ZombieRemoveBackend
            + GridItemEditBackend
            + rsvz_backend_api::IceTrailClearBackend
            + TransientClearBackend
            + DeadObjectReclaimBackend
            + InputBackend,
        CurrentBackend: LawnMowerReadBackend + LawnMowerClearBackend + LawnMowerRestoreBackend,
    {
        self.validate()?;
        crate::access::with_exclusive_backend(|backend| {
            // Acquire exclusive access before the first native side effect.
            self.started = true;
            backend.release_mouse().map_err(operation_error)?;
            backend.clear_ice_trails().map_err(operation_error)?;
            let had_mowers = backend.lawn_mowers().map_err(operation_error)?.next().is_some();
            if had_mowers {
                backend.clear_lawn_mowers().map_err(operation_error)?;
            }
            for plant in backend.present_plants().map_err(operation_error)? {
                backend.remove_plant(plant).map_err(operation_error)?;
            }
            for zombie in backend.present_zombies().map_err(operation_error)? {
                backend.remove_zombie(zombie).map_err(operation_error)?;
            }
            for item in backend.grid_items().map_err(operation_error)? {
                backend.remove_grid_item(item).map_err(operation_error)?;
            }
            backend.clear_native_projectiles().map_err(operation_error)?;
            backend.clear_native_items().map_err(operation_error)?;
            backend.reclaim_dead_objects().map_err(operation_error)?;
            backend.set_scene_layout(self.scene).map_err(operation_error)?;
            if had_mowers {
                backend.initialize_lawn_mowers_ready().map_err(operation_error)?;
            }
            Ok(())
        })
    }
}
