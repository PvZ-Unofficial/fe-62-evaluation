//! Current timing values composed from native scalar reads.
pub use crate::logic::timing::{is_flag_wave, is_huge_wave_countdown};
use crate::runtime::RuntimeResult;
use rsvz_backend_api::{ClockBackend, CurrentWaveBackend, WaveHealthBackend, WaveTimingBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{Wave, WaveTimingSnapshot};

/// Reads the initial refresh health recorded for the current wave.
pub fn zombie_health_wave_start() -> i32
where
    CurrentBackend: WaveHealthBackend,
{
    crate::access::with_backend(|backend| {
        crate::live_value::read_or_abort(backend.zombie_health_wave_start(), "initial wave health")
    })
}

/// Reads native refresh health for a one-based wave, preserving the game's HP rules.
pub fn total_zombies_health_in_wave(wave: Wave) -> i32
where
    CurrentBackend: WaveHealthBackend,
{
    if wave.0 <= 0 {
        crate::diagnostics::abort_operation(crate::runtime::RuntimeError::new("wave must be positive"));
    }
    crate::access::with_backend(|backend| {
        crate::live_value::read_or_abort(backend.total_zombies_health_in_wave(wave.0 - 1), "wave health")
    })
}

/// Reads current wave timing, aborting the user callback on necessary read failure.
pub fn wave_timing() -> WaveTimingSnapshot
where
    CurrentBackend: WaveTimingBackend,
{
    crate::live_value::read_or_abort(read_wave_timing(), "failed to read wave timing")
}

// Framework resampling runs outside user callback boundaries and must return errors.
pub(crate) fn read_wave_timing() -> RuntimeResult<WaveTimingSnapshot>
where
    CurrentBackend: WaveTimingBackend,
{
    CurrentBackend::with_backend_shared(|access| {
        let backend = access;
        Ok(WaveTimingSnapshot {
            clock: backend.clock()?,
            current_wave: backend.current_wave()?,
            total_waves: Some(backend.total_waves()?),
            refresh_countdown: Some(backend.refresh_countdown()?),
            initial_countdown: Some(backend.initial_countdown()?),
            huge_wave_countdown: Some(backend.huge_wave_countdown()?),
            level_end_countdown: Some(backend.level_end_countdown()?),
        })
    })
    .map_err(|error| crate::runtime::RuntimeError::new(error.to_string()))?
}

/// Reads the current refresh countdown; a necessary read failure ends the current callback.
pub fn refresh_countdown() -> i32
where
    CurrentBackend: WaveTimingBackend,
{
    crate::live_value::read_or_abort(
        CurrentBackend::with_backend_shared(|access| access.refresh_countdown())
            .map_err(|error| crate::runtime::RuntimeError::new(error.to_string()))
            .and_then(|result| result.map_err(Into::into)),
        "failed to read refresh countdown",
    )
}

/// Reads the current game clock without sampling full wave timing.
pub fn clock() -> i32
where
    CurrentBackend: ClockBackend,
{
    crate::access::with_backend(|backend| crate::live_value::read_or_abort(backend.clock(), "failed to read clock"))
}

/// Only an absent Board produces None; access and other read faults still abort.
pub fn clock_if_available() -> Option<i32>
where
    CurrentBackend: ClockBackend,
{
    crate::access::with_backend(|backend| match backend.clock() {
        Ok(value) => Some(value),
        Err(error) if error.is_board_unavailable() => None,
        Err(error) => crate::diagnostics::abort_operation(error.into()),
    })
}

pub fn current_wave() -> Wave
where
    rsvz_current::CurrentBackend: rsvz_backend_api::CurrentWaveBackend,
{
    use rsvz_backend_api::CurrentWaveBackend;
    crate::access::with_backend(|backend| crate::live_value::read_or_abort(backend.current_wave(), "current_wave"))
}

pub fn total_waves() -> i32
where
    rsvz_current::CurrentBackend: rsvz_backend_api::WaveTimingBackend,
{
    use rsvz_backend_api::WaveTimingBackend;
    crate::access::with_backend(|backend| crate::live_value::read_or_abort(backend.total_waves(), "total_waves"))
}

pub fn initial_countdown() -> i32
where
    rsvz_current::CurrentBackend: rsvz_backend_api::WaveTimingBackend,
{
    use rsvz_backend_api::WaveTimingBackend;
    crate::access::with_backend(|backend| {
        crate::live_value::read_or_abort(backend.initial_countdown(), "initial_countdown")
    })
}

pub fn huge_wave_countdown() -> i32
where
    rsvz_current::CurrentBackend: rsvz_backend_api::WaveTimingBackend,
{
    use rsvz_backend_api::WaveTimingBackend;
    crate::access::with_backend(|backend| {
        crate::live_value::read_or_abort(backend.huge_wave_countdown(), "huge_wave_countdown")
    })
}

pub fn level_end_countdown() -> i32
where
    rsvz_current::CurrentBackend: rsvz_backend_api::WaveTimingBackend,
{
    use rsvz_backend_api::WaveTimingBackend;
    crate::access::with_backend(|backend| {
        crate::live_value::read_or_abort(backend.level_end_countdown(), "level_end_countdown")
    })
}
