//! Grid item capabilities. Entity operations use current borrowed handles.

use crate::backend::Backend;
use rsvz_model::model::{Grid, GridItemId, GridItemKind};

/// Allocation occupancy, including dead objects awaiting native reclamation.
pub trait GridItemPoolBackend: Backend {
    fn grid_item_pool_active_count(&self) -> Result<u32, Self::Error>;
    fn grid_item_pool_capacity(&self) -> Result<u32, Self::Error>;
}

pub trait GridItemStateBackend: GridItemReadBackend {
    /// Whether the current mode uses the endless vase contents distribution.
    fn endless_vases_active(&self) -> Result<bool, Self::Error>;
    /// Native vase contents; backends without vases reject this optional kind.
    fn vase_content_kind<'a>(&'a self, item: Self::GridItemHandle<'a>) -> Result<i32, Self::Error>;
    fn vase_plant_kind<'a>(&'a self, item: Self::GridItemHandle<'a>) -> Result<i32, Self::Error>;
    fn vase_zombie_kind<'a>(&'a self, item: Self::GridItemHandle<'a>) -> Result<i32, Self::Error>;

    fn grid_item_countdown<'a>(&'a self, item: Self::GridItemHandle<'a>) -> i32;
}

/// Live grid-item read capability.
pub trait GridItemReadBackend: Backend {
    /// Grid-item handle valid for the current read lifetime.
    type GridItemHandle<'a>: Copy
    where
        Self: 'a;
    /// Live grid-item handle iterator.
    type GridItemIter<'a>: Iterator<Item = Self::GridItemHandle<'a>>
    where
        Self: 'a;

    /// Current readable live grid-item view.
    fn grid_items(&self) -> Result<Self::GridItemIter<'_>, Self::Error>;

    /// Reads grid-item ID from a validated handle.
    fn grid_item_id<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> GridItemId;
    /// Reads grid-item kind from a validated handle.
    fn grid_item_kind<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> Result<GridItemKind, Self::Error>;
    /// Reads the native grid-item row and column independently.
    fn grid_item_row<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> i32;
    fn grid_item_col<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> i32;
    /// Re-checks whether a validated handle is still live.
    fn grid_item_is_alive<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> bool;
}

/// Three distinct native grid-item creation atoms.
pub trait GridItemCreateBackend: GridItemEditBackend {
    fn add_ladder<'a>(&'a self, grid: Grid) -> Result<Self::GridItemHandle<'a>, Self::Error>;
    fn add_crater<'a>(&'a self, grid: Grid) -> Result<Self::GridItemHandle<'a>, Self::Error>;
    fn add_gravestone<'a>(&'a self, grid: Grid) -> Result<Self::GridItemHandle<'a>, Self::Error>;
}

/// Native challenge grave placement, including removal of existing plants at
/// the target, enabling graves and the native grave effect. This is deliberately
/// separate from allocating a plain gravestone entity.
pub trait ChallengeGravePlacementBackend: Backend {
    fn spawn_challenge_grave(&self, grid: Grid) -> Result<(), Self::Error>;
}

/// Explicit native rake placement without consuming the player's rake stock.
/// False means the native mode/terrain guards declined placement.
pub trait RakePlacementBackend: Backend {
    fn place_rake(&self, grid: Grid) -> Result<bool, Self::Error>;
}

/// Grid-item editing capability.
pub trait GridItemEditBackend: GridItemReadBackend {
    /// Removes a current-frame grid-item handle.
    fn remove_grid_item<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> Result<(), Self::Error>;
}
