use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::PortableBackend;

macro_rules! handle {
    ($name:ident, $raw:ty) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name<'a> {
            ptr: NonNull<$raw>,
            _borrow: PhantomData<&'a PortableBackend>,
        }

        impl<'a> $name<'a> {
            pub(crate) fn new(ptr: NonNull<$raw>) -> Self {
                Self {
                    ptr,
                    _borrow: PhantomData,
                }
            }

            pub(crate) const fn as_ptr(self) -> *const $raw {
                self.ptr.as_ptr()
            }

            pub(crate) fn borrowed(self) -> pvzp_rs::Borrowed<'a, $raw> {
                // SAFETY: this handle's backend borrow excludes slot reclamation and Board replacement.
                unsafe { pvzp_rs::Borrowed::from_non_null(self.ptr) }
            }
        }
    };
}

handle!(PortablePlantHandle, pvzp_rs::raw::pvzp_rs_plant);
handle!(PortableZombieHandle, pvzp_rs::raw::pvzp_rs_zombie);
handle!(PortableProjectileHandle, pvzp_rs::raw::pvzp_rs_projectile);
handle!(PortableGridItemHandle, pvzp_rs::raw::pvzp_rs_grid_item);
handle!(PortableItemHandle, pvzp_rs::raw::pvzp_rs_item);
handle!(PortableMowerHandle, pvzp_rs::raw::pvzp_rs_mower);
handle!(PortableSeedHandle, pvzp_rs::raw::pvzp_rs_seed);

impl PortablePlantHandle<'_> {
    pub(crate) const fn as_mut_ptr(self) -> *mut pvzp_rs::raw::pvzp_rs_plant {
        self.ptr.as_ptr()
    }
}

impl PortableZombieHandle<'_> {
    pub(crate) const fn as_mut_ptr(self) -> *mut pvzp_rs::raw::pvzp_rs_zombie {
        self.ptr.as_ptr()
    }
}

macro_rules! live_iterator {
    ($name:ident, $handle:ident, $method:ident, $is_live:expr) => {
        pub struct $name<'a> {
            backend: &'a PortableBackend,
            next: u32,
            limit: u32,
        }

        impl<'a> $name<'a> {
            pub(crate) const fn new(backend: &'a PortableBackend, limit: u32) -> Self {
                Self {
                    backend,
                    next: 0,
                    limit,
                }
            }
        }

        impl<'a> Iterator for $name<'a> {
            type Item = $handle<'a>;

            fn next(&mut self) -> Option<Self::Item> {
                if self.next >= self.limit {
                    return None;
                }
                // Like 1051 next_matching, resolve the pool once for this call,
                // then skip vacant slots under the existing backend borrow.
                let pool = self
                    .backend
                    .world()
                    .expect("iterator retains a live Board borrow")
                    .$method();
                while self.next < self.limit {
                    let index = self.next;
                    self.next += 1;
                    let ptr = pool
                        .get(index as i32)
                        .expect("validated iterator index and pool")
                        .map(pvzp_rs::Borrowed::as_non_null);
                    let Some(ptr) = ptr else {
                        continue;
                    };
                    let is_live = $is_live(self.backend, ptr);
                    if is_live {
                        return Some($handle::new(ptr));
                    }
                }
                None
            }
        }
    };
}

live_iterator!(
    PortableGridItemIter,
    PortableGridItemHandle,
    grid_items,
    |_, ptr: NonNull<pvzp_rs::raw::pvzp_rs_grid_item>| {
        // SAFETY: the iterator resolved an occupied slot under its backend borrow.
        !unsafe { std::ptr::addr_of!((*ptr.as_ptr()).mDead).read() }
    }
);
live_iterator!(PortableItemIter, PortableItemHandle, items, |_,
                                                             ptr: NonNull<
    pvzp_rs::raw::pvzp_rs_item,
>| {
    // SAFETY: the iterator resolved an occupied slot under its backend borrow.
    !unsafe { std::ptr::addr_of!((*ptr.as_ptr()).mDead).read() }
});

