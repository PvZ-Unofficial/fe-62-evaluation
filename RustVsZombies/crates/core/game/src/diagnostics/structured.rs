//! Serialize borrowed concrete errors only at the diagnostic output boundary.
use super::LogRecord;
use crate::cob::{CobBatchError, CobError, CobManagerCallError, CobManagerError};
use serde::{Serialize, Serializer, ser::SerializeMap};
use std::{
    error::Error,
    hash::{Hash, Hasher},
};
fn cause<'a, T: Error + 'static>(mut error: Option<&'a (dyn Error + 'static)>) -> Option<&'a T> {
    while let Some(value) = error {
        if let Some(value) = value.downcast_ref::<T>() {
            return Some(value);
        }
        error = value.source();
    }
    None
}
pub(super) struct Payload<'a, 'b>(pub &'a LogRecord<'b>);
impl Serialize for Payload<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let record = self.0;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("message", record.message)?;
        map.serialize_entry("message_truncated", &record.message_truncated)?;
        map.serialize_entry(
            "level",
            match record.level {
                super::LogLevel::Debug => "debug",
                super::LogLevel::Info => "info",
                super::LogLevel::Warning => "warning",
                super::LogLevel::Error => "error",
            },
        )?;
        if let Some(error) = cause::<CobError>(record.error) {
            map.serialize_entry("cannon", &Cannon(error))?;
        } else if let Some(error) = cause::<CobManagerError>(record.error) {
            map.serialize_entry("cannon", &Cannon(&CobError::new("炮管理器操作", *error)))?;
        } else if let Some(CobManagerCallError::Manager(error)) = cause::<CobManagerCallError>(record.error) {
            map.serialize_entry("cannon", &Cannon(&CobError::new("炮管理器操作", *error)))?;
        }
        if let Some(error) = cause::<CobBatchError>(record.error) {
            let report = &error.report;
            map.serialize_entry("processed", &report.processed)?;
            map.serialize_entry("accepted", &report.accepted)?;
            map.serialize_entry("total", &report.total)?;
            map.serialize_entry("complete", &report.complete)?;
            map.serialize_entry("stopped_at", &report.stopped_at)?;
            map.serialize_entry("indices", &report.indices)?;
            struct Failures<'a>(&'a [CobError]);
            impl Serialize for Failures<'_> {
                fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                    s.collect_seq(self.0.iter().map(Cannon))
                }
            }
            map.serialize_entry("failures", &Failures(&report.failures))?;
        }
        if let Some(error) = cause::<crate::logic::cards::CardError>(record.error) {
            map.serialize_entry("card", &Card(error))?;
        }
        if let Some(error) = cause::<crate::logic::cards::CardBatchError>(record.error) {
            map.serialize_entry("card_processed", &error.report.plants.len())?;
            map.serialize_entry("card_failures", &CardFailures(&error.report.failures))?;
        }
        if let Some(error) = cause::<crate::logic::shovel::ShovelError>(record.error) {
            map.serialize_entry("shovel", &Shovel(error))?;
        }
        if let Some(error) = cause::<crate::logic::shovel::ShovelBatchError>(record.error) {
            struct Failures<'a>(&'a [crate::logic::shovel::ShovelFailure]);
            impl Serialize for Failures<'_> {
                fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                    s.collect_seq(self.0.iter().map(|f| (f.input_number, Shovel(&f.error))))
                }
            }
            map.serialize_entry("shovel_failures", &Failures(&error.failures))?;
        }
        if let Some(error) = cause::<crate::logic::ice_filler::IceFillerError>(record.error) {
            use crate::logic::ice_filler::IceFillerError::*;
            let code = match error {
                Rejected(_) => "action_rejected",
                EmptyIceSeedList => "empty_ice_seed_list",
                InvalidIceSeed(_) => "invalid_ice_seed",
                InvalidGrid => "invalid_grid",
                NightScene => "night_scene",
                MissingIceSeed => "missing_ice_seed",
                MissingCoffeeSeed => "missing_coffee_seed",
                MixedIceSeedList => "mixed_ice_seed_list",
            };
            map.serialize_entry("ice_filler_code", code)?;
        }
        if let Some(error) = cause::<crate::logic::plant_fixer::PlantFixerError>(record.error) {
            use crate::logic::plant_fixer::PlantFixerError::*;
            let code = match error {
                Rejected(_) => "action_rejected",
                InvalidCoordinates(_) | InvalidGrid { .. } => "invalid_grid",
                ImitatorTarget => "imitator_target",
                InvalidThreshold(_) => "invalid_threshold",
                InvalidRunInterval(_) => "invalid_run_interval",
                MissingSeed(_) => "missing_seed",
            };
            map.serialize_entry("plant_fixer_code", code)?;
            match error {
                InvalidGrid { grid, .. } => map.serialize_entry("plant_fixer_grid", &(grid.row, grid.col))?,
                InvalidThreshold(value) => map.serialize_entry("plant_fixer_threshold", value)?,
                InvalidRunInterval(value) => map.serialize_entry("plant_fixer_interval", value)?,
                MissingSeed(kind) => map.serialize_entry("plant_fixer_kind", &(*kind as i32))?,
                _ => {}
            }
        }
        map.end()
    }
}
struct Cannon<'a>(&'a CobError);
impl Serialize for Cannon<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let error = self.0;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("code", error.code())?;
        map.serialize_entry("operation", error.operation)?;
        map.serialize_entry("input_number", &error.input_number)?;
        map.serialize_entry("target", &error.target.map(|t| (t.row, t.drop_col)))?;
        map.serialize_entry("cannon", &error.cannon.map(|g| (g.row, g.col)))?;
        map.serialize_entry(
            "mode",
            &error.mode.map(|mode| match mode {
                crate::cob::CobSequentialMode::Space => "space",
                crate::cob::CobSequentialMode::Time => "time",
                crate::cob::CobSequentialMode::Priority => "priority",
            }),
        )?;
        if let CobManagerCallError::Manager(cause) = &error.cause {
            match cause {
                CobManagerError::InvalidInput { row, column } => {
                    map.serialize_entry("input_row", row)?;
                    map.serialize_entry("input_column", column)?;
                }
                CobManagerError::InvalidRawInput {
                    cannon,
                    row,
                    column,
                    cannon_one_based,
                    target_one_based,
                } => {
                    map.serialize_entry("input_cannon", &(cannon.row, cannon.col))?;
                    map.serialize_entry("input_row", row)?;
                    map.serialize_entry("input_column", column)?;
                    map.serialize_entry("cannon_one_based", cannon_one_based)?;
                    map.serialize_entry("target_one_based", target_one_based)?;
                }
                CobManagerError::InputConversion { reason } => {
                    map.serialize_entry("conversion_reason", reason)?;
                }
                CobManagerError::InvalidSlot { requested, count } => {
                    map.serialize_entry("requested_slot", requested)?;
                    map.serialize_entry("list_count", count)?;
                }
                CobManagerError::InvalidModeForOperation { mode, operation } => {
                    map.serialize_entry("rejected_operation", operation)?;
                    map.serialize_entry(
                        "rejected_mode",
                        match mode {
                            crate::cob::CobSequentialMode::Space => "space",
                            crate::cob::CobSequentialMode::Time => "time",
                            crate::cob::CobSequentialMode::Priority => "priority",
                        },
                    )?;
                }
                CobManagerError::Cooling {
                    remaining, available, ..
                } => {
                    map.serialize_entry("remaining_cs", remaining)?;
                    map.serialize_entry("available_cs", available)?;
                }
                CobManagerError::SelectionUnavailable { evidence, recover } => {
                    map.serialize_entry("recover", recover)?;
                    map.serialize_entry("examined", &evidence.examined)?;
                    map.serialize_entry("missing", &evidence.missing)?;
                    map.serialize_entry("reserved", &evidence.reserved)?;
                    map.serialize_entry("unrecoverable", &evidence.unrecoverable)?;
                    map.serialize_entry("earliest", &evidence.earliest.map(|(g, t)| (g.row, g.col, t)))?;
                }
                _ => {}
            }
        }
        map.end()
    }
}
pub(super) fn key(record: &LogRecord<'_>) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    fn cannon(error: &CobError, hash: &mut impl Hasher) {
        error.code().hash(hash);
        error.operation.hash(hash);
        error.input_number.hash(hash);
        error.target.map(|t| (t.row, t.drop_col.to_bits())).hash(hash);
        error.cannon.hash(hash);
        error.mode.hash(hash);
        match &error.cause {
            CobManagerCallError::Manager(CobManagerError::InvalidInput { row, column }) => {
                row.hash(hash);
                column.hash(hash);
            }
            CobManagerCallError::Manager(CobManagerError::InvalidRawInput {
                cannon,
                row,
                column,
                cannon_one_based,
                target_one_based,
            }) => {
                cannon.hash(hash);
                row.hash(hash);
                column.hash(hash);
                cannon_one_based.hash(hash);
                target_one_based.hash(hash);
            }
            CobManagerCallError::Manager(CobManagerError::InputConversion { reason }) => reason.hash(hash),
            _ => {}
        }
    }
    record.level.hash(&mut hash);
    record.error.is_some().hash(&mut hash);
    if let Some(error) = cause::<CobError>(record.error) {
        cannon(error, &mut hash);
    } else if let Some(error) = cause::<CobBatchError>(record.error) {
        for error in &error.report.failures {
            cannon(error, &mut hash);
        }
    } else if cause::<crate::logic::cards::CardBatchError>(record.error).is_some()
        || cause::<crate::logic::shovel::ShovelBatchError>(record.error).is_some()
    {
        // The displayed message is bounded; distinct failures beyond that prefix
        // must not be folded into one repeat record. Hash the borrowed full error.
        struct TextHash<'a>(&'a mut std::collections::hash_map::DefaultHasher);
        impl std::fmt::Write for TextHash<'_> {
            fn write_str(&mut self, text: &str) -> std::fmt::Result {
                self.0.write(text.as_bytes());
                Ok(())
            }
        }
        let _ = std::fmt::write(&mut TextHash(&mut hash), format_args!("{}", record.error.unwrap()));
    } else {
        record.message.hash(&mut hash);
    }
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(error: crate::runtime::RuntimeError) -> serde_json::Value {
        let record = LogRecord {
            error: Some(&error),
            ..LogRecord::new(
                super::super::LogLevel::Error,
                "context",
                super::super::LogContext::Unscoped,
            )
        };
        serde_json::to_value(Payload(&record)).unwrap()
    }

    #[test]
    fn shovel_and_manager_payloads_follow_runtime_sources() {
        use crate::logic::ice_filler::IceFillerError;
        use crate::logic::plant_fixer::PlantFixerError;
        use crate::logic::shovel::{ShovelError, ShovelTarget};
        use crate::model::{CardSelection, Grid, GridError, PlantKind};
        use crate::runtime::RuntimeError;

        let error = ShovelError::Rejected {
            grid: Grid { row: 2, col: 4 },
            target: ShovelTarget::Plant(CardSelection::Imitator(PlantKind::IceShroom)),
            cause: RuntimeError::from_source(GridError::NegativeCoordinate),
        };
        let value = payload(RuntimeError::from(error).context("outer"));
        assert_eq!(value["shovel"]["coordinates"], serde_json::json!([3, 5]));
        assert_eq!(value["shovel"]["target"], "plant");
        assert_eq!(value["shovel"]["plant_kind"], PlantKind::IceShroom as i32);
        assert_eq!(value["shovel"]["imitator"], true);
        let error = IceFillerError::Rejected(RuntimeError::from_source(GridError::NegativeCoordinate));
        let error = RuntimeError::from(error).context("coffee");
        assert!(error.find_source::<GridError>().is_some());
        assert_eq!(payload(error)["ice_filler_code"], "action_rejected");
        let value = payload(RuntimeError::from(PlantFixerError::InvalidRunInterval(0)).context("config"));
        assert_eq!(value["plant_fixer_code"], "invalid_run_interval");
        assert_eq!(value["plant_fixer_interval"], 0);
    }

    #[cfg(all(test, feature = "backend-tests"))]
    #[test]
    fn card_payload_preserves_selection_coordinates_and_batch_positions() {
        use crate::logic::cards::{card, cards};
        use crate::model::{CardSelection, PlantKind};
        use crate::runtime::RuntimeError;
        let selection = CardSelection::Imitator(PlantKind::IceShroom);
        let value = payload(RuntimeError::from(card(selection, 0, 1).unwrap_err()).context("outer"));
        assert_eq!(value["card"]["code"], "invalid_grid");
        assert_eq!(value["card"]["coordinates"], serde_json::json!([0, 1]));
        assert_eq!(value["card"]["plant_kind"], PlantKind::IceShroom as i32);
        assert_eq!(value["card"]["imitator"], true);
        let value =
            payload(RuntimeError::from(cards([(selection, 0, 1), (selection, 1, 0)]).unwrap_err()).context("outer"));
        assert_eq!(value["card_processed"], 2);
        assert_eq!(value["card_failures"][0][0], 1);
        assert_eq!(value["card_failures"][1][0], 2);
        assert_eq!(value["card_failures"][1][1]["coordinates"], serde_json::json!([1, 0]));
    }
    #[test]
    fn equal_text_does_not_merge_logs_and_operation_failures() {
        let log = LogRecord::new(
            super::super::LogLevel::Error,
            "same text",
            super::super::LogContext::Unscoped,
        );
        let error = crate::runtime::RuntimeError::new("same text");
        let failure = LogRecord {
            error: Some(&error),
            ..log
        };
        assert_ne!(key(&log), key(&failure));
    }

    #[test]
    fn a_truncated_batch_prefix_does_not_hide_distinct_later_failures() {
        use crate::logic::shovel::{ShovelBatchError, ShovelError, ShovelFailure};
        let batch = |col| ShovelBatchError {
            failures: vec![ShovelFailure {
                input_number: 1000,
                error: ShovelError::InvalidGrid {
                    coordinates: Some((0, col)),
                    cause: crate::model::GridError::NegativeCoordinate,
                },
            }],
        };
        let first = batch(1);
        let second = batch(2);
        let record = LogRecord {
            error: Some(&first),
            message_truncated: true,
            ..LogRecord::new(
                super::super::LogLevel::Error,
                "same truncated prefix",
                super::super::LogContext::Unscoped,
            )
        };
        assert_ne!(
            key(&record),
            key(&LogRecord {
                error: Some(&second),
                ..record
            })
        );
    }
}

