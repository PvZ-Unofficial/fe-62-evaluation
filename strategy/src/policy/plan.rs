//! Read-only projection of the existing Control/Forecast plan, never a legality mask.
use super::*;
pub const WIDTH: usize = 32;
pub type Plan = [f32; WIDTH];
pub fn forecast(mut x: Plan, f: &Forecast) -> Plan {
    use prediction::EffectKind as E;
    for (i, kind) in [E::Ice, E::Doom, E::Cherry, E::Jalapeno].into_iter().enumerate() {
        let first = f
            .events
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == kind)
            .min_by_key(|(index, e)| (e.at, *index))
            .map(|(_, e)| e);
        if let Some(e) = first {
            let o = 8 + i * 5;
            x[o] = 1.;
            x[o + 1] = e.at as f32 / 6000.;
            x[o + 2] = e.pos.0 as f32 / 6.;
            x[o + 3] = e.pos.1 as f32 / 9.;
            x[o + 4] = bit(e.certain);
        }
    }
    x
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_events_preserve_conditional_flag_and_absence() {
        use prediction::EffectKind as E;
        let f = Forecast {
            events: vec![
                prediction::Effect {
                    at: 500,
                    kind: E::Ice,
                    pos: (5, 1),
                    certain: false,
                    body_at: None,
                    landing: None,
                    shape: None,
                },
                prediction::Effect {
                    at: 100,
                    kind: E::Ice,
                    pos: (2, 3),
                    certain: true,
                    body_at: None,
                    landing: None,
                    shape: None,
                },
                prediction::Effect {
                    at: 300,
                    kind: E::Jalapeno,
                    pos: (6, 2),
                    certain: false,
                    body_at: None,
                    landing: None,
                    shape: None,
                },
            ],
        };
        let x = forecast([0.; WIDTH], &f);
        assert_eq!(x[8], 1.);
        assert_eq!(x[9], 100. / 6000.);
        assert_eq!(x[12], 1.);
        assert!(x[13..23].iter().all(|v| *v == 0.));
        assert_eq!(x[23], 1.);
        assert_eq!(x[27], 0.);
    }
}
