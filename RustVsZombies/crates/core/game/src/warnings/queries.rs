use crate::{
    access::{operation_error, with_backend},
    runtime::{RuntimeError, RuntimeResult},
};
use rsvz_backend_api::ZombieVerticalPositionBackend;
use rsvz_backend_api::{
    CursorSeedReadBackend, PlantPresenceBackend, PlantReadBackend, PlantStateBackend, SceneBackend,
    SeedBankReadBackend, ZombieRawFactsBackend, ZombieReadBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{CardSelection, CheckedCardSelection, I32RepresentableF32, PlantKind, ZombieKind, ZombiePhase};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JackWarningMode {
    Off,
    OnlyWhenHit,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BalloonNotice {
    BiteBlover,
    Warning,
}
impl BalloonNotice {
    pub const fn text(self) -> &'static str {
        match self {
            Self::BiteBlover => "Bite Blover",
            Self::Warning => "Balloon Warning",
        }
    }
}

/// A-TAS qualification, not permission for a script to consume a held card.
/// As in AGetCardIndex, only the first matching packet supplies usability.
pub fn card_usable_or_held(selection: CheckedCardSelection) -> RuntimeResult<bool>
where
    CurrentBackend: SeedBankReadBackend + CursorSeedReadBackend,
{
    with_backend(|backend| {
        for packet in backend.seeds().map_err(operation_error)? {
            // Empty or special packets cannot match an ordinary checked card.
            if backend.seed_selection(packet).ok() == Some(selection) {
                if backend.seed_is_usable(packet) {
                    return Ok(true);
                }
                break;
            }
        }
        if let Some(kind) = selection.imitator_target() {
            Ok(backend.cursor_imitater_kind().map_err(operation_error)? == kind.code())
        } else {
            Ok(backend.cursor_seed_kind().map_err(operation_error)? == selection.packet_kind().code())
        }
    })
}

fn available(kind: PlantKind) -> RuntimeResult<bool>
where
    CurrentBackend: SeedBankReadBackend + CursorSeedReadBackend,
{
    card_usable_or_held(
        CardSelection::Plant(kind)
            .checked()
            .expect("ordinary warning counter-card"),
    )
}

fn integer(value: f32) -> RuntimeResult<i32> {
    I32RepresentableF32::new(value)
        .map(|value| value.get() as i32)
        .map_err(|e| RuntimeError::new(e.to_string()))
}

/// Fixed A-TAS explosion geometry, including its integer center conversion.
fn explosion_hits(kind: PlantKind, plant_x: i32, plant_y: i32, zombie_x: f32, zombie_y: f32) -> RuntimeResult<bool> {
    let x = i64::from(integer(zombie_x + 60.0)?);
    let y = i64::from(integer(zombie_y + 60.0)?);
    let plant_y = i64::from(plant_y);
    let dy = if y < plant_y {
        plant_y - y
    } else {
        (y - plant_y - 80).max(0)
    };
    if dy > 90 {
        return Ok(false);
    }
    let dx = ((8100 - dy * dy) as f64).sqrt() as i64;
    let (left, right) = match kind {
        PlantKind::TallNut => (10, 90),
        PlantKind::Pumpkin => (0, 100),
        PlantKind::CobCannon => (0, 140),
        _ => (10, 70),
    };
    Ok(i64::from(plant_x) + left - dx <= x && x <= i64::from(plant_x) + right + dx)
}

pub fn jack_should_pause(mode: JackWarningMode) -> RuntimeResult<bool>
where
    CurrentBackend: SeedBankReadBackend
        + CursorSeedReadBackend
        + SceneBackend
        + PlantStateBackend
        + PlantPresenceBackend
        + ZombieRawFactsBackend,
{
    if mode == JackWarningMode::Off {
        return Ok(false);
    }
    let night = with_backend(|backend| backend.scene())
        .map_err(operation_error)?
        .is_night();
    if !available(PlantKind::CherryBomb)?
        && !available(PlantKind::Jalapeno)?
        && !(night && available(PlantKind::DoomShroom)?)
    {
        return Ok(false);
    }
    with_backend(|backend| {
        for zombie in backend.zombies().map_err(operation_error)? {
            if backend.zombie_kind(zombie).map_err(operation_error)? != ZombieKind::JackInTheBox
                || backend.zombie_phase(zombie).map_err(operation_error)? != ZombiePhase::JackInTheBoxPopping
                || backend.zombie_phase_counter(zombie) != 110
            {
                continue;
            }
            if mode == JackWarningMode::All {
                return Ok(true);
            }
            for plant in backend.present_plants().map_err(operation_error)? {
                let kind = backend.plant_raw_kind(plant).map_err(operation_error)?;
                if backend.plant_is_squished(plant)
                    || backend.plant_on_bungee_state(plant) == 2
                    || kind == PlantKind::Squash && matches!(backend.plant_state(plant), 5 | 6 | 7)
                    || matches!(
                        kind,
                        PlantKind::Blover | PlantKind::CherryBomb | PlantKind::Jalapeno | PlantKind::CoffeeBean
                    )
                    || night && matches!(kind, PlantKind::DoomShroom | PlantKind::IceShroom)
                {
                    continue;
                }
                if explosion_hits(
                    kind,
                    backend.plant_x(plant),
                    backend.plant_y(plant),
                    backend.zombie_pos_x(zombie),
                    backend.zombie_pos_y(zombie),
                )? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    })
}

// Preserve C++'s mixed float/double arithmetic, including its slow-1 boundary.
fn displacement(time: i32, speed: f32, slow: i32) -> f32 {
    if slow == 0 {
        speed * time as f32
    } else if slow > time {
        (0.4 * f64::from(speed) * f64::from(time)) as f32
    } else {
        (0.4 * f64::from(speed) * f64::from(slow - 1) + f64::from(speed * (time - (slow - 1)) as f32)) as f32
    }
}

/// Returns the first native-order balloon warning. An already-entered balloon
/// terminates the scan exactly as in A-TAS, rather than warning about a later one.
pub fn balloon_notice() -> RuntimeResult<Option<BalloonNotice>>
where
    CurrentBackend: SeedBankReadBackend + CursorSeedReadBackend + ZombieRawFactsBackend,
{
    if !available(PlantKind::Blover)? {
        return Ok(None);
    }
    with_backend(|backend| {
        for zombie in backend.zombies().map_err(operation_error)? {
            if backend.zombie_kind(zombie).map_err(operation_error)? != ZombieKind::Balloon {
                continue;
            }
            let x = backend.zombie_pos_x(zombie);
            if integer(x)? <= -100 {
                return Ok(None);
            }
            let speed = backend.zombie_speed_x(zombie);
            let slow = backend.zombie_chilled_countdown(zombie);
            if !speed.is_finite() || speed < 0.0 || slow < 0 {
                return Err(RuntimeError::new("invalid native balloon motion"));
            }
            if integer(x - displacement(49, speed, slow))? <= -100 {
                return Ok(Some(BalloonNotice::BiteBlover));
            }
            if integer(x - displacement(50, speed, slow))? <= -100 {
                return Ok(Some(BalloonNotice::Warning));
            }
        }
        Ok(None)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn balloon_slow_boundaries_keep_the_reference_last_slow_tick() {
        assert_eq!(displacement(50, 1.0, 0), 50.0);
        assert_eq!(displacement(50, 1.0, 1), 50.0);
        assert_eq!(displacement(50, 1.0, 50), 20.6);
        assert_eq!(displacement(50, 1.0, 51), 20.0);
        assert_eq!(integer(-99.9).unwrap(), -99);
        assert_eq!(integer(-100.1).unwrap(), -100);
    }
    #[test]
    fn jack_geometry_uses_real_pixels_and_inclusive_edges() {
        assert!(explosion_hits(PlantKind::Peashooter, 100, 100, 140.0, 40.0).unwrap());
        assert!(!explosion_hits(PlantKind::Peashooter, 100, 100, 140.0, -51.0).unwrap());
        assert!(explosion_hits(PlantKind::Pumpkin, 100, 100, 230.0, 40.0).unwrap());
        assert!(!explosion_hits(PlantKind::Peashooter, 100, 100, 230.0, 40.0).unwrap());
        assert!(explosion_hits(PlantKind::CobCannon, 100, 100, 270.0, 40.0).unwrap());
        assert!(!explosion_hits(PlantKind::CobCannon, 100, 100, 271.0, 40.0).unwrap());
    }
}
