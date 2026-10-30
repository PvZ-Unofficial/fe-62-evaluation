use rsvz_backend_api::artifact::SessionArtifact;
use rsvz_backend_api::error::RuntimeError;
use rsvz_model::SessionShard;

use crate::PeBackend;

#[derive(Clone, Copy)]
pub struct DispatchInput {
    pub shard: SessionShard,
    pub stop_requested: bool,
    pub completed_rounds: u64,
}

impl DispatchInput {
    /// Delivers boundaries retained while a logical module could not be dispatched.
    pub fn with_boundaries(mut self, world_replaced: bool, completed_rounds: u64) -> Self {
        debug_assert!(!world_replaced);
        self.completed_rounds = completed_rounds;
        self
    }
    pub const fn script_dispatch_allowed(self) -> bool {
        true
    }
    #[must_use]
    pub const fn session_shard(self) -> SessionShard {
        self.shard
    }

    #[must_use]
    pub const fn world_replaced(self) -> bool {
        false
    }

    #[must_use]
    pub const fn completed_rounds(self) -> u64 {
        self.completed_rounds
    }

    #[must_use]
    pub const fn opening_ready(self) -> bool {
        true
    }

    #[must_use]
    pub const fn registration_safe_point_required(self) -> bool {
        false
    }

    #[must_use]
    pub const fn opening_transition_requires_native_update(self) -> bool {
        true
    }

    #[must_use]
    pub const fn main_ui_reload_supported(self) -> bool {
        false
    }
}

pub enum DispatchResult {
    /// No logical module is executing; preserve the native update policy.
    Idle,
    Continue,
    SkipUpdate,
    /// Explicit request for the native host to perform the existing initial entry.
    EnterBattle,
    Stop {
        cleanup_complete: bool,
        artifact: Option<SessionArtifact>,
        error: Option<RuntimeError>,
    },
}

pub type DispatchEntry = fn(&mut PeBackend, DispatchInput) -> DispatchResult;
