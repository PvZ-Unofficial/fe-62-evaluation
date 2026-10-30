use crate::error::{Pvz1051Error, Result};
use rsvz_model::{KernelPultProjectileRule, MaidCheat, PlantDamageRule};
use std::cell::RefCell;

use super::spec::PatchBundle;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoolPatchId {
    SpawnCountPreserved,
    SeedCountReadDisabled,
    SlowUpdateProgress,
    SeedBankTopmost,
    SeedRechargeShadingHidden,
    UpgradePlantingUnrestricted,
    CratersExpireImmediately,
    ChilledEffectsDisabled,
    ButterEffectsDisabled,
    ImpThrowDisabled,
    ZombieWalkingStopped,
    IceTrailsDisabled,
    BackgroundRunning,
    PauseDialogDisabled,
    TemporaryUnlock,
    HiddenPagesVisible,
    SeedRechargeIgnored,
    SunCostIgnored,
    FogRevealed,
    VaseContentsVisible,
    CobFixedDelay,
    CobRechargeShortened,
    ItemDropDisabled,
    SpecialEventsDisabled,
    NaturalSunDropDisabled,
    AutoCollectNormal,
    CobDriftFixed,
    MushroomsAwake,
    ZombiesDieAtHouse,
    PlantingRestrictionsIgnored,
    ProfileReadonly,
    ZombieSpawnStopped,
}

impl BoolPatchId {
    const ALL: [Self; 32] = [
        Self::SpawnCountPreserved,
        Self::SeedCountReadDisabled,
        Self::BackgroundRunning,
        Self::PauseDialogDisabled,
        Self::IceTrailsDisabled,
        Self::SlowUpdateProgress,
        Self::SeedBankTopmost,
        Self::SeedRechargeShadingHidden,
        Self::UpgradePlantingUnrestricted,
        Self::CratersExpireImmediately,
        Self::ChilledEffectsDisabled,
        Self::ButterEffectsDisabled,
        Self::ImpThrowDisabled,
        Self::ZombieWalkingStopped,
        Self::TemporaryUnlock,
        Self::HiddenPagesVisible,
        Self::SeedRechargeIgnored,
        Self::SunCostIgnored,
        Self::FogRevealed,
        Self::VaseContentsVisible,
        Self::CobFixedDelay,
        Self::CobRechargeShortened,
        Self::ItemDropDisabled,
        Self::SpecialEventsDisabled,
        Self::NaturalSunDropDisabled,
        Self::AutoCollectNormal,
        Self::CobDriftFixed,
        Self::MushroomsAwake,
        Self::ZombiesDieAtHouse,
        Self::PlantingRestrictionsIgnored,
        Self::ProfileReadonly,
        Self::ZombieSpawnStopped,
    ];

    const fn mask(self) -> u32 {
        1 << self as u8
    }

