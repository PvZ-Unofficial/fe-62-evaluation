use super::{card_parts, contact_rect, invalid_kind, validate_grid};
use crate::handles::{PortablePlantHandle, PortablePlantIter};
use crate::{PortableBackend, PortableBackendError, Result};
use rsvz_backend_api::backend::{
    CobFireBackend, ImitatorMorphBackend, PlantContactBackend, PlantCostBackend, PlantCreateBackend,
    PlantDamageRuleEditBackend, PlantEffectCountdownWriteBackend, PlantEffectRuleEditBackend, PlantHealthWriteBackend,
    PlantIdleAnimationBackend, PlantPlacementBackend, PlantPoolBackend, PlantReadBackend, PlantRemoveBackend,
    PlantSleepBackend, PlantStateBackend, PlantStateCountdownWriteBackend, PlantStateWriteBackend,
    PlantVisualStateBackend, PlantingRuleEditBackend,
};
use rsvz_model::model::{
    CheckedCardSelection, ContactRect, DamageRangeFlags, Grid, NonNegativeI32, ObjectEditOutcome, PixelPos,
    PlantDamageRule, PlantId, PlantKind, PlantWeapon, Plantability, PositiveFiniteF32, PositiveHp, SunAmount,
};

impl PlantReadBackend for PortableBackend {
    type PlantHandle<'a> = PortablePlantHandle<'a>;
    type PlantIter<'a> = PortablePlantIter<'a>;

    fn plants(&self) -> Result<Self::PlantIter<'_>> {
        Ok({
            let pool = self.world()?.plants();
            let limit = pool.max_used_count();
            crate::handles::PortablePlantIter::new(self, limit)
        })
    }

    fn plant(&self, id: PlantId) -> Result<Option<Self::PlantHandle<'_>>> {
        let world = self.world()?;
        Ok(world
            .plants()
            .try_to_get(id.raw())
            .map(pvzp_rs::Borrowed::as_non_null)
            .map(PortablePlantHandle::new)
            .filter(|handle| handle.is_live()))
    }

    fn plant_id<'a>(&'a self, handle: Self::PlantHandle<'a>) -> PlantId {
        // SAFETY: the occupied slot remains valid for this backend borrow, including same-frame death.
        PlantId::from_raw(unsafe { pvzp_rs::raw::pvzp_rs_plant_id(handle.as_ptr()) })
    }

    fn plant_kind<'a>(&'a self, handle: Self::PlantHandle<'a>) -> Result<PlantKind> {
        let packet = read_raw!(handle, mSeedType);
        let effective = if packet == PlantKind::Imitator.code() {
            read_raw!(handle, mImitaterType)
        } else {
            packet
        };
        PlantKind::try_from_code(effective).map_err(|_| invalid_kind("plant", effective))
    }

    fn plant_raw_kind<'a>(&'a self, handle: Self::PlantHandle<'a>) -> Result<PlantKind> {
        let raw = read_raw!(handle, mSeedType);
        PlantKind::try_from_code(raw).map_err(|_| invalid_kind("plant", raw))
    }

    fn plant_x<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, _base.mX)
    }

    fn plant_state<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, mState)
    }

    fn plant_is_squished<'a>(&'a self, handle: Self::PlantHandle<'a>) -> bool {
        read_raw!(handle, mSquished)
    }

    fn plant_on_bungee_state<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, mOnBungeeState)
    }

    fn plant_is_sleeping<'a>(&'a self, handle: Self::PlantHandle<'a>) -> bool {
        read_raw!(handle, mIsAsleep)
    }

    fn plant_wake_up_counter<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, mWakeUpCounter)
    }

    fn plant_state_countdown<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, mStateCountdown)
    }

    fn plant_effect_countdown<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, mDoSpecialCountdown)
    }

    fn plant_disappear_countdown<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, mDisappearCountdown)
    }

    fn plant_reanim_circulation<'a>(&'a self, handle: Self::PlantHandle<'a>) -> Result<Option<f32>> {
        let plant = handle.borrowed();
        let Some(reanimation) = plant.reanimation()? else {
            return Ok(None);
        };
        // SAFETY: native lookup resolved a live body reanimation for this immediate scalar read.
        Ok(Some(unsafe {
            std::ptr::addr_of!((*reanimation.as_ptr()).mAnimTime).read()
        }))
    }

    fn plant_eating_flash_counter<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, mEatenFlashCountdown)
    }

    fn plant_hp<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, mPlantHealth)
    }

    fn plant_row<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, _base.mRow)
    }
    fn plant_col<'a>(&'a self, handle: Self::PlantHandle<'a>) -> i32 {
        read_raw!(handle, mPlantCol)
    }

    fn plant_is_alive<'a>(&'a self, handle: Self::PlantHandle<'a>) -> bool {
        handle.is_live()
    }
}

