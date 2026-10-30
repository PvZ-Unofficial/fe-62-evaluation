use super::{invalid_kind, validate_grid};
use crate::handles::{PortableGridItemHandle, PortableGridItemIter};
use crate::{PortableBackend, PortableBackendError, Result};
use rsvz_backend_api::backend::{
    GridItemCreateBackend, GridItemEditBackend, GridItemReadBackend, GridItemStateBackend,
};
use rsvz_model::model::{Grid, GridItemId, GridItemKind};

impl GridItemReadBackend for PortableBackend {
    type GridItemHandle<'a> = PortableGridItemHandle<'a>;
    type GridItemIter<'a> = PortableGridItemIter<'a>;

    fn grid_items(&self) -> Result<Self::GridItemIter<'_>> {
        Ok({
            let pool = self.world()?.grid_items();
            let limit = pool.max_used_count();
            crate::handles::PortableGridItemIter::new(self, limit)
        })
    }

    fn grid_item_id<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> GridItemId {
        // SAFETY: the occupied slot remains valid for this backend borrow, including same-frame death.
        GridItemId::from_raw(unsafe { pvzp_rs::raw::pvzp_rs_grid_item_id(handle.as_ptr()) })
    }

    fn grid_item_kind<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> Result<GridItemKind> {
        let raw = read_raw!(handle, mGridItemType);
        GridItemKind::try_from_code(raw).map_err(|_| invalid_kind("grid-item", raw))
    }

    fn grid_item_row<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> i32 {
        read_raw!(handle, mGridY)
    }
    fn grid_item_col<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> i32 {
        read_raw!(handle, mGridX)
    }

    fn grid_item_is_alive<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> bool {
        !read_raw!(handle, mDead)
    }
}

impl GridItemCreateBackend for PortableBackend {
    fn add_ladder<'a>(&'a self, grid: Grid) -> Result<PortableGridItemHandle<'a>> {
        validate_grid(self, grid)?;
        self.world()?
            .add_ladder(grid.col, grid.row)?
            .map(pvzp_rs::Borrowed::as_non_null)
            .map(PortableGridItemHandle::new)
            .ok_or(PortableBackendError::OperationRejected("add ladder"))
    }

    fn add_crater<'a>(&'a self, grid: Grid) -> Result<PortableGridItemHandle<'a>> {
        validate_grid(self, grid)?;
        self.world()?
            .add_crater(grid.col, grid.row)?
            .map(pvzp_rs::Borrowed::as_non_null)
            .map(PortableGridItemHandle::new)
            .ok_or(PortableBackendError::OperationRejected("add crater"))
    }

    fn add_gravestone<'a>(&'a self, grid: Grid) -> Result<PortableGridItemHandle<'a>> {
        validate_grid(self, grid)?;
        self.world()?
            .add_gravestone(grid.col, grid.row)?
            .map(pvzp_rs::Borrowed::as_non_null)
            .map(PortableGridItemHandle::new)
            .ok_or(PortableBackendError::OperationRejected("add gravestone"))
    }
}

impl rsvz_backend_api::ChallengeGravePlacementBackend for PortableBackend {
    fn spawn_challenge_grave(&self, grid: Grid) -> Result<()> {
        validate_grid(self, grid)?;
        self.world()?
            .spawn_challenge_grave(grid.row, grid.col)
            .map_err(Into::into)
    }
}

impl GridItemEditBackend for PortableBackend {
    fn remove_grid_item<'a>(&'a self, handle: Self::GridItemHandle<'a>) -> Result<()> {
        handle.borrowed().die().map_err(Into::into)
    }
}

impl rsvz_backend_api::RakePlacementBackend for PortableBackend {
    fn place_rake(&self, grid: Grid) -> Result<bool> {
        validate_grid(self, grid)?;
        self.world()?.place_rake(grid.row, grid.col).map_err(Into::into)
    }
}

impl GridItemStateBackend for PortableBackend {
    fn endless_vases_active(&self) -> Result<bool> {
        use rsvz_backend_api::NativeGameModeBackend;
        Ok(self.native_game_mode()? == 60)
    }

    fn vase_content_kind<'a>(&'a self, item: Self::GridItemHandle<'a>) -> Result<i32> {
        Ok(read_raw!(item, mScaryPotType) as i32)
    }
    fn vase_plant_kind<'a>(&'a self, item: Self::GridItemHandle<'a>) -> Result<i32> {
        Ok(read_raw!(item, mSeedType) as i32)
    }
    fn vase_zombie_kind<'a>(&'a self, item: Self::GridItemHandle<'a>) -> Result<i32> {
        Ok(read_raw!(item, mZombieType) as i32)
    }

    fn grid_item_countdown<'a>(&'a self, item: Self::GridItemHandle<'a>) -> i32 {
        read_raw!(item, mGridItemCounter)
    }
}
