//! Ordered lineup placement with immediately usable cannons.
use super::{LineupApplyOptions, LineupDocument, LineupObject};
use crate::{
    access::operation_error,
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::IceTrailClearBackend;
use rsvz_backend_api::{
    BoardReadinessBackend, ChallengeGravePlacementBackend, DeadObjectReclaimBackend, GameUiBackend,
    GridItemCreateBackend, GridItemEditBackend, GridItemPoolBackend, GridItemReadBackend, IZombiePlantStyleBackend,
    LawnMowerClearBackend, PlantCreateBackend, PlantPoolBackend, PlantPresenceBackend, PlantRemoveBackend,
    PlantSleepBackend, PlantStateCountdownWriteBackend, RakePlacementBackend, SceneBackend, SceneLayoutBackend,
    WorldIdentityBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{GameUi, GridItemKind, WorldId};

pub trait NativeLineupBackend:
    BoardReadinessBackend
    + GameUiBackend
    + WorldIdentityBackend
    + SceneLayoutBackend
    + IZombiePlantStyleBackend
    + PlantCreateBackend
    + PlantRemoveBackend
    + PlantPresenceBackend
    + rsvz_backend_api::PlantStateWriteBackend
    + rsvz_backend_api::PlantIdleAnimationBackend
    + rsvz_backend_api::PlantEffectCountdownWriteBackend
    + rsvz_backend_api::IceTrailClearBackend
    + PlantSleepBackend
    + PlantStateCountdownWriteBackend
    + PlantPoolBackend
    + GridItemCreateBackend
    + GridItemPoolBackend
    + ChallengeGravePlacementBackend
    + RakePlacementBackend
    + LawnMowerClearBackend
    + DeadObjectReclaimBackend
{
}
impl<T> NativeLineupBackend for T where
    T: BoardReadinessBackend
        + GameUiBackend
        + WorldIdentityBackend
        + SceneLayoutBackend
        + IZombiePlantStyleBackend
        + PlantCreateBackend
        + PlantRemoveBackend
        + PlantPresenceBackend
        + rsvz_backend_api::PlantStateWriteBackend
        + rsvz_backend_api::PlantIdleAnimationBackend
        + rsvz_backend_api::PlantEffectCountdownWriteBackend
        + rsvz_backend_api::IceTrailClearBackend
        + PlantSleepBackend
        + PlantStateCountdownWriteBackend
        + PlantPoolBackend
        + GridItemCreateBackend
        + GridItemPoolBackend
        + ChallengeGravePlacementBackend
        + RakePlacementBackend
        + LawnMowerClearBackend
        + DeadObjectReclaimBackend
{
}

pub struct NativeLineupPlan {
    document: LineupDocument,
    options: LineupApplyOptions,
    world: WorldId,
    started: bool,
    placed: usize,
    skipped: usize,
    izombie_style: bool,
}
impl NativeLineupPlan {
    /// Check lineup inputs against the native pool sizes before replacing a Board.
    /// The caller validates the reset's source state; placement and scene resources
    /// are checked again on the fresh Board by `prepare`. Old grid items do not survive reset.
    pub fn validate_reset(document: &LineupDocument, options: LineupApplyOptions) -> RuntimeResult<()>
    where
        CurrentBackend: NativeLineupBackend,
    {
        crate::access::with_backend(|backend| {
            if backend.scene().map_err(operation_error)? != document.scene() && !options.allow_scene_switch {
                return Err(RuntimeError::new("lineup scene differs; scene switching is disabled"));
            }
            validate_capacity(
                document,
                backend.plant_pool_capacity().map_err(operation_error)? as usize,
                backend.grid_item_pool_capacity().map_err(operation_error)? as usize,
                0,
            )
        })
    }

    pub fn prepare(document: LineupDocument, options: LineupApplyOptions, izombie_style: bool) -> RuntimeResult<Self>
    where
        CurrentBackend: NativeLineupBackend,
    {
        let world = crate::board::world_id()?.ok_or_else(|| RuntimeError::new("no lineup world"))?;
        let plan = Self {
            document,
            options,
            world,
            started: false,
            placed: 0,
            skipped: 0,
            izombie_style,
        };
        plan.validate()?;
        Ok(plan)
    }
    pub fn started(&self) -> bool {
        self.started
    }
    pub fn placed(&self) -> usize {
        self.placed
    }
    pub fn skipped(&self) -> usize {
        self.skipped
    }
    pub fn world(&self) -> WorldId {
        self.world
    }
    pub fn validate(&self) -> RuntimeResult<()>
    where
        CurrentBackend: NativeLineupBackend,
    {
        if self.started {
            return Err(RuntimeError::new("lineup plan was already submitted"));
        }
        crate::access::with_backend(|backend| {
            let ready = match backend.game_ui().map_err(operation_error)? {
                GameUi::Playing => backend.playing_board_ready(),
                GameUi::LevelIntro => backend.level_intro_board_ready(),
                _ => return Err(RuntimeError::new("lineup requires a chooser or battle")),
            }
            .map_err(operation_error)?;
            if !ready || backend.world_identity().map_err(operation_error)? != Some(self.world) {
                return Err(RuntimeError::new("lineup world changed or is not ready"));
            }
            if backend.scene().map_err(operation_error)? != self.document.scene() {
                if !self.options.allow_scene_switch {
                    return Err(RuntimeError::new("lineup scene differs; scene switching is disabled"));
                }
                backend
                    .validate_scene_layout(self.document.scene())
                    .map_err(operation_error)?;
            }
            let mut items = 0;
            for item in backend.grid_items().map_err(operation_error)? {
                if !cleared_item(backend.grid_item_kind(item).map_err(operation_error)?) {
                    items += 1;
                }
            }
            validate_capacity(
                &self.document,
                backend.plant_pool_capacity().map_err(operation_error)? as usize,
                backend.grid_item_pool_capacity().map_err(operation_error)? as usize,
                items,
            )
        })
    }
    pub fn apply(&mut self) -> RuntimeResult<()>
    where
        CurrentBackend: NativeLineupBackend,
    {
        self.validate()?;
        crate::access::with_exclusive_backend(|backend| {
            // Obtain exclusive access before the first removal. A caller holding
            // an entity view must fail without leaving a half-cleared lineup.
            self.started = true;
            backend.clear_ice_trails().map_err(operation_error)?;
            for item in backend.grid_items().map_err(operation_error)? {
                if cleared_item(backend.grid_item_kind(item).map_err(operation_error)?) {
                    backend.remove_grid_item(item).map_err(operation_error)?;
                }
            }
            for plant in backend.present_plants().map_err(operation_error)? {
                backend.remove_plant(plant).map_err(operation_error)?;
            }
            if backend.scene().map_err(operation_error)? != self.document.scene() {
                backend
                    .set_scene_layout(self.document.scene())
                    .map_err(operation_error)?;
            }
            if self.options.clear_lawn_mowers {
                backend.clear_lawn_mowers().map_err(operation_error)?;
            }
            backend.reclaim_dead_objects().map_err(operation_error)?;
            // PT places rake records first, then follows source order for the
            // remaining objects. Repeated plant layers are not normalized away.
            for object in self.document.objects() {
                if let LineupObject::Rake(grid) = object {
                    if backend.place_rake(*grid).map_err(operation_error)? {
                        self.placed += 1;
                    } else {
                        self.skipped += 1;
                    }
                }
            }
            for &object in self.document.objects() {
                let reclaim = match object {
                    LineupObject::Plant(grid, plant) => {
                        let selection = plant
                            .selection
                            .checked()
                            .map_err(|e| RuntimeError::new(e.to_string()))?;
                        let mut handle = backend.add_plant(selection, grid).map_err(operation_error)?;
                        let imitate = selection.imitator_target().is_some();
                        if imitate {
                            handle = backend.morph_imitator(handle).map_err(operation_error)?;
                        }
                        if self.izombie_style {
                            backend.setup_izombie_plant(handle).map_err(operation_error)?;
                        }
                        if let Some(awake) = plant
                            .awake
                            .filter(|_| super::validate::should_apply_mushroom_awake_state(self.document.scene()))
                        {
                            backend.set_plant_sleeping(handle, !awake).map_err(operation_error)?;
                        }
                        if selection.effective_kind() == rsvz_model::PlantKind::CobCannon {
                            super::apply::ready_cannon(backend, handle)?;
                        }
                        if plant.grown {
                            backend.set_plant_state_countdown(handle, 1).map_err(operation_error)?;
                        }
                        imitate
                    }
                    LineupObject::Ladder(grid) => {
                        backend.add_ladder(grid).map_err(operation_error)?;
                        false
                    }
                    LineupObject::Grave(grid) => {
                        backend.spawn_challenge_grave(grid).map_err(operation_error)?;
                        true
                    }
                    LineupObject::Rake(_) => false,
                };
                if !matches!(object, LineupObject::Rake(_)) {
                    self.placed += 1;
                }
                // No plant handle escapes its arm. Reclaim imitater placeholders
                // before the next allocation without advancing a game frame.
                if reclaim {
                    backend.reclaim_dead_objects().map_err(operation_error)?;
                }
            }
            Ok(())
        })
    }
}
fn validate_capacity(
    document: &LineupDocument, plant_capacity: usize, item_capacity: usize, mut items: usize,
) -> RuntimeResult<()> {
    let mut plants = 0;
    for object in document.objects() {
        if let LineupObject::Plant(_, plant) = object {
            plants += 1;
            // Imitater allocates its successor before reclaiming the placeholder.
            let needed = plants
                + usize::from(
                    plant
                        .selection
                        .checked()
                        .map_err(|e| RuntimeError::new(e.to_string()))?
                        .imitator_target()
                        .is_some(),
                );
            if needed > plant_capacity {
                return Err(RuntimeError::new("lineup exceeds safe native plant capacity"));
            }
        } else {
            items += 1;
        }
    }
    if items > item_capacity {
        return Err(RuntimeError::new("lineup exceeds native grid-item capacity"));
    }
    Ok(())
}

fn cleared_item(kind: GridItemKind) -> bool {
    matches!(
        kind,
        GridItemKind::Grave | GridItemKind::Crater | GridItemKind::Ladder | GridItemKind::Rake
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_preflight_checks_capacity_without_retaining_old_items() {
        let document = LineupDocument::parse_auto("0,0 1 1 0 0 0,30 1 2 0 0 0").unwrap();
        assert!(validate_capacity(&document, 1, 1, 0).is_ok());
        assert!(validate_capacity(&document, 0, 1, 0).is_err());
        assert!(validate_capacity(&document, 1, 0, 0).is_err());
        assert!(validate_capacity(&document, 1, 1, 1).is_err());
        let imitater = LineupDocument::parse_auto("0,0 1 1 0 0 1").unwrap();
        assert!(validate_capacity(&imitater, 1, 0, 0).is_err());
        assert!(validate_capacity(&imitater, 2, 0, 0).is_ok());
    }
}
