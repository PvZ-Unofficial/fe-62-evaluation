//! Shared candidate scorer. IDs are action coordinates, never entity identities.
use super::*;
use prediction::{Forecast, Outcome, Pad, Scene, Zombie};
use std::io::Write;
mod experience;
mod hierarchical;
mod lite;
mod local;
mod placement_audit;
pub mod plan;
mod tree;

pub const FEATURES: usize = 192;
const CARDS: [CardSelection; 7] = [
    sel_const(PuffShroom),
    sel_const(Blover),
    sel_const(Squash),
    sel_const(CherryBomb),
    sel_const(Jalapeno),
    sel_const(IceShroom),
    COPY,
];
const fn sel_const(kind: PlantKind) -> CardSelection {
    CardSelection::Plant(kind)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Rule,
    Norm,
    Full,
    Cheap,
    Lite,
    Local,
}
#[derive(Clone, Copy)]
pub struct Context {
    pub wave: i32,
    pub time: i32,
    pub ice_due: i32,
    pub doom_due: i32,
    pub ice_allowed: [bool; 2],
    pub ice_required: bool,
    pub cherry: bool,
    pub jala: bool,
    pub cherry_done: bool,
    pub jala_done: bool,
    pub pending_ice: i32,
    pub last_ice: i32,
    pub last_blover_wave: i32,
    pub round: u32,
    pub plan: Option<plan::Plan>,
}
#[derive(Clone, Copy, Debug)]
pub struct Action {
    pub kind: usize,
    pub pos: Pos,
}
impl Action {
    pub fn id(self) -> usize {
        self.kind * 100 + self.pos.0 as usize * 10 + self.pos.1 as usize
    }
    pub fn card(self) -> Option<CardSelection> {
        self.kind.checked_sub(1).and_then(|i| CARDS.get(i).copied())
    }
}
struct Candidate {
    action: Action,
    x: [f32; FEATURES],
    score: f32,
    ice: Option<IceActivation>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IceActivation {
    at: Option<i32>,
    impact: i32,
}
impl IceActivation {
    fn key(self) -> i32 {
        self.at.unwrap_or(prediction::NEVER)
    }
    fn status(self) -> &'static str {
        if self.at.is_none() {
            "cancelled_before_morph"
        } else if self.impact < prediction::NEVER {
            "early_hammer_activation"
        } else {
            "conditional_nominal"
        }
    }
}
type IceMemo = Option<(i32, Outcome)>;
enum Model {
    Hierarchical(Box<hierarchical::Scorer>),
    Linear(Vec<f32>, f32),
    Mlp {
        hidden: usize,
        w1: Vec<f32>,
        b1: Vec<f32>,
        w2: Vec<f32>,
        b2: f32,
    },
    Mlp2 {
        hidden1: usize,
        hidden2: usize,
        w1: Vec<f32>,
        b1: Vec<f32>,
        w2: Vec<f32>,
        b2: Vec<f32>,
        w3: Vec<f32>,
        b3: f32,
    },
}
impl Model {
    fn input_dim(&self) -> usize {
        match self {
            Self::Hierarchical(s) => s.position.input_dim(),
            Self::Linear(w, _) => w.len(),
            Self::Mlp { hidden, w1, .. } => w1.len() / hidden,
            Self::Mlp2 { hidden1, w1, .. } => w1.len() / hidden1,
        }
    }