impl ImitatorMorphBackend for PortableBackend {
    fn imitator_morph_successor(&self, placeholder: PlantId) -> Result<Option<PlantId>> {
        Ok({
            self.world()?
                .imitater_successor(placeholder.raw())
                .map(PlantId::from_raw)
        })
    }
}

impl PlantCostBackend for PortableBackend {
    fn current_plant_cost(&self, selection: CheckedCardSelection) -> Result<SunAmount> {
        let (packet, imitater) = card_parts(selection);
        let cost = self.world()?.current_plant_cost(packet, imitater)?;
        let cost = u32::try_from(cost).map_err(|_| PortableBackendError::NumericOutOfRange("plant cost"))?;
        SunAmount::new(cost).map_err(|_| PortableBackendError::NumericOutOfRange("plant cost"))
    }
}

impl PlantPlacementBackend for PortableBackend {
    fn is_column_challenge(&self) -> Result<bool> {
        self.world()?.is_column_challenge().map_err(Into::into)
    }
    fn is_wallnut_bowling(&self) -> Result<bool> {
        self.world()?.is_wallnut_bowling().map_err(Into::into)
    }
    fn can_plant_at(&self, selection: CheckedCardSelection, grid: Grid) -> Result<Plantability> {
        validate_grid(self, grid)?;
        let reason = self
            .world()?
            .can_plant_at(selection.effective_kind().code(), grid.col, grid.row)?;
        Ok(Plantability::from_game_rule_code(reason))
    }
}

impl PlantPoolBackend for PortableBackend {
    fn plant_pool_next_slot(&self) -> Result<u32> {
        Ok(self.world()?.plants().free_list_head())
    }

    fn plant_pool_active_count(&self) -> Result<u32> {
        Ok(self.world()?.plants().active_count())
    }

    fn plant_pool_capacity(&self) -> Result<u32> {
        Ok(self.world()?.plants().capacity())
    }
}

impl PlantCreateBackend for PortableBackend {
    fn morph_imitator<'a>(&'a self, placeholder: PortablePlantHandle<'a>) -> Result<PortablePlantHandle<'a>> {
        placeholder
            .borrowed()
            .imitater_morph()?
            .map(pvzp_rs::Borrowed::as_non_null)
            .map(PortablePlantHandle::new)
            .ok_or(PortableBackendError::OperationRejected("imitater morph"))
    }
    fn new_plant<'a>(&'a self, selection: CheckedCardSelection, grid: Grid) -> Result<PortablePlantHandle<'a>> {
        validate_grid(self, grid)?;
        let (packet, imitater) = card_parts(selection);
        self.world()?
            .new_plant(grid.col, grid.row, packet, imitater)?
            .map(pvzp_rs::Borrowed::as_non_null)
            .map(PortablePlantHandle::new)
            .ok_or(PortableBackendError::OperationRejected("new plant"))
    }

    fn add_plant<'a>(&'a self, selection: CheckedCardSelection, grid: Grid) -> Result<PortablePlantHandle<'a>> {
        validate_grid(self, grid)?;
        let (packet, imitater) = card_parts(selection);
        self.world()?
            .add_plant(grid.col, grid.row, packet, imitater)?
            .map(pvzp_rs::Borrowed::as_non_null)
            .map(PortablePlantHandle::new)
            .ok_or(PortableBackendError::OperationRejected("add plant"))
    }
}

