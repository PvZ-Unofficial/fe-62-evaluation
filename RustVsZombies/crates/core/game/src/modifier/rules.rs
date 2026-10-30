//! Native modifier rules. Reads and non-rejecting writes abort on infrastructure faults.
//! Lease-managed 1051 switches release only patches owned by this runner; reads
//! observe actual native state, including foreign patches. Acquisition conflicts abort.
//! Instant effects affect the shared native special-effect countdown, not just ice and ash.

use crate::{access::with_backend, live_value::read_or_abort, runtime::RuntimeResult};
use rsvz_backend_api::{
    CobRuleEditBackend, DropRuleEditBackend, MaidCheatsBackend, PlantDamageRuleEditBackend, PlantEffectRuleEditBackend,
    PlantingRuleEditBackend, ProjectileRuleEditBackend, SeedRuleEditBackend, SunCostRuleEditBackend,
    ZombieRuleEditBackend,
};
use rsvz_current::CurrentBackend;

pub fn seed_recharge_ignored() -> bool
where
    CurrentBackend: SeedRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.seed_recharge_ignored(), "seed_recharge_ignored"))
}

pub fn sun_cost_ignored() -> bool
where
    CurrentBackend: SunCostRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.sun_cost_ignored(), "sun_cost_ignored"))
}

pub fn easy_planting_cheat() -> bool
where
    CurrentBackend: PlantingRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.easy_planting_cheat(), "easy_planting_cheat"))
}

pub fn set_easy_planting_cheat(value: bool)
where
    CurrentBackend: PlantingRuleEditBackend,
{
    crate::live_value::read_or_abort(
        crate::control::declare_native_rule!(
            bool,
            value,
            easy_planting_cheat,
            set_easy_planting_cheat,
            CONTROL_EASY_PLANTING_CHEAT
        ),
        "set_easy_planting_cheat",
    )
}

pub fn planting_restrictions_ignored() -> bool
where
    CurrentBackend: PlantingRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.planting_restrictions_ignored(), "planting_restrictions_ignored"))
}

pub fn set_planting_restrictions_ignored(value: bool)
where
    CurrentBackend: PlantingRuleEditBackend,
{
    crate::live_value::read_or_abort(
        crate::control::declare_native_rule!(
            bool,
            value,
            planting_restrictions_ignored,
            set_planting_restrictions_ignored,
            CONTROL_PLANTING_RESTRICTIONS_IGNORED
        ),
        "set_planting_restrictions_ignored",
    )
}

pub fn instant_ice_and_ash_effects() -> bool
where
    CurrentBackend: PlantEffectRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.instant_ice_and_ash_effects(), "instant_ice_and_ash_effects"))
}

pub fn set_instant_ice_and_ash_effects(value: bool)
where
    CurrentBackend: PlantEffectRuleEditBackend,
{
    use rsvz_model::PlantSpecialCountdownRule::{Immediate, Normal};
    crate::live_value::read_or_abort(
        set_plant_special_countdown_rule(if value { Immediate } else { Normal }),
        "set_instant_ice_and_ash_effects",
    );
}

pub fn set_plant_special_countdown_rule(rule: rsvz_model::PlantSpecialCountdownRule) -> RuntimeResult<()>
where
    CurrentBackend: PlantEffectRuleEditBackend,
{
    crate::control::declare_native_rule!(
        rsvz_model::PlantSpecialCountdownRule,
        rule,
        plant_special_countdown_rule,
        set_plant_special_countdown_rule,
        CONTROL_PLANT_SPECIAL_COUNTDOWN_RULE
    )
}

pub fn mushrooms_awake() -> bool
where
    CurrentBackend: PlantEffectRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.mushrooms_awake(), "mushrooms_awake"))
}

