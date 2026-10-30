//! Native grid-item creation; only successful objects receive an ID.
use rsvz_backend_api::{GridItemCreateBackend, GridItemReadBackend};
pub fn add_ladder(grid: rsvz_model::Grid) -> crate::runtime::RuntimeResult<rsvz_model::GridItemId>
where
    rsvz_current::CurrentBackend: GridItemCreateBackend,
{
    crate::access::with_backend(|backend| {
        backend
            .add_ladder(grid)
            .map(|handle| backend.grid_item_id(handle))
            .map_err(crate::access::operation_error)
    })
}
pub fn add_crater(grid: rsvz_model::Grid) -> crate::runtime::RuntimeResult<rsvz_model::GridItemId>
where
    rsvz_current::CurrentBackend: GridItemCreateBackend,
{
    crate::access::with_backend(|backend| {
        backend
            .add_crater(grid)
            .map(|handle| backend.grid_item_id(handle))
            .map_err(crate::access::operation_error)
    })
}
pub fn add_gravestone(grid: rsvz_model::Grid) -> crate::runtime::RuntimeResult<rsvz_model::GridItemId>
where
    rsvz_current::CurrentBackend: GridItemCreateBackend,
{
    crate::access::with_backend(|backend| {
        backend
            .add_gravestone(grid)
            .map(|handle| backend.grid_item_id(handle))
            .map_err(crate::access::operation_error)
    })
}