    // The first 104 inputs are identical across a decision's candidates. Keep the
    // original accumulation order so cached inference is bit-for-bit equivalent.
    fn common_prefix(&self, x: &[f32]) -> Vec<f32> {
        let input = self.input_dim();
        match self {
            Self::Hierarchical(s) => s.position.common_prefix(x),
            Self::Linear(w, b) => vec![x[..104].iter().zip(&w[..104]).fold(*b, |s, (x, w)| s + x * w)],
            Self::Mlp { hidden, w1, b1, .. }
            | Self::Mlp2 {
                hidden1: hidden,
                w1,
                b1,
                ..
            } => (0..*hidden)
                .map(|h| {
                    x[..104]
                        .iter()
                        .zip(&w1[h * input..h * input + 104])
                        .fold(b1[h], |s, (x, w)| s + x * w)
                })
                .collect(),
        }
    }
    fn score_prefixed(&self, x: &[f32], prefix: &[f32]) -> f32 {
        let input = self.input_dim();
        match self {
            Self::Hierarchical(s) => s.position.score_prefixed(x, prefix),
            Self::Linear(w, _) => x[104..input]
                .iter()
                .zip(&w[104..])
                .fold(prefix[0], |s, (x, w)| s + x * w),
            Self::Mlp { hidden, w1, w2, b2, .. } => (0..*hidden).fold(*b2, |s, h| {
                let y = x[104..input]
                    .iter()
                    .zip(&w1[h * input + 104..(h + 1) * input])
                    .fold(prefix[h], |s, (x, w)| s + x * w);
                s + y.max(0.) * w2[h]
            }),
            Self::Mlp2 {
                hidden1,
                hidden2,
                w1,
                w2,
                b2,
                w3,
                b3,
                ..
            } => {
                let mut first = [0.; 256];
                for h in 0..*hidden1 {
                    first[h] = x[104..input]
                        .iter()
                        .zip(&w1[h * input + 104..(h + 1) * input])
                        .fold(prefix[h], |s, (x, w)| s + x * w)
                        .max(0.);
                }
                (0..*hidden2).fold(*b3, |s, h| {
                    let y = first[..*hidden1]
                        .iter()
                        .zip(&w2[h * hidden1..(h + 1) * hidden1])
                        .fold(b2[h], |s, (x, w)| s + x * w);
                    s + y.max(0.) * w3[h]
                })
            }
        }
    }
    fn parse(text: &str) -> Result<Self, String> {
        let mut tokens = text.split_whitespace();
        let format = tokens.next().ok_or("empty model")?;
        if format == "RSVZ_TREE_HIER_V1" {
            return hierarchical::parse_trees(tokens).map(|s| Self::Hierarchical(Box::new(s)));
        }
        if format == "RSVZ_HIER_V1" {
            return hierarchical::parse(tokens).map(|s| Self::Hierarchical(Box::new(s)));
        }
        let input: usize = tokens
            .next()
            .ok_or("missing input")?
            .parse()
            .map_err(|_| "invalid input")?;
        if input != FEATURES && input != local::FEATURES {
            return Err(format!("feature count {input}, expected 192 or 288"));
        }
        let hidden = if matches!(format, "RSVZ_MLP_V1" | "RSVZ_MLP2_V1") {
            tokens
                .next()
                .ok_or("missing hidden")?
                .parse::<usize>()
                .map_err(|_| "invalid hidden")?
        } else {
            0
        };
        let hidden2 = if format == "RSVZ_MLP2_V1" {
            tokens
                .next()
                .ok_or("missing second hidden")?
                .parse::<usize>()
                .map_err(|_| "invalid second hidden")?
        } else {
            0
        };
        if format != "RSVZ_MLP_V1" && format != "RSVZ_LINEAR_V1" && format != "RSVZ_MLP2_V1" {
            return Err("unknown model format".into());
        }
        if format != "RSVZ_LINEAR_V1"
            && (!(1..=256).contains(&hidden) || format == "RSVZ_MLP2_V1" && !(1..=256).contains(&hidden2))
        {
            return Err("hidden must be 1..256".into());
        }
        let values: Vec<f32> = tokens
            .map(|s| s.parse::<f32>().map_err(|_| "invalid weight".to_string()))
            .collect::<Result<_, _>>()?;
        let expected = if hidden == 0 {
            input + 1
        } else if hidden2 > 0 {
            hidden * input + hidden + hidden * hidden2 + hidden2 * 2 + 1
        } else {
            hidden * input + hidden * 2 + 1
        };
        if values.len() != expected || values.iter().any(|v| !v.is_finite()) {
            return Err(format!("invalid weights: expected {expected} finite values"));
        }
        if hidden == 0 {
            return Ok(Self::Linear(values[..input].to_vec(), values[input]));
        }
        let n = hidden * input;
        if hidden2 > 0 {
            let n2 = n + hidden + hidden * hidden2;
            return Ok(Self::Mlp2 {
                hidden1: hidden,
                hidden2,
                w1: values[..n].to_vec(),
                b1: values[n..n + hidden].to_vec(),
                w2: values[n + hidden..n2].to_vec(),
                b2: values[n2..n2 + hidden2].to_vec(),
                w3: values[n2 + hidden2..n2 + 2 * hidden2].to_vec(),
                b3: values[n2 + 2 * hidden2],
            });
        }
        Ok(Self::Mlp {
            hidden,
            w1: values[..n].to_vec(),
            b1: values[n..n + hidden].to_vec(),
            w2: values[n + hidden..n + 2 * hidden].to_vec(),
            b2: values[n + 2 * hidden],
        })
    }
    fn score(&self, x: &[f32]) -> f32 {
        let input = self.input_dim();
        match self {
            Self::Hierarchical(s) => s.position.score(x),
            Self::Linear(w, b) => x[..input].iter().zip(w).fold(*b, |s, (x, w)| s + x * w),
            Self::Mlp { hidden, w1, b1, w2, b2 } => (0..*hidden).fold(*b2, |s, h| {
                let y = x[..input]
                    .iter()
                    .zip(&w1[h * input..(h + 1) * input])
                    .fold(b1[h], |a, (x, w)| a + x * w);
                s + y.max(0.) * w2[h]
            }),
            Self::Mlp2 { .. } => self.score_prefixed(x, &self.common_prefix(x)),
        }
    }
}
pub struct Policy {
    pub mode: Mode,
    model: Option<Model>,
    pub decision: u64,
    rule_after: Option<u64>,
    pub wave_window: Option<(i32, i32)>,
    pub window_first_round_only: bool,
    window_wave: i32,
    last_window_active: Option<bool>,
    trace: Option<std::io::BufWriter<std::fs::File>>,
    trace_every: u64,
    trace_actions: bool,
    trace_start: u64,
    trace_end: u64,
    overrides: Vec<(u64, usize)>,
    pub round: u32,
    profile: bool,
    profile_ns: [u128; 5],
}
impl Default for Policy {
    fn default() -> Self {
        let mode = match std::env::var("SIX_POLICY").as_deref() {
            Ok("norm") => Mode::Norm,
            Ok("mlp") => Mode::Full,
            Ok("mlp-cheap") => Mode::Cheap,
            Ok("mlp-lite") => Mode::Lite,
            Ok("mlp-local") => Mode::Local,
            Ok("rule") | Err(_) => Mode::Rule,
            Ok(s) => panic!("unknown SIX_POLICY {s}"),
        };
        let model = std::env::var("SIX_POLICY_MODEL").ok().map(|p| {
            Model::parse(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("model {p}: {e}")))
                .unwrap_or_else(|e| panic!("model {p}: {e}"))
        });
        if model.as_ref().is_some_and(|m| matches!(m, Model::Hierarchical(_))) {
            assert!(mode == Mode::Lite, "hierarchical model requires mlp-lite");
        }
        if mode != Mode::Rule {
            if let Some(m) = &model {
                assert_eq!(
                    m.input_dim(),
                    if mode == Mode::Local { local::FEATURES } else { FEATURES },
                    "model dimension does not match policy feature mode"
                );
            }
        }
        if matches!(mode, Mode::Full | Mode::Cheap | Mode::Lite | Mode::Local) && model.is_none() {
            panic!("MLP mode requires SIX_POLICY_MODEL");
        }
        let trace = std::env::var("SIX_POLICY_TRACE").ok().map(|p| {
            std::io::BufWriter::new(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(process_path(&p))
                    .expect("policy trace"),
            )
        });
        let overrides = std::env::var("SIX_POLICY_OVERRIDES")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| {
                let (d, a) = s.split_once(':').expect("override decision:action");
                (d.parse().expect("decision"), a.parse().expect("action"))
            })
            .collect();
        Self {
            mode,
            model,
            decision: 0,
            rule_after: std::env::var("SIX_POLICY_RULE_AFTER_DECISION")
                .ok()
                .map(|v| v.parse().expect("rule-after decision index")),
            wave_window: std::env::var("SIX_POLICY_WAVE_WINDOW")
                .ok()
                .map(|v| parse_wave_window(&v).expect("wave window must be start:end within 1..20")),
            window_first_round_only: setting("SIX_POLICY_WINDOW_FIRST_ROUND_ONLY", 0) != 0,
            window_wave: 0,
            last_window_active: None,
            trace,
            trace_every: setting("SIX_POLICY_TRACE_EVERY", 100) as u64,
            trace_actions: setting("SIX_POLICY_TRACE_ACTIONS", 0) != 0,
            trace_start: setting("SIX_POLICY_TRACE_START", 0) as u64,
            trace_end: setting("SIX_POLICY_TRACE_END", u32::MAX) as u64,
            overrides,
            round: 0,
            profile: setting("SIX_POLICY_PROFILE", 0) != 0,
            profile_ns: [0; 5],
        }
    }
}
impl Policy {
    pub fn needs_plan(&self) -> bool {
        experience::plan_enabled()
            || self
                .model
                .as_ref()
                .is_some_and(|m| matches!(m,Model::Hierarchical(s) if s.requires_plan()))
    }
    pub fn enabled(&self) -> bool {
        learning_enabled(self.mode, self.decision, self.rule_after)
            && window_enabled(
                self.wave_window,
                self.window_wave,
                self.round,
                self.window_first_round_only,
            )
    }
    pub fn window_transition(&mut self, wave: i32) -> Option<bool> {
        self.window_wave = wave;
        self.wave_window?;
        let active = self.enabled();
        if self.last_window_active == Some(active) {
            return None;
        }
        self.last_window_active = Some(active);
        Some(active)
    }
    fn regime(&self) -> &'static str {
        if self.wave_window.is_some() {
            "model_window"
        } else if self.rule_after.is_some() {
            "model_then_rule"
        } else {
            "pure_model"
        }
    }
    pub fn choose(&mut self, planner: &mut planner::Planner, forecast: &Forecast, c: &Context) -> Action {
        let mut timer = self.profile.then(std::time::Instant::now);
        let scene = &planner.scene;
        let actions = legal_actions(scene, c);
        placement_audit::inspect(self.decision, &actions, c);
        let balloons = balloon_front(scene);
        let blover = blover_allowed(c, balloons);
        let mut global = Self::global(scene, c);
        global[31] = bit(blover);
        let model_prefix = self.model.as_ref().map(|m| m.common_prefix(&global));
        let full = matches!(self.mode, Mode::Full | Mode::Norm) && actions.len() > 1;
        // All observed ground actors share their baseline. Candidate interaction filters
        // only skip a provably unchanged prediction, never remove a legal candidate.
        if let Some(t) = timer {
            self.profile_ns[0] += t.elapsed().as_nanos();
            timer = Some(std::time::Instant::now());
        }
        let baseline = if full {
            planner.policy_baselines(forecast)
        } else {
            Vec::new()
        };
        let scene = &planner.scene;
        let (targets, bases): (Vec<Zombie>, Vec<Outcome>) = baseline.into_iter().unzip();
        let lite = if matches!(self.mode, Mode::Lite | Mode::Local) {
            Some(lite::Context::new(scene, forecast))
        } else {
            None
        };
        let norm_p = std::env::var("SIX_POLICY_NORM_P").unwrap_or_else(|_| "2".into());
        let norm_p = if norm_p == "max" {
            f32::INFINITY
        } else {
            norm_p.parse::<f32>().expect("norm p")
        };
        assert!(norm_p >= 1., "norm p must be >=1 or max");
        if let Some(t) = timer {
            self.profile_ns[1] += t.elapsed().as_nanos();
        }
        let mut ice_memo = vec![None; targets.len() * 2];
        let mut candidates = Vec::with_capacity(actions.len());
        for action in actions {
            timer = self.profile.then(std::time::Instant::now);
            let mut x = global;
            action_features(&mut x, action, scene, c, balloons);
            if let Some(t) = timer {
                self.profile_ns[2] += t.elapsed().as_nanos();
                timer = Some(std::time::Instant::now());
            }
            let ice = if full {
                prediction_features(&mut x, action, scene, forecast, &targets, &bases, norm_p, &mut ice_memo)
            } else {
                None
            };
            if let Some(lite) = &lite {
                lite.features(&mut x, action, scene, forecast);
            }
            if let Some(t) = timer {
                self.profile_ns[3] += t.elapsed().as_nanos();
                timer = Some(std::time::Instant::now());
            }
            let score = self.model.as_ref().map_or_else(
                || norm(&x),
                |m| {
                    if self.mode == Mode::Local {
                        m.score_prefixed(&local::expand(&x, action, scene, c), model_prefix.as_ref().unwrap())
                    } else {
                        m.score_prefixed(&x, model_prefix.as_ref().unwrap())
                    }
                },
            );
            if let Some(t) = timer {
                self.profile_ns[4] += t.elapsed().as_nanos();
            }
            candidates.push(Candidate { action, x, score, ice });
        }
        let hierarchy = match self.model.as_ref() {
            Some(Model::Hierarchical(s)) => Some(s.rank_with_plan(&candidates, c.plan.as_ref())),
            _ => None,
        };
        let forced = self
            .overrides
            .iter()
            .find(|(d, _)| *d == self.decision)
            .map(|(_, a)| *a);
        let selected = if let Some(id) = forced {
            candidates
                .iter()
                .position(|a| a.action.id() == id)
                .unwrap_or_else(|| panic!("override action {id} unavailable at decision {}", self.decision))
        } else if let Some(h) = &hierarchy {
            h.selected
        } else {
            candidates
                .iter()
                .enumerate()
                .max_by(|(i, a), (j, b)| a.score.total_cmp(&b.score).then(j.cmp(i)))
                .map(|(i, _)| i)
                .unwrap()
        };
        let (selected, explored, probability) = if forced.is_none() {
            explore_choice(
                selected,
                candidates.len(),
                setting("SIX_POLICY_EPSILON_PPM", 0),
                diagnostics::seed(),
                self.decision,
                setting("SIX_POLICY_EXPLORE_SEED", 0),
            )
        } else {
            (selected, false, 1.)
        };
        if experience::enabled() {
            assert!(self.mode == Mode::Lite, "replay supports mlp-lite192 only");
            let data = experience::wants(candidates[selected].action, rsvz::timing::clock()).then(|| {
                experience::encode_candidates(&candidates.iter().map(|v| (v.action, v.x)).collect::<Vec<_>>())
            });
            experience::prepare(
                replay_meta(
                    c,
                    candidates[selected].action,
                    false,
                    explored,
                    probability,
                    diagnostics::seed(),
                ),
                selected,
                data,
            );
        }
        if let Some(ice) = candidates[selected].ice {
            telemetry::record(
                "policy_ice_forecast",
                &[
                    self.decision as i64,
                    candidates[selected].action.id() as i64,
                    ice.at.map_or(-1, i64::from),
                    if ice.impact < prediction::NEVER {
                        ice.impact as i64
                    } else {
                        -1
                    },
                ],
            );
        }
        if self.trace_every > 0
            && self.decision >= self.trace_start
            && self.decision <= self.trace_end
            && (self.decision % self.trace_every == 0
                || forced.is_some()
                || self.trace_actions && candidates[selected].action.kind != 0)
        {
            let trace_regime = self.regime();
            if let Some(out) = &mut self.trace {
                write!(out,"{{\"schema\":{},\"prediction_revision\":4,\"clock\":{},\"seed\":{},\"round\":{},\"decision\":{},\"wave\":{},\"time\":{},\"selected\":{},\"forced\":{},\"regime\":\"{}\",\"window_start\":{},\"window_end\":{},\"first_round_only\":{},\"lite\":{},\"actions\":[",if self.mode==Mode::Local {3}else{2},rsvz::timing::clock(),diagnostics::seed(),c.round,self.decision,c.wave,c.time,candidates[selected].action.id(),forced.is_some(),trace_regime,self.wave_window.map_or(0,|w|w.0),self.wave_window.map_or(0,|w|w.1),self.window_first_round_only,matches!(self.mode,Mode::Lite|Mode::Local)).expect("trace");
                for (i, a) in candidates.iter().enumerate() {
                    if i > 0 {
                        write!(out, ",").expect("trace");
                    }
                    if self.mode == Mode::Local {
                        write!(
                            out,
                            "{{\"id\":{},\"score\":{},\"x\":{:?},\"ice_effect_at\":{},\"ice_impact\":{},\"ice_status\":\"{}\"}}",
                            a.action.id(),
                            a.score,
                            local::expand(&a.x, a.action, scene, c),json_time(a.ice.and_then(|v|v.at)),json_time(a.ice.map(|v|v.impact)),a.ice.map_or("not_resolved",|v|v.status())
                        )
                        .expect("trace");
                    } else {
                        write!(
                            out,
                            "{{\"id\":{},\"score\":{},\"x\":{:?},\"ice_effect_at\":{},\"ice_impact\":{},\"ice_status\":\"{}\"}}",
                            a.action.id(),
                            a.score,
                            a.x,json_time(a.ice.and_then(|v|v.at)),json_time(a.ice.map(|v|v.impact)),a.ice.map_or("not_resolved",|v|v.status())
                        )
                        .expect("trace");
                    }
                }
                if let Some(h) = &hierarchy {
                    writeln!(out,"],\"hierarchy\":{{\"selected_kind\":{},\"greedy_kind\":{},\"intent_dim\":{},\"intent_layout\":\"{}\",\"intent_backend\":\"{}\",\"model_family\":\"{}\",\"mask\":{:?},\"prior\":{:?},\"residual\":{:?},\"kind_scores\":{:?},\"intent_x\":{:?},\"base_scores\":{:?}}}}}",candidates[selected].action.kind,h.kind,h.input.len(),hierarchical::layout(h.input.len()),h.backend,h.family,h.mask,h.prior,h.residual,h.scores,h.input,h.base_scores).expect("trace");
                } else {
                    writeln!(out, "]}}").expect("trace");
                }
                out.flush().expect("trace");
            }
        }
        self.decision += 1;
        if self.profile && self.decision % 1000 == 0 {
            rsvz::log(
                rsvz::LogLevel::Info,
                format_args!(
                    "six_policy_profile seed={} decision={} ns={:?}",
                    diagnostics::seed(),
                    self.decision,
                    self.profile_ns
                ),
            );
        }
        candidates[selected].action
    }
    pub fn executed(&mut self, action: Action, success: bool) {
        experience::finish(success);
        telemetry::record(
            "policy_action",
            &[(self.decision - 1) as i64, action.id() as i64, i64::from(success)],
        );
        if self.rule_after == Some(self.decision - 1) {
            rsvz::log(
                rsvz::LogLevel::Info,
                format_args!(
                    "six_policy_continuation seed={} clock={} after_decision={} regime=rule state_reset=false",
                    diagnostics::seed(),
                    rsvz::timing::clock(),
                    self.decision - 1
                ),
            );
        }
        let trace_regime = self.regime();
        if let Some(out) = &mut self.trace {
            writeln!(
                out,
                "{{\"clock\":{},\"seed\":{},\"round\":{},\"execution\":{},\"action\":{},\"success\":{},\"reason\":\"{}\",\"regime\":\"{}\",\"window_start\":{},\"window_end\":{},\"first_round_only\":{}}}",
                rsvz::timing::clock(),
                diagnostics::seed(),
                self.round,
                self.decision - 1,
                action.id(),
                success,
                if success { "executed" } else { "executor_rejected" },trace_regime,self.wave_window.map_or(0,|w|w.0),self.wave_window.map_or(0,|w|w.1),self.window_first_round_only
            )
            .expect("trace");
        }
    }
    fn global(scene: &Scene, c: &Context) -> [f32; FEATURES] {
        let mut x = [0.; FEATURES];
        x[0] = 1.;
        x[1] = c.wave as f32 / 20.;
        x[2] = c.time as f32 / 6000.;
        x[3] = rsvz::board::sun() as f32 / 10000.;
        x[4] = (c.ice_due - c.time) as f32 / 6000.;
        x[5] = (c.doom_due - c.time) as f32 / 6000.;
        x[6] = bit(c.cherry_done);
        x[7] = bit(c.jala_done);
        x[8] = bit(c.pending_ice >= 0);
        x[9] = (scene.now - c.last_ice).min(10000) as f32 / 10000.;
        x[10] = c.round as f32 / 10.;
        for (i, card) in CARDS.iter().enumerate() {
            x[11 + i] = cd(*card) as f32 / 5000.;
            x[18 + i] = bit(ready(*card));
        }
        x[25] = bit(c.ice_allowed[0]);
        x[26] = bit(c.ice_allowed[1]);
        x[27] = bit(c.cherry);
        x[28] = bit(c.jala);
        x[29] = (c.wave - c.last_blover_wave) as f32 / 20.;
        x[30] = scene.zombies.len() as f32 / 100.;
        for row in 1..=6 {
            let offset = 32 + (row - 1) * 12;
            let zs: Vec<_> = scene.row_zombies(row as i32).collect();
            x[offset] = zs.len() as f32 / 20.;
            x[offset + 1] = zs.iter().map(|z| z.hp + z.armor).sum::<i32>() as f32 / 30000.;
            x[offset + 2] = zs.iter().map(|z| z.x).fold(1000., f32::min) / 1000.;
            for (i, test) in [
                Z::GigaGargantuar,
                Z::Gargantuar,
                Z::Football,
                Z::Imp,
                Z::Balloon,
                Z::Zomboni,
            ]
            .iter()
            .enumerate()
            {
                x[offset + 3 + i] = zs.iter().filter(|z| z.kind == *test).map(|z| z.x).fold(1000., f32::min) / 1000.;
            }
            x[offset + 9] = zs.iter().filter(|z| z.frozen > 0).count() as f32 / 20.;
            x[offset + 10] = scene
                .plants
                .iter()
                .filter(|p| p.pos.0 == row as i32 && p.kind == PuffShroom)
                .count() as f32
                / 9.;
            x[offset + 11] = scene
                .plants
                .iter()
                .filter(|p| p.pos.0 == row as i32 && matches!(p.kind, GloomShroom | TwinSunflower | UmbrellaLeaf))
                .map(|p| p.hp)
                .sum::<i32>() as f32
                / 3000.;
        }
        x
    }
}
fn learning_enabled(mode: Mode, decision: u64, rule_after: Option<u64>) -> bool {
    mode != Mode::Rule && rule_after.is_none_or(|last| decision <= last)
}
fn parse_wave_window(value: &str) -> Result<(i32, i32), ()> {
    let (a, b) = value.split_once(':').ok_or(())?;
    let a = a.parse().map_err(|_| ())?;
    let b = b.parse().map_err(|_| ())?;
    if (1..=20).contains(&a) && (a..=20).contains(&b) {
        Ok((a, b))
    } else {
        Err(())
    }
}
fn window_enabled(window: Option<(i32, i32)>, wave: i32, round: u32, first_only: bool) -> bool {
    window.is_none_or(|(a, b)| (!first_only || round == 0) && (a..=b).contains(&wave))
}
fn balloon_front(scene: &Scene) -> f32 {
    scene
        .zombies
        .iter()
        .filter(|z| z.kind == Z::Balloon && z.phase == ZombiePhase::BalloonFlying)
        .map(|z| z.x)
        .fold(f32::INFINITY, f32::min)
}
fn blover_allowed(c: &Context, x: f32) -> bool {
    x.is_finite()
        && (c.wave >= 2 || x + 100. <= 0.37 * 80.)
        && (c.wave >= c.last_blover_wave + 2 || matches!(c.wave, 9 | 19 | 20) || x + 100. <= 0.37 * 80.)
}
fn legal_actions(scene: &Scene, c: &Context) -> Vec<Action> {
    let mut actions = vec![Action { kind: 0, pos: (0, 0) }];
    let balloons = scene
        .zombies
        .iter()
        .filter(|z| z.kind == Z::Balloon && z.phase == ZombiePhase::BalloonFlying)
        .map(|z| z.x)
        .fold(f32::INFINITY, f32::min);
    let blover = balloons.is_finite()
        && (c.wave >= 2 || balloons + 100. <= 0.37 * 80.)
        && (c.wave >= c.last_blover_wave + 2 || matches!(c.wave, 9 | 19 | 20) || balloons + 100. <= 0.37 * 80.);
    for kind in 1..=7 {
        if !ready(CARDS[kind - 1])
            || !match kind {
                2 => blover,
                4 => c.cherry,
                5 => c.jala,
                6 => c.ice_allowed[0],
                7 => c.ice_allowed[1],
                _ => true,
            }
        {
            continue;
        }
        for row in 1..=6 {
            for col in 1..=9 {
                let pos = (row, col);
                // Remove only disposable Puff, and supply a legal Lily where required.
                if combat::candidate(CARDS[kind - 1], pos)
                    && !(kind == 1 && main_kind(pos) == Some(PuffShroom))
                    && rsvz::board::sun() as i32
                        >= card_cost(kind)
                            + if matches!(row, 3 | 4) && hp(LilyPad, pos).is_none() {
                                25
                            } else {
                                0
                            }
                {
                    actions.push(Action { kind, pos });
                }
            }
        }
    }
    if c.cherry {
        actions.push(Action { kind: 8, pos: (0, 0) });
    }
    if c.jala {
        actions.push(Action { kind: 9, pos: (0, 0) });
    }
    if c.ice_required && actions.iter().any(|a| matches!(a.kind, 6 | 7)) {
        actions.retain(|a| matches!(a.kind, 6 | 7));
    }
    actions
}
fn prediction_features(
    x: &mut [f32; FEATURES], action: Action, scene: &Scene, forecast: &Forecast, targets: &[Zombie], bases: &[Outcome],
    norm_p: f32, ice_memo: &mut [IceMemo],
) -> Option<IceActivation> {
    let mut activation = None;
    let base_risk: Vec<f32> = bases.iter().map(|o| risk(*o)).collect();
    let before_norm = pnorm(&base_risk, norm_p);
    x[144] = targets.len() as f32 / scene.zombies.len().max(1) as f32;
    x[145] = before_norm;
    let mut risks = base_risk.clone();
    if action.pos.0 > 0 {
        let kind = match action.card().unwrap() {
            CardSelection::Plant(k) => k,
            CardSelection::Imitator(_) => Imitator,
        };
        let mut pad = Pad::new(action.pos, kind);
        pad.order = scene.next_plant_order;
        pad.trigger_window = prediction::HORIZON;
        let mut squash_predictor = squash::Predictor::default();
        let cleared_scene = if scene.main(action.pos).is_some_and(|p| p.kind == PuffShroom) {
            let mut s = scene.clone();
            s.plants.retain(|p| p.pos != action.pos || p.kind != PuffShroom);
            Some(s)
        } else {
            None
        };
        let scene = cleared_scene.as_ref().unwrap_or(scene);
        let (f, ice) = resolved_candidate_forecast(scene, forecast, action, targets, &mut pad);
        activation = ice;
        x[151] = 1.;
        x[152] = 1.;
        for (i, (&z, &base)) in targets.iter().zip(bases).enumerate() {
            let affected = match action.kind {
                4 => (z.row - action.pos.0).abs() <= 1,
                5 => z.row == action.pos.0,
                6 | 7 => true,
                _ => z.row == action.pos.0 && prediction::could_contact_pad(z, pad),
            };
            if !affected {
                continue;
            }
            let mut target_pad = pad;
            if action.kind == 3 {
                let proof = squash_predictor
                    .evaluate(scene, &f, action.pos, &[z.id], false)
                    .or_else(|| squash_predictor.prestore(scene, &f, action.pos, &[z.id]));
                let Some(choice) = proof else {
                    x[144] -= 1. / scene.zombies.len().max(1) as f32;
                    continue;
                };
                target_pad.squash_wait = choice.wait;
                target_pad.lifetime = choice.wait + 126;
                target_pad.squash_victim = Some(z.id);
            }
            let after = if matches!(action.kind, 6 | 7) && z.row != action.pos.0 {
                let slot = (action.kind - 6) * targets.len() + i;
                let key = ice.expect("resolved ice candidate").key();
                if let Some((cached_key, outcome)) = ice_memo[slot].filter(|(cached_key, _)| *cached_key == key) {
                    debug_assert_eq!(cached_key, key);
                    outcome
                } else {
                    let outcome = predict(z, scene, &f, None);
                    ice_memo[slot] = Some((key, outcome));
                    outcome
                }
            } else {
                predict(
                    z,
                    scene,
                    &f,
                    if z.row == action.pos.0 { Some(target_pad) } else { None },
                )
            };
            risks[i] = risk(after);
            x[147] += base.hp_at_end as f32 / 30000.;
            x[148] += after.hp_at_end as f32 / 30000.;
            x[150] += (base.hp_at_end - after.hp_at_end) as f32 / 30000.;
            x[151] = x[151].min(bounded_time(after.trigger));
            x[152] = x[152].min(bounded_time(after.impact));
            x[153] += bit(after.squash_hit) / 10.;
            x[154] += bit(base.safe()) / 10.;
            x[155] += bit(after.safe()) / 10.;
            x[156] += bit(!base.safe() && after.safe()) / 10.;
            x[157] += (after.gloom_damage - base.gloom_damage) as f32 / 30000.;
            x[158] += bit(after.trigger < 751) / 10.;
            x[159] += z.hp as f32 / 30000.;
        }
    }
    x[146] = pnorm(&risks, norm_p);
    x[149] = x[145] - x[146];
    // Signed coverage distinguishes conditional planned effects from observed ones.
    // Candidate placement is legal, but successful activation is not a guarantee.
    x[144] = x[144].clamp(0., 1.);
    if matches!(action.kind, 4..=7) || forecast.events.iter().any(|e| !e.certain) {
        x[144] = -x[144];
    }
    activation
}