/// Even when full detail exceeds the record bound, keep known batch counts and
/// an explicitly truncated prefix before a dialog can interrupt its caller.
pub(super) struct Summary<'a, 'b>(pub &'a LogRecord<'b>);
impl Serialize for Summary<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("details_truncated", &true)?;
        if let Some(error) = cause::<CobBatchError>(self.0.error) {
            let report = &error.report;
            map.serialize_entry("processed", &report.processed)?;
            map.serialize_entry("accepted", &report.accepted)?;
            map.serialize_entry("total", &report.total)?;
            map.serialize_entry("complete", &report.complete)?;
            map.serialize_entry("stopped_at", &report.stopped_at)?;
            map.serialize_entry("failure_count", &report.failures.len())?;
            map.serialize_entry("indices_prefix", &report.indices[..report.indices.len().min(64)])?;
            map.serialize_entry("indices_truncated", &(report.indices.len() > 64))?;
            map.serialize_entry("first_code", &report.failures.first().map(CobError::code))?;
            map.serialize_entry("last_code", &report.failures.last().map(CobError::code))?;
        } else if let Some(error) = cause::<CobError>(self.0.error) {
            map.serialize_entry("code", error.code())?;
            map.serialize_entry("input_number", &error.input_number)?;
        }
        if let Some(error) = cause::<crate::logic::cards::CardBatchError>(self.0.error) {
            map.serialize_entry("card_processed", &error.report.plants.len())?;
            map.serialize_entry("card_failure_count", &error.report.failures.len())?;
            map.serialize_entry(
                "card_first_failure",
                &error
                    .report
                    .failures
                    .first()
                    .map(|failure| (failure.input_number, Card(&failure.error))),
            )?;
            map.serialize_entry(
                "card_last_failure",
                &error
                    .report
                    .failures
                    .last()
                    .map(|failure| (failure.input_number, Card(&failure.error))),
            )?;
        } else if let Some(error) = cause::<crate::logic::cards::CardError>(self.0.error) {
            map.serialize_entry("card", &Card(error))?;
        }
        if let Some(error) = cause::<crate::logic::shovel::ShovelBatchError>(self.0.error) {
            map.serialize_entry("shovel_failure_count", &error.failures.len())?;
            map.serialize_entry(
                "shovel_first_failure",
                &error
                    .failures
                    .first()
                    .map(|failure| (failure.input_number, Shovel(&failure.error))),
            )?;
            map.serialize_entry(
                "shovel_last_failure",
                &error
                    .failures
                    .last()
                    .map(|failure| (failure.input_number, Shovel(&failure.error))),
            )?;
        } else if let Some(error) = cause::<crate::logic::shovel::ShovelError>(self.0.error) {
            map.serialize_entry("shovel", &Shovel(error))?;
        }
        map.end()
    }
}

