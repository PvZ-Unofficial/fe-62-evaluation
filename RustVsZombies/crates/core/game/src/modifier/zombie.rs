//! Direct zombie state modifier helpers.

use crate::backend::{ZombieBodyHealthWriteBackend, ZombieCreateBackend, ZombieReadBackend, ZombieXWriteBackend};
use crate::model::{Grid, I32RepresentableF32, ObjectEditOutcome, PositiveHp, ZombieId, ZombieKind};

use super::value::ModifierValueError;

/// Failure from the backend-neutral user-facing zombie placement composition.
#[derive(Debug, thiserror::Error)]
pub enum SpawnZombieError {
    #[error("zombie placement rejected: {0}")]
    Rejected(crate::runtime::RuntimeError),
}

/// Places one zombie at a grid cell and returns the newly allocated cross-frame ID.
pub fn spawn_zombie(kind: ZombieKind, grid: Grid) -> Result<ZombieId, SpawnZombieError>
where
    rsvz_current::CurrentBackend: ZombieCreateBackend,
{
    crate::access::with_backend(|backend| {
        let zombie = backend
            .place_zombie(kind, grid)
            .map_err(|error| SpawnZombieError::Rejected(crate::access::operation_error(error)))?;
        Ok(backend.zombie_id(zombie))
    })
}

/// Sets main body health for a live zombie ID.
pub fn set_zombie_body_hp(id: ZombieId, hp: i32) -> Result<ObjectEditOutcome, ModifierValueError>
where
    rsvz_current::CurrentBackend: ZombieReadBackend + ZombieBodyHealthWriteBackend,
{
    crate::access::with_backend(|backend| {
        let Some(handle) = crate::live_value::read_or_abort(backend.zombie(id), "zombie") else {
            return Ok(ObjectEditOutcome::Missing);
        };
        let hp = PositiveHp::new(hp).map_err(ModifierValueError::InvalidValue)?;
        Ok(backend
            .set_zombie_body_hp(handle, hp)
            .map_err(|error| ModifierValueError::Rejected(crate::access::operation_error(error)))?)
    })
}

/// Sets main body health for live zombies of `kind` and returns the number of applied edits.
pub fn set_zombie_body_hp_by_kind(kind: ZombieKind, hp: i32) -> Result<usize, ModifierValueError>
where
    rsvz_current::CurrentBackend: ZombieReadBackend + ZombieBodyHealthWriteBackend,
{
    crate::access::with_backend(|backend| {
        let hp = PositiveHp::new(hp).map_err(ModifierValueError::InvalidValue)?;
        let mut applied = 0;
        for zombie in crate::live_value::read_or_abort(backend.zombies(), "zombies") {
            if backend
                .zombie_kind(zombie)
                .unwrap_or_else(|error| crate::diagnostics::abort_operation(error.into()))
                == kind
                && backend
                    .set_zombie_body_hp(zombie, hp)
                    .map_err(|error| ModifierValueError::Rejected(crate::access::operation_error(error)))?
                    == ObjectEditOutcome::Applied
            {
                applied += 1;
            }
        }
        Ok(applied)
    })
}

/// Sets main body health for every live zombie and returns the number of applied edits.
pub fn set_all_zombies_body_hp(hp: i32) -> Result<usize, ModifierValueError>
where
    rsvz_current::CurrentBackend: ZombieReadBackend + ZombieBodyHealthWriteBackend,
{
    crate::access::with_backend(|backend| {
        let hp = PositiveHp::new(hp).map_err(ModifierValueError::InvalidValue)?;
        let mut applied = 0;
        for zombie in crate::live_value::read_or_abort(backend.zombies(), "zombies") {
            if backend
                .set_zombie_body_hp(zombie, hp)
                .map_err(|error| ModifierValueError::Rejected(crate::access::operation_error(error)))?
                == ObjectEditOutcome::Applied
            {
                applied += 1;
            }
        }
        Ok(applied)
    })
}

/// Sets the horizontal coordinate for a live zombie ID.
pub fn set_zombie_x(id: ZombieId, x: f32) -> Result<ObjectEditOutcome, ModifierValueError>
where
    rsvz_current::CurrentBackend: ZombieReadBackend + ZombieXWriteBackend,
{
    crate::access::with_backend(|backend| {
        let x = I32RepresentableF32::new(x).map_err(ModifierValueError::InvalidValue)?;
        let Some(handle) = crate::live_value::read_or_abort(backend.zombie(id), "zombie") else {
            return Ok(ObjectEditOutcome::Missing);
        };
        Ok(backend
            .set_zombie_x(handle, x)
            .map_err(|error| ModifierValueError::Rejected(crate::access::operation_error(error)))?)
    })
}

/// Applies the selected PT action to occupied, not-disappeared objects. Native
/// update performs any later removal; this does not call DieWithLoot/DieNoLoot.
pub fn apply_zombie_batch(action: rsvz_model::ZombieBatchAction) -> crate::runtime::RuntimeResult<usize>
where
    rsvz_current::CurrentBackend: rsvz_backend_api::ZombiePresenceBackend + rsvz_backend_api::ZombieStatusWriteBackend,
{
    use rsvz_backend_api::{ZombiePresenceBackend, ZombieStatusWriteBackend};
    use rsvz_model::{ZombieBatchAction as Action, ZombieDeathState};
    crate::access::with_backend(|backend| {
        let mut changed = 0;
        for zombie in backend.present_zombies().map_err(crate::access::operation_error)? {
            let result = match action {
                Action::Kill => backend.set_zombie_death_state(zombie, ZombieDeathState::Mowered),
                Action::Disappear => backend.set_zombie_death_state(zombie, ZombieDeathState::Dying),
                Action::Black => backend.set_zombie_death_state(zombie, ZombieDeathState::Burned),
                Action::Hypno => backend.set_zombie_mind_controlled(zombie, true),
                Action::Freeze | Action::Decelerate => {
                    backend
                        .set_zombie_frozen_countdown(
                            zombie,
                            rsvz_model::NonNegativeI32::new(if action == Action::Freeze { 600 } else { 1 }).unwrap(),
                        )
                        .map_err(crate::access::operation_error)?;
                    backend.set_zombie_chilled_countdown(zombie, rsvz_model::NonNegativeI32::new(2000).unwrap())
                }
                Action::Immobilize => {
                    backend.set_zombie_buttered_countdown(zombie, rsvz_model::NonNegativeI32::new(500).unwrap())
                }
                Action::BlowAway => backend.set_zombie_blown_away(zombie, true),
                Action::Dying => backend.set_zombie_has_head(zombie, false),
                Action::Bigger => backend.set_zombie_scale(zombie, rsvz_model::PositiveFiniteF32::new(2.0).unwrap()),
            };
            result.map_err(crate::access::operation_error)?;
            changed += 1;
        }
        Ok(changed)
    })
}
