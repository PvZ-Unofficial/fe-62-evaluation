//! A-TAS warning predicates and automatic pause helpers.
mod queries;
pub use queries::{BalloonNotice, JackWarningMode, balloon_notice, card_usable_or_held, jack_should_pause};

use crate::{
    control::{self, OwnedRule},
    runtime::RuntimeResult,
};
use rsvz_backend_api::PlantPresenceBackend;
use rsvz_backend_api::{
    AdvancedPauseBackend, ClockBackend, CursorSeedReadBackend, FrameTimingBackend, GameUiBackend, HintCaptionBackend,
    PlantStateBackend, SceneBackend, SeedBankReadBackend, WorldIdentityBackend, ZombieRawFactsBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::AdvancedPauseOptions;
use rsvz_schedule::{
    state_hook::{StateHookHandle, with_state_hooks_ref},
    tick::{
        TickControl, TickHandle, TickLifetime, TickOptions, TickPhase, TickPriority, TickTaskState, with_scheduler,
    },
};
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy)]
struct Request<T> {
    value: T,
    pause: AdvancedPauseOptions,
}
impl<T> Request<T> {
    fn new(value: T, pause: AdvancedPauseOptions) -> Self {
        Self { value, pause }
    }
}
std::thread_local! {
    static JACK: RefCell<OwnedRule<Request<JackWarningMode>>> = const { RefCell::new(OwnedRule::new()) };
    static BALLOON: RefCell<OwnedRule<Request<bool>>> = const { RefCell::new(OwnedRule::new()) };
}
struct Counters {
    balloon: u16,
    pause: u32,
}
impl Default for Counters {
    fn default() -> Self {
        Self {
            balloon: 200,
            pause: 480,
        }
    }
}
std::thread_local! {
    static COUNTERS: RefCell<Counters> = RefCell::new(Counters::default());
    static TASK: Cell<Option<TickHandle>> = const { Cell::new(None) };
    static CLEANUP: Cell<Option<StateHookHandle>> = const { Cell::new(None) };
}

pub fn jack_mode() -> JackWarningMode {
    JACK.with_borrow(|rule| {
        rule.effective_declaration()
            .map_or(JackWarningMode::Off, |request| request.value)
    })
}
pub fn balloon_enabled() -> bool {
    BALLOON.with_borrow(|rule| rule.effective_declaration().is_some_and(|request| request.value))
}

/// Remaining A-TAS manual pause/step debounce. Callers choose whether to honor
/// it; ordinary game_pause/single_step APIs retain their existing behavior.
pub fn remaining_pause_delay_ms() -> u32 {
    COUNTERS.with_borrow(|state| 480_u32.saturating_sub(state.pause))
}

pub fn set_jack(mode: JackWarningMode, pause: AdvancedPauseOptions) -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend
        + ClockBackend
        + CursorSeedReadBackend
        + FrameTimingBackend
        + GameUiBackend
        + HintCaptionBackend
        + PlantStateBackend
        + PlantPresenceBackend
        + SceneBackend
        + SeedBankReadBackend
        + WorldIdentityBackend
        + ZombieRawFactsBackend,
{
    control::declare(
        &JACK,
        Request::new(mode, pause),
        || Ok(Request::new(JackWarningMode::Off, AdvancedPauseOptions::default())),
        |_| Ok(()),
    )?;
    if mode != JackWarningMode::Off {
        ensure_task();
    }
    Ok(())
}
pub fn set_balloon(enabled: bool, pause: AdvancedPauseOptions) -> RuntimeResult<()>
where
    CurrentBackend: AdvancedPauseBackend
        + ClockBackend
        + CursorSeedReadBackend
        + FrameTimingBackend
        + GameUiBackend
        + HintCaptionBackend
        + PlantStateBackend
        + PlantPresenceBackend
        + SceneBackend
        + SeedBankReadBackend
        + WorldIdentityBackend
        + ZombieRawFactsBackend,
{
    control::declare(
        &BALLOON,
        Request::new(enabled, pause),
        || Ok(Request::new(false, AdvancedPauseOptions::default())),
        |_| Ok(()),
    )?;
    if enabled {
        ensure_task();
    }
    Ok(())
}
pub fn release_jack() -> RuntimeResult<()> {
    control::release(
        &JACK,
        || Ok(Request::new(JackWarningMode::Off, AdvancedPauseOptions::default())),
        |_| Ok(()),
    )
}
pub fn release_balloon() -> RuntimeResult<()> {
    control::release(
        &BALLOON,
        || Ok(Request::new(false, AdvancedPauseOptions::default())),
        |_| Ok(()),
    )
}