/// Enables the native rule and wakes existing uncrushed sleeping plants.
/// Disabling only releases the rule; it does not put awakened plants back to sleep.
pub fn set_mushrooms_awake(value: bool)
where
    CurrentBackend: PlantEffectRuleEditBackend
        + rsvz_backend_api::PlantPresenceBackend
        + rsvz_backend_api::PlantSleepBackend
        + rsvz_backend_api::GameUiBackend
        + rsvz_backend_api::BoardReadinessBackend,
{
    use rsvz_backend_api::{
        BoardReadinessBackend, GameUiBackend, PlantPresenceBackend, PlantReadBackend, PlantSleepBackend,
    };
    let result = crate::control::declare_persistent(
        &CONTROL_MUSHROOMS_AWAKE,
        value,
        || with_backend(|backend| backend.mushrooms_awake()).map_err(crate::access::operation_error),
        |enabled| {
            with_backend(|backend| {
                backend
                    .set_mushrooms_awake(enabled)
                    .map_err(crate::access::operation_error)?;
                if !enabled {
                    return Ok(());
                }
                let ready = match backend.game_ui().map_err(crate::access::operation_error)? {
                    rsvz_model::GameUi::LevelIntro => backend.level_intro_board_ready(),
                    rsvz_model::GameUi::Playing => backend.playing_board_ready(),
                    _ => return Ok(()),
                }
                .map_err(crate::access::operation_error)?;
                if ready {
                    for plant in backend.present_plants().map_err(crate::access::operation_error)? {
                        if !backend.plant_is_squished(plant) && backend.plant_is_sleeping(plant) {
                            backend
                                .set_plant_sleeping(plant, false)
                                .map_err(crate::access::operation_error)?;
                        }
                    }
                }
                Ok(())
            })
        },
    );
    crate::live_value::read_or_abort(result, "set_mushrooms_awake");
}

pub fn plant_damage_rule() -> rsvz_model::PlantDamageRule
where
    CurrentBackend: PlantDamageRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.plant_damage_rule(), "plant_damage_rule"))
}

pub fn set_plant_damage_rule(value: rsvz_model::PlantDamageRule)
where
    CurrentBackend: PlantDamageRuleEditBackend,
{
    crate::live_value::read_or_abort(request_plant_damage_rule(Some(value)), "set_plant_damage_rule")
}

/// `None` releases this source's declaration; it does not declare false/normal.
pub fn request_plant_damage_rule(value: Option<rsvz_model::PlantDamageRule>) -> RuntimeResult<()>
where
    CurrentBackend: PlantDamageRuleEditBackend,
{
    crate::control::request_native_rule!(
        rsvz_model::PlantDamageRule,
        value,
        plant_damage_rule,
        set_plant_damage_rule,
        CONTROL_PLANT_DAMAGE_RULE
    )
}

pub fn kernel_pult_projectile_rule() -> rsvz_model::KernelPultProjectileRule
where
    CurrentBackend: ProjectileRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.kernel_pult_projectile_rule(), "kernel_pult_projectile_rule"))
}

pub fn set_kernel_pult_projectile_rule(value: rsvz_model::KernelPultProjectileRule)
where
    CurrentBackend: ProjectileRuleEditBackend,
{
    crate::live_value::read_or_abort(
        crate::control::declare_native_rule!(
            rsvz_model::KernelPultProjectileRule,
            value,
            kernel_pult_projectile_rule,
            set_kernel_pult_projectile_rule,
            CONTROL_KERNEL_PULT_PROJECTILE_RULE
        ),
        "set_kernel_pult_projectile_rule",
    )
}

pub fn cob_fixed_delay() -> bool
where
    CurrentBackend: CobRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.cob_fixed_delay(), "cob_fixed_delay"))
}

pub fn cob_recharge_shortened() -> bool
where
    CurrentBackend: CobRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.cob_recharge_shortened(), "cob_recharge_shortened"))
}

pub fn set_cob_recharge_shortened(value: bool)
where
    CurrentBackend: CobRuleEditBackend,
{
    crate::live_value::read_or_abort(
        request_cob_recharge_shortened(Some(value)),
        "set_cob_recharge_shortened",
    )
}

