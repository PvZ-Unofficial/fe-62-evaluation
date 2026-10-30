//! Current Board queries. Necessary read failures abort the calling operation.
use crate::{access::with_backend, live_value::read_or_abort};
use rsvz_backend_api::{RandomControlBackend, SceneBackend, SunQueryBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{RandomStreamKind, SceneKind};

pub fn sun() -> u32
where
    CurrentBackend: SunQueryBackend,
{
    with_backend(|backend| read_or_abort(backend.sun(), "failed to read sun"))
}

/// Only an absent Board produces None; access and other read faults still abort.
pub fn sun_if_available() -> Option<u32>
where
    CurrentBackend: SunQueryBackend,
{
    with_backend(|backend| match backend.sun() {
        Ok(value) => Some(value),
        Err(error) if error.is_board_unavailable() => None,
        Err(error) => crate::diagnostics::abort_operation(error.into()),
    })
}

pub fn scene() -> SceneKind
where
    CurrentBackend: SceneBackend,
{
    with_backend(|backend| read_or_abort(backend.scene(), "failed to read scene"))
}

pub fn random_locked(stream: RandomStreamKind) -> bool
where
    CurrentBackend: RandomControlBackend,
{
    with_backend(|backend| backend.random_locked(stream))
}

pub fn ice_path_x(row: u32) -> i32
where
    rsvz_current::CurrentBackend: rsvz_backend_api::BoardStateBackend,
{
    use rsvz_backend_api::BoardStateBackend;
    crate::access::with_backend(|backend| crate::live_value::read_or_abort(backend.ice_path_x(row), "ice_path_x"))
}

pub fn ice_path_countdown(row: u32) -> u32
where
    rsvz_current::CurrentBackend: rsvz_backend_api::BoardStateBackend,
{
    use rsvz_backend_api::BoardStateBackend;
    crate::access::with_backend(|backend| {
        crate::live_value::read_or_abort(backend.ice_path_countdown(row), "ice_path_countdown")
    })
}

pub fn natural_sun_generated() -> i32
where
    rsvz_current::CurrentBackend: rsvz_backend_api::BoardStateBackend,
{
    use rsvz_backend_api::BoardStateBackend;
    crate::access::with_backend(|backend| {
        crate::live_value::read_or_abort(backend.natural_sun_generated(), "natural_sun_generated")
    })
}

pub fn natural_sun_countdown() -> i32
where
    rsvz_current::CurrentBackend: rsvz_backend_api::BoardStateBackend,
{
    use rsvz_backend_api::BoardStateBackend;
    crate::access::with_backend(|backend| {
        crate::live_value::read_or_abort(backend.natural_sun_countdown(), "natural_sun_countdown")
    })
}

pub fn game_ui() -> rsvz_model::GameUi
where
    rsvz_current::CurrentBackend: rsvz_backend_api::GameUiBackend,
{
    use rsvz_backend_api::GameUiBackend;
    crate::access::with_backend(|backend| crate::live_value::read_or_abort(backend.game_ui(), "game_ui"))
}

pub fn cursor_type() -> i32
where
    rsvz_current::CurrentBackend: rsvz_backend_api::CursorQueryBackend,
{
    use rsvz_backend_api::CursorQueryBackend;
    crate::access::with_backend(|backend| crate::live_value::read_or_abort(backend.cursor_type(), "cursor_type"))
}

/// Current battle outcome, independent of the UI screen.
pub fn battle_status() -> rsvz_model::BattleStatus
where
    CurrentBackend: rsvz_backend_api::BattleStatusBackend,
{
    use rsvz_backend_api::BattleStatusBackend;
    with_backend(|backend| read_or_abort(backend.battle_status(), "battle_status"))
}

/// Current world identity for invalidating cross-world requests and selections.
/// A world lifetime change is distinct from a completed round or a clock rewind.
pub fn world_id() -> crate::runtime::RuntimeResult<Option<rsvz_model::WorldId>>
where
    rsvz_current::CurrentBackend: rsvz_backend_api::WorldIdentityBackend,
{
    use rsvz_backend_api::WorldIdentityBackend;
    crate::access::with_backend(|backend| backend.world_identity()).map_err(crate::access::operation_error)
}