fn pause<T>(request: Request<T>) -> RuntimeResult<bool>
where
    CurrentBackend: AdvancedPauseBackend + ClockBackend + GameUiBackend + WorldIdentityBackend,
{
    let apply = || {
        crate::game_pause::set_with_options(true, request.pause)?;
        COUNTERS.with_borrow_mut(|state| state.pause = 0);
        Ok(true)
    };
    apply()
}

fn ensure_task()
where
    CurrentBackend: AdvancedPauseBackend
        + ClockBackend
        + CursorSeedReadBackend
        + FrameTimingBackend
        + GameUiBackend
        + HintCaptionBackend
        + PlantStateBackend
        + PlantPresenceBackend
        + SceneBackend
        + SeedBankReadBackend
        + WorldIdentityBackend
        + ZombieRawFactsBackend,
{
    if !CLEANUP
        .get()
        .is_some_and(|handle| with_state_hooks_ref(|hooks| hooks.is_registered(handle)))
    {
        CLEANUP.set(Some(crate::state_hook::register_native_cleanup(i32::MAX, || {
            COUNTERS.set(Counters::default());
            Ok(())
        })));
    }
    with_scheduler(|scheduler| {
        if TASK
            .get()
            .is_some_and(|handle| scheduler.state(handle) != TickTaskState::Stopped)
        {
            return;
        }
        TASK.set(Some(
            scheduler.spawn(
                TickOptions::any_dispatch()
                    .lifetime(TickLifetime::Session)
                    .priority(TickPriority::HIGH)
                    .idle_neutral()
                    .name("automatic_warnings"),
                |meta| {
                    let jack = JACK.with_borrow(|rule| rule.declaration());
                    let balloon = BALLOON.with_borrow(|rule| rule.declaration());
                    if meta.phase == TickPhase::Playing && meta.is_new_frame {
                        // A-TAS runs JackPause before BalloonPause in its fight runner.
                        if let Some(request) = jack
                            && request.value != JackWarningMode::Off
                            && jack_should_pause(request.value)?
                        {
                            pause(request)?;
                        }
                        if let Some(request) = balloon
                            && request.value
                        {
                            let ready = COUNTERS.with_borrow_mut(|state| {
                                state.balloon = (state.balloon + 1).min(200);
                                state.balloon == 200
                            });
                            if ready && balloon_notice()?.is_some() && pause(request)? {
                                COUNTERS.with_borrow_mut(|state| state.balloon = 0);
                            }
                        }
                    }
                    // GLOBAL debounce continues while advanced pause freezes game time,
                    // and even after the triggering warning has been disabled.
                    if COUNTERS.with_borrow(|state| state.pause < 480) {
                        let duration = crate::access::with_backend(|backend| backend.native_frame_duration_ms())
                            .map_err(crate::access::operation_error)?;
                        COUNTERS.with_borrow_mut(|state| state.pause = state.pause.saturating_add(duration));
                    }
                    if meta.phase == TickPhase::Playing
                        && balloon.is_some_and(|request| request.value)
                        && let Some(notice) = balloon_notice()?
                    {
                        crate::caption::hint(notice.text(), 2)?;
                    }
                    let requested = JACK.with_borrow(|rule| {
                        rule.declaration()
                            .is_some_and(|request| request.value != JackWarningMode::Off)
                    }) || BALLOON
                        .with_borrow(|rule| rule.declaration().is_some_and(|request| request.value));
                    if !requested && COUNTERS.with_borrow(|state| state.pause >= 480) {
                        Ok(TickControl::Stop)
                    } else {
                        Ok(TickControl::Continue)
                    }
                },
            ),
        ));
    });
}
