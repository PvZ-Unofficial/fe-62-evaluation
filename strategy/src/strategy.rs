use super::*;
use diagnostics::Condition as C;
use rsvz::core::model::ActivationFact;
mod stack_layout;

// Flag-wave Football full-damage boundary is332cs under natural movement.
// Leave one update of margin; this is an effect time, not a planting time.
const W20_JALA_EFFECT: i32 = 333;

// All times are activation times. The budget permits a later next ice;
// it never delays an ice whose card and coverage conditions are already ready.
fn early_nuke_due(doom_ready: i32, previous_length: i32, prior_ice: i32) -> i32 {
    doom_ready
        .max(4801 - previous_length)
        .max(5001 + prior_ice - previous_length - 950 - 200)
}

#[derive(Clone, Copy)]
struct CherryPlan {
    effect_clock: i32,
    checked_at: i32,
    serial: u32,
    flag: bool,
    pos: Pos,
}

pub struct Control {
    stack_prepared: bool,
    decision_period: i32,
    cherry_period: i32,
    next_economy_check: i32,
    next_ash_retry: i32,
    next_w20_guard: i32,
    wave: i32,
    wave_start: i32,
    previous_length: i32,
    last_ice: i32,
    first_ice: [i32; 21],
    ice_count: [u8; 21],
    pending_ice: i32,
    ice_due: i32,
    late_ice_floor: i32,
    ice_floor_sampled: bool,
    normal_ice_advanced: bool,
    tail_ice_due: i32,
    next_tail_ice_check: i32,
    doom_due: i32,
    planned_doom: Option<Pos>,
    next_pit_check: i32,
    ice_effect_wave: i32,
    nuke_done: [bool; 21],
    pending_doom_wave: i32,
    carried_doom: i32,
    doom_uses: usize,
    even_after_nine: bool,
    ash_births: u32,
    cherry_done: bool,
    jala_done: bool,
    opening_end: i32,
    early: bool,
    early_row1_protected: bool,
    early_alternative: Option<Pos>,
    squash_check: Option<(i32, i32, i32, bool)>,
    last_jala_wave: i32,
    last_jala_effect: i32,
    tail_hold_logged: bool,
    planner: planner::Planner,
    forecast: prediction::Forecast,
    next_w20_doom_check: i32,
    cherry_plans: [Option<CherryPlan>; 3],
    policy: policy::Policy,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            stack_prepared: false,
            decision_period: setting("SIX_DECISION_PERIOD", 20).max(1) as i32,
            cherry_period: setting("SIX_CHERRY_PERIOD", 250).max(1) as i32,
            next_economy_check: 0,
            next_ash_retry: 0,
            next_w20_guard: 0,
            wave: 0,
            wave_start: 0,
            previous_length: 0,
            last_ice: -99999,
            first_ice: [-1; 21],
            ice_count: [0; 21],
            pending_ice: -1,
            ice_due: 0,
            late_ice_floor: 0,
            ice_floor_sampled: false,
            normal_ice_advanced: false,
            tail_ice_due: 2600,
            next_tail_ice_check: 0,
            doom_due: 2301,
            planned_doom: None,
            next_pit_check: 0,
            ice_effect_wave: 0,
            nuke_done: [false; 21],
            pending_doom_wave: 0,
            carried_doom: 0,
            doom_uses: 0,
            even_after_nine: false,
            ash_births: 0,
            cherry_done: false,
            jala_done: false,
            opening_end: 0,
            early: false,
            early_row1_protected: false,
            early_alternative: None,
            squash_check: None,
            last_jala_wave: 0,
            last_jala_effect: -99999,
            tail_hold_logged: false,
            planner: planner::Planner::default(),
            forecast: prediction::Forecast::default(),
            next_w20_doom_check: 0,
            cherry_plans: [None; 3],
            policy: policy::Policy::default(),
        }
    }
}
impl Control {
    pub fn sample_boundary(&mut self) {
        self.planner.observe(rsvz::timing::clock(), self.last_ice);
    }
    pub fn replay_endpoint(&mut self, seed: u32, kind: u8) {
        if !policy::replay_enabled() {
            return;
        }
        let (now, w, t) = time();
        let mut fresh = self.planner.scene.clone();
        fresh.sample(now, self.last_ice);
        let old_scene = std::mem::replace(&mut self.planner.scene, fresh);
        let old_forecast = std::mem::take(&mut self.forecast);
        let old_cherry_plans = self.cherry_plans;
        let old_cherry_guard = self.planner.cherry_guard.clone();
        let old_pit_plan = (self.planned_doom, self.next_pit_check);
        let old_retention = self.planner.save_retention();
        if w > 0 {
            self.refresh_forecast(now, w, t);
        }
        let context = self.policy_context(now, w, t);
        self.forecast.events.retain(|e| {
            e.certain
                || !matches!(
                    e.kind,
                    prediction::EffectKind::Cherry | prediction::EffectKind::Jalapeno
                )
        });
        policy::replay_endpoint(&self.planner.scene, &self.forecast, &context, seed, kind);
        self.cherry_plans = old_cherry_plans;
        self.planner.cherry_guard = old_cherry_guard;
        (self.planned_doom, self.next_pit_check) = old_pit_plan;
        self.planner.restore_retention(old_retention);
        self.forecast = old_forecast;
        self.planner.scene = old_scene;
    }
    pub fn next_round(&mut self) {
        self.policy.round += 1;
        self.wave = 0;
        self.planned_doom = None;
        self.next_pit_check = 0;
        self.normal_ice_advanced = false;
        self.wave_start = 0;
        self.previous_length = 0;
        self.first_ice.fill(-1);
        self.ice_count.fill(0);
        self.ice_effect_wave = 0;
        self.ice_due = 830;
        self.late_ice_floor = 830;
        self.ice_floor_sampled = false;
        self.nuke_done.fill(false);
        self.pending_doom_wave = 0;
        self.carried_doom = 0;
        self.even_after_nine = false;
        self.ash_births = 0;
        self.cherry_done = false;
        self.jala_done = false;
        self.next_economy_check = 0;
        self.early = false;
        self.early_row1_protected = false;
        self.early_alternative = None;
        self.squash_check = None;
        self.next_w20_doom_check = 0;
        self.tail_hold_logged = false;
        self.tail_ice_due = 2600;
        self.next_tail_ice_check = 0;
        self.last_jala_wave = 0;
        self.opening_end = rsvz::core::timeline::with_timeline_ref(|tl| tl.wave_clocks().refresh_clock(Wave(1)))
            .unwrap_or_else(|| rsvz::timing::clock() + 600);
        self.planner.next_round();
        telemetry::record(
            "round_carry",
            &[self.opening_end as i64, self.doom_uses as i64, self.last_ice as i64],
        );
    }
    fn fill_opening_forecast(f: &mut prediction::Forecast, first: i32) {
        f.events.clear();
        let mut ice = [cd(sel(IceShroom)) + 100, cd(COPY) + 420];
        Self::reserve_ice(f, &mut ice, first + 830);
        f.add(
            (first + 2300).max(cd(sel(CherryBomb)) + 100),
            prediction::EffectKind::Cherry,
            (2, 4),
            false,
        );
        f.add(
            (first + 2400).max(cd(sel(Jalapeno)) + 100),
            prediction::EffectKind::Jalapeno,
            (6, 2),
            false,
        );
        f.events.sort_by_key(|e| e.at);
    }
    fn opening_tick(&mut self, now: i32) {
        let period = if self.policy.enabled() { 5 } else { self.decision_period };
        if self.opening_end == 0 || now % period != 0 {
            return;
        }
        self.planner.observe(now, self.last_ice);
        if self.policy.enabled() {
            if now % setting("SIX_POLICY_PERIOD", 5).max(1) as i32 != 0 {
                return;
            }
            self.policy_tick(now, 0, -(self.opening_end - now));
            return;
        }
        let first = (self.opening_end - now).max(0);
        Self::fill_opening_forecast(&mut self.forecast, first);
        for p in &self.planner.scene.plants {
            let kind = match p.kind {
                IceShroom | Imitator => prediction::EffectKind::Ice,
                CherryBomb => prediction::EffectKind::Cherry,
                Jalapeno => prediction::EffectKind::Jalapeno,
                DoomShroom => prediction::EffectKind::Doom,
                _ => continue,
            };
            self.forecast.add(p.effect.max(1), kind, p.pos, true);
        }
        prediction::resolve_live_ice(&self.planner.scene, &mut self.forecast);
        self.resolve_cherries(now, 0, first);
        policy::shadow_context(self.policy_context(now, 0, -first), &self.forecast, &self.planner.scene);
        self.planner.refresh_imp_reservation(&self.forecast);
        combat::maintenance();
        if self.planner.tail_group(0, &self.forecast) {
            self.planner.observe(now, self.last_ice);
        }
        if self.planner.support_squash(0, &self.forecast) {
            self.planner.observe(now, self.last_ice);
        }
        self.planner.pads(0, -first, &self.forecast);
        policy::shadow_context(self.policy_context(now, 0, -first), &self.forecast, &self.planner.scene);
        policy::shadow_wait();
        diagnostics::balance(rsvz::board::sun());
    }
    pub fn motion_changed(&mut self, fact: &rsvz::event::GargantuarMotionFact) {
        self.planner.motion_changed(fact);
    }
    pub fn ice_planted(&mut self, w: i32, now: i32, lead: i32) {
        self.ice_count[w as usize] += 1;
        self.pending_ice = now + lead;
        self.ice_effect_wave = w;
        if !self.policy.enabled() {
            let (_, _, t) = time();
            self.sync_shadow(w, t);
        }
    }
    pub fn activated(&mut self, a: &ActivationFact, reason: i32) {
        match a.kind {
            ActivationKind::Ice => {
                self.planner
                    .retention_ice_observed((a.grid.row + 1, a.grid.col + 1), a.main_counter);
                self.last_ice = a.main_counter + 1;
                self.pending_ice = -1;
                let (_, _, t) = time();
                if self.first_ice[self.wave as usize] < 0 {
                    self.first_ice[self.wave as usize] = t;
                }
                if t > 906 && self.wave != 10 && self.wave != 20 && self.ice_count[self.ice_effect_wave as usize] == 1 {
                    diagnostics::mark(C::LateIce);
                }
            }
            ActivationKind::Doom => {
                let origin = if self.pending_doom_wave > 0 {
                    self.pending_doom_wave
                } else {
                    self.wave
                };
                self.nuke_done[origin as usize] = true;
                if self.nuke_wave(self.wave) && !self.late_nuke(self.wave) {
                    self.nuke_done[self.wave as usize] = true;
                }
                self.pending_doom_wave = 0;
                self.carried_doom = 0;
                self.doom_uses += 1;
                telemetry::record(
                    "track_nuke_effect",
                    &[origin as i64, self.wave as i64, self.doom_uses as i64],
                );
                let left = rsvz::with_frame(|b| {
                    b.zombies().any(|z| {
                        z.is_alive()
                            && z.has_head()
                            && z.x() < 430.
                            && matches!(z.kind(), Z::Football | Z::Gargantuar | Z::GigaGargantuar)
                    })
                });
                if left {
                    diagnostics::mark(C::DoomLeftMiss);
                }
            }
            ActivationKind::Squash if reason == 6 || reason == 22 => {
                self.squash_check = Some((a.main_counter + 3, a.grid.row + 1, self.wave, reason == 22))
            }
            ActivationKind::Jalapeno => {
                self.last_jala_effect = a.main_counter + 1;
                if self.last_jala_wave != self.wave {
                    diagnostics::mark(C::LateJalapeno);
                }
            }
            ActivationKind::Cherry => self.cherry_done = true,
            _ => {}
        }
    }
    fn nuke_wave(&self, w: i32) -> bool {
        match w {
            2 | 4 | 6 | 8 | 9 | 19 | 20 => true,
            10..=18 => w % 2 == if self.even_after_nine { 0 } else { 1 },
            _ => false,
        }
    }
    fn late_nuke(&self, w: i32) -> bool {
        w == 9 || w == 19 && self.even_after_nine
    }
    fn ash_wave(&self, w: i32) -> bool {
        !self.nuke_wave(w) || self.late_nuke(w) || w == 20
    }
    fn need_nuke(&self, w: i32) -> bool {
        self.carried_doom > 0 || self.nuke_wave(w) && !self.nuke_done[w as usize]
    }
    fn known_next_clock(w: i32) -> Option<i32> {
        if w >= 20 {
            return None;
        }
        rsvz::core::timeline::with_timeline_ref(|tl| tl.wave_clocks().observed_refresh_clock(Wave(w + 1)))
    }
    fn pit(index: usize) -> Pos {
        [(3, 7), (4, 7), (4, 8), (3, 8)][index % 4]
    }
    fn enter(&mut self, now: i32, w: i32, t: i32) {
        self.planner.invalidate();
        if self.wave > 0 {
            self.previous_length = now - t - self.wave_start;
            if self.previous_length < 2450 {
                diagnostics::mark(C::ShortWave);
            }
            if self.nuke_wave(self.wave) && !self.nuke_done[self.wave as usize] {
                self.carried_doom = self.wave;
                if self.wave == 9 {
                    self.even_after_nine = true;
                }
                telemetry::record(
                    "track_carried_nuke",
                    &[self.wave as i64, w as i64, self.previous_length as i64],
                );
            }
        }
        self.planned_doom = None;
        self.next_pit_check = 0;
        self.wave = w;
        self.wave_start = now - t;
        self.normal_ice_advanced = false;
        self.cherry_done = false;
        self.jala_done = false;
        self.next_economy_check = 0;

        self.tail_hold_logged = false;
        self.tail_ice_due = 2600;
        self.next_tail_ice_check = 0;
        if self.ash_wave(w) {
            self.ash_births |= 1 << w;
        }
        // An early nuke must leave enough total time for the previous AJ wave's
        // ice and A/J to recover, including W2's initially-ready first nuke.
        let prior_ice = self.first_ice[(w - 1).max(0) as usize].max(830);
        let early_due = early_nuke_due(cd(sel(DoomShroom)) + 100, self.previous_length, prior_ice);
        self.early_alternative = None;
        self.early = self.nuke_wave(w)
            && !self.late_nuke(w)
            && !matches!(w, 10 | 20)
            && self.carried_doom == 0
            && self.previous_length > 2969
            && early_due <= 1831;
        self.guard_early_nuke(w);
        self.ice_due = if matches!(w, 10 | 20) {
            970
        } else if self.early {
            100
        } else if self.nuke_wave(w) && !self.late_nuke(w) {
            783
        } else {
            830
        };
        self.late_ice_floor = self.ice_due;
        self.ice_floor_sampled = false;
        self.doom_due = if self.carried_doom > 0 {
            (cd(sel(DoomShroom)) + 100).max(300)
        } else if self.late_nuke(w) {
            (cd(sel(DoomShroom)) + 100).max(4801)
        } else if w == 20 {
            (cd(sel(DoomShroom)) + 100).max(1700)
        } else if self.early {
            early_due
        } else {
            2301
        };
        diagnostics::wave(w, self.previous_length);
        telemetry::record(
            "track_plan",
            &[
                w as i64,
                self.previous_length as i64,
                self.ice_due as i64,
                self.doom_due as i64,
                i64::from(self.nuke_wave(w)),
                i64::from(self.ash_wave(w)),
                self.doom_uses as i64,
                i64::from(self.even_after_nine),
            ],
        );
    }
    fn ice_target(&self, w: i32, _t: i32, tail: bool) -> i32 {
        if tail {
            if w == 20 { 3256 } else { self.tail_ice_due }
        } else {
            self.ice_due
        }
    }
    fn normal_nuke_first_ice(&self, w: i32) -> bool {
        !self.early && self.carried_doom == 0 && self.nuke_wave(w) && !self.late_nuke(w) && !matches!(w, 10 | 20)
    }
    fn current_row_one_red(&self, w: i32) -> bool {
        self.planner
            .scene
            .row_zombies(1)
            .any(|z| z.kind == Z::GigaGargantuar && z.wave == w && !z.carried)
    }
    fn sample_first_ice_floor(&mut self, now: i32, w: i32, t: i32) {
        let physical = self.planner.late_ice_floor(w, t, self.ash_wave(w));
        self.ice_due = self.ice_due.max(physical);
        if self.normal_nuke_first_ice(w)
            && self
                .planner
                .scene
                .row_zombies(1)
                .any(|z| z.kind == Z::Football && (z.carried || z.wave < w))
        {
            // Assess the actual current-wave plan, not the previous wave's relative times.
            self.refresh_forecast(now, w, t);
            if self.planner.old_football_needs_ice(w, &self.forecast) {
                let ready_at = t + (cd(sel(IceShroom)) + 100).min(cd(COPY) + 420);
                let cover = if self.preferred(w) == (4, 7) && self.current_row_one_red(w) {
                    730
                } else {
                    0
                };
                let at = self
                    .planner
                    .unobstructed_ice_floor(w, t, physical)
                    .max(ready_at)
                    .max(cover);
                if at < self.ice_due {
                    telemetry::record("normal_ice_advanced", &[w as i64, self.ice_due as i64, at as i64]);
                    self.ice_due = at;
                    self.normal_ice_advanced = true;
                    self.next_pit_check = 0;
                }
            }
        }
        self.late_ice_floor = self.ice_due;
        telemetry::record("late_ice_window", &[w as i64, self.ice_due as i64]);
    }
    fn update_tail_ice(&mut self, now: i32, w: i32, t: i32, force: bool) {
        if !matches!(w, 9 | 19)
            || self.ice_count[w as usize] != 1
            || self.pending_ice >= 0
            || !force && now < self.next_tail_ice_check
        {
            return;
        }
        self.next_tail_ice_check = now + 100;
        self.tail_ice_due = match self.planner.tail_ice_delay() {
            Some(delay) => t + delay + 2,
            None => (self.last_ice - self.wave_start + 2000).max(t + 1),
        };
        telemetry::record("tail_ice_position", &[w as i64, self.tail_ice_due as i64]);
    }
    fn fits_known_next_ice(now: i32, next: Option<i32>, other_ready: i32) -> bool {
        next.is_none_or(|clock| other_ready <= (clock - now).max(0) + 970)
    }
    fn tail_ice_allowed(&self, now: i32, w: i32, other_ready: i32) -> bool {
        if w != 19 {
            return true;
        }
        // Only an observed refresh can constrain the other card's deadline.
        // A hypothetical early clear must not delay the current wave's needed ice.
        Self::fits_known_next_ice(now, Self::known_next_clock(w), other_ready)
    }
    fn ice_tick(&mut self, now: i32, w: i32, t: i32) {
        let _perf = crate::perf::scope("strategy::ice_tick");
        if self.pending_ice >= 0 {
            if now > self.pending_ice + 10 {
                // Keep the attempted use counted. A missing activation is reported,
                // never replaced with an unscheduled ice; later scheduled ice stays intact.
                diagnostics::mark(C::IceLost);
                self.pending_ice = -1;
            } else {
                return;
            }
        }
        let count = self.ice_count[w as usize];
        if count == 0 && !self.early && !self.ice_floor_sampled {
            return;
        }
        let tail = matches!(w, 9 | 19 | 20) && count == 1;
        if count > 0 && !tail {
            return;
        }
        let target = self.ice_target(w, t, tail);
        let ordinary = cd(sel(IceShroom)) + 100;
        let copied = cd(COPY) + 420;
        let (c, lead) = if ordinary <= copied {
            (sel(IceShroom), 100)
        } else {
            (COPY, 420)
        };
        if tail && !self.tail_ice_allowed(now, w, ordinary.max(copied)) {
            if !self.tail_hold_logged && t + lead >= target && ready(c) {
                self.tail_hold_logged = true;
                telemetry::record(
                    "tail_ice_reserved",
                    &[w as i64, t as i64, ordinary as i64, copied as i64],
                );
            }
            return;
        }
        if t + lead < target {
            return;
        }
        if !ready(c) {
            if !tail {
                diagnostics::mark(C::IceCd);
            }
            return;
        }
        // Exact target gets 1cs resolution; expensive retries run once per observation.
        if t + lead != target && now % 5 != 0 {
            return;
        }
        if self.decision_period > 5 && self.planner.scene.now != now {
            self.planner.observe(now, self.last_ice);
            self.refresh_forecast(now, w, t);
        }
        let position_tail = tail && matches!(w, 9 | 19);
        if position_tail {
            self.update_tail_ice(now, w, t, true);
            if t + lead < self.tail_ice_due || !self.planner.tail_ice_inside(c, None) {
                self.refresh_forecast(now, w, t);
                return;
            }
            self.refresh_forecast(now, w, t);
        }
        let earliest_effect = if position_tail {
            (self.tail_ice_due - t).max(0)
        } else if !tail && !self.early {
            (self.late_ice_floor - t).max(0)
        } else {
            0
        };
        if count == 0 {
            let wait = self.planner.first_ice_wait(w, c, None, &self.forecast);
            if wait > 0 {
                self.ice_due = t + lead + wait;
                self.refresh_forecast(now, w, t);
                return;
            }
        }
        let pad = self
            .planner
            .ice_pad(c, &self.forecast, earliest_effect, position_tail)
            .filter(|&pos| count != 0 || self.planner.first_ice_inside(w, c, Some(pos), &self.forecast));
        let used = pad.is_some_and(|pos| place(c, pos, 14, self.planner.retention_ice_cell(pos)));
        if used
            || combat::ice(c, |pos| {
                self.planner.ice_timing_safe(pos, &self.forecast, earliest_effect)
                    && (count != 0 || self.planner.first_ice_inside(w, c, Some(pos), &self.forecast))
            })
        {
            let changed = self
                .planner
                .retention_ice_planted(if used { pad } else { None }, now + lead);
            self.ice_planted(w, now, lead);
            if changed {
                self.next_pit_check = 0;
                self.planner.observe(now, self.last_ice);
                self.refresh_forecast(now, w, t);
            }
        }
    }
    fn guard_early_nuke(&mut self, w: i32) {
        self.early_row1_protected = false;
        if !self.early {
            return;
        }
        // All current-wave zombies are instantiated at refresh, including those
        // still waiting to enter. Do not screen by x, visibility or spawn delay.
        let (red, white) = rsvz::with_frame(|b| {
            let mut seen = (false, false);
            for z in b
                .zombies()
                .filter(|z| z.is_alive() && z.row() == 0 && z.from_wave() + 1 == w && !telemetry::carried(z.id()))
            {
                seen.0 |= z.kind() == Z::GigaGargantuar;
                seen.1 |= z.kind() == Z::Gargantuar;
            }
            seen
        });
        // W19 whites missed by early 4-7 can survive W20 and enter home in W1.
        // Other waves retain the existing red-only coverage requirement.
        let protected = red || w == 19 && white;
        self.early_row1_protected = protected;
        if combat::doom_position(self.preferred(w), false) != Some((4, 7)) {
            return;
        }
        if protected {
            self.early_alternative = combat::doom_position_except(self.preferred(w), false, Some((4, 7)));
            self.early = self.early_alternative.is_some();
        }
        let alternate = self.early_alternative.unwrap_or((0, 0));
        telemetry::record(
            "early_nuke_row1_guard",
            &[
                i64::from(red),
                alternate.0 as i64,
                alternate.1 as i64,
                i64::from(self.early),
                i64::from(white),
                i64::from(protected),
            ],
        );
    }
    fn preferred(&self, w: i32) -> Pos {
        if let Some(pos) = self.planner.retained_nuke(w) {
            return pos;
        }
        if let Some(pos) = self.planned_doom {
            return pos;
        }
        if self.early {
            if let Some(pos) = self.early_alternative {
                return pos;
            }
        }
        let preferred = Self::pit(self.doom_uses);
        // Share the coverage decision with the only-4-7 fallback above.
        if self.early && self.early_row1_protected && w != 20 && preferred == (4, 7) {
            (3, 8)
        } else {
            preferred
        }
    }
    fn doom_tick(&mut self, w: i32, t: i32) {
        if !self.need_nuke(w) {
            return;
        }
        // A planted doom is pending until the observed activation.
        let pending = rsvz::with_frame(|b| b.plants().any(|p| p.is_alive() && p.raw_kind() == DoomShroom));
        if pending {
            return;
        }
        let target = self.doom_due;
        if t >= target - 100 {
            if self.late_nuke(w) && Self::known_next_clock(w).is_some_and(|next| next <= rsvz::timing::clock() + 100) {
                return;
            }
            if w == 20 && combat::doom_position(self.preferred(w), false) == Some((4, 7)) {
                let now = rsvz::timing::clock();
                if now < self.next_w20_doom_check {
                    return;
                }
                self.planner.observe(now, self.last_ice);
                self.refresh_forecast(now, w, t);
                let at = prediction::w20_doom_coverage(&self.planner.scene, &self.forecast, 100);
                if at > 100 {
                    self.next_w20_doom_check = now + (at - 100).clamp(1, 20);
                    return;
                }
            }
            if combat::doom(self.preferred(w), false) {
                self.pending_doom_wave = if self.carried_doom > 0 { self.carried_doom } else { w };
            }
        }
    }
    fn jala_target(&self, w: i32) -> i32 {
        if w == 20 { W20_JALA_EFFECT } else { 2400 }
    }
    fn sync_shadow(&mut self, w: i32, t: i32) {
        if self.policy.enabled() || !policy::shadow_enabled() {
            return;
        }
        let now = rsvz::timing::clock();
        let mut fresh = self.planner.scene.clone();
        fresh.sample(now, self.last_ice);
        let old_scene = std::mem::replace(&mut self.planner.scene, fresh);
        let old_forecast = std::mem::take(&mut self.forecast);
        let old_cherry_plans = self.cherry_plans;
        let old_cherry_guard = self.planner.cherry_guard.clone();
        let old_pit_plan = (self.planned_doom, self.next_pit_check);
        let old_retention = self.planner.save_retention();
        self.refresh_forecast(now, w, t);
        policy::shadow_context(self.policy_context(now, w, t), &self.forecast, &self.planner.scene);
        self.cherry_plans = old_cherry_plans;
        self.planner.cherry_guard = old_cherry_guard;
        (self.planned_doom, self.next_pit_check) = old_pit_plan;
        self.planner.restore_retention(old_retention);
        self.forecast = old_forecast;
        self.planner.scene = old_scene;
    }
    fn shadow_skip(&mut self, kind: usize, w: i32, t: i32) {
        self.sync_shadow(w, t);
        policy::shadow_skip(kind);
    }
    fn ash_tick(&mut self, w: i32, t: i32, rows: &[combat::Row; 6]) {
        let _perf = crate::perf::scope("strategy::ash_tick");
        let aj = self.ash_wave(w);
        if aj && t >= if w == 20 { 400 } else { 2200 } && !self.cherry_done {
            let pressure = w == 20
                || rows[0].red
                || rows[1].red
                || rows[0].foot < 900.
                || rows[1].foot < 900.
                || rows[0].giant < 500.
                || rows[1].giant < 500.
                || rows[0].car < 900.
                || rows[1].car < 900.;
            if w != 20
                && !self.planner.cherry_damage_required()
                && pressure
                && self
                    .planner
                    .ash_unneeded(&self.forecast, prediction::EffectKind::Cherry, &[1, 2])
            {
                self.shadow_skip(8, w, t);
                self.cherry_done = true;
                self.planner.cherry_omitted(w);
                diagnostics::mark(C::AshSaved);
                telemetry::record("economy_cherry", &[w as i64]);
            } else if pressure {
                let pos = if !ready(sel(CherryBomb)) {
                    None
                } else {
                    self.forecast
                        .events
                        .iter()
                        .find(|e| !e.certain && e.kind == prediction::EffectKind::Cherry && e.at <= 100)
                        .map(|e| e.pos)
                };
                if let Some(pos) = pos {
                    let pos = self.planner.cherry_with_squash(&self.forecast).unwrap_or(pos);
                    self.cherry_done = place(sel(CherryBomb), pos, 4, true);
                    if self.cherry_done {
                        self.planner.register_cherry_misses(&self.forecast, pos);
                    }
                    if self.cherry_done && w != 20 {
                        self.planner.log_economy_rejection();
                    }
                }
            } else {
                self.shadow_skip(8, w, t);
                self.cherry_done = true;
                if w != 20 {
                    self.planner.cherry_omitted(w);
                }
                diagnostics::mark(C::AshSaved);
            }
        }
        self.sync_shadow(w, t);
        // Cherry can be omitted, or a free Puff can be planted to replace its
        // football duty. The pepper decision must see that actual new plan.
        if aj && self.cherry_done && !self.jala_done && t >= if w == 20 { self.jala_target(w) - 100 } else { 2120 } {
            self.planner.observe(rsvz::timing::clock(), self.last_ice);
            self.refresh_forecast(rsvz::timing::clock(), w, t);
        }
        // Squash lands about 182cs later, with another >=200cs until a normal
        // refresh. Starting at 2120 cannot itself shorten this wave below 2500.
        if self.take_economy_check(rsvz::timing::clock(), w, t) && self.planner.economy_jalapeno(&self.forecast) {
            self.shadow_skip(9, w, t);
            self.jala_done = true;
            diagnostics::mark(C::AshSaved);
            telemetry::record("economy_jalapeno", &[w as i64]);
        }
        self.sync_shadow(w, t);
        if aj && t >= self.jala_target(w) - 100 && !self.jala_done {
            let r = rows[5];
            if w == 20 || r.red || r.foot < 900. || r.giant < 900. || r.car < 900. || r.imp < 900. {
                if w != 20 && self.planner.economy_jalapeno(&self.forecast) {
                    self.shadow_skip(9, w, t);
                    self.jala_done = true;
                    diagnostics::mark(C::AshSaved);
                    telemetry::record("economy_jalapeno", &[w as i64]);
                    return;
                }
                let used = self.planner.jalapeno_pad(&self.forecast);
                if used || combat::jalapeno(6, 5) {
                    if w != 20 {
                        self.planner.log_economy_rejection();
                    }
                    self.jala_done = true;
                    self.last_jala_wave = w;
                }
            } else {
                self.shadow_skip(9, w, t);
                self.jala_done = true;
                diagnostics::mark(C::AshSaved);
            }
        }
        self.sync_shadow(w, t);
    }
    fn clear_sixth_cars(&mut self, now: i32, w: i32, t: i32) {
        let outstanding = self.planner.scene.row_zombies(6).any(|z| {
            z.kind == Z::Zomboni
                && (z.carried
                    || z.wave < w
                    || w > 0 && self.nuke_done[w as usize]
                    || self.jala_done && self.last_jala_effect >= self.wave_start)
        });
        if !outstanding || self.planner.sixth_clearance_pending() {
            return;
        }
        // Next-wave water nukes cannot serve as a promise to clear these left-side cars.
        // Keep the actual visible obligation across wave changes, with no natural-death waiver.
        let squash = self.planner.economy_jalapeno(&self.forecast);
        let pepper = !squash && combat::jalapeno(6, 24);
        if pepper || squash {
            self.jala_done = true;
            if pepper {
                self.last_jala_wave = w;
            }
            telemetry::record("sixth_car_clearance", &[w as i64, i64::from(pepper)]);
            self.planner.observe(now, self.last_ice);
            self.refresh_forecast(now, w, t);
        }
    }
    fn check_squash_result(&mut self, w: i32) {
        if let Some((due, row, from_wave, economy)) = self.squash_check {
            if rsvz::timing::clock() >= due {
                self.squash_check = None;
                self.planner.observe(rsvz::timing::clock(), self.last_ice);
                let missed = if economy {
                    self.planner.economy_squash_missed()
                } else {
                    rsvz::with_frame(|b| {
                        b.zombies().any(|z| {
                            z.is_alive()
                                && z.has_head()
                                && z.row() + 1 == row
                                && z.from_wave() < from_wave
                                && matches!(z.kind(), Z::Football | Z::Zomboni)
                                && z.x() < 600.
                        })
                    })
                };
                if missed {
                    if row == 6 {
                        self.jala_done = false;
                    }
                    diagnostics::mark(C::SquashMiss);
                    if combat::jalapeno(row, 9) {
                        if row == 6 {
                            self.jala_done = true;
                        }
                        self.last_jala_wave = w;
                        diagnostics::mark(C::EmergencyAsh);
                    }
                }
            }
        }
    }
    fn reserve_ice(f: &mut prediction::Forecast, available: &mut [i32; 2], target: i32) -> i32 {
        let index = usize::from(available[1] < available[0]);
        let at = target.max(available[index]).max(1);
        f.add(at, prediction::EffectKind::Ice, (5, 1), false);
        available[index] = at + 5001;
        at
    }
    fn delay_cherry(f: &mut prediction::Forecast, index: usize, at: i32) {
        let mut available = at;
        for event in f
            .events
            .iter_mut()
            .skip(index)
            .filter(|e| e.kind == prediction::EffectKind::Cherry && !e.certain)
        {
            event.at = event.at.max(available);
            available = event.at + 5001;
        }
    }
    fn refresh_forecast(&mut self, now: i32, w: i32, t: i32) {
        let _perf = crate::perf::scope("strategy::refresh_forecast");
        self.update_tail_ice(now, w, t, false);
        use prediction::EffectKind as E;
        let need_nuke = self.need_nuke(w);
        let ash_wave = self.ash_wave(w);
        let pref = self.preferred(w);
        let mut doom_pos = combat::doom_position(pref, false).unwrap_or(pref);
        let jala_target = self.jala_target(w);
        let mut ice_available = [cd(sel(IceShroom)) + 100, cd(COPY) + 420];
        let mut doom_available = cd(sel(DoomShroom)) + 100;
        if hp(LilyPad, doom_pos).is_none() {
            doom_available = doom_available.max(cd(sel(LilyPad)) + 100);
        }
        let mut cherry_available = cd(sel(CherryBomb)) + 100;
        let mut jala_available = cd(sel(Jalapeno)) + 100;
        let tail_allowed = self.tail_ice_allowed(now, w, ice_available[0].max(ice_available[1]));
        let next = w + 1;
        let mut next_nuke = self.nuke_wave(next);
        let mut next_ash = self.ash_wave(next);
        let next_late = self.late_nuke(next);
        let length = Self::known_next_clock(w).map_or_else(
            || {
                if matches!(w, 9 | 19) {
                    5245
                } else if need_nuke {
                    let effect = self
                        .planner
                        .scene
                        .plants
                        .iter()
                        .find(|p| p.kind == DoomShroom)
                        .map_or((self.doom_due - t).max(doom_available), |p| p.effect.max(1));
                    t + effect + 200
                } else {
                    3099
                }
            },
            |clock| clock - self.wave_start,
        );
        let next_start = (length - t).max(1);
        let crosses_tail = self.late_nuke(w)
            && need_nuke
            && Self::known_next_clock(w).is_some()
            && (self.doom_due - t).max(doom_available) >= next_start;
        if crosses_tail && w == 9 {
            next_nuke = true;
            next_ash = false;
        }
        let f = &mut self.forecast;
        f.events.clear();
        let mut pending = [false; 4];
        for p in &self.planner.scene.plants {
            let (kind, index, lead) = match p.kind {
                IceShroom => (E::Ice, 0, p.effect.max(1)),
                Imitator => (E::Ice, 0, (self.pending_ice - now).max(1)),
                DoomShroom => (E::Doom, 1, p.effect.max(1)),
                CherryBomb => (E::Cherry, 2, p.effect.max(1)),
                Jalapeno => (E::Jalapeno, 3, p.effect.max(1)),
                _ => continue,
            };
            f.add(lead, kind, p.pos, true);
            pending[index] = true;
        }
        let mut current_first_ice = self.first_ice[w as usize].max(self.ice_due);
        if self.ice_count[w as usize] == 0 && !pending[0] {
            current_first_ice = t + Self::reserve_ice(f, &mut ice_available, self.ice_due - t);
        }
        if matches!(w, 9 | 19 | 20) && self.ice_count[w as usize] < 2 && tail_allowed {
            let target = if w == 20 { 3256 } else { self.tail_ice_due };
            if target - t <= prediction::HORIZON {
                Self::reserve_ice(f, &mut ice_available, target - t);
            }
        }
        let mut next_pit = self.doom_uses;
        let mut current_nuke = None;
        if need_nuke && !pending[1] {
            let mut at = (self.doom_due - t).max(doom_available).max(1);
            if crosses_tail {
                at = at.max(next_start + 300);
            }
            f.add(at, E::Doom, doom_pos, false);
            current_nuke = Some(now + at);
            doom_available = at + 5001;
            next_pit += 1;
        } else if pending[1] {
            next_pit += 1;
        }
        if ash_wave && !self.cherry_done && !pending[2] {
            let at = ((if w == 20 { 500 } else { 2300 }) - t).max(cherry_available).max(1);
            f.add(at, E::Cherry, (2, 9), false);
            cherry_available = at + 5001;
        }
        if ash_wave && !self.jala_done && !pending[3] {
            let at = (jala_target - t).max(jala_available).max(1);
            f.add(at, E::Jalapeno, (6, 2), false);
            jala_available = at + 5001;
        }
        if w < 20 {
            let early_due = early_nuke_due(doom_available - next_start, length, current_first_ice);
            let next_early = next_nuke && !next_late && !matches!(next, 10 | 20) && length > 2969 && early_due <= 1831;
            let next_ice = if next_early {
                100
            } else if matches!(next, 10 | 20) {
                970
            } else if next_nuke && !next_late {
                783
            } else {
                830
            };
            let _next_ice_at = Self::reserve_ice(f, &mut ice_available, next_start + next_ice);
            if next_nuke && !crosses_tail {
                let target = if next_late {
                    4801
                } else if next == 20 {
                    1700
                } else if next_early {
                    early_due
                } else {
                    2301
                };
                let pos = if next_early && Self::pit(next_pit) == (4, 7) {
                    (3, 8)
                } else {
                    Self::pit(next_pit)
                };
                f.add((next_start + target).max(doom_available), E::Doom, pos, false);
            }
            if next_ash {
                f.add(
                    (next_start + if next == 20 { 500 } else { 2300 }).max(cherry_available),
                    E::Cherry,
                    if next == 20 { (2, 9) } else { (2, 4) },
                    false,
                );
                f.add(
                    (if next == 20 {
                        next_start + W20_JALA_EFFECT
                    } else {
                        next_start + 2400
                    })
                    .max(jala_available),
                    E::Jalapeno,
                    (6, 2),
                    false,
                );
            }
        }
        if w == 20 {
            let first = (6600 - t).max(1);
            Self::reserve_ice(f, &mut ice_available, first + 830);
            f.add((first + 2300).max(cherry_available), E::Cherry, (2, 4), false);
            f.add((first + 2400).max(jala_available), E::Jalapeno, (6, 2), false);
        }
        f.events.sort_by_key(|e| e.at);
        self.planner.set_current_nuke(w, current_nuke);
        self.planner.add_retention_body(w, f);
        self.planner.add_squash_forecast(f);
        f.events.sort_by_key(|e| e.at);
        prediction::resolve_live_ice(&self.planner.scene, f);
        self.resolve_cherries(now, w, next_start);
        if need_nuke && !pending[1] {
            if let Some(index) = self
                .forecast
                .events
                .iter()
                .position(|e| e.kind == E::Doom && !e.certain)
            {
                let at = self.forecast.events[index].at;
                if now >= self.next_pit_check || self.planned_doom != Some(doom_pos) {
                    doom_pos = self.planner.nuke_position(
                        &self.forecast,
                        at,
                        doom_pos,
                        self.early && self.early_row1_protected
                            || self.normal_ice_advanced && self.ice_due < 730 && self.current_row_one_red(w),
                    );
                    self.planned_doom = Some(doom_pos);
                    self.next_pit_check = now + 500;
                }
                self.forecast.events[index].pos = doom_pos;
                self.forecast.events[index].shape = Some(rsvz::core::contact::grid_explosion_shape(
                    rsvz::core::model::GridExplosionKind::DoomShroom,
                    grid(doom_pos),
                ));
            }
        }
        if w == 20 && doom_pos == (4, 7) && !pending[1] {
            if let Some(index) = self
                .forecast
                .events
                .iter()
                .position(|e| e.kind == E::Doom && !e.certain)
            {
                let at =
                    prediction::w20_doom_coverage(&self.planner.scene, &self.forecast, self.forecast.events[index].at);
                self.forecast.events[index].at = at;
                self.forecast.events.sort_by_key(|e| e.at);
            }
        }
    }
    fn resolve_cherries(&mut self, now: i32, w: i32, next_start: i32) {
        use prediction::EffectKind as E;
        let next = w + 1;
        // Resolve every unplanted Cherry through the same selector, at its own
        // activation time. Execution consumes this exact current-wave event.
        let mut index = 0;
        let mut planned = 0;
        let mut plans = [None; 3];
        while index < self.forecast.events.len() {
            let e = self.forecast.events[index];
            if e.kind == E::Cherry && !e.certain {
                let flag = if e.at < next_start { w == 20 } else { next == 20 };
                let effect_clock = now + e.at;
                let serial = telemetry::serial();
                // A future planting location is a revisable plan, not a cached
                // safety verdict. Recompute on operations and at the planning interval; near
                // planting always resolve against the current observation.
                let cached = self
                    .cherry_plans
                    .iter()
                    .flatten()
                    .find(|p| {
                        p.effect_clock == effect_clock
                            && p.flag == flag
                            && p.serial == serial
                            && (p.checked_at == now || e.at > 100 && now - p.checked_at < self.cherry_period)
                            && self.planner.cherry_site_available(p.pos, e.at)
                    })
                    .copied();
                let mut plan = cached.or_else(|| {
                    self.planner
                        .cherry_position(&self.forecast, e.at, flag)
                        .map(|pos| CherryPlan {
                            effect_clock,
                            checked_at: now,
                            serial,
                            flag,
                            pos,
                        })
                });
                if let Some((recovered, pos)) =
                    self.planner
                        .cherry_recovery_plan(&self.forecast, e.at, flag, plan.map(|p| p.pos))
                {
                    Self::delay_cherry(&mut self.forecast, index, recovered);
                    plan = Some(CherryPlan {
                        effect_clock: now + recovered,
                        checked_at: now,
                        serial,
                        flag,
                        pos,
                    });
                }
                if let Some(plan) = plan {
                    let pos = plan.pos;
                    if planned < plans.len() {
                        plans[planned] = Some(plan);
                        planned += 1;
                    }
                    self.forecast.events[index].pos = pos;
                    self.forecast.events[index].shape = Some(rsvz::core::contact::grid_explosion_shape(
                        rsvz::core::model::GridExplosionKind::CherryBomb,
                        grid(pos),
                    ));
                } else {
                    self.forecast.events.remove(index);
                    continue;
                }
            }
            index += 1;
        }
        self.cherry_plans = plans;
        self.forecast.events.sort_by_key(|e| e.at);
    }
    fn visible_plan(&self, w: i32, t: i32) -> Option<policy::plan::Plan> {
        if !self.policy.needs_plan() {
            return None;
        }
        let mut x = [0.; policy::plan::WIDTH];
        let ash = w > 0 && self.ash_wave(w);
        let pos = self.preferred(w);
        x[0] = f32::from(w > 0 && self.need_nuke(w));
        x[1] = f32::from(ash);
        x[2] = f32::from(self.early);
        x[3] = f32::from(self.even_after_nine);
        x[4] = self.carried_doom as f32 / 20.;
        x[5] = cd(sel(DoomShroom)) as f32 / 5000.;
        x[6] = pos.0 as f32 / 6.;
        x[7] = pos.1 as f32 / 9.;
        if ash && !self.cherry_done {
            x[28] = 1.;
            x[29] = ((if w == 20 { 400 } else { 2200 }) - t) as f32 / 6000.;
        }
        if ash && !self.jala_done {
            x[30] = 1.;
            x[31] = (self.jala_target(w) - 100 - t) as f32 / 6000.;
        }
        if w == 0 {
            let mut opening = prediction::Forecast::default();
            Self::fill_opening_forecast(&mut opening, (-t).max(0));
            Some(policy::plan::forecast(x, &opening))
        } else {
            Some(policy::plan::forecast(x, &self.forecast))
        }
    }
    fn policy_context(&self, now: i32, w: i32, t: i32) -> policy::Context {
        let count = self.ice_count[w as usize];
        let tail = matches!(w, 9 | 19 | 20) && count == 1;
        let target = self.ice_target(w, t, tail);
        let ice_request = w > 0
            && self.pending_ice < 0
            && (count == 0 || tail)
            && (!tail || self.tail_ice_allowed(now, w, (cd(sel(IceShroom)) + 100).max(cd(COPY) + 420)));
        let (ice_allowed, ice_required) = policy::ice_window(ice_request, target, t, [cd(sel(IceShroom)), cd(COPY)]);
        policy::Context {
            wave: w,
            time: t,
            ice_due: target,
            doom_due: self.doom_due,
            ice_allowed,
            ice_required,
            // First trial window: all times from observation to the end of every wave.
            // No pressure, safety, row or economy heuristic removes an action.
            cherry: w > 0 && !self.cherry_done,
            jala: w > 0 && !self.jala_done,
            cherry_done: self.cherry_done,
            jala_done: self.jala_done,
            pending_ice: self.pending_ice,
            last_ice: self.last_ice,
            last_blover_wave: self.planner.last_blover_wave,
            round: self.policy.round,
            plan: self.visible_plan(w, t),
        }
    }
    fn policy_tick(&mut self, now: i32, w: i32, t: i32) {
        if self.pending_ice >= 0 && now > self.pending_ice + 10 {
            diagnostics::mark(C::IceLost);
            self.pending_ice = -1;
            self.ice_count[self.ice_effect_wave as usize] =
                self.ice_count[self.ice_effect_wave as usize].saturating_sub(1);
        }
        for _ in 0..8 {
            if !self.policy.enabled() {
                break;
            }
            let mut c = self.policy_context(now, w, t);
            if w > 0 {
                self.refresh_forecast(now, w, t);
            } else {
                self.forecast.events.clear();
            }
            c.plan = self.visible_plan(w, t); // Snapshot full plan before the unchanged Lite damage filter.
            // Learning owns optional ash; unchosen actions are never promised as damage.
            self.forecast.events.retain(|e| {
                e.certain
                    || !matches!(
                        e.kind,
                        prediction::EffectKind::Cherry | prediction::EffectKind::Jalapeno
                    )
            });
            let action = self.policy.choose(&mut self.planner, &self.forecast, &c);
            let success = match action.kind {
                0 => true,
                8 => {
                    self.cherry_done = true;
                    true
                }
                9 => {
                    self.jala_done = true;
                    true
                }
                _ => place(action.card().unwrap(), action.pos, 30 + action.kind as i32, true),
            };
            self.policy.executed(action, success);
            if action.kind == 0 || !success {
                break;
            }
            match action.kind {
                2 => self.planner.last_blover_wave = w,
                4 => self.cherry_done = true,
                5 => {
                    self.jala_done = true;
                    self.last_jala_wave = w;
                }
                6 => self.ice_planted(w, now, 100),
                7 => self.ice_planted(w, now, 420),
                _ => {}
            }
            self.planner.observe(now, self.last_ice);
        }
    }
    fn economy_check_due(&self, now: i32, w: i32, t: i32) -> bool {
        !self.policy.enabled()
            && w > 0
            && w != 20
            && self.ash_wave(w)
            && !self.jala_done
            && (2120..2300).contains(&t)
            && now >= self.next_economy_check
    }
    fn early_cherry_allowed(&self, w: i32, t: i32) -> bool {
        let scheduled_plant = if w == 20 { 400 } else { 2200 };
        w > 0 && self.ash_wave(w) && !self.cherry_done && (scheduled_plant - 200..scheduled_plant).contains(&t)
    }
    fn timed_ash_due(&self, now: i32, w: i32, t: i32, cherry_ready: bool, jala_ready: bool) -> bool {
        if !self.ash_wave(w) {
            return false;
        }
        let cherry_at = if w == 20 { 400 } else { 2200 };
        let jala_at = self.jala_target(w) - 100;
        (!self.cherry_done && t >= cherry_at && cherry_ready && (t == cherry_at || now >= self.next_ash_retry))
            || (!self.jala_done && t >= jala_at && jala_ready && (t == jala_at || now >= self.next_ash_retry))
    }
    fn take_economy_check(&mut self, now: i32, w: i32, t: i32) -> bool {
        if !self.economy_check_due(now, w, t) {
            return false;
        }
        self.next_economy_check = now + 25;
        true
    }
    pub fn tick(&mut self) {
        if !self.stack_prepared {
            self.stack_prepared = true;
            match stack_layout::prepare() {
                Ok(true) => return,
                Ok(false) => {}
                Err(e) => {
                    rsvz::fail_script(e);
                    return;
                }
            }
        }
        let _perf = crate::perf::scope("strategy::tick");
        let (now, w, t) = time();
        let period = if self.policy.enabled() { 5 } else { self.decision_period };
        if let Some(active) = self.policy.window_transition(w) {
            let (start, end) = self.policy.wave_window.unwrap();
            rsvz::log(
                rsvz::LogLevel::Info,
                format_args!(
                    "six_policy_window seed={} clock={} round={} wave={} window={}:{} first_round_only={} active={} decision={} state_reset=false last_ice={} pending_ice={} doom_uses={} last_blover_wave={} cherry_done={} jala_done={} sun={} cds={:?}",
                    diagnostics::seed(),
                    now,
                    self.policy.round,
                    w,
                    start,
                    end,
                    self.policy.window_first_round_only,
                    active,
                    self.policy.decision,
                    self.last_ice,
                    self.pending_ice,
                    self.doom_uses,
                    self.planner.last_blover_wave,
                    self.cherry_done,
                    self.jala_done,
                    rsvz::board::sun(),
                    [
                        cd(sel(PuffShroom)),
                        cd(sel(Blover)),
                        cd(sel(Squash)),
                        cd(sel(CherryBomb)),
                        cd(sel(Jalapeno)),
                        cd(sel(IceShroom)),
                        cd(COPY)
                    ]
                ),
            );
        }
        if w == 0 {
            self.opening_tick(now);
            return;
        }
        if w != self.wave {
            self.enter(now, w, t);
        }
        // Cheap readiness/deadline checks run every tick. Replan only on an
        // actual due action; unsuccessful expensive searches retain a backoff.
        let ash_due =
            !self.policy.enabled() && self.timed_ash_due(now, w, t, ready(sel(CherryBomb)), ready(sel(Jalapeno)));
        let decision_due = now % period == 0 || self.economy_check_due(now, w, t) || ash_due;
        if decision_due {
            self.planner.observe(now, self.last_ice);
            if !self.ice_floor_sampled && t >= 5 {
                self.ice_floor_sampled = true;
                if !self.early {
                    self.sample_first_ice_floor(now, w, t);
                }
            }
            self.refresh_forecast(now, w, t);
        }
        if !self.policy.enabled() {
            self.sync_shadow(w, t);
        }
        let before_operations = telemetry::serial();
        let before_plan = (self.cherry_done, self.jala_done);
        if self.policy.enabled() {
            self.doom_tick(w, t);
            let count = self.ice_count[w as usize];
            let target = self.ice_target(w, t, matches!(w, 9 | 19 | 20) && count == 1);
            let exact_ice = self.pending_ice < 0
                && (count == 0 || matches!(w, 9 | 19 | 20) && count == 1)
                && ((ready(sel(IceShroom)) && t + 100 == target) || (ready(COPY) && t + 420 == target));
            let ice_context = self.policy_context(now, w, t);
            let fixed_ice_wakeup = now % 5 == 0
                && ((self.pending_ice >= 0 && now > self.pending_ice + 10)
                    || ice_context.ice_required
                        && (ice_context.ice_allowed[0] && ready(sel(IceShroom))
                            || ice_context.ice_allowed[1] && ready(COPY)));
            if now % setting("SIX_POLICY_PERIOD", 5).max(1) as i32 == 0 || exact_ice || fixed_ice_wakeup {
                self.planner.observe(now, self.last_ice);
                combat::maintenance();
                diagnostics::balance(rsvz::board::sun());
                self.policy_tick(now, w, t);
            }
            return;
        }
        // Ordinary decisions use the configured cadence; actual ice/nuke timing stays at 1cs.
        self.ice_tick(now, w, t);
        self.doom_tick(w, t);
        combat::maintenance();
        self.check_squash_result(w);
        // A cheap visible-state gate wakes a row-specific prediction only near
        // the home stretch. Never pad every W20 Football merely for existing.
        if w == 20
            && now >= self.next_w20_guard
            && ready(sel(PuffShroom))
            && rsvz::with_frame(|b| {
                b.zombies()
                    .any(|z| z.is_alive() && z.has_head() && z.kind() == Z::Football && z.row() == 5 && z.x() < 200.)
            })
        {
            self.next_w20_guard = now + 5;
            self.planner.observe(now, self.last_ice);
            self.refresh_forecast(now, w, t);
            if self.planner.guard_w20_football(&self.forecast) {
                self.planner.observe(now, self.last_ice);
                self.refresh_forecast(now, w, t);
            }
        }
        if !decision_due {
            if self.planner.blover_contact_due(w, t, &self.forecast) {
                self.planner.observe(now, self.last_ice);
                self.refresh_forecast(now, w, t);
                self.planner.blover(w, t, &self.forecast);
            }
            self.sync_shadow(w, t);
            policy::shadow_wait();
            return;
        }
        let rows = combat::scan(&self.planner.scene, w);
        if !rows.iter().any(|r| r.red) {
            diagnostics::mark(C::RedAbsent);
        }
        if rows[4].old_foot < 900. {
            diagnostics::mark(C::OldFootball5);
        }
        if w == 20 && rows.iter().any(|r| r.giant < 500. || r.old_foot < 700.) {
            diagnostics::mark(C::TailResidual);
        }
        let sun = rsvz::board::sun();
        diagnostics::balance(sun);
        if sun < 150 {
            diagnostics::mark(C::LowSun);
        }
        self.clear_sixth_cars(now, w, t);
        self.planner.refresh_imp_reservation(&self.forecast);
        if w != 20 {
            self.planner.required_squash(w, self.ash_births, &self.forecast);
        }
        self.sync_shadow(w, t);
        self.ash_tick(w, t, &rows);
        self.next_ash_retry = if (!self.cherry_done && t >= if w == 20 { 400 } else { 2200 } && ready(sel(CherryBomb)))
            || (!self.jala_done && t >= self.jala_target(w) - 100 && ready(sel(Jalapeno)))
        {
            now + period
        } else {
            now // A card becoming ready must wake immediately, not at the next grid tick.
        };
        self.sync_shadow(w, t);
        if telemetry::serial() != before_operations {
            self.planner.observe(now, self.last_ice);
        }
        if telemetry::serial() != before_operations || before_plan != (self.cherry_done, self.jala_done) {
            self.refresh_forecast(now, w, t);
        }
        if (w == 20 || self.planner.has_carried_tail()) && self.planner.tail_group(w, &self.forecast) {
            self.planner.observe(now, self.last_ice);
            self.refresh_forecast(now, w, t);
        }
        self.sync_shadow(w, t);
        let before_pads = telemetry::serial();
        self.planner.blover(w, t, &self.forecast);
        if telemetry::serial() != before_pads {
            self.planner.observe(now, self.last_ice);
        }
        self.sync_shadow(w, t);
        if self.planner.support_squash(w, &self.forecast) {
            self.planner.observe(now, self.last_ice);
            self.refresh_forecast(now, w, t);
        }
        self.sync_shadow(w, t);
        self.planner.pads(w, t, &self.forecast);
        if self.planner.retention_started(now) {
            self.planner.observe(now, self.last_ice);
            self.refresh_forecast(now, w, t);
        }
        if matches!(w, 9..=12 | 19 | 20) && ready(sel(PuffShroom)) {
            if telemetry::serial() != before_pads {
                self.planner.observe(now, self.last_ice);
            }
            self.planner.carry_pad(w, &self.forecast);
        }
        let early_cherry = self.early_cherry_allowed(w, t);
        let urgent = self.planner.emergency(&self.forecast, early_cherry);
        for row in [1, 2, 5, 6] {
            if !urgent[(row - 1) as usize] {
                continue;
            }
            self.sync_shadow(w, t);
            if early_cherry && !self.cherry_done && self.planner.advance_cherry(row, &self.forecast) {
                diagnostics::mark(C::EmergencyAsh);
                self.cherry_done = true;
                telemetry::record("cherry_advanced", &[w as i64, t as i64]);
                self.planner.observe(now, self.last_ice);
                self.refresh_forecast(now, w, t);
            } else if self.planner.rescue_squash(row, &self.forecast) {
                diagnostics::mark(C::EmergencyAsh);
            }
        }
        // Optional traffic separation is the last Puff consumer, after rescue.
        if ready(sel(PuffShroom)) {
            if telemetry::serial() != before_pads {
                self.planner.observe(now, self.last_ice);
            }
            self.planner.bait_catapult(&self.forecast);
        }
        self.sync_shadow(w, t);
        policy::shadow_wait();
    }
}