/// Resolve this candidate's one global ice event before evaluating any lane.
/// A short copy-consumption/hammer probe is not a placement safety mask.
fn resolved_candidate_forecast(
    scene: &Scene, forecast: &Forecast, action: Action, targets: &[Zombie], pad: &mut Pad,
) -> (Forecast, Option<IceActivation>) {
    let mut f = candidate_forecast(forecast, action);
    let lead = match action.kind {
        6 => 100,
        7 => 420,
        _ => return (f, None),
    };
    let index = f
        .events
        .iter()
        .rposition(|e| e.kind == prediction::EffectKind::Ice && !e.certain && e.pos == action.pos && e.at == lead)
        .expect("candidate ice event");
    let candidate = f.events.remove(index);
    let mut probe = *pad;
    probe.complete_outcome = true;
    probe.trigger_window = lead;
    let mut impact = prediction::NEVER;
    for &z in targets.iter().filter(|z| z.row == action.pos.0) {
        let out = prediction::simulate_until(z, scene, &f, Some(probe), lead);
        telemetry::work_prediction(1, out);
        impact = impact.min(out.impact);
    }
    let at = if action.kind == 7 && impact < 320 {
        None
    } else {
        Some(lead.min(impact.saturating_add(1)))
    };
    if impact < lead {
        // Keep the plant through the impact update; all other actors see it gone
        // on the following update. The impacting actor still records its hit.
        pad.lifetime = pad.lifetime.min(impact + 1);
    }
    if let Some(at) = at {
        let mut candidate = candidate;
        candidate.at = at;
        f.events.push(candidate);
        f.events.sort_by_key(|e| e.at);
    }
    (f, Some(IceActivation { at, impact }))
}

