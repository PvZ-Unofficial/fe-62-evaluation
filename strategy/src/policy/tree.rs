//! Dependency-free scalar numeric XGBoost trees. Exported leaf values already contain eta.
use std::str::SplitWhitespace;
#[derive(Clone, Copy)]
struct Node {
    feature: i32,
    value: f32,
    left: i32,
    right: i32,
    default_left: bool,
}
struct Tree {
    class: usize,
    nodes: Vec<Node>,
}
pub(super) struct Ensemble {
    trees: Vec<Tree>,
}
fn number<T: std::str::FromStr>(tokens: &mut SplitWhitespace<'_>, name: &str) -> Result<T, String> {
    tokens
        .next()
        .ok_or_else(|| format!("missing {name}"))?
        .parse()
        .map_err(|_| format!("invalid {name}"))
}
impl Tree {
    fn validate(&self, inputs: usize, kinds: usize) -> Result<(), String> {
        if self.class >= kinds || self.nodes.is_empty() {
            return Err("invalid tree class or empty tree".into());
        }
        let mut parents = vec![0usize; self.nodes.len()];
        for n in &self.nodes {
            if !n.value.is_finite() {
                return Err("nonfinite threshold or leaf".into());
            }
            if n.feature == -1 {
                if n.left != -1 || n.right != -1 {
                    return Err("leaf must have no children".into());
                }
            } else {
                if n.feature < 0
                    || n.feature as usize >= inputs
                    || n.left < 0
                    || n.right < 0
                    || n.left == n.right
                    || n.left as usize >= self.nodes.len()
                    || n.right as usize >= self.nodes.len()
                {
                    return Err("invalid split feature or child index".into());
                }
                parents[n.left as usize] += 1;
                parents[n.right as usize] += 1;
            }
        }
        if parents[0] != 0 || parents[1..].iter().any(|&n| n != 1) {
            return Err("tree must have one root and unique parents".into());
        }
        let mut seen = vec![false; self.nodes.len()];
        let mut pending = vec![0usize];
        while let Some(i) = pending.pop() {
            if seen[i] {
                return Err("cyclic tree".into());
            }
            seen[i] = true;
            let n = self.nodes[i];
            if n.feature >= 0 {
                pending.push(n.right as usize);
                pending.push(n.left as usize);
            }
        }
        if seen.iter().any(|v| !*v) {
            return Err("disconnected tree".into());
        }
        Ok(())
    }
    fn leaf(&self, x: &[f32]) -> f32 {
        let mut index = 0usize;
        loop {
            let n = self.nodes[index];
            if n.feature == -1 {
                return n.value;
            }
            let value = x[n.feature as usize];
            let left = if value.is_nan() {
                n.default_left
            } else {
                value < n.value
            };
            index = if left { n.left } else { n.right } as usize;
        }
    }
}
impl Ensemble {
    pub fn parse(tokens: &mut SplitWhitespace<'_>, count: usize, inputs: usize, kinds: usize) -> Result<Self, String> {
        let mut trees = Vec::new();
        for _ in 0..count {
            let class = number(tokens, "tree class")?;
            let nodes_count: usize = number(tokens, "node count")?;
            if nodes_count == 0 {
                return Err("empty tree".into());
            }
            // Grow only from actual parsed tokens: corrupt counts never reserve unbounded memory.
            let mut nodes = Vec::new();
            for _ in 0..nodes_count {
                let feature = number(tokens, "feature")?;
                let value = number(tokens, "threshold/leaf")?;
                let left = number(tokens, "left child")?;
                let right = number(tokens, "right child")?;
                let missing: u8 = number(tokens, "default_left")?;
                if missing > 1 {
                    return Err("default_left must be 0 or 1".into());
                }
                nodes.push(Node {
                    feature,
                    value,
                    left,
                    right,
                    default_left: missing == 1,
                });
            }
            let tree = Tree { class, nodes };
            tree.validate(inputs, kinds)?;
            trees.push(tree);
        }
        Ok(Self { trees })
    }
    pub fn add_to(&self, mut scores: [f32; 10], x: &[f32]) -> [f32; 10] {
        // Match native output_margin with base_margin=prior. No XGB base_score is added.
        for tree in &self.trees {
            scores[tree.class] += tree.leaf(x);
        }
        scores
    }
    pub fn residual(&self, x: &[f32]) -> [f32; 10] {
        self.add_to([0.; 10], x)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn parse(text: &str, n: usize) -> Result<Ensemble, String> {
        let mut t = text.split_whitespace();
        let e = Ensemble::parse(&mut t, n, 486, 10)?;
        if t.next().is_some() {
            return Err("trailing".into());
        }
        Ok(e)
    }
    #[test]
    fn threshold_is_strict_and_nan_follows_default_direction() {
        let e = parse("2 3 0 0.5 1 2 1 -1 3 -1 -1 0 -1 -4 -1 -1 0", 1).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(e.residual(&[0.499])[2], 3.);
        assert_eq!(e.residual(&[0.5])[2], -4.);
        assert_eq!(e.residual(&[f32::NAN])[2], 3.);
        let e = parse("2 3 0 0.5 1 2 0 -1 3 -1 -1 0 -1 -4 -1 -1 0", 1).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(e.residual(&[f32::NAN])[2], -4.);
    }
    #[test]
    fn accumulates_from_prior_in_tree_order_without_base_score() {
        let e = parse("3 1 -1 -100000000 -1 -1 0 3 1 -1 1 -1 -1 0", 2).unwrap_or_else(|e| panic!("{e}"));
        let mut prior = [0.; 10];
        prior[3] = 100000000.;
        assert_eq!(e.add_to(prior, &[])[3], 1.);
        assert_eq!(prior[3] + e.residual(&[])[3], 0.);
        assert_eq!(parse("", 0).unwrap_or_else(|e| panic!("{e}")).add_to(prior, &[]), prior);
    }
    #[test]
    fn rejects_invalid_structure_feature_class_and_nonfinite_values() {
        for text in [
            "10 1 -1 0 -1 -1 0",
            "0 1 -1 NaN -1 -1 0",
            "0 1 -1 0 0 -1 0",
            "0 3 486 1 1 2 0 -1 0 -1 -1 0 -1 0 -1 -1 0",
            "0 3 0 1 1 3 0 -1 0 -1 -1 0 -1 0 -1 -1 0",
            "0 3 0 1 0 2 0 -1 0 -1 -1 0 -1 0 -1 -1 0",
            "0 1 -1 0 -1 -1 2",
            "0 2 -1 0 -1 -1 0 -1 0 -1 -1 0",
            "0 1000000000",
        ] {
            assert!(parse(text, 1).is_err(), "accepted {text}");
        }
    }
}
