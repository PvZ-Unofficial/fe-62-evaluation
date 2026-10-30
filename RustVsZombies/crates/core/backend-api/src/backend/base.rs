use rsvz_model::model::BattleStatus;

/// Minimal backend identity.
///
/// Concrete game semantics are exposed through capability traits such as
/// `SceneBackend`, `PlantReadBackend`, or `SeedPacketBackend`. Specific backends can still use
/// raw pointers, ABI wrappers, hooks, or external process memory internally, but those details must
/// not enter safe-facing trait signatures.
pub trait Backend: Sized {
    /// 后端自己的错误类型。
    type Error: std::error::Error + 'static;
}

/// Identity of the current world, even while its game phase is not yet ready.
pub trait WorldIdentityBackend: Backend {
    fn world_identity(&self) -> Result<Option<rsvz_model::WorldId>, Self::Error>;
}

/// Native dead-object and effect-pool reclamation, without advancing game time. Requires
/// exclusive access at the outer update boundary; every borrowed entity is
/// invalidated just as at the native end-of-update deletion queue.
pub trait DeadObjectReclaimBackend: Backend {
    fn reclaim_dead_objects(&mut self) -> Result<(), Self::Error>;
}

/// Battle or simulation status capability, separated from host UI state.
pub trait BattleStatusBackend: Backend {
    /// Reads the current battle/simulation status if the backend can classify it.
    fn battle_status(&self) -> Result<BattleStatus, Self::Error>;
}
