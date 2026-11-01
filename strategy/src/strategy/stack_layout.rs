//! Initial layout only: keep ladder Pumpkins behind temporary plants in slot order.
use super::*;
use rsvz::core::backend::*;
use rsvz::core::model::{GridItemKind, PlantId, PositiveHp};

pub(super) fn prepare() -> RuntimeResult<bool> {
    rsvz::with_backend(|b| {
        // Mid-fight injection is not initial lineup construction.
        if b.zombies().map_err(err)?.any(|z| b.zombie_is_alive(z)) {
            return Ok(false);
        }
        let mut shields = Vec::new();
        for p in b.plants().map_err(err)? {
            if b.plant_is_alive(p)
                && b.plant_raw_kind(p).map_err(err)? == Pumpkin
                && matches!((b.plant_row(p) + 1, b.plant_col(p) + 1), (2, 2) | (5, 2))
            {
                shields.push((b.plant_id(p), (b.plant_row(p) + 1, b.plant_col(p) + 1), b.plant_hp(p)));
            }
        }
        if shields.len() != 2 || shields.iter().all(|(id, _, _)| id.index() >= 96) {
            return Ok(false);
        }
        let ladders: Vec<_> = b
            .grid_items()
            .map_err(err)?
            .filter(|g| b.grid_item_kind(*g).ok() == Some(GridItemKind::Ladder))
            .map(|g| (b.grid_item_row(g) + 1, b.grid_item_col(g) + 1))
            .collect();
        for &(id, _, _) in &shields {
            if let Some(p) = b.plant(id).map_err(err)? {
                b.remove_plant(p).map_err(err)?;
            }
        }
        let mut placeholders: Vec<PlantId> = Vec::new();
        while b.plant_pool_next_slot().map_err(err)? < 96 {
            let p = b
                .add_plant(sel(PuffShroom).checked().unwrap(), grid((1, 9)))
                .map_err(err)?;
            placeholders.push(b.plant_id(p));
        }
        for &(_, pos, health) in &shields {
            let p = b.add_plant(sel(Pumpkin).checked().unwrap(), grid(pos)).map_err(err)?;
            b.set_plant_hp(p, PositiveHp::new(health).map_err(err)?).map_err(err)?;
        }
        for id in placeholders {
            if let Some(p) = b.plant(id).map_err(err)? {
                b.remove_plant(p).map_err(err)?;
            }
        }
        for &(_, pos, _) in &shields {
            if ladders.contains(&pos)
                && !b.grid_items().map_err(err)?.any(|g| {
                    b.grid_item_kind(g).ok() == Some(GridItemKind::Ladder)
                        && b.grid_item_row(g) + 1 == pos.0
                        && b.grid_item_col(g) + 1 == pos.1
                })
            {
                b.add_ladder(grid(pos)).map_err(err)?;
            }
        }
        Ok(true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
    #[test]
    fn temporary_pad_is_selected_before_high_slot_ladder_pumpkin() {
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        world.with_backend(|b| {
            b.set_zombie_spawn_stopped(true).unwrap();
            b.finish_card_selection().unwrap();
            for pos in [(2, 2), (5, 2)] {
                b.add_plant(sel(GloomShroom).checked().unwrap(), grid(pos)).unwrap();
                b.add_plant(sel(Pumpkin).checked().unwrap(), grid(pos)).unwrap();
                b.add_ladder(grid(pos)).unwrap();
            }
            PeBackend::scope_backend(b, || prepare().unwrap());
            assert_eq!(b.plants().unwrap().filter(|p| b.plant_is_alive(*p)).count(), 4);
            assert_eq!(
                b.grid_items()
                    .unwrap()
                    .filter(|g| b.grid_item_kind(*g).ok() == Some(GridItemKind::Ladder))
                    .count(),
                2
            );
            assert!(
                b.plants()
                    .unwrap()
                    .filter(|p| b.plant_raw_kind(*p).ok() == Some(Pumpkin))
                    .all(|p| b.plant_id(p).index() >= 96)
            );
        });
        // Native cleanup releases the deliberately vacated low slots.
        world.update_world().unwrap();
        world.with_backend(|b| {
            let p = b.add_plant(sel(PuffShroom).checked().unwrap(), grid((2, 3))).unwrap();
            assert!(b.plant_id(p).index() < 96);
            let z = b.place_zombie(Z::GigaGargantuar, grid((2, 3))).unwrap();
            b.set_zombie_x(z, rsvz::core::model::I32RepresentableF32::new(230.).unwrap())
                .unwrap();
        });
        for _ in 0..150 {
            world.update_world().unwrap();
        }
        world.with_backend(|b| {
            assert!(
                b.plants()
                    .unwrap()
                    .filter(|p| b.plant_raw_kind(*p).ok() == Some(Pumpkin))
                    .all(|p| b.plant_hp(p) == 4000)
            );
            assert!(!b.plants().unwrap().any(|p| b.plant_is_alive(p)
                && !b.plant_is_squished(p)
                && b.plant_raw_kind(p).ok() == Some(PuffShroom)));
        });
    }
}
