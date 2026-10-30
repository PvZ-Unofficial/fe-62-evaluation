use rsvz::core::model::{KeyCode, KeyChord};
use rsvz::key::{self, KeyBindOptions};
fn register() -> rsvz::runtime::RuntimeResult<()> {
    let first = key::on_press(KeyCode::Q, || Ok(()))?;
    let second = key::on_release(KeyCode::Q, || Ok(()))?;
    assert_ne!(first, second);
    let chord = KeyChord::new(Some(KeyCode::W), KeyChord::CONTROL).unwrap();
    let _ = key::bind(chord, KeyBindOptions::new(), || Ok(()))?;
    rsvz::with_scheduler(|s| { s.stop(first); s.pause(second); s.resume(second); });
    Ok(())
}
fn main() {
    // Native registration requires the real host; the live input script covers execution.
    let _register: fn() -> rsvz::runtime::RuntimeResult<()> = register;
}
