//! event operations for the selected backend.

use crate::diagnostics::report_runtime_error;
use crate::runtime::RuntimeError;
use crate::session::{DispatchOutcome, fail_script, record_dispatch_outcome, script_stop_requested};
use rsvz_backend_api::backend::NativeEventSink;
use rsvz_schedule::event::{EventDispatcher, PublicEventDispatch, with_events, with_events_ref};
use std::cell::Cell;

thread_local! {
    static NEXT_NATIVE_TOKEN: Cell<u32> = const { Cell::new(1) };
}

fn with_event_source(body: impl FnOnce()) {
    crate::modules::with_local_source(|| {
        if EVENT_SINK_INSTALLED.get() {
            body();
        }
    });
}

std::thread_local! {
    pub(crate) static EVENT_SINK_INSTALLED: Cell<bool> = const { Cell::new(false) };
    // The interest actually installed for this source outlives logical fight
    // callbacks, which are cleared before physical sink release.
    static INSTALLED_INTEREST: Cell<rsvz_model::EventInterest> = const { Cell::new(rsvz_model::EventInterest::from_raw(0)) };
}

pub(crate) fn dispatch_public_events(overflowed: bool, mut dispatch: Option<PublicEventDispatch>) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if overflowed {
            record_dispatch_outcome(DispatchOutcome::RecoverableError);
            report_runtime_error("public event queue overflowed; newest events were dropped");
        }
        let Some(dispatch) = dispatch.as_mut() else {
            return false;
        };
        loop {
            let callback = with_events(|events| events.take_next_public_callback(dispatch));
            let Some(callback) = callback else {
                break false;
            };
            let outcome = EventDispatcher::run_public_callback_catching(callback);
            match with_events(|events| events.finish_public_callback(outcome)) {
                rsvz_schedule::event::PublicEventCallbackResult::Continue => {}
                rsvz_schedule::event::PublicEventCallbackResult::Aborted(error) => {
                    record_dispatch_outcome(DispatchOutcome::RecoverableError);
                    report_runtime_error(error);
                }
                rsvz_schedule::event::PublicEventCallbackResult::Panicked => {
                    record_dispatch_outcome(DispatchOutcome::RecoverableError);
                    report_runtime_error("public event callback panicked and was removed");
                }
                rsvz_schedule::event::PublicEventCallbackResult::TimelineTerminated => break true,
            }
            if script_stop_requested() {
                break false;
            }
        }
    }));
    if let Some(dispatch) = dispatch {
        with_events(|events| events.end_public_dispatch(dispatch));
    }
    match result {
        Ok(true) => rsvz_schedule::timeline::terminate_timeline_callback(),
        Ok(false) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

pub(crate) fn native_event_sink(interest: rsvz_model::EventInterest) -> NativeEventSink {
    NativeEventSink {
        interest,
        begin_with_token: Some(native_event_begin_with_token),
        adopt_decision: Some(native_event_adopt_decision),
        begin_logic_frame: native_event_begin_logic_frame,
        begin_plant_effect: native_event_begin_plant_effect,
        finish_plant_effect: native_event_finish_plant_effect,
        emit_home_entry: native_event_emit_home_entry,
        emit_activation: native_event_emit_activation,
        emit_gargantuar_spawned: native_event_emit_gargantuar_spawned,
        emit_imp_thrown: native_event_emit_imp_thrown,
        emit_gargantuar_ash_hit: native_event_emit_gargantuar_ash_hit,
        emit_gargantuar_motion: Some(native_event_emit_gargantuar_motion),
        end_logic_frame: native_event_end_logic_frame,
    }
}

fn native_event_begin_logic_frame(board_epoch: u64, main_counter: i32) {
    with_event_source(|| {
        native_event_call((), || {
            with_events(|events| events.begin_logic_frame(board_epoch, main_counter));
        })
    });
}

fn native_event_begin_plant_effect(fact: rsvz_model::PlantEffectAttemptFact) -> rsvz_model::BeginPlantEffect {
    use rsvz_model::EventToken;
    let raw = NEXT_NATIVE_TOKEN.get();
    NEXT_NATIVE_TOKEN.set(raw.wrapping_add(1).max(1));
    native_event_begin_with_token(fact, EventToken::from_raw(raw))
}

fn native_event_begin_with_token(
    fact: rsvz_model::PlantEffectAttemptFact, token: rsvz_model::EventToken,
) -> rsvz_model::BeginPlantEffect {
    crate::modules::with_local_source(|| {
        if !EVENT_SINK_INSTALLED.get() {
            return rsvz_model::BeginPlantEffect::FAIL_OPEN;
        }
        native_event_call(rsvz_model::BeginPlantEffect::FAIL_OPEN, || {
            with_events(|events| events.begin_plant_effect_with_token(fact, token))
        })
    })
}

fn native_event_adopt_decision(token: rsvz_model::EventToken, decision: rsvz_model::EventDecision) {
    with_event_source(|| {
        native_event_call((), || {
            with_events(|events| events.adopt_effect_decision(token, decision))
        })
    });
}

fn native_event_finish_plant_effect(token: rsvz_model::EventToken, outcome: rsvz_model::PlantEffectOutcome) {
    if !token.is_valid() {
        return;
    }
    with_event_source(|| {
        native_event_call((), || {
            with_events(|events| events.finish_plant_effect(token, outcome));
        })
    });
}

fn native_event_emit_gargantuar_motion(fact: rsvz_model::GargantuarMotionFact) {
    with_event_source(|| {
        native_event_call((), || {
            with_events(|events| events.emit_gargantuar_motion(fact));
        })
    });
}

fn native_event_emit_home_entry(fact: rsvz_model::HomeEntryFact) {
    with_event_source(|| {
        native_event_call((), || {
            with_events(|events| events.emit_home_entry(fact));
        })
    });
}

fn native_event_emit_gargantuar_spawned(fact: rsvz_model::GargantuarSpawnedFact) {
    with_event_source(|| {
        native_event_call((), || {
            with_events(|events| events.emit_gargantuar_spawned(fact));
        })
    });
}

fn native_event_emit_imp_thrown(fact: rsvz_model::ImpThrownFact) {
    with_event_source(|| {
        native_event_call((), || {
            with_events(|events| events.emit_imp_thrown(fact));
        })
    });
}

fn native_event_emit_gargantuar_ash_hit(fact: rsvz_model::GargantuarAshHitFact) {
    with_event_source(|| {
        native_event_call((), || {
            with_events(|events| events.emit_gargantuar_ash_hit(fact));
        })
    });
}

fn native_event_end_logic_frame(status: rsvz_model::EventFrameStatus) {
    with_event_source(|| {
        native_event_call((), || {
            with_events(|events| events.end_logic_frame(status));
        })
    });
}

pub(crate) fn set_native_event_membership(enabled: bool) -> Result<(), RuntimeError>
where
    rsvz_current::CurrentBackend: rsvz_backend_api::NativeEventBackend,
{
    use rsvz_backend_api::NativeEventBackend;
    use rsvz_current::CurrentBackend;
    let old = INSTALLED_INTEREST.get();
    let requested = if enabled {
        with_events_ref(EventDispatcher::interest)
    } else {
        rsvz_model::EventInterest::default()
    };
    let next = requested;
    if old != next {
        CurrentBackend::with_backend(|backend| {
            if next.is_empty() {
                backend.remove_native_event_sink()
            } else if old.is_empty() {
                backend.install_native_event_sink(native_event_sink(next))
            } else {
                backend.update_native_event_sink(native_event_sink(next))
            }
        })
        .map_err(|error| RuntimeError::new(error.to_string()))?;
    }
    INSTALLED_INTEREST.set(requested);
    EVENT_SINK_INSTALLED.set(enabled && !requested.is_empty());
    Ok(())
}

fn native_event_call<R: Copy>(fallback: R, call: impl FnOnce() -> R) -> R {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(call));
    let Ok(value) = result else {
        fail_script(RuntimeError::new("native event ingress panicked or reentered"));
        return fallback;
    };
    if let Some(fault) = with_events_ref(EventDispatcher::fault) {
        fail_script(RuntimeError::new(fault.to_string()));
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_observer_runs_after_the_native_frame_and_preserves_identity() {
        use rsvz_model::{BattleStatus, GargantuarMotionFact, ZombieId};
        use std::rc::Rc;
        with_events(EventDispatcher::begin_script_generation);
        let observed = Rc::new(Cell::new(None));
        let result = Rc::clone(&observed);
        on_gargantuar_motion(EventOptions::new(), move |fact| result.set(Some(*fact))).unwrap();
        assert_eq!(
            with_events(|events| events.freeze()).unwrap(),
            EventInterest::GARGANTUAR_MOTION
        );
        EVENT_SINK_INSTALLED.set(true);
        native_event_begin_logic_frame(1, 1);
        let fact = GargantuarMotionFact {
            zombie_id: ZombieId::from_raw(65545),
            main_counter: 1,
            reasons: 2,
        };
        native_event_emit_gargantuar_motion(fact);
        assert_eq!(observed.get(), None);
        native_event_end_logic_frame(BattleStatus::Running);
        dispatch_public_events(false, with_events_ref(EventDispatcher::begin_public_dispatch));
        assert_eq!(observed.get(), Some(fact));
        EVENT_SINK_INSTALLED.set(false);
        with_events(EventDispatcher::clear_session);
    }

    #[test]
    fn general_observer_receives_activation_events() {
        use rsvz_model::{ActivationFact, ActivationKind, BattleStatus, Grid, PixelPos};
        use std::rc::Rc;
        with_events(EventDispatcher::begin_script_generation);
        let calls = Rc::new(Cell::new(0));
        let observed = Rc::clone(&calls);
        on(EventOptions::default(), move |event| {
            if matches!(event, GameEvent::Activation(_)) {
                observed.set(observed.get() + 1);
            }
        })
        .unwrap();
        assert!(
            with_events(|events| events.freeze())
                .unwrap()
                .contains(EventInterest::ACTIVATION)
        );
        EVENT_SINK_INSTALLED.set(true);
        native_event_begin_logic_frame(1, 1);
        native_event_emit_activation(ActivationFact {
            kind: ActivationKind::Ice,
            grid: Grid { row: 0, col: 0 },
            position: PixelPos { x: 40, y: 80 },
            target_x: 80,
            main_counter: 1,
            wave: 0,
            refresh_countdown: 100,
            initial_countdown: 100,
        });
        native_event_end_logic_frame(BattleStatus::Running);
        dispatch_public_events(false, with_events_ref(EventDispatcher::begin_public_dispatch));
        assert_eq!(calls.get(), 1);
        EVENT_SINK_INSTALLED.set(false);
        with_events(EventDispatcher::clear_session);
    }

    #[test]
    fn native_effect_is_dispatched_once_and_ignores_a_detached_sink() {
        use rsvz_model::{
            BattleStatus, Grid, PlantEffect, PlantEffectAttemptFact, PlantEffectOutcome, PlantEffectSource, PlantId,
            PlantKind, ZombieId,
        };
        use std::rc::Rc;
        let calls = Rc::new(Cell::new(0));
        with_events(EventDispatcher::begin_script_generation);
        let count = Rc::clone(&calls);
        on_plant_effect(EventOptions::default(), move |_| count.set(count.get() + 1)).unwrap();
        with_events(|events| events.freeze()).unwrap();
        EVENT_SINK_INSTALLED.set(true);
        native_event_begin_logic_frame(1, 1);
        let fact = PlantEffectAttemptFact {
            source: PlantEffectSource::Bite(ZombieId::from_raw(1)),
            plant_id: PlantId::from_raw(2),
            raw_kind: PlantKind::WallNut,
            effective_kind: PlantKind::WallNut,
            grid: Grid::new(0, 0).unwrap(),
            hp_before: 4000,
            max_hp: 4000,
            effect: PlantEffect::HpDamage { native_requested: 4 },
            main_counter: 1,
        };
        let token = rsvz_model::EventToken::from_raw(41);
        let began = native_event_begin_with_token(fact, token);
        assert_eq!(began.token, token);
        native_event_adopt_decision(token, rsvz_model::EventDecision::SuppressByMeasurement);
        native_event_finish_plant_effect(token, PlantEffectOutcome::SuppressedByMeasurement);
        native_event_end_logic_frame(BattleStatus::Running);
        assert!(with_events_ref(EventDispatcher::fault).is_none());
        dispatch_public_events(false, with_events_ref(EventDispatcher::begin_public_dispatch));
        assert_eq!(calls.get(), 1);
        EVENT_SINK_INSTALLED.set(false);
        assert!(!native_event_begin_plant_effect(fact).token.is_valid());
        with_events(EventDispatcher::clear_session);
    }

    #[test]
    fn logical_reset_does_not_forget_the_physical_sink() {
        EVENT_SINK_INSTALLED.set(true);
        crate::frame::reset_runtime_state_preserving_backend();
        assert!(EVENT_SINK_INSTALLED.get());
        EVENT_SINK_INSTALLED.set(false);
    }
}

pub fn install_internal_event_interceptor(
    interceptor: Box<dyn rsvz_schedule::event::InternalEventInterceptor>,
) -> crate::runtime::RuntimeResult<()> {
    with_events(|events| events.install_internal_interceptor(interceptor))
        .map_err(|error| RuntimeError::new(error.to_string()))
}

use rsvz_model::{EventInterest, GameEvent, HomeEntryEvent, PlantEffectEvent, PlantEffectSource};
use rsvz_schedule::event::{EventCommandOutcome, EventHandle, EventOptions, EventRegistrationError};

pub fn on<F>(options: EventOptions, callback: F) -> Result<EventHandle, EventRegistrationError>
where
    F: FnMut(&GameEvent) + 'static,
{
    with_events(|events| {
        events.register_public_handler(
            EventInterest::PLANT_EFFECT
                .union(EventInterest::HOME_ENTRY)
                .union(EventInterest::ACTIVATION),
            options,
            Box::new(callback),
        )
    })
}

/// Opt-in movement invalidation. PE implements this interest; other native backends reject it.
pub fn on_gargantuar_motion<F>(options: EventOptions, mut callback: F) -> Result<EventHandle, EventRegistrationError>
where
    F: FnMut(&rsvz_model::GargantuarMotionFact) + 'static,
{
    with_events(|events| {
        events.register_public_handler(
            EventInterest::GARGANTUAR_MOTION,
            options,
            Box::new(move |event| {
                if let GameEvent::GargantuarMotion(fact) = event {
                    callback(fact);
                }
            }),
        )
    })
}

pub fn on_plant_effect<F>(options: EventOptions, mut callback: F) -> Result<EventHandle, EventRegistrationError>
where
    F: FnMut(&PlantEffectEvent) + 'static,
{
    with_events(|events| {
        events.register_public_handler(
            EventInterest::PLANT_EFFECT,
            options,
            Box::new(move |event| {
                if let GameEvent::PlantEffect(event) = event {
                    callback(event);
                }
            }),
        )
    })
}

pub fn on_gargantuar_smash<F>(options: EventOptions, mut callback: F) -> Result<EventHandle, EventRegistrationError>
where
    F: FnMut(&PlantEffectEvent) + 'static,
{
    with_events(|events| {
        events.register_public_handler(
            EventInterest::GARGANTUAR,
            options,
            Box::new(move |event| {
                if let GameEvent::PlantEffect(event) = event
                    && matches!(event.source, PlantEffectSource::Gargantuar(_))
                {
                    callback(event);
                }
            }),
        )
    })
}

pub fn on_home_entry<F>(options: EventOptions, mut callback: F) -> Result<EventHandle, EventRegistrationError>
where
    F: FnMut(&HomeEntryEvent) + 'static,
{
    with_events(|events| {
        events.register_public_handler(
            EventInterest::HOME_ENTRY,
            options,
            Box::new(move |event| {
                if let GameEvent::HomeEntry(event) = event {
                    callback(event);
                }
            }),
        )
    })
}

pub fn remove(handle: EventHandle) -> EventCommandOutcome {
    with_events(|events| events.remove_public_handler(handle))
}
pub fn reserve(capacity: usize) -> Result<(), EventRegistrationError> {
    with_events(|events| events.reserve_public_events(capacity))
}

fn native_event_emit_activation(fact: rsvz_model::ActivationFact) {
    with_event_source(|| {
        native_event_call((), || with_events(|events| events.emit_activation(fact)));
    });
}
