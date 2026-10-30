//! Resource operations for the selected backend.

/// PvZ natural-sun aging value used by the stable-drop policy.
pub const STABLE_NATURAL_SUN_GENERATED: i32 = 52;
/// Canonical disabled natural-sun countdown used by deterministic worlds.
pub const STABLE_NATURAL_SUN_COUNTDOWN: i32 = 425;

use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::{
    CoinProfileBackend, DropRuleEditBackend, PickupLimitsBackend, SeedRuleEditBackend, SunCostRuleEditBackend,
    SunWriteBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::NonNegativeI32;

pub fn coins() -> RuntimeResult<u32>
where
    CurrentBackend: CoinProfileBackend,
{
    crate::access::with_backend(|backend| backend.coins()).map_err(crate::access::operation_error)
}

/// Uses the native storage unit (one unit represents ten displayed coins).
pub fn set_coins(value: u32) -> RuntimeResult<()>
where
    CurrentBackend: CoinProfileBackend,
{
    i32::try_from(value).map_err(|_| RuntimeError::new("coins must fit in a nonnegative i32"))?;
    crate::access::with_backend(|backend| backend.set_coins(value)).map_err(crate::access::operation_error)
}

pub fn sunlight_limit() -> RuntimeResult<u32>
where
    CurrentBackend: PickupLimitsBackend,
{
    crate::access::with_backend(|backend| backend.sunlight_limit()).map_err(crate::access::operation_error)
}
pub fn set_sunlight_limit(value: u32) -> RuntimeResult<()>
where
    CurrentBackend: PickupLimitsBackend,
{
    i32::try_from(value).map_err(|_| RuntimeError::new("sunlight limit must fit in a nonnegative i32"))?;
    crate::control::declare_native_rule!(u32, value, sunlight_limit, set_sunlight_limit, CONTROL_SUNLIGHT_LIMIT)
}
pub fn money_limit() -> RuntimeResult<u32>
where
    CurrentBackend: PickupLimitsBackend,
{
    crate::access::with_backend(|backend| backend.money_limit()).map_err(crate::access::operation_error)
}
pub fn set_money_limit(value: u32) -> RuntimeResult<()>
where
    CurrentBackend: PickupLimitsBackend,
{
    i32::try_from(value).map_err(|_| RuntimeError::new("money limit must fit in a nonnegative i32"))?;
    crate::control::declare_native_rule!(u32, value, money_limit, set_money_limit, CONTROL_MONEY_LIMIT)
}

/// Enables or disables native seed cooldown checks.
pub fn set_seed_recharge_ignored(enabled: bool)
where
    CurrentBackend: SeedRuleEditBackend,
{
    crate::live_value::read_or_abort(
        request_seed_recharge_ignored(Some(enabled)),
        "set_seed_recharge_ignored",
    )
}

/// `None` releases this source's declaration; it does not declare false/normal.
pub fn request_seed_recharge_ignored(value: Option<bool>) -> RuntimeResult<()>
where
    CurrentBackend: SeedRuleEditBackend,
{
    crate::control::request_native_rule!(
        bool,
        value,
        seed_recharge_ignored,
        set_seed_recharge_ignored,
        CONTROL_SEED_RECHARGE_IGNORED
    )
}

/// Sets the current battle sun resource.
pub fn set_sun(value: u32) -> RuntimeResult<()>
where
    CurrentBackend: SunWriteBackend,
{
    i32::try_from(value).map_err(|_| RuntimeError::new("sun must fit in a non-negative i32"))?;
    crate::access::with_backend(|backend| backend.set_sun(value)).map_err(crate::access::operation_error)
}

/// Enables or disables native sun-cost checks.
pub fn set_sun_cost_ignored(enabled: bool)
where
    CurrentBackend: SunCostRuleEditBackend,
{
    crate::live_value::read_or_abort(request_sun_cost_ignored(Some(enabled)), "set_sun_cost_ignored")
}

/// `None` releases this source's declaration; it does not declare false/normal.
pub fn request_sun_cost_ignored(value: Option<bool>) -> RuntimeResult<()>
where
    CurrentBackend: SunCostRuleEditBackend,
{
    crate::control::request_native_rule!(
        bool,
        value,
        sun_cost_ignored,
        set_sun_cost_ignored,
        CONTROL_SUN_COST_IGNORED
    )
}

/// Stabilizes natural-sun aging using the shared PvZ policy value.
pub fn stabilize_natural_sun_drop_aging()
where
    CurrentBackend: DropRuleEditBackend,
{
    super::set_natural_sun_generated(
        NonNegativeI32::new(STABLE_NATURAL_SUN_GENERATED).expect("non-negative policy constant"),
    );
}

/// Disables natural falling sun and gives backends the same dormant state.
pub fn stabilize_natural_sun_drop()
where
    CurrentBackend: DropRuleEditBackend,
{
    super::set_natural_sun_drop_disabled(true);
    stabilize_natural_sun_drop_aging();
    super::set_natural_sun_countdown(
        NonNegativeI32::new(STABLE_NATURAL_SUN_COUNTDOWN).expect("non-negative policy constant"),
    );
}

crate::control::rule_ledgers! {
    CONTROL_SUNLIGHT_LIMIT: u32,
    CONTROL_MONEY_LIMIT: u32,
    CONTROL_SEED_RECHARGE_IGNORED: bool,
    CONTROL_SUN_COST_IGNORED: bool,
}
