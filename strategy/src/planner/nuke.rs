//! Bounded pit comparison: project each actor once, then compare six circles.
use super::*;
use rsvz::core::contact::{circle_hits_rect, grid_explosion_shape};
use rsvz::core::model::GridExplosionKind;

#[derive(Clone, Copy)]
pub(super) struct Retention {
    wave: i32,
    started: i32,
    until: i32,
    target: ZombieId,
    pit: Pos,
    ice: Option<(Pos, i32)>,
}
#[derive(Clone)]
pub(crate) struct Snapshot {
    debts: Vec<(ZombieId, i32)>,
    hold: Option<Retention>,
    current: Option<(i32, i32)>,
    next: i32,
    exclude47: bool,
}
fn covered(pos: Pos, z: Zombie, out: Outcome, at: i32) -> bool {
    let shape = grid_explosion_shape(GridExplosionKind::DoomShroom, grid(pos));
    out.death > at
        && (z.row - pos.0).abs() <= shape.row_range
        && circle_hits_rect(shape.circle, z.rect_at(out.x_at_end))
}
fn contribution(pos: Pos, z: Zombie, out: Outcome, at: i32) -> i64 {
    if !covered(pos, z, out, at) {
        return 0;
    }
    let left = ((600. - out.x_at_end).max(0.) as i64).min(600);
    let value = if z.giant() {
        1800.min(out.hp_at_end.max(0)) as i64
    } else {
        600
    };
    value * (600 + left)
}

