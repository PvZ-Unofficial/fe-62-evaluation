//! Board-wide cleanup helpers built from fine-grained edit capabilities.

use crate::backend::{PlantReadBackend, PlantRemoveBackend, ZombieKillBackend, ZombieReadBackend, ZombieRemoveBackend};

/// PT's clear-plants action includes all occupied not-dead slots, even crushed
/// or off-board plants. It retains native Die side effects and pool identities.
pub fn clear_present_plants() -> crate::runtime::RuntimeResult<usize>
where
    rsvz_current::CurrentBackend: crate::backend::PlantPresenceBackend + PlantRemoveBackend,
{
    use crate::backend::PlantPresenceBackend;
    crate::access::with_backend(|backend| {
        let mut removed = 0;
        for plant in backend.present_plants().map_err(crate::access::operation_error)? {
            backend.remove_plant(plant).map_err(crate::access::operation_error)?;
            removed += 1;
        }
        Ok(removed)
    })
}

/// Removes one category of existing grid items through their native Die action.
pub fn clear_grid_items(kind: rsvz_model::GridItemKind) -> crate::runtime::RuntimeResult<usize>
where
    rsvz_current::CurrentBackend: crate::backend::GridItemEditBackend,
{
    use crate::backend::{GridItemEditBackend, GridItemReadBackend};
    crate::access::with_backend(|backend| {
        let mut removed = 0;
        for item in backend.grid_items().map_err(crate::access::operation_error)? {
            if backend.grid_item_kind(item).map_err(crate::access::operation_error)? == kind {
                backend.remove_grid_item(item).map_err(crate::access::operation_error)?;
                removed += 1;
            }
        }
        Ok(removed)
    })
}

/// Removes all currently live plants through the P10 atom.
pub fn clear_plants() -> usize
where
    rsvz_current::CurrentBackend: PlantRemoveBackend,
{
    crate::access::with_backend(|backend| {
        let mut removed = 0;
        for plant in crate::live_value::read_or_abort(backend.plants(), "plants") {
            crate::live_value::read_or_abort(backend.remove_plant(plant), "remove_plant");
            removed += 1;
        }
        removed
    })
}

/// Removes all currently live zombies without loot through Z11.
pub fn clear_zombies() -> usize
where
    rsvz_current::CurrentBackend: ZombieRemoveBackend,
{
    crate::access::with_backend(|backend| {
        let mut removed = 0;
        for zombie in crate::live_value::read_or_abort(backend.zombies(), "zombies") {
            crate::live_value::read_or_abort(backend.remove_zombie(zombie), "remove_zombie");
            removed += 1;
        }
        removed
    })
}

/// Kills all currently live zombies through Z12 when the backend supports loot semantics.
pub fn kill_all_zombies() -> usize
where
    rsvz_current::CurrentBackend: ZombieKillBackend,
{
    crate::access::with_backend(|backend| {
        let mut killed = 0;
        for zombie in crate::live_value::read_or_abort(backend.zombies(), "zombies") {
            crate::live_value::read_or_abort(backend.kill_zombie(zombie), "kill_zombie");
            killed += 1;
        }
        killed
    })
}

/// Removes the live grid item with this ID; a missing ID is a successful no-op.
pub fn remove_grid_item_by_id(id: rsvz_model::GridItemId)
where
    rsvz_current::CurrentBackend: crate::backend::GridItemEditBackend,
{
    crate::frame::with_frame(|frame| {
        if let Some(item) = frame.grid_items().find(|item| item.id() == id) {
            item.remove();
        }
    });
}

/// Clears only existing ice trails; other objects and future ice creation are unchanged.
pub fn clear_ice_trails() -> crate::runtime::RuntimeResult<()>
where
    rsvz_current::CurrentBackend: rsvz_backend_api::IceTrailClearBackend,
{
    use rsvz_backend_api::IceTrailClearBackend;
    crate::access::with_backend(|b| b.clear_ice_trails()).map_err(crate::access::operation_error)
}