live_iterator!(
    PortableProjectileIter,
    PortableProjectileHandle,
    projectiles,
    |backend: &PortableBackend, ptr| {
        let handle = PortableProjectileHandle::new(ptr);
        // SAFETY: this handle names an occupied entry in the borrowed Board.
        !unsafe { std::ptr::addr_of!((*handle.as_ptr()).mDead).read() }
            && (rsvz_backend_api::ProjectileReadBackend::projectile_motion(backend, handle) != 9
                || rsvz_backend_api::ZombieReadBackend::zombie(
                    backend,
                    rsvz_model::ZombieId::from_raw(
                        rsvz_backend_api::ProjectileReadBackend::projectile_target_zombie_id(backend, handle),
                    ),
                )
                .expect("projectile iterator retains its Board")
                .is_some())
    }
);

live_iterator!(PortablePresentPlantIter, PortablePlantHandle, plants, |_,
                                                                       ptr: NonNull<
    pvzp_rs::raw::pvzp_rs_plant,
>| {
    // SAFETY: the pool returned an occupied slot retained by the backend borrow.
    !unsafe { std::ptr::addr_of!((*ptr.as_ptr()).mDead).read() }
});

live_iterator!(PortablePlantIter, PortablePlantHandle, plants, |_, ptr| {
    PortablePlantHandle::new(ptr).is_live()
});

live_iterator!(PortableZombieIter, PortableZombieHandle, zombies, |_, ptr| {
    PortableZombieHandle::new(ptr).is_live()
});

pub struct PortableSeedIter<'a> {
    backend: &'a PortableBackend,
    next: u32,
    count: u32,
}

impl<'a> PortableSeedIter<'a> {
    pub(crate) const fn new(backend: &'a PortableBackend, count: u32) -> Self {
        Self {
            backend,
            next: 0,
            count,
        }
    }
}

impl<'a> Iterator for PortableSeedIter<'a> {
    type Item = PortableSeedHandle<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.next < self.count {
            let index = self.next;
            self.next += 1;
            let world = self.backend.world().expect("seed iterator retains a live Board borrow");
            let seed = world.seed_at(index).expect("seed iterator retains its bounded bank");
            // SAFETY: the borrowed bank packet remains valid across this scalar read.
            if unsafe { std::ptr::addr_of!((*seed.as_ptr()).mPacketType).read() } == -1
                && world.has_conveyor().expect("seed iterator retains its live Board")
            {
                continue;
            }
            return Some(PortableSeedHandle::new(seed.as_non_null()));
        }
        None
    }
}

impl PortablePlantHandle<'_> {
    pub(crate) fn is_live(self) -> bool {
        let ptr = self.as_ptr();
        // SAFETY: handles name occupied slots for the current backend borrow.
        unsafe {
            !std::ptr::addr_of!((*ptr).mDead).read()
                && !std::ptr::addr_of!((*ptr).mSquished).read()
                && std::ptr::addr_of!((*ptr).mIsOnBoard).read()
                && std::ptr::addr_of!((*ptr).mOnBungeeState).read() != 2
                && !(std::ptr::addr_of!((*ptr).mSeedType).read() == 17 && std::ptr::addr_of!((*ptr).mState).read() == 7)
        }
    }
}

impl PortableZombieHandle<'_> {
    pub(crate) fn is_live(self) -> bool {
        let ptr = self.as_ptr();
        // SAFETY: handles name occupied slots for the current backend borrow.
        unsafe {
            !std::ptr::addr_of!((*ptr).mDead).read()
                && !matches!(std::ptr::addr_of!((*ptr).mZombiePhase).read(), 1..=3)
                && std::ptr::addr_of!((*ptr).mFromWave).read() >= 0
        }
    }
}

live_iterator!(
    PortablePresentZombieIter,
    PortableZombieHandle,
    zombies,
    |_, ptr: NonNull<pvzp_rs::raw::pvzp_rs_zombie>| {
        // SAFETY: occupied slot under the current Board borrow; no live-only filtering.
        !unsafe { std::ptr::addr_of!((*ptr.as_ptr()).mDead).read() }
    }
);

live_iterator!(PortableMowerIter, PortableMowerHandle, lawn_mowers, |_,
                                                                     ptr: NonNull<
    pvzp_rs::raw::pvzp_rs_mower,
>| {
    // SAFETY: the iterator resolved an occupied slot under its backend borrow.
    !unsafe { std::ptr::addr_of!((*ptr.as_ptr()).mDead).read() }
});
