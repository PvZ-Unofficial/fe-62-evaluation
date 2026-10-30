//! Hierarchical kind choice followed by conditional cell choice; no new game reads.
use super::{Candidate, FEATURES, Model};

pub(super) const INPUTS: usize = 434;
pub(super) const WAVE_INPUTS: usize = 454;
pub(super) const PLAN_INPUTS: usize = 486;
const KINDS: usize = 10;
pub(super) struct Scorer {
    pub base: Model,
    pub position: Model,
    inputs: usize,
    hidden: usize,
    w1: Vec<f32>,
    b1: Vec<f32>,
    w2: Vec<f32>,
    b2: [f32; KINDS],
    trees: Option<super::tree::Ensemble>,
}
pub(super) struct Decision {
    pub selected: usize,
    pub kind: usize,
    pub mask: [bool; KINDS],
    pub prior: [f32; KINDS],
    pub residual: [f32; KINDS],
    pub scores: [f32; KINDS],
    pub input: Vec<f32>,
    pub base_scores: Vec<f32>,
    pub backend: &'static str,
    pub family: &'static str,
}
fn integer(tokens: &mut std::str::SplitWhitespace<'_>, name: &str) -> Result<usize, String> {
    tokens
        .next()
        .ok_or_else(|| format!("missing {name}"))?
        .parse()
        .map_err(|_| format!("invalid {name}"))
}
fn nested(tokens: &mut std::str::SplitWhitespace<'_>, count: usize) -> Result<Model, String> {
    let block: Vec<_> = tokens.by_ref().take(count).collect();
    if block.len() != count || block.is_empty() {
        return Err("truncated nested model".into());
    }
    if matches!(block[0], "RSVZ_HIER_V1" | "RSVZ_TREE_HIER_V1") {
        return Err("recursive hierarchy is not supported".into());
    }
    let model = Model::parse(&block.join(" "))?;
    if model.input_dim() != FEATURES {
        return Err("hierarchical scalar models require input192".into());
    }
    Ok(model)
}
pub(super) fn parse(mut tokens: std::str::SplitWhitespace<'_>) -> Result<Scorer, String> {
    let candidate_inputs = integer(&mut tokens, "input")?;
    let inputs = integer(&mut tokens, "intent input")?;
    let kinds = integer(&mut tokens, "kinds")?;
    if candidate_inputs != FEATURES || !matches!(inputs, INPUTS | WAVE_INPUTS | PLAN_INPUTS) || kinds != KINDS {
        return Err("hierarchy dimensions must be 192 (434, 454, or 486) 10".into());
    }
    let hidden = integer(&mut tokens, "intent hidden")?;
    if !(1..=256).contains(&hidden) {
        return Err("intent hidden must be 1..256".into());
    }
    let nb = integer(&mut tokens, "base token count")?;
    let np = integer(&mut tokens, "position token count")?;
    let base = nested(&mut tokens, nb)?;
    let position = nested(&mut tokens, np)?;
    let weights: Vec<f32> = tokens
        .map(|v| v.parse::<f32>().map_err(|_| "invalid intent weight".to_string()))
        .collect::<Result<_, _>>()?;
    let n = hidden * inputs;
    let expected = n + hidden + KINDS * hidden + KINDS;
    if weights.len() != expected || weights.iter().any(|v| !v.is_finite()) {
        return Err(format!("expected {expected} finite intent weights"));
    }
    Ok(Scorer {
        base,
        position,
        inputs,
        hidden,
        w1: weights[..n].to_vec(),
        b1: weights[n..n + hidden].to_vec(),
        w2: weights[n + hidden..expected - KINDS].to_vec(),
        b2: weights[expected - KINDS..].try_into().unwrap(),
        trees: None,
    })
}
pub(super) fn parse_trees(mut tokens: std::str::SplitWhitespace<'_>) -> Result<Scorer, String> {
    let candidate_inputs = integer(&mut tokens, "input")?;
    let inputs = integer(&mut tokens, "intent input")?;
    let kinds = integer(&mut tokens, "kinds")?;
    if candidate_inputs != FEATURES || inputs != PLAN_INPUTS || kinds != KINDS {
        return Err("tree hierarchy dimensions must be 192 486 10".into());
    }
    let nb = integer(&mut tokens, "base token count")?;
    let np = integer(&mut tokens, "position token count")?;
    let count = integer(&mut tokens, "tree count")?;
    let base = nested(&mut tokens, nb)?;
    let position = nested(&mut tokens, np)?;
    let trees = super::tree::Ensemble::parse(&mut tokens, count, inputs, kinds)?;
    if tokens.next().is_some() {
        return Err("trailing tree model tokens".into());
    }
    Ok(Scorer {
        base,
        position,
        inputs,
        hidden: 0,
        w1: Vec::new(),
        b1: Vec::new(),
        w2: Vec::new(),
        b2: [0.; KINDS],
        trees: Some(trees),
    })
}
pub(super) fn layout(inputs: usize) -> &'static str {
    match inputs {
        INPUTS => "lite-kind-minmax-v1",
        WAVE_INPUTS => "lite-kind-minmax-wave20-v1",
        PLAN_INPUTS => "lite-kind-minmax-wave20-plan32-v1",
        _ => unreachable!("validated intent input dimension"),
    }
}
#[cfg(test)]
pub(super) fn aggregate(candidates: &[Candidate], inputs: usize) -> (Vec<f32>, [bool; KINDS]) {
    aggregate_with_plan(candidates, inputs, None)
}
pub(super) fn aggregate_with_plan(
    candidates: &[Candidate], inputs: usize, plan: Option<&super::plan::Plan>,
) -> (Vec<f32>, [bool; KINDS]) {
    assert!(!candidates.is_empty());
    assert!(matches!(inputs, INPUTS | WAVE_INPUTS | PLAN_INPUTS));
    let mut x = vec![0.; inputs];
    x[..104].copy_from_slice(&candidates[0].x[..104]);
    let mut mask = [false; KINDS];
    for c in candidates {
        let k = c.action.kind;
        assert!(k < KINDS);
        let offset = 114 + k * 32;
        if !mask[k] {
            x[offset..offset + 16].copy_from_slice(&c.x[144..160]);
            x[offset + 16..offset + 32].copy_from_slice(&c.x[144..160]);
            mask[k] = true;
            x[104 + k] = 1.;
        } else {
            for i in 0..16 {
                x[offset + i] = x[offset + i].min(c.x[144 + i]);
                x[offset + 16 + i] = x[offset + 16 + i].max(c.x[144 + i]);
            }
        }
    }
    if inputs >= WAVE_INPUTS {
        let wave = (x[1] * 20.).round() as i32;
        if (1..=20).contains(&wave) {
            x[INPUTS + wave as usize - 1] = 1.;
        }
    }
    if inputs == PLAN_INPUTS {
        let plan = plan.expect("486 requires explicit Plan32; missing is not zero");
        assert!(plan.iter().all(|v| v.is_finite()), "nonfinite Plan32");
        x[WAVE_INPUTS..].copy_from_slice(plan);
    }
    (x, mask)
}
impl Scorer {
    pub fn requires_plan(&self) -> bool {
        self.inputs == PLAN_INPUTS
    }

