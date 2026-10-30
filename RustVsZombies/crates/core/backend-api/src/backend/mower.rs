use crate::Backend;
use rsvz_model::LawnMowerId;

pub trait LawnMowerReadBackend: Backend {
    type MowerHandle<'a>: Copy
    where
        Self: 'a;
    type MowerIter<'a>: Iterator<Item = Self::MowerHandle<'a>>
    where
        Self: 'a;
    fn lawn_mowers(&self) -> Result<Self::MowerIter<'_>, Self::Error>;
    fn lawn_mower_id<'a>(&'a self, mower: Self::MowerHandle<'a>) -> LawnMowerId;
    fn lawn_mower_row<'a>(&'a self, mower: Self::MowerHandle<'a>) -> i32;
    fn lawn_mower_state<'a>(&'a self, mower: Self::MowerHandle<'a>) -> i32;
}
pub trait LawnMowerEditBackend: LawnMowerReadBackend {
    fn start_lawn_mower<'a>(&'a self, mower: Self::MowerHandle<'a>) -> Result<(), Self::Error>;
    /// Native Die, including native bonus-mower replacement. Does not recycle
    /// occupied slots; any replacement is outside the caller's original set.
    fn kill_lawn_mower<'a>(&'a self, mower: Self::MowerHandle<'a>) -> Result<(), Self::Error>;
}
/// Restore through native initialization: mode guards bypassed, visible at the
/// ready position. This creates new objects rather than resurrecting dead slots.
pub trait LawnMowerRestoreBackend: Backend {
    fn initialize_lawn_mowers_ready(&mut self) -> Result<(), Self::Error>;
}
