// Run as a bare script with `rsvz run-portable ... --close-game-on-finish`.
#[rsvz::script]
fn script() -> rsvz::runtime::RuntimeResult<()> {
    use rsvz::prelude::*;
    use rsvz::runtime::RuntimeError;
    use rsvz::tick::{TickControl, TickOptions};

    select_cards("P");
    rsvz::setup::skip_seed_chooser();
    rsvz::tick::spawn(TickOptions::playing_frame(), |_| {
        rsvz::with_backend(|backend| -> Result<(), RuntimeError> {
            backend.set_profile_readonly(true)?;
            backend.back_to_main_menu()?;
            assert_eq!(backend.game_ui()?, rsvz::core::model::GameUi::Menu);
            assert!(backend.sun().unwrap_err().is_board_unavailable());
            let original = backend.sun_cost_ignored()?;
            backend.set_sun_cost_ignored(!original)?;
            assert_eq!(backend.sun_cost_ignored()?, !original);
            backend.set_sun_cost_ignored(original)?;
            backend.set_seed_recharge_ignored(true)?;
            assert!(backend.seed_recharge_ignored()?);
            backend.set_seed_recharge_ignored(false)?;
            backend.set_profile_readonly(true)?;
            Ok(())
        })?;
        rsvz::stop_script();
        Ok(TickControl::Stop)
    });
    Ok(())
}