    fn residual(&self, x: &[f32]) -> [f32; KINDS] {
        if let Some(trees) = &self.trees {
            return trees.residual(x);
        }
        let mut hidden = [0.; 256];
        for (h, v) in hidden[..self.hidden].iter_mut().enumerate() {
            *v = x
                .iter()
                .zip(&self.w1[h * self.inputs..(h + 1) * self.inputs])
                .fold(self.b1[h], |sum, (x, w)| sum + x * w)
                .max(0.);
        }
        std::array::from_fn(|k| {
            hidden[..self.hidden]
                .iter()
                .zip(&self.w2[k * self.hidden..(k + 1) * self.hidden])
                .fold(self.b2[k], |sum, (h, w)| sum + h * w)
        })
    }
    #[cfg(test)]
    pub fn rank(&self, candidates: &[Candidate]) -> Decision {
        self.rank_with_plan(candidates, None)
    }
    pub fn rank_with_plan(&self, candidates: &[Candidate], plan: Option<&super::plan::Plan>) -> Decision {
        let (input, mask) = aggregate_with_plan(candidates, self.inputs, plan);
        let residual = self.residual(&input);
        let prefix = self.base.common_prefix(&candidates[0].x);
        let mut prior = [0.; KINDS];
        let mut seen = [false; KINDS];
        let base_scores: Vec<_> = candidates
            .iter()
            .map(|c| {
                let score = self.base.score_prefixed(&c.x, &prefix);
                let k = c.action.kind;
                if !seen[k] || score.total_cmp(&prior[k]).is_gt() {
                    prior[k] = score;
                    seen[k] = true;
                }
                score
            })
            .collect();
        let scores = if let Some(trees) = &self.trees {
            trees.add_to(prior, &input)
        } else {
            std::array::from_fn(|k| {
                if residual[k] == 0. {
                    prior[k]
                } else {
                    prior[k] + residual[k]
                }
            })
        };
        let kind = (0..KINDS)
            .filter(|&k| mask[k])
            .max_by(|&a, &b| scores[a].total_cmp(&scores[b]).then(b.cmp(&a)))
            .unwrap();
        let selected = candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| c.action.kind == kind)
            .max_by(|(_, a), (_, b)| a.score.total_cmp(&b.score).then(b.action.id().cmp(&a.action.id())))
            .map(|(i, _)| i)
            .unwrap();
        Decision {
            selected,
            kind,
            mask,
            prior,
            residual,
            scores,
            input,
            base_scores,
            family: if self.trees.is_some() {
                "tree-kind-hybrid"
            } else {
                "neural-hierarchical"
            },
            backend: if self.trees.is_some() {
                "xgboost-numeric-trees"
            } else {
                "relu-mlp"
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Action;
    use super::*;
    fn candidate(kind: usize, row: i32, col: i32, score: f32) -> Candidate {
        let mut x = [0.; FEATURES];
        x[0] = 1.;
        x[114] = row as f32;
        x[115] = col as f32;
        Candidate {
            action: Action { kind, pos: (row, col) },
            x,
            score,
            ice: None,
        }
    }
    fn scorer() -> Scorer {
        let mut w = vec![0.; FEATURES];
        w[114] = 2.;
        w[115] = 1.;
        Scorer {
            base: Model::Linear(w.clone(), 0.),
            position: Model::Linear(w, 0.),
            inputs: INPUTS,
            hidden: 1,
            w1: vec![0.; INPUTS],
            b1: vec![1.],
            w2: vec![0.; KINDS],
            b2: [0.; KINDS],
            trees: None,
        }
    }
    #[test]
    fn aggregate_keeps_all_legal_kind_masks_and_min_max_slots() {
        let mut a = candidate(2, 1, 1, 0.);
        let mut b = candidate(2, 1, 2, 0.);
        let c = candidate(0, 0, 0, 0.);
        for i in 0..16 {
            a.x[144 + i] = i as f32;
            b.x[144 + i] = 15. - i as f32;
        }
        let (x, mask) = aggregate(&[a, b, c], INPUTS);
        assert!(mask[0] && mask[2]);
        assert!(!mask[1]);
        assert_eq!(x[104], 1.);
        assert_eq!(x[106], 1.);
        assert_eq!(x[105], 0.);
        for i in 0..16 {
            assert_eq!(x[178 + i], (i as f32).min(15. - i as f32));
            assert_eq!(x[194 + i], (i as f32).max(15. - i as f32));
        }
        assert!(x[146..178].iter().all(|v| *v == 0.));
    }
    #[test]
    fn zero_residual_and_identical_position_preserve_flat_argmax() {
        let s = scorer();
        let mut candidates = vec![
            candidate(0, 0, 0, 0.),
            candidate(1, 1, 2, 0.),
            candidate(2, 1, 2, 0.),
            candidate(2, 2, 1, 0.),
        ];
        for c in &mut candidates {
            c.score = s.position.score(&c.x);
        }
        let r = s.rank(&candidates);
        let flat = candidates
            .iter()
            .enumerate()
            .max_by(|(i, a), (j, b)| a.score.total_cmp(&b.score).then(j.cmp(i)))
            .unwrap()
            .0;
        assert_eq!(r.selected, flat);
        for k in 0..KINDS {
            assert_eq!(r.scores[k].to_bits(), r.prior[k].to_bits());
        }
        candidates.pop();
        let r = s.rank(&candidates);
        assert_eq!(r.kind, 1);
        assert_eq!(candidates[r.selected].action.id(), 112);
    }
    #[test]
    fn kind_residual_can_override_prior_without_position_multiplicity() {
        let mut s = scorer();
        s.b2[0] = 100.;
        let candidates = vec![
            candidate(0, 0, 0, 0.),
            candidate(2, 2, 1, 1000.),
            candidate(2, 2, 2, 1000.),
        ];
        assert_eq!(s.rank(&candidates).selected, 0);
        s.b2[9] = 10000.;
        assert_eq!(s.rank(&candidates).kind, 0, "masked skip cannot win");
    }
    #[test]
    fn kind_and_position_are_distinct_and_ties_use_lowest_action_id() {
        let mut s = scorer();
        s.b2[2] = 100.;
        let candidates = vec![candidate(0, 0, 0, 500.), candidate(2, 2, 2, 1.), candidate(2, 1, 1, 1.)];
        let r = s.rank(&candidates);
        assert_eq!(r.kind, 2);
        assert_eq!(r.selected, 2);
    }
    #[test]
    fn parser_loads_complete_nested_models_and_rejects_bad_counts() {
        let scalar = format!("RSVZ_LINEAR_V1 192 {}", vec!["0"; 193].join(" "));
        let n = scalar.split_whitespace().count();
        let weights = vec!["0"; INPUTS + 1 + KINDS + KINDS].join(" ");
        let text = format!("RSVZ_HIER_V1 192 434 10 1 {n} {n} {scalar} {scalar} {weights}");
        let model = Model::parse(&text).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(model.input_dim(), 192);
        assert!(Model::parse(&text.replace("192 434 10 1", "192 433 10 1")).is_err());
        assert!(Model::parse(&(text.clone() + " 0")).is_err());
    }
    #[test]
    fn zero_residual_does_not_change_negative_zero_prior_bits() {
        let mut s = scorer();
        s.base = Model::Linear(vec![-0.; FEATURES], -0.);
        s.position = Model::Linear(vec![-0.; FEATURES], -0.);
        let mut c = candidate(0, 0, 0, 0.);
        c.score = s.position.score(&c.x);
        let h = s.rank(&[c]);
        assert_eq!(h.prior[0].to_bits(), (-0f32).to_bits());
        assert_eq!(h.scores[0].to_bits(), h.prior[0].to_bits());
    }
    #[test]
    fn categorical_wave_appends_only_twenty_visible_wave_bits() {
        for wave in 0..=21 {
            let mut c = candidate(0, 0, 0, 0.);
            c.x[1] = wave as f32 / 20.;
            let (old, old_mask) = aggregate(std::slice::from_ref(&c), INPUTS);
            let (new, new_mask) = aggregate(&[c], WAVE_INPUTS);
            assert_eq!(old_mask, new_mask);
            assert_eq!(&new[..INPUTS], old.as_slice());
            for i in 0..20 {
                assert_eq!(new[INPUTS + i], if wave == i + 1 { 1. } else { 0. });
            }
        }
    }
    #[test]
    fn zero_extended_wave_weights_preserve_existing_nonzero_residual_bits() {
        let mut c = candidate(2, 2, 1, 1.);
        c.x[1] = 19. / 20.;
        let mut s = scorer();
        s.w1[1] = 0.25;
        s.b1[0] = 0.5;
        s.w2[2] = 0.75;
        s.b2[2] = -0.125;
        let old = s.rank(std::slice::from_ref(&c));
        s.inputs = WAVE_INPUTS;
        s.w1.extend([0.; 20]);
        let new = s.rank(&[c]);
        assert_eq!(old.selected, new.selected);
        assert_eq!(old.kind, new.kind);
        for k in 0..KINDS {
            assert_eq!(old.residual[k].to_bits(), new.residual[k].to_bits());
            assert_eq!(old.scores[k].to_bits(), new.scores[k].to_bits());
        }
    }
    #[test]
    fn parser_distinguishes_454_layout_and_requires_its_extra_weights() {
        let scalar = format!("RSVZ_LINEAR_V1 192 {}", vec!["0"; 193].join(" "));
        let n = scalar.split_whitespace().count();
        let weights = vec!["0"; WAVE_INPUTS + 1 + KINDS + KINDS].join(" ");
        let text = format!("RSVZ_HIER_V1 192 454 10 1 {n} {n} {scalar} {scalar} {weights}");
        let Model::Hierarchical(s) = Model::parse(&text).unwrap_or_else(|e| panic!("{e}")) else {
            panic!("hierarchy");
        };
        assert_eq!(s.inputs, WAVE_INPUTS);
        assert_eq!(layout(s.inputs), "lite-kind-minmax-wave20-v1");
        assert!(Model::parse(&text.replacen("192 454 10", "192 434 10", 1)).is_err());
    }
    #[test]
    fn plan_layout_preserves_454_and_appends_the_explicit_shared_snapshot() {
        let mut c = candidate(0, 0, 0, 0.);
        c.x[1] = 10. / 20.;
        let plan = std::array::from_fn(|i| i as f32 / 32.);
        let (old, mask) = aggregate(std::slice::from_ref(&c), WAVE_INPUTS);
        let (new, new_mask) = aggregate_with_plan(&[c], PLAN_INPUTS, Some(&plan));
        assert_eq!(mask, new_mask);
        assert_eq!(&new[..WAVE_INPUTS], old.as_slice());
        assert_eq!(&new[WAVE_INPUTS..], &plan);
        assert_eq!(layout(PLAN_INPUTS), "lite-kind-minmax-wave20-plan32-v1");
    }
    #[test]
    #[should_panic(expected = "486 requires explicit Plan32")]
    fn plan_model_refuses_missing_plan() {
        aggregate(&[candidate(0, 0, 0, 0.)], PLAN_INPUTS);
    }
    #[test]
    fn zero_tree_hierarchy_preserves_base_and_position_selection() {
        let scalar = format!("RSVZ_LINEAR_V1 192 {}", vec!["0"; 193].join(" "));
        let n = scalar.split_whitespace().count();
        let model = Model::parse(&format!("RSVZ_TREE_HIER_V1 192 486 10 {n} {n} 0 {scalar} {scalar}"))
            .unwrap_or_else(|e| panic!("{e}"));
        let Model::Hierarchical(s) = model else {
            panic!("hierarchy");
        };
        let p = [0.; 32];
        let d = s.rank_with_plan(&[candidate(0, 0, 0, 0.), candidate(2, 1, 1, 0.)], Some(&p));
        assert_eq!(d.selected, 0);
        assert_eq!(d.backend, "xgboost-numeric-trees");
        assert_eq!(d.prior, d.scores);
    }
}