impl Planner {
    pub(super) fn keep_nuke_dependency(&mut self, z: Zombie, out: Outcome, f: &Forecast) {
        if z.kind != Z::GigaGargantuar
            || z.x > 500.
            || !out.safe()
            || out.death == NEVER
            || self.nuke_debts.iter().any(|(id, _)| *id == z.id)
        {
            return;
        }
        let Some(index) = f.events.iter().enumerate().find_map(|(i, e)| {
            (e.kind == prediction::EffectKind::Doom && !e.certain && out.ash_mask & (1 << i) != 0).then_some(i)
        }) else {
            return;
        };
        let without = Forecast {
            events: f
                .events
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, e)| *e)
                .collect(),
        };
        let independent = run(z, &self.scene, &without, None);
        if !independent.safe() {
            self.nuke_debts.push((z.id, telemetry::damage_history(z.id).0));
            telemetry::record(
                "nuke_damage_dependency",
                &[z.id.raw() as i64, f.events[index].at as i64],
            );
        }
    }
    pub fn save_retention(&self) -> Snapshot {
        Snapshot {
            debts: self.nuke_debts.clone(),
            hold: self.nuke_hold,
            current: self.current_nuke,
            next: self.next_nuke_retention,
            exclude47: self.nuke_exclude47,
        }
    }
    pub fn restore_retention(&mut self, saved: Snapshot) {
        self.nuke_debts = saved.debts;
        self.nuke_hold = saved.hold;
        self.current_nuke = saved.current;
        self.next_nuke_retention = saved.next;
        self.nuke_exclude47 = saved.exclude47;
    }
    pub fn set_current_nuke(&mut self, wave: i32, clock: Option<i32>) {
        let plan = clock.map(|at| (wave, at));
        if self.current_nuke != plan {
            self.next_nuke_retention = 0;
        }
        self.current_nuke = plan;
        if self.nuke_hold.is_some_and(|h| plan != Some((h.wave, h.until))) {
            self.nuke_hold = None;
        }
    }
    pub(super) fn group_pad_keeps_jacks_in_gloom(&self, pad: Pad) -> bool {
        // This admission rule protects optional forward storage in the two
        // lanes covered by the front Glooms. It is not a veto on back-field
        // emergency blockers, whose lanes need not have a column-six gun.
        if !matches!(pad.pos.0, 2 | 5) || pad.pos.1 <= 6 {
            return true;
        }
        let rect = pad.rect();
        self.scene.row_zombies(pad.pos.0).all(|z| {
            if z.kind != Z::JackInTheBox
                || z.phase != ZombiePhase::JackInTheBoxRunning
                || !prediction::could_contact_pad(*z, pad)
            {
                return true;
            }
            // First chewing contact while moving left. If already overlapping,
            // it stops at its present position rather than at the grid center.
            let stop = z.x.min((rect.right - 15) as f32);
            self.scene.plants.iter().any(|p| {
                p.kind == GloomShroom
                    && p.pos.1 == 6
                    && (p.pos.0 - z.row).abs() <= 1
                    && z.rect_at(stop).left < p.attack.right
            })
        })
    }
    pub fn nuke_position(&mut self, f: &Forecast, at: i32, preferred: Pos, exclude47: bool) -> Pos {
        self.nuke_debts.retain(|(id, hits)| {
            self.scene.zombies.iter().any(|z| z.id == *id) && telemetry::damage_history(*id).0 <= *hits
        });
        self.nuke_exclude47 = exclude47;
        let positions = [preferred, (3, 7), (4, 7), (3, 8), (4, 8), (3, 9), (4, 9)];
        let legal: Vec<_> = positions
            .into_iter()
            .filter(|&p| !(exclude47 && p == (4, 7)) && combat::candidate(sel(DoomShroom), p))
            .collect();
        if legal.is_empty() {
            return preferred;
        }
        if let Some(hold) = self.nuke_hold.filter(|p| {
            p.until == self.scene.now + at
                && self.scene.zombies.iter().any(|z| z.id == p.target)
                && legal.contains(&p.pit)
        }) {
            return hold.pit;
        }
        let without = Forecast {
            events: f
                .events
                .iter()
                .filter(|e| !(e.kind == prediction::EffectKind::Doom && !e.certain && e.at == at))
                .copied()
                .collect(),
        };
        let mut score = vec![0i64; legal.len()];
        let mut protected = vec![0usize; legal.len()];
        for &z in &self.scene.zombies {
            if !(z.giant() || z.kind == Z::Football || z.kind == Z::Zomboni) {
                continue;
            }
            let out = prediction::simulate_until(z, &self.scene, &without, None, at);
            if out.death <= at {
                continue;
            }
            // Damage on left-hand unresolved actors has less opportunity to be
            // replaced by the next ash. Count all hits, with a continuous urgency bonus.
            let left = ((600. - out.x_at_end).max(0.) as i64).min(600);
            let value = if z.giant() {
                1800.min(out.hp_at_end.max(0)) as i64
            } else {
                600
            };
            for (i, &pos) in legal.iter().enumerate() {
                let shape = grid_explosion_shape(GridExplosionKind::DoomShroom, grid(pos));
                if (z.row - pos.0).abs() <= shape.row_range && circle_hits_rect(shape.circle, z.rect_at(out.x_at_end)) {
                    score[i] += value * (600 + left);
                    protected[i] += usize::from(self.nuke_debts.iter().any(|(id, _)| *id == z.id));
                }
            }
        }
        let mut best = 0;
        for i in 1..legal.len() {
            if (protected[i], score[i]) > (protected[best], score[best]) {
                best = i;
            }
        }
        legal[best]
    }
    pub fn retained_nuke(&self, wave: i32) -> Option<Pos> {
        self.nuke_hold
            .filter(|p| {
                p.wave == wave
                    && self.scene.now <= p.until
                    && self.scene.zombies.iter().any(|z| z.id == p.target)
                    && !(self.nuke_exclude47 && p.pit == (4, 7))
                    && combat::candidate(sel(DoomShroom), p.pit)
            })
            .map(|p| p.pit)
    }
    pub fn retention_started(&self, now: i32) -> bool {
        self.nuke_hold.is_some_and(|p| p.started == now)
    }
    pub fn add_retention_body(&self, wave: i32, f: &mut Forecast) {
        let Some(p) = self.nuke_hold.filter(|p| {
            p.wave == wave && self.scene.now <= p.until && self.scene.zombies.iter().any(|z| z.id == p.target)
        }) else {
            return;
        };
        if !f
            .events
            .iter()
            .any(|e| e.kind == prediction::EffectKind::Doom && e.pos == p.pit && self.scene.now + e.at == p.until)
        {
            return;
        }
        let Some((pos, clock)) = p.ice else { return };
        if let Some(e) = f
            .events
            .iter_mut()
            .find(|e| e.kind == prediction::EffectKind::Ice && !e.certain && self.scene.now + e.at == clock)
        {
            e.pos = pos;
            e.body_at = Some((e.at - 100).max(0));
        }
    }
    pub fn retention_ice_observed(&mut self, pos: Pos, clock: i32) {
        if self
            .nuke_hold
            .is_some_and(|p| p.ice.is_some_and(|(wanted, at)| clock >= at - 5 && pos != wanted))
        {
            self.nuke_hold = None;
        }
    }
    pub fn retention_ice_planted(&mut self, pos: Option<Pos>, effect: i32) -> bool {
        let invalid = self.nuke_hold.is_some_and(|p| {
            p.ice
                .is_some_and(|(wanted, at)| effect >= at - 5 && pos != Some(wanted))
        });
        if invalid {
            self.nuke_hold = None;
        }
        invalid
    }
    pub fn retention_ice_cell(&self, pos: Pos) -> bool {
        self.nuke_hold.is_some_and(|p| {
            p.ice
                .is_some_and(|(wanted, at)| wanted == pos && (self.scene.now + 100 - at).abs() <= 5)
        })
    }
    pub(super) fn retention_ice_pad(
        &self, card: CardSelection, f: &Forecast, earliest: i32, tail: bool,
    ) -> Option<(Pos, Zombie, Outcome, Outcome)> {
        if card != sel(IceShroom) {
            return None;
        }
        let hold = self.nuke_hold.filter(|p| p.wave == time().1)?;
        let (pos, at) = hold.ice?;
        if self.scene.now > hold.until
            || (self.scene.now + 100 - at).abs() > 5
            || !combat::candidate(card, pos)
            || !combat::jack_safe(pos, 100)
            || !rsvz::is_safe_ice(grid(pos)).unwrap_or(false)
            || tail && !self.tail_ice_inside(card, Some(pos))
        {
            return None;
        }
        let z = *self.scene.zombies.iter().find(|z| z.id == hold.target)?;
        let mut pad = Pad::new(pos, IceShroom);
        pad.order = self.scene.next_plant_order;
        // The committed plan may replace its spent/prestored Puff on this cell.
        let mut scene = self.scene.clone();
        scene.plants.retain(|p| p.pos != pos || p.kind != PuffShroom);
        let after = run(z, &scene, f, Some(pad));
        if after.impact < earliest.max(100) || after.trigger >= 100 {
            return None;
        }
        Some((pos, z, run(z, &self.scene, f, None), after))
    }
    pub(super) fn try_nuke_retention(
        &mut self, wave: i32, f: &Forecast, ordinary: Option<&(Pos, Zombie, Outcome, Outcome)>,
    ) -> bool {
        if wave == 20
            || !ready(sel(PuffShroom))
            || self.puff_window_hold == Some(self.scene.now)
            || self.scene.now < self.next_nuke_retention
            || self.retained_nuke(wave).is_some()
        {
            return false;
        }
        let Some(doom) = f
            .events
            .iter()
            .find(|e| e.kind == prediction::EffectKind::Doom && !e.certain && (200..=2500).contains(&e.at))
            .copied()
        else {
            return false;
        };
        if self.current_nuke != Some((wave, self.scene.now + doom.at)) {
            return false;
        }
        self.next_nuke_retention = self.scene.now + 100;
        let doom_bit = f
            .events
            .iter()
            .position(|e| e.kind == prediction::EffectKind::Doom && e.at == doom.at && !e.certain)
            .map_or(0, |i| 1u32 << i);
        let mut anchors = Vec::new();
        for index in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[index];
            if z.kind == Z::GigaGargantuar && matches!(z.row, 1 | 6) && z.x < 600. {
                let feet = foot_columns(z);
                if !(&feet[..]).iter().any(|&col| {
                    col > 0
                        && self.scene.main((z.row, col)).is_none()
                        && legal(sel(PuffShroom), (z.row, col))
                        && prediction::contact(z, z.x, Pad::new((z.row, col), PuffShroom).rect())
                }) {
                    continue;
                }
                let base = self.assessment(z, f);
                if !base.safe() && base.ash_mask & doom_bit == 0 {
                    anchors.push((z, base));
                }
            }
        }
        anchors.sort_by_key(|(_, base)| base.danger);
        let mut rows = [0u8; 6];
        anchors.retain(|(z, _)| {
            let seen = &mut rows[(z.row - 1) as usize];
            let keep = *seen < 2;
            *seen += 1;
            keep
        });
        if anchors.is_empty() {
            return false;
        }
        let pits: Vec<_> = [doom.pos, (3, 7), (4, 7), (3, 8), (4, 8), (3, 9), (4, 9)]
            .into_iter()
            .filter(|&p| {
                !(self.nuke_exclude47 && p == (4, 7))
                    && combat::candidate(sel(DoomShroom), p)
                    && combat::jack_safe(p, 100)
            })
            .fold(Vec::new(), |mut v, p| {
                if !v.contains(&p) {
                    v.push(p);
                }
                v
            });
        let without = Forecast {
            events: f
                .events
                .iter()
                .copied()
                .filter(|e| !(e.kind == prediction::EffectKind::Doom && !e.certain && e.at == doom.at))
                .collect(),
        };
        let base: Vec<_> = self
            .scene
            .zombies
            .iter()
            .copied()
            .filter(|z| z.giant() || matches!(z.kind, Z::Football | Z::Zomboni))
            .map(|z| (z, prediction::simulate_until(z, &self.scene, &without, None, doom.at)))
            .collect();
        let scores: Vec<_> = pits
            .iter()
            .map(|&pit| base.iter().map(|&(z, o)| contribution(pit, z, o, doom.at)).sum::<i64>())
            .collect();
        let mut best_score = scores.iter().copied().max().unwrap_or(0);
        let mut best = None;
        let ice = without
            .events
            .iter()
            .find(|e| e.kind == prediction::EffectKind::Ice)
            .copied()
            .filter(|e| {
                !e.certain
                    && e.at > 100
                    && e.at < doom.at
                    && e.at - 100 < 751
                    && cd(sel(IceShroom)) + 100 <= e.at
                    && cd(sel(IceShroom)) + 100 <= cd(COPY) + 420
            });
        let mut cells = Vec::new();
        for (z, baseline) in anchors {
            let ice_road = ice.map_or(800, |e| {
                self.scene
                    .row_zombies(z.row)
                    .filter(|a| a.kind == Z::Zomboni)
                    .map(|&car| {
                        prediction::simulate_until(car, &self.scene, &without, None, e.at - 100).x_at_end as i32 + 118
                    })
                    .min()
                    .unwrap_or(800)
            });
            let feet = foot_columns(z);
            for &col in &feet[..] {
                let pos = (z.row, col);
                if col == 0
                    || cells.contains(&pos)
                    || self.scene.main(pos).is_some()
                    || !legal(sel(PuffShroom), pos)
                    || !combat::jack_safe(pos, 1)
                {
                    continue;
                }
                if let Some((_, other, before, _)) = ordinary {
                    if other.row != z.row && (before.danger <= 751 || self.last_pad_window_closes(*other, *before, f)) {
                        continue;
                    }
                }
                let mut pad = Pad::new(pos, PuffShroom);
                pad.order = self.scene.next_plant_order;
                pad.complete_outcome = true;
                pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, z, pos);
                if !prediction::could_contact_pad(z, pad) {
                    continue;
                }
                let first = prediction::simulate_until(z, &self.scene, &without, Some(pad), doom.at);
                if first.trigger >= 751 || first.impact <= first.trigger {
                    continue;
                }
                if !self.imp_allows_pad(z, baseline, first, f) {
                    continue;
                }
                let mut bodies = vec![None];
                if let Some(e) = ice {
                    let before = prediction::simulate_until(z, &self.scene, &without, Some(pad), e.at - 100);
                    if before.death > e.at - 100 && before.danger > e.at - 100 {
                        let future = Zombie {
                            x: before.x_at_end,
                            rect: z.rect_at(before.x_at_end),
                            ..z
                        };
                        let feet = foot_columns(future);
                        for &col in &feet[..] {
                            let ip = (z.row, col);
                            if col > 0
                                && ice_road >= [108, 188, 268, 348, 428, 508, 588, 668, 751][(col - 1) as usize]
                                && combat::candidate(sel(IceShroom), ip)
                            {
                                bodies.push(Some((ip, e.at)));
                            }
                        }
                    }
                }
                for body in bodies {
                    let mut plan = Forecast {
                        events: without.events.clone(),
                    };
                    if let Some((ip, at)) = body {
                        let e = plan
                            .events
                            .iter_mut()
                            .find(|e| e.kind == prediction::EffectKind::Ice && e.at == at)
                            .unwrap();
                        e.pos = ip;
                        e.body_at = Some(at - 100);
                    }
                    let anchor = if body.is_none() {
                        first
                    } else {
                        prediction::simulate_until(z, &self.scene, &plan, Some(pad), doom.at)
                    };
                    if anchor.danger <= doom.at
                        || body.is_some_and(|(_, at)| anchor.first_ice != at)
                        || !pits.iter().any(|&pit| {
                            covered(pit, z, anchor, doom.at)
                                && !base
                                    .iter()
                                    .find(|(a, _)| a.id == z.id)
                                    .is_some_and(|&(a, o)| covered(pit, a, o, doom.at))
                        })
                    {
                        continue;
                    }
                    if !cells.contains(&pos) {
                        cells.push(pos);
                    }
                    let changed: Vec<_> = base
                        .iter()
                        .filter(|(a, _)| a.row == z.row)
                        .map(|&(a, _)| {
                            (
                                a,
                                if a.id == z.id {
                                    anchor
                                } else {
                                    prediction::simulate_until(a, &self.scene, &plan, Some(pad), doom.at)
                                },
                            )
                        })
                        .collect();
                    let Some((_, anchor)) = changed.iter().find(|(a, _)| a.id == z.id) else {
                        continue;
                    };
                    if anchor.danger <= doom.at || body.is_some_and(|(_, at)| anchor.first_ice != at) {
                        continue;
                    }
                    for (i, &pit) in pits.iter().enumerate() {
                        if !covered(pit, z, *anchor, doom.at)
                            || base
                                .iter()
                                .find(|(a, _)| a.id == z.id)
                                .is_some_and(|&(a, o)| covered(pit, a, o, doom.at))
                        {
                            continue;
                        }
                        let old = base
                            .iter()
                            .filter(|(a, _)| a.row == z.row)
                            .map(|&(a, o)| contribution(pit, a, o, doom.at))
                            .sum::<i64>();
                        let new = changed
                            .iter()
                            .map(|&(a, o)| contribution(pit, a, o, doom.at))
                            .sum::<i64>();
                        let score = scores[i] - old + new;
                        if score > best_score {
                            best_score = score;
                            best = Some((
                                pos,
                                Retention {
                                    wave,
                                    started: self.scene.now,
                                    until: self.scene.now + doom.at,
                                    target: z.id,
                                    pit,
                                    ice: body.map(|(p, at)| (p, self.scene.now + at)),
                                },
                            ));
                        }
                    }
                }
            }
        }
        if let Some((pos, hold)) = best {
            if play(sel(PuffShroom), pos, 31) {
                self.nuke_hold = Some(hold);
                telemetry::record(
                    "nuke_retention_plan",
                    &[
                        hold.target.raw() as i64,
                        pos.0 as i64,
                        pos.1 as i64,
                        hold.pit.0 as i64,
                        hold.pit.1 as i64,
                        hold.until as i64,
                        hold.ice.map_or(0, |p| p.0.1) as i64,
                        hold.ice.map_or(0, |p| p.1) as i64,
                    ],
                );
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz::core::model::{ContactRect, PlantId};
    #[test]
    fn puff_and_scheduled_ice_can_retain_a_pair_for_an_alternate_pit() {
        use rsvz::core::backend::*;
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            initial_sun: 8000,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        world.with_backend(|b| {
            b.set_zombie_spawn_stopped(true).unwrap();
            for c in [sel(PuffShroom), sel(IceShroom), COPY, sel(DoomShroom), sel(LilyPad)] {
                b.select_card(c.checked().unwrap()).unwrap();
            }
            b.finish_card_selection().unwrap();
            for pos in [(3, 7), (4, 7), (3, 8), (4, 8), (3, 9), (4, 9)] {
                b.new_plant(sel(LilyPad).checked().unwrap(), grid(pos)).unwrap();
                if pos.1 != 8 {
                    b.new_plant(sel(WallNut).checked().unwrap(), grid(pos)).unwrap();
                }
            }
            PeBackend::scope_backend(b, || {
                let a = Zombie {
                    age: 0,
                    id: ZombieId::from_raw(65537),
                    kind: Z::GigaGargantuar,
                    row: 6,
                    wave: 1,
                    carried: false,
                    x: 417.46,
                    hp: 4180,
                    armor: 0,
                    phase: ZombiePhase::ZombieNormal,
                    progress: 0.304364,
                    speed: 0.367675,
                    frozen: 0,
                    chilled: 0,
                    object: true,
                    eating: false,
                    rect: ContactRect::new(400, 437, 525, 591),
                };
                let other = Zombie {
                    id: ZombieId::from_raw(65538),
                    x: 432.58,
                    progress: 0.172042,
                    speed: 0.363840,
                    rect: ContactRect::new(415, 437, 540, 591),
                    ..a
                };
                let mut p = Planner::default();
                p.scene = Scene::fixture(vec![a, other]);
                p.scene.plants.push(prediction::Plant {
                    id: PlantId::from_raw(65537),
                    kind: GloomShroom,
                    pos: (5, 2),
                    hp: 300,
                    rect: ContactRect::new(130, 420, 190, 500),
                    attack: ContactRect::new(40, 340, 280, 580),
                    shooting: 71,
                    launch: 57,
                    state: 0,
                    phase_left: 0,
                    effect: 0,
                });
                let mut f = Forecast::default();
                f.add(380, prediction::EffectKind::Ice, (5, 1), false);
                f.add(1898, prediction::EffectKind::Doom, (3, 8), false);
                f.add(2928, prediction::EffectKind::Ice, (5, 1), false);
                f.add(4498, prediction::EffectKind::Jalapeno, (6, 2), false);
                assert!(
                    !p.try_nuke_retention(2, &f, None),
                    "a forecast next-wave nuke is not a current commitment"
                );
                p.set_current_nuke(2, Some(1898));
                assert!(p.try_nuke_retention(2, &f, None));
                let hold = p.nuke_hold.unwrap();
                assert_eq!(hold.pit, (4, 8));
                assert!(hold.ice.is_some());
                let doom = f
                    .events
                    .iter_mut()
                    .find(|e| e.kind == prediction::EffectKind::Doom)
                    .unwrap();
                doom.pos = hold.pit;
                doom.shape = Some(grid_explosion_shape(GridExplosionKind::DoomShroom, grid(hold.pit)));
                p.add_retention_body(2, &mut f);
                assert!(f.events.iter().any(|e| e.body_at == Some(280) && e.pos.0 == 6));
                assert_eq!(p.retained_nuke(2), Some((4, 8)));
                let planted = rsvz::with_backend(|b| {
                    b.plants()
                        .unwrap()
                        .find(|&p| b.plant_raw_kind(p).unwrap() == PuffShroom)
                        .map(|p| (b.plant_row(p) + 1, b.plant_col(p) + 1))
                        .unwrap()
                });
                let mut pad = Pad::new(planted, PuffShroom);
                pad.complete_outcome = true;
                for target in [a, other] {
                    let out = prediction::simulate_until(target, &p.scene, &f, Some(pad), 1898);
                    assert_eq!(out.first_ice, 380);
                    assert!(out.ash_hits >= 1);
                }
                assert_eq!(p.retained_nuke(3), None);
                p.retention_ice_observed((5, 1), 380);
                assert!(p.retained_nuke(2).is_none());
                p.nuke_hold = Some(hold);
                p.set_current_nuke(3, Some(1898));
                assert!(p.retained_nuke(2).is_none());
            });
        });
    }
    #[test]
    fn optional_grouping_must_not_stop_a_jack_outside_the_gloom() {
        let z = Zombie {
            age: 0,
            id: ZombieId::from_raw(65537),
            kind: Z::JackInTheBox,
            row: 5,
            wave: 20,
            carried: false,
            x: 800.,
            hp: 500,
            armor: 0,
            phase: ZombiePhase::JackInTheBoxRunning,
            progress: 0.,
            speed: 0.68,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(835, 400, 895, 515),
        };
        let mut p = Planner::default();
        p.scene = Scene::fixture(vec![z]);
        p.scene.plants.push(prediction::Plant {
            id: PlantId::from_raw(65537),
            kind: GloomShroom,
            pos: (4, 6),
            hp: 300,
            rect: ContactRect::new(450, 300, 510, 380),
            attack: ContactRect::new(360, 200, 600, 600),
            shooting: 0,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: 0,
        });
        assert!(!p.group_pad_keeps_jacks_in_gloom(Pad::new((5, 9), PuffShroom)));
        assert!(p.group_pad_keeps_jacks_in_gloom(Pad::new((5, 6), PuffShroom)));
        // A row-one Jack must not suppress the last useful Football/red pad:
        // row one intentionally has no neighbouring front column-six Gloom.
        p.scene = Scene::fixture(vec![Zombie { row: 1, ..z }]);
        assert!(p.group_pad_keeps_jacks_in_gloom(Pad::new((1, 1), PuffShroom)));
        assert!(p.group_pad_keeps_jacks_in_gloom(Pad::new((1, 2), PuffShroom)));
    }
}
