use rsvz_backend_api::backend::{
    CobRuleEditBackend, DancerClockWriteBackend, DropRuleEditBackend, MaidCheatsBackend, PlantDamageRuleEditBackend,
    PlantEffectRuleEditBackend, PlantingRuleEditBackend, ProfileReadonlyBackend, ProjectileRuleEditBackend,
    SeedRuleEditBackend, SunCostRuleEditBackend, SunWriteBackend, VisibilityEditBackend, ZombieRuleEditBackend,
};
use rsvz_model::model::{KernelPultProjectileRule, MaidCheat, NonNegativeI32, PlantDamageRule};

use crate::error::{Pvz1051Error, Result};
use crate::patches;
use crate::patches::leases::{self, BoolPatchId};
use crate::raw::layout as ptrs;
use crate::runtime::Pvz1051Backend;

impl rsvz_backend_api::CoinProfileBackend for Pvz1051Backend {
    fn coins(&self) -> Result<u32> {
        let profile = std::ptr::NonNull::new(unsafe { ptrs::LawnApp::player_info(self.app().as_ptr()) })
            .ok_or(Pvz1051Error::NullUserData)?;
        u32::try_from(unsafe { ptrs::PlayerInfo::coins(profile.as_ptr()) })
            .map_err(|_| Pvz1051Error::InvariantViolated("profile coins are negative"))
    }
    fn set_coins(&self, value: u32) -> Result<()> {
        let value = i32::try_from(value).map_err(|_| Pvz1051Error::AbiPreconditionFailed("coins exceed i32"))?;
        if BoolPatchId::ProfileReadonly.enabled()? {
            return Err(Pvz1051Error::OperationRejected("profile is read-only"));
        }
        let profile = std::ptr::NonNull::new(unsafe { ptrs::LawnApp::player_info(self.app().as_ptr()) })
            .ok_or(Pvz1051Error::NullUserData)?;
        // SAFETY: the current profile is non-null, writable and uses the verified +0x28 coin scalar.
        unsafe { ptrs::PlayerInfo::set_coins(profile.as_ptr(), value) };
        Ok(())
    }
}

impl rsvz_backend_api::PickupLimitsBackend for Pvz1051Backend {
    fn sunlight_limit(&self) -> Result<u32> {
        patches::pickup_limits::sunlight_limit()
    }
    fn set_sunlight_limit(&self, value: u32) -> Result<()> {
        patches::pickup_limits::set_sunlight_limit(value)
    }
    fn money_limit(&self) -> Result<u32> {
        patches::pickup_limits::money_limit()
    }
    fn set_money_limit(&self, value: u32) -> Result<()> {
        patches::pickup_limits::set_money_limit(value)
    }
}

impl rsvz_backend_api::DanceModeBackend for Pvz1051Backend {
    fn dance_mode_enabled(&self) -> Result<bool> {
        // SAFETY: current App and the verified persistent Dance flag.
        Ok(unsafe { ptrs::LawnApp::dance_mode(self.app().as_ptr()) })
    }
    fn restore_dance_mode(&self, enabled: bool) -> Result<()> {
        // SAFETY: native outer access; SetDanceMode retains its animation side
        // effects when a Board exists, while menu cleanup restores only App.
        unsafe {
            let app = self.app().as_ptr();
            let board = ptrs::LawnApp::board(app);
            if board.is_null() {
                ptrs::LawnApp::set_dance_mode(app, enabled);
            } else {
                crate::raw::abi::board_set_dance_mode(board, i32::from(enabled));
            }
        }
        Ok(())
    }
    fn set_dance_mode(&self, enabled: bool) -> Result<()> {
        let board = self.board()?;
        // SAFETY: current Board, verified BL/stack ABI; the call does not recycle entities.
        unsafe { crate::raw::abi::board_set_dance_mode(board.as_ptr(), i32::from(enabled)) };
        Ok(())
    }
}

impl rsvz_backend_api::SeedBankVisibilityBackend for Pvz1051Backend {
    fn seed_bank_visible(&self) -> Result<bool> {
        // SAFETY: current non-null bank, verified bool offset.
        Ok(unsafe { ptrs::SeedBank::visible(self.seed_bank()?.as_ptr()) })
    }
    fn set_seed_bank_visible(&self, visible: bool) -> Result<()> {
        // SAFETY: scalar update does not invalidate any borrowed seed packet.
        unsafe { ptrs::SeedBank::set_visible(self.seed_bank()?.as_ptr(), visible) };
        Ok(())
    }
}

macro_rules! bool_patch_methods {
    ($get:ident, $set:ident, $id:ident) => {
        fn $get(&self) -> Result<bool> {
            BoolPatchId::$id.enabled()
        }

        fn $set(&self, enabled: bool) -> Result<()> {
            leases::set_bool_patch(BoolPatchId::$id, enabled)
        }
    };
}

