//! A-TAS SafeCard timing and geometry, with explicitly delegated fixed-slot use.
use crate::{
    access::{operation_error, with_backend},
    runtime::{RuntimeError, RuntimeResult},
    user_card::{CursorRequest, Outcome, UserCursorBackend},
};
use rsvz_backend_api::*;
use rsvz_current::CurrentBackend;
use rsvz_model::*;

#[derive(Clone, Copy, Debug)]
pub struct SmartCardOptions {
    pub survival_frames: u32,
}
impl SmartCardOptions {
    /// A-TAS SmartAsh: imitators/ordinary plants need 1cs; cherry, jalapeno
    /// and night ice/doom need 99cs. These are gameplay rules, not GUI defaults.
    pub fn for_card(card: CheckedCardSelection, scene: SceneKind) -> Self {
        let kind = card.effective_kind();
        Self {
            survival_frames: if card.imitator_target().is_none()
                && (matches!(kind, PlantKind::CherryBomb | PlantKind::Jalapeno)
                    || scene.is_night() && matches!(kind, PlantKind::IceShroom | PlantKind::DoomShroom))
            {
                99
            } else {
                1
            },
        }
    }
}
pub struct SmartCard {
    cursor: CursorRequest,
    grid: Grid,
    options: SmartCardOptions,
    last_clock: Option<i32>,
    started: bool,
}
impl SmartCard {
    pub fn capture() -> RuntimeResult<Self>
    where
        CurrentBackend: UserCursorBackend + SceneBackend,
    {
        Self::from_cursor(CursorRequest::capture()?)
    }
    pub fn from_cursor(cursor: CursorRequest) -> RuntimeResult<Self>
    where
        CurrentBackend: SceneBackend,
    {
        let (_, card) = cursor
            .card
            .ok_or_else(|| RuntimeError::new("hold a seed-bank card before smart planting"))?;
        let grid = smart_grid(cursor.position, cursor.preview_row)
            .ok_or_else(|| RuntimeError::new("cursor is outside the lawn"))?;
        let scene = with_backend(|b| b.scene()).map_err(operation_error)?;
        Ok(Self {
            cursor,
            grid,
            options: SmartCardOptions::for_card(card, scene),
            last_clock: None,
            started: false,
        })
    }
    pub fn with_options(mut self, options: SmartCardOptions) -> RuntimeResult<Self> {
        if options.survival_frames == 0 {
            return Err(RuntimeError::new("survival time must be positive"));
        }
        self.options = options;
        Ok(self)
    }
    pub fn started(&self) -> bool {
        self.started
    }
    pub fn cancel(&mut self) {
        self.cursor.cancel();
    }
    pub fn advance(&mut self) -> RuntimeResult<Outcome>
    where
        CurrentBackend:
            UserCursorBackend + crate::logic::cards::CardContext + InputBackend + ClockBackend + ZombieRawFactsBackend,
    {
        if let Some(outcome) = self.cursor.validate()? {
            self.cursor.cancel();
            return Ok(outcome);
        }
        let clock = with_backend(|b| b.clock()).map_err(operation_error)?;
        if self.last_clock == Some(clock) {
            return Ok(Outcome::Waiting);
        }
        self.last_clock = Some(clock);
        let (slot, card) = self.cursor.card.expect("captured card");
        let scene = with_backend(|b| b.scene()).map_err(operation_error)?;
        let waiting = with_backend(|b| -> RuntimeResult<bool> {
            // Match the original cherry query even when the selected card differs.
            let cherry = CardSelection::Plant(PlantKind::CherryBomb)
                .checked()
                .expect("valid kind");
            let needs_pot = matches!(
                b.can_plant_at(cherry, self.grid).map_err(operation_error)?,
                Plantability::Rejected(PlantRejectReason::RequiresPot)
                    | Plantability::Rejected(PlantRejectReason::GameRule(5))
            );
            let (x, y) = plant_position(scene, self.grid);
            let mut under_car = false;
            if needs_pot {
                for zombie in b.zombies().map_err(operation_error)? {
                    if matches!(
                        b.zombie_kind(zombie).map_err(operation_error)?,
                        ZombieKind::Zomboni | ZombieKind::Catapult
                    ) && b.zombie_row(zombie) == self.grid.row
                    {
                        let zx = b.zombie_pos_x(zombie) as i32;
                        if x + 50 >= zx + 10 && x + 30 <= zx + 143 {
                            under_car = true;
                            break;
                        }
                    }
                }
            }
            for zombie in b.zombies().map_err(operation_error)? {
                if b.zombie_kind(zombie).map_err(operation_error)? == ZombieKind::JackInTheBox
                    && b.zombie_phase(zombie).map_err(operation_error)? == ZombiePhase::JackInTheBoxPopping
                    && explosion_hits(
                        x,
                        y,
                        card.packet_kind(),
                        b.zombie_pos_x(zombie) as i32,
                        b.zombie_pos_y(zombie) as i32,
                    )
                {
                    let countdown = b.zombie_phase_counter(zombie);
                    if if under_car {
                        countdown == 1
                    } else {
                        i64::from(countdown) <= i64::from(self.options.survival_frames)
                    } {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        })?;
        if waiting {
            return Ok(Outcome::Waiting);
        }
        self.cursor.release()?;
        self.started = true;
        let receipt = crate::logic::cards::plant_captured_slot(slot, card, self.grid)
            .map_err(|e| RuntimeError::new(e.to_string()))?;
        Ok(Outcome::Planted {
            plant: receipt.main,
            container: receipt.auto_container,
        })
    }
}
fn smart_grid(position: PixelPos, row: i32) -> Option<Grid> {
    if position.x < 0 || !(45..=565).contains(&position.y) {
        return None;
    }
    let col = ((i64::from(position.x) + 65) / 80).min(9) - 1;
    let grid = Grid { row, col: col as i32 };
    grid.is_in_bounds(6, 9).then_some(grid)
}
pub(crate) fn plant_position(scene: SceneKind, grid: Grid) -> (i32, i32) {
    let x = 40 + grid.col * 80;
    let y = if scene.has_roof() {
        70 + grid.row * 85 + (5 - grid.col).max(0) * 20
    } else {
        80 + grid.row * if scene.has_pool() { 85 } else { 100 }
    };
    (x, y)
}
fn explosion_hits(x: i32, y: i32, kind: PlantKind, zx: i32, zy: i32) -> bool {
    let dy = (i64::from(y) - i64::from(zy) - 60)
        .max(i64::from(zy) + 60 - i64::from(y) - 80)
        .max(0);
    if dy > 90 {
        return false;
    }
    let dx = ((8100 - dy * dy) as f64).sqrt() as i64;
    let (left, right) = match kind {
        PlantKind::TallNut => (10, 90),
        PlantKind::Pumpkin => (0, 100),
        PlantKind::CobCannon => (0, 140),
        _ => (10, 70),
    };
    let cx = i64::from(zx) + 60;
    i64::from(x) + left - dx <= cx && cx <= i64::from(x) + right + dx
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atas_boundaries_and_policy() {
        assert_eq!(smart_grid(PixelPos { x: 55, y: 65 }, 0), Some(Grid { row: 0, col: 0 }));
        assert_eq!(smart_grid(PixelPos { x: 999, y: 65 }, 0).unwrap().col, 8);
        assert!(smart_grid(PixelPos { x: -1, y: 65 }, 0).is_none());
        let card = CardSelection::Plant(PlantKind::IceShroom).checked().unwrap();
        assert_eq!(SmartCardOptions::for_card(card, SceneKind::Day).survival_frames, 1);
        assert_eq!(SmartCardOptions::for_card(card, SceneKind::Night).survival_frames, 99);
        assert!(explosion_hits(40, 80, PlantKind::CherryBomb, 0, 20));
        assert!(!explosion_hits(40, 80, PlantKind::CherryBomb, 0, 191));
        assert_eq!(plant_position(SceneKind::Roof, Grid { row: 0, col: 0 }), (40, 170));
    }
}
