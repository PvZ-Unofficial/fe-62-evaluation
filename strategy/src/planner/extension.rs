//! Optional improvements around an executable, unchanged A/B policy endpoint.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
    Ordinary,
    Ash,
    Front,
    FootStore,
    Carry,
    Bank,
}
#[derive(Clone, Copy)]
pub(super) struct Offer {
    pub pos: Pos,
    pub target: Option<Zombie>,
    pub base: Outcome,
    pub after: Outcome,
    pub reason: i32,
    pub source: Source,
}
#[derive(Clone, Copy, Debug)]
enum NeedKind {
    Danger,
    Window { edge: f32 },
}

#[derive(Clone, Copy)]
struct Need {
    z: Zombie,
    at: i32,
    strong: bool,
    kind: NeedKind,
    before: Outcome,
    fallback: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Service {
    Resolved,
    Preserved,
    Served, // Positive control, but the original obligation is still outstanding.
    Unknown,
    NoGain,
}

impl Service {
    fn covered(self) -> bool {
        matches!(self, Self::Resolved | Self::Preserved)
    }
    fn helpful(self) -> bool {
        self.covered() || self == Self::Served
    }
}

fn service(need: Need, after: Outcome) -> Service {
    let certain = !need.strong || after.known_through(need.at);
    if after.death <= need.at && after.death <= after.danger {
        return if !need.strong || after.death < after.uncertain_at {
            Service::Resolved
        } else {
            Service::Served
        };
    }
    let preserved = match need.kind {
        NeedKind::Danger => after.danger > need.at && after.end_at >= need.at,
        NeedKind::Window { edge } => after.danger > need.at && after.end_at == need.at && after.x_at_end >= edge,
    };
    if preserved && certain {
        return Service::Preserved;
    }
    let before = need.before;
    let changed = after.death < before.death
        || after.danger > before.danger
        || after.ash_mask & !before.ash_mask != 0
        || before.end_at == after.end_at
            && (after.x_at_end > before.x_at_end + 0.01 || after.hp_at_end < before.hp_at_end);
    // Merely catching an already descending hammer is not a new stop.
    if changed && (after.trigger < after.impact || after.danger > before.danger) {
        Service::Served
    } else if after.end_at < need.at && after.danger > need.at {
        Service::Unknown
    } else {
        Service::NoGain
    }
}

fn ground_request(z: Zombie, imp_padding: bool) -> bool {
    matches!(z.row, 1 | 2 | 5 | 6)
        && (z.giant()
            || z.kind == Z::Football
            || imp_padding && z.kind == Z::Imp && z.phase == ZombiePhase::ZombieNormal)
}

// Keys from different groups must share centiseconds, not bucket indices.
fn deadline_key(danger: i32, band: i32, recovery: i32, score_far: bool) -> (bool, i32) {
    let far = danger > recovery;
    let width = band.max(1);
    (far, if far && score_far { 0 } else { danger / width * width })
}

#[derive(Clone, Copy)]
struct Group {
    mode: u32,
    band: i32,
    delay: f64,
    ash: f64,
    plan: f64,
    trigger: f64,
    urgency: f64,
    damage: f64,
    room: f64,
    stock: f64,
    wait_mode: bool,
    wait: f64,
}
fn number(group: &str, name: &str, default: f64, low: f64, high: f64) -> f64 {
    let key = format!("SIX_{group}_{name}");
    let n = std::env::var(&key).map_or(default, |s| s.parse::<f64>().expect("numeric extension parameter"));
    assert!(n.is_finite() && (low..=high).contains(&n), "invalid {key}");
    n
}
impl Group {
    fn read(group: &str) -> Self {
        let n = |name, default, lo, hi| number(group, name, default, lo, hi);
        let mode = n("MODE", 0., 0., 2.);
        let band = n("BAND", 0., 0., 200.);
        assert!(mode.fract() == 0. && band.fract() == 0.);
        Self {
            mode: mode as u32,
            band: band as i32,
            delay: n("DELAY", 0., -2., 2.),
            ash: n("ASH", 0., -2., 2.),
            plan: n("PLAN", 0., -1., 1.),
            trigger: n("TRIGGER", 0., -1., 1.),
            urgency: n("URGENCY", 1., 0., 4.),
            damage: n("DAMAGE", 0., 0., 4.),
            room: n("ROOM", 0., 0., 4.),
            stock: n("STOCK", 0., 0., 2.),
            wait_mode: n("WAIT_MODE", 0., 0., 1.) != 0.,
            wait: n("WAIT", 0., -2., 2.),
        }
    }
    fn active(self) -> bool {
        self.mode != 0
    }
}
struct Config {
    ordinary: Group,
    front: Group,
    pool: u32,
    requests: bool,
    relax: bool,
    horizon: i32,
    order: i32,
    bank: i32,
    front_horizon: i32,
    front_order: i32,
    sites: bool,
    debt: f64,
}
impl Default for Config {
    fn default() -> Self {
        let integer = |name, default, lo, hi| {
            let n = number("EXT", name, default, lo, hi);
            assert!(n.fract() == 0.);
            n as i32
        };
        let horizon = integer("HORIZON", 0., 0., 3000.);
        assert!(horizon == 0 || horizon >= 750);
        let front_horizon = number("FRONT", "WINDOW", 0., 0., 3000.);
        assert!(front_horizon.fract() == 0. && (front_horizon == 0. || front_horizon >= 750.));
        let front_order = number("FRONT", "ORDER", 0., 0., 3.);
        assert!(front_order.fract() == 0.);
        Self {
            front_horizon: front_horizon as i32,
            front_order: front_order as i32,
            sites: integer("SITES", 0., 0., 1.) != 0,
            debt: number("EXT", "DEBT", 0.25, 0., 2.),
            ordinary: Group::read("ORD"),
            front: Group::read("FRONT"),
            pool: integer("POOL", 0., 0., 2.) as u32,
            requests: integer("REQUESTS", 0., 0., 1.) != 0,
            relax: integer("RELAX", 0., 0., 1.) != 0,
            horizon,
            order: integer("ORDER", 0., 0., 3.),
            bank: integer("BANK", 0., 0., 2.),
        }
    }
}
#[derive(Clone, Copy)]
struct Link {
    offer: Offer,
    sources: u8,
}
pub(super) struct State {
    cfg: Config,
    pub collecting: bool,
    offers: [Vec<Link>; 54],
    needs: Vec<Need>,
    checkpoints: Vec<(ZombieId, Outcome)>,
    local_scores: Vec<(ZombieId, Pos, [f64; 3])>,
    solid: Forecast,
    context: Option<(u64, u64)>,
    needs_ready: bool,
    recovery: i32,
    far_score: bool,
    shared_order: bool,
    scope: Cell<Option<(i32, u64, i32, bool)>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            cfg: Config::default(),
            collecting: false,
            offers: std::array::from_fn(|_| Vec::new()),
            needs: Vec::with_capacity(128),
            checkpoints: Vec::with_capacity(128),
            local_scores: Vec::with_capacity(128),
            solid: Forecast::default(),
            context: None,
            needs_ready: false,
            recovery: 751 + setting("SIX_DECISION_PERIOD", 20).max(1) as i32 - 1,
            far_score: false,
            shared_order: false,
            scope: Cell::new(None),
        }
    }
}
impl State {
    pub(super) fn requires_evidence(&self) -> bool {
        self.cfg.relax || self.cfg.pool != 0 && self.cfg.requests
    }
}
impl Planner {
    fn extension_scope_state(&self) -> (i32, bool) {
        let now = rsvz::timing::clock();
        if let Some((clock, rev, wave, allowed)) = self.extension.scope.get() {
            if clock == now && rev == self.scene.revision {
                return (wave, allowed);
            }
        }
        let wave = time().1;
        let allowed = !matches!(wave, 0 | 20)
            && !self.scene.zombies.iter().any(|z| {
                z.carried
                    && matches!(z.row, 1 | 2 | 5 | 6)
                    && (z.giant() || z.kind == Z::Football || self.imp_padding && z.kind == Z::Imp)
            });
        self.extension
            .scope
            .set(Some((now, self.scene.revision, wave, allowed)));
        (wave, allowed)
    }
    fn extension_scope(&self) -> bool {
        self.extension_scope_state().1
    }
    pub(super) fn invalidate_extension(&mut self) {
        self.extension.context = None;
        self.extension.scope.set(None);
        self.extension.needs_ready = false;
        self.extension.needs.clear();
        self.extension.checkpoints.clear();
        self.extension.local_scores.clear();
    }
    fn extension_context(&mut self, f: &Forecast) {
        let key = (self.scene.revision, forecast_key(f));
        if self.extension.context != Some(key) {
            self.extension.context = Some(key);
            self.extension.needs_ready = false;
            self.extension.needs.clear();
            self.extension.checkpoints.clear();
            self.extension.local_scores.clear();
        }
    }
    pub(super) fn pool_active(&self) -> bool {
        self.extension.cfg.pool != 0 && ready(sel(PuffShroom)) && self.extension_scope()
    }
    fn front_target(z: Zombie, base: Outcome, wave: i32) -> bool {
        z.wave == 10
            && !z.carried
            && z.kind == Z::GigaGargantuar
            && matches!(z.row, 2 | 5)
            && matches!(wave, 10..=12)
            && base.danger > 751
    }
    fn group(&self, z: Zombie, base: Outcome) -> Group {
        if Self::front_target(z, base, self.extension_scope_state().0) {
            self.extension.cfg.front
        } else {
            self.extension.cfg.ordinary
        }
    }
    pub(super) fn begin_order(&mut self, source: Source) {
        let ordinary = self.extension.cfg.ordinary.active();
        let front = self.extension.cfg.front.active();
        if !ordinary && !front {
            self.extension.far_score = false;
            self.extension.shared_order = false;
            return;
        }
        let wave = self.extension_scope_state().0;
        let front_context = front
            && matches!(wave, 10..=12)
            && self
                .scene
                .zombies
                .iter()
                .any(|z| z.wave == 10 && !z.carried && z.kind == Z::GigaGargantuar && matches!(z.row, 2 | 5));
        self.extension.shared_order = match source {
            Source::Front => front,
            Source::Ordinary => ordinary || wave == 12 && front_context,
            _ => ordinary || front_context,
        };
        let ordinary_score = self.extension.cfg.ordinary.mode == 2;
        let front_score = self.extension.cfg.front.mode == 2;
        self.extension.far_score = match source {
            Source::Front => front_score,
            Source::Ordinary => ordinary_score || wave == 12 && front_context && front_score,
            _ => ordinary_score || front_context && front_score,
        };
    }
    pub(super) fn changed_preferences(&self, _z: Zombie, _base: Outcome) -> bool {
        self.extension.shared_order && self.extension_scope()
    }
    pub(super) fn fresh_contact_needed(&self, at: i32) -> bool {
        self.extension.cfg.relax && at != self.scene.now && self.extension_scope()
    }
    pub(super) fn candidate_request_guard(&self) -> bool {
        self.extension.collecting && self.extension.cfg.requests
    }
    pub(super) fn changed_carry(&self, w: i32) -> bool {
        (self.extension.cfg.ordinary.active() || matches!(w, 10..=12) && self.extension.cfg.front.active())
            && w != 20
            && self.extension_scope()
    }
    pub(super) fn carry_horizon(&self, wave: i32) -> i32 {
        let horizon = if matches!(wave, 10..=12) {
            self.extension.cfg.front_horizon
        } else {
            self.extension.cfg.horizon
        };
        if horizon != 0 && self.extension_scope() {
            horizon
        } else {
            self.tuning.horizon
        }
    }
    pub(super) fn carry_columns(&self, wave: i32) -> &'static [i32; 3] {
        let order = if matches!(wave, 10..=12) {
            self.extension.cfg.front_order
        } else {
            self.extension.cfg.order
        };
        if order != 0 && self.extension_scope() {
            &[[5, 4, 3], [3, 4, 5], [4, 5, 3]][(order - 1) as usize]
        } else {
            self.tuning.columns()
        }
    }
    pub(super) fn extended_sites(&self, z: Zombie) -> bool {
        self.extension.cfg.sites && matches!(z.row, 2 | 5) && z.giant() && self.extension_scope()
    }
    pub(super) fn bank_rows(&self) -> &'static [i32; 2] {
        let order = self.extension.cfg.bank;
        if order != 0 && self.extension_scope() {
            &[[5, 2], [2, 5]][(order - 1) as usize]
        } else {
            self.tuning.bank_rows()
        }
    }
    fn offer_group(&self, offer: Offer) -> Group {
        let z = offer.target.unwrap();
        if self.extension.cfg.requests
            && self.extension.needs_ready
            && self
                .extension
                .context
                .is_some_and(|(rev, _)| rev == self.scene.revision)
            && self.extension.needs.iter().any(|n| n.strong && n.z.id == z.id)
        {
            self.extension.cfg.ordinary
        } else {
            self.group(z, offer.base)
        }
    }
    fn preference_key(&self, offer: Offer) -> (bool, i32) {
        let Some(_) = offer.target else { return (true, NEVER) };
        let g = self.offer_group(offer);
        deadline_key(
            offer.base.danger,
            g.band,
            self.extension.recovery,
            self.extension.far_score,
        )
    }
    fn extra_local(&mut self, offer: Offer, f: &Forecast, g: Group) -> f64 {
        if g.damage == 0. && g.room == 0. {
            return g.stock
                * f64::from(
                    offer.after.trigger >= self.extension.recovery && offer.after.trigger < offer.after.impact,
                );
        }
        let z = offer.target.unwrap();
        self.extension_context(f);
        if let Some((_, _, v)) = self
            .extension
            .local_scores
            .iter()
            .find(|(id, pos, _)| *id == z.id && *pos == offer.pos)
        {
            return g.damage * v[0] + g.room * v[1] + g.stock * v[2];
        }
        let at = self.extension.recovery;
        let before = if let Some((_, v)) = self.extension.checkpoints.iter().find(|(id, _)| *id == z.id) {
            *v
        } else {
            let v = run_until(z, &self.scene, f, None, at, 1);
            self.extension.checkpoints.push((z.id, v));
            v
        };
        let mut pad = Pad::new(offer.pos, PuffShroom);
        pad.replaces_occupant = self.scene.main(offer.pos).is_some_and(|p| p.kind == PuffShroom);
        pad.order = self.scene.next_plant_order;
        pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, z, offer.pos);
        pad.complete_outcome = true;
        let after = run_until(z, &self.scene, f, Some(pad), at, 1);
        let (damage, room) = if before.end_at == at && after.end_at == at && before.death > at && after.death > at {
            (
                ((after.gloom_damage - before.gloom_damage) as f64 / 160.).clamp(-2., 2.),
                ((after.x_at_end - before.x_at_end) as f64 / 80.).clamp(-2., 2.),
            )
        } else {
            (0., 0.)
        };
        let stock = f64::from(offer.after.trigger >= at && offer.after.trigger < offer.after.impact);
        let value = g.damage * damage + g.room * room + g.stock * stock;
        self.extension
            .local_scores
            .push((z.id, offer.pos, [damage, room, stock]));
        value
    }
    fn extension_score(&mut self, offer: Offer, f: &Forecast) -> f64 {
        let Some(_) = offer.target else {
            return 0.05;
        };
        self.extension_context(f);
        let g = self.offer_group(offer);
        let urgency = if self.extension.far_score && offer.base.danger > self.extension.recovery {
            g.urgency * (751. / offer.base.danger.max(1) as f64).min(1.)
        } else {
            0.
        };
        if g.mode == 0 {
            return self.tuning.score(offer.base, offer.after, f) + urgency
                - if self.extension.cfg.relax {
                    self.tuning.forecast_loss(offer.base, offer.after)
                } else {
                    0.
                };
        }
        let tuned = self.tuning.adjusted(g.delay, g.ash, g.plan, g.trigger);
        let score = tuned.score(offer.base, offer.after, f) - tuned.forecast_loss(offer.base, offer.after);
        score + urgency + self.extra_local(offer, f, g)
    }
    pub(super) fn extension_waits(&mut self, offer: Offer, f: &Forecast) -> bool {
        if !self.extension.cfg.ordinary.wait_mode && !self.extension.cfg.front.wait_mode {
            return false;
        }
        self.extension_context(f);
        let Some(_) = offer.target else {
            return false;
        };
        let g = self.offer_group(offer);
        g.wait_mode
            && self.extension_scope()
            && offer.base.danger > self.extension.recovery
            && self.extension_score(offer, f) <= g.wait
    }
    pub(super) fn prefer_extended(&mut self, new: Offer, old: Offer, f: &Forecast) -> bool {
        self.extension_context(f);
        let nk = self.preference_key(new);
        let ok = self.preference_key(old);
        nk < ok || nk == ok && self.extension_score(new, f) > self.extension_score(old, f)
    }
    pub(super) fn relax_future_rejection(&self, base: Outcome, after: Outcome) -> bool {
        if !self.extension.cfg.relax || !self.extension_scope() {
            return false;
        }
        let at = after.danger;
        if at <= self.extension.recovery && after.known_through(at) && base.known_through(at) {
            return false;
        }
        after.trigger < after.impact
            || after.danger > base.danger
            || after.death < base.death
            || after.ash_mask & !base.ash_mask != 0
    }
    fn prepare_needs(&mut self, f: &Forecast) {
        if self.extension.needs_ready {
            return;
        }
        self.extension.needs_ready = true;
        self.extension.solid.events.clear();
        self.extension
            .solid
            .events
            .extend(f.events.iter().copied().filter(|e| e.certain));
        let recovery = self.puff_recovery();
        for i in 0..self.scene.zombies.len() {
            let z = self.scene.zombies[i];
            if !ground_request(z, self.imp_padding) {
                continue;
            }
            let (_, hi) = prediction::attack_offsets(z);
            // Only a real currently legal opportunity can reserve a card.
            let edges = pad_columns(z).iter().copied().filter_map(|col| {
                let mut pad = Pad::new((z.row, col), PuffShroom);
                pad.offset_x = 4;
                let edge = (pad.rect().left + 20 - hi + 1) as f32;
                (z.x >= edge && self.scene.main((z.row, col)).is_none() && legal(sel(PuffShroom), (z.row, col)))
                    .then_some(edge)
            });
            let last_edge = edges.min_by(f32::total_cmp);
            let could_close =
                last_edge.is_some_and(|edge| z.x - prediction::max_step(z.kind) * (recovery as f32) < edge);
            if !could_close && !prediction::could_threaten(z, &self.scene, recovery) {
                continue;
            }
            // Short queries first. Only an actually closing window needs a long
            // unresolved-threat check; no long trajectory for every nearby actor.
            let short = run_until(z, &self.scene, f, None, recovery, 1);
            let danger = short.danger <= recovery;
            let closed = short.end_at == recovery && last_edge.is_some_and(|edge| short.x_at_end < edge);
            if short.death > recovery && (danger || closed) {
                let base = if danger {
                    short
                } else {
                    self.cached_outcome(z, f)
                        .unwrap_or_else(|| run(z, &self.scene, f, None))
                };
                if !danger && base.safe() {
                    continue;
                }
                let at = if danger { short.danger } else { recovery };
                let strong = short.known_through(at) && (danger || base.known_through(base.danger));
                self.extension.needs.push(Need {
                    z,
                    at,
                    strong,
                    before: short,
                    fallback: false,
                    kind: if danger {
                        NeedKind::Danger
                    } else {
                        NeedKind::Window {
                            edge: last_edge.unwrap(),
                        }
                    },
                });
            } else if could_close && short.death <= recovery && short.assumptions & prediction::ASSUME_PLAN != 0 {
                // A plan-dependent clearance is a soft fallback debt, never a
                // fabricated certainty that all future ice/ash will fail.
                let bare = run_until(z, &self.scene, &self.extension.solid, None, recovery, 1);
                if bare.death > recovery
                    && (bare.danger <= recovery
                        || bare.end_at == recovery && last_edge.is_some_and(|edge| bare.x_at_end < edge))
                {
                    self.extension.needs.push(Need {
                        z,
                        at: bare.danger.min(recovery),
                        strong: false,
                        before: bare,
                        fallback: true,
                        kind: if bare.danger <= recovery {
                            NeedKind::Danger
                        } else {
                            NeedKind::Window {
                                edge: last_edge.unwrap(),
                            }
                        },
                    });
                }
            }
        }
    }

    fn puff_recovery(&self) -> i32 {
        self.extension.recovery
    }

    fn need_service(&self, pos: Pos, f: &Forecast, need: Need) -> Service {
        if pos.0 != need.z.row {
            return Service::NoGain;
        }
        let mut pad = Pad::new(pos, PuffShroom);
        pad.replaces_occupant = self.scene.main(pos).is_some_and(|p| p.kind == PuffShroom);
        pad.order = self.scene.next_plant_order;
        pad.lifetime = prediction::puff_external_destruction_bound(&self.scene, need.z, pos);
        if !prediction::could_contact_pad(need.z, pad) {
            return Service::NoGain;
        }
        pad.complete_outcome = true;
        let forecast = if need.fallback { &self.extension.solid } else { f };
        let after = run_until(need.z, &self.scene, forecast, Some(pad), need.at, 1);
        service(need, after)
    }

    // Count residual obligations separately from actual help. Partial blocking
    // cannot erase a lost future window, yet remains eligible for best effort.
    fn missed_needs(&self, pos: Pos, f: &Forecast) -> (usize, usize, bool, i32) {
        let (mut hard, mut soft, mut helpful, mut deadline) = (0, 0, false, NEVER);
        for &need in &self.extension.needs {
            let result = self.need_service(pos, f, need);
            if !result.covered() {
                if need.strong {
                    hard += 1;
                } else {
                    soft += 1;
                }
            }
            if need.strong && result.helpful() {
                helpful = true;
                deadline = deadline.min(need.at);
            }
        }
        (hard, soft, helpful, deadline)
    }

    pub(super) fn puff_spend_allowed(&mut self, pos: Pos, f: &Forecast) -> bool {
        if self.extension.cfg.pool == 0 || !self.extension.cfg.requests || !self.extension_scope() {
            return true;
        }
        self.extension_context(f);
        self.prepare_needs(f);
        self.missed_needs(pos, f).0 == 0
    }
    pub(super) fn offer_puff(&mut self, offer: Offer) {
        let bucket = &mut self.extension.offers[((offer.pos.0 - 1) * 9 + offer.pos.1 - 1) as usize];
        if let Some(link) = bucket
            .iter_mut()
            .find(|l| l.offer.target.map(|z| z.id) == offer.target.map(|z| z.id))
        {
            link.sources |= 1 << offer.source as u8;
            if (offer.source as u8) < (link.offer.source as u8) {
                link.offer = offer;
            }
        } else {
            bucket.push(Link {
                offer,
                sources: 1 << offer.source as u8,
            });
        }
    }
    pub(super) fn pooled_puff(&mut self, w: i32, t: i32, f: &Forecast) {
        self.extension_context(f);
        for b in &mut self.extension.offers {
            b.clear();
        }
        self.extension.collecting = true;
        self.choose_pad_filtered(PuffShroom, f, None, false, false, |_, _| true);
        if self.spare_puff() {
            self.choose_ash_pad(f);
            if matches!(w, 10 | 11) {
                self.choose_pad_filtered(PuffShroom, f, None, true, false, |_, _| true);
            }
            if matches!(w, 10..=12 | 19) {
                self.carry_pad(w, f);
            }
            if w > 1 && t > 500 && self.scene.main((5, 3)).is_none() && combat::jack_safe((5, 3), 100) {
                let foot = self
                    .scene
                    .row_zombies(5)
                    .find(|z| z.kind == Z::Football && z.x > 210. && z.x < 600.)
                    .copied();
                if let Some(z) = foot {
                    let base = self.assessment(z, f);
                    let mut pad = Pad::new((5, 3), PuffShroom);
                    pad.order = self.scene.next_plant_order;
                    pad.trigger_window = 2500;
                    if !base.safe() && legal(sel(PuffShroom), (5, 3)) {
                        let after = run(z, &self.scene, f, Some(pad));
                        if after.trigger < NEVER && after.impact > after.trigger && after.danger >= base.danger {
                            self.offer_puff(Offer {
                                pos: (5, 3),
                                target: Some(z),
                                base,
                                after,
                                reason: 12,
                                source: Source::FootStore,
                            });
                        }
                    }
                }
            }
        }
        self.extension.collecting = false;
        if self.extension.offers.iter().all(Vec::is_empty) {
            return;
        }
        if self.extension.cfg.requests {
            self.prepare_needs(f);
        }
        // One ordering mode for this complete candidate set. A score-mode group
        // cannot receive unconditional priority merely because its old key was 0.
        self.extension.far_score = self
            .extension
            .offers
            .iter()
            .flatten()
            .any(|link| link.offer.target.is_some() && self.offer_group(link.offer).mode == 2);
        let hard = self.extension.needs.iter().filter(|n| n.strong).count();
        let mut best: Option<(Offer, (usize, i32, u8, bool, i32), f64)> = None;
        for i in 0..54 {
            if self.extension.offers[i].is_empty() {
                continue;
            }
            let pos = self.extension.offers[i][0].offer.pos;
            let (missed, debt, helpful, deadline) = if self.extension.cfg.requests {
                self.missed_needs(pos, f)
            } else {
                (0, 0, false, NEVER)
            };
            if hard > 0 && missed == hard && !helpful {
                continue;
            }
            for j in 0..self.extension.offers[i].len() {
                let offer = self.extension.offers[i][j].offer;
                if !helpful && self.extension_waits(offer, f) {
                    continue;
                }
                let stage = if self.extension.cfg.pool == 1 {
                    offer.source as u8
                } else {
                    0
                };
                let urgency = self.preference_key(offer);
                let key = (missed, deadline, stage, urgency.0, urgency.1);
                // One cost for residual requests. Its strength is independent of
                // the per-target resource benefit; never count the same benefit twice.
                let score = self.extension_score(offer, f) - self.extension.cfg.debt * debt as f64;
                if best.is_none_or(|(_, k, v)| key < k || key == k && score > v) {
                    best = Some((offer, key, score));
                }
            }
        }
        if let Some((offer, _, _)) = best {
            if let Some(z) = offer.target {
                self.record_choice(z, offer.base, offer.after, offer.pos, PuffShroom, f);
            }
            if let Some(z) = offer.target {
                if self.scene.main(offer.pos).is_none() && self.later_pad_better(z, offer.pos, f, offer.after) {
                    self.puff_window_hold = Some(self.scene.now);
                    return;
                }
            }
            if play_puff(offer.pos, offer.reason) {
                if let Some(z) = offer.target {
                    self.recheck_planted(z.id, offer.pos, f);
                } else {
                    self.observe(rsvz::timing::clock(), self.last_ice);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use rsvz::core::model::{ContactRect, PlantId};

    fn giant(id: u32, x: f32) -> Zombie {
        Zombie {
            age: 0,
            id: ZombieId::from_raw(id),
            kind: Z::GigaGargantuar,
            row: 1,
            wave: 9,
            carried: false,
            x,
            hp: 6000,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.37,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(x as i32 - 17, 0, x as i32 + 108, 154),
        }
    }

    fn window(z: Zombie, before: Outcome) -> Need {
        Need {
            z,
            at: 770,
            strong: false,
            kind: NeedKind::Window { edge: 16. },
            before,
            fallback: false,
        }
    }

    #[test]
    fn block_is_helpful_without_erasing_a_remaining_window_obligation() {
        let z = giant(65537, 100.);
        let mut p = Planner::default();
        p.scene = Scene::fixture(vec![z]);
        p.scene.plants.push(prediction::Plant {
            id: PlantId::from_raw(65537),
            kind: GloomShroom,
            pos: (2, 1),
            hp: 300,
            rect: ContactRect::new(10, 0, 70, 80),
            attack: ContactRect::new(-80, 0, 200, 154),
            shooting: 0,
            launch: 200,
            state: 0,
            phase_left: 0,
            effect: 0,
        });
        let f = Forecast::default();
        let before = run_until(z, &p.scene, &f, None, 770, 1);
        let need = window(z, before);
        assert_eq!(p.need_service((1, 1), &f, need), Service::Served);
        p.extension.needs.push(need);
        assert_eq!(p.missed_needs((1, 1), &f).1, 1);
    }

    #[test]
    fn an_empty_hammer_cannot_discharge_a_window_without_changing_the_trajectory() {
        let mut z = giant(65537, 100.);
        z.phase = ZombiePhase::GargantuarSmashing;
        z.progress = 0.62;
        let mut p = Planner::default();
        p.scene = Scene::fixture(vec![z, giant(65538, 100.)]);
        let f = Forecast::default();
        let before = run_until(z, &p.scene, &f, None, 770, 1);
        let need = window(z, before);
        assert_eq!(p.need_service((1, 1), &f, need), Service::NoGain);
        p.extension.needs.push(need);
        assert_eq!(p.missed_needs((1, 1), &f).1, 1);
    }

    #[test]
    fn saving_a_known_strike_is_useful_even_without_a_new_hammer_animation() {
        let z = giant(65537, 100.);
        let before = Outcome {
            danger: 5,
            end_at: 5,
            ..Outcome::default()
        };
        let need = Need {
            z,
            at: 5,
            strong: true,
            kind: NeedKind::Danger,
            before,
            fallback: false,
        };
        let after = Outcome {
            trigger: 5,
            impact: 5,
            end_at: 5,
            ..Outcome::default()
        };
        assert_eq!(service(need, after), Service::Preserved);
    }

    #[test]
    fn airborne_and_landing_imps_do_not_enter_ground_requests_and_toggle_is_respected() {
        let mut z = giant(65537, 50.);
        z.kind = Z::Imp;
        z.phase = ZombiePhase::ImpGettingThrown;
        assert!(!ground_request(z, true));
        z.phase = ZombiePhase::ImpLanding;
        assert!(!ground_request(z, true));
        z.phase = ZombiePhase::ZombieNormal;
        assert!(ground_request(z, true));
        assert!(!ground_request(z, false));
    }

    #[test]
    fn checkpoint_does_not_compare_health_from_different_stopping_times() {
        let out = Outcome {
            end_at: 100,
            danger: 100,
            hp_at_end: 500,
            ..Outcome::default()
        };
        assert_eq!(out.checkpoint_hp(200), None);
        let out = Outcome {
            end_at: 100,
            death: 100,
            hp_at_end: -20,
            ..Outcome::default()
        };
        assert_eq!(out.checkpoint_hp(200), Some(0));
    }

    fn offer(z: Zombie, danger: i32, after: i32) -> Offer {
        Offer {
            pos: (z.row, 3),
            target: Some(z),
            base: Outcome {
                danger,
                ..Outcome::default()
            },
            after: Outcome {
                danger: after,
                trigger: 1,
                impact: 134,
                ..Outcome::default()
            },
            reason: 10,
            source: Source::Ordinary,
        }
    }
    #[test]
    fn default_and_wait_only_leave_original_ranking_dispatch_disabled() {
        let mut p = Planner::default();
        let z = giant(65537, 100.);
        let out = Outcome {
            danger: 1000,
            ..Outcome::default()
        };
        assert!(!p.changed_preferences(z, out));
        assert!(!p.changed_carry(10));
        assert!(!p.pool_active());
        p.extension.cfg.ordinary.wait_mode = true;
        assert!(!p.changed_preferences(z, out));
        assert!(!p.changed_carry(10));
    }
    #[test]
    fn same_cell_keeps_every_target_and_purpose() {
        let mut p = Planner::default();
        let a = offer(giant(65537, 100.), 2000, 2500);
        let b = offer(giant(65538, 100.), 1000, 1500);
        p.offer_puff(a);
        p.offer_puff(b);
        p.offer_puff(Offer {
            source: Source::Carry,
            ..a
        });
        let links = &p.extension.offers[2];
        assert_eq!(links.len(), 2);
        assert_eq!(
            links[0].sources,
            (1 << Source::Ordinary as u8) | (1 << Source::Carry as u8)
        );
        assert_eq!(links[1].offer.target.unwrap().id, b.target.unwrap().id);
    }
    #[test]
    fn w10_group_is_not_the_wave_number_alone() {
        let mut z = giant(65537, 500.);
        z.row = 2;
        z.wave = 10;
        let out = Outcome {
            danger: 2000,
            ..Outcome::default()
        };
        assert!(Planner::front_target(z, out, 11));
        assert!(!Planner::front_target(z, Outcome { danger: 200, ..out }, 11));
        z.wave = 9;
        assert!(!Planner::front_target(z, out, 10));
        z.wave = 10;
        z.carried = true;
        assert!(!Planner::front_target(z, out, 10));
    }
    #[test]
    fn strong_window_request_upgrades_front_control_without_discarding_identity() {
        let mut p = Planner::default();
        let mut z = giant(65537, 100.);
        z.row = 2;
        z.wave = 10;
        let o = offer(z, 2000, 2500);
        assert!(Planner::front_target(z, o.base, 11));
        p.extension.cfg.ordinary.mode = 1;
        p.extension.cfg.front.mode = 2;
        p.extension.cfg.requests = true;
        p.extension.context = Some((p.scene.revision, 0));
        p.extension.needs_ready = true;
        p.extension.needs.push(Need {
            strong: true,
            ..window(z, o.base)
        });
        assert_eq!(p.offer_group(o).mode, 1);
    }

    #[test]
    fn preparing_requests_does_not_populate_main_prediction_cache_or_reservations() {
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
            b.select_card(sel(PuffShroom).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            PeBackend::scope_backend(b, || {
                let mut p = Planner::default();
                p.scene = Scene::fixture(vec![giant(65537, 100.)]);
                p.prepare_needs(&Forecast::default());
                assert!(!p.extension.needs.is_empty());
                assert!(p.cache.is_empty());
                assert!(p.pad_retry.is_empty());
                assert!(p.imp_puff_hold.is_none());
                assert_eq!(p.next_carry, 0);
            });
        });
    }

    #[test]
    fn a_bank_plant_is_observed_even_without_a_representative_zombie() {
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
            b.select_card(sel(PuffShroom).checked().unwrap()).unwrap();
            b.finish_card_selection().unwrap();
            PeBackend::scope_backend(b, || {
                let mut p = Planner::default();
                p.scene = Scene::fixture(vec![]);
                p.extension.cfg.pool = 2;
                assert!(p.scene.main((5, 3)).is_none());
                p.pooled_puff(19, 600, &Forecast::default());
                assert!(
                    p.scene.main((5, 3)).is_some(),
                    "actual bank planting must refresh the scene"
                );
            });
        });
    }

    #[test]
    fn mixed_band_widths_compare_actual_times_not_bucket_numbers() {
        assert!(deadline_key(1000, 0, 770, false) < deadline_key(3000, 200, 770, false));
        assert_eq!(deadline_key(3000, 200, 770, false), (true, 3000));
        assert_eq!(deadline_key(1199, 200, 770, false), (true, 1000));
        assert_eq!(deadline_key(1000, 0, 770, false), (true, 1000));
    }
    #[test]
    fn shared_score_mode_keeps_the_whole_recovery_window_ahead_of_distant_work() {
        assert!(deadline_key(760, 0, 770, true) < deadline_key(780, 200, 770, true));
        assert_eq!(deadline_key(3000, 200, 770, true), deadline_key(1000, 0, 770, true));
    }
    #[test]
    fn mixed_group_order_is_transitive_in_both_shared_modes() {
        for mode in [false, true] {
            let candidates = [(760, 0, 1), (780, 200, 5), (1000, 0, 2), (1199, 200, 9), (3000, 25, 3)];
            let key = |(d, b, score): (i32, i32, i32)| (deadline_key(d, b, 770, mode), -score);
            for &a in &candidates {
                for &b in &candidates {
                    for &c in &candidates {
                        if key(a) < key(b) && key(b) < key(c) {
                            assert!(key(a) < key(c));
                        }
                    }
                }
            }
        }
    }
}