#[cfg(test)]
mod plan_tests {
    #[test]
    fn deferred_cherry_also_defers_its_next_card_availability() {
        use crate::prediction::{EffectKind, Forecast};
        let mut f = Forecast::default();
        // Only scheduling is tested; no backend-dependent explosion geometry.
        f.add(100, EffectKind::Ice, (2, 7), false);
        f.add(300, EffectKind::Ice, (5, 1), false);
        f.add(5101, EffectKind::Ice, (2, 4), false);
        f.events[0].kind = EffectKind::Cherry;
        f.events[2].kind = EffectKind::Cherry;
        super::Control::delay_cherry(&mut f, 0, 175);
        assert_eq!(f.events.iter().map(|e| e.at).collect::<Vec<_>>(), vec![175, 300, 5176]);
    }
    use super::*;
    #[test]
    fn normal_ice_advance_keeps_track_roles_separate() {
        let mut c = Control::default();
        for w in [2, 4, 6, 8, 11, 13, 15, 17, 19] {
            assert!(c.normal_nuke_first_ice(w));
        }
        for w in [1, 3, 9, 10, 12, 20] {
            assert!(!c.normal_nuke_first_ice(w));
        }
        c.even_after_nine = true;
        assert!(c.normal_nuke_first_ice(12));
        assert!(!c.normal_nuke_first_ice(19));
        c.early = true;
        assert!(!c.normal_nuke_first_ice(12));
        c.early = false;
        c.carried_doom = 9;
        assert!(!c.normal_nuke_first_ice(12));
    }
    #[test]
    fn early_nuke_budget_is_shared_and_keeps_deadline() {
        assert_eq!(early_nuke_due(1520, 3037, 970), 1784);
        assert_eq!(2934 - (1784 + 200), 950);
        assert_eq!(2934 - (1831 + 200), 903);
        assert!(early_nuke_due(1832, 3037, 970) > 1831);
    }
    #[test]
    fn early_nuke_guard_protects_w19_whites_and_current_reds() {
        use rsvz::core::backend::*;
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        for (wave, kind, row, current_wave, other_site, expected_early) in [
            (2, Z::Gargantuar, 1, true, false, true),
            (9, Z::Gargantuar, 1, true, false, true),
            (19, Z::Gargantuar, 1, true, false, false),
            (19, Z::Gargantuar, 1, true, true, true),
            (19, Z::Gargantuar, 2, true, false, true),
            (19, Z::Gargantuar, 1, false, false, true),
            (19, Z::Normal, 1, true, false, true),
            (2, Z::GigaGargantuar, 2, true, false, true),
            (2, Z::GigaGargantuar, 1, false, false, true),
            (2, Z::GigaGargantuar, 1, true, false, false),
            (19, Z::GigaGargantuar, 1, true, false, false),
            (2, Z::GigaGargantuar, 1, true, true, true),
        ] {
            telemetry::clear();
            let mut world = PeWorldOwner::new_reset(PeWorldConfig {
                scene: pe_rs::SceneType::Fog,
                initial_sun: 8000,
                ..Default::default()
            })
            .unwrap()
            .install_current()
            .unwrap();
            let wave = world.with_backend(|b| {
                b.set_zombie_spawn_stopped(true).unwrap();
                b.select_card(sel(DoomShroom).checked().unwrap()).unwrap();
                b.select_card(sel(LilyPad).checked().unwrap()).unwrap();
                b.finish_card_selection().unwrap();
                for pos in [(3, 7), (4, 7), (3, 8), (4, 8), (3, 9), (4, 9)] {
                    b.new_plant(sel(LilyPad).checked().unwrap(), grid(pos)).unwrap();
                    if pos != (4, 7) && !(other_site && pos == (4, 8)) {
                        b.new_plant(sel(WallNut).checked().unwrap(), grid(pos)).unwrap();
                    }
                }
                b.add_zombie_in_row(kind, row - 1, wave - 1 - i32::from(!current_wave))
                    .unwrap()
                    .expect("guard test zombie");
                wave
            });
            world.with_backend(|b| {
                PeBackend::scope_backend(b, || {
                    let mut c = Control::default();
                    c.early = true;
                    c.guard_early_nuke(wave);
                    assert_eq!(
                        c.early, expected_early,
                        "{kind:?} row={row} current={current_wave} alternative={other_site}"
                    );
                    if other_site {
                        assert_eq!(c.preferred(wave), (4, 8));
                    }
                    c.next_round();
                    assert_eq!(c.early_alternative, None);
                })
            });
        }
    }

