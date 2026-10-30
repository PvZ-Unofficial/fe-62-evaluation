//! Cannon failures retain commands and partial results until their final consumer.
use std::fmt;

use super::{CobManagerCallError, CobManagerError, CobSequentialMode};
use crate::{
    model::{CobTarget, Grid},
    runtime::RuntimeError,
};

/// Observations collected during the existing selection pass, not another scan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CobSelectionEvidence {
    pub examined: usize,
    pub missing: usize,
    pub reserved: usize,
    pub unrecoverable: usize,
    pub earliest: Option<(Grid, i32)>,
}

impl fmt::Display for CobSelectionEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "检查 {} 项：炮不存在 {} 项，已预约 {} 项，不可恢复 {} 项",
            self.examined, self.missing, self.reserved, self.unrecoverable
        )?;
        if let Some((grid, remaining)) = self.earliest {
            write!(
                f,
                "；最早候选炮 ({}, {}) 还需 {}cs",
                grid.row + 1,
                grid.col + 1,
                remaining
            )?;
        }
        Ok(())
    }
}

/// One rejected cannon command. Coordinates use core conventions.
#[derive(Debug)]
pub struct CobError {
    pub operation: &'static str,
    /// One-based position within the input batch.
    pub input_number: usize,
    pub target: Option<CobTarget>,
    pub cannon: Option<Grid>,
    pub mode: Option<CobSequentialMode>,
    pub cause: CobManagerCallError,
}

impl fmt::Display for CobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}第 {} 项失败", self.operation, self.input_number)?;
        if let Some(target) = self.target {
            write!(f, "，落点 ({}, {})", target.row + 1, target.drop_col)?;
        }
        if let Some(grid) = self.cannon {
            write!(f, "，炮位 ({}, {})", grid.row + 1, grid.col + 1)?;
        }
        if let Some(mode) = self.mode {
            write!(f, "，炮序 {mode:?}")?;
        }
        write!(f, "：{}", self.cause)
    }
}

impl std::error::Error for CobError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

impl CobError {
    /// Stable category for diagnostic consumers; never parsed from human text.
    pub fn code(&self) -> &'static str {
        match &self.cause {
            CobManagerCallError::Backend(_) => "runtime_failure",
            CobManagerCallError::Rejected { .. } => "action_rejected",
            CobManagerCallError::Registration { .. } => "task_registration",
            CobManagerCallError::Manager(cause) => match cause {
                CobManagerError::EmptyList => "empty_list",
                CobManagerError::InvalidNext => "invalid_cursor",
                CobManagerError::InvalidSlot { .. } => "invalid_slot",
                CobManagerError::InvalidTarget
                | CobManagerError::InvalidInput { .. }
                | CobManagerError::InvalidRawInput { .. }
                | CobManagerError::InputConversion { .. } => "invalid_input",
                CobManagerError::InvalidModeForOperation { .. } => "invalid_mode",
                CobManagerError::CobNotFound(_) => "cannon_missing",
                CobManagerError::CobNotInList(_) => "cannon_not_in_list",
                CobManagerError::NoReadyCob | CobManagerError::SelectionUnavailable { recover: false, .. } => {
                    "no_ready_cannon"
                }
                CobManagerError::NoRecoverableCob | CobManagerError::SelectionUnavailable { recover: true, .. } => {
                    "no_recoverable_cannon"
                }
                CobManagerError::Reserved { .. } => "reserved",
                CobManagerError::Cooling { .. } => "cooldown",
                CobManagerError::Unrecoverable(_) => "unrecoverable",
                CobManagerError::NoLatestCob => "no_previous_shot",
                CobManagerError::NonRoofScene => "not_roof",
            },
        }
    }
    pub(crate) fn new(operation: &'static str, cause: impl Into<CobManagerCallError>) -> Self {
        Self {
            operation,
            input_number: 1,
            target: None,
            cannon: None,
            mode: None,
            cause: cause.into(),
        }
    }
}

impl From<RuntimeError> for CobError {
    fn from(error: RuntimeError) -> Self {
        Self::new("发炮", CobManagerCallError::Backend(error))
    }
}
impl From<CobManagerError> for CobError {
    fn from(error: CobManagerError) -> Self {
        Self::new("发炮", error)
    }
}
impl From<CobError> for RuntimeError {
    fn from(error: CobError) -> Self {
        Self::from_source(error)
    }
}

/// A processed prefix; accepted commands may be pending, not yet fired.
#[derive(Debug, Default)]
pub struct CobBatchReport {
    /// Present for ordinary batches; raw and prepared commands keep only counts.
    pub indices: Vec<Option<i32>>,
    pub processed: usize,
    pub accepted: usize,
    pub total: Option<usize>,
    pub complete: bool,
    pub failures: Vec<CobError>,
    /// Zero-based input at which execution stopped, if any.
    pub stopped_at: Option<usize>,
}

#[derive(Debug)]
pub struct CobBatchError {
    pub report: CobBatchReport,
}

impl fmt::Display for CobBatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "批量发炮：已处理 {} 项，接受 {} 项，失败 {} 项{}",
            self.report.processed,
            self.report.accepted,
            self.report.failures.len(),
            if self.report.complete { "" } else { "，提前停止" }
        )?;
        for error in &self.report.failures {
            write!(f, "\n{error}")?;
        }
        Ok(())
    }
}
impl std::error::Error for CobBatchError {}
impl From<RuntimeError> for CobBatchError {
    fn from(error: RuntimeError) -> Self {
        Self {
            report: CobBatchReport {
                failures: vec![error.into()],
                stopped_at: Some(0),
                ..Default::default()
            },
        }
    }
}
impl From<CobBatchError> for RuntimeError {
    fn from(error: CobBatchError) -> Self {
        Self::from_source(error)
    }
}
