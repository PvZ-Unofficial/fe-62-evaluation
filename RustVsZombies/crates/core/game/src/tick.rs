//! tick operations for the selected backend.

use crate::runtime::RuntimeError;
use crate::runtime::RuntimeResult;
use crate::session::{DispatchOutcome, record_dispatch_outcome, script_stop_requested};
use rsvz_current::CurrentBackend;
use rsvz_schedule::tick::{TickControl, TickOptions};
use rsvz_schedule::tick::{TickDispatchResult, TickMeta, TickScheduler, with_scheduler};
use std::ops::ControlFlow;

pub fn dispatch_scheduler_tick(meta: TickMeta) -> TickDispatchResult
where
    rsvz_current::CurrentBackend:
        rsvz_backend_api::BoardReadinessBackend + rsvz_backend_api::GameUiBackend + rsvz_backend_api::WaveTimingBackend,
{
    dispatch_scheduler_tick_with_error_handler(meta, false, None, &mut ControlFlow::Break)
}

fn dispatch_scheduler_tick_with_error_handler(
    mut meta: TickMeta, finalizers_only: bool, lane_only: Option<rsvz_schedule::TickLane>,
    handle_error: &mut impl FnMut(RuntimeError) -> ControlFlow<RuntimeError>,
) -> TickDispatchResult
where
    rsvz_current::CurrentBackend:
        rsvz_backend_api::BoardReadinessBackend + rsvz_backend_api::GameUiBackend + rsvz_backend_api::WaveTimingBackend,
{
    let mut epoch = CurrentBackend::backend_access_epoch();
    if !finalizers_only && lane_only.is_none() {
        loop {
            if epoch != CurrentBackend::backend_access_epoch() {
                epoch = CurrentBackend::backend_access_epoch();
                if let Err(error) = refresh_meta(&mut meta) {
                    return TickDispatchResult::TaskError(error);
                }
            }
            match crate::key::take_event() {
                Ok(Some(event)) => {
                    crate::key::set_event(event);
                    let result = dispatch_scheduler_tick_with_error_handler(
                        meta,
                        false,
                        Some(rsvz_schedule::TickLane::Input),
                        handle_error,
                    );
                    crate::key::set_event(0);
                    if !matches!(result, TickDispatchResult::Continue) || script_stop_requested() {
                        return result;
                    }
                }
                Ok(None) => break,
                Err(error) => return TickDispatchResult::TaskError(error),
            }
        }
    }
    let mut order = with_scheduler(|scheduler| {
        if lane_only == Some(rsvz_schedule::TickLane::Input) {
            scheduler.begin_runtime_dispatch_input(meta)
        } else if lane_only == Some(rsvz_schedule::TickLane::Paint) {
            scheduler.begin_runtime_dispatch_paint(meta)
        } else if finalizers_only {
            scheduler.begin_runtime_finalizer_dispatch_tick(meta)
        } else {
            scheduler.begin_runtime_dispatch_tick(meta)
        }
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        loop {
            if epoch != CurrentBackend::backend_access_epoch() {
                epoch = CurrentBackend::backend_access_epoch();
                if let Err(error) = refresh_meta(&mut meta) {
                    break TickDispatchResult::TaskError(error);
                }
            }
            let callback = with_scheduler(|scheduler| scheduler.take_next_runtime_callback(&mut order, meta));
            let Some(callback) = callback else {
                break TickDispatchResult::Continue;
            };
            let outcome = TickScheduler::run_runtime_callback_catching(callback, meta);
            match with_scheduler(|scheduler| scheduler.finish_runtime_callback(outcome)) {
                None => {}
                Some(TickDispatchResult::TaskError(error)) => match handle_error(error) {
                    ControlFlow::Continue(()) => {}
                    ControlFlow::Break(error) => break TickDispatchResult::TaskError(error),
                },
                Some(result) => break result,
            }
            if script_stop_requested() {
                break TickDispatchResult::Continue;
            }
        }
    }));
    with_scheduler(|scheduler| scheduler.end_runtime_dispatch_tick(order));
    result.unwrap_or_else(|payload| {
        rsvz_schedule::timeline::resume_timeline_termination(payload);
        TickDispatchResult::TaskPanic
    })
}

// Input callbacks can replace the world just like ordinary scheduler callbacks.
// Revalidate before selecting callbacks for the next queued input event.
fn refresh_meta(meta: &mut TickMeta) -> RuntimeResult<()>
where
    CurrentBackend:
        rsvz_backend_api::BoardReadinessBackend + rsvz_backend_api::GameUiBackend + rsvz_backend_api::WaveTimingBackend,
{
    let facts = crate::runtime::runtime_frame_facts()?;
    meta.is_new_frame =
        facts.phase == rsvz_schedule::tick::TickPhase::Playing && (meta.is_new_frame || facts.clock != meta.clock);
    meta.phase = facts.phase;
    meta.game_ui = facts.game_ui;
    meta.clock = facts.clock;
    Ok(())
}

