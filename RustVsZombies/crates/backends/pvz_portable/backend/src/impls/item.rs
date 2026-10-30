use super::invalid_kind;
use crate::handles::{PortableItemHandle, PortableItemIter};
use crate::{PortableBackend, Result};
use rsvz_backend_api::backend::{AutoCollectBackend, ItemClickCollectBackend, ItemReadBackend};
use rsvz_model::model::{ItemId, ItemKind};

impl ItemClickCollectBackend for PortableBackend {
    fn click_collect_item<'a>(&'a self, item: Self::ItemHandle<'a>, play_sound: bool) -> Result<()> {
        item.borrowed().collect(play_sound).map_err(Into::into)
    }
}

impl AutoCollectBackend for PortableBackend {
    fn normal_auto_collect_enabled(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::normal_auto_collect_enabled())
    }
    fn set_normal_auto_collect_enabled(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_normal_auto_collect_enabled(enabled).map_err(Into::into)
    }
}

impl ItemReadBackend for PortableBackend {
    type ItemHandle<'a> = PortableItemHandle<'a>;
    type ItemIter<'a> = PortableItemIter<'a>;

    fn items(&self) -> Result<Self::ItemIter<'_>> {
        let pool = self.world()?.items();
        let limit = pool.max_used_count();
        Ok(PortableItemIter::new(self, limit))
    }

    fn item_id<'a>(&'a self, handle: Self::ItemHandle<'a>) -> ItemId {
        // SAFETY: the occupied slot remains valid for this backend borrow, including same-frame death.
        ItemId::from_raw(unsafe { pvzp_rs::raw::pvzp_rs_item_id(handle.as_ptr()) })
    }

    fn item_kind<'a>(&'a self, handle: Self::ItemHandle<'a>) -> Result<ItemKind> {
        let raw = read_raw!(handle, mType);
        ItemKind::from_raw(raw).ok_or_else(|| invalid_kind("item", raw))
    }

    fn item_x<'a>(&'a self, handle: Self::ItemHandle<'a>) -> f32 {
        read_raw!(handle, mPosX)
    }
    fn item_y<'a>(&'a self, handle: Self::ItemHandle<'a>) -> f32 {
        read_raw!(handle, mPosY)
    }

    fn item_being_collected<'a>(&'a self, handle: Self::ItemHandle<'a>) -> bool {
        read_raw!(handle, mIsBeingCollected)
    }
}
