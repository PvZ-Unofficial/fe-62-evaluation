//! No-op configuration must not require a running world or install fake drawing.
#[cfg(feature = "pvz-emulator")]
#[test]
fn headless_overlay_configuration_validates_without_creating_a_task() {
    let mut options = rsvz::overlay::Options::default();
    options.info = rsvz::overlay::InfoMode::Advanced;
    options.index = rsvz::overlay::IndexMode::All;
    rsvz::overlay::configure(options).unwrap();
    rsvz::hud::set_hidden(true).unwrap();
    rsvz::hud::release().unwrap();
    assert!(rsvz_game::frame::runtime_timed_operations_idle());
    options.marker_duration = u32::MAX;
    assert!(rsvz::overlay::configure(options).is_err());
    assert!(rsvz_game::frame::runtime_timed_operations_idle());
}
