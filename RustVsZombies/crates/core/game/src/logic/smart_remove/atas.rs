//! A-TAS 20250124 removal rules. The older native-geometry algorithm remains
//! available separately; choosing this baseline is explicit at the call site.
use super::*;
use rsvz_backend_api::PlantPresenceBackend;

fn alive<'a>(backend: &'a CurrentBackend, plant: <CurrentBackend as PlantReadBackend>::PlantHandle<'a>) -> bool
where
    CurrentBackend: PlantReadBackend,
{
    !backend.plant_is_squished(plant)
        && backend.plant_on_bungee_state(plant) != 2
        && !(crate::live_value::read_or_abort(backend.plant_raw_kind(plant), "plant_raw_kind") == PlantKind::Squash
            && backend.plant_state(plant) == 7)
}

fn range(x: i32, left: i32, right: i32) -> Result<AbsoluteContactRange, SmartRemoveError> {
    x.checked_add(left)
        .zip(x.checked_add(right))
        .map(|(left, right)| AbsoluteContactRange::new(left, right))
        .ok_or_else(|| SmartRemoveError::Backend(RuntimeError::new("smart removal coordinate overflow")))
}

fn plant_range(kind: PlantKind, x: i32) -> Result<AbsoluteContactRange, SmartRemoveError> {
    let (left, right) = match kind {
        PlantKind::TallNut => (30, 70),
        PlantKind::Pumpkin => (20, 80),
        PlantKind::CobCannon => (20, 120),
        _ => (30, 50),
    };
    range(x, left, right)
}

fn bounds<'a>(
    backend: &'a CurrentBackend, plant: <CurrentBackend as PlantReadBackend>::PlantHandle<'a>,
) -> Result<PlantDefenseBounds, SmartRemoveError>
where
    CurrentBackend: PlantReadBackend,
{
    let kind = backend
        .plant_raw_kind(plant)
        .map_err(|e| SmartRemoveError::Backend(crate::access::operation_error(e)))?;
    let x = backend.plant_x(plant);
    Ok(PlantDefenseBounds {
        id: backend.plant_id(plant),
        kind,
        defense_kind: PlantDefenseKind::ChewCrushSmash,
        grid: crate::plant::grid_from_handle(backend, plant),
        x,
        range: plant_range(kind, x)?,
    })
}

// C++ compares native floats to double literals. Do not round the boundary
// constants to f32, or floats adjacent to 0.641/0.643/0.645 change classification.
fn hammer_windows(cr: f32, last: f32, frozen: bool, ice: bool) -> (bool, bool) {
    let (cr, last) = (f64::from(cr), f64::from(last));
    let normal = 0.641 < cr && cr < 0.643;
    let slowed = 0.643 < cr && cr < 0.645 && 0.639 < last && last < 0.641;
    (
        !frozen && !ice && (normal || slowed),
        cr < 0.641 || (normal && !frozen) || slowed,
    )
}

// Missing layers are absent, not additional targets that a hammer must hit.
// A veto also suppresses vehicle removal in the A-TAS source.
fn hammer_decision(
    attacks: &[ZombieAttackBounds], own: PlantDefenseBounds, inner: [Option<PlantDefenseBounds>; 2],
) -> (bool, bool) {
    let mut remove = false;
    for &attack in attacks {
        if !hits(attack, Some(own)) {
            continue;
        }
        if inner
            .iter()
            .all(|&plant| plant.is_none_or(|plant| hits(attack, Some(plant))))
        {
            return (false, true);
        }
        remove = true;
    }
    (remove, false)
}

