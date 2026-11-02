//! Optional local entity relations. Only already sampled, permitted visible facts.
use super::*;

pub(super) const FEATURES: usize = 288;
const WIDTH: usize = 24;

fn normal(phase: ZombiePhase) -> bool {
    matches!(phase, ZombiePhase::ZombieNormal | ZombiePhase::JackInTheBoxRunning)
}
fn target(z: Zombie, pos: Pos, wave: i32) -> [f32; WIDTH] {
    let smashing = z.phase == ZombiePhase::GargantuarSmashing;
    let throwing = matches!(
        z.phase,
        ZombiePhase::GargantuarThrowing | ZombiePhase::ImpGettingThrown | ZombiePhase::ImpLanding
    );
    let known_age = !z.carried && (1..=wave).contains(&z.wave);
    [
        1.,
        (z.x - (pos.1 * 80 - 40) as f32) / 1000.,
        (z.row - pos.0) as f32 / 2.,
        z.hp as f32 / 6000.,
        z.armor as f32 / 6000.,
        if z.frozen > 0 { 0. } else { z.speed },
        z.frozen as f32 / 400.,
        z.chilled as f32 / 2000.,
        z.progress,
        bit(z.eating),
        bit(z.carried),
        bit(!z.carried && z.wave == wave && wave > 0),
        if known_age { (wave - z.wave) as f32 / 20. } else { -1. },
        bit(z.object),
        z.kind as i32 as f32 / 32.,
        z.phase as i32 as f32 / 100.,
        bit(z.giant()),
        bit(z.kind == Z::Football),
        bit(z.kind == Z::Imp),
        bit(matches!(z.kind, Z::Zomboni | Z::Catapult)),
        bit(normal(z.phase)),
        bit(smashing),
        bit(throwing),
        bit(!normal(z.phase) && !smashing && !throwing),
    ]
}
fn order(a: &[f32; WIDTH], b: &[f32; WIDTH]) -> std::cmp::Ordering {
    // Prefer this lane, then nearest horizontal distance; remaining ties use only
    // feature values. IDs and enumeration/slot order never enter the comparison.
    a[2].abs()
        .total_cmp(&b[2].abs())
        .then(a[1].abs().total_cmp(&b[1].abs()))
        .then_with(|| {
            a.iter()
                .zip(b)
                .map(|(x, y)| x.total_cmp(y))
                .find(|o| !o.is_eq())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}
pub(super) fn expand(base: &[f32; super::FEATURES], action: Action, scene: &Scene, c: &Context) -> [f32; FEATURES] {
    let mut x = [0.; FEATURES];
    x[..super::FEATURES].copy_from_slice(base);
    if (1..=20).contains(&c.wave) {
        x[192 + c.wave as usize - 1] = 1.;
    }
    if action.pos.0 == 0 {
        return x;
    }
    let mut closest = [[0.; WIDTH]; 3];
    let mut count = 0;
    for &z in &scene.zombies {
        if (z.row - action.pos.0).abs() > 1 {
            continue;
        }
        x[284] += 1. / 20.;
        if z.row == action.pos.0 {
            x[285] += bit(!normal(z.phase)) / 20.;
            x[287] += z.armor as f32 / 30000.;
        }
        if z.carried || (z.wave > 0 && z.wave < c.wave) {
            x[286] += (z.hp + z.armor) as f32 / 30000.;
        }
        let value = target(z, action.pos, c.wave);
        let insert = (0..count)
            .find(|&i| order(&value, &closest[i]).is_lt())
            .unwrap_or(count);
        if insert < 3 {
            for i in ((insert + 1)..3).rev() {
                closest[i] = closest[i - 1];
            }
            closest[insert] = value;
            count = (count + 1).min(3);
        }
    }
    for (i, values) in closest.iter().enumerate() {
        x[212 + i * WIDTH..212 + (i + 1) * WIDTH].copy_from_slice(values);
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz::core::model::{ContactRect, ZombieId};
    fn zombie(x: f32, kind: Z) -> Zombie {
        Zombie {
            age: 0,
            id: ZombieId::from_raw(65537),
            kind,
            row: 2,
            wave: 3,
            carried: false,
            x,
            hp: 6000,
            armor: 200,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.2,
            speed: 0.37,
            frozen: 0,
            chilled: 0,
            object: true,
            eating: false,
            rect: ContactRect::new(x as i32, 0, x as i32 + 80, 100),
        }
    }
    fn context() -> Context {
        Context {
            plan: None,
            wave: 4,
            time: 100,
            ice_due: 830,
            doom_due: 2301,
            ice_allowed: [false; 2],
            ice_required: false,
            cherry: true,
            jala: true,
            cherry_done: false,
            jala_done: false,
            pending_ice: -1,
            last_ice: 0,
            last_blover_wave: 2,
            round: 0,
        }
    }
    #[test]
    fn local_fields_are_visible_sorted_and_old_prefix_is_unchanged() {
        let mut scene = Scene::default();
        scene.zombies = vec![
            zombie(600., Z::GigaGargantuar),
            zombie(300., Z::Football),
            zombie(450., Z::Gargantuar),
            zombie(800., Z::Imp),
        ];
        let base = [0.125; super::super::FEATURES];
        let a = expand(&base, Action { kind: 1, pos: (2, 3) }, &scene, &context());
        scene.zombies.reverse();
        for (i, z) in scene.zombies.iter_mut().enumerate() {
            z.id = ZombieId::from_raw(90000 + i as u32);
        }
        let b = expand(&base, Action { kind: 1, pos: (2, 3) }, &scene, &context());
        assert_eq!(a, b);
        assert_eq!(&a[..192], &base);
        assert_eq!(a[195], 1.);
        assert_eq!(a[212 + 14], Z::Football as i32 as f32 / 32.);
        assert_eq!(a[236 + 14], Z::Gargantuar as i32 as f32 / 32.);
        assert_eq!(a[260 + 14], Z::GigaGargantuar as i32 as f32 / 32.);
        assert_eq!(a[212 + 12], 0.05);
    }
    #[test]
    fn local_frozen_speed_and_unknown_cross_round_age_do_not_leak() {
        let mut z = zombie(300., Z::GigaGargantuar);
        z.frozen = 200;
        z.carried = true;
        let a = target(z, (2, 3), 4);
        z.speed = 999.;
        z.id = ZombieId::from_raw(1);
        let b = target(z, (2, 3), 4);
        assert_eq!(a, b);
        assert_eq!(a[5], 0.);
        assert_eq!(a[6], 0.5);
        assert_eq!(a[12], -1.);
    }
}
