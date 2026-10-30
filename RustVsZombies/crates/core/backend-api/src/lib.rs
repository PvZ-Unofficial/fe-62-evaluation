//! Safe-facing backend capability traits.
//!
//! Infallible Board reads without an entity handle require the concrete backend's
//! active Board access scope. Scope acquisition is fallible; bypassing it is a
//! low-level contract error and must panic safely, never dereference a null pointer.
//! Game-facing operations establish that scope and preserve callback-local failure
//! handling. Entity borrows retain their lifetime constraints; absent IDs return
//! `None`. UI restrictions, invalid values and genuine native failures retain `Result`.

pub mod access;
pub mod artifact;
pub mod diagnostics;
pub mod error;
pub mod opening;

pub mod backend {
    mod base;

    pub mod advanced_pause;
    pub mod event;
    pub mod fast_forward;
    pub mod gameplay;
    pub mod grid_item;
    pub mod item;
    pub mod mower;
    pub mod projectile;
    pub mod seed;

    pub mod scene;
    pub use scene::TransientClearBackend;
    pub mod replay;
    pub use replay::{NativeCheckpoint, NativeCheckpointBackend, ReplayInput};
    pub mod assist_control;
    pub use assist_control::*;
    pub mod user_card;
    pub use user_card::*;
    pub mod keyboard;
    pub mod modifier;
    pub mod plant;
    pub mod read;
    pub mod state;
    pub mod surface;
    pub mod timing;
    pub mod zombie;

    pub use advanced_pause::AdvancedPauseBackend;
    pub use base::{Backend, BattleStatusBackend, DeadObjectReclaimBackend, WorldIdentityBackend};

    pub use event::{NativeEventBackend, NativeEventSink};
    pub use fast_forward::{FastForwardBackend, SeedChooserFastForwardBackend};
    pub use gameplay::{
        BoardSupportBackend, CurrentNaturalSpawnBackend, LawnMowerClearBackend, SceneEditBackend, SceneLayoutBackend,
        SpawnScheduleBackend, SpawnScheduleReadBackend, SunMoneyBackend, WorldResetBackend, pool_has_free_slot,
        pool_has_reserved_slot,
    };
    pub use grid_item::{
        ChallengeGravePlacementBackend, GridItemCreateBackend, GridItemEditBackend, GridItemPoolBackend,
        GridItemReadBackend, GridItemStateBackend, RakePlacementBackend,
    };
    pub use item::{AutoCollectBackend, ItemClickCollectBackend, ItemReadBackend};
    pub use keyboard::KeyboardStateBackend;
    pub use modifier::{
        CobImpactDelayBackend, CobRuleEditBackend, CoinProfileBackend, CommonZombieDanceBackend,
        CraterLifetimeRuleBackend, DanceModeBackend, DancerClockWriteBackend, DropRuleEditBackend,
        HiddenModeUnlockBackend, HiddenPagesBackend, IceTrailClearBackend, IceTrailRuleBackend, MaidCheatsBackend,
        PickupLimitsBackend, PlantDamageRuleEditBackend, PlantEffectRuleEditBackend, PlantingRuleEditBackend,
        ProfileProgressBackend, ProfileReadonlyBackend, ProjectileRuleEditBackend, SeedRenderRulesBackend,
        SeedRuleEditBackend, SunCostRuleEditBackend, SunWriteBackend, TemporaryUnlockBackend, TrophyUnlockBackend,
        UpgradePlantingRuleBackend, VisibilityEditBackend, ZombieBehaviorRulesBackend, ZombieDamageRuleEditBackend,
        ZombieRuleEditBackend,
    };
    pub use mower::{LawnMowerEditBackend, LawnMowerReadBackend, LawnMowerRestoreBackend};
    pub use plant::{
        CobFireBackend, IZombiePlantStyleBackend, ImitatorMorphBackend, NativePlantPlacementBackend,
        PlantContactBackend, PlantCostBackend, PlantCreateBackend, PlantEffectCountdownWriteBackend,
        PlantHealthWriteBackend, PlantIdleAnimationBackend, PlantPlacementBackend, PlantPoolBackend,
        PlantPresenceBackend, PlantReadBackend, PlantRemoveBackend, PlantSleepBackend, PlantSlotEditBackend,
        PlantStateBackend, PlantStateCountdownWriteBackend, PlantStateWriteBackend, PlantVisualStateBackend,
    };
    pub use projectile::ProjectileReadBackend;
    pub mod paint;
    pub use paint::{GameHudBackend, HudField, NativePaintBackend, OptionalOverlayBackend, PaintCanvas};
    pub use read::{
        BoardReadinessBackend, ClockBackend, CurrentWaveBackend, CursorQueryBackend, CursorSeedReadBackend,
        FrameTimingBackend, GameUiBackend, GridGeometryBackend, GridTerrainBackend, MouseStateBackend,
        NativeGameModeBackend, SceneBackend, SunQueryBackend,
    };
    pub use seed::{
        CardAppendSelectionBackend, CardSelectionReadBackend, ChooserCooldownReadBackend, SeedBankEditBackend,
        SeedBankReadBackend, SeedBankVisibilityBackend, SeedChooserControlBackend, SeedCooldownReadBackend,
        SeedDefinitionBackend, SeedPacketBackend,
    };
    pub use state::{BackendIdentityBackend, BoardStateBackend, RandomControlBackend, SunProductionModeBackend};
    pub use surface::{
        AudioBackend, BattleEntryBackend, DisplayBackend, GameSpeedHintBackend, GardenBackend, HintCaptionBackend,
        InputBackend, MainMenuBackend, NativeCaptionReadBackend, NativeLevelFlowBackend, NativeNavigationBackend,
        WindowRulesBackend,
    };
    pub use timing::{RefreshCountdownWriteBackend, WaveHealthBackend, WaveRefreshControlBackend, WaveTimingBackend};
    pub use zombie::{
        ZombieBodyHealthWriteBackend, ZombieContactBackend, ZombieCreateBackend, ZombieKillBackend,
        ZombiePhaseCountdownWriteBackend, ZombiePoolBackend, ZombiePositionWriteBackend, ZombiePresenceBackend,
        ZombieRawFactsBackend, ZombieReadBackend, ZombieRemoveBackend, ZombieRowTranslationBackend, ZombieStateBackend,
        ZombieStatusWriteBackend, ZombieVerticalPositionBackend, ZombieXWriteBackend,
    };
}

pub use backend::*;
