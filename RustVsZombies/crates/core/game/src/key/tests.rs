use super::*;
fn event(key: KeyCode, down: bool, modifiers: u8, repeat: bool) -> u32 {
    u32::from(key.raw_virtual_key())
        | (u32::from(down) << 8)
        | (u32::from(modifiers) << 9)
        | (u32::from(repeat) << 13)
        | (1 << 14)
}
#[test]
fn physical_edges_repeat_chords_focus_and_queue_boundaries() {
    let chord = KeyChord::new(Some(KeyCode::K), KeyChord::CONTROL).unwrap();
    let mut edge = Edge::default();
    assert!(!edge.accept(chord, event(KeyCode::K, true, 0, false), false));
    assert!(!edge.accept(chord, event(KeyCode::CONTROL, true, 1, false), false));
    assert!(!edge.accept(chord, event(KeyCode::K, true, 1, true), false));
    assert!(!edge.accept(chord, event(KeyCode::K, false, 1, false), false));
    assert!(edge.accept(chord, event(KeyCode::K, true, 1, false), false));
    assert!(!edge.accept(chord, 0, false)); // focus loss never invokes a callback
    assert!(!edge.accept(chord, event(KeyCode::CONTROL, true, 1, false), false));
    assert!(edge.accept(chord, event(KeyCode::K, true, 1, false), false));
    assert!(edge.accept(chord, event(KeyCode::K, false, 1, false), true));
    let mut q = Queue::new();
    for n in 0..128 {
        q.push(n);
    }
    for n in 0..128 {
        assert_eq!(q.take().unwrap(), Some(n));
    }
    assert_eq!(q.take().unwrap(), None);
    for n in 0..129 {
        q.push(n);
    }
    assert!(q.take().is_err());
    assert_eq!(q.take().unwrap(), None);
}

#[test]
fn chords_do_not_fall_through_to_single_keys_on_either_release_order() {
    let single = KeyChord::new(Some(KeyCode::R), 0).unwrap();
    let combo = KeyChord::new(Some(KeyCode::R), KeyChord::CONTROL).unwrap();
    for release_control_first in [false, true] {
        let mut plain_edge = Edge::default();
        let mut combo_edge = Edge::default();
        let mut press = |key, down, modifiers, expected| {
            let input = event(key, down, modifiers, false);
            assert_eq!(
                (
                    plain_edge.accept(single, input, false),
                    combo_edge.accept(combo, input, false),
                ),
                expected,
            );
        };
        press(KeyCode::CONTROL, true, 1, (false, false));
        press(KeyCode::R, true, 1, (false, true));
        if release_control_first {
            press(KeyCode::CONTROL, false, 0, (false, false));
            press(KeyCode::R, false, 0, (false, false));
        } else {
            press(KeyCode::R, false, 1, (false, false));
            press(KeyCode::CONTROL, false, 0, (false, false));
        }
        press(KeyCode::R, true, 0, (true, false));
        press(KeyCode::CONTROL, true, 1, (false, false));
        press(KeyCode::CONTROL, false, 0, (false, false));
        press(KeyCode::R, false, 0, (false, false));
        press(KeyCode::R, true, 0, (true, false));
    }
}

#[test]
fn modifier_changes_repeats_and_focus_do_not_rearm_a_held_primary_key() {
    let chord = KeyChord::new(Some(KeyCode::R), KeyChord::CONTROL).unwrap();
    let mut edge = Edge::default();
    assert!(!edge.accept(chord, event(KeyCode::R, true, 5, false), false));
    assert!(!edge.accept(chord, event(KeyCode::SHIFT, false, 1, false), false));
    assert!(!edge.accept(chord, event(KeyCode::R, true, 1, true), false));
    assert!(!edge.accept(chord, event(KeyCode::R, false, 1, false), false));
    assert!(edge.accept(chord, event(KeyCode::R, true, 1, false), false));
    assert!(!edge.accept(chord, event(KeyCode::SHIFT, true, 5, false), false));
    assert!(!edge.accept(chord, event(KeyCode::SHIFT, false, 1, false), false));
    assert!(!edge.accept(chord, event(KeyCode::R, true, 1, false), false));
    assert!(!edge.accept(chord, 0, false));
    assert!(!edge.accept(chord, event(KeyCode::R, true, 1, true), false));
    assert!(!edge.accept(chord, event(KeyCode::R, false, 1, false), false));
    assert!(edge.accept(chord, event(KeyCode::R, true, 1, false), false));
}

#[test]
fn modifier_only_bindings_still_fire_on_press() {
    for (key, mask) in [(KeyCode::SHIFT, KeyChord::SHIFT), (KeyCode::ALT, KeyChord::ALT)] {
        let chord = KeyChord::new(None, mask).unwrap();
        let mut edge = Edge::default();
        assert!(edge.accept(chord, event(key, true, mask, false), false));
        assert!(!edge.accept(chord, event(key, true, mask, true), false));
        assert!(!edge.accept(chord, event(key, false, 0, false), false));
        assert!(edge.accept(chord, event(key, true, mask, false), false));
    }
}

#[test]
fn native_input_is_queued_once_and_reset_discards_pending_input() {
    discard_events();
    INSTALLED.set(true);
    record(17);
    assert_eq!(take_event().unwrap(), Some(17));
    assert_eq!(take_event().unwrap(), None);
    record(23);
    discard_events();
    assert_eq!(take_event().unwrap(), None);
    INSTALLED.set(false);
    record(29);
    assert_eq!(take_event().unwrap(), None);
}
