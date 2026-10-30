//! Cross-frame object IDs.

/// Opaque world lifetime/mutation identity, scoped to one backend host process.
/// It is not a clock or a completed-round count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct WorldId(u64);
impl WorldId {
    #[doc(hidden)]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> u64 {
        self.0
    }
}

macro_rules! data_array_id {
    ($name:ident) => {
        /// A native pool ID that must be re-resolved before use.
        ///
        /// IDs can repeat after a world reset or restore. Callers retaining an ID
        /// across those boundaries must separately validate the world's identity.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(transparent)]
        pub struct $name(u32);

        impl $name {
            /// Decode a native ID for lookup in the current pool.
            #[doc(hidden)]
            #[inline(always)]
            pub const fn from_raw(raw: u32) -> Self {
                Self(raw)
            }

            /// 返回后端内部 object ID。
            #[doc(hidden)]
            #[inline(always)]
            pub const fn raw(self) -> u32 {
                self.0
            }

            /// 后端对象槽位索引。
            #[doc(hidden)]
            #[inline(always)]
            pub const fn index(self) -> u16 {
                (self.0 & 0xffff) as u16
            }

            /// 后端对象 generation/key。
            #[doc(hidden)]
            #[inline(always)]
            pub const fn generation(self) -> u16 {
                (self.0 >> 16) as u16
            }
        }
    };
}

data_array_id!(PlantId);
data_array_id!(ZombieId);
data_array_id!(ItemId);
data_array_id!(GridItemId);
data_array_id!(ProjectileId);
data_array_id!(LawnMowerId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_id_splits_generation_and_index() {
        let id = ZombieId::from_raw(0x03e9_0007);
        assert_eq!(id.index(), 7);
        assert_eq!(id.generation(), 1001);
        assert_eq!(id.raw(), 0x03e9_0007);
    }
    #[test]
    fn object_ids_keep_the_native_u32_layout() {
        for (size, alignment) in [
            (size_of::<PlantId>(), align_of::<PlantId>()),
            (size_of::<ZombieId>(), align_of::<ZombieId>()),
            (size_of::<ItemId>(), align_of::<ItemId>()),
            (size_of::<GridItemId>(), align_of::<GridItemId>()),
            (size_of::<ProjectileId>(), align_of::<ProjectileId>()),
            (size_of::<LawnMowerId>(), align_of::<LawnMowerId>()),
        ] {
            assert_eq!(size, size_of::<u32>());
            assert_eq!(alignment, align_of::<u32>());
        }
    }
}
