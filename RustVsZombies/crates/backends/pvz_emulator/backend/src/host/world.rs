use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rsvz_backend_api::artifact::SessionArtifact;
use rsvz_model::SessionShard;

use super::{profile, wire};
use crate::runtime::PeWorldOwner;
use crate::{DispatchEntry, DispatchInput, DispatchResult, PeUpdateOutcome, PeWorldConfig};

pub(super) struct WorkerOutput {
    pub result: wire::WorkerResult,
    pub artifact: Option<SessionArtifact>,
    pub error: Option<String>,
}

pub(super) fn failed_worker(worker_index: usize, seed: u32, error: String) -> WorkerOutput {
    WorkerOutput {
        result: wire::WorkerResult {
            cleanup_complete: false,
            diagnostics: Vec::new(),
            worker_index,
            seed,
            stop_reason: wire::StopReason::Failed {},
            wall_ns: 0,
            frames: 0,
            completed_levels: 0,
            partial_frames: 0,
            profile: None,
            performance_window: None,
        },
        artifact: None,
        error: Some(error),
    }
}

pub(super) fn run_worker(
    worker_index: usize, worker_count: usize, seed: u32, config: &wire::PeRunConfig, cancelled: &AtomicBool,
    dispatch: DispatchEntry,
) -> Result<WorkerOutput, String> {
    let started = Instant::now();
    let world_config = PeWorldConfig {
        battle_seed: seed,
        level_seed: seed,
        ..PeWorldConfig::default()
    };
    let owner = PeWorldOwner::new_reset_deferred_spawn(world_config).map_err(|error| error.to_string())?;
    let mut world = owner.install_current().map_err(|error| error.to_string())?;
    let shard = SessionShard {
        index: u32::try_from(worker_index).map_err(|_error| "worker index does not fit u32")?,
        count: u32::try_from(worker_count).map_err(|_error| "worker count does not fit u32")?,
        seed_base: match config.seed_mode {
            wire::SeedMode::Fixed { seed } | wire::SeedMode::Base { seed } => seed,
        },
    };
    let mut cleanup_complete = false;
    let mut frames = 0u64;
    let mut partial_frames = 0u64;
    let mut completed_levels = 0u64;
    let mut completed_rounds = 0;
    let mut profile = profile::WorkerProfile::new(config.profile);
    let mut performance = profile::PerformanceWindow::new(config.performance_window_ms);

    let (stop_reason, artifact, error) = loop {
        performance.poll(worker_index, started, frames, completed_levels);
        let limit = limit_reason(config, cancelled, started, frames, completed_levels);
        let mut frame_profile = profile.begin_frame();
        let output = frame_profile.measure_run(|| {
            world.with_backend(|backend| {
                // Survival closing countdown remains an operable Playing interval.
                dispatch(
                    backend,
                    DispatchInput {
                        shard,
                        stop_requested: limit.is_some(),
                        completed_rounds,
                    },
                )
            })
        });
        completed_rounds = 0;
        match output {
            DispatchResult::Continue | DispatchResult::Idle => {}
            DispatchResult::SkipUpdate | DispatchResult::EnterBattle => {
                profile.finish_frame(frame_profile);
                continue;
            }
            DispatchResult::Stop {
                artifact,
                error,
                cleanup_complete: cleaned,
            } => {
                cleanup_complete = cleaned;
                profile.finish_frame(frame_profile);
                performance.poll(worker_index, started, frames, completed_levels);
                let cancelled = error
                    .as_ref()
                    .is_some_and(|error| error.find_source::<rsvz_backend_api::error::UserCancelled>().is_some());
                let error = error.map(|error| error.to_string());
                let stop_reason = if cancelled {
                    wire::StopReason::Cancelled {}
                } else if error.is_some() {
                    wire::StopReason::Failed {}
                } else {
                    limit.unwrap_or(wire::StopReason::Script {})
                };
                break (stop_reason, artifact, error);
            }
        }

        let update = frame_profile.measure_backend_update(|| world.update_world());
        profile.finish_frame(frame_profile);
        match update {
            Ok(PeUpdateOutcome::Normal) => {
                frames = frames.saturating_add(1);
                partial_frames = partial_frames.saturating_add(1);
            }
            Ok(PeUpdateOutcome::ObjectiveReached) => {
                frames = frames.saturating_add(1);
                completed_levels = completed_levels.saturating_add(1);
                completed_rounds = 1;
                partial_frames = 0;
            }
            Ok(PeUpdateOutcome::GameOver) => {
                frames = frames.saturating_add(1);
                partial_frames = partial_frames.saturating_add(1);
                let mut terminal_profile = profile.begin_frame();
                let output = terminal_profile.measure_run(|| {
                    world.with_backend(|backend| {
                        dispatch(
                            backend,
                            DispatchInput {
                                shard,
                                stop_requested: false,
                                completed_rounds: 0,
                            },
                        )
                    })
                });
                profile.finish_frame(terminal_profile);
                performance.poll(worker_index, started, frames, completed_levels);
                match output {
                    DispatchResult::Stop {
                        artifact,
                        error,
                        cleanup_complete: cleaned,
                    } => {
                        cleanup_complete = cleaned;
                        let stop_reason = if error.is_some() {
                            wire::StopReason::Failed {}
                        } else {
                            wire::StopReason::GameOver {}
                        };
                        break (stop_reason, artifact, error.map(|error| error.to_string()));
                    }
                    DispatchResult::Continue
                    | DispatchResult::Idle
                    | DispatchResult::SkipUpdate
                    | DispatchResult::EnterBattle => {
                        // The terminal dispatch can reset a measurement world for its next trial.
                        // Only the post-dispatch state can decide whether GameOver still applies.
                        if !world
                            .with_backend(|backend| backend.game_over())
                            .map_err(|error| error.to_string())?
                        {
                            continue;
                        }
                        // Preserve the protocol error, but let the normal stop boundary release
                        // session callbacks before the worker's TLS starts being destroyed.
                        let _ = world.with_backend(|backend| {
                            dispatch(
                                backend,
                                DispatchInput {
                                    shard,
                                    stop_requested: true,
                                    completed_rounds: 0,
                                },
                            )
                        });
                        break (
                            wire::StopReason::Failed {},
                            None,
                            Some("dispatch did not stop after GameOver".to_owned()),
                        );
                    }
                }
            }
            Err(error) => {
                break (wire::StopReason::Failed {}, None, Some(error.to_string()));
            }
        }
    };
    Ok(WorkerOutput {
        result: wire::WorkerResult {
            cleanup_complete,
            diagnostics: Vec::new(),
            worker_index,
            seed,
            stop_reason,
            wall_ns: started.elapsed().as_nanos(),
            frames,
            completed_levels,
            partial_frames,
            profile: profile.snapshot(),
            performance_window: performance.captured,
        },
        artifact,
        error,
    })
}

fn limit_reason(
    config: &wire::PeRunConfig, cancelled: &AtomicBool, started: Instant, frames: u64, levels: u64,
) -> Option<wire::StopReason> {
    if cancelled.load(Ordering::Acquire) {
        return Some(wire::StopReason::Cancelled {});
    }
    if let Some(limit) = config.limits.max_wall_ms
        && started.elapsed() >= Duration::from_millis(limit)
    {
        return Some(wire::StopReason::MaxWall {});
    }
    if config.limits.max_sim_frames.is_some_and(|limit| frames >= limit) {
        return Some(wire::StopReason::MaxSimFrames {});
    }
    if config.limits.max_levels.is_some_and(|limit| levels >= limit) {
        return Some(wire::StopReason::MaxLevels {});
    }
    None
}