impl SunWriteBackend for Pvz1051Backend {
    fn set_sun(&self, value: u32) -> Result<()> {
        let board = self.board()?;
        // SAFETY: `board` is non-null and the scalar write is limited to Board::mSunMoney.
        unsafe { ptrs::Board::set_sun(board.as_ptr(), value.min(i32::MAX as u32) as i32) };
        Ok(())
    }
}

impl DancerClockWriteBackend for Pvz1051Backend {
    fn set_dancer_clock(&self, value: u32) -> Result<()> {
        let app = self.app();
        // SAFETY: `app` is current and this writes only LawnApp::mAppCounter, the scalar read by
        // Zombie::GetDancerFrame.
        unsafe { *ptrs::LawnApp::app_counter_mut(app.as_ptr()) = value as i32 };
        Ok(())
    }
}

impl SeedRuleEditBackend for Pvz1051Backend {
    bool_patch_methods!(seed_recharge_ignored, set_seed_recharge_ignored, SeedRechargeIgnored);
}

impl SunCostRuleEditBackend for Pvz1051Backend {
    bool_patch_methods!(sun_cost_ignored, set_sun_cost_ignored, SunCostIgnored);
}

impl VisibilityEditBackend for Pvz1051Backend {
    bool_patch_methods!(fog_revealed, set_fog_revealed, FogRevealed);
    bool_patch_methods!(vase_contents_visible, set_vase_contents_visible, VaseContentsVisible);
}

impl ProjectileRuleEditBackend for Pvz1051Backend {
    fn kernel_pult_projectile_rule(&self) -> Result<KernelPultProjectileRule> {
        patches::variant::kernel_pult_projectile_rule::rule()
    }

    fn set_kernel_pult_projectile_rule(&self, rule: KernelPultProjectileRule) -> Result<()> {
        leases::set_kernel_pult_projectile_rule(rule)
    }
}

impl PlantEffectRuleEditBackend for Pvz1051Backend {
    fn plant_special_countdown_rule(&self) -> Result<rsvz_model::PlantSpecialCountdownRule> {
        patches::variant::plant_special_countdown_rule::rule()
    }
    fn set_plant_special_countdown_rule(&self, rule: rsvz_model::PlantSpecialCountdownRule) -> Result<()> {
        leases::set_plant_special_countdown_rule(rule)
    }
    fn instant_ice_and_ash_effects(&self) -> Result<bool> {
        Ok(self.plant_special_countdown_rule()? == rsvz_model::PlantSpecialCountdownRule::Immediate)
    }
    fn set_instant_ice_and_ash_effects(&self, enabled: bool) -> Result<()> {
        use rsvz_model::PlantSpecialCountdownRule::{Immediate, Normal};
        self.set_plant_special_countdown_rule(if enabled { Immediate } else { Normal })
    }
    bool_patch_methods!(mushrooms_awake, set_mushrooms_awake, MushroomsAwake);
}

impl CobRuleEditBackend for Pvz1051Backend {
    bool_patch_methods!(cob_fixed_delay, set_cob_fixed_delay, CobFixedDelay);
    bool_patch_methods!(cob_recharge_shortened, set_cob_recharge_shortened, CobRechargeShortened);
    bool_patch_methods!(cob_drift_fixed, set_cob_drift_fixed, CobDriftFixed);
}

impl DropRuleEditBackend for Pvz1051Backend {
    bool_patch_methods!(item_drop_disabled, set_item_drop_disabled, ItemDropDisabled);
    bool_patch_methods!(
        natural_sun_drop_disabled,
        set_natural_sun_drop_disabled,
        NaturalSunDropDisabled
    );

    fn set_natural_sun_generated(&self, count: NonNegativeI32) -> Result<()> {
        let board = self.board()?;
        // SAFETY: `board` is non-null and the write is limited to `Board::mNumSunsFallen`.
        unsafe { ptrs::Board::set_num_suns_fallen(board.as_ptr(), count.get()) };
        Ok(())
    }

    fn set_natural_sun_countdown(&self, countdown: NonNegativeI32) -> Result<()> {
        let board = self.board()?;
        // SAFETY: `board` is non-null and the write is limited to `Board::mSunCountDown`.
        unsafe { ptrs::Board::set_sun_countdown(board.as_ptr(), countdown.get()) };
        Ok(())
    }
}