/// `None` releases this source's declaration; it does not declare false/normal.
pub fn request_cob_recharge_shortened(value: Option<bool>) -> RuntimeResult<()>
where
    CurrentBackend: CobRuleEditBackend,
{
    crate::control::request_native_rule!(
        bool,
        value,
        cob_recharge_shortened,
        set_cob_recharge_shortened,
        CONTROL_COB_RECHARGE_SHORTENED
    )
}

pub fn cob_drift_fixed() -> bool
where
    CurrentBackend: CobRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.cob_drift_fixed(), "cob_drift_fixed"))
}

pub fn set_cob_drift_fixed(value: bool)
where
    CurrentBackend: CobRuleEditBackend,
{
    crate::live_value::read_or_abort(
        crate::control::declare_native_rule!(
            bool,
            value,
            cob_drift_fixed,
            set_cob_drift_fixed,
            CONTROL_COB_DRIFT_FIXED
        ),
        "set_cob_drift_fixed",
    )
}

pub fn item_drop_disabled() -> bool
where
    CurrentBackend: DropRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.item_drop_disabled(), "item_drop_disabled"))
}
pub fn set_item_drop_disabled(value: bool) -> RuntimeResult<()>
where
    CurrentBackend: DropRuleEditBackend,
{
    crate::control::declare_native_rule!(
        bool,
        value,
        item_drop_disabled,
        set_item_drop_disabled,
        CONTROL_ITEM_DROP_DISABLED
    )
}

pub fn natural_sun_drop_disabled() -> bool
where
    CurrentBackend: DropRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.natural_sun_drop_disabled(), "natural_sun_drop_disabled"))
}

pub fn set_natural_sun_drop_disabled(value: bool)
where
    CurrentBackend: DropRuleEditBackend,
{
    crate::live_value::read_or_abort(
        crate::control::declare_native_rule!(
            bool,
            value,
            natural_sun_drop_disabled,
            set_natural_sun_drop_disabled,
            CONTROL_NATURAL_SUN_DROP_DISABLED
        ),
        "set_natural_sun_drop_disabled",
    )
}

pub fn jack_explosions_disabled() -> bool
where
    CurrentBackend: ZombieRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.jack_explosions_disabled(), "jack_explosions_disabled"))
}

pub fn set_jack_explosions_disabled(value: bool)
where
    CurrentBackend: ZombieRuleEditBackend,
{
    use rsvz_model::ZombieExplosionRule::{Never, Normal};
    crate::live_value::read_or_abort(
        set_jack_explosion_rule(if value { Never } else { Normal }),
        "set_jack_explosions_disabled",
    )
}
pub fn set_jack_explosion_rule(rule: rsvz_model::ZombieExplosionRule) -> RuntimeResult<()>
where
    CurrentBackend: ZombieRuleEditBackend,
{
    crate::control::declare_native_rule!(
        rsvz_model::ZombieExplosionRule,
        rule,
        jack_explosion_rule,
        set_jack_explosion_rule,
        CONTROL_JACK_EXPLOSION_RULE
    )
}

pub fn pepper_explosions_disabled() -> RuntimeResult<bool>
where
    CurrentBackend: ZombieRuleEditBackend,
{
    with_backend(|b| b.pepper_explosions_disabled()).map_err(crate::access::operation_error)
}
pub fn set_pepper_explosions_disabled(value: bool) -> RuntimeResult<()>
where
    CurrentBackend: ZombieRuleEditBackend,
{
    use rsvz_model::ZombieExplosionRule::{Never, Normal};
    set_pepper_explosion_rule(if value { Never } else { Normal })
}
pub fn set_pepper_explosion_rule(rule: rsvz_model::ZombieExplosionRule) -> RuntimeResult<()>
where
    CurrentBackend: ZombieRuleEditBackend,
{
    crate::control::declare_native_rule!(
        rsvz_model::ZombieExplosionRule,
        rule,
        pepper_explosion_rule,
        set_pepper_explosion_rule,
        CONTROL_PEPPER_EXPLOSION_RULE
    )
}

pub fn special_events_disabled() -> bool
where
    CurrentBackend: ZombieRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.special_events_disabled(), "special_events_disabled"))
}

