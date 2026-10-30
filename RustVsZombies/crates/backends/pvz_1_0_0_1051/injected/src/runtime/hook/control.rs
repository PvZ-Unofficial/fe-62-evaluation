use std::cell::{Cell, RefCell};

use rsvz_model::{AdvancedPauseOptions, FastForwardOptions, FastForwardStopReason, SeedChooserFastForwardOptions};

use crate::error::{Pvz1051Error, Result};
use crate::patches::ImitatorMorphHookGuard;
use crate::patches::world_lifetime::WorldLifetimeHookGuard;
use crate::raw::layout as ptrs;
use crate::runtime::Pvz1051Backend;
use crate::runtime::advanced_pause::AdvancedPauseRuntime;
use crate::runtime::fast_forward::{FastForwardLoopDecision, FastForwardRuntime, SeedChooserFastForwardRuntime};

use super::{lifecycle, profiler};

thread_local! {
    static REPICK_GUARD: RefCell<Option<crate::patches::chooser_repick::RepickGuard>> = const { RefCell::new(None) };
    static FAST_FORWARD: RefCell<FastForwardRuntime> = const { RefCell::new(FastForwardRuntime::new()) };
    static SEED_CHOOSER_FAST_FORWARD: RefCell<SeedChooserFastForwardRuntime> =
        const { RefCell::new(SeedChooserFastForwardRuntime::new()) };
    static ADVANCED_PAUSE: RefCell<AdvancedPauseRuntime> =
        const { RefCell::new(AdvancedPauseRuntime::new()) };
    static IMITATOR_MORPH_HOOK: RefCell<Option<ImitatorMorphHookGuard>> =
        const { RefCell::new(None) };
    static WORLD_LIFETIME_HOOK: RefCell<Option<WorldLifetimeHookGuard>> = const { RefCell::new(None) };
    static ORIGINAL_GAME_SPEED: Cell<Option<f64>> = const { Cell::new(None) };
}

fn ensure_accepting_requests() -> Result<()> {
    if lifecycle::accepting_dispatch() {
        Ok(())
    } else {
        Err(Pvz1051Error::AbiPreconditionFailed(
            "1051 runtime is not accepting control requests",
        ))
    }
}

pub(crate) fn start_fast_forward(options: FastForwardOptions) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.ff_start)(options));
    }
    ensure_accepting_requests()?;
    if advanced_pause_active() {
        return Err(Pvz1051Error::AbiPreconditionFailed(
            "fast-forward cannot start while advanced pause is active",
        ));
    }
    let requested = FAST_FORWARD.with(|state| state.borrow_mut().start(options))?;
    if requested {
        profiler::reset();
    }
    Ok(())
}

pub(crate) fn stop_fast_forward(reason: FastForwardStopReason) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.ff_stop)(reason));
    }
    FAST_FORWARD.with(|state| {
        let mut state = state.borrow_mut();
        let frames = state.frames_advanced();
        let was_active = state.is_active();
        state.stop(reason)?;
        if was_active {
            finish_profile(reason, frames);
        }
        Ok(())
    })
}

pub(crate) fn fast_forward_active() -> bool {
    if let Some(host) = crate::module_link::services() {
        return (host.ff_active)();
    }
    FAST_FORWARD.with(|state| state.borrow().is_active())
}

pub(crate) fn stop_seed_chooser_fast_forward() {
    if let Some(host) = crate::module_link::services() {
        (host.chooser_stop)();
        return;
    }
    SEED_CHOOSER_FAST_FORWARD.with_borrow_mut(|state| state.stop());
}

pub(crate) fn request_seed_chooser_fast_forward(options: SeedChooserFastForwardOptions) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.chooser_start)(options));
    }
    ensure_accepting_requests()?;
    if advanced_pause_active() || fast_forward_active() {
        return Err(Pvz1051Error::AbiPreconditionFailed(
            "seed chooser fast-forward conflicts with another physical loop",
        ));
    }
    SEED_CHOOSER_FAST_FORWARD.with(|state| {
        state.borrow_mut().request(options);
    });
    Ok(())
}