/// The fixed ice deadline governs time; the scorer still owns card and cell.
pub fn ice_window(request: bool, target: i32, time: i32, cooldowns: [i32; 2]) -> ([bool; 2], bool) {
    let earliest = (cooldowns[0] + 100).min(cooldowns[1] + 420);
    let latest_lead = (target - time).max(earliest) + 5;
    (
        [
            request && time + 100 >= target && 100 <= latest_lead,
            request && time + 420 >= target && 420 <= latest_lead,
        ],
        request && time + earliest >= target,
    )
}
fn candidate_forecast(forecast: &Forecast, action: Action) -> Forecast {
    let mut f = Forecast::default();
    f.events = forecast.events.clone();
    if let Some((effect, lead)) = match action.kind {
        4 => Some((prediction::EffectKind::Cherry, 100)),
        5 => Some((prediction::EffectKind::Jalapeno, 100)),
        6 => Some((prediction::EffectKind::Ice, 100)),
        7 => Some((prediction::EffectKind::Ice, 420)),
        _ => None,
    } {
        if matches!(action.kind, 6 | 7) {
            if let Some(index) = f
                .events
                .iter()
                .position(|e| e.kind == prediction::EffectKind::Ice && !e.certain)
            {
                f.events.remove(index);
            }
        }
        f.add(lead, effect, action.pos, false);
        f.events.sort_by_key(|e| e.at);
    }
    f
}
fn risk(o: Outcome) -> f32 {
    if o.safe() {
        0.
    } else {
        1. - bounded_time(o.danger) + 0.2 * (o.hp_at_end.max(0) as f32 / 6000.)
    }
}
fn pnorm(values: &[f32], p: f32) -> f32 {
    if p.is_infinite() {
        values.iter().copied().fold(0., f32::max)
    } else {
        values.iter().map(|v| v.powf(p)).sum::<f32>().powf(1. / p)
    }
}
fn json_time(time: Option<i32>) -> String {
    time.filter(|&t| t < prediction::NEVER)
        .map_or_else(|| "null".into(), |t| t.to_string())
}
fn bounded_time(t: i32) -> f32 {
    t.min(6000) as f32 / 6000.
}
fn bit(value: bool) -> f32 {
    if value { 1. } else { 0. }
}
fn predict(z: Zombie, s: &Scene, f: &Forecast, p: Option<Pad>) -> Outcome {
    let o = prediction::simulate_action(z, s, f, p);
    telemetry::work_prediction(if p.is_some() { 1 } else { 0 }, o);
    o
}
fn card_cost(kind: usize) -> i32 {
    match kind {
        2 => 100,
        3 => 50,
        4 => 150,
        5 => 125,
        6 | 7 => 75,
        _ => 0,
    }
}
fn action_features(x: &mut [f32; FEATURES], a: Action, s: &Scene, c: &Context, balloons: f32) {
    x[104 + a.kind] = 1.;
    x[114] = a.pos.0 as f32 / 6.;
    x[115] = a.pos.1 as f32 / 9.;
    x[116] = card_cost(a.kind) as f32 / 150.;
    x[117] = bit(a.pos.0 == 3 || a.pos.0 == 4);
    x[118] = bit(a.pos.0 > 0 && main_kind(a.pos) == Some(PuffShroom));
    x[119] = bit(x[117] > 0. && hp(LilyPad, a.pos).is_none());
    x[120] = bit(matches!(a.kind, 6 | 7));
    x[121] = bit(a.kind == 2) * (1. - balloons.min(1000.) / 1000.);
    x[122] = bit(matches!(a.kind, 4 | 5)) * c.time as f32 / 6000.;
    if a.pos.0 > 0 {
        x[160 + a.pos.0 as usize - 1] = 1.;
        x[166 + a.pos.1 as usize - 1] = 1.;
        let start = 32 + (a.pos.0 as usize - 1) * 12;
        for i in 0..12 {
            x[175 + i] = x[start + i];
        }
        x[189] = s
            .plants
            .iter()
            .filter(|p| p.pos.0 == a.pos.0 && p.kind == Squash)
            .count() as f32
            / 9.;
        x[190] = s
            .plants
            .iter()
            .filter(|p| matches!(p.kind, IceShroom | Imitator))
            .map(|p| p.effect)
            .min()
            .unwrap_or(5000) as f32
            / 5000.;
        x[191] = s
            .plants
            .iter()
            .filter(|p| p.pos.0 == a.pos.0 && p.kind == PuffShroom)
            .map(|p| (p.pos.1 - a.pos.1) as f32 / 9.)
            .min_by(|a, b| a.abs().total_cmp(&b.abs()).then(a.total_cmp(b)))
            .unwrap_or(1.);
    }
    x[187] = ((if c.wave == 20 { 500 } else { 2300 }) - c.time) as f32 / 6000.;
    x[188] = ((if c.wave == 20 { 700 } else { 2400 }) - c.time) as f32 / 6000.;
    if a.pos.0 == 0 {
        return;
    }
    let px = (a.pos.1 * 80 - 40) as f32;
    let zs: Vec<_> = s.row_zombies(a.pos.0).collect();
    if let Some(z) = zs.iter().min_by(|a, b| {
        (a.x - px)
            .abs()
            .total_cmp(&(b.x - px).abs())
            .then(a.x.total_cmp(&b.x))
            .then((a.kind as i32).cmp(&(b.kind as i32)))
            .then(a.hp.cmp(&b.hp))
            .then((a.phase as i32).cmp(&(b.phase as i32)))
            .then(a.progress.total_cmp(&b.progress))
            .then(a.frozen.cmp(&b.frozen))
            .then(a.eating.cmp(&b.eating))
            .then(a.speed.total_cmp(&b.speed))
    }) {
        x[123] = (z.x - px) / 1000.;
        x[124] = z.hp as f32 / 6000.;
        x[125] = bit(z.giant());
        x[126] = bit(z.frozen > 0);
        x[127] = if z.frozen > 0 { 0. } else { z.speed };
        x[128] = bit(z.phase == ZombiePhase::GargantuarSmashing);
        x[129] = z.progress;
        x[130] = bit(z.eating);
    }
    x[131] = zs.iter().filter(|z| (z.x - px).abs() < 150.).count() as f32 / 10.;
    x[132] = zs.iter().filter(|z| z.x >= px && z.x < px + 100.).count() as f32 / 10.;
    x[133] = s
        .zombies
        .iter()
        .filter(|z| (z.row - a.pos.0).abs() <= 1 && (z.x - px).abs() < 150.)
        .map(|z| z.hp.min(1800))
        .sum::<i32>() as f32
        / 18000.;
    x[134] = zs.iter().map(|z| z.hp.min(1800)).sum::<i32>() as f32 / 18000.;
    x[135] = s
        .plants
        .iter()
        .filter(|p| p.pos.0 == a.pos.0 && p.kind == GloomShroom)
        .count() as f32
        / 3.;
    x[136] = bit(a.kind == 1) * x[132];
    x[137] = bit(a.kind == 3) * x[131];
    x[138] = bit(a.kind == 4) * x[133];
    x[139] = bit(a.kind == 5) * x[134];
    x[140] = bit(matches!(a.kind, 6 | 7)) * x[132];
    x[141] = bit(a.kind == 3) * (x[124] + x[125]);
    x[142] = bit(a.kind == 1) * (1. - x[115]);
    x[143] = bit(a.kind == 2) * x[132];
}
fn norm(x: &[f32; FEATURES]) -> f32 {
    // Explicit, trainable comparison baseline; these preferences are scores, never legality.
    0.02 * x[104] - 0.03 * x[105] - 0.15 * x[106] - 0.25 * x[107] - 0.4 * x[108] - 0.4 * x[109]
        + 2. * x[120]
        + 2. * x[121]
        + 2. * x[136]
        + 3. * x[137]
        + 4. * x[138]
        + 4. * x[139]
        + 5. * x[149]
        + 2. * x[150]
        + 4. * x[156]
        + 0.1 * x[158]
        - 0.05 * x[118]
        - 0.1 * x[119]
}
#[cfg(test)]
mod tests {
    use super::*;
    fn ice_test_zombie(row: i32, x: f32) -> Zombie {
        use rsvz::core::model::{ContactRect, ZombieId};
        Zombie {
            age: 0,
            id: ZombieId::from_raw(65537 + row as u32),
            kind: Z::GigaGargantuar,
            row,
            wave: 3,
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
    #[test]
    fn copy_destroyed_at_134_has_no_global_ice_and_nominal_cell_keeps_420() {
        let mut scene = Scene::default();
        let attacker = ice_test_zombie(5, 460.);
        let remote = ice_test_zombie(1, 500.);
        scene.zombies = vec![attacker, remote];
        let mut forecast = Forecast::default();
        forecast.add(420, prediction::EffectKind::Ice, (5, 1), false);
        let mut bad_pad = Pad::new((5, 6), Imitator);
        let (bad, activation) = resolved_candidate_forecast(
            &scene,
            &forecast,
            Action { kind: 7, pos: (5, 6) },
            &scene.zombies,
            &mut bad_pad,
        );
        assert_eq!(activation, Some(IceActivation { at: None, impact: 134 }));
        assert_eq!(bad_pad.lifetime, 135);
        assert!(!bad.events.iter().any(|e| e.kind == prediction::EffectKind::Ice));
        let remote_bad = prediction::simulate_action(remote, &scene, &bad, None);
        assert_eq!(remote_bad.hp_at_end, 6000);
        let local_bad = prediction::simulate_action(attacker, &scene, &bad, Some(bad_pad));
        assert_eq!(local_bad.impact, 134);
        assert_eq!(local_bad.hp_at_end, 6000);
        let mut nominal_pad = Pad::new((5, 1), Imitator);
        let (nominal, activation) = resolved_candidate_forecast(
            &scene,
            &forecast,
            Action { kind: 7, pos: (5, 1) },
            &scene.zombies,
            &mut nominal_pad,
        );
        assert_eq!(
            activation,
            Some(IceActivation {
                at: Some(420),
                impact: prediction::NEVER
            })
        );
        assert_eq!(
            prediction::simulate_action(remote, &scene, &nominal, None).hp_at_end,
            5980
        );
    }
    #[test]
    fn copy_consumed_before_morph_cancels_but_ice_hammers_advance_globally() {
        let mut eater = ice_test_zombie(5, 200.);
        eater.kind = Z::Football;
        eater.speed = 0.68;
        let mut scene = Scene::default();
        scene.zombies = vec![eater];
        let (_, ate) = resolved_candidate_forecast(
            &scene,
            &Forecast::default(),
            Action { kind: 7, pos: (5, 3) },
            &scene.zombies,
            &mut Pad::new((5, 3), Imitator),
        );
        assert_eq!(ate, Some(IceActivation { at: None, impact: 297 }));
        let mut hammer = ice_test_zombie(5, 460.);
        hammer.phase = ZombiePhase::GargantuarSmashing;
        hammer.progress = 0.642;
        scene.zombies = vec![hammer];
        let (awake, early) = resolved_candidate_forecast(
            &scene,
            &Forecast::default(),
            Action { kind: 6, pos: (5, 6) },
            &scene.zombies,
            &mut Pad::new((5, 6), IceShroom),
        );
        assert_eq!(early, Some(IceActivation { at: Some(2), impact: 1 }));
        assert_eq!(awake.events[0].at, 2);
        hammer.frozen = 320;
        hammer.progress = 0.62;
        scene.zombies = vec![hammer];
        let (morphed, early) = resolved_candidate_forecast(
            &scene,
            &Forecast::default(),
            Action { kind: 7, pos: (5, 6) },
            &scene.zombies,
            &mut Pad::new((5, 6), Imitator),
        );
        let early = early.unwrap();
        assert!((320..420).contains(&early.impact));
        assert_eq!(early.at, Some(early.impact + 1));
        assert_eq!(morphed.events[0].at, early.impact + 1);
    }
    #[test]
    fn global_ice_memo_distinguishes_cancelled_and_realized_candidates_in_both_orders() {
        let mut scene = Scene::default();
        scene.zombies = vec![ice_test_zombie(5, 460.), ice_test_zombie(1, 500.)];
        let mut f = Forecast::default();
        f.add(420, prediction::EffectKind::Ice, (5, 1), false);
        let bases: Vec<_> = scene
            .zombies
            .iter()
            .map(|&z| prediction::simulate_action(z, &scene, &f, None))
            .collect();
        let bad = Action { kind: 7, pos: (5, 6) };
        let nominal = Action { kind: 7, pos: (5, 1) };
        for actions in [[bad, nominal], [nominal, bad]] {
            let mut memo = vec![None; 4];
            for action in actions {
                let mut cached = [0.; FEATURES];
                let mut fresh = cached;
                let result =
                    prediction_features(&mut cached, action, &scene, &f, &scene.zombies, &bases, 2., &mut memo)
                        .unwrap();
                let _ = prediction_features(
                    &mut fresh,
                    action,
                    &scene,
                    &f,
                    &scene.zombies,
                    &bases,
                    2.,
                    &mut [None; 4],
                );
                assert_eq!(cached, fresh);
                assert_eq!(memo[3].unwrap().0, result.key());
                assert_eq!(
                    memo[3].unwrap().1.hp_at_end,
                    if action.id() == bad.id() { 6000 } else { 5980 }
                );
            }
        }
    }
    #[test]
    fn early_ice_is_global_and_does_not_reuse_nominal_memo_entry() {
        let mut attacker = ice_test_zombie(5, 460.);
        attacker.phase = ZombiePhase::GargantuarSmashing;
        attacker.progress = 0.642;
        let mut scene = Scene::default();
        scene.zombies = vec![attacker, ice_test_zombie(1, 500.)];
        let mut f = Forecast::default();
        f.add(100, prediction::EffectKind::Ice, (5, 1), false);
        let bases: Vec<_> = scene
            .zombies
            .iter()
            .map(|&z| prediction::simulate_action(z, &scene, &f, None))
            .collect();
        let mut memo = vec![None; 4];
        for (action, at) in [
            (Action { kind: 6, pos: (5, 1) }, 100),
            (Action { kind: 6, pos: (5, 6) }, 2),
        ] {
            let mut cached = [0.; FEATURES];
            let mut fresh = cached;
            let activation =
                prediction_features(&mut cached, action, &scene, &f, &scene.zombies, &bases, 2., &mut memo).unwrap();
            let _ = prediction_features(
                &mut fresh,
                action,
                &scene,
                &f,
                &scene.zombies,
                &bases,
                2.,
                &mut [None; 4],
            );
            assert_eq!(activation.at, Some(at));
            assert_eq!(memo[1].unwrap().0, at);
            assert_eq!(cached, fresh);
            assert_eq!(memo[1].unwrap().1.hp_at_end, 5980);
        }
    }
    #[test]
    fn zero_extended_local_model_preserves_192_scores_bit_for_bit() {
        let hidden1 = 4;
        let hidden2 = 3;
        let w1: Vec<f32> = (0..hidden1 * FEATURES)
            .map(|i| ((i * 31 % 113) as f32 - 56.) / 71.)
            .collect();
        let mut extended = Vec::new();
        for row in w1.chunks_exact(FEATURES) {
            extended.extend_from_slice(row);
            extended.resize(extended.len() + local::FEATURES - FEATURES, 0.);
        }
        let b1 = vec![0.2, -0.1, 0.7, 0.3];
        let w2: Vec<f32> = (0..hidden1 * hidden2).map(|i| (i as f32 - 5.) / 7.).collect();
        let b2 = vec![0.1, 0.2, 0.3];
        let w3 = vec![0.4, -0.6, 0.9];
        let b3 = 0.173;
        let old = Model::Mlp2 {
            hidden1,
            hidden2,
            w1,
            b1: b1.clone(),
            w2: w2.clone(),
            b2: b2.clone(),
            w3: w3.clone(),
            b3,
        };
        let mut values = extended;
        values.extend(b1);
        values.extend(w2);
        values.extend(b2);
        values.extend(w3);
        values.push(b3);
        let text = format!(
            "RSVZ_MLP2_V1 288 4 3 {}",
            values.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ")
        );
        let new = Model::parse(&text).unwrap();
        assert_eq!(new.input_dim(), 288);
        for candidate in 0..54 {
            let x: [f32; FEATURES] = std::array::from_fn(|i| ((i * 39 + candidate * 71) % 163) as f32 / 91.);
            let mut local: [f32; local::FEATURES] = std::array::from_fn(|i| i as f32 / 313.);
            local[..FEATURES].copy_from_slice(&x);
            assert_eq!(old.score(&x).to_bits(), new.score(&local).to_bits());
            assert_eq!(
                old.score_prefixed(&x, &old.common_prefix(&x)).to_bits(),
                new.score_prefixed(&local, &new.common_prefix(&x)).to_bits()
            );
        }
    }
    #[test]
    fn optional_wave_window_has_inclusive_bounds_and_no_default_effect() {
        assert_eq!(parse_wave_window("10:12"), Ok((10, 12)));
        for bad in ["0:12", "12:10", "10:21", "10", "10:12:13"] {
            assert!(parse_wave_window(bad).is_err());
        }
        for round in 0..3 {
            for wave in 0..=20 {
                assert!(window_enabled(None, wave, round, false));
                assert_eq!(
                    window_enabled(Some((10, 12)), wave, round, false),
                    (10..=12).contains(&wave)
                );
                assert_eq!(
                    window_enabled(Some((10, 12)), wave, round, true),
                    round == 0 && (10..=12).contains(&wave)
                );
            }
        }
    }
    #[test]
    fn second_identity_layer_preserves_single_layer_score_bits() {
        let hidden = 64;
        let w1: Vec<f32> = (0..hidden * FEATURES)
            .map(|i| ((i * 731 % 199) as f32 - 99.) / 113.)
            .collect();
        let b1: Vec<f32> = (0..hidden).map(|i| i as f32 / 17.).collect();
        let output: Vec<f32> = (0..hidden).map(|i| (i as f32 - 20.) / 97.).collect();
        let old = Model::Mlp {
            hidden,
            w1: w1.clone(),
            b1: b1.clone(),
            w2: output.clone(),
            b2: 0.173,
        };
        let new = Model::Mlp2 {
            hidden1: hidden,
            hidden2: hidden,
            w1,
            b1,
            w2: (0..hidden * hidden).map(|i| bit(i / hidden == i % hidden)).collect(),
            b2: vec![0.; hidden],
            w3: output,
            b3: 0.173,
        };
        for candidate in 0..54 {
            let x: [f32; FEATURES] = std::array::from_fn(|i| ((i * 39 + candidate * 71) % 163) as f32 / 91.);
            assert_eq!(old.score(&x).to_bits(), new.score(&x).to_bits());
            assert_eq!(
                old.score_prefixed(&x, &old.common_prefix(&x)).to_bits(),
                new.score_prefixed(&x, &new.common_prefix(&x)).to_bits()
            );
        }
    }
    #[test]
    fn two_layer_format_checks_dimensions_and_matches_numeric_reference() {
        assert!(Model::parse("RSVZ_MLP2_V1 192 64 0").is_err());
        assert!(Model::parse("RSVZ_MLP2_V1 192 64 257").is_err());
        let mut weights = vec![0.; FEATURES * 3];
        weights[0] = 0.5;
        weights[1] = 0.25;
        weights[FEATURES] = -1.;
        weights[FEATURES + 1] = 2.;
        weights[FEATURES * 2 + 1] = 0.25;
        weights.extend([0.1, 0., 0.5, 2., -1., 1., -1., 3., 0.5, -0.5, 0.1, 1.5, -2., 0.2]);
        let text = format!(
            "RSVZ_MLP2_V1 192 3 2 {}",
            weights.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ")
        );
        let model = Model::parse(&text).unwrap();
        let mut x = [0.; FEATURES];
        x[0] = 2.;
        x[1] = -1.;
        assert!((model.score(&x) - 2.375).abs() < 0.000001);
        assert!(Model::parse(&(text + " 0")).is_err());
    }
    #[test]
    fn rule_continuation_starts_only_after_the_requested_decision() {
        assert!(learning_enabled(Mode::Cheap, 0, Some(0)));
        assert!(!learning_enabled(Mode::Cheap, 1, Some(0)));
        assert!(learning_enabled(Mode::Full, 100, Some(100)));
        assert!(!learning_enabled(Mode::Full, 101, Some(100)));
        assert!(learning_enabled(Mode::Cheap, u64::MAX, None));
        assert!(!learning_enabled(Mode::Rule, 0, Some(100)));
    }
    #[test]
    fn squash_does_not_invent_damage_for_an_unproven_distant_victim() {
        use rsvz::core::model::{ContactRect, ZombieId};
        let mut scene = Scene::default();
        let z = Zombie {
            age: 0,
            id: ZombieId::from_raw(65537),
            kind: Z::GigaGargantuar,
            row: 6,
            wave: 1,
            carried: false,
            x: 800.,
            hp: 6000,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.37,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(783, 0, 908, 154),
        };
        scene.zombies.push(z);
        let base = Outcome {
            danger: 1000,
            hp_at_end: 6000,
            ..Outcome::default()
        };
        let mut x = [0.; FEATURES];
        prediction_features(
            &mut x,
            Action { kind: 3, pos: (6, 1) },
            &scene,
            &Forecast::default(),
            &[z],
            &[base],
            2.,
            &mut [None; 2],
        );
        assert_eq!(x[153], 0.);
        assert_eq!(x[149], 0.);
        assert_eq!(x[144], 0.);
    }
    #[test]
    fn same_ice_effect_on_other_rows_is_cached_without_changing_features() {
        use rsvz::core::model::{ContactRect, ZombieId};
        let mut scene = Scene::default();
        let z = Zombie {
            age: 0,
            id: ZombieId::from_raw(65537),
            kind: Z::GigaGargantuar,
            row: 6,
            wave: 1,
            carried: false,
            x: 800.,
            hp: 6000,
            armor: 0,
            phase: ZombiePhase::ZombieNormal,
            progress: 0.,
            speed: 0.37,
            frozen: 0,
            chilled: 0,
            object: false,
            eating: false,
            rect: ContactRect::new(783, 0, 908, 154),
        };
        scene.zombies.push(z);
        let base = Outcome {
            danger: 1000,
            hp_at_end: 6000,
            ..Outcome::default()
        };
        let mut a = [0.; FEATURES];
        let mut b = a;
        let mut uncached = a;
        let mut memo = [None; 2];
        prediction_features(
            &mut a,
            Action { kind: 6, pos: (1, 1) },
            &scene,
            &Forecast::default(),
            &[z],
            &[base],
            2.,
            &mut memo,
        );
        assert!(memo[0].is_some());
        prediction_features(
            &mut b,
            Action { kind: 6, pos: (2, 4) },
            &scene,
            &Forecast::default(),
            &[z],
            &[base],
            2.,
            &mut memo,
        );
        prediction_features(
            &mut uncached,
            Action { kind: 6, pos: (2, 4) },
            &scene,
            &Forecast::default(),
            &[z],
            &[base],
            2.,
            &mut [None; 2],
        );
        assert_eq!(a, b);
        assert_eq!(b, uncached);
    }
    #[test]
    fn shared_global_inference_preserves_every_score_bit() {
        for hidden in [32, 64, 128] {
            let model = Model::Mlp {
                hidden,
                w1: (0..hidden * FEATURES)
                    .map(|i| ((i * 731 % 199) as f32 - 99.) / 113.)
                    .collect(),
                b1: (0..hidden).map(|i| i as f32 / 17.).collect(),
                w2: (0..hidden).map(|i| (i as f32 - 20.) / 97.).collect(),
                b2: 0.173,
            };
            let mut x: [f32; FEATURES] = std::array::from_fn(|i| ((i * 31 % 113) as f32 - 57.) / 71.);
            let prefix = model.common_prefix(&x);
            for candidate in 0..54 {
                for i in 104..FEATURES {
                    x[i] = ((i * 39 + candidate * 71) % 163) as f32 / 91.;
                }
                assert_eq!(model.score(&x).to_bits(), model.score_prefixed(&x, &prefix).to_bits());
            }
        }
    }
    #[test]
    fn fixed_ice_deadline_allows_card_choice_but_not_infinite_wait() {
        assert_eq!(ice_window(true, 830, 410, [0, 0]), ([false, true], false));
        assert_eq!(ice_window(true, 830, 730, [0, 0]), ([true, false], true));
        assert_eq!(ice_window(true, 830, 410, [1000, 0]), ([false, true], true));
        assert_eq!(ice_window(true, 830, 900, [0, 0]), ([true, false], true));
        assert_eq!(ice_window(false, 830, 900, [0, 0]), ([false, false], false));
    }
    #[test]
    fn candidate_ice_replaces_only_current_uncertain_plan() {
        let mut f = Forecast::default();
        f.add(420, prediction::EffectKind::Ice, (5, 1), false);
        f.add(3000, prediction::EffectKind::Ice, (5, 1), false);
        f.events.push(prediction::Effect {
            at: 200,
            kind: prediction::EffectKind::Doom,
            pos: (3, 8),
            certain: true,
            body_at: None,
            landing: None,
            shape: None,
        });
        let after = candidate_forecast(&f, Action { kind: 7, pos: (1, 2) });
        assert_eq!(after.events.len(), 3);
        assert_eq!(
            after
                .events
                .iter()
                .filter(|e| e.kind == prediction::EffectKind::Ice && e.at == 420)
                .count(),
            1
        );
        assert_eq!(
            after
                .events
                .iter()
                .filter(|e| e.kind == prediction::EffectKind::Doom && e.certain)
                .count(),
            1
        );
    }
    #[test]
    fn norm_is_over_individual_risks() {
        assert_eq!(pnorm(&[3., 4.], 1.), 7.);
        assert_eq!(pnorm(&[3., 4.], 2.), 5.);
        assert_eq!(pnorm(&[3., 4.], f32::INFINITY), 4.);
        assert_eq!(pnorm(&[], 2.), 0.);
    }
    #[test]
    fn model_validation_and_shared_score() {
        assert!(Model::parse("RSVZ_LINEAR_V1 159").is_err());
        let mut weights = vec![0.; FEATURES + 1];
        weights[104] = 2.;
        weights[FEATURES] = 3.;
        let model = Model::parse(&format!(
            "RSVZ_LINEAR_V1 {} {}",
            FEATURES,
            weights.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ")
        ))
        .unwrap();
        let mut x = [0.; FEATURES];
        x[104] = 1.;
        assert_eq!(model.score(&x), 5.);
        assert_eq!(Action { kind: 7, pos: (6, 9) }.id(), 769);
    }
}

fn process_path(path: &str) -> std::path::PathBuf {
    let pid = std::process::id();
    let worker = rsvz::session::session_shard().index;
    let path = path
        .replace("{pid}", &pid.to_string())
        .replace("{worker}", &worker.to_string());
    let path = std::path::Path::new(&path);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path.extension().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{stem}.{pid}.w{worker}.{extension}"))
}

struct Shadow {
    context: Option<Context>,
    out: Option<std::io::BufWriter<std::fs::File>>,
    enabled: bool,
    serial: u64,
    forecast: Forecast,
    full: bool,
    lite: bool,
    local: bool,
    audit: Option<std::io::BufWriter<std::fs::File>>,
    scene: Scene,
    model: Option<Model>,
    all: bool,
    pending: Option<(Action, Vec<u8>)>,
    action_clock: i32,
}
impl Default for Shadow {
    fn default() -> Self {
        let path = std::env::var("SIX_POLICY_SHADOW").ok();
        let replay_demo = experience::demo();
        let audit_path = path
            .clone()
            .or_else(|| replay_demo.then(|| std::env::var("SIX_REPLAY").expect("replay path")));
        assert!(
            !replay_demo || path.is_none(),
            "use binary replay or shadow JSON, not both"
        );
        let full = setting("SIX_POLICY_SHADOW_FULL", 0) != 0;
        let local = setting("SIX_POLICY_SHADOW_LOCAL", 0) != 0;
        let lite = replay_demo || local || setting("SIX_POLICY_SHADOW_LITE", 0) != 0;
        assert!(!(full && lite), "shadow full and lite/local are separate feature modes");
        let model = std::env::var("SIX_POLICY_SHADOW_MODEL")
            .ok()
            .map(|p| Model::parse(&std::fs::read_to_string(p).expect("shadow model")).expect("shadow model format"));
        if let Some(m) = &model {
            assert_eq!(
                m.input_dim(),
                if local { local::FEATURES } else { FEATURES },
                "shadow model dimension mismatch"
            );
        }
        Self {
            context: None,
            audit: audit_path.as_ref().map(|p| {
                std::io::BufWriter::new(
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(process_path(p).with_extension("audit"))
                        .expect("shadow audit"),
                )
            }),
            out: path.as_ref().map(|p| {
                std::io::BufWriter::new(
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(process_path(&p))
                        .expect("shadow output"),
                )
            }),
            enabled: path.is_some() || replay_demo,
            serial: 0,
            forecast: Forecast::default(),
            scene: Scene::default(),
            model,
            all: setting("SIX_POLICY_SHADOW_ALL", 0) != 0,
            pending: None,
            action_clock: -1,
            full,
            lite,
            local,
        }
    }
}
thread_local! {static SHADOW:RefCell<Shadow>=RefCell::new(Shadow::default());}
pub fn shadow_enabled() -> bool {
    SHADOW.with_borrow(|s| s.enabled)
}
pub fn shadow_context(context: Context, forecast: &Forecast, scene: &Scene) {
    SHADOW.with_borrow_mut(|s| {
        if s.enabled {
            s.context = Some(context);
            s.scene = scene.clone();
            if s.full || s.lite {
                s.forecast.events = forecast
                    .events
                    .iter()
                    .copied()
                    .filter(|e| {
                        e.certain
                            || !matches!(
                                e.kind,
                                prediction::EffectKind::Cherry | prediction::EffectKind::Jalapeno
                            )
                    })
                    .collect();
            }
        }
    });
}
pub fn shadow_prepare(card: CardSelection, pos: Pos) -> bool {
    let Some(kind) = CARDS.iter().position(|c| *c == card) else {
        return false;
    };
    if !SHADOW.with_borrow(|s| s.enabled && s.pending.is_none()) {
        return false;
    }
    shadow_record(Action { kind: kind + 1, pos }, true, "plant");
    SHADOW.with_borrow(|s| s.pending.is_some())
}
pub fn shadow_finish(success: bool) {
    SHADOW.with_borrow_mut(|s| {
        let Some((action, line)) = s.pending.take() else {
            return;
        };
        if experience::demo() {
            experience::finish(success);
        }
        if success {
            s.action_clock = rsvz::timing::clock();
            if let Some(out) = s.out.as_mut() {
                out.write_all(&line).expect("shadow");
                out.flush().expect("shadow");
            }
        } else if let Some(out) = &mut s.audit {
            writeln!(
                out,
                "{{\"seed\":{},\"clock\":{},\"round\":{},\"teacher\":{},\"reason\":\"executor_rejected\"}}",
                diagnostics::seed(),
                rsvz::timing::clock(),
                s.context.map_or(0, |c| c.round),
                action.id()
            )
            .expect("audit");
            out.flush().expect("audit");
        }
    });
}
pub fn shadow_skip(kind: usize) {
    debug_assert!(matches!(kind, 8 | 9));
    shadow_record(Action { kind, pos: (0, 0) }, false, "rule_skip");
    SHADOW.with_borrow_mut(|s| {
        if s.enabled {
            s.action_clock = rsvz::timing::clock();
        }
    });
}
fn wait_reason(now: i32, action_clock: i32, sparse: i32, dense: i32) -> Option<&'static str> {
    if now == action_clock {
        Some("action_stop")
    } else if now % sparse.max(1) == 0 {
        Some("periodic_wait")
    } else if dense > 0 && now % dense == 0 {
        Some("dense_wait")
    } else {
        None
    }
}
pub fn shadow_wait() {
    if !shadow_enabled() {
        return;
    }
    if experience::demo() {
        let now = rsvz::timing::clock();
        if now % 5 == 0 || SHADOW.with_borrow(|s| s.action_clock == now) {
            shadow_record(Action { kind: 0, pos: (0, 0) }, false, "replay_wait");
        }
        return;
    }
    let reason = SHADOW.with_borrow(|s| {
        wait_reason(
            rsvz::timing::clock(),
            s.action_clock,
            setting("SIX_POLICY_SHADOW_WAIT_EVERY", 100) as i32,
            setting("SIX_POLICY_SHADOW_DENSE_EVERY", 0) as i32,
        )
    });
    if let Some(reason) = reason {
        shadow_record(Action { kind: 0, pos: (0, 0) }, false, reason);
    }
}
fn shadow_record(teacher: Action, defer: bool, reason: &str) {
    SHADOW.with_borrow_mut(|shadow| {
        if !shadow.enabled{return;}
        let Some(c)=shadow.context else{return;};
        let mut scene=shadow.scene.clone();scene.sample(rsvz::timing::clock(),c.last_ice);
        let mut global=Policy::global(&scene,&c);
        let balloons=balloon_front(&scene);global[31]=bit(blover_allowed(&c,balloons));
        let all=legal_actions(&scene,&c);
        if !all.iter().any(|a|a.id()==teacher.id()) {
            if experience::demo() {experience::gap(rsvz::timing::clock());}
            let reason=if teacher.card().is_some_and(|card|!ready(card)){"card_not_ready"}else if matches!(teacher.kind,6|7)&&!c.ice_allowed[teacher.kind-6]{"outside_ice_window"}else if c.ice_required {"fixed_ice_deadline"}else if teacher.kind==2&&!blover_allowed(&c,balloons){"blover_demand_or_quota"}else if teacher.kind==4&&!c.cherry||teacher.kind==5&&!c.jala {"ash_done_this_wave"}else{"plant_legality"};
            if let Some(out)=&mut shadow.audit {writeln!(out,"{{\"seed\":{},\"clock\":{},\"round\":{},\"wave\":{},\"time\":{},\"teacher\":{},\"reason\":\"{}\",\"ice_required\":{},\"ice_allowed\":{:?},\"ice_due\":{},\"pending_ice\":{}}}",diagnostics::seed(),rsvz::timing::clock(),c.round,c.wave,c.time,teacher.id(),reason,c.ice_required,c.ice_allowed,c.ice_due,c.pending_ice).expect("audit");out.flush().expect("audit");}
            return;
        }
        if experience::demo() {
            let data=experience::wants(teacher,rsvz::timing::clock()).then(||replay_features(&scene,&shadow.forecast,&c,&all));
            let selected=all.iter().position(|a|a.id()==teacher.id()).unwrap();
            experience::prepare(replay_meta(&c,teacher,true,false,1.,diagnostics::seed()),selected,data);
            if defer {shadow.pending=Some((teacher,Vec::new()));} else {experience::finish(true);}
            return;
        }
        let negatives:Vec<_>=all.into_iter().filter(|a|a.id()!=teacher.id()).collect();
        if negatives.is_empty(){return;}
        let mut planner=planner::Planner::default();planner.scene=scene;
        let baseline=if shadow.full {planner.policy_baselines(&shadow.forecast)}else{Vec::new()};
        let (targets,bases):(Vec<_>,Vec<_>)=baseline.into_iter().unzip();
        let scene=&planner.scene;
        let lite=if shadow.lite {Some(lite::Context::new(scene,&shadow.forecast))}else{None};
        let mut ice_memo=vec![None;targets.len()*2];
        let mut actions=vec![teacher];
        if teacher.kind!=0&&negatives.iter().any(|a|a.kind==0) {actions.push(Action{kind:0,pos:(0,0)});}
        // Spread negatives across cards/cells; no zombie identifiers or pool order are used.
        let n=negatives.len();
        for i in 0..8 {let a=negatives[(i*n/8+(shadow.serial as usize*17)%n)%n];if !actions.iter().any(|b|b.id()==a.id()){actions.push(a);}}
        if shadow.all || shadow.model.is_some() {
            let mut ranked:Vec<(Action,f32)>=negatives.iter().copied().map(|a| {
                let mut x=global;action_features(&mut x,a,scene,&c,balloons);
                if shadow.full {let _=prediction_features(&mut x,a,scene,&shadow.forecast,&targets,&bases,2.,&mut ice_memo);}
                if let Some(lite)=&lite {lite.features(&mut x,a,scene,&shadow.forecast);}
                let score=shadow.model.as_ref().map_or(0.,|model|if shadow.local {model.score(&local::expand(&x,a,scene,&c))}else{model.score(&x)});(a,score)
            }).collect();
            ranked.sort_by(|(a,sa),(b,sb)|sb.total_cmp(sa).then(a.id().cmp(&b.id())));
            let count=if shadow.all {ranked.len()}else{16};
            for (a,_) in ranked.into_iter().take(count) {if !actions.iter().any(|b|b.id()==a.id()){actions.push(a);}}
        }
        let mut out=Vec::new();
        write!(out,"{{\"collector_revision\":2,\"sample_reason\":\"{}\",\"schema\":{},\"prediction_revision\":4,\"clock\":{},\"seed\":{},\"round\":{},\"decision\":{},\"wave\":{},\"time\":{},\"teacher\":{},\"teacher_legal\":true,\"full\":{},\"lite\":{},\"local\":{},\"actions\":[",reason,if shadow.local {3}else{2},rsvz::timing::clock(),diagnostics::seed(),c.round,shadow.serial,c.wave,c.time,teacher.id(),shadow.full,shadow.lite,shadow.local).expect("shadow");
        for (i,a) in actions.iter().enumerate() {let mut x=global;action_features(&mut x,*a,scene,&c,balloons);if shadow.full {let _=prediction_features(&mut x,*a,scene,&shadow.forecast,&targets,&bases,2.,&mut ice_memo);}if let Some(lite)=&lite {lite.features(&mut x,*a,scene,&shadow.forecast);}if i>0{write!(out,",").expect("shadow");}if shadow.local {write!(out,"{{\"id\":{},\"x\":{:?}}}",a.id(),local::expand(&x,*a,scene,&c)).expect("shadow");}else{write!(out,"{{\"id\":{},\"x\":{:?}}}",a.id(),x).expect("shadow");}}
        writeln!(out,"]}}").expect("shadow");
        if defer {shadow.pending=Some((teacher,out));}else{let output=shadow.out.as_mut().unwrap();output.write_all(&out).expect("shadow");output.flush().expect("shadow");}
        shadow.serial+=1;
    });
}

#[cfg(test)]
mod collector_tests {
    use super::wait_reason;
    #[test]
    fn sparse_dense_and_post_action_sources_are_disjoint() {
        assert_eq!(wait_reason(2500, -1, 500, 50), Some("periodic_wait"));
        assert_eq!(wait_reason(2650, -1, 500, 50), Some("dense_wait"));
        assert_eq!(wait_reason(2655, 2655, 500, 50), Some("action_stop"));
        assert_eq!(wait_reason(2655, -1, 500, 50), None);
        assert_eq!(wait_reason(2650, -1, 500, 0), None);
    }
}

fn explore_choice(
    greedy: usize, count: usize, epsilon: u32, seed: u32, decision: u64, salt: u32,
) -> (usize, bool, f32) {
    assert!(epsilon <= 1_000_000, "epsilon ppm must be <=1000000");
    if epsilon == 0 {
        return (greedy, false, 1.);
    }
    fn mix(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9e3779b97f4a7c15);
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        x ^ (x >> 31)
    }
    let key = (u64::from(seed) << 32) ^ u64::from(salt) ^ decision.wrapping_mul(0x9e3779b97f4a7c15);
    let explored = mix(key) % 1_000_000 < u64::from(epsilon);
    let selected = if explored {
        (mix(key ^ 0xa0761d6478bd642f) % count as u64) as usize
    } else {
        greedy
    };
    let eps = epsilon as f32 / 1_000_000.;
    (
        selected,
        explored,
        eps / count as f32 + if selected == greedy { 1. - eps } else { 0. },
    )
}
fn replay_meta(c: &Context, a: Action, demo: bool, explored: bool, probability: f32, seed: u32) -> experience::Meta {
    experience::Meta {
        seed,
        round: c.round,
        clock: rsvz::timing::clock(),
        wave: c.wave,
        time: c.time,
        action: a.id() as u16,
        demo,
        explored,
        probability,
        plan: c.plan,
    }
}
fn replay_features(scene: &Scene, forecast: &Forecast, c: &Context, actions: &[Action]) -> Vec<u8> {
    let mut global = Policy::global(scene, c);
    let balloons = balloon_front(scene);
    global[31] = bit(blover_allowed(c, balloons));
    let lite = lite::Context::new(scene, forecast);
    let candidates: Vec<_> = actions
        .iter()
        .map(|a| {
            let mut x = global;
            action_features(&mut x, *a, scene, c, balloons);
            lite.features(&mut x, *a, scene, forecast);
            (*a, x)
        })
        .collect();
    experience::encode_candidates(&candidates)
}
pub fn replay_enabled() -> bool {
    experience::enabled()
}
pub fn replay_begin(seed: u32, clock: i32) {
    experience::begin(seed, clock);
}
pub fn replay_reward(rounds: u32, clock: i32) {
    experience::reward(rounds, clock);
}
pub fn replay_end(clock: i32, kind: u8) {
    experience::end(clock, kind);
}
pub fn replay_exit() {
    experience::exit();
}
pub fn replay_endpoint(scene: &Scene, forecast: &Forecast, c: &Context, seed: u32, kind: u8) {
    if !experience::enabled() {
        return;
    }
    let actions = legal_actions(scene, c);
    experience::endpoint(
        replay_meta(c, Action { kind: 0, pos: (0, 0) }, experience::demo(), false, 1., seed),
        replay_features(scene, forecast, c, &actions),
        kind,
    );
}

