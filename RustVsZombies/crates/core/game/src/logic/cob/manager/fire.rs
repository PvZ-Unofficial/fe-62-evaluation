use super::*;

impl CobManagerState {
    pub(super) fn fire_one(&mut self, target: CobTarget) -> Result<usize, CobManagerCallError>
    where
        CurrentBackend: ClockBackend + CobBackend,
    {
        crate::access::with_backend(|backend| {
            validate_target(target)?;
            let mut fire_target = None;
            let selected = self
                .select_cob_for_fire(false, |_grid| {
                    Ok((
                        0,
                        *fire_target.get_or_insert_with(|| {
                            cob_target_to_pixel(target)
                                .unwrap_or_else(|error| crate::diagnostics::abort_operation(error.into()))
                        }),
                    ))
                })
                .map_err(CobManagerCallError::Manager)?;
            check_fire_attempt(selected.cob, self.entries[selected.index].grid, selected.fire_target)?;
            self.next = selected.index;
            self.record_latest(
                backend
                    .clock()
                    .unwrap_or_else(|error| crate::diagnostics::abort_operation(error.into())),
                selected.index,
            );
            self.skip_unchecked(1).map_err(CobManagerCallError::Manager)?;
            Ok(selected.index)
        })
    }

    pub(super) fn raw_fire_one(
        &mut self, drop: CobFireDrop, roof: bool, input_number: usize,
    ) -> Result<(), CobManagerCallError>
    where
        CurrentBackend: ClockBackend
            + CobBackend
            + 'static
            + rsvz_backend_api::BoardReadinessBackend
            + rsvz_backend_api::GameUiBackend,
    {
        {
            validate_grid(drop.cob_grid)?;
            validate_target(drop.target)?;
            let Some(cob) = find_cob_at(drop.cob_grid) else {
                return Err(CobManagerError::CobNotFound(drop.cob_grid).into());
            };
            if self.runtime.contains(cob) {
                return Err(CobManagerError::Reserved { grid: drop.cob_grid }.into());
            }
            let fire_target = cob_target_to_pixel(drop.target).unwrap_or_else(|error| {
                crate::diagnostics::abort_operation(RuntimeError::from_source(error).context("底层操作失败"))
            });
            if roof {
                let recover = cob_recover_time(cob);
                if recover < 0 {
                    return Err(CobManagerError::Unrecoverable(drop.cob_grid).into());
                }
                let delay = classic_roof_fire_delay(drop.cob_grid.col + 1, drop.target.drop_col)?;
                if recover > delay {
                    return Err(CobManagerError::Cooling {
                        grid: drop.cob_grid,
                        remaining: recover,
                        available: delay,
                    }
                    .into());
                }
                schedule_cob_fire(
                    &self.runtime,
                    delay,
                    cob,
                    fire_target,
                    FireCommand {
                        epoch: crate::session::world_epoch(),
                        execution: crate::modules::current_execution(rsvz_schedule::module::current_module()),
                        operation: "延迟明确发炮",
                        input_number,
                        target: drop.target,
                        grid: drop.cob_grid,
                        mode: None,
                    },
                )?;
                return Ok(());
            }
            check_fire_attempt(cob, drop.cob_grid, fire_target)?;
            Ok(())
        }
    }

