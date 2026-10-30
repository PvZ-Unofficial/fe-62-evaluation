//! Ordered results of an immediate multi-card command.
use super::{CardError, CardResult, PlantId, RuntimeError};

#[derive(Debug)]
pub struct CardFailure {
    /// One-based position in the input batch.
    pub input_number: usize,
    pub error: CardError,
}

#[derive(Debug, Default)]
pub struct CardBatchReport {
    /// One entry per attempted card; successful plants are never rolled back.
    pub plants: Vec<Option<PlantId>>,
    pub failures: Vec<CardFailure>,
}
impl CardBatchReport {
    pub(super) fn push(&mut self, result: CardResult<PlantId>) {
        self.plants.push(match result {
            Ok(id) => Some(id),
            Err(error) => {
                self.failures.push(CardFailure {
                    input_number: self.plants.len() + 1,
                    error,
                });
                None
            }
        });
    }
    pub(super) fn finish(self) -> Result<Self, CardBatchError> {
        if self.failures.is_empty() {
            Ok(self)
        } else {
            Err(CardBatchError { report: self })
        }
    }
}
#[derive(Debug)]
pub struct CardBatchError {
    pub report: CardBatchReport,
}
impl std::fmt::Display for CardBatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "批量种卡：已处理 {} 项，失败 {} 项",
            self.report.plants.len(),
            self.report.failures.len()
        )?;
        for failure in &self.report.failures {
            write!(f, "\n第 {} 项：{}", failure.input_number, failure.error)?;
        }
        Ok(())
    }
}
impl std::error::Error for CardBatchError {}
impl From<CardBatchError> for RuntimeError {
    fn from(error: CardBatchError) -> Self {
        Self::from_source(error)
    }
}