#[cfg(test)]
mod exploration_tests {
    use super::explore_choice;
    #[test]
    fn zero_epsilon_preserves_greedy_and_probability_is_mixture() {
        assert_eq!(explore_choice(3, 10, 0, 7, 0, 1), (3, false, 1.));
        for d in 0..1000 {
            let (i, _, p) = explore_choice(3, 10, 250_000, 7, d, 1);
            assert!(i < 10);
            assert_eq!(p, if i == 3 { 0.775 } else { 0.025 });
        }
    }
}
#[cfg(test)]
mod fixed_input_audit {
    use super::Model;
    use std::io::Write;

    #[test]
    #[ignore = "requires SIX_AUDIT_MODEL, SIX_AUDIT_INPUT, SIX_AUDIT_OUTPUT"]
    fn score_hierarchical_candidates() {
        let model = Model::parse(
            &std::fs::read_to_string(std::env::var("SIX_AUDIT_MODEL").expect("model path")).expect("model read"),
        )
        .expect("model parse");
        let Model::Hierarchical(scorer) = model else {
            panic!("requires hierarchical model");
        };
        let input = std::fs::read_to_string(std::env::var("SIX_AUDIT_INPUT").expect("input path")).expect("input read");
        let mut tokens = input.split_whitespace();
        let states: usize = tokens.next().expect("state count").parse().expect("integer");
        let with_plan = super::setting("SIX_AUDIT_PLAN", 0) != 0;
        assert!(
            !scorer.requires_plan() || with_plan,
            "486 audit input requires SIX_AUDIT_PLAN=1 and real Plan32"
        );
        let mut out = std::io::BufWriter::new(
            std::fs::File::create(std::env::var("SIX_AUDIT_OUTPUT").expect("output path")).expect("output create"),
        );
        for _ in 0..states {
            let state: u64 = tokens.next().expect("state id").parse().expect("integer");
            let count: usize = tokens.next().expect("candidate count").parse().expect("integer");
            assert!(count > 0);
            let plan = with_plan.then(|| {
                let mut p = [0.; super::plan::WIDTH];
                for v in &mut p {
                    *v = tokens.next().expect("Plan32").parse::<f32>().expect("plan float");
                    assert!(v.is_finite());
                }
                p
            });
            let mut candidates = Vec::with_capacity(count);
            let mut prefix = Vec::new();
            let mut common = None;
            for _ in 0..count {
                let id: usize = tokens.next().expect("action").parse().expect("integer");
                let mut x = [0.; super::FEATURES];
                for v in &mut x {
                    *v = tokens.next().expect("feature").parse::<f32>().expect("float");
                    assert!(v.is_finite());
                }
                let bits: Vec<_> = x[..104].iter().map(|v| v.to_bits()).collect();
                if let Some(old) = &common {
                    assert_eq!(old, &bits);
                } else {
                    prefix = scorer.position.common_prefix(&x);
                    common = Some(bits);
                }
                let score = scorer.position.score_prefixed(&x, &prefix);
                candidates.push(super::Candidate {
                    action: super::Action {
                        kind: id / 100,
                        pos: (((id % 100) / 10) as i32, (id % 10) as i32),
                    },
                    x,
                    score,
                    ice: None,
                });
            }
            let h = scorer.rank_with_plan(&candidates, plan.as_ref());
            let ids: Vec<_> = candidates.iter().map(|c| c.action.id()).collect();
            let scores: Vec<_> = candidates.iter().map(|c| c.score).collect();
            writeln!(out,"{{\"state\":{},\"selected\":{},\"selected_kind\":{},\"intent_dim\":{},\"intent_layout\":\"{}\",\"intent_backend\":\"{}\",\"model_family\":\"{}\",\"actions\":{:?},\"position_scores\":{:?},\"base_scores\":{:?},\"mask\":{:?},\"prior\":{:?},\"residual\":{:?},\"kind_scores\":{:?},\"intent_x\":{:?}}}",state,candidates[h.selected].action.id(),h.kind,h.input.len(),super::hierarchical::layout(h.input.len()),h.backend,h.family,ids,scores,h.base_scores,h.mask,h.prior,h.residual,h.scores,h.input).expect("write");
        }
        assert!(tokens.next().is_none());
        out.flush().expect("flush");
    }

