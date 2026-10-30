//! Explicit, allocation-free wall-clock timing for physical host iterations.

use std::time::{Duration, Instant};

/// Totals for completed host iterations, including any waits within measured intervals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameProfileStats {
    pub iterations: u64,
    pub loop_total_ns: u128,
    pub run_total_ns: u128,
    pub backend_update_ns: u128,
}

impl FrameProfileStats {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            iterations: 0,
            loop_total_ns: 0,
            run_total_ns: 0,
            backend_update_ns: 0,
        }
    }
}

/// Disabled recorders execute work without reading the clock.
#[derive(Debug)]
pub struct FrameIterationRecorder {
    loop_started: Option<Instant>,
    run_total_ns: u128,
    backend_update_ns: u128,
}

impl FrameIterationRecorder {
    #[must_use]
    pub fn start(enabled: bool) -> Self {
        Self {
            loop_started: enabled.then(Instant::now),
            run_total_ns: 0,
            backend_update_ns: 0,
        }
    }

    pub fn record_run_duration(&mut self, elapsed: Duration) {
        if self.loop_started.is_some() {
            self.run_total_ns = self.run_total_ns.saturating_add(elapsed.as_nanos());
        }
    }

    pub fn record_backend_update_duration(&mut self, elapsed: Duration) {
        if self.loop_started.is_some() {
            self.backend_update_ns = self.backend_update_ns.saturating_add(elapsed.as_nanos());
        }
    }

    pub fn measure_run<R>(&mut self, f: impl FnOnce() -> R) -> R {
        if self.loop_started.is_none() {
            return f();
        }
        let started = Instant::now();
        let result = f();
        self.record_run_duration(started.elapsed());
        result
    }

    pub fn measure_backend_update<R>(&mut self, f: impl FnOnce() -> R) -> R {
        if self.loop_started.is_none() {
            return f();
        }
        let started = Instant::now();
        let result = f();
        self.record_backend_update_duration(started.elapsed());
        result
    }

    pub fn finish(self, stats: &mut FrameProfileStats) {
        if let Some(started) = self.loop_started {
            self.finish_with_loop_duration(stats, started.elapsed());
        }
    }

    fn finish_with_loop_duration(self, stats: &mut FrameProfileStats, elapsed: Duration) {
        stats.iterations = stats.iterations.saturating_add(1);
        stats.loop_total_ns = stats.loop_total_ns.saturating_add(elapsed.as_nanos());
        stats.run_total_ns = stats.run_total_ns.saturating_add(self.run_total_ns);
        stats.backend_update_ns = stats.backend_update_ns.saturating_add(self.backend_update_ns);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_completed_iterations_and_repeated_intervals() {
        let mut stats = FrameProfileStats::new();
        for _ in 0..2 {
            let mut iteration = FrameIterationRecorder::start(true);
            iteration.record_run_duration(Duration::from_nanos(10));
            iteration.record_run_duration(Duration::from_nanos(20));
            iteration.record_backend_update_duration(Duration::from_nanos(50));
            iteration.finish_with_loop_duration(&mut stats, Duration::from_nanos(100));
        }
        assert_eq!(
            stats,
            FrameProfileStats {
                iterations: 2,
                loop_total_ns: 200,
                run_total_ns: 60,
                backend_update_ns: 100,
            }
        );
    }

    #[test]
    fn disabled_recorder_executes_work_without_collecting_stats() {
        let mut stats = FrameProfileStats::new();
        let mut iteration = FrameIterationRecorder::start(false);
        let mut calls = 0;
        assert_eq!(
            iteration.measure_run(|| {
                calls += 1;
                7
            }),
            7
        );
        assert_eq!(
            iteration.measure_backend_update(|| {
                calls += 1;
                9
            }),
            9
        );
        iteration.record_run_duration(Duration::from_secs(1));
        iteration.record_backend_update_duration(Duration::from_secs(1));
        iteration.finish(&mut stats);
        assert_eq!(calls, 2);
        assert_eq!(stats, FrameProfileStats::new());
    }
}