    #[test]
    fn early_preferred_47_matches_red_and_w19_white_guard() {
        use rsvz::core::backend::*;
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        for (wave, kind, row, expected) in [
            (2, Z::Gargantuar, 1, (4, 7)),
            (9, Z::Gargantuar, 1, (4, 7)),
            (19, Z::Gargantuar, 1, (3, 8)),
            (19, Z::Gargantuar, 2, (4, 7)),
            (2, Z::GigaGargantuar, 2, (4, 7)),
            (2, Z::GigaGargantuar, 1, (3, 8)),
            (19, Z::GigaGargantuar, 1, (3, 8)),
        ] {
            telemetry::clear();
            let mut world = PeWorldOwner::new_reset(PeWorldConfig {
                scene: pe_rs::SceneType::Fog,
                initial_sun: 8000,
                ..Default::default()
            })
            .unwrap()
            .install_current()
            .unwrap();
            let wave = world.with_backend(|b| {
                b.set_zombie_spawn_stopped(true).unwrap();
                b.select_card(sel(DoomShroom).checked().unwrap()).unwrap();
                b.select_card(sel(LilyPad).checked().unwrap()).unwrap();
                b.finish_card_selection().unwrap();
                b.add_zombie_in_row(kind, row - 1, wave - 1)
                    .unwrap()
                    .expect("preference test zombie");
                wave
            });
            world.with_backend(|b| {
                PeBackend::scope_backend(b, || {
                    let mut c = Control::default();
                    c.early = true;
                    c.doom_uses = 1;
                    c.guard_early_nuke(wave);
                    assert!(c.early);
                    assert_eq!(c.preferred(wave), expected);
                    assert_eq!(combat::doom_position(c.preferred(wave), false), Some(expected));
                    c.guard_early_nuke(wave + 1);
                    assert_eq!(
                        c.preferred(wave + 1),
                        (4, 7),
                        "previous-wave red must not force redirection"
                    );
                    c.next_round();
                    assert!(!c.early_row1_protected);
                })
            });
        }
    }

