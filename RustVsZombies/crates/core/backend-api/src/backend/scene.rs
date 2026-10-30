use super::Backend;
/// Destruction of one native object family, including its attachments.
pub trait TransientClearBackend: Backend {
    fn clear_native_projectiles(&mut self) -> Result<(), Self::Error>;
    fn clear_native_items(&mut self) -> Result<(), Self::Error>;
}