    /// Read-only export parity gate. No frame, backend, or simulation is accessed.
    /// Input: state_count, then repeated state_id candidate_count and
    /// candidate_id followed by model.input_dim() float32 values.
    #[test]
    #[ignore = "requires SIX_AUDIT_MODEL, SIX_AUDIT_INPUT, SIX_AUDIT_OUTPUT"]
    fn score_real_candidates() {
        let model_path = std::env::var("SIX_AUDIT_MODEL").expect("model path");
        let input_path = std::env::var("SIX_AUDIT_INPUT").expect("candidate input path");
        let output_path = std::env::var("SIX_AUDIT_OUTPUT").expect("score output path");
        let model = Model::parse(&std::fs::read_to_string(model_path).expect("model read")).expect("model parse");
        let input = std::fs::read_to_string(input_path).expect("input read");
        let mut tokens = input.split_whitespace();
        let states: usize = tokens
            .next()
            .expect("state count")
            .parse()
            .expect("state count integer");
        let mut out = std::io::BufWriter::new(std::fs::File::create(output_path).expect("output create"));
        for _ in 0..states {
            let state: u64 = tokens.next().expect("state id").parse().expect("state integer");
            let count: usize = tokens
                .next()
                .expect("candidate count")
                .parse()
                .expect("candidate integer");
            assert!(count > 0, "state must have legal candidates");
            let mut first = None;
            let mut prefix = Vec::new();
            for _ in 0..count {
                let action: u16 = tokens.next().expect("action id").parse().expect("action integer");
                let x: Vec<f32> = (0..model.input_dim())
                    .map(|_| tokens.next().expect("feature").parse().expect("float32 feature"))
                    .collect();
                assert!(x.iter().all(|v| v.is_finite()), "nonfinite input");
                let bits: Vec<_> = x[..104].iter().map(|v| v.to_bits()).collect();
                if let Some(old) = &first {
                    assert_eq!(&bits, old, "common prefix changed within state");
                } else {
                    prefix = model.common_prefix(&x);
                    first = Some(bits);
                }
                let score = model.score_prefixed(&x, &prefix);
                writeln!(out, "{state} {action} {score:.9e} {}", score.to_bits()).expect("score write");
            }
        }
        assert!(tokens.next().is_none(), "trailing input tokens");
        out.flush().expect("score flush");
    }
}
