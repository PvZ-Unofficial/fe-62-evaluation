//! Native plant-pool relocation. Plant is a 0x14c-byte scalar/resource-ID record;
//! its last dword is the DataArray ID, not a C++ ownership-bearing member.
use super::{DATA_ARRAY_INDEX_MASK, DATA_ARRAY_KEY_MASK, DataArray, Plant};
use crate::error::{Pvz1051Error, Result};
use std::{
    mem::{MaybeUninit, size_of},
    ptr,
};

impl DataArray<Plant> {
    unsafe fn plant_id_ptr(this: *mut Self, index: u32) -> *mut u32 {
        // SAFETY: caller bounds index against this live pool's capacity.
        unsafe {
            Self::item_at(this, index)
                .cast::<u8>()
                .add(size_of::<Plant>() - 4)
                .cast()
        }
    }
    pub(crate) unsafe fn plant_slot_id(this: *const Self, index: u32) -> Result<Option<u32>> {
        unsafe {
            if this.is_null() || (*this).block.is_null() || index >= (*this).max_size {
                return Err(Pvz1051Error::OperationRejected("invalid plant slot"));
            }
            Ok(Self::occupied_item_at(this, index).map(|plant| Self::item_id(plant.as_ptr())))
        }
    }
    pub(crate) unsafe fn relocate_plant(
        this: *mut Self, source_id: u32, target: u32, expected_target: u32, commit: bool,
    ) -> Result<(u32, u32)> {
        // SAFETY: caller owns exclusive native access; all indices, occupancy and
        // free-list links are checked before the first mutation below.
        unsafe {
            if this.is_null()
                || (*this).block.is_null()
                || (*this).max_size == 0
                || (*this).max_size >= 65536
                || (*this).max_used_count > (*this).max_size
                || (*this).size > (*this).max_used_count
                || (*this).free_list_head > (*this).max_used_count
                || (*this).next_key == 0
                || (*this).next_key >= 65536
            {
                return Err(Pvz1051Error::OperationRejected("invalid plant pool header"));
            }
            let source =
                Self::try_to_get(this, source_id).ok_or(Pvz1051Error::OperationRejected("plant identity changed"))?;
            if Plant::is_dead(source.as_ptr()) || target >= (*this).max_size {
                return Err(Pvz1051Error::OperationRejected("source retired or target out of range"));
            }
            let source_slot = source_id & DATA_ARRAY_INDEX_MASK;
            let target_id = Self::plant_slot_id(this, target)?.unwrap_or(0);
            if source_slot == target {
                if expected_target != 0 && expected_target != source_id {
                    return Err(Pvz1051Error::OperationRejected("target identity changed"));
                }
                return Ok((source_id, 0));
            }
            if target_id != expected_target {
                return Err(Pvz1051Error::OperationRejected("target occupancy changed"));
            }
            let old_max = (*this).max_used_count;
            let (mut previous, mut target_previous, mut free_count) = (old_max, old_max, 0);
            let mut found = false;
            if target_id == 0 {
                let mut index = (*this).free_list_head;
                while index != old_max {
                    if index >= old_max {
                        return Err(Pvz1051Error::OperationRejected("invalid plant free list"));
                    }
                    let id = Self::id_at(this, index);
                    free_count += 1;
                    if id & DATA_ARRAY_KEY_MASK != 0 || free_count > old_max {
                        return Err(Pvz1051Error::OperationRejected("invalid plant free list"));
                    }
                    if index == target {
                        found = true;
                        target_previous = previous;
                    }
                    previous = index;
                    index = id;
                }
                if free_count != old_max - (*this).size || (target < old_max && !found) {
                    return Err(Pvz1051Error::OperationRejected("incomplete plant free list"));
                }
            }
            if !commit {
                return Ok((source_id, target_id));
            }
            let fresh = |index: u32, old: u32| {
                loop {
                    let id = ((*this).next_key << 16) | index;
                    (*this).next_key += 1;
                    if (*this).next_key == 65536 {
                        (*this).next_key = 1;
                    }
                    if id != old {
                        break id;
                    }
                }
            };
            let moved = fresh(target, target_id);
            let swapped = if target_id != 0 {
                fresh(source_slot, source_id)
            } else {
                0
            };
            let destination = Self::item_at(this, target);
            if target_id != 0 {
                // MaybeUninit permits copying opaque native padding without
                // asserting Rust initialization of every payload byte.
                ptr::swap_nonoverlapping(
                    source.as_ptr().cast::<MaybeUninit<Plant>>(),
                    destination.cast::<MaybeUninit<Plant>>(),
                    1,
                );
                ptr::write_unaligned(Self::plant_id_ptr(this, source_slot), swapped);
            } else {
                if target < old_max {
                    let next = Self::id_at(this, target);
                    if target_previous == old_max {
                        (*this).free_list_head = next;
                    } else {
                        ptr::write_unaligned(Self::plant_id_ptr(this, target_previous), next);
                    }
                } else {
                    if target == old_max {
                        if previous == old_max {
                            (*this).free_list_head = target + 1;
                        } else {
                            ptr::write_unaligned(Self::plant_id_ptr(this, previous), target + 1);
                        }
                    } else {
                        for index in old_max..target {
                            ptr::write_unaligned(Self::plant_id_ptr(this, index), index + 1);
                        }
                        ptr::write_unaligned(Self::plant_id_ptr(this, target - 1), target + 1);
                    }
                    (*this).max_used_count = target + 1;
                }
                ptr::copy_nonoverlapping(
                    source.as_ptr().cast::<MaybeUninit<Plant>>(),
                    destination.cast::<MaybeUninit<Plant>>(),
                    1,
                );
                ptr::write_unaligned(Self::plant_id_ptr(this, source_slot), (*this).free_list_head);
                (*this).free_list_head = source_slot;
            }
            ptr::write_unaligned(Self::plant_id_ptr(this, target), moved);
            Ok((moved, swapped))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Pool {
        items: Vec<Plant>,
        header: DataArray<Plant>,
    }
    impl Pool {
        fn new() -> Self {
            let mut items: Vec<Plant> = (0..8).map(|_| unsafe { std::mem::zeroed() }).collect();
            let header = DataArray {
                block: items.as_mut_ptr(),
                max_used_count: 5,
                max_size: 8,
                free_list_head: 1,
                size: 3,
                next_key: 2000,
                name: std::ptr::null(),
            };
            let mut pool = Self { items, header };
            for index in 0..5 {
                unsafe {
                    ptr::write_unaligned(
                        DataArray::plant_id_ptr(&mut pool.header, index),
                        if index % 2 == 0 {
                            ((1001 + index) << 16) | index
                        } else {
                            index + 2
                        },
                    );
                }
            }
            pool
        }
        fn id(&self, index: u32) -> u32 {
            unsafe { DataArray::id_at(&self.header, index) }
        }
        fn payload(&self, index: usize) -> Vec<u8> {
            unsafe {
                std::slice::from_raw_parts((&self.items[index] as *const Plant).cast(), size_of::<Plant>() - 4).to_vec()
            }
        }
        fn state(&self) -> (u32, u32, u32, u32, Vec<u8>) {
            (
                self.header.max_used_count,
                self.header.free_list_head,
                self.header.size,
                self.header.next_key,
                unsafe {
                    std::slice::from_raw_parts(self.items.as_ptr().cast(), self.items.len() * size_of::<Plant>())
                        .to_vec()
                },
            )
        }
        fn check(&self) {
            let mut seen = [false; 8];
            let mut index = self.header.free_list_head;
            let mut free = 0;
            while index != self.header.max_used_count {
                assert!(index < self.header.max_used_count);
                assert!(!seen[index as usize]);
                seen[index as usize] = true;
                assert_eq!(self.id(index) & DATA_ARRAY_KEY_MASK, 0);
                free += 1;
                index = self.id(index);
            }
            assert_eq!(free, self.header.max_used_count - self.header.size);
            for index in 0..self.header.max_used_count {
                assert_eq!(seen[index as usize], self.id(index) & DATA_ARRAY_KEY_MASK == 0);
            }
        }
    }
    #[test]
    fn moves_and_swaps_preserve_payload_allocator_and_fresh_identities() {
        let mut pool = Pool::new();
        pool.check();
        unsafe {
            ptr::write_unaligned(
                (&mut pool.items[0] as *mut Plant).cast::<u8>().add(0x40).cast::<i32>(),
                777,
            );
        }
        let original = pool.id(0);
        let payload = pool.payload(0);
        let (one, _) = unsafe { DataArray::relocate_plant(&mut pool.header, original, 1, 0, true) }.unwrap();
        pool.check();
        assert_eq!(pool.payload(1), payload);
        assert_ne!(one, original);
        let displaced = pool.id(2);
        let (two, other) = unsafe { DataArray::relocate_plant(&mut pool.header, one, 2, displaced, true) }.unwrap();
        pool.check();
        assert_eq!(pool.payload(2), payload);
        assert_ne!(other, displaced);
        assert!(unsafe { DataArray::try_to_get(&pool.header, one) }.is_none());
        let (high, _) = unsafe { DataArray::relocate_plant(&mut pool.header, two, 7, 0, true) }.unwrap();
        pool.check();
        assert_eq!(pool.header.max_used_count, 8);
        assert_eq!(pool.header.size, 3);
        assert_eq!(pool.payload(7), payload);
        let (low, _) = unsafe { DataArray::relocate_plant(&mut pool.header, high, 5, 0, true) }.unwrap();
        pool.check();
        let (back, _) = unsafe { DataArray::relocate_plant(&mut pool.header, low, 7, 0, true) }.unwrap();
        pool.check();
        assert_ne!(back, high);
        assert!(unsafe { DataArray::try_to_get(&pool.header, high) }.is_none());
        assert_eq!(pool.payload(7), payload);
    }
    #[test]
    fn preflight_rejects_stale_targets_and_corrupt_free_lists_without_writes() {
        let mut pool = Pool::new();
        let source = pool.id(0);
        let before = pool.state();
        assert!(unsafe { DataArray::relocate_plant(&mut pool.header, source, 2, 0, true) }.is_err());
        assert_eq!(pool.state(), before);
        unsafe {
            ptr::write_unaligned(DataArray::plant_id_ptr(&mut pool.header, 1), 1);
        }
        let before = pool.state();
        assert!(unsafe { DataArray::relocate_plant(&mut pool.header, source, 7, 0, true) }.is_err());
        assert_eq!(pool.state(), before);
    }
    #[test]
    fn native_generation_wrap_skips_immediate_identity_reuse() {
        let mut pool = Pool::new();
        pool.header.next_key = 65535;
        let source = pool.id(0);
        let target = pool.id(2);
        let (moved, swapped) = unsafe { DataArray::relocate_plant(&mut pool.header, source, 2, target, true) }.unwrap();
        assert_eq!(moved >> 16, 65535);
        assert_eq!(swapped >> 16, 1);
        assert_eq!(pool.header.next_key, 2);
        pool.check();
        pool.header.next_key = swapped >> 16;
        let (moved_back, _) = unsafe { DataArray::relocate_plant(&mut pool.header, moved, 0, swapped, true) }.unwrap();
        assert_ne!(moved_back, swapped);
        pool.check();
    }
}