pub(crate) fn set_advanced_pause(backend: &Pvz1051Backend, enabled: bool, options: AdvancedPauseOptions) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.pause_set)(enabled, options));
    }
    if enabled {
        ensure_accepting_requests()?;
        if fast_forward_active() {
            return Err(Pvz1051Error::AbiPreconditionFailed(
                "advanced pause cannot start while fast-forward is active",
            ));
        }
        if options.draw_mask {
            return Err(Pvz1051Error::KindUnavailable("advanced pause mask drawing"));
        }
        if options.play_sound {
            return Err(Pvz1051Error::KindUnavailable("advanced pause sound feedback"));
        }
        crate::runtime::advanced_pause::ensure_can_enable(backend)?;
    }
    ADVANCED_PAUSE.with(|state| state.borrow_mut().set_enabled(enabled, options))
}

pub(crate) fn advanced_pause_active() -> bool {
    if let Some(host) = crate::module_link::services() {
        return (host.pause_active)();
    }
    ADVANCED_PAUSE.with(|state| state.borrow().is_active())
}

pub(crate) fn advanced_pause_options() -> AdvancedPauseOptions {
    if let Some(host) = crate::module_link::services() {
        return (host.pause_options)();
    }
    ADVANCED_PAUSE.with_borrow(|state| state.options())
}

pub(super) fn update_advanced_pause_frame() -> Result<()> {
    ADVANCED_PAUSE.with(|state| state.borrow_mut().update_frame())
}

pub(super) fn ensure_native_observers() -> Result<()> {
    REPICK_GUARD.with_borrow_mut(|slot| -> Result<()> {
        if slot.is_none() {
            *slot = Some(crate::patches::chooser_repick::RepickGuard::install()?);
        }
        Ok(())
    })?;
    WORLD_LIFETIME_HOOK.with_borrow_mut(|slot| -> Result<()> {
        if slot.is_none() {
            *slot = Some(WorldLifetimeHookGuard::install()?);
        }
        Ok(())
    })?;
    IMITATOR_MORPH_HOOK.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(ImitatorMorphHookGuard::install()?);
        }
        Ok(())
    })
}

pub(super) fn restore_observers() -> Result<()> {
    REPICK_GUARD.with_borrow_mut(|slot| -> Result<()> {
        if let Some(guard) = slot.as_mut() {
            guard.restore()?;
        }
        *slot = None;
        Ok(())
    })?;
    let result = IMITATOR_MORPH_HOOK.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(hook) = slot.as_mut() else {
            return Ok(());
        };
        hook.restore()?;
        *slot = None;
        Ok(())
    });
    crate::runtime::imitator_morph::reset();
    let world_result = WORLD_LIFETIME_HOOK.with_borrow_mut(|slot| -> Result<()> {
        if let Some(hook) = slot.as_mut() {
            hook.restore()?;
            *slot = None;
        }
        Ok(())
    });
    result.and(world_result)
}

pub(crate) fn set_game_speed(speed: f32) -> Result<()> {
    set_game_speed_multiplier(f64::from(speed))
}

pub(crate) fn set_game_speed_multiplier(speed: f64) -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.game_speed)(speed));
    }
    let app = crate::runtime::backend::current_app()?;
    // Keep the native control/dispatch loop alive at the supported lower bound.
    crate::patches::leases::set_bool_patch(
        crate::patches::leases::BoolPatchId::SlowUpdateProgress,
        speed > 0.0 && speed <= 0.1,
    )?;
    ORIGINAL_GAME_SPEED.with(|original| {
        if original.get().is_none() {
            // SAFETY: the active LawnApp was validated above and these are
            // process-local scalar timing controls.
            original.set(Some(unsafe { ptrs::LawnApp::update_multiplier(app.as_ptr()) }));
        }
    });
    // SAFETY: the active LawnApp was validated above. Values are bounded by
    // the public backend implementation before reaching this function.
    unsafe {
        ptrs::LawnApp::set_update_multiplier(app.as_ptr(), speed);
    }
    Ok(())
}