impl ZombieRuleEditBackend for Pvz1051Backend {
    fn jack_explosion_rule(&self) -> Result<rsvz_model::ZombieExplosionRule> {
        patches::variant::jack_explosion_rule::rule()
    }
    fn set_jack_explosion_rule(&self, rule: rsvz_model::ZombieExplosionRule) -> Result<()> {
        leases::set_jack_explosion_rule(rule)
    }
    fn jack_explosions_disabled(&self) -> Result<bool> {
        Ok(self.jack_explosion_rule()? == rsvz_model::ZombieExplosionRule::Never)
    }
    fn set_jack_explosions_disabled(&self, enabled: bool) -> Result<()> {
        use rsvz_model::ZombieExplosionRule::{Never, Normal};
        self.set_jack_explosion_rule(if enabled { Never } else { Normal })
    }
    fn pepper_explosion_rule(&self) -> Result<rsvz_model::ZombieExplosionRule> {
        patches::variant::pepper_explosion_rule::rule()
    }
    fn set_pepper_explosion_rule(&self, rule: rsvz_model::ZombieExplosionRule) -> Result<()> {
        leases::set_pepper_explosion_rule(rule)
    }
    fn pepper_explosions_disabled(&self) -> Result<bool> {
        Ok(self.pepper_explosion_rule()? == rsvz_model::ZombieExplosionRule::Never)
    }
    fn set_pepper_explosions_disabled(&self, enabled: bool) -> Result<()> {
        use rsvz_model::ZombieExplosionRule::{Never, Normal};
        self.set_pepper_explosion_rule(if enabled { Never } else { Normal })
    }
    bool_patch_methods!(
        special_events_disabled,
        set_special_events_disabled,
        SpecialEventsDisabled
    );
    bool_patch_methods!(zombie_spawn_stopped, set_zombie_spawn_stopped, ZombieSpawnStopped);
    bool_patch_methods!(zombies_die_at_house, set_zombies_die_at_house, ZombiesDieAtHouse);
}

impl PlantingRuleEditBackend for Pvz1051Backend {
    fn easy_planting_cheat(&self) -> Result<bool> {
        let app = self.app();
        // SAFETY: `app` is non-null and the read is limited to `LawnApp::mEasyPlantingCheat`.
        Ok(unsafe { ptrs::LawnApp::is_easy_planting_cheat_enabled(app.as_ptr()) })
    }

    fn set_easy_planting_cheat(&self, enabled: bool) -> Result<()> {
        let app = self.app();
        // SAFETY: `app` is non-null and the write is limited to `LawnApp::mEasyPlantingCheat`,
        // verified from SeedPacket::CanPickUp/MouseDown and decomp field layout.
        unsafe { ptrs::LawnApp::set_easy_planting_cheat(app.as_ptr(), enabled) };
        Ok(())
    }

    bool_patch_methods!(
        planting_restrictions_ignored,
        set_planting_restrictions_ignored,
        PlantingRestrictionsIgnored
    );
}

impl PlantDamageRuleEditBackend for Pvz1051Backend {
    fn plant_damage_rule(&self) -> Result<PlantDamageRule> {
        crate::impls::event::damage_rule()
    }

    fn set_plant_damage_rule(&self, rule: PlantDamageRule) -> Result<()> {
        leases::set_plant_damage_rule(rule)
    }
}

impl ProfileReadonlyBackend for Pvz1051Backend {
    fn set_profile_readonly(&self, readonly: bool) -> Result<()> {
        if !readonly && crate::impls::reset::profile_isolation_active() {
            return Err(Pvz1051Error::AbiPreconditionFailed(
                "profile readonly is held by an active world-reset session",
            ));
        }
        leases::set_bool_patch(BoolPatchId::ProfileReadonly, readonly)
    }
}

impl MaidCheatsBackend for Pvz1051Backend {
    fn maid_cheat(&self) -> Result<MaidCheat> {
        patches::variant::maid_cheat::state()
    }

    fn set_maid_cheat(&self, cheat: MaidCheat) -> Result<()> {
        leases::set_maid_cheat(cheat)
    }
}

impl rsvz_backend_api::CraterLifetimeRuleBackend for Pvz1051Backend {
    fn craters_expire_immediately(&self) -> Result<bool> {
        BoolPatchId::CratersExpireImmediately.enabled()
    }
    fn set_craters_expire_immediately(&self, enabled: bool) -> Result<()> {
        leases::set_bool_patch(BoolPatchId::CratersExpireImmediately, enabled)
    }
}