    fn bundle(self) -> &'static PatchBundle {
        match self {
            Self::SpawnCountPreserved => &super::mods::spawn_count_preserved::BUNDLE,
            Self::SeedCountReadDisabled => &super::mods::seed_count_read_disabled::BUNDLE,
            Self::BackgroundRunning => &super::mods::background_running::BUNDLE,
            Self::PauseDialogDisabled => &super::mods::pause_dialog_disabled::BUNDLE,
            Self::IceTrailsDisabled => &super::mods::ice_trails_disabled::BUNDLE,
            Self::SlowUpdateProgress => &super::mods::slow_update_progress::BUNDLE,
            Self::SeedBankTopmost => &super::mods::seed_bank_topmost::BUNDLE,
            Self::SeedRechargeShadingHidden => &super::mods::seed_recharge_shading_hidden::BUNDLE,
            Self::UpgradePlantingUnrestricted => &super::mods::upgrade_planting_unrestricted::BUNDLE,
            Self::CratersExpireImmediately => &super::mods::craters_expire_immediately::BUNDLE,
            Self::ChilledEffectsDisabled => &super::mods::chilled_effects_disabled::BUNDLE,
            Self::ButterEffectsDisabled => &super::mods::butter_effects_disabled::BUNDLE,
            Self::ImpThrowDisabled => &super::mods::imp_throw_disabled::BUNDLE,
            Self::ZombieWalkingStopped => &super::mods::zombie_walking_stopped::BUNDLE,

            Self::TemporaryUnlock => &super::mods::temporary_unlock::BUNDLE,
            Self::HiddenPagesVisible => &super::mods::hidden_pages_visible::BUNDLE,
            Self::SeedRechargeIgnored => &super::mods::seed_recharge_ignored::BUNDLE,
            Self::SunCostIgnored => &super::mods::sun_cost_ignored::BUNDLE,
            Self::FogRevealed => &super::mods::fog_revealed::BUNDLE,
            Self::VaseContentsVisible => &super::mods::vase_contents_visible::BUNDLE,
            Self::CobFixedDelay => &super::mods::cob_fixed_delay::BUNDLE,
            Self::CobRechargeShortened => &super::mods::cob_recharge_shortened::BUNDLE,
            Self::ItemDropDisabled => &super::mods::item_drop_disabled::BUNDLE,
            Self::SpecialEventsDisabled => &super::mods::special_events_disabled::BUNDLE,
            Self::NaturalSunDropDisabled => &super::mods::natural_sun_drop_disabled::BUNDLE,
            Self::AutoCollectNormal => &super::mods::auto_collect_normal::BUNDLE,
            Self::CobDriftFixed => &super::mods::cob_drift_fixed::BUNDLE,
            Self::MushroomsAwake => &super::mods::mushrooms_awake::BUNDLE,
            Self::ZombiesDieAtHouse => &super::mods::zombies_die_at_house::BUNDLE,
            Self::PlantingRestrictionsIgnored => &super::mods::planting_restrictions_ignored::BUNDLE,
            Self::ProfileReadonly => &super::mods::profile_readonly::BUNDLE,
            Self::ZombieSpawnStopped => &super::mods::zombie_spawn_stopped::BUNDLE,
        }
    }

    pub(crate) fn enabled(self) -> Result<bool> {
        self.bundle().enabled()
    }
}

#[derive(Debug, Default)]
struct PatchLeaseManager {
    bool_patches: u32,
    original_kernel_pult_rule: Option<KernelPultProjectileRule>,
    original_zombie_damage_rule: Option<rsvz_model::ZombieDamageRule>,
    original_jack_explosion_rule: Option<rsvz_model::ZombieExplosionRule>,
    original_pepper_explosion_rule: Option<rsvz_model::ZombieExplosionRule>,
    original_special_countdown_rule: Option<rsvz_model::PlantSpecialCountdownRule>,
    original_maid_cheat: Option<MaidCheat>,
}

thread_local! {
    static MANAGER: RefCell<PatchLeaseManager> = RefCell::new(PatchLeaseManager::default());
}

pub(crate) fn set_bool_patch(patch: BoolPatchId, enabled: bool) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.bool_patch)(patch, enabled));
    }
    if enabled {
        MANAGER.with_borrow_mut(|manager| manager.acquire_bool_patch(patch))
    } else {
        MANAGER.with_borrow_mut(|manager| manager.release_bool_patch(patch))
    }
}

pub(crate) fn bool_patch_owned(patch: BoolPatchId) -> bool {
    if let Some(host) = crate::module_link::services() {
        return (host.bool_owned)(patch);
    }
    MANAGER.with_borrow(|manager| manager.bool_patches & patch.mask() != 0)
}

pub(crate) fn set_plant_special_countdown_rule(rule: rsvz_model::PlantSpecialCountdownRule) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.plant_special_countdown_rule)(rule));
    }
    MANAGER.with_borrow_mut(|manager| {
        let original = match manager.original_special_countdown_rule {
            Some(original) => original,
            None => super::variant::plant_special_countdown_rule::rule()?,
        };
        super::variant::plant_special_countdown_rule::set_rule(rule)?;
        manager.original_special_countdown_rule = Some(original);
        Ok(())
    })
}

pub(crate) fn set_kernel_pult_projectile_rule(rule: KernelPultProjectileRule) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.kernel_pult_projectile_rule)(rule));
    }
    MANAGER.with_borrow_mut(|manager| manager.set_kernel_pult_projectile_rule(rule))
}

pub(crate) fn set_plant_damage_rule(rule: PlantDamageRule) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.plant_damage_rule)(rule));
    }
    MANAGER.with_borrow_mut(|manager| manager.set_plant_damage_rule(rule))
}

