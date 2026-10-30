//! Native game-canvas callbacks. Painting never advances timeline or game logic.
use crate::runtime::{RuntimeError, RuntimeResult};
pub use rsvz_backend_api::PaintCanvas;
use rsvz_backend_api::{
    BoardReadinessBackend, GameUiBackend, NativePaintBackend, OptionalOverlayBackend, WaveTimingBackend,
};
use rsvz_current::CurrentBackend;
pub use rsvz_model::{Argb, PaintFont};
use rsvz_schedule::{TickControl, TickHandle, TickLane, TickOptions};
use std::cell::{Cell, RefCell};

thread_local! {
    static INSTALLED: Cell<bool> = const { Cell::new(false) };
}
std::thread_local! {
    static PENDING_ERROR:RefCell<Option<RuntimeError>>=const{RefCell::new(None)};
}
fn defer_error(error: RuntimeError) {
    PENDING_ERROR.with_borrow_mut(|pending| {
        if pending.is_none() {
            *pending = Some(error);
        }
    });
}
/// Register an actual native painting callback, with the requested lifecycle.
/// PE intentionally has no NativePaintBackend implementation.
pub fn on_paint(
    options: TickOptions, callback: impl FnMut(&mut dyn PaintCanvas) -> RuntimeResult<()> + 'static,
) -> RuntimeResult<TickHandle>
where
    CurrentBackend: NativePaintBackend + BoardReadinessBackend + GameUiBackend + WaveTimingBackend,
{
    register(options, callback)
}

pub(crate) fn register(
    options: TickOptions, mut callback: impl FnMut(&mut dyn PaintCanvas) -> RuntimeResult<()> + 'static,
) -> RuntimeResult<TickHandle>
where
    CurrentBackend: OptionalOverlayBackend + BoardReadinessBackend + GameUiBackend + WaveTimingBackend,
{
    if CurrentBackend::OVERLAY_IS_NOOP {
        return Err(RuntimeError::new(
            "native paint callbacks are unavailable on this backend",
        ));
    }
    ensure_entry()?;
    let mut unavailable_reported = false;
    Ok(crate::tick::spawn(
        options.lane(TickLane::Paint).idle_neutral(),
        move |_| {
            let result = match CurrentBackend::with_paint_canvas(|canvas| {
                canvas.set_clip(None)?;
                callback(canvas)
            }) {
                Ok(result) => {
                    unavailable_reported = false;
                    result
                }
                Err(error) => {
                    if !unavailable_reported {
                        defer_error(crate::access::operation_error(error));
                        unavailable_reported = true;
                    }
                    // Device loss occurs before the user callback. Retry on a
                    // later native paint without repeatedly logging the outage.
                    return Ok(TickControl::Continue);
                }
            };
            match result {
                Ok(()) => Ok(TickControl::Continue),
                Err(error) => {
                    PENDING_ERROR.with_borrow_mut(|pending| {
                        if pending.is_none() {
                            *pending = Some(error);
                        }
                    });
                    // A native modal error dialog must never run on the draw stack.
                    // The caller may resume the task after correcting the cause.
                    Ok(TickControl::Pause)
                }
            }
        },
    ))
}

fn ensure_entry() -> RuntimeResult<()>
where
    CurrentBackend: OptionalOverlayBackend + BoardReadinessBackend + GameUiBackend + WaveTimingBackend,
{
    if INSTALLED.get() {
        return Ok(());
    }
    crate::access::with_backend(|backend| backend.set_paint_entry(Some(dispatch)))
        .map_err(crate::access::operation_error)?;
    crate::state_hook::register_native_cleanup(i32::MAX, || {
        drop(PENDING_ERROR.take());
        release()
    });
    crate::tick::spawn(
        TickOptions::any_dispatch()
            .lifetime(rsvz_schedule::TickLifetime::Session)
            .idle_neutral()
            .name("paint_errors"),
        |_| {
            if let Some(error) = PENDING_ERROR.with_borrow_mut(Option::take) {
                crate::diagnostics::report_operation_error(error);
            }
            Ok(TickControl::Continue)
        },
    );
    INSTALLED.set(true);
    Ok(())
}

fn release() -> RuntimeResult<()>
where
    CurrentBackend: OptionalOverlayBackend,
{
    crate::access::with_backend(|backend| backend.set_paint_entry(None)).map_err(crate::access::operation_error)?;
    INSTALLED.set(false);
    Ok(())
}

fn dispatch(backend: &mut CurrentBackend)
where
    CurrentBackend: OptionalOverlayBackend + BoardReadinessBackend + GameUiBackend + WaveTimingBackend,
{
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::modules::with_local_source(|| {
            if !INSTALLED.get() || crate::session::script_stop_requested() {
                return;
            }
            CurrentBackend::scope_backend(backend, || {
                // Shared access prevents a paint callback from replacing the world.
                crate::access::with_backend(|_| {
                    let result = crate::runtime::runtime_frame_facts().map(|facts| {
                        crate::tick::dispatch_scheduler_paint_reporting(
                            rsvz_schedule::TickMeta {
                                phase: facts.phase,
                                game_ui: facts.game_ui,
                                clock: facts.clock,
                                is_new_frame: true,
                            },
                            &mut defer_error,
                        )
                    });
                    match result {
                        Ok(rsvz_schedule::TickDispatchResult::Continue) => {}
                        Ok(rsvz_schedule::TickDispatchResult::TaskError(error)) | Err(error) => {
                            crate::session::fail_script(error);
                        }
                        Ok(_) => crate::session::fail_script(RuntimeError::new("native paint callback panicked")),
                    }
                });
            });
        })
    }));
    if let Err(payload) = outcome {
        // Paint is a separate native ingress: a user stop is consumed here,
        // after canvas/backend guards unwind. The next update performs cleanup.
        if !rsvz_schedule::timeline::is_timeline_termination(&*payload) {
            std::panic::resume_unwind(payload);
        }
    }
}