impl PlantRemoveBackend for PortableBackend {
    fn shovel_plant<'a>(&'a self, handle: Self::PlantHandle<'a>) -> Result<()> {
        handle.borrowed().shovel().map_err(Into::into)
    }
    fn remove_plant<'a>(&'a self, handle: Self::PlantHandle<'a>) -> Result<()> {
        handle.borrowed().die().map_err(Into::into)
    }
}

impl PlantSleepBackend for PortableBackend {
    fn set_plant_sleeping<'a>(&'a self, handle: Self::PlantHandle<'a>, asleep: bool) -> Result<()> {
        handle.borrowed().set_sleeping(asleep).map_err(Into::into)
    }

    fn set_plant_wake_up_counter<'a>(&'a self, handle: Self::PlantHandle<'a>, counter: i32) -> Result<()> {
        write_raw!(handle, counter, mWakeUpCounter);
        Ok(())
    }
}

impl PlantIdleAnimationBackend for PortableBackend {
    fn play_plant_idle_animation<'a>(&'a self, handle: Self::PlantHandle<'a>, fps: PositiveFiniteF32) -> Result<()> {
        handle.borrowed().play_idle(fps.get()).map_err(Into::into)
    }
}

impl PlantStateCountdownWriteBackend for PortableBackend {
    fn set_plant_state_countdown<'a>(&'a self, handle: Self::PlantHandle<'a>, countdown: i32) -> Result<()> {
        write_raw!(handle, countdown, mStateCountdown);
        Ok(())
    }
}

impl PlantStateWriteBackend for PortableBackend {
    fn set_plant_state<'a>(&'a self, handle: Self::PlantHandle<'a>, state: i32) -> Result<()> {
        write_raw!(handle, state, mState);
        Ok(())
    }
}

impl PlantVisualStateBackend for PortableBackend {
    fn set_plant_eating_flash_counter<'a>(&'a self, plant: Self::PlantHandle<'a>, value: i32) -> Result<()> {
        write_raw!(plant, value, mEatenFlashCountdown);
        Ok(())
    }

    fn update_plant_reanim_color<'a>(&'a self, plant: Self::PlantHandle<'a>) -> Result<()> {
        plant.borrowed().update_reanim_color().map_err(Into::into)
    }
}

impl CobFireBackend for PortableBackend {
    fn fire_cob<'a>(&'a self, cob: Self::PlantHandle<'a>, target: PixelPos) -> Result<()> {
        if self.plant_kind(cob)? != PlantKind::CobCannon {
            return Err(PortableBackendError::OperationRejected("plant is not CobCannon"));
        }
        cob.borrowed().fire_cob(target.x, target.y).map_err(Into::into)
    }
}

impl PlantEffectRuleEditBackend for PortableBackend {
    fn plant_special_countdown_rule(&self) -> Result<rsvz_model::PlantSpecialCountdownRule> {
        use rsvz_model::PlantSpecialCountdownRule::*;
        match pvzp_rs::modifier::plant_special_countdown_rule() {
            0 => Ok(Normal),
            1 => Ok(Immediate),
            2 => Ok(Inverted),
            value => Err(super::invalid_kind("plant special countdown rule", value)),
        }
    }
    fn set_plant_special_countdown_rule(&self, rule: rsvz_model::PlantSpecialCountdownRule) -> Result<()> {
        use rsvz_model::PlantSpecialCountdownRule::*;
        pvzp_rs::modifier::set_plant_special_countdown_rule(match rule {
            Normal => 0,
            Immediate => 1,
            Inverted => 2,
        })
        .map_err(Into::into)
    }
    fn instant_ice_and_ash_effects(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::instant_ice_and_ash_effects())
    }

    fn set_instant_ice_and_ash_effects(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_instant_ice_and_ash_effects(enabled).map_err(Into::into)
    }

    fn mushrooms_awake(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::mushrooms_awake())
    }

    fn set_mushrooms_awake(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_mushrooms_awake(enabled).map_err(Into::into)
    }
}

impl PlantingRuleEditBackend for PortableBackend {
    fn easy_planting_cheat(&self) -> Result<bool> {
        pvzp_rs::modifier::easy_planting_cheat().map_err(Into::into)
    }