pub(crate) fn set_maid_cheat(state: MaidCheat) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.maid_cheat)(state));
    }
    MANAGER.with_borrow_mut(|manager| manager.set_maid_cheat(state))
}

pub(crate) fn release_all_owned_patches() -> Result<()> {
    MANAGER.with_borrow_mut(PatchLeaseManager::release_all)
}

fn try_each_best_effort<T, E>(
    items: impl IntoIterator<Item = T>, mut try_item: impl FnMut(T) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    let mut first_error = None;

    for item in items {
        if let Err(error) = try_item(item)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}

impl PatchLeaseManager {
    fn acquire_bool_patch(&mut self, patch: BoolPatchId) -> Result<()> {
        if self.bool_patches & patch.mask() != 0 {
            return Ok(());
        }
        patch.bundle().set_enabled(true)?;
        self.bool_patches |= patch.mask();
        Ok(())
    }

    fn release_bool_patch(&mut self, patch: BoolPatchId) -> Result<()> {
        patch.bundle().set_enabled(false)?;
        self.bool_patches &= !patch.mask();
        Ok(())
    }

    fn set_kernel_pult_projectile_rule(&mut self, rule: KernelPultProjectileRule) -> Result<()> {
        let original = match self.original_kernel_pult_rule {
            Some(original) => original,
            None => super::variant::kernel_pult_projectile_rule::rule()?,
        };
        super::variant::kernel_pult_projectile_rule::set_rule(rule)?;
        self.original_kernel_pult_rule.get_or_insert(original);
        Ok(())
    }

    fn set_plant_damage_rule(&mut self, rule: PlantDamageRule) -> Result<()> {
        crate::impls::event::set_owned_damage_rule(rule)
    }

    fn set_maid_cheat(&mut self, state: MaidCheat) -> Result<()> {
        let original = match self.original_maid_cheat {
            Some(original) => original,
            None => super::variant::maid_cheat::state()?,
        };
        super::variant::maid_cheat::set(state)?;
        self.original_maid_cheat.get_or_insert(original);
        Ok(())
    }

    fn release_all(&mut self) -> Result<()> {
        let bool_result = try_each_best_effort(BoolPatchId::ALL, |patch| self.release_bool_patch(patch));
        let variant_result = self.restore_variant_rules();
        let limits_result = super::pickup_limits::restore();
        bool_result.and(variant_result).and(limits_result)
    }

    fn restore_variant_rules(&mut self) -> Result<()> {
        let mut first_error = None;
        if let Some(original) = self.original_zombie_damage_rule {
            match super::variant::zombie_damage_rule::set_rule(original) {
                Ok(()) => self.original_zombie_damage_rule = None,
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        if let Some(original) = self.original_pepper_explosion_rule {
            match super::variant::pepper_explosion_rule::set_rule(original) {
                Ok(()) => self.original_pepper_explosion_rule = None,
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if let Some(original) = self.original_jack_explosion_rule {
            match super::variant::jack_explosion_rule::set_rule(original) {
                Ok(()) => self.original_jack_explosion_rule = None,
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if let Some(original) = self.original_special_countdown_rule {
            match super::variant::plant_special_countdown_rule::set_rule(original) {
                Ok(()) => self.original_special_countdown_rule = None,
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if let Some(original) = self.original_kernel_pult_rule
            && let Err(error) = super::variant::kernel_pult_projectile_rule::set_rule(original)
        {
            first_error.get_or_insert(error);
        } else {
            self.original_kernel_pult_rule = None;
        }
        if let Err(error) = crate::impls::event::restore_owned_damage_rule()
            && first_error.is_none()
        {
            first_error = Some(error);
        }
        if let Some(original) = self.original_maid_cheat {
            match super::variant::maid_cheat::set(original) {
                Ok(()) => self.original_maid_cheat = None,
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct PatchRange {
        owner: &'static str,
        start: usize,
        end: usize,
    }

    impl PatchRange {
        fn new(owner: &'static str, start: usize, len: usize) -> Self {
            Self {
                owner,
                start,
                end: start.checked_add(len).expect("test patch range must not overflow"),
            }
        }

        fn overlaps(self, other: Self) -> bool {
            self.start < other.end && other.start < self.end
        }
    }

    fn current_lease_and_rule_ranges() -> Vec<PatchRange> {
        let mut ranges = Vec::new();
        for entry in super::super::variant::zombie_damage_rule::BUNDLE.entries {
            ranges.push(PatchRange::new(entry.name, entry.addr, entry.variants[0].1.len()));
        }
        for entry in super::super::variant::pepper_explosion_rule::BUNDLE.entries {
            ranges.push(PatchRange::new(entry.name, entry.addr, entry.variants[0].1.len()));
        }
        for entry in super::super::variant::jack_explosion_rule::BUNDLE.entries {
            ranges.push(PatchRange::new(entry.name, entry.addr, entry.variants[0].1.len()));
        }
        for patch in &BoolPatchId::ALL {
            for spec in patch.bundle().specs {
                ranges.push(PatchRange::new(spec.name, spec.addr, spec.len()));
            }
        }
        for entry in super::super::variant::plant_special_countdown_rule::BUNDLE.entries {
            ranges.push(PatchRange::new(entry.name, entry.addr, entry.variants[0].1.len()));
        }
        for entry in super::super::variant::kernel_pult_projectile_rule::BUNDLE.entries {
            ranges.push(PatchRange::new(entry.name, entry.addr, entry.variants[0].1.len()));
        }
        for entry in super::super::variant::plant_damage_rule::BUNDLE.entries {
            ranges.push(PatchRange::new(entry.name, entry.addr, entry.variants[0].1.len()));
        }
        for entry in super::super::variant::maid_cheat::BUNDLE.entries {
            ranges.push(PatchRange::new(entry.name, entry.addr, entry.variants[0].1.len()));
        }
        ranges.push(PatchRange::new(
            "measurement logical-frame Board::Update call",
            0x4526f7,
            5,
        ));
        ranges
    }

    #[test]
    fn best_effort_release_keeps_trying_after_the_first_error() {
        let mut attempted = Vec::new();
        let result = try_each_best_effort([1, 2, 3], |item| {
            attempted.push(item);
            match item {
                1 => Err("first restore failed"),
                3 => Err("later restore failed"),
                _ => Ok(()),
            }
        });

        assert_eq!(attempted, [1, 2, 3]);
        assert_eq!(result, Err("first restore failed"));
    }

    #[test]
    fn current_lease_and_rule_patch_ranges_do_not_overlap() {
        let ranges = current_lease_and_rule_ranges();
        for (index, left) in ranges.iter().copied().enumerate() {
            for right in ranges[index + 1..].iter().copied() {
                assert!(
                    !left.overlaps(right),
                    "current patch owners overlap: {} {left:?} and {} {right:?}",
                    left.owner,
                    right.owner
                );
            }
        }
    }
}

pub(crate) fn set_jack_explosion_rule(rule: rsvz_model::ZombieExplosionRule) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.jack_explosion_rule)(rule));
    }
    MANAGER.with_borrow_mut(|manager| {
        let original = match manager.original_jack_explosion_rule {
            Some(original) => original,
            None => super::variant::jack_explosion_rule::rule()?,
        };
        super::variant::jack_explosion_rule::set_rule(rule)?;
        manager.original_jack_explosion_rule = Some(original);
        Ok(())
    })
}

pub(crate) fn set_pepper_explosion_rule(rule: rsvz_model::ZombieExplosionRule) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.pepper_explosion_rule)(rule));
    }
    MANAGER.with_borrow_mut(|manager| {
        let original = match manager.original_pepper_explosion_rule {
            Some(original) => original,
            None => super::variant::pepper_explosion_rule::rule()?,
        };
        super::variant::pepper_explosion_rule::set_rule(rule)?;
        manager.original_pepper_explosion_rule = Some(original);
        Ok(())
    })
}

pub(crate) fn set_zombie_damage_rule(rule: rsvz_model::ZombieDamageRule) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.zombie_damage_rule)(rule));
    }
    MANAGER.with_borrow_mut(|manager| {
        let original = match manager.original_zombie_damage_rule {
            Some(original) => original,
            None => super::variant::zombie_damage_rule::rule()?,
        };
        super::variant::zombie_damage_rule::set_rule(rule)?;
        manager.original_zombie_damage_rule.get_or_insert(original);
        Ok(())
    })
}