pub(crate) fn restore_game_speed() -> Result<()> {
    let Some(update_multiplier) = ORIGINAL_GAME_SPEED.get() else {
        return Ok(());
    };
    let app = crate::runtime::backend::current_app()?;
    // SAFETY: this exact scalar value was captured before this runner first
    // changed it and unload is executing on the game thread.
    unsafe {
        ptrs::LawnApp::set_update_multiplier(app.as_ptr(), update_multiplier);
    }
    crate::patches::leases::set_bool_patch(crate::patches::leases::BoolPatchId::SlowUpdateProgress, false)?;
    ORIGINAL_GAME_SPEED.set(None);
    Ok(())
}

pub(super) fn seed_chooser_before_update() -> Result<bool> {
    SEED_CHOOSER_FAST_FORWARD.with(|state| state.borrow_mut().before_tick())
}

pub(super) fn advance_seed_chooser_update() -> Result<bool> {
    SEED_CHOOSER_FAST_FORWARD.with(|state| state.borrow_mut().advance_widget_frame())
}

pub(super) fn fast_forward_before_update() -> Result<FastForwardLoopDecision> {
    FAST_FORWARD.with(|state| {
        let mut state = state.borrow_mut();
        let frames = state.frames_advanced();
        let was_active = state.is_active();
        let decision = state.before_tick()?;
        if matches!(decision, FastForwardLoopDecision::Stopped(_)) && was_active {
            let FastForwardLoopDecision::Stopped(reason) = decision else {
                unreachable!()
            };
            finish_profile(reason, frames);
        }
        Ok(decision)
    })
}

pub(super) fn advance_fast_forward_update() -> Result<FastForwardLoopDecision> {
    FAST_FORWARD.with(|state| {
        let mut state = state.borrow_mut();
        let frames = state.frames_advanced();
        let was_active = state.is_active();
        let decision = state.advance_logic_frame()?;
        if matches!(decision, FastForwardLoopDecision::Stopped(_)) && was_active {
            let FastForwardLoopDecision::Stopped(reason) = decision else {
                unreachable!()
            };
            finish_profile(reason, frames);
        }
        Ok(decision)
    })
}

pub(super) fn restore_all() -> Result<()> {
    crate::patches::keyboard::set(None)?;
    let mut first_error = super::paint::restore().err();
    if let Err(error) = crate::patches::mower::restore()
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    if let Err(error) = crate::patches::rake::restore()
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    if let Err(error) = stop_fast_forward(FastForwardStopReason::RuntimeExit)
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    SEED_CHOOSER_FAST_FORWARD.with(|state| {
        state.borrow_mut().stop();
    });
    if let Err(error) = ADVANCED_PAUSE.with(|state| state.borrow_mut().restore())
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    if let Err(error) = crate::patches::leases::release_all_owned_patches()
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    if let Err(error) = crate::patches::restore_random()
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    if let Err(error) = crate::patches::restore_sun_production()
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    if let Err(error) = restore_game_speed()
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    first_error.map_or(Ok(()), Err)
}

pub(crate) fn prepare_world_reset() -> Result<()> {
    if let Some(host) = crate::module_link::services() {
        return crate::module_link::checked((host.prepare_reset)());
    }
    let mut first_error = None;
    if let Err(error) = stop_fast_forward(FastForwardStopReason::RuntimeExit) {
        first_error = Some(error);
    }
    SEED_CHOOSER_FAST_FORWARD.with(|state| {
        state.borrow_mut().stop();
    });
    if let Err(error) = ADVANCED_PAUSE.with(|state| state.borrow_mut().restore())
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    crate::runtime::imitator_morph::reset();
    first_error.map_or(Ok(()), Err)
}

fn finish_profile(reason: FastForwardStopReason, frames: u64) {
    if let Some(summary) = profiler::finish_summary(reason, frames) {
        profiler::write_summary_to_temp(&summary);
    }
}