/// Dispatches current runtime tasks while reporting ordinary task errors and
/// continuing with the remaining tasks from the same frame.
///
/// A repeating task retains its registration after an error. One-shot tasks
/// consume their execution opportunity. Task panics remain terminal.
pub fn dispatch_scheduler_tick_reporting(
    meta: TickMeta, report_error: &mut impl FnMut(RuntimeError),
) -> TickDispatchResult
where
    rsvz_current::CurrentBackend:
        rsvz_backend_api::BoardReadinessBackend + rsvz_backend_api::GameUiBackend + rsvz_backend_api::WaveTimingBackend,
{
    dispatch_scheduler_tick_with_error_handler(meta, false, None, &mut |error| {
        record_dispatch_outcome(DispatchOutcome::RecoverableError);
        report_error(error);
        ControlFlow::Continue(())
    })
}

pub(crate) fn dispatch_scheduler_finalizers_reporting(
    meta: TickMeta, report_error: &mut impl FnMut(RuntimeError),
) -> TickDispatchResult
where
    rsvz_current::CurrentBackend:
        rsvz_backend_api::BoardReadinessBackend + rsvz_backend_api::GameUiBackend + rsvz_backend_api::WaveTimingBackend,
{
    dispatch_scheduler_tick_with_error_handler(meta, true, None, &mut |error| {
        record_dispatch_outcome(DispatchOutcome::RecoverableError);
        report_error(error);
        ControlFlow::Continue(())
    })
}

pub(crate) fn dispatch_scheduler_paint_reporting(
    meta: TickMeta, report_error: &mut impl FnMut(RuntimeError),
) -> TickDispatchResult
where
    CurrentBackend:
        rsvz_backend_api::BoardReadinessBackend + rsvz_backend_api::GameUiBackend + rsvz_backend_api::WaveTimingBackend,
{
    dispatch_scheduler_tick_with_error_handler(meta, false, Some(rsvz_schedule::TickLane::Paint), &mut |error| {
        report_error(error);
        ControlFlow::Continue(())
    })
}

pub fn spawn<F>(options: TickOptions, callback: F) -> rsvz_schedule::TickHandle
where
    F: FnMut(TickMeta) -> RuntimeResult<TickControl> + 'static,
{
    with_scheduler(|scheduler| scheduler.spawn(options, callback))
}

pub(crate) fn spawn_finalizer<F>(options: TickOptions, callback: F) -> rsvz_schedule::TickHandle
where
    F: FnMut(TickMeta) -> RuntimeResult<TickControl> + 'static,
{
    with_scheduler(|scheduler| scheduler.spawn_runtime_finalizer(options, callback))
}

/// Output accepted by callbacks with a scoped Frame.
pub trait FrameTickOutput {
    fn into_tick_result(self) -> RuntimeResult<TickControl>;
}
impl FrameTickOutput for () {
    fn into_tick_result(self) -> RuntimeResult<TickControl> {
        Ok(TickControl::Continue)
    }
}
impl FrameTickOutput for TickControl {
    fn into_tick_result(self) -> RuntimeResult<TickControl> {
        Ok(self)
    }
}
impl FrameTickOutput for RuntimeResult<()> {
    fn into_tick_result(self) -> RuntimeResult<TickControl> {
        self.map(|_| TickControl::Continue)
    }
}
impl FrameTickOutput for RuntimeResult<TickControl> {
    fn into_tick_result(self) -> RuntimeResult<TickControl> {
        self
    }
}

pub fn on_frame<O: FrameTickOutput>(
    mut callback: impl for<'frame> FnMut(crate::frame::Frame<'frame>) -> O + 'static,
) -> rsvz_schedule::TickHandle {
    spawn(TickOptions::playing_frame(), move |_| {
        crate::frame::with_frame(|frame| callback(frame).into_tick_result())
    })
}

pub fn spawn_frame<O: FrameTickOutput>(
    options: TickOptions, mut callback: impl for<'frame> FnMut(crate::frame::Frame<'frame>) -> O + 'static,
) -> RuntimeResult<rsvz_schedule::TickHandle> {
    if options.availability == rsvz_schedule::TickAvailability::AnyDispatch {
        return Err(RuntimeError::new(
            "Frame tick tasks require Playing or Active availability",
        ));
    }
    Ok(spawn(options, move |_| {
        crate::frame::with_frame(|frame| callback(frame).into_tick_result())
    }))
}