pub fn set_special_events_disabled(value: bool)
where
    CurrentBackend: ZombieRuleEditBackend,
{
    crate::live_value::read_or_abort(
        crate::control::declare_native_rule!(
            bool,
            value,
            special_events_disabled,
            set_special_events_disabled,
            CONTROL_SPECIAL_EVENTS_DISABLED
        ),
        "set_special_events_disabled",
    )
}

pub fn zombie_spawn_stopped() -> bool
where
    CurrentBackend: ZombieRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.zombie_spawn_stopped(), "zombie_spawn_stopped"))
}

pub fn set_zombie_spawn_stopped(value: bool)
where
    CurrentBackend: ZombieRuleEditBackend,
{
    crate::live_value::read_or_abort(request_zombie_spawn_stopped(Some(value)), "set_zombie_spawn_stopped")
}

/// `None` releases this source's declaration; it does not declare false/normal.
pub fn request_zombie_spawn_stopped(value: Option<bool>) -> RuntimeResult<()>
where
    CurrentBackend: ZombieRuleEditBackend,
{
    crate::control::request_native_rule!(
        bool,
        value,
        zombie_spawn_stopped,
        set_zombie_spawn_stopped,
        CONTROL_ZOMBIE_SPAWN_STOPPED
    )
}

pub fn zombies_die_at_house() -> bool
where
    CurrentBackend: ZombieRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.zombies_die_at_house(), "zombies_die_at_house"))
}

pub fn set_zombies_die_at_house(value: bool)
where
    CurrentBackend: ZombieRuleEditBackend,
{
    crate::live_value::read_or_abort(
        crate::control::declare_native_rule!(
            bool,
            value,
            zombies_die_at_house,
            set_zombies_die_at_house,
            CONTROL_ZOMBIES_DIE_AT_HOUSE
        ),
        "set_zombies_die_at_house",
    )
}

pub fn maid_cheat() -> rsvz_model::MaidCheat
where
    CurrentBackend: MaidCheatsBackend,
{
    with_backend(|backend| read_or_abort(backend.maid_cheat(), "maid_cheat"))
}

pub fn set_natural_sun_generated(value: rsvz_model::NonNegativeI32)
where
    CurrentBackend: DropRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.set_natural_sun_generated(value), "set_natural_sun_generated"))
}

pub fn set_natural_sun_countdown(value: rsvz_model::NonNegativeI32)
where
    CurrentBackend: DropRuleEditBackend,
{
    with_backend(|backend| read_or_abort(backend.set_natural_sun_countdown(value), "set_natural_sun_countdown"))
}

