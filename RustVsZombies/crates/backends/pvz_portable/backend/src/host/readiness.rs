use std::cell::Cell;

use rsvz_backend_api::opening::SeedChooserOpeningAction;
use rsvz_model::GameUi;

use crate::{PortableBackend, PortableBackendError};

const CONTINUE_DIALOG_DELAY: u8 = 3;

thread_local! {
    static LAST_UI: Cell<Option<GameUi>> = const { Cell::new(None) };
    static AUTO_ENTRY_REQUESTED: Cell<bool> = const { Cell::new(false) };
    static CONTINUE_DIALOG_COUNTDOWN: Cell<Option<u8>> = const { Cell::new(None) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AfterDispatch {
    KeepSkip,
    AdvanceNative,
}

pub(super) fn before_dispatch(
    backend: &PortableBackend, stop_requested: bool,
) -> Result<SeedChooserOpeningAction, PortableBackendError> {
    if stop_requested {
        return Ok(SeedChooserOpeningAction::DispatchReady);
    }
    let ui = native::game_ui(backend)?;
    let previous = LAST_UI.replace(Some(ui));
    if returned_to_menu(previous, ui) {
        native::restore_game_speed()?;
    }
    if advance_continue_dialog(ui)? {
        return Ok(SeedChooserOpeningAction::AdvanceNative);
    }
    if is_menu(ui) && !returned_to_menu(previous, ui) && AUTO_ENTRY_REQUESTED.get() {
        return Ok(SeedChooserOpeningAction::AdvanceNative);
    }
    if ui == GameUi::Playing && native::pending_seed_selection(backend)? {
        return Ok(SeedChooserOpeningAction::AdvanceNative);
    }
    if ui != GameUi::LevelIntro {
        return Ok(SeedChooserOpeningAction::DispatchReady);
    }
    let readiness = native::seed_chooser_readiness(backend)?;
    Ok(readiness.opening_action())
}

pub(super) fn after_dispatch(backend: &mut PortableBackend) -> Result<AfterDispatch, PortableBackendError> {
    let ui = native::game_ui(backend)?;
    if !is_menu(ui) || AUTO_ENTRY_REQUESTED.get() {
        return Ok(AfterDispatch::KeepSkip);
    }
    match native::enter_game(backend) {
        Ok(()) => {
            AUTO_ENTRY_REQUESTED.set(true);
            CONTINUE_DIALOG_COUNTDOWN.set(Some(CONTINUE_DIALOG_DELAY));
            Ok(AfterDispatch::KeepSkip)
        }
        Err(error) if error.is_board_unavailable() => Ok(AfterDispatch::AdvanceNative),
        Err(error) => Err(error),
    }
}

fn advance_continue_dialog(ui: GameUi) -> Result<bool, PortableBackendError> {
    let Some(remaining) = CONTINUE_DIALOG_COUNTDOWN.get() else {
        return Ok(false);
    };
    let remaining = remaining.saturating_sub(1);
    if remaining > 0 {
        CONTINUE_DIALOG_COUNTDOWN.set(Some(remaining));
        return Ok(true);
    }
    let clicked = native::click_continue_dialog_if_present()?;
    if clicked || is_menu(ui) {
        CONTINUE_DIALOG_COUNTDOWN.set(Some(1));
        return Ok(true);
    }
    CONTINUE_DIALOG_COUNTDOWN.set(None);
    Ok(false)
}

fn returned_to_menu(previous: Option<GameUi>, current: GameUi) -> bool {
    is_menu(current) && previous.is_some_and(|previous| !is_menu(previous))
}

fn is_menu(ui: GameUi) -> bool {
    matches!(ui, GameUi::Loading | GameUi::Menu | GameUi::Challenge)
}

pub(super) fn clear() {
    LAST_UI.set(None);
    AUTO_ENTRY_REQUESTED.set(false);
    CONTINUE_DIALOG_COUNTDOWN.set(None);
    #[cfg(test)]
    {
        native::UI.set(GameUi::Playing);
        native::READINESS.set(rsvz_backend_api::opening::SeedChooserReadiness::Ready);
        native::PENDING_SELECTION.set(false);
        native::CLICKED.set(false);
        native::ENTRY_UNAVAILABLE.set(false);
        native::RESTORES.set(0);
        native::CLICKS.set(0);
        native::ENTRIES.set(0);
    }
}

// Only native I/O is substituted in unit tests; dispatch decisions above are shared.
#[cfg(not(test))]
mod native {
    use super::*;
    use rsvz_backend_api::backend::{BattleEntryBackend, GameUiBackend};
    use rsvz_backend_api::opening::SeedChooserReadiness;
    use rsvz_model::{BattleConfig, EndlessBattleConfig, SceneKind};

    pub(super) fn restore_game_speed() -> Result<(), PortableBackendError> {
        pvzp_rs::restore_game_speed().map_err(Into::into)
    }
    pub(super) fn click_continue_dialog_if_present() -> Result<bool, PortableBackendError> {
        pvzp_rs::click_continue_dialog_if_present().map_err(Into::into)
    }

    pub(super) fn game_ui(backend: &PortableBackend) -> Result<GameUi, PortableBackendError> {
        backend.game_ui()
    }
    pub(super) fn pending_seed_selection(backend: &PortableBackend) -> Result<bool, PortableBackendError> {
        let world = backend.world()?;
        // The Survival closing countdown is still playable; only the completed
        // Board may already be clearing/rebuilding its seed bank.
        Ok(world.level_complete())
    }
    pub(super) fn seed_chooser_readiness(
        backend: &PortableBackend,
    ) -> Result<SeedChooserReadiness, PortableBackendError> {
        backend.seed_chooser_readiness()
    }
    pub(super) fn enter_game(backend: &mut PortableBackend) -> Result<(), PortableBackendError> {
        backend.enter_game(BattleConfig::Endless(EndlessBattleConfig::new(SceneKind::Pool)))
    }
}

#[cfg(test)]
pub(super) mod native {
    use super::*;
    use rsvz_backend_api::opening::SeedChooserReadiness;

    thread_local! {
        pub static UI: Cell<GameUi> = const { Cell::new(GameUi::Playing) };
        pub static READINESS: Cell<SeedChooserReadiness> = const { Cell::new(SeedChooserReadiness::Ready) };
        pub static PENDING_SELECTION: Cell<bool> = const { Cell::new(false) };
        pub static CLICKED: Cell<bool> = const { Cell::new(false) };
        pub static RESTORES: Cell<u32> = const { Cell::new(0) };
        pub static CLICKS: Cell<u32> = const { Cell::new(0) };
        pub static ENTRIES: Cell<u32> = const { Cell::new(0) };
        pub static ENTRY_UNAVAILABLE: Cell<bool> = const { Cell::new(false) };
    }
    pub fn game_ui(_: &PortableBackend) -> Result<GameUi, PortableBackendError> {
        Ok(UI.get())
    }
    pub fn pending_seed_selection(_: &PortableBackend) -> Result<bool, PortableBackendError> {
        Ok(PENDING_SELECTION.get())
    }
    pub fn seed_chooser_readiness(_: &PortableBackend) -> Result<SeedChooserReadiness, PortableBackendError> {
        Ok(READINESS.get())
    }
    pub fn restore_game_speed() -> Result<(), PortableBackendError> {
        RESTORES.set(RESTORES.get() + 1);
        Ok(())
    }
    pub fn click_continue_dialog_if_present() -> Result<bool, PortableBackendError> {
        CLICKS.set(CLICKS.get() + 1);
        Ok(CLICKED.replace(false))
    }
    pub fn enter_game(_: &mut PortableBackend) -> Result<(), PortableBackendError> {
        ENTRIES.set(ENTRIES.get() + 1);
        if ENTRY_UNAVAILABLE.get() {
            Err(PortableBackendError::BoardUnavailable)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz_backend_api::opening::SeedChooserReadiness;

    #[test]
    fn menu_entry_dialog_and_return_follow_the_production_decisions() {
        let mut backend = PortableBackend::new();
        clear();
        native::UI.set(GameUi::Menu);
        native::ENTRY_UNAVAILABLE.set(true);
        assert_eq!(after_dispatch(&mut backend).unwrap(), AfterDispatch::AdvanceNative);
        native::ENTRY_UNAVAILABLE.set(false);
        assert_eq!(after_dispatch(&mut backend).unwrap(), AfterDispatch::KeepSkip);
        for _ in 0..2 {
            assert_eq!(
                before_dispatch(&backend, false).unwrap(),
                SeedChooserOpeningAction::AdvanceNative
            );
        }
        assert_eq!(native::CLICKS.get(), 0);
        native::CLICKED.set(true);
        assert_eq!(
            before_dispatch(&backend, false).unwrap(),
            SeedChooserOpeningAction::AdvanceNative
        );
        native::UI.set(GameUi::LevelIntro);
        assert_eq!(
            before_dispatch(&backend, false).unwrap(),
            SeedChooserOpeningAction::DispatchReady
        );
        assert_eq!(native::CLICKS.get(), 2);
        native::UI.set(GameUi::Playing);
        native::PENDING_SELECTION.set(true);
        assert_eq!(
            before_dispatch(&backend, false).unwrap(),
            SeedChooserOpeningAction::AdvanceNative
        );
        assert_eq!(
            before_dispatch(&backend, true).unwrap(),
            SeedChooserOpeningAction::DispatchReady
        );
        native::UI.set(GameUi::Menu);
        assert_eq!(
            before_dispatch(&backend, false).unwrap(),
            SeedChooserOpeningAction::DispatchReady
        );
        assert_eq!(native::RESTORES.get(), 1);
        assert_eq!(after_dispatch(&mut backend).unwrap(), AfterDispatch::KeepSkip);
        assert_eq!(native::ENTRIES.get(), 2);
        assert_eq!(
            before_dispatch(&backend, false).unwrap(),
            SeedChooserOpeningAction::AdvanceNative
        );
        clear();
    }

    #[test]
    fn chooser_waits_for_player_to_return_from_view_lawn() {
        let backend = PortableBackend::new();
        clear();
        native::UI.set(GameUi::LevelIntro);
        native::READINESS.set(SeedChooserReadiness::NotSeedChoosing);
        assert_eq!(
            before_dispatch(&backend, false).unwrap(),
            SeedChooserOpeningAction::DispatchPrepare
        );
        native::READINESS.set(SeedChooserReadiness::ViewLawnStable { time: 200 });
        assert_eq!(
            before_dispatch(&backend, false).unwrap(),
            SeedChooserOpeningAction::AdvanceNative
        );
        assert_eq!(
            native::READINESS.get(),
            SeedChooserReadiness::ViewLawnStable { time: 200 }
        );
        // Other readiness states are classified by backend-api's own tests.
        native::READINESS.set(SeedChooserReadiness::Ready);
        assert_eq!(
            before_dispatch(&backend, false).unwrap(),
            SeedChooserOpeningAction::DispatchReady
        );
        clear();
    }
}
