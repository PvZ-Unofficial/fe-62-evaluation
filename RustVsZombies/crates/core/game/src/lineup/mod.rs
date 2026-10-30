//! Parsed lineup model and portable lineup-code parsers.

pub(crate) mod apply;
mod capture;
mod cell;
mod document;
mod encode;
mod error;
mod model;
mod native_apply;
mod parse;
mod validate;

#[cfg(test)]
mod tests;

pub use apply::LineupApplyOptions;
pub use apply::apply_lineup;
pub use apply::{ApplyLineupError, LineupApplyBackend};
pub use capture::capture_text;
pub use cell::{LineupBase, LineupCell, LineupPlant};
pub use document::{LineupDocument, LineupObject};
pub use error::LineupParseError;
pub use model::Lineup;
pub use native_apply::{NativeLineupBackend, NativeLineupPlan};

pub use crate::setup::{LineupReloadPolicy, PendingLineup};
