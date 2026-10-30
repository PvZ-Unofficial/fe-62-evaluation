use crate::backend::Backend;
use rsvz_model::model::KeyCode;

/// Native keyboard facts and the fixed event ingress.
pub trait KeyboardStateBackend: Backend {
    /// Installs the fixed record-only keyboard ingress; never a user callback.
    fn set_key_event_sink(&self, sink: Option<crate::backend::event::KeyboardEventSink>) -> Result<(), Self::Error>;

    /// Returns whether `key` is currently physically down.
    fn key_is_down(&self, key: KeyCode) -> bool;

    /// Returns whether keyboard input should currently be accepted for this backend.
    fn input_focused(&self) -> Result<bool, Self::Error>;
}