    pub(super) fn delay_fire(
        &mut self, target: CobTarget, roof: bool, recover: bool, input_number: usize,
    ) -> Result<usize, CobManagerCallError>
    where
        CurrentBackend: ClockBackend
            + CobBackend
            + 'static
            + rsvz_backend_api::BoardReadinessBackend
            + rsvz_backend_api::GameUiBackend,
    {
        crate::access::with_backend(|backend| {
            validate_target(target)?;
            let allow_wait = recover;
            let mut fire_target = None;
            let selected = match self.select_cob_for_fire(allow_wait, |grid| {
                let fire_target = *fire_target.get_or_insert_with(|| {
                    cob_target_to_pixel(target).unwrap_or_else(|error| {
                        crate::diagnostics::abort_operation(RuntimeError::from_source(error).context("底层操作失败"))
                    })
                });
                if roof {
                    classic_roof_fire_delay(grid.col + 1, target.drop_col).map(|lead| (lead, fire_target))
                } else {
                    Ok((0, fire_target))
                }
            }) {
                Ok(selected) => selected,
                Err(error) => return Err(CobManagerCallError::Manager(error)),
            };
            let due_clock = backend
                .clock()
                .unwrap_or_else(|error| {
                    crate::diagnostics::abort_operation(RuntimeError::from_source(error).context("底层操作失败"))
                })
                .saturating_add(selected.delay);
            schedule_cob_fire(
                &self.runtime,
                selected.delay,
                selected.cob,
                selected.fire_target,
                FireCommand {
                    epoch: crate::session::world_epoch(),
                    execution: crate::modules::current_execution(rsvz_schedule::module::current_module()),
                    operation: "延迟发炮",
                    input_number,
                    target,
                    grid: self.entries[selected.index].grid,
                    mode: Some(self.mode),
                },
            )?;
            self.next = selected.index;
            self.skip_unchecked(1).map_err(CobManagerCallError::Manager)?;
            self.record_latest(due_clock, selected.index);
            Ok(selected.index)
        })
    }

    pub(super) fn select_cob_for_fire<L>(
        &mut self, allow_wait: bool, mut profile_for: L,
    ) -> Result<SelectedFireCob, CobManagerError>
    where
        CurrentBackend: CobFireBackend,
        L: FnMut(Grid) -> Result<(i32, PixelPos), CobManagerError>,
    {
        if self.entries.is_empty() {
            return Err(CobManagerError::EmptyList);
        }
        let start_index = if self.mode == CobSequentialMode::Priority {
            0
        } else {
            self.current_index()?
        };

        let scan = if self.mode == CobSequentialMode::Space {
            1
        } else {
            self.entries.len()
        };
        let runtime = self.runtime.clone();
        let runtime = runtime.0.borrow();
        let mut best: Option<(i32, SelectedFireCob)> = None;
        let mut evidence = crate::cob::CobSelectionEvidence::default();

        for offset in 0..scan {
            let index = (start_index + offset) % self.entries.len();
            let grid = self.entries[index].grid;
            evidence.examined += 1;
            let Some(cob) = self.refresh_entry_at(index) else {
                evidence.missing += 1;
                continue;
            };
            if runtime.cobs.contains(&cob) {
                evidence.reserved += 1;
                continue;
            }
            {
                let wait = cob_recover_time(cob);
                if wait < 0 {
                    evidence.unrecoverable += 1;
                }
                if wait >= 0 {
                    let (lead, fire_target) = profile_for(grid)?;
                    let ready_in = wait - lead;
                    if ready_in <= 0 {
                        let Ok(_) = i32::try_from(index) else {
                            return Err(CobManagerError::InvalidNext);
                        };
                        return Ok(SelectedFireCob {
                            index,
                            cob,
                            delay: lead,
                            fire_target,
                        });
                    }
                    let selected = SelectedFireCob {
                        index,
                        cob,
                        delay: wait,
                        fire_target,
                    };
                    if best.as_ref().is_none_or(|(best_ready_in, _)| ready_in < *best_ready_in) {
                        best = Some((ready_in, selected));
                        evidence.earliest = Some((grid, ready_in));
                    }
                }
            }
        }

        let Some((_, selected)) = best else {
            return Err(CobManagerError::SelectionUnavailable {
                recover: allow_wait,
                evidence,
            });
        };
        let Ok(_) = i32::try_from(selected.index) else {
            return Err(CobManagerError::InvalidNext);
        };
        if allow_wait {
            Ok(selected)
        } else {
            self.next = selected.index;
            Err(CobManagerError::SelectionUnavailable {
                recover: allow_wait,
                evidence,
            })
        }
    }
}