/// Runs the A-TAS 20250124 algorithm using borrowed objects from one native
/// access scope. Same-frame removals do not free slots; the original layer map
/// stays valid until this scope ends, matching the reference's traversal order.
pub fn tick_atas_smart_remove(state: &mut SmartRemoveState) -> Result<(), SmartRemoveError>
where
    CurrentBackend: PlantPresenceBackend + ZombieRawFactsBackend + PlantRemoveBackend + PlantVisualStateBackend,
{
    crate::access::with_backend(|backend| {
        let mut map: [[_; LAYERS]; ROWS * COLS] = std::array::from_fn(|_| std::array::from_fn(|_| None));
        let mut ice = false;
        for plant in crate::live_value::read_or_abort(backend.present_plants(), "present_plants")
            .filter(|&plant| alive(backend, plant))
        {
            let kind = backend
                .plant_raw_kind(plant)
                .map_err(|e| SmartRemoveError::Backend(crate::access::operation_error(e)))?;
            let grid = crate::plant::grid_from_handle(backend, plant);
            let cell = cell_index(grid).ok_or(SmartRemoveError::InvalidPlantGrid(grid))?;
            let layer = layer_index(classify_plant_layer(kind, backend.plant_state(plant)))
                .ok_or(SmartRemoveError::InvalidPlantGrid(grid))?;
            map[cell][layer] = Some(plant);
            ice |= kind == PlantKind::IceShroom
                && backend.plant_effect_countdown(plant) == 1
                && backend.plant_hp(plant) >= 0;
        }
        state.hammer_vec.clear();
        state.sparkle_vec.clear();
        state.crush_vec.clear();
        for zombie in crate::live_value::read_or_abort(backend.zombies(), "zombies") {
            let kind = backend
                .zombie_kind(zombie)
                .map_err(|e| SmartRemoveError::Backend(crate::access::operation_error(e)))?;
            let phase = backend
                .zombie_phase(zombie)
                .map_err(|e| SmartRemoveError::Backend(crate::access::operation_error(e)))?;
            let giant = matches!(kind, ZombieKind::Gargantuar | ZombieKind::GigaGargantuar);
            if !(giant && phase == ZombiePhase::GargantuarSmashing
                || matches!(kind, ZombieKind::Zomboni | ZombieKind::Catapult) && phase == ZombiePhase::ZombieNormal)
            {
                continue;
            }
            let x = rsvz_model::I32RepresentableF32::new(backend.zombie_pos_x(zombie))
                .map_err(|e| SmartRemoveError::Backend(RuntimeError::new(e.to_string())))?
                .get() as i32;
            let attack = ZombieAttackBounds {
                id: backend.zombie_id(zombie),
                kind,
                row: backend.zombie_row(zombie),
                x,
                range: if giant { range(x, -30, 59)? } else { range(x, 10, 143)? },
            };
            if giant {
                let cr = backend
                    .zombie_reanim_anim_time(zombie)
                    .map_err(|e| SmartRemoveError::Backend(crate::access::operation_error(e)))?
                    .ok_or(SmartRemoveError::MissingZombie(attack.id))?;
                let last = backend
                    .zombie_reanim_last_time(zombie)
                    .map_err(|e| SmartRemoveError::Backend(crate::access::operation_error(e)))?
                    .ok_or(SmartRemoveError::MissingZombie(attack.id))?;
                let (hammer, sparkle) = hammer_windows(cr, last, backend.zombie_frozen_countdown(zombie) != 0, ice);
                if hammer {
                    state.hammer_vec.push(attack);
                }
                if state.is_sparkle && sparkle {
                    state.sparkle_vec.push(attack);
                }
            } else {
                state.crush_vec.push(attack);
            }
        }
        for plant in crate::live_value::read_or_abort(backend.present_plants(), "present_plants")
            .filter(|&plant| alive(backend, plant))
        {
            if state.is_sparkle && backend.plant_eating_flash_counter(plant) == 59 {
                set_flash_and_update_color(backend, plant, 0)?;
            }
            let own = bounds(backend, plant)?;
            let cell = cell_index(own.grid).ok_or(SmartRemoveError::InvalidPlantGrid(own.grid))?;
            let pumpkin = own.kind == PlantKind::Pumpkin;
            if pumpkin {
                if map[cell][0].is_none() && map[cell][3].is_none() {
                    continue;
                }
            } else if map[cell][0].is_none()
                || !(own.kind == PlantKind::TallNut
                    || matches!(own.kind, PlantKind::PuffShroom | PlantKind::SunShroom)
                        && (1..=4).contains(&(own.x % 10)))
            {
                continue;
            }
            let base = map[cell][0].map(|plant| bounds(backend, plant)).transpose()?;
            let common = if pumpkin {
                map[cell][3].map(|plant| bounds(backend, plant)).transpose()?
            } else {
                None
            };
            let inner = [base, common];
            let (remove, veto) = hammer_decision(&state.hammer_vec, own, inner);
            let (sparkle, _) = hammer_decision(&state.sparkle_vec, own, inner);
            if remove || sparkle {
                let mut right_index = None;
                if own.grid.col < 8 {
                    let right = &map[cell + 1];
                    let doom = right[3].is_some_and(|plant| {
                        crate::live_value::read_or_abort(backend.plant_raw_kind(plant), "plant_raw_kind")
                            == PlantKind::DoomShroom
                            && backend.plant_effect_countdown(plant) == 1
                    });
                    if !doom {
                        let target = right[1]
                            .or_else(|| right[3].filter(|&plant| !plant_disappears_immediately(backend, plant)))
                            .or(right[0]);
                        if let Some(target) = target {
                            let target = bounds(backend, target)?;
                            if state
                                .hammer_vec
                                .iter()
                                .filter(|&&attack| hits(attack, Some(target)))
                                .take(2)
                                .count()
                                < 2
                            {
                                right_index = Some(target.id.index());
                            }
                        }
                    }
                }
                let stack_allowed = right_index.is_none_or(|right| {
                    own.id.index() < right
                        && (pumpkin || map[cell][1].is_none_or(|plant| backend.plant_id(plant).index() < right))
                });
                if stack_allowed {
                    if remove {
                        backend
                            .remove_plant(plant)
                            .map_err(|e| SmartRemoveError::Backend(crate::access::operation_error(e)))?;
                        continue;
                    }
                    set_flash_and_update_color(backend, plant, 60)?;
                }
            }
            if !veto
                && state.crush_vec.iter().any(|&attack| {
                    hits(attack, Some(own)) && inner.iter().flatten().any(|&plant| !hits(attack, Some(plant)))
                })
            {
                backend
                    .remove_plant(plant)
                    .map_err(|e| SmartRemoveError::Backend(crate::access::operation_error(e)))?;
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_float_edges_are_compared_to_the_reference_double_literals() {
        for value in [0.639_f32, 0.641, 0.643, 0.645] {
            for cr in [value.next_down(), value, value.next_up()] {
                for last in [0.639_f32, 0.64, 0.641] {
                    for frozen in [false, true] {
                        for ice in [false, true] {
                            let c = f64::from(cr);
                            let l = f64::from(last);
                            let reference_hammer = !frozen
                                && !ice
                                && ((0.641 < c && c < 0.643) || (0.643 < c && c < 0.645 && 0.639 < l && l < 0.641));
                            let reference_sparkle = c < 0.641
                                || (0.641 < c && c < 0.643 && !frozen)
                                || (0.643 < c && c < 0.645 && 0.639 < l && l < 0.641);
                            assert_eq!(
                                hammer_windows(cr, last, frozen, ice),
                                (reference_hammer, reference_sparkle)
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(hammer_windows(0.642, 0.63, false, false), (true, true));
        assert_eq!(hammer_windows(0.642, 0.63, false, true), (false, true));
    }

    #[test]
    fn reference_ranges_preserve_endpoints_and_reject_overflow() {
        for (kind, left, right) in [
            (PlantKind::Peashooter, 30, 50),
            (PlantKind::TallNut, 30, 70),
            (PlantKind::Pumpkin, 20, 80),
            (PlantKind::CobCannon, 20, 120),
        ] {
            assert_eq!(
                plant_range(kind, 100).unwrap(),
                AbsoluteContactRange::new(100 + left, 100 + right)
            );
        }
        assert!(plant_range(PlantKind::Pumpkin, i32::MAX).is_err());
    }
}
