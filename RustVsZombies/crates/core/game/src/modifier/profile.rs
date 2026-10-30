//! PT profile operations; ordinary nonzero challenge records/consumables survive.
use crate::runtime::RuntimeResult;
use rsvz_backend_api::{GameUiBackend, HiddenPagesBackend, ProfileProgressBackend, TemporaryUnlockBackend};
use rsvz_current::CurrentBackend;

pub fn set_temporary_unlock_enabled(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: TemporaryUnlockBackend,
{
    crate::control::declare_persistent(
        &CONTROL_TEMPORARY_UNLOCK_ENABLED,
        enabled,
        || {
            crate::access::with_backend(|backend| backend.temporary_unlock_enabled())
                .map_err(crate::access::operation_error)
        },
        |value| {
            crate::with_backend(|backend| backend.set_temporary_unlock_enabled(value))
                .map_err(crate::access::operation_error)
        },
    )
}

/// Controls page-button visibility when the native challenge screen is created.
/// Does not change the player's unlock progress or the current screen's state.
pub fn set_hidden_pages_visible(enabled: bool) -> RuntimeResult<()>
where
    CurrentBackend: HiddenPagesBackend,
{
    crate::control::declare_native_rule!(
        bool,
        enabled,
        hidden_pages_visible,
        set_hidden_pages_visible,
        CONTROL_HIDDEN_PAGES_VISIBLE
    )
}

pub fn unlock_gold_sunflower() -> RuntimeResult<()>
where
    CurrentBackend: ProfileProgressBackend + GameUiBackend,
{
    crate::with_backend(|backend| {
        backend
            .ensure_profile_writable()
            .map_err(crate::access::operation_error)?;
        let completed = backend.finished_adventure().map_err(crate::access::operation_error)?;
        if (completed as i32) < 2 {
            backend
                .set_finished_adventure(2)
                .map_err(crate::access::operation_error)?;
        }
        for index in (15..48).chain(50..59).chain(60..69) {
            if backend
                .challenge_record(index)
                .map_err(crate::access::operation_error)?
                == 0
            {
                backend
                    .set_challenge_record(index, 1)
                    .map_err(crate::access::operation_error)?;
            }
        }
        for index in 0..10 {
            backend
                .set_challenge_record(index, if index < 5 { 5 } else { 10 })
                .map_err(crate::access::operation_error)?;
        }
        if completed == 0 && backend.game_ui().map_err(crate::access::operation_error)? == rsvz_model::GameUi::Menu {
            backend
                .sync_game_selector(true)
                .map_err(crate::access::operation_error)?;
        }
        Ok(())
    })
}

pub fn unlock_shop_items() -> RuntimeResult<()>
where
    CurrentBackend: ProfileProgressBackend,
{
    crate::access::with_backend(|backend| {
        backend
            .ensure_profile_writable()
            .map_err(crate::access::operation_error)?;
        for index in (0..9).chain([13, 16, 17, 18, 19, 20, 22, 23, 25, 27, 29]) {
            backend.set_purchase(index, 1).map_err(crate::access::operation_error)?;
        }
        backend.set_purchase(21, 4).map_err(crate::access::operation_error)?;
        for index in 10..13 {
            backend.set_purchase(index, 0).map_err(crate::access::operation_error)?;
        }
        for (index, value) in [(14, 1020), (15, 1020), (26, 1010), (28, 1010)] {
            if backend.purchase(index).map_err(crate::access::operation_error)? == 0 {
                backend
                    .set_purchase(index, value)
                    .map_err(crate::access::operation_error)?;
            }
        }
        Ok(())
    })
}

crate::control::rule_ledgers! {
    CONTROL_HIDDEN_PAGES_VISIBLE: bool,
    CONTROL_TEMPORARY_UNLOCK_ENABLED: bool,
}
