//! Bounded preferences over the existing legal Puff candidates. No physics changes.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Tuning {
    pub pad_mode: bool,
    pub ash_mode: bool,
    pub horizon: i32,
    band: i32,
    clear: f64,
    delay: f64,
    ash: f64,
    conditional: f64,
    trigger: f64,
    order: usize,
    bank_order: usize,
}

fn number(name: &str, default: f64, low: f64, high: f64, integer: bool) -> f64 {
    let key = format!("SIX_TUNE_{name}");
    let value = match std::env::var(&key) {
        Ok(value) => value.parse::<f64>().unwrap_or_else(|_| panic!("invalid {key}")),
        Err(std::env::VarError::NotPresent) => default,
        Err(_) => panic!("invalid {key}"),
    };
    assert!(
        value.is_finite() && (low..=high).contains(&value) && (!integer || value.fract() == 0.),
        "invalid {key}"
    );
    value
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            pad_mode: number("PAD_MODE", 0., 0., 1., true) != 0.,
            ash_mode: number("ASH_MODE", 0., 0., 1., true) != 0.,
            horizon: number("HORIZON", 2500., 1500., 3000., true) as i32,
            band: number("BAND", 100., 0., 200., true) as i32,
            clear: number("CLEAR", 2., 0., 4., false),
            delay: number("DELAY", 1., 0., 4., false),
            ash: number("ASH", 1., 0., 4., false),
            conditional: number("CONDITIONAL", 0.5, 0., 2., false),
            trigger: number("TRIGGER", 0.25, 0., 2., false),
            order: number("ORDER", 0., 0., 2., true) as usize,
            bank_order: number("BANK_ORDER", 0., 0., 1., true) as usize,
        }
    }
}

impl Tuning {
    pub fn adjusted(mut self, delay: f64, ash: f64, conditional: f64, trigger: f64) -> Self {
        self.delay = (self.delay + delay).clamp(0., 4.);
        self.ash = (self.ash + ash).clamp(0., 4.);
        self.conditional = (self.conditional + conditional).clamp(0., 2.);
        self.trigger = (self.trigger + trigger).clamp(0., 2.);
        self
    }

    pub fn columns(self) -> &'static [i32; 3] {
        &[[5, 4, 3], [3, 4, 5], [4, 5, 3]][self.order]
    }

    pub fn bank_rows(self) -> &'static [i32; 2] {
        &[[5, 2], [2, 5]][self.bank_order]
    }

    fn urgency(self, base: Outcome) -> (bool, i32) {
        // An immediate request never loses to an optional, more distant request.
        (base.danger > 751, base.danger / self.band.max(1))
    }

    pub fn score(self, base: Outcome, after: Outcome, f: &Forecast) -> f64 {
        let cleared = after.death < NEVER && after.death <= after.danger;
        let new_clear = cleared && (base.death == NEVER || base.danger < base.death);
        let delay = if cleared {
            0.
        } else {
            ((after.danger.min(prediction::HORIZON) - base.danger.min(prediction::HORIZON)).max(0) as f64 / 751.)
                .min(2.)
        };
        // Count only still-unresolved targets; do not reward the same clearance twice.
        let hits = if cleared {
            0.
        } else {
            (after.ash_hits - base.ash_hits).max(0) as f64
        };
        let benefit = self.clear * f64::from(new_clear) + self.delay * delay + self.ash * hits;
        // This is a conservative potential-dependency flag, not a success probability.
        let conditional = f
            .events
            .iter()
            .any(|e| !e.certain && e.at <= after.death.min(prediction::HORIZON));
        benefit / (1. + self.conditional * f64::from(conditional))
            - self.trigger * after.trigger.min(prediction::HORIZON) as f64 / 2500.
    }

    pub fn forecast_loss(self, base: Outcome, after: Outcome) -> f64 {
        // Baseline and post-intervention speed assumptions need not match.
        // Do not turn that biased deadline difference into an indirect veto.
        self.ash * (base.ash_hits - after.ash_hits).max(0) as f64
    }

    pub fn prefer_pad(
        self, base: Outcome, after: Outcome, old_base: Outcome, old_after: Outcome, f: &Forecast,
    ) -> bool {
        let key = self.urgency(base);
        let old_key = self.urgency(old_base);
        key < old_key || key == old_key && self.score(base, after, f) > self.score(old_base, old_after, f)
    }

    pub fn prefer_ash(
        self, base: Outcome, after: Outcome, event: usize, old_base: Outcome, old_after: Outcome, old: usize,
        f: &Forecast,
    ) -> bool {
        let score = self.score(base, after, f);
        let old_score = self.score(old_base, old_after, f);
        score > old_score || score == old_score && f.events[event].at < f.events[old].at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Tuning {
        Tuning {
            pad_mode: true,
            ash_mode: true,
            horizon: 2500,
            band: 200,
            clear: 2.,
            delay: 1.,
            ash: 1.,
            conditional: 0.5,
            trigger: 0.25,
            order: 0,
            bank_order: 0,
        }
    }

    #[test]
    fn urgent_window_precedes_even_a_large_distant_reward() {
        let t = config();
        let urgent = Outcome {
            danger: 750,
            ..Outcome::default()
        };
        let distant = Outcome {
            danger: 760,
            ..Outcome::default()
        };
        let small = Outcome {
            danger: 800,
            trigger: 1,
            ..Outcome::default()
        };
        let clear = Outcome {
            death: 1000,
            trigger: 1,
            ..Outcome::default()
        };
        assert!(t.prefer_pad(urgent, small, distant, clear, &Forecast::default()));
        assert!(!t.prefer_pad(distant, clear, urgent, small, &Forecast::default()));
    }

    #[test]
    fn finite_clearance_beats_a_censored_safe_outcome_in_the_same_band() {
        let t = config();
        let base = Outcome {
            danger: 1000,
            ..Outcome::default()
        };
        let censored = Outcome {
            trigger: 1,
            ..Outcome::default()
        };
        let cleared = Outcome {
            death: 1500,
            trigger: 1,
            ..Outcome::default()
        };
        let mut t = t;
        t.clear = 4.;
        assert!(t.prefer_pad(base, cleared, base, censored, &Forecast::default()));
        assert!(t.score(base, censored, &Forecast::default()).is_finite());
    }

    #[test]
    fn zero_band_preserves_strict_deadline_order_and_plans_discount_only_benefit() {
        let mut t = config();
        t.band = 0;
        let base = Outcome {
            danger: 1000,
            ..Outcome::default()
        };
        let later = Outcome {
            danger: 1001,
            ..Outcome::default()
        };
        let after = Outcome {
            death: 1400,
            trigger: 1,
            ..Outcome::default()
        };
        let mut f = Forecast::default();
        assert!(t.prefer_pad(base, after, later, after, &f));
        let score = t.score(base, after, &f);
        f.add(500, prediction::EffectKind::Ice, (5, 1), false);
        assert!(t.score(base, after, &f) < score);
        assert_eq!(t.columns(), &[5, 4, 3]);
        assert_eq!(t.bank_rows(), &[5, 2]);
    }
}