pub fn set_craters_expire_immediately(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::CraterLifetimeRuleBackend,
{
    use rsvz_backend_api::CraterLifetimeRuleBackend;
    crate::control::declare_native_rule!(
        bool,
        enabled,
        craters_expire_immediately,
        set_craters_expire_immediately,
        CONTROL_CRATERS_EXPIRE_IMMEDIATELY
    )
}

pub fn set_chilled_effects_disabled(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::ZombieBehaviorRulesBackend,
{
    use rsvz_backend_api::ZombieBehaviorRulesBackend;
    crate::control::declare_native_rule!(
        bool,
        enabled,
        chilled_effects_disabled,
        set_chilled_effects_disabled,
        CONTROL_CHILLED_EFFECTS_DISABLED
    )
}

pub fn set_butter_effects_disabled(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::ZombieBehaviorRulesBackend,
{
    use rsvz_backend_api::ZombieBehaviorRulesBackend;
    crate::control::declare_native_rule!(
        bool,
        enabled,
        butter_effects_disabled,
        set_butter_effects_disabled,
        CONTROL_BUTTER_EFFECTS_DISABLED
    )
}

pub fn set_imp_throw_disabled(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::ZombieBehaviorRulesBackend,
{
    use rsvz_backend_api::ZombieBehaviorRulesBackend;
    crate::control::declare_native_rule!(
        bool,
        enabled,
        imp_throw_disabled,
        set_imp_throw_disabled,
        CONTROL_IMP_THROW_DISABLED
    )
}

pub fn set_zombie_walking_stopped(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::ZombieBehaviorRulesBackend,
{
    use rsvz_backend_api::ZombieBehaviorRulesBackend;
    crate::control::declare_native_rule!(
        bool,
        enabled,
        zombie_walking_stopped,
        set_zombie_walking_stopped,
        CONTROL_ZOMBIE_WALKING_STOPPED
    )
}

/// PT's paired jack opening and jalapeno explosion rule. The capability checks
/// precede either declaration; a native write failure may still be partial.
pub fn set_zombie_explosion_rule(rule: rsvz_model::ZombieExplosionRule) -> RuntimeResult<()>
where
    CurrentBackend: ZombieRuleEditBackend,
{
    with_backend(|backend| {
        backend.jack_explosion_rule().map_err(crate::access::operation_error)?;
        backend.pepper_explosion_rule().map_err(crate::access::operation_error)
    })?;
    set_jack_explosion_rule(rule)?;
    set_pepper_explosion_rule(rule)
}

/// Applies PT's native ice rule and schedules all existing row trails to expire.
/// Disable restores native creation; it never recreates a trail already removed.
pub fn set_ice_trails_disabled(value: bool) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::IceTrailRuleBackend
        + rsvz_backend_api::GameUiBackend
        + rsvz_backend_api::BoardReadinessBackend,
{
    use rsvz_backend_api::{BoardReadinessBackend, GameUiBackend, IceTrailRuleBackend};
    crate::control::declare_persistent(
        &CONTROL_ICE_TRAILS_DISABLED,
        value,
        || with_backend(|backend| backend.ice_trails_disabled()).map_err(crate::access::operation_error),
        |enabled| {
            with_backend(|backend| {
                backend
                    .set_ice_trails_disabled(enabled)
                    .map_err(crate::access::operation_error)?;
                if !enabled {
                    return Ok(());
                }
                let ready = match backend.game_ui().map_err(crate::access::operation_error)? {
                    rsvz_model::GameUi::LevelIntro => backend.level_intro_board_ready(),
                    rsvz_model::GameUi::Playing => backend.playing_board_ready(),
                    _ => return Ok(()),
                }
                .map_err(crate::access::operation_error)?;
                if ready {
                    for row in 0..6 {
                        backend
                            .set_ice_trail_countdown(
                                row,
                                rsvz_model::NonNegativeI32::new(1).expect("one is nonnegative"),
                            )
                            .map_err(crate::access::operation_error)?;
                    }
                }
                Ok(())
            })
        },
    )
}

/// Declares the PT eight-point zombie strength rule for this module.
pub fn set_zombie_damage_rule(rule: rsvz_model::ZombieDamageRule) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::ZombieDamageRuleEditBackend,
{
    use rsvz_backend_api::ZombieDamageRuleEditBackend;
    crate::control::declare_native_rule!(
        rsvz_model::ZombieDamageRule,
        rule,
        zombie_damage_rule,
        set_zombie_damage_rule,
        CONTROL_ZOMBIE_DAMAGE_RULE
    )
}

pub fn zombie_damage_rule() -> rsvz_model::ZombieDamageRule
where
    CurrentBackend: rsvz_backend_api::ZombieDamageRuleEditBackend,
{
    use rsvz_backend_api::ZombieDamageRuleEditBackend;
    with_backend(|backend| read_or_abort(backend.zombie_damage_rule(), "zombie_damage_rule"))
}

pub fn set_seed_bank_topmost(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::SeedRenderRulesBackend,
{
    use rsvz_backend_api::SeedRenderRulesBackend;
    if CurrentBackend::SEED_PRESENTATION_IS_NOOP {
        return Ok(());
    }
    crate::control::declare_native_rule!(
        bool,
        enabled,
        seed_bank_topmost,
        set_seed_bank_topmost,
        CONTROL_SEED_BANK_TOPMOST
    )
}

pub fn set_seed_recharge_shading_hidden(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::SeedRenderRulesBackend,
{
    request_seed_recharge_shading_hidden(Some(enabled))
}

/// `None` releases only the current source.
pub fn request_seed_recharge_shading_hidden(value: Option<bool>) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::SeedRenderRulesBackend,
{
    use rsvz_backend_api::SeedRenderRulesBackend;
    if CurrentBackend::SEED_PRESENTATION_IS_NOOP {
        return Ok(());
    }
    crate::control::request_native_rule!(
        bool,
        value,
        seed_recharge_shading_hidden,
        set_seed_recharge_shading_hidden,
        CONTROL_SEED_RECHARGE_SHADING_HIDDEN
    )
}

pub fn set_upgrade_planting_unrestricted(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::UpgradePlantingRuleBackend,
{
    request_upgrade_planting_unrestricted(Some(enabled))
}

/// `None` releases only the current source.
pub fn request_upgrade_planting_unrestricted(value: Option<bool>) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::UpgradePlantingRuleBackend,
{
    use rsvz_backend_api::UpgradePlantingRuleBackend;
    crate::control::request_native_rule!(
        bool,
        value,
        upgrade_planting_unrestricted,
        set_upgrade_planting_unrestricted,
        CONTROL_UPGRADE_PLANTING_UNRESTRICTED
    )
}

/// Optional fog presentation; release preserves remaining sources and baseline.
pub fn request_fog_revealed(value: Option<bool>) -> RuntimeResult<()>
where
    CurrentBackend: rsvz_backend_api::VisibilityEditBackend,
{
    use rsvz_backend_api::VisibilityEditBackend;
    if CurrentBackend::VISIBILITY_EDITS_ARE_NOOP {
        return Ok(());
    }
    crate::control::request_native_rule!(bool, value, fog_revealed, set_fog_revealed, CONTROL_FOG_REVEALED)
}

crate::control::rule_ledgers! {
    CONTROL_EASY_PLANTING_CHEAT: bool,
    CONTROL_PLANTING_RESTRICTIONS_IGNORED: bool,
    CONTROL_PLANT_SPECIAL_COUNTDOWN_RULE: rsvz_model::PlantSpecialCountdownRule,
    CONTROL_PLANT_DAMAGE_RULE: rsvz_model::PlantDamageRule,
    CONTROL_KERNEL_PULT_PROJECTILE_RULE: rsvz_model::KernelPultProjectileRule,
    CONTROL_COB_RECHARGE_SHORTENED: bool,
    CONTROL_COB_DRIFT_FIXED: bool,
    CONTROL_ITEM_DROP_DISABLED: bool,
    CONTROL_NATURAL_SUN_DROP_DISABLED: bool,
    CONTROL_JACK_EXPLOSION_RULE: rsvz_model::ZombieExplosionRule,
    CONTROL_PEPPER_EXPLOSION_RULE: rsvz_model::ZombieExplosionRule,
    CONTROL_SPECIAL_EVENTS_DISABLED: bool,
    CONTROL_ZOMBIE_SPAWN_STOPPED: bool,
    CONTROL_ZOMBIES_DIE_AT_HOUSE: bool,
    CONTROL_CRATERS_EXPIRE_IMMEDIATELY: bool,
    CONTROL_CHILLED_EFFECTS_DISABLED: bool,
    CONTROL_BUTTER_EFFECTS_DISABLED: bool,
    CONTROL_IMP_THROW_DISABLED: bool,
    CONTROL_ZOMBIE_WALKING_STOPPED: bool,
    CONTROL_ZOMBIE_DAMAGE_RULE: rsvz_model::ZombieDamageRule,
    CONTROL_SEED_BANK_TOPMOST: bool,
    CONTROL_SEED_RECHARGE_SHADING_HIDDEN: bool,
    CONTROL_UPGRADE_PLANTING_UNRESTRICTED: bool,
    CONTROL_FOG_REVEALED: bool,
    CONTROL_MUSHROOMS_AWAKE: bool,
    CONTROL_ICE_TRAILS_DISABLED: bool,
}
