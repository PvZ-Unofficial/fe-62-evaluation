//! Spare-Puff traffic separation; never changes the ash plan or promises a kill.
use super::*;

// One native firing/reload interval. Do not credit repeated shots or the
// lifetime of the bait: it may be eaten before the next target check.
const STOP: i32 = 299;

fn walking_x(car: Zombie, at: i32, stop: i32) -> f32 {
    let start = car.frozen.max(stop);
    let moving = (at - start).max(0);
    let slow = moving.min((car.chilled - start).max(0));
    car.x - car.speed * (moving as f32 - slow as f32 * 0.5)
}

impl Planner {
    pub fn bait_catapult(&mut self, f: &Forecast) {
        if !self.spare_puff() || self.scene.now < self.next_catapult_bait {
            return;
        }
        self.next_catapult_bait = self.scene.now + 25;
        if !self.scene.row_zombies(1).any(|z| self.scene.catapult_can_be_baited(*z)) {
            return;
        }
        let ready_again = 751 + setting("SIX_DECISION_PERIOD", 20).max(1) as i32;
        for i in 0..self.scene.zombies.len() {
            let foot = self.scene.zombies[i];
            if foot.row != 1 || foot.kind != Z::Football || foot.x < 250. || self.assessment(foot, f).safe() {
                continue;
            }
            // Protect this Football's own next interception as well as all
            // other rows. Passing a car ID excludes no edible-threat actor.
            let Some(car) = self
                .scene
                .row_zombies(1)
                .copied()
                .find(|z| self.scene.catapult_can_be_baited(*z))
            else {
                return;
            };
            if self.reserve_real_pad_window(f, car.id) || self.puff_reserved_for_blover(foot) {
                return;
            }
            for col in [2, 1] {
                let pos = (1, col);
                if self.scene.main(pos).is_some() || !legal(sel(PuffShroom), pos) || !combat::jack_safe(pos, 100) {
                    continue;
                }
                let mut pad = Pad::new(pos, PuffShroom);
                pad.order = self.scene.next_plant_order;
                // Worst Puff offset for acquiring the car as a shooter.
                let target_x = col * 80 - 40 + 4;
                let edge = pad.rect().right - 20 - prediction::attack_offsets(foot).0;
                let contact =
                    prediction::first_left_of(foot, &self.scene, f, edge + foot.rect.right - foot.x as i32, 2500);
                if !(ready_again..2500).contains(&contact) {
                    continue;
                }
                // This cheap travel test covers only a single unchanged cold
                // interval. Reobserve after planned ice instead of guessing.
                if f.events
                    .iter()
                    .any(|e| e.kind == prediction::EffectKind::Ice && e.at <= contact)
                {
                    continue;
                }
                let crush_edge = (pad.rect().right - 30 + 4) as f32;
                let blocking_car = self.scene.row_zombies(1).copied().find(|z| {
                    self.scene.catapult_can_be_baited(*z)
                        && z.x >= (target_x + 102) as f32
                        && walking_x(*z, contact, 0) <= crush_edge
                        && walking_x(*z, contact, STOP) > crush_edge
                        && run_until(*z, &self.scene, f, None, contact, 2).death > contact
                });
                let Some(car) = blocking_car else { continue };
                // Keep the bait intact through the next decision tick. Shared
                // chewing, hammer, vehicle and basketball checks remain active.
                // No benefit from the prospective stop is credited here.
                if !prediction::prestore_clear(&self.scene, pos, 25, f)
                    || prediction::external_destruction_bound(&self.scene, foot, pad) <= 25
                {
                    continue;
                }
                let after = run_until(foot, &self.scene, f, Some(pad), 25, 1);
                if after.impact <= 25 || after.danger <= 25 {
                    continue;
                }
                if play_puff(pos, 24) {
                    telemetry::record(
                        "catapult_bait",
                        &[
                            car.id.raw() as i64,
                            foot.id.raw() as i64,
                            col as i64,
                            contact as i64,
                            (car.x * 100.) as i64,
                        ],
                    );
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catapult_bait_delay_does_not_extend_chill() {
        let car = Zombie {
            age: 0,
            id: ZombieId::from_raw(65537),
            kind: Z::Catapult,
            row: 1,
            wave: 1,
            carried: false,
            x: 400.,
            hp: 850,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.3,
            frozen: 0,
            chilled: 200,
            object: false,
            eating: false,
            rect: rsvz::core::model::ContactRect::new(400, 0, 553, 140),
        };
        assert!((walking_x(car, 1000, 0) - 130.).abs() < 0.01);
        assert!((walking_x(car, 1000, 300) - 190.).abs() < 0.01);
    }
}
