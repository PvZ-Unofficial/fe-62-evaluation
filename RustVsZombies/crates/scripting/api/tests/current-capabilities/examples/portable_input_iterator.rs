use rsvz::core::model::{MouseButton, PixelPos};
use rsvz_backend_api::*;
use rsvz_current::CurrentBackend;

fn borrowed(backend: &mut CurrentBackend) {
    let mut plants = backend.plants().unwrap();
    backend.mouse_up(PixelPos { x: 10, y: 10 }, MouseButton::Left).unwrap();
    let _ = plants.next();
}
fn main() {}