impl CobManagerState
where
    CurrentBackend: ClockBackend
        + CobBackend
        + SceneBackend
        + 'static
        + rsvz_backend_api::BoardReadinessBackend
        + rsvz_backend_api::GameUiBackend,
{
    pub(super) fn try_fire<C>(
        &mut self, row: i32, col: C, correction: RoofCorrection,
    ) -> Result<i32, crate::cob::CobError>
    where
        (i32, C): IntoCobTarget,
    {
        self.try_single((row, col), false, correction)
    }

    pub(super) fn try_recover_fire<C>(
        &mut self, row: i32, col: C, correction: RoofCorrection,
    ) -> Result<i32, crate::cob::CobError>
    where
        (i32, C): IntoCobTarget,
    {
        self.try_single((row, col), true, correction)
    }

    pub(super) fn try_single<T: IntoCobTarget>(
        &mut self, target: T, recover: bool, correction: RoofCorrection,
    ) -> Result<i32, crate::cob::CobError> {
        let mut outcome = None;
        self.visit_fire_targets(
            [target],
            recover,
            match (correction, recover) {
                (RoofCorrection::None, false) => "发炮（不修正）",
                (RoofCorrection::None, true) => "恢复发炮（不修正）",
                (_, true) => "恢复发炮",
                _ => "发炮",
            },
            correction,
            &mut |_, result| {
                outcome = Some(result);
            },
        );
        outcome.expect("one input produces one result")
    }

    pub(super) fn try_fire_many<T: IntoCobTargets>(
        &mut self, targets: T,
    ) -> Result<crate::cob::CobBatchReport, crate::cob::CobBatchError> {
        self.try_fire_many_with(targets, false, "发炮", RoofCorrection::Automatic)
    }

    pub(super) fn try_recover_fire_many<T: IntoCobTargets>(
        &mut self, targets: T,
    ) -> Result<crate::cob::CobBatchReport, crate::cob::CobBatchError> {
        self.try_fire_many_with(targets, true, "恢复发炮", RoofCorrection::Automatic)
    }

    pub(super) fn try_fire_many_with<T: IntoCobTargets>(
        &mut self, targets: T, recover: bool, operation: &'static str, correction: RoofCorrection,
    ) -> Result<crate::cob::CobBatchReport, crate::cob::CobBatchError> {
        self.collect_fire(targets, recover, operation, correction, true)
    }

    fn collect_fire<T: IntoCobTargets>(
        &mut self, targets: T, recover: bool, operation: &'static str, correction: RoofCorrection, keep_indices: bool,
    ) -> Result<crate::cob::CobBatchReport, crate::cob::CobBatchError> {
        let mut report = crate::cob::CobBatchReport {
            total: targets.cob_target_count(),
            ..Default::default()
        };
        let complete = self.visit_fire_targets(targets, recover, operation, correction, &mut |index, result| {
            report.processed += 1;
            match result {
                Ok(index) => {
                    report.accepted += 1;
                    if keep_indices {
                        report.indices.push(Some(index));
                    }
                }
                Err(error) => {
                    if error.cause.stops_batch() {
                        report.stopped_at = Some(index);
                    }
                    report.failures.push(error);
                    if keep_indices {
                        report.indices.push(None);
                    }
                }
            }
        });
        report.complete = complete;
        if report.failures.is_empty() {
            Ok(report)
        } else {
            Err(crate::cob::CobBatchError { report })
        }
    }

    pub(super) fn visit_fire_targets<T, F>(
        &mut self, targets: T, recover: bool, operation: &'static str, correction: RoofCorrection, visit: &mut F,
    ) -> bool
    where
        T: IntoCobTargets,
        F: FnMut(usize, Result<i32, crate::cob::CobError>),
    {
        crate::access::with_backend(|backend| {
            if let ListSource::Rediscover(discovery) = self.list_source {
                match discovery {
                    Discovery::All(order) => self.auto_set_list_by(order),
                    Discovery::Column(column) => self.auto_set_list_in_column(column),
                }
            }
            let mut roof = match correction {
                RoofCorrection::Automatic => None,
                RoofCorrection::None => Some(false),
            };
            let mut index = 0;
            targets
                .try_for_each_cob_target_result(&mut |input| {
                    let target = input.as_ref().ok().copied();
                    let result = match input {
                        Err(error) => Err(error.into()),
                        Ok(target) => {
                            let roof = *roof.get_or_insert_with(|| {
                                crate::live_value::read_or_abort(backend.scene(), "scene").has_roof()
                            });
                            if recover || roof {
                                self.delay_fire(target, roof, recover, index + 1)
                            } else {
                                self.fire_one(target)
                            }
                        }
                    };
                    let stop = result.as_ref().is_err_and(|error| error.stops_batch());
                    let result = result.map(|index| index as i32).map_err(|cause| crate::cob::CobError {
                        operation,
                        input_number: index + 1,
                        target,
                        cannon: cause.cannon(),
                        mode: Some(self.mode),
                        cause,
                    });
                    visit(index, result);
                    index += 1;
                    if stop { Err(()) } else { Ok(()) }
                })
                .is_ok()
        })
    }

    pub(super) fn try_raw_fire<D: IntoCobFireDrops>(
        &mut self, drops: D, correction: RoofCorrection,
    ) -> Result<(), crate::cob::CobBatchError> {
        crate::access::with_backend(|backend| {
            let mut report = crate::cob::CobBatchReport {
                total: drops.cob_drop_count(),
                ..Default::default()
            };
            let mut roof = match correction {
                RoofCorrection::Automatic => None,
                RoofCorrection::None => Some(false),
            };
            report.complete = drops
                .try_for_each_cob_fire_drop_result(&mut |input| {
                    let drop = input.as_ref().ok().copied();
                    let result = match input {
                        Err(error) => Err(error.into()),
                        Ok(drop) => {
                            let roof = *roof.get_or_insert_with(|| {
                                crate::live_value::read_or_abort(backend.scene(), "scene").has_roof()
                            });
                            self.raw_fire_one(drop, roof, report.processed + 1)
                        }
                    };
                    let index = report.processed;
                    report.processed += 1;
                    match result {
                        Ok(()) => {
                            report.accepted += 1;
                            Ok(())
                        }
                        Err(cause) => {
                            let stop = cause.stops_batch();
                            report.failures.push(crate::cob::CobError {
                                operation: "明确发炮",
                                input_number: index + 1,
                                target: drop.map(|drop| drop.target),
                                cannon: drop.map(|drop| drop.cob_grid),
                                mode: None,
                                cause,
                            });
                            if stop {
                                report.stopped_at = Some(index);
                                Err(())
                            } else {
                                Ok(())
                            }
                        }
                    }
                })
                .is_ok();
            if report.failures.is_empty() {
                Ok(())
            } else {
                Err(crate::cob::CobBatchError { report })
            }
        })
    }

    pub(super) fn fire_prepared(
        &mut self, targets: &[CobTarget], recover: bool, operation: &'static str,
    ) -> RuntimeResult<()> {
        self.collect_fire(targets, recover, operation, RoofCorrection::Automatic, false)
            .map(|_| ())
            .map_err(Into::into)
    }
}

pub(super) fn check_fire_attempt(cob: PlantId, grid: Grid, target: PixelPos) -> Result<(), CobManagerCallError>
where
    CurrentBackend: CobFireBackend,
{
    match fire_cob_attempt(cob, target)
        .unwrap_or_else(|error| crate::diagnostics::abort_operation(RuntimeError::from_source(error)))
    {
        CobFireAttempt::Fired => Ok(()),
        CobFireAttempt::NotReady(remaining) => Err(CobManagerError::Cooling {
            grid,
            remaining,
            available: 0,
        }
        .into()),
        CobFireAttempt::Unavailable => Err(CobManagerError::Unrecoverable(grid).into()),
        CobFireAttempt::Rejected(error) => Err(CobManagerCallError::Rejected {
            cannon: grid,
            source: error,
        }),
    }
}