    fn set_easy_planting_cheat(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_easy_planting_cheat(enabled).map_err(Into::into)
    }

    fn planting_restrictions_ignored(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::planting_restrictions_ignored())
    }

    fn set_planting_restrictions_ignored(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_planting_restrictions_ignored(enabled).map_err(Into::into)
    }
}

impl PlantDamageRuleEditBackend for PortableBackend {
    fn plant_damage_rule(&self) -> Result<PlantDamageRule> {
        match pvzp_rs::modifier::plant_damage_rule()? {
            0 => Ok(PlantDamageRule::Normal),
            1 => Ok(PlantDamageRule::Invincible),
            2 => Ok(PlantDamageRule::Weak),
            value => Err(invalid_kind("plant damage rule", value)),
        }
    }

    fn set_plant_damage_rule(&self, rule: PlantDamageRule) -> Result<()> {
        let raw = match rule {
            PlantDamageRule::Normal => 0,
            PlantDamageRule::Invincible => 1,
            PlantDamageRule::Weak => 2,
        };
        pvzp_rs::modifier::set_plant_damage_rule(raw).map_err(Into::into)
    }
}

impl PlantHealthWriteBackend for PortableBackend {
    fn set_plant_hp<'a>(&'a self, handle: Self::PlantHandle<'a>, hp: PositiveHp) -> Result<ObjectEditOutcome> {
        write_raw!(handle, hp.get(), mPlantHealth);
        Ok(ObjectEditOutcome::Applied)
    }
}

impl PlantEffectCountdownWriteBackend for PortableBackend {
    fn set_plant_effect_countdown<'a>(
        &'a self, handle: Self::PlantHandle<'a>, target_countdown: NonNegativeI32,
    ) -> Result<ObjectEditOutcome> {
        write_raw!(handle, target_countdown.get(), mDoSpecialCountdown);
        Ok(ObjectEditOutcome::Applied)
    }
}

impl PlantContactBackend for PortableBackend {
    fn plant_hit_box<'a>(&'a self, plant: Self::PlantHandle<'a>) -> Result<ContactRect> {
        let plant = plant.borrowed();
        contact_rect(plant.hit_box())
    }

    fn plant_attack_rect<'a>(&'a self, plant: Self::PlantHandle<'a>, weapon: PlantWeapon) -> Result<ContactRect> {
        let plant = plant.borrowed();
        contact_rect(plant.attack_rect(weapon as i32)?)
    }

    fn plant_damage_range_flags<'a>(&'a self, plant: Self::PlantHandle<'a>, weapon: PlantWeapon) -> DamageRangeFlags {
        // SAFETY: the live handle and typed weapon satisfy the scalar atom's preconditions.
        let bits = unsafe { pvzp_rs::raw::pvzp_rs_plant_damage_range_flags(plant.as_mut_ptr(), weapon as i32) };
        DamageRangeFlags::from_bits_retain(bits)
    }
}

impl PlantStateBackend for PortableBackend {
    fn plant_y<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, _base.mY)
    }

    fn plant_width<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, _base.mWidth)
    }
    fn plant_height<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, _base.mHeight)
    }

    fn plant_max_hp<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, mPlantMaxHealth)
    }

    fn plant_target_x<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, mTargetX)
    }
    fn plant_target_y<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, mTargetY)
    }

    fn plant_target_object<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, mTargetZombieID) as i32
    }

    fn plant_direction<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        if read_raw!(plant, mSeedType) == 17
            && (3..=7).contains(&read_raw!(plant, mState))
            && read_raw!(plant, mTargetX) < read_raw!(plant, _base.mX)
        {
            1
        } else {
            -1
        }
    }

    fn plant_collision<'a>(&'a self, plant: Self::PlantHandle<'a>) -> Result<ContactRect> {
        self.plant_hit_box(plant)
    }

    fn plant_imitater_kind<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, mImitaterType)
    }

    fn plant_recently_eaten_countdown<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, mRecentlyEatenCountdown)
    }
    fn plant_launch_counter<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, mLaunchCounter)
    }
    fn plant_shooting_counter<'a>(&'a self, plant: Self::PlantHandle<'a>) -> i32 {
        read_raw!(plant, mShootingCounter)
    }
}

