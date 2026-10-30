//! Replay indices count saved records; they are not game clocks.
use super::ids::WorldId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReplayMode {
    #[default]
    Idle,
    Recording,
    Playing,
    Recovering,
    Fault,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplayStatus {
    pub mode: ReplayMode,
    pub paused: bool,
    /// Non-interpolated playback stopped at the last snapshot with an unplayed input tail.
    pub tail_pending: bool,
    pub start_index: u64,
    /// Exclusive end, also the next recording index.
    pub end_index: u64,
    pub position: Option<u64>,
    pub interval_cs: u32,
    pub current_clock: Option<i32>,
    pub world: Option<WorldId>,
}
impl ReplayStatus {
    pub fn playback_owned(self) -> bool {
        matches!(
            self.mode,
            ReplayMode::Playing | ReplayMode::Recovering | ReplayMode::Fault
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ReplayRestoreKind {
    Seek,
    Recover,
}

/// Issued only after native restore and readiness checks have succeeded.
/// It invalidates old entity identities, without incrementing natural rounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct RestoreReceipt {
    pub sequence: u64,
    pub before: WorldId,
    pub after: WorldId,
    pub kind: ReplayRestoreKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplayCursor {
    pub x: i32,
    pub y: i32,
    pub kind: i32,
    pub buttons: i32,
}
