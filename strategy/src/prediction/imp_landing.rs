use super::*;

pub(crate) fn observed_landing(z: Zombie) -> Option<(Zombie, i32)> {
    use rsvz::core::backend::*;
    rsvz::with_backend(|b| {
        let actor = b.zombie(z.id).ok().flatten()?;
        let mut x = z.x;
        let mut delay = 0;
        if z.phase == ZombiePhase::ImpGettingThrown {
            let mut height = b.zombie_altitude(actor);
            let mut velocity = b.zombie_speed_z(actor);
            while height > 0. && delay < 200 {
                velocity -= 0.05;
                height += velocity;
                x -= b.zombie_speed_x(actor);
                delay += 1;
            }
            delay += 25;
        } else {
            let frames = b.zombie_reanim_frame_count(actor).ok().flatten()?;
            // Return the unchilled remainder; simulate_imp_arrival applies
            // observed and scheduled chill to the landing animation once.
            delay = ((1. - z.progress) * frames as f32 / (24. * 0.01)).ceil() as i32 + 1;
        }
        let y = b.zombie_int_y(actor);
        Some((
            Zombie {
                x,
                rect: rsvz::core::model::ContactRect::new(x as i32 + 36, y, x as i32 + 78, y + 115),
                phase: ZombiePhase::ZombieNormal,
                progress: 0.,
                speed: prediction::max_speed(Z::Imp),
                frozen: 0,
                ..z
            },
            delay,
        ))
    })
}
