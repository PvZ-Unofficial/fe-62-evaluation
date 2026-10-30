use crate::world::{out_bool, out_native_value, out_optional_ptr, out_value};
use crate::{Borrowed, Error, PlantRef, ReanimationRef, Result, raw};

impl<'a> Borrowed<'a, raw::pvzp_rs_seed> {
    pub fn can_pick_up(self) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: `seed` is borrowed from this world's fixed seed bank.
            unsafe { raw::pvzp_rs_seed_can_pick_up(self.as_ptr(), out) }
        })
    }

    pub fn was_planted(self) -> Result<()> {
        // SAFETY: `seed` is borrowed from this world's fixed seed bank.
        Error::from_status(unsafe { raw::pvzp_rs_seed_was_planted(self.as_ptr()) })
    }
}

impl<'a> Borrowed<'a, raw::pvzp_rs_plant> {
    pub fn setup_izombie(self) -> Result<()> {
        // SAFETY: borrow-bound plant; native setup changes existing animations/state only.
        Error::from_status(unsafe { raw::pvzp_rs_plant_setup_izombie(self.as_ptr()) })
    }
    pub fn shovel(self) -> Result<()> {
        // SAFETY: this live plant is borrow-bound to its Board; no pool relocation occurs.
        Error::from_status(unsafe { raw::pvzp_rs_plant_shovel(self.as_ptr()) })
    }
    pub fn die(self) -> Result<()> {
        // SAFETY: `plant` is borrow-bound to this world.
        Error::from_status(unsafe { raw::pvzp_rs_plant_die(self.as_ptr()) })
    }

    pub fn imitater_morph(self) -> Result<Option<PlantRef<'a>>> {
        out_optional_ptr(|out| {
            // SAFETY: plant belongs to this borrowed Board. Native morph does
            // not advance or replace the world and returns a new fixed-pool slot.
            unsafe { raw::pvzp_rs_plant_imitater_morph(self.as_ptr(), out) }
        })
        .map(|ptr| ptr.map(Borrowed::new))
    }

    pub fn set_sleeping(self, asleep: bool) -> Result<()> {
        // SAFETY: `plant` is borrow-bound and the boolean is copied.
        Error::from_status(unsafe { raw::pvzp_rs_plant_set_sleeping(self.as_ptr(), u8::from(asleep)) })
    }

    pub fn play_idle(self, fps: f32) -> Result<()> {
        // SAFETY: `plant` is borrow-bound and `fps` is copied.
        Error::from_status(unsafe { raw::pvzp_rs_plant_play_idle(self.as_ptr(), fps) })
    }

    pub fn update_reanim_color(self) -> Result<()> {
        // SAFETY: `plant` is borrow-bound to this world.
        Error::from_status(unsafe { raw::pvzp_rs_plant_update_reanim_color(self.as_ptr()) })
    }

    pub fn fire_cob(self, target_x: i32, target_y: i32) -> Result<()> {
        // SAFETY: `plant` is borrow-bound and target coordinates are copied.
        Error::from_status(unsafe { raw::pvzp_rs_plant_fire_cob(self.as_ptr(), target_x, target_y) })
    }

    pub fn hit_box(self) -> raw::pvzp_rs_rect_i32 {
        unsafe {
            // SAFETY: the live entity and validated weapon satisfy the native query,
            // which initializes all four output fields in a single call.
            out_native_value(|out| raw::pvzp_rs_plant_hit_box(self.as_ptr(), out))
        }
    }

    pub fn attack_rect(self, weapon: i32) -> Result<raw::pvzp_rs_rect_i32> {
        if !(0..=1).contains(&weapon) {
            return Err(Error::InvalidArgument { detail: weapon });
        }
        Ok(unsafe {
            // SAFETY: the live entity and validated weapon satisfy the native query,
            // which initializes all four output fields in a single call.
            out_native_value(|out| raw::pvzp_rs_plant_attack_rect(self.as_ptr(), weapon, out))
        })
    }

    pub fn damage_range_flags(self, weapon: i32) -> Result<u32> {
        if !(0..=1).contains(&weapon) {
            return Err(Error::InvalidArgument { detail: weapon });
        }
        // SAFETY: the plant is borrowed and the copied weapon discriminant was validated.
        Ok(unsafe { raw::pvzp_rs_plant_damage_range_flags(self.as_ptr(), weapon) })
    }

    pub fn reanimation(self) -> Result<Option<ReanimationRef<'a>>> {
        out_optional_ptr(|out| {
            // SAFETY: `plant` is borrow-bound and `out` is writable.
            unsafe { raw::pvzp_rs_plant_reanimation(self.as_ptr(), out) }
        })
        .map(|ptr| ptr.map(Borrowed::new))
    }
}

