//! Event-driven keyboard bindings; callbacks run at the ordinary safe dispatch boundary.
use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::{FastForwardBackend, KeyboardStateBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{KeyChord, KeyCode};
use rsvz_schedule::{TickControl, TickHandle, TickLane, TickOptions};
use std::cell::{Cell, RefCell};

/// Options for a keyboard binding. Fast-forward permits only explicitly marked cancel actions.
#[derive(Clone, Copy, Debug)]
pub struct KeyBindOptions {
    pub tick_options: TickOptions,
    pub on_release: bool,
    pub cancel_fast_forward: bool,
}
impl KeyBindOptions {
    pub const fn new() -> Self {
        Self {
            tick_options: TickOptions::active_phase(),
            on_release: false,
            cancel_fast_forward: false,
        }
    }
    pub const fn tick_options(mut self, value: TickOptions) -> Self {
        self.tick_options = value;
        self
    }
    pub const fn cancel_fast_forward(mut self, value: bool) -> Self {
        self.cancel_fast_forward = value;
        self
    }
}
impl Default for KeyBindOptions {
    fn default() -> Self {
        Self::new()
    }
}

// Fixed-size pending input facts, not a mirror of game keyboard state.
struct Queue {
    entries: [u32; 128],
    head: usize,
    len: usize,
    overflow: bool,
}
impl Queue {
    const fn new() -> Self {
        Self {
            entries: [0; 128],
            head: 0,
            len: 0,
            overflow: false,
        }
    }
    fn push(&mut self, event: u32) {
        if self.len == self.entries.len() {
            self.overflow = true;
            return;
        }
        self.entries[(self.head + self.len) % self.entries.len()] = event;
        self.len += 1;
    }
    fn take(&mut self) -> RuntimeResult<Option<u32>> {
        if self.overflow {
            *self = Self::new();
            return Err(RuntimeError::new("keyboard event queue overflow"));
        }
        if self.len == 0 {
            return Ok(None);
        }
        let event = self.entries[self.head];
        self.head = (self.head + 1) % self.entries.len();
        self.len -= 1;
        Ok(Some(event))
    }
}
thread_local! { static INSTALLED: Cell<bool> = const {Cell::new(false)}; }
std::thread_local! {
    static QUEUE: RefCell<Queue> = const {RefCell::new(Queue::new())};
    static EVENT: Cell<u32> = const {Cell::new(0)};
}
// Packed Copy fact: VK[0:7], down[8], Ctrl/Alt/Shift/Meta[9:12], repeat[13], focus[14], skipping[15].
fn record(event: u32) {
    if INSTALLED.get() {
        QUEUE.with_borrow_mut(|q| q.push(event));
    }
}
pub(crate) fn take_event() -> RuntimeResult<Option<u32>> {
    if !INSTALLED.get() {
        return Ok(None);
    }
    QUEUE.with_borrow_mut(Queue::take)
}
pub(crate) fn discard_events() {
    QUEUE.with_borrow_mut(|queue| *queue = Queue::new());
    EVENT.set(0);
}
pub(crate) fn set_event(event: u32) {
    EVENT.set(event);
}
fn ensure_entry() -> RuntimeResult<()>
where
    CurrentBackend: KeyboardStateBackend,
{
    if INSTALLED.get() {
        return Ok(());
    }
    crate::access::with_backend(|b| b.set_key_event_sink(Some(record))).map_err(crate::access::operation_error)?;
    QUEUE.with_borrow_mut(|q| *q = Queue::new());
    crate::state_hook::register_native_cleanup(i32::MAX, || {
        crate::access::with_backend(|b| b.set_key_event_sink(None)).map_err(crate::access::operation_error)?;
        INSTALLED.set(false);
        QUEUE.with_borrow_mut(|q| *q = Queue::new());
        EVENT.set(0);
        Ok(())
    });
    INSTALLED.set(true);
    Ok(())
}

#[derive(Default)]
struct Edge {
    primary_down: bool,
    chord_down: bool,
}
impl Edge {
    fn accept(&mut self, chord: KeyChord, event: u32, release: bool) -> bool {
        if event & (1 << 14) == 0 {
            *self = Self::default();
            return false;
        }
        let key = (event & 255) as u16;
        let down = event & (1 << 8) != 0;
        let repeat = event & (1 << 13) != 0;
        let primary_press = match chord.key() {
            Some(primary) if primary.raw_virtual_key() == key => {
                let fresh = down && !self.primary_down;
                self.primary_down = down;
                fresh
            }
            Some(_) => false,
            None => true,
        };
        let pressed = (chord.key().is_none() || self.primary_down) && ((event >> 9) & 15) as u8 == chord.modifiers();
        let fire = !repeat
            && if release {
                self.chord_down && !pressed
            } else {
                // Releasing Ctrl while R remains held must not synthesize an R press.
                primary_press && !self.chord_down && pressed
            };
        self.chord_down = pressed;
        fire
    }
}
/// Register a single-key press. Native autorepeat never repeats the callback.
pub fn on_press<F>(key: KeyCode, callback: F) -> RuntimeResult<TickHandle>
where
    CurrentBackend: KeyboardStateBackend + FastForwardBackend,
    F: FnMut() -> RuntimeResult<()> + 'static,
{
    bind(single_key(key)?, KeyBindOptions::new(), callback)
}
/// Register a single-key release.
pub fn on_release<F>(key: KeyCode, callback: F) -> RuntimeResult<TickHandle>
where
    CurrentBackend: KeyboardStateBackend + FastForwardBackend,
    F: FnMut() -> RuntimeResult<()> + 'static,
{
    bind(
        single_key(key)?,
        KeyBindOptions {
            on_release: true,
            ..KeyBindOptions::new()
        },
        callback,
    )
}
fn single_key(key: KeyCode) -> RuntimeResult<KeyChord> {
    let modifier = match key.raw_virtual_key() {
        0x10 | 0xa0 | 0xa1 => 4,
        0x11 | 0xa2 | 0xa3 => 1,
        0x12 | 0xa4 | 0xa5 => 2,
        0x5b | 0x5c => 8,
        _ => 0,
    };
    KeyChord::new(if modifier == 0 { Some(key) } else { None }, modifier).map_err(|e| RuntimeError::new(e.to_string()))
}
/// Register a chord with lifecycle options. Cancel bindings must only cancel while skipping.
/// Press bindings with a primary key match modifiers on its initial key-down;
/// changing modifiers while that key is held does not trigger another press.
pub fn bind<F>(chord: KeyChord, options: KeyBindOptions, mut callback: F) -> RuntimeResult<TickHandle>
where
    CurrentBackend: KeyboardStateBackend + FastForwardBackend,
    F: FnMut() -> RuntimeResult<()> + 'static,
{
    ensure_entry()?;
    let mut edge = Edge::default();
    Ok(crate::tick::spawn(
        options
            .tick_options
            .lane(TickLane::Input)
            .frame_gate(rsvz_schedule::TickFrameGate::EveryDispatch)
            .trigger(rsvz_schedule::TickTrigger::Repeating),
        move |_| {
            let event = EVENT.get();
            if edge.accept(chord, event, options.on_release) {
                let skipping = event & (1 << 15) != 0 || crate::access::with_backend(|b| b.fast_forward_active());
                if !skipping || options.cancel_fast_forward {
                    callback()?;
                }
            }
            Ok(TickControl::Continue)
        },
    ))
}

#[cfg(test)]
mod tests;
