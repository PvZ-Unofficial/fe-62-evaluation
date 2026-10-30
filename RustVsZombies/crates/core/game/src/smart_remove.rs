//! Smart removal with optional cosmetic highlighting.

use crate::logic::smart_remove::tick_smart_remove_with_visuals as tick_impl;
use crate::logic::{ContactGeometryBackend, smart_remove::SmartRemoveState};
use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::{
    PlantPresenceBackend, PlantReadBackend, PlantRemoveBackend, PlantVisualStateBackend, ZombieReadBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::SmartRemoveOptions;
use rsvz_schedule::tick::{
    TickControl, TickHandle, TickLane, TickLifetime, TickOptions, TickPhase, TickPriority, TickTaskState,
    with_scheduler,
};

pub use crate::resources::SmartRemoveResource;
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy)]
struct Request {
    options: SmartRemoveOptions,
    atas: bool,
}

std::thread_local! {
    static REQUEST: RefCell<crate::control::OwnedRule<Option<Request>>> =
        const { RefCell::new(crate::control::OwnedRule::new()) };
}

std::thread_local! {
    static SMART_REMOVE: RefCell<SmartRemoveResource> = RefCell::new(SmartRemoveResource::default());
    static REQUEST_TASK: Cell<Option<TickHandle>> = const { Cell::new(None) };
    static REQUEST_STATE: RefCell<SmartRemoveState> = RefCell::new(SmartRemoveState::default());
}

/// Explicit session-owned automation request. `None` requests disabled; use
/// `release_request` to relinquish control. Existing start/stop preferences are
/// retained while a request is active and resume after the last request releases.
/// The declaration and legacy task belong to this execution module.
pub fn request(options: Option<SmartRemoveOptions>) -> RuntimeResult<()>
where
    CurrentBackend: PlantPresenceBackend
        + ZombieReadBackend
        + ContactGeometryBackend
        + PlantRemoveBackend
        + PlantVisualStateBackend,
{
    request_impl(options.map(|options| Request { options, atas: false }))
}

/// Requests the A-TAS 20250124 algorithm, including its fixed geometry and
/// same-frame stack decisions. Presentation remains an explicit caller choice.
pub fn request_atas(options: Option<SmartRemoveOptions>) -> RuntimeResult<()>
where
    CurrentBackend: PlantPresenceBackend
        + ZombieReadBackend
        + ContactGeometryBackend
        + PlantRemoveBackend
        + PlantVisualStateBackend,
{
    request_impl(options.map(|options| Request { options, atas: true }))
}

fn request_impl(request: Option<Request>) -> RuntimeResult<()>
where
    CurrentBackend: PlantPresenceBackend
        + ZombieReadBackend
        + ContactGeometryBackend
        + PlantRemoveBackend
        + PlantVisualStateBackend,
{
    crate::control::declare(&REQUEST, request, || Ok(None), |_| Ok(()))?;
    with_scheduler(|scheduler| {
        if REQUEST_TASK
            .get()
            .is_some_and(|handle| scheduler.state(handle) != TickTaskState::Stopped)
        {
            return;
        }
        REQUEST_TASK.set(Some(
            scheduler.spawn(
                TickOptions::active_phase()
                    .lifetime(TickLifetime::Session)
                    .lane(TickLane::After)
                    .priority(TickPriority::HIGH)
                    .idle_neutral()
                    .name("smart_remove_request"),
                |meta| {
                    let selected = REQUEST.with_borrow(|rule| rule.effective_declaration().flatten());
                    if let Some(request) = selected
                        && (request.atas || meta.phase == TickPhase::Playing && meta.is_new_frame)
                    {
                        REQUEST_STATE.with_borrow_mut(|state| {
                            state.set_highlight(request.options.highlight);
                            if request.atas {
                                crate::logic::smart_remove::tick_atas_smart_remove(state)
                            } else {
                                tick_impl(state)
                            }
                            .map_err(runtime_error)
                        })?;
                    }
                    Ok(TickControl::Continue)
                },
            ),
        ));
    });
    Ok(())
}