impl<'a> Borrowed<'a, raw::pvzp_rs_zombie> {
    pub fn pos_y_based_on_row(self, row: i32) -> Result<f32> {
        out_value(|out| {
            // SAFETY: `zombie` is borrow-bound and `out` is writable.
            unsafe { raw::pvzp_rs_zombie_pos_y_based_on_row(self.as_ptr(), row, out) }
        })
    }

    pub fn hit_box(self) -> Result<raw::pvzp_rs_rect_i32> {
        out_value(|out| {
            // SAFETY: `zombie` is borrow-bound and `out` is writable.
            unsafe { raw::pvzp_rs_zombie_hit_box(self.as_ptr(), out) }
        })
    }

    pub fn effected_by_damage(self, flags: u32) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: `zombie` is borrow-bound and `out` is writable.
            unsafe { raw::pvzp_rs_zombie_effected_by_damage(self.as_ptr(), flags, out) }
        })
    }

    pub fn can_target_plant(self, plant: PlantRef<'a>, attack_type: i32) -> Result<bool> {
        out_bool(|out| {
            // SAFETY: both objects are borrow-bound and `out` is writable.
            unsafe { raw::pvzp_rs_zombie_can_target_plant(self.as_ptr(), plant.as_ptr(), attack_type, out) }
        })
    }

    pub fn die_no_loot(self) -> Result<()> {
        // SAFETY: `zombie` is borrow-bound to this world.
        Error::from_status(unsafe { raw::pvzp_rs_zombie_die_no_loot(self.as_ptr()) })
    }

    pub fn is_dead_or_dying(self) -> bool {
        // SAFETY: the zombie borrow remains address-valid and the query only reads its fields.
        unsafe { raw::pvzp_rs_zombie_is_dead_or_dying(self.as_ptr()) != 0 }
    }

    /// Applies native loot-producing death without looking up the current Board again.
    pub fn die_with_loot(self) -> Result<()> {
        // SAFETY: this occupied zombie slot remains valid for its World borrow.
        Error::from_status(unsafe { raw::pvzp_rs_zombie_die_with_loot(self.as_ptr()) })
    }
    pub fn reanimation(self) -> Result<Option<ReanimationRef<'a>>> {
        out_optional_ptr(|out| {
            // SAFETY: `zombie` is borrow-bound and `out` is writable.
            unsafe { raw::pvzp_rs_zombie_reanimation(self.as_ptr(), out) }
        })
        .map(|ptr| ptr.map(Borrowed::new))
    }
}

impl<'a> Borrowed<'a, raw::pvzp_rs_projectile> {
    pub fn hit_box(self) -> raw::pvzp_rs_rect_i32 {
        unsafe {
            // SAFETY: the live entity and validated weapon satisfy the native query,
            // which initializes all four output fields in a single call.
            out_native_value(|out| raw::pvzp_rs_projectile_hit_box(self.as_ptr(), out))
        }
    }
}

impl<'a> Borrowed<'a, raw::pvzp_rs_grid_item> {
    pub fn die(self) -> Result<()> {
        // SAFETY: `item` is borrow-bound to this world.
        Error::from_status(unsafe { raw::pvzp_rs_grid_item_die(self.as_ptr()) })
    }
}

impl<'a> Borrowed<'a, raw::pvzp_rs_item> {
    pub fn collect(self, play_sound: bool) -> Result<()> {
        // SAFETY: `item` is borrow-bound and the boolean is copied.
        Error::from_status(unsafe { raw::pvzp_rs_item_collect(self.as_ptr(), u8::from(play_sound)) })
    }
}

impl<'a> Borrowed<'a, raw::pvzp_rs_mower> {
    pub fn start(self) -> Result<()> {
        // SAFETY: borrow-bound native mower; StartMower does not recycle slots.
        Error::from_status(unsafe { raw::pvzp_rs_mower_start(self.as_ptr()) })
    }
    pub fn die(self) -> Result<()> {
        // SAFETY: occupied slot remains allocated; native bonus replacement is
        // capacity-checked before Die and does not invalidate existing borrows.
        Error::from_status(unsafe { raw::pvzp_rs_mower_die(self.as_ptr()) })
    }
}
