use rsvz::prelude::*;
use rsvz::tick::{TickControl, TickOptions};

fn main() {
    let _tick: fn() = rsvz::plant_fixer::tick;
    with_frame(|frame| {
        for item in frame.items() {
            let _ = (item.id(), item.kind(), item.x(), item.y(), item.being_collected());
            item.collect(false);
        }
        for seed in frame.seeds() {
            let _ = (
                seed.slot(),
                seed.selection(),
                seed.cooldown_remaining(),
                seed.can_pick_up(),
            );
        }
        for item in frame.grid_items() {
            let _ = (item.id(), item.kind(), item.grid(), item.countdown(), item.is_alive());
            item.remove();
        }
        for p in frame.projectiles() {
            let _ = (
                p.id(),
                p.kind_code(),
                p.motion_code(),
                p.row(),
                p.x(),
                p.y(),
                p.z(),
                p.velocity_x(),
                p.velocity_y(),
                p.velocity_z(),
                p.acceleration(),
                p.collision(),
                p.damage_flags(),
                p.age(),
            );
        }
        for p in frame.plants() {
            let _ = (
                p.x(),
                p.y(),
                p.max_hp(),
                p.wake_up_counter(),
                p.state_countdown(),
                p.effect_countdown(),
                p.disappear_countdown(),
                p.recently_eaten_countdown(),
                p.on_bungee_state(),
                p.animation_progress(),
                p.hit_box(),
            );
        }

        for zombie in frame.zombies() {
            let _ = (zombie.from_wave(), zombie.phase_counter(), zombie.y(), zombie.speed_x());
            let _ = (zombie.is_blown_away(), zombie.chilled_countdown(), zombie.is_frozen());
            let _ = (
                zombie.accessory_1_hp(),
                zombie.accessory_2_hp(),
                zombie.max_hp(),
                zombie.accessory_1_max_hp(),
                zombie.accessory_2_max_hp(),
            );
            let _ = (
                zombie.altitude(),
                zombie.is_eating(),
                zombie.is_mind_controlled(),
                zombie.has_head(),
                zombie.has_object(),
                zombie.has_flat_tires(),
                zombie.is_in_pool(),
                zombie.is_on_high_ground(),
                zombie.buttered_countdown(),
                zombie.is_dead_or_dying(),
                zombie.collision(),
            );
        }
        frame.for_each_plant_at_anchor_grid(Grid { row: 0, col: 0 }, |plant| {
            let _ = plant.is_squished();
        });
    });
    at_frame(1, 401, |frame| {
        for zombie in frame.zombies() {
            let _ = (zombie.id(), zombie.kind(), zombie.row(), zombie.age());
            let _ = (zombie.motion_state(), zombie.stable_x_at(8), zombie.stable_x_trace(8));
            if zombie.hp() < 100 {
                zombie.remove();
                fire(2, 9);
                let _ = zombie.is_alive();
            }
        }
        frame.plants().map(|plant| plant.id()).collect::<Vec<_>>()
    });
    let _ = try_at_frame(1, 402, |frame| frame.plants().next().map(|plant| plant.hp()));
    tick::on_frame(|frame| {
        for plant in frame.plants() {
            let _ = (plant.kind(), plant.grid(), plant.is_alive());
            if plant.hp() < 10 {
                plant.remove();
            }
        }
    });
    let _ = tick::spawn_frame(TickOptions::active_phase(), |_| -> RuntimeResult<TickControl> {
        Ok(TickControl::Pause)
    });
}