/// Releases only this source's explicit request; does not alter legacy options.
pub fn release_request() -> RuntimeResult<()> {
    crate::control::release(&REQUEST, || Ok(None), |_| Ok(()))
}

/// Effective automation setting, including this module's legacy start/stop state.
pub fn effective_enabled() -> bool {
    match REQUEST.with_borrow(|rule| rule.effective_declaration()) {
        Some(options) => options.is_some(),
        None => SMART_REMOVE.with_borrow(|resource| {
            resource.enabled
                && resource
                    .task
                    .is_some_and(|handle| with_scheduler(|scheduler| scheduler.state(handle) != TickTaskState::Stopped))
        }),
    }
}

pub fn with_smart_remove<R>(f: impl FnOnce(&mut SmartRemoveResource) -> R) -> R {
    crate::resources::ensure_script_reset(&SMART_REMOVE);
    SMART_REMOVE.with_borrow_mut(f)
}

fn start_state(options: SmartRemoveOptions) {
    with_smart_remove(|resource| {
        let mut core = SmartRemoveState::default();
        core.set_highlight(options.highlight);
        resource.enabled = true;
        resource.core = core;
    });
}

fn register_tick()
where
    CurrentBackend: PlantReadBackend
        + ZombieReadBackend
        + ContactGeometryBackend
        + PlantRemoveBackend
        + PlantVisualStateBackend
        + 'static,
{
    with_smart_remove(|resource| {
        with_scheduler(|scheduler| {
            if resource
                .task
                .is_some_and(|handle| scheduler.state(handle) != TickTaskState::Stopped)
            {
                return;
            }
            resource.task = Some(
                scheduler.spawn(
                    TickOptions::playing_frame()
                        .lane(TickLane::After)
                        .priority(TickPriority::HIGH)
                        .idle_neutral()
                        .name("smart_remove"),
                    |_meta| {
                        tick()?;
                        Ok(TickControl::Continue)
                    },
                ),
            );
        });
    });
}

pub fn start() -> RuntimeResult<()>
where
    CurrentBackend: PlantReadBackend
        + ZombieReadBackend
        + ContactGeometryBackend
        + PlantRemoveBackend
        + PlantVisualStateBackend
        + 'static,
{
    start_with_options(SmartRemoveOptions::default())
}

pub fn start_with_options(options: SmartRemoveOptions) -> RuntimeResult<()>
where
    CurrentBackend: PlantReadBackend
        + ZombieReadBackend
        + ContactGeometryBackend
        + PlantRemoveBackend
        + PlantVisualStateBackend
        + 'static,
{
    start_state(options);
    register_tick();
    Ok(())
}

pub fn stop() {
    with_smart_remove(|resource| resource.enabled = false);
}

pub fn set_highlight(highlight: bool) {
    with_smart_remove(|resource| resource.core.set_highlight(highlight));
}

#[must_use]
pub fn options() -> SmartRemoveOptions {
    with_smart_remove(|resource| SmartRemoveOptions {
        highlight: resource.core.highlight(),
    })
}

#[must_use]
pub fn is_running() -> bool {
    with_smart_remove(|resource| resource.enabled)
}

pub fn tick() -> RuntimeResult<()>
where
    CurrentBackend:
        PlantReadBackend + ZombieReadBackend + ContactGeometryBackend + PlantRemoveBackend + PlantVisualStateBackend,
{
    // A coordinated request runs through its own task exactly once. Suppressed
    // legacy tasks retain their options so releasing control can resume them.
    if REQUEST.with_borrow(|rule| rule.effective_declaration().is_some()) {
        return Ok(());
    }
    with_smart_remove(|resource| {
        if !resource.enabled {
            return Ok(());
        }
        tick_impl(&mut resource.core).map_err(runtime_error)
    })
}

pub fn reset() {
    with_smart_remove(SmartRemoveResource::reset_for_script);
}

fn runtime_error(error: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::new(error.to_string())
}