impl rsvz_backend_api::ZombieBehaviorRulesBackend for Pvz1051Backend {
    fn chilled_effects_disabled(&self) -> Result<bool> {
        BoolPatchId::ChilledEffectsDisabled.enabled()
    }
    fn set_chilled_effects_disabled(&self, enabled: bool) -> Result<()> {
        leases::set_bool_patch(BoolPatchId::ChilledEffectsDisabled, enabled)
    }
    fn butter_effects_disabled(&self) -> Result<bool> {
        BoolPatchId::ButterEffectsDisabled.enabled()
    }
    fn set_butter_effects_disabled(&self, enabled: bool) -> Result<()> {
        leases::set_bool_patch(BoolPatchId::ButterEffectsDisabled, enabled)
    }
    fn imp_throw_disabled(&self) -> Result<bool> {
        BoolPatchId::ImpThrowDisabled.enabled()
    }
    fn set_imp_throw_disabled(&self, enabled: bool) -> Result<()> {
        leases::set_bool_patch(BoolPatchId::ImpThrowDisabled, enabled)
    }
    fn zombie_walking_stopped(&self) -> Result<bool> {
        BoolPatchId::ZombieWalkingStopped.enabled()
    }
    fn set_zombie_walking_stopped(&self, enabled: bool) -> Result<()> {
        leases::set_bool_patch(BoolPatchId::ZombieWalkingStopped, enabled)
    }
}

impl rsvz_backend_api::IceTrailRuleBackend for Pvz1051Backend {
    fn ice_trails_disabled(&self) -> Result<bool> {
        BoolPatchId::IceTrailsDisabled.enabled()
    }
    fn set_ice_trails_disabled(&self, enabled: bool) -> Result<()> {
        leases::set_bool_patch(BoolPatchId::IceTrailsDisabled, enabled)
    }
    fn set_ice_trail_countdown(&self, row: u8, value: NonNegativeI32) -> Result<()> {
        if row >= 6 {
            return Err(Pvz1051Error::AbiPreconditionFailed("ice trail row must be in 0..6"));
        }
        // SAFETY: live Board, checked index, and a nonnegative scalar; no pool mutation.
        unsafe { ptrs::Board::set_ice_trail_countdown(self.board()?.as_ptr(), row, value.get()) };
        Ok(())
    }
}

impl rsvz_backend_api::ZombieDamageRuleEditBackend for Pvz1051Backend {
    fn zombie_damage_rule(&self) -> Result<rsvz_model::ZombieDamageRule> {
        crate::patches::variant::zombie_damage_rule::rule()
    }
    fn set_zombie_damage_rule(&self, rule: rsvz_model::ZombieDamageRule) -> Result<()> {
        crate::patches::leases::set_zombie_damage_rule(rule)
    }
}

impl rsvz_backend_api::SeedRenderRulesBackend for Pvz1051Backend {
    fn seed_bank_topmost(&self) -> Result<bool> {
        BoolPatchId::SeedBankTopmost.enabled()
    }
    fn set_seed_bank_topmost(&self, enabled: bool) -> Result<()> {
        crate::patches::leases::set_bool_patch(crate::patches::leases::BoolPatchId::SeedBankTopmost, enabled)
    }
    fn seed_recharge_shading_hidden(&self) -> Result<bool> {
        BoolPatchId::SeedRechargeShadingHidden.enabled()
    }
    fn set_seed_recharge_shading_hidden(&self, enabled: bool) -> Result<()> {
        crate::patches::leases::set_bool_patch(crate::patches::leases::BoolPatchId::SeedRechargeShadingHidden, enabled)
    }
}

impl rsvz_backend_api::UpgradePlantingRuleBackend for Pvz1051Backend {
    fn upgrade_planting_unrestricted(&self) -> Result<bool> {
        BoolPatchId::UpgradePlantingUnrestricted.enabled()
    }
    fn set_upgrade_planting_unrestricted(&self, enabled: bool) -> Result<()> {
        crate::patches::leases::set_bool_patch(
            crate::patches::leases::BoolPatchId::UpgradePlantingUnrestricted,
            enabled,
        )
    }
}

impl rsvz_backend_api::IceTrailClearBackend for Pvz1051Backend {
    fn clear_ice_trails(&self) -> Result<()> {
        let board = self.board()?;
        // SAFETY: current game-thread Board; six rows. Resolve effects by full ID exactly
        // as native UpdateIce 0x414100 does, then use its existing 0x5160C0 death call.
        unsafe {
            let effects = ptrs::LawnApp::effect_system(self.app().as_ptr());
            let holder = if effects.is_null() {
                std::ptr::null_mut()
            } else {
                ptrs::EffectSystem::particle_system_holder(effects)
            };
            for row in 0..6 {
                let ice =
                    ptrs::ParticleSystemHolder::try_get(holder, ptrs::Board::ice_particle_id(board.as_ptr(), row));
                ptrs::Board::clear_ice_row(board.as_ptr(), row);
                if !ice.is_null() {
                    crate::raw::abi::particle_system_delete(ice);
                }
            }
        }
        Ok(())
    }
}
