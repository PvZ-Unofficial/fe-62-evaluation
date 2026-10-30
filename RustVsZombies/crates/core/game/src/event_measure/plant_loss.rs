use rsvz_model::{EffectOutcomeFact, PlantEffectOutcome, PlantEffectSource, Wave, WaveClockState};

use super::PlantLossSampleReport;

/// One witness per failure reason; never stores native pointers or grows on the hot path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PlantLossSample {
    pub trial_sequence: u64,
    pub seed: u32,
    pub world_epoch: u64,
    pub main_counter: i32,
    pub wave_time: Option<(i32, i32)>,
    pub fact: PlantLossFact,
}

/// Reporting-only values: no later entity lookup is performed from this witness.
/// Keep native IDs compact beside the sample's already stored world/trial, rather
/// than retaining a larger live lookup key and unused event token/decision data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PlantLossFact {
    plant_id: u32,
    actor_id: u32,
    source: &'static str,
    outcome: &'static str,
    raw_kind: rsvz_model::PlantKind,
    effective_kind: rsvz_model::PlantKind,
    grid: rsvz_model::Grid,
}
impl From<EffectOutcomeFact> for PlantLossFact {
    fn from(fact: EffectOutcomeFact) -> Self {
        let key = fact.key;
        let actor_id = match key.source {
            PlantEffectSource::Jack(id)
            | PlantEffectSource::Bite(id)
            | PlantEffectSource::Gargantuar(id)
            | PlantEffectSource::ZomboniCrush(id)
            | PlantEffectSource::CatapultCrush(id)
            | PlantEffectSource::Bungee(id) => id.raw(),
            PlantEffectSource::Basketball(id) => id.raw(),
        };
        Self {
            plant_id: key.plant_id.raw(),
            actor_id,
            source: key.source.as_str(),
            outcome: match fact.outcome {
                PlantEffectOutcome::Killed => "killed",
                PlantEffectOutcome::Squished => "squished",
                PlantEffectOutcome::Stolen => "stolen",
                _ => unreachable!("only destructive outcomes produce loss samples"),
            },
            raw_kind: key.raw_kind,
            effective_kind: key.effective_kind,
            grid: key.grid,
        }
    }
}

impl PlantLossSample {
    pub fn resolve_time(&mut self, clocks: &WaveClockState) {
        self.wave_time = (0..=64)
            .filter_map(|wave| clocks.refresh_clock(Wave(wave)).map(|clock| (wave, clock)))
            .filter(|(_, clock)| *clock <= self.main_counter)
            .max_by_key(|(_, clock)| *clock)
            .map(|(wave, clock)| (wave, self.main_counter.saturating_sub(clock)));
    }

    pub fn merge(target: &mut Option<Self>, other: Option<Self>) {
        if let Some(other) = other
            && target
                .is_none_or(|old| (other.trial_sequence, other.main_counter) < (old.trial_sequence, old.main_counter))
        {
            *target = Some(other);
        }
    }

    pub fn report(self) -> PlantLossSampleReport {
        let key = self.fact;
        PlantLossSampleReport {
            trial_sequence: self.trial_sequence,
            seed: self.seed,
            world_epoch: self.world_epoch,
            main_counter: self.main_counter,
            wave: self.wave_time.map(|time| time.0),
            time: self.wave_time.map(|time| time.1),
            plant_id: key.plant_id,
            raw_kind: key.raw_kind.report_name().to_owned(),
            effective_kind: key.effective_kind.report_name().to_owned(),
            row: key.grid.row + 1,
            col: key.grid.col + 1,
            source: key.source.into(),
            actor_id: key.actor_id,
            outcome: key.outcome.into(),
        }
    }
}