    #[test]
    fn tail_position_target_changes_only_w9_w19_second_ice() {
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
            .unwrap()
            .install_current()
            .unwrap();
        world.with_backend(|backend| {
            PeBackend::scope_backend(backend, || {
                let mut c = Control::default();
                c.ice_due = 830;
                c.tail_ice_due = 3400;
                assert_eq!(c.ice_target(9, 0, true), 3400);
                assert_eq!(c.ice_target(19, 0, true), 3400);
                assert_eq!(c.ice_target(20, 0, true), 3256);
                assert_eq!(c.ice_target(9, 0, false), 830);
                assert_eq!(c.ice_target(19, 0, false), 830);
                c.ice_count[19] = 1;
                c.last_ice = 1000;
                c.wave_start = 500;
                c.update_tail_ice(1100, 19, 600, true);
                assert_eq!(c.tail_ice_due, 2500);
            })
        });
    }
    #[test]
    fn hypothetical_next_wave_does_not_block_the_w19_second_ice() {
        // First Ice at783 gives ordinary Ice's next activation at5784.
        // Unknown refresh no longer invents a3540 start for W20.
        assert!(Control::fits_known_next_ice(2600, None, 3184));
        assert!(Control::fits_known_next_ice(2600, Some(5245), 3184));
        assert!(!Control::fits_known_next_ice(2600, Some(3540), 3184));
        assert!(Control::fits_known_next_ice(2600, Some(3540), 1910));
        assert!(!Control::fits_known_next_ice(2600, Some(3540), 1911));
    }
    #[test]
    fn early_cherry_is_only_one_current_wave_use_within_two_seconds() {
        let mut c = Control::default();
        assert!(!c.early_cherry_allowed(0, 2100));
        assert!(!c.early_cherry_allowed(2, 2100));
        for w in [1, 3, 9, 20] {
            let due = if w == 20 { 400 } else { 2200 };
            assert!(!c.early_cherry_allowed(w, due - 201));
            assert!(c.early_cherry_allowed(w, due - 200));
            assert!(c.early_cherry_allowed(w, due - 1));
            assert!(!c.early_cherry_allowed(w, due));
            assert!(!c.early_cherry_allowed(w, due + 1));
        }
        c.cherry_done = true;
        assert!(!c.early_cherry_allowed(3, 2100));
    }
    #[test]
    fn missing_planted_ice_does_not_refund_the_scheduled_use() {
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
            .unwrap()
            .install_current()
            .unwrap();
        world.with_backend(|backend| {
            PeBackend::scope_backend(backend, || {
                let mut c = Control::default();
                c.pending_ice = 1000;
                c.ice_effect_wave = 3;
                c.ice_count[3] = 1;
                c.ice_tick(1011, 3, 500);
                assert_eq!(c.pending_ice, -1);
                assert_eq!(c.ice_count[3], 1);
                assert_eq!(c.first_ice[3], -1);
                assert_eq!(c.ice_target(9, 500, true), 2600);
            })
        });
    }
    #[test]
    fn timed_ash_deadlines_ignore_global_clock_phase() {
        for phase in 0..20 {
            let mut c = Control::default();
            // Retry backoff must never suppress another card's exact deadline.
            c.next_ash_retry = 10000;
            assert!(c.timed_ash_due(1000 + phase, 20, 233, false, true));
            assert!(!c.timed_ash_due(1000 + phase, 20, 232, false, true));
            assert!(c.timed_ash_due(1000 + phase, 20, 400, true, false));
            assert!(c.timed_ash_due(1000 + phase, 3, 2200, true, false));
            assert!(c.timed_ash_due(1000 + phase, 3, 2300, false, true));
            assert!(!c.timed_ash_due(1000 + phase, 20, 233, false, false));
            c.jala_done = true;
            assert!(!c.timed_ash_due(1000 + phase, 20, 233, false, true));
        }
    }
    #[test]
    fn early_economy_window_wakes_between_twenty_frame_ticks() {
        let mut c = Control::default();
        let mut checks = Vec::new();
        for now in 3000..3400 {
            let t = now - 1000;
            if (now % c.decision_period == 0 || c.economy_check_due(now, 3, t)) && c.take_economy_check(now, 3, t) {
                checks.push(t);
            }
        }
        assert_eq!(checks, (2120..2300).step_by(25).collect::<Vec<_>>());
        assert!(!c.economy_check_due(4000, 20, 2200));
    }
    #[test]
    fn opening_resolves_cherry_with_the_normal_selector() {
        use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, pe_rs, runner_internal::PeWorldOwner};
        let mut world = PeWorldOwner::new_reset(PeWorldConfig {
            scene: pe_rs::SceneType::Fog,
            ..Default::default()
        })
        .unwrap()
        .install_current()
        .unwrap();
        world.with_backend(|b| {
            PeBackend::scope_backend(b, || {
                let mut c = Control::default();
                c.forecast.add(2300, prediction::EffectKind::Cherry, (2, 4), false);
                c.resolve_cherries(0, 0, 600);
                assert_eq!(c.forecast.events[0].pos, (1, 9));
                c.cherry_plans.fill(None);
                c.resolve_cherries(0, 1, 3099);
                assert_eq!(c.forecast.events[0].pos, (1, 9));
            })
        });
    }
    #[test]
    fn observed_ninth_wave_track_flag_changes_current_plan_not_candidates() {
        let mut c = Control::default();
        assert!(!c.nuke_wave(10));
        assert!(c.ash_wave(10));
        assert!(c.nuke_wave(11));
        c.even_after_nine = true;
        assert!(c.nuke_wave(10));
        assert!(!c.ash_wave(10));
        assert!(!c.nuke_wave(11));
        assert!(c.ash_wave(11));
        c.nuke_done[10] = true;
        assert!(!c.need_nuke(10));
        c.carried_doom = 9;
        assert!(c.need_nuke(10));
    }
}
