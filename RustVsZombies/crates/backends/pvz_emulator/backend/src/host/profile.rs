use rsvz_profiling::{FrameIterationRecorder, FrameProfileStats};

use super::wire;
use std::time::{Duration, Instant};

pub(super) struct PerformanceWindow {
    requested_ms: Option<u64>,
    pub(super) captured: Option<wire::PerformanceWindow>,
}

impl PerformanceWindow {
    pub(super) fn new(requested_ms: Option<u64>) -> Self {
        Self {
            requested_ms,
            captured: None,
        }
    }

    pub(super) fn poll(&mut self, worker_index: usize, started: Instant, frames: u64, levels: u64) {
        if self.requested_ms.is_some() && self.captured.is_none() {
            self.observe(worker_index, started.elapsed(), frames, levels);
        }
    }

    pub(super) fn observe(&mut self, worker_index: usize, elapsed: Duration, frames: u64, completed_levels: u64) {
        if self.captured.is_none()
            && let Some(requested_ms) = self.requested_ms
            && elapsed >= Duration::from_millis(requested_ms)
        {
            let sample = wire::PerformanceWindow {
                worker_index,
                requested_ms,
                wall_ns: elapsed.as_nanos(),
                frames,
                completed_levels,
            };
            // A single checkpoint is available to tooling immediately, even when
            // a script continues for a long tail. This is independent of script policy.
            eprintln!(
                "PE_PERFORMANCE_WINDOW {}",
                serde_json::to_string(&sample).expect("performance counters serialize")
            );
            self.captured = Some(sample);
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    #[test]
    fn coarse_snapshot_contains_only_iteration_totals() {
        assert!(WorkerProfile::new(false).snapshot().is_none());
        let profile = WorkerProfile {
            frame: Some(FrameProfileStats {
                iterations: 2,
                loop_total_ns: 200,
                run_total_ns: 60,
                backend_update_ns: 100,
            }),
        };
        assert_eq!(
            serde_json::to_value(profile.snapshot().unwrap()).unwrap(),
            serde_json::json!({
                "iterations": 2, "loop_total_ns": 200, "run_total_ns": 60, "backend_update_ns": 100,
            })
        );
    }

    #[test]
    fn prefix_is_frozen_while_tail_counters_continue() {
        let mut window = PerformanceWindow::new(Some(10));
        window.observe(2, Duration::from_millis(9), 9, 0);
        assert!(window.captured.is_none());
        window.observe(2, Duration::from_millis(10), 10, 1);
        let sealed = window.captured;
        window.observe(2, Duration::from_secs(100), 999, 99);
        assert_eq!(window.captured, sealed);
        assert_eq!(window.captured.unwrap().frames, 10);
    }
}

pub(super) struct WorkerProfile {
    frame: Option<FrameProfileStats>,
}

impl WorkerProfile {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            frame: enabled.then(FrameProfileStats::new),
        }
    }

    pub(super) fn begin_frame(&self) -> FrameIterationRecorder {
        FrameIterationRecorder::start(self.frame.is_some())
    }

    pub(super) fn finish_frame(&mut self, frame: FrameIterationRecorder) {
        if let Some(total) = self.frame.as_mut() {
            frame.finish(total);
        }
    }

    pub(super) fn snapshot(&self) -> Option<wire::RawProfile> {
        self.frame.map(|profile| wire::RawProfile {
            iterations: profile.iterations,
            loop_total_ns: profile.loop_total_ns,
            run_total_ns: profile.run_total_ns,
            backend_update_ns: profile.backend_update_ns,
        })
    }
}
