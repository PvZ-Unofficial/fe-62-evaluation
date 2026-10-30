//! Fixed native-to-runtime event instrumentation boundary.

use rsvz_model::model::{
    BeginPlantEffect, EventFrameStatus, EventInterest, EventToken, GargantuarAshHitFact, GargantuarSpawnedFact,
    HomeEntryFact, ImpThrownFact, PlantEffectAttemptFact, PlantEffectOutcome,
};

use crate::backend::Backend;

/// Fixed keyboard instrumentation ingress. Records facts only; never executes user code.
/// Packed event: VK bits 0..7, down 8, Ctrl/Alt/Shift/Meta 9..12, repeat 13,
/// focus 14, fast-forward 15. Zero resets edges without invoking release callbacks.
pub type KeyboardEventSink = fn(u32);

/// Copy-only callback table installed for one fight attempt.
///
/// These callbacks are runtime-owned function items. Backends must not retain
/// native object references or call them outside the installing thread.
#[derive(Clone, Copy)]
pub struct NativeEventSink {
    pub interest: EventInterest,
    pub begin_with_token: Option<fn(PlantEffectAttemptFact, EventToken) -> BeginPlantEffect>,
    pub adopt_decision: Option<fn(EventToken, rsvz_model::EventDecision)>,
    pub begin_logic_frame: fn(board_epoch: u64, main_counter: i32),
    pub begin_plant_effect: fn(PlantEffectAttemptFact) -> BeginPlantEffect,
    pub finish_plant_effect: fn(EventToken, PlantEffectOutcome),
    pub emit_activation: fn(rsvz_model::ActivationFact),
    pub emit_home_entry: fn(HomeEntryFact),
    pub emit_gargantuar_spawned: fn(GargantuarSpawnedFact),
    pub emit_imp_thrown: fn(ImpThrownFact),
    pub emit_gargantuar_ash_hit: fn(GargantuarAshHitFact),
    pub emit_gargantuar_motion: Option<fn(rsvz_model::GargantuarMotionFact)>,
    pub end_logic_frame: fn(EventFrameStatus),
}

/// Physical hook ownership needed by the generic runtime event model.
pub trait NativeEventBackend: Backend {
    fn install_native_event_sink(&mut self, sink: NativeEventSink) -> Result<(), Self::Error>;
    /// Replaces the shared interest/callbacks between native logical frames.
    /// On failure, retain or restore the previous sink before returning.
    fn update_native_event_sink(&mut self, sink: NativeEventSink) -> Result<(), Self::Error>;
    fn remove_native_event_sink(&mut self) -> Result<(), Self::Error>;
}

/// Fixed Copy-only event subscription for an independently linked native module.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleEventSink {
    pub interest: u32,
    pub begin_logic_frame: extern "C" fn(u64, i32),
    pub begin_plant_effect: extern "C" fn(PlantEffectAttemptFact, EventToken) -> BeginPlantEffect,
    pub adopt_decision: extern "C" fn(EventToken, rsvz_model::EventDecision),
    pub finish_plant_effect: extern "C" fn(EventToken, PlantEffectOutcome),
    pub emit_activation: extern "C" fn(rsvz_model::ActivationFact),
    pub emit_home_entry: extern "C" fn(HomeEntryFact),
    pub emit_gargantuar_spawned: extern "C" fn(GargantuarSpawnedFact),
    pub emit_imp_thrown: extern "C" fn(ImpThrownFact),
    pub emit_gargantuar_ash_hit: extern "C" fn(GargantuarAshHitFact),
    pub end_logic_frame: extern "C" fn(EventFrameStatus),
}
