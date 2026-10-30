//! AvZ AFodder/C: candidate selection at execution time and position/type cleanup.
//!
//! `c.set_cards([puff, pot])?; 300 << c(80, 1256);`
//! Non-positive retention means no cleanup. Cleanup does not move the expression cursor.
//! Unlike `card`, this uses native ACard semantics without automatically planting containers.

use super::expr::{Expr, Leaf, PrepareContext};
use super::primitives::{IntoCobRows, IntoDropCol};
use rsvz_backend_api::{
    CurrentWaveBackend, GridTerrainBackend, SceneBackend, SeedBankReadBackend, SeedDefinitionBackend,
    ZombieRawFactsBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_game::fodder::{self, Fodder as Manager};
use rsvz_game::logic::cards::CardContext;
use rsvz_model::Grid;
use std::cell::RefCell;
use std::rc::Rc;

pub use rsvz_game::fodder::Constraint as FodderConstraint;

#[expect(non_upper_case_globals, reason = "DSL predicates mirror function-style script names")]
pub const curr_wave: FodderConstraint = FodderConstraint::CurrentWave;
#[expect(non_upper_case_globals, reason = "DSL predicates mirror function-style script names")]
pub const prev_waves: FodderConstraint = FodderConstraint::PreviousWaves;

pub fn wave_in(waves: impl IntoIterator<Item = i32>) -> FodderConstraint {
    FodderConstraint::RelativeWaves(waves.into_iter().collect())
}

/// Inclusive bounds on the zombie's truncated native x coordinate (AvZ AbscIn).
pub fn absc_in(left: i32, right: i32) -> FodderConstraint {
    FodderConstraint::Abscissa(left, right)
}

/// Independent managers can be created with `Fodder::new()`; `c` uses the default list.
#[derive(Clone)]
pub struct Fodder {
    manager: Manager,
    constraints: Option<Rc<[FodderConstraint]>>,
}

#[expect(non_upper_case_globals, reason = "low DSL uses lower-case callable names")]
pub const c: Fodder = Fodder {
    manager: Manager::DEFAULT,
    constraints: None,
};

impl std::ops::Deref for Fodder {
    type Target = Manager;
    fn deref(&self) -> &Self::Target {
        &self.manager
    }
}

impl Default for Fodder
where
    CurrentBackend: SeedBankReadBackend + SeedDefinitionBackend,
{
    fn default() -> Self {
        Self::new()
    }
}

impl Fodder {
    pub fn new() -> Self
    where
        CurrentBackend: SeedBankReadBackend + SeedDefinitionBackend,
    {
        Self {
            manager: Manager::new(),
            constraints: None,
        }
    }
    /// Constraints are ORed; `&` combines predicates within a constraint.
    pub fn trigger_by<T: Into<FodderConstraint>>(&self, constraints: impl IntoIterator<Item = T>) -> Self {
        Self {
            manager: self.manager.clone(),
            constraints: Some(constraints.into_iter().map(Into::into).collect()),
        }
    }

    fn expression(&self, delay: i32, positions: Result<Vec<Grid>, String>) -> Expr
    where
        CurrentBackend: CardContext + SceneBackend + SeedDefinitionBackend + ZombieRawFactsBackend + CurrentWaveBackend,
    {
        let positions = match positions {
            Ok(value) => value,
            Err(error) => return Expr::error(error),
        };
        let manager = self.manager.clone();
        manager.install();
        Expr::from_leaf(FodderLeaf {
            manager,
            delay,
            positions: positions.into(),
            constraints: self.constraints.clone(),
        })
    }
}

/// Packed rows (`1256`), a `(packed_rows, column)` pair, or a list of such pairs.
pub trait IntoFodderPositions {
    fn into_fodder_positions(self) -> Result<Vec<Grid>, String>;
}
impl IntoFodderPositions for i32 {
    fn into_fodder_positions(self) -> Result<Vec<Grid>, String> {
        (self, 9).into_fodder_positions()
    }
}
impl<C: IntoDropCol> IntoFodderPositions for (i32, C) {
    fn into_fodder_positions(self) -> Result<Vec<Grid>, String> {
        let col = self.1.into_drop_col();
        if !col.is_finite() || !(0.5..9.5).contains(&col) {
            return Err("fodder column must round to 1..=9".into());
        }
        let col = (col + 0.5) as i32;
        self.0
            .into_cob_rows()?
            .into_iter()
            .map(|row| Grid::from_one_based(row, col).map_err(|e| e.to_string()))
            .collect()
    }
}
impl<C: IntoDropCol, const N: usize> IntoFodderPositions for [(i32, C); N] {
    fn into_fodder_positions(self) -> Result<Vec<Grid>, String> {
        positions(self)
    }
}
impl<C: IntoDropCol> IntoFodderPositions for Vec<(i32, C)> {
    fn into_fodder_positions(self) -> Result<Vec<Grid>, String> {
        positions(self)
    }
}
impl<C: IntoDropCol + Copy> IntoFodderPositions for &[(i32, C)] {
    fn into_fodder_positions(self) -> Result<Vec<Grid>, String> {
        positions(self.iter().copied())
    }
}
fn positions<C: IntoDropCol>(input: impl IntoIterator<Item = (i32, C)>) -> Result<Vec<Grid>, String> {
    let mut result = Vec::new();
    for position in input {
        result.extend(position.into_fodder_positions()?);
    }
    Ok(result)
}

crate::callable::impl_callable! {
    impl<> Fodder
    where { CurrentBackend: CardContext + SceneBackend + SeedDefinitionBackend + ZombieRawFactsBackend + CurrentWaveBackend + GridTerrainBackend, }
    call_as(this; delay: i32) -> Expr { this.expression(delay, Ok(fodder::land_positions())) }
}
crate::callable::impl_callable! {
    impl<P> Fodder
    where { P: IntoFodderPositions, CurrentBackend: CardContext + SceneBackend + SeedDefinitionBackend + ZombieRawFactsBackend + CurrentWaveBackend, }
    call_as(this; delay: i32, positions: P) -> Expr { this.expression(delay, positions.into_fodder_positions()) }
}

struct FodderLeaf {
    manager: Manager,
    delay: i32,
    positions: Rc<[Grid]>,
    constraints: Option<Rc<[FodderConstraint]>>,
}
impl Leaf for FodderLeaf
where
    CurrentBackend: CardContext + SceneBackend + ZombieRawFactsBackend + CurrentWaveBackend,
{
    fn prepare(&self, context: &mut PrepareContext<'_>) {
        let targets = Rc::new(RefCell::new(Vec::with_capacity(self.positions.len())));
        let planted = Rc::clone(&targets);
        let manager = self.manager.clone();
        let positions = Rc::clone(&self.positions);
        let constraints = self.constraints.clone();
        context.push(
            context.semantic_time(),
            Box::new(move || {
                manager.plant(
                    &positions,
                    constraints.as_deref().unwrap_or(&[]),
                    &mut planted.borrow_mut(),
                )
            }),
        );
        if self.delay > 0 {
            context.push(
                context.semantic_time() + i128::from(self.delay),
                Box::new(move || fodder::remove(&targets.borrow())),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packed_positions_preserve_order_duplicates_and_native_column_rounding() {
        let positions = [(151, 8.4), (26, 8.6)].into_fodder_positions().unwrap();
        assert_eq!(
            positions,
            vec![
                Grid { row: 0, col: 7 },
                Grid { row: 4, col: 7 },
                Grid { row: 0, col: 7 },
                Grid { row: 1, col: 8 },
                Grid { row: 5, col: 8 }
            ]
        );
        assert!(10.into_fodder_positions().is_err());
        assert!((2, f32::NAN).into_fodder_positions().is_err());
    }
}