struct Card<'a>(&'a crate::logic::cards::CardError);
impl Serialize for Card<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use crate::logic::cards::CardErrorKind;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("code", self.0.code())?;
        map.serialize_entry("coordinates", &self.0.coordinates)?;
        if let Some(selection) = self.0.selection {
            let (kind, imitator) = match selection {
                crate::model::CardSelection::Plant(kind) => (kind, false),
                crate::model::CardSelection::Imitator(kind) => (kind, true),
            };
            map.serialize_entry("plant_kind", &(kind as i32))?;
            map.serialize_entry("imitator", &imitator)?;
        }
        if let CardErrorKind::Cooldown { remaining, .. } = self.0.kind() {
            map.serialize_entry("remaining_cs", &remaining)?;
        }
        if let CardErrorKind::MissingSlot { slot } = self.0.kind() {
            map.serialize_entry("slot", &slot.index())?;
        }
        if let CardErrorKind::Rejected { reason, .. } = self.0.kind() {
            use crate::model::PlantRejectReason::*;
            map.serialize_entry(
                "reject_reason",
                match reason {
                    OutOfBounds => "out_of_bounds",
                    NotEnoughSun => "not_enough_sun",
                    Occupied => "occupied",
                    WrongTerrain => "wrong_terrain",
                    RequiresPot => "requires_pot",
                    RequiresLilypad => "requires_lilypad",
                    PoolFull => "pool_full",
                    GameRule(_) => "game_rule",
                },
            )?;
            if let GameRule(code) = reason {
                map.serialize_entry("game_rule", &code)?;
            }
        }
        map.end()
    }
}
struct CardFailures<'a>(&'a [crate::logic::cards::CardFailure]);
impl Serialize for CardFailures<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|f| (f.input_number, Card(&f.error))))
    }
}
struct Shovel<'a>(&'a crate::logic::shovel::ShovelError);
impl Serialize for Shovel<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use crate::logic::shovel::ShovelError;
        let mut map = serializer.serialize_map(None)?;
        match self.0 {
            ShovelError::InvalidGrid { coordinates, .. } => {
                map.serialize_entry("code", "invalid_grid")?;
                map.serialize_entry("coordinates", coordinates)?;
            }
            ShovelError::Rejected { grid, target, .. } => {
                map.serialize_entry("code", "action_rejected")?;
                map.serialize_entry("coordinates", &(i64::from(grid.row) + 1, i64::from(grid.col) + 1))?;
                match target {
                    crate::logic::shovel::ShovelTarget::Default => map.serialize_entry("target", "default")?,
                    crate::logic::shovel::ShovelTarget::Plant(selection) => {
                        let (kind, imitator) = match selection {
                            crate::model::CardSelection::Plant(kind) => (kind, false),
                            crate::model::CardSelection::Imitator(kind) => (kind, true),
                        };
                        map.serialize_entry("target", "plant")?;
                        map.serialize_entry("plant_kind", &(*kind as i32))?;
                        map.serialize_entry("imitator", &imitator)?;
                    }
                }
            }
        }
        map.end()
    }
}