impl rsvz_backend_api::PlantPresenceBackend for PortableBackend {
    type PresentPlantIter<'a> = crate::handles::PortablePresentPlantIter<'a>;
    fn present_plants(&self) -> Result<Self::PresentPlantIter<'_>> {
        let limit = self.world()?.plants().max_used_count();
        Ok(crate::handles::PortablePresentPlantIter::new(self, limit))
    }
}

impl rsvz_backend_api::SeedRenderRulesBackend for PortableBackend {
    fn seed_bank_topmost(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::seed_bank_topmost())
    }
    fn set_seed_bank_topmost(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_seed_bank_topmost(enabled).map_err(Into::into)
    }
    fn seed_recharge_shading_hidden(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::seed_recharge_shading_hidden())
    }
    fn set_seed_recharge_shading_hidden(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_seed_recharge_shading_hidden(enabled).map_err(Into::into)
    }
}

impl rsvz_backend_api::UpgradePlantingRuleBackend for PortableBackend {
    fn upgrade_planting_unrestricted(&self) -> Result<bool> {
        Ok(pvzp_rs::modifier::upgrade_planting_unrestricted())
    }
    fn set_upgrade_planting_unrestricted(&self, enabled: bool) -> Result<()> {
        pvzp_rs::modifier::set_upgrade_planting_unrestricted(enabled).map_err(Into::into)
    }
}

impl rsvz_backend_api::PlantSlotEditBackend for PortableBackend {
    fn validate_plant_slot_change(&self, source: PlantId, target: u32, expected_target: Option<PlantId>) -> Result<()> {
        if source.generation() == 0 || expected_target.is_some_and(|id| id.generation() == 0) {
            return Err(PortableBackendError::OperationRejected("invalid plant identity"));
        }
        self.world()?
            .validate_plant_slot_change(source.raw(), target, expected_target.map_or(0, PlantId::raw))
            .map_err(Into::into)
    }

    fn plant_slot_id(&self, slot: u32) -> Result<Option<PlantId>> {
        Ok(self.world()?.plant_slot_id(slot)?.map(PlantId::from_raw))
    }
    fn relocate_plant_slot(
        &mut self, source: PlantId, target: u32, expected_target: Option<PlantId>,
    ) -> Result<rsvz_model::PlantSlotChange> {
        if source.generation() == 0 || expected_target.is_some_and(|id| id.generation() == 0) {
            return Err(PortableBackendError::OperationRejected("invalid plant identity"));
        }
        let mut world = self.world()?;
        // SAFETY: the mutable backend excludes every entity/pool borrow; the
        // physical host only lends it at its outer dispatch, not inside paint.
        let (moved, swapped) =
            unsafe { world.relocate_plant_slot(source.raw(), target, expected_target.map_or(0, PlantId::raw)) }?;
        Ok(rsvz_model::PlantSlotChange {
            moved: rsvz_model::PlantIdMapping {
                previous: source,
                current: PlantId::from_raw(moved),
            },
            swapped: expected_target
                .filter(|_| swapped != 0)
                .map(|previous| rsvz_model::PlantIdMapping {
                    previous,
                    current: PlantId::from_raw(swapped),
                }),
        })
    }
}

impl rsvz_backend_api::IZombiePlantStyleBackend for PortableBackend {
    fn setup_izombie_plant<'a>(&'a self, plant: Self::PlantHandle<'a>) -> Result<()> {
        plant.borrowed().setup_izombie().map_err(Into::into)
    }
}

impl rsvz_backend_api::NativePlantPlacementBackend for PortableBackend {
    fn add_placement_plant<'a>(
        &'a self, selection: rsvz_model::PlantPlacementSelection, grid: Grid,
    ) -> Result<PortablePlantHandle<'a>> {
        validate_grid(self, grid)?;
        self.world()?
            .add_plant(grid.col, grid.row, selection.packet(), selection.imitater())?
            .map(pvzp_rs::Borrowed::as_non_null)
            .map(PortablePlantHandle::new)
            .ok_or(PortableBackendError::OperationRejected("native AddPlant"))
    }
}
