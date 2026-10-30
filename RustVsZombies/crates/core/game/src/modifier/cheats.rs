//! Current cheat-rule operations.
use rsvz_backend_api::{CobRuleEditBackend, DanceModeBackend, MaidCheatsBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::MaidCheat;

/// Performs one native Dance-mode operation. Repeated calls restart eligible walkers.
pub fn set_dance_mode(enabled: bool)
where
    CurrentBackend: DanceModeBackend,
{
    crate::access::with_backend(|backend| {
        crate::live_value::read_or_abort(backend.set_dance_mode(enabled), "set_dance_mode")
    });
}

pub fn set_maid_cheat(cheat: MaidCheat)
where
    CurrentBackend: MaidCheatsBackend,
{
    crate::live_value::read_or_abort(
        crate::control::declare_native_rule!(
            rsvz_model::MaidCheat,
            cheat,
            maid_cheat,
            set_maid_cheat,
            CONTROL_MAID_CHEAT
        ),
        "set_maid_cheat",
    )
}

/// Enables or disables the native fixed cob-impact delay rule.
pub fn set_cob_fixed_delay(enabled: bool)
where
    CurrentBackend: CobRuleEditBackend,
{
    crate::live_value::read_or_abort(
        crate::control::declare_native_rule!(
            bool,
            enabled,
            cob_fixed_delay,
            set_cob_fixed_delay,
            CONTROL_COB_FIXED_DELAY
        ),
        "set_cob_fixed_delay",
    )
}

crate::control::rule_ledgers! {
    CONTROL_MAID_CHEAT: rsvz_model::MaidCheat,
    CONTROL_COB_FIXED_DELAY: bool,
}
