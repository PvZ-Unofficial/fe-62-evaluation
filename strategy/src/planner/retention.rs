//! W10/W11: distinguish an actual clearance from a truncated/conditional horizon.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Need {
    Clear,
    Intervention,
    Conditional,
}

pub(super) fn classify(out: Outcome, f: &Forecast) -> Need {
    if out.danger < out.death {
        return Need::Intervention;
    }
    if out.death == NEVER {
        return Need::Conditional;
    }
    // Credit the next planned nuke and the ice leading up to it. Do not call
    // survival dependent on a later, uncommitted ash cycle established safety.
    let next_nuke = f
        .events
        .iter()
        .find(|e| e.kind == prediction::EffectKind::Doom)
        .map(|e| e.at);
    if f.events
        .iter()
        .any(|e| !e.certain && e.at <= out.death && !next_nuke.is_some_and(|at| e.at <= at))
    {
        Need::Conditional
    } else {
        Need::Clear
    }
}

pub(super) fn protected_hits(out: Outcome, f: &Forecast) -> u32 {
    let next_nuke = f.events.iter().position(|e| e.kind == prediction::EffectKind::Doom);
    f.events.iter().enumerate().fold(0, |mask, (i, e)| {
        if e.certain || Some(i) == next_nuke {
            mask | (out.ash_mask & (1 << i))
        } else {
            mask
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_unfinished_horizon_is_not_a_clearance() {
        assert_eq!(classify(Outcome::default(), &Forecast::default()), Need::Conditional);
        let out = Outcome {
            death: 400,
            ..Outcome::default()
        };
        assert_eq!(classify(out, &Forecast::default()), Need::Clear);
    }
    #[test]
    fn later_uncommitted_cycles_do_not_certify_safety() {
        let mut f = Forecast::default();
        // Geometry is irrelevant to this classification test.
        for (at, kind) in [
            (500, prediction::EffectKind::Doom),
            (1500, prediction::EffectKind::Cherry),
        ] {
            f.events.push(prediction::Effect {
                at,
                kind,
                pos: (2, 4),
                certain: false,
                body_at: None,
                landing: None,
                shape: None,
            });
        }
        assert_eq!(
            classify(
                Outcome {
                    death: 700,
                    ..Outcome::default()
                },
                &f
            ),
            Need::Clear
        );
        assert_eq!(
            classify(
                Outcome {
                    death: 1600,
                    ..Outcome::default()
                },
                &f
            ),
            Need::Conditional
        );
        assert_eq!(
            classify(
                Outcome {
                    death: 1600,
                    danger: 1200,
                    ..Outcome::default()
                },
                &f
            ),
            Need::Intervention
        );
    }
}
