//! One activation-time plan shared by forecasting and actual Cherry planting.
use super::*;
use rsvz::core::contact::{circle_hits_rect, grid_explosion_shape};
use rsvz::core::model::{ContactRect, GridExplosionKind, PlantThreatShape};

pub(super) const CANDIDATES: usize = 12;

pub(super) fn position(index: usize) -> Pos {
    (if index % 2 == 0 { 2 } else { 1 }, index as i32 / 2 + 4)
}
fn index(pos: Pos) -> usize {
    ((pos.1 - 4) * 2 + i32::from(pos.0 == 1)) as usize
}

#[derive(Clone, Copy)]
pub(super) struct Debt {
    pub id: ZombieId,
    pub hits: i32,
    pub planned: bool,
}
#[derive(Clone)]
pub(crate) struct State {
    pub(super) debts: Vec<Debt>,
    pub(super) alternative: Option<Pos>,
    pub(super) alternative_targets: Vec<ZombieId>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            debts: Vec::with_capacity(64),
            alternative: None,
            alternative_targets: Vec::with_capacity(64),
        }
    }
}
impl State {
    pub fn refresh(&mut self, scene: &Scene) {
        self.debts
            .retain(|d| scene.zombies.iter().any(|z| z.id == d.id) && telemetry::damage_history(d.id).0 <= d.hits);
    }
    pub fn required(&self, id: ZombieId) -> bool {
        self.debts.iter().any(|d| d.id == id)
    }
}
fn damage_gap(z: Zombie, hits: i32) -> bool {
    z.kind == Z::GigaGargantuar
        && z.x <= 500.
        && match z.row {
            1 => z.hp > 5400 || hits == 0 && z.hp > 3600,
            2 => z.hp > 3600,
            _ => false,
        }
}

#[cfg(test)]
#[test]
fn low_health_does_not_erase_an_unresolved_cherry_obligation() {
    use rsvz_pvz_emulator_backend::{PeBackend, PeWorldConfig, runner_internal::PeWorldOwner};
    let mut world = PeWorldOwner::new_reset(PeWorldConfig::default())
        .unwrap()
        .install_current()
        .unwrap();
    world.with_backend(|b| {
        PeBackend::scope_backend(b, || {
            let z = Zombie {
                age: 0,
                id: ZombieId::from_raw(65537),
                kind: Z::GigaGargantuar,
                row: 2,
                wave: 1,
                carried: false,
                x: 300.,
                hp: 2200,
                armor: 0,
                phase: ZombiePhase::ZombieNormal,
                progress: 0.,
                speed: 0.37,
                frozen: 0,
                chilled: 0,
                object: false,
                eating: false,
                rect: ContactRect::new(283, 100, 408, 254),
            };
            let scene = Scene::fixture(vec![z]);
            let mut state = State::default();
            assert!(!damage_gap(z, 1));
            assert!(select(&scene, &Forecast::default(), 100, [true; CANDIDATES], false, &mut state).is_some());
            assert!(state.required(z.id));
        })
    });
}

struct Target {
    id: ZombieId,
    required: bool,
    current: ContactRect,
    needs_retention: bool,
    row: i32,
    rect: ContactRect,
    // 0: no damage obligation; 1: other threat; 2: red damage; 3: urgent loss.
    priority: usize,
    dependency: Option<i32>,
}

fn hits(target: &Target, shape: PlantThreatShape, pos: Pos, current: bool) -> bool {
    (target.row - pos.0).abs() <= shape.row_range
        && circle_hits_rect(shape.circle, if current { target.current } else { target.rect })
}
fn score(targets: &[Target], shapes: &[PlantThreatShape; CANDIDATES], i: usize, obligations: bool) -> [i32; 5] {
    let mut score = [0; 5];
    for target in targets {
        if hits(target, shapes[i], position(i), false) {
            if obligations && target.required {
                score[0] += 1;
            }
            if target.priority > 0 {
                score[5 - target.priority] += 1;
            }
        }
        // If it will leave every candidate, keep a currently covering plan so
        // the dedicated retention check can preserve that opportunity.
        if obligations && target.required && target.needs_retention && hits(target, shapes[i], position(i), true) {
            score[1] += 1;
        }
    }
    score
}
fn choose_with(
    targets: &[Target], shapes: &[PlantThreatShape; CANDIDATES], legal: [bool; CANDIDATES], obligations: bool,
) -> Option<Pos> {
    let mut best = None;
    for i in (0..CANDIDATES).rev().filter(|&i| legal[i]) {
        let value = (
            score(targets, shapes, i, obligations),
            targets
                .iter()
                .filter(|t| t.priority >= 2 && hits(t, shapes[i], position(i), false))
                .map(|t| -t.rect.left)
                .max()
                .unwrap_or(i32::MIN),
        );
        if best.is_none_or(|(_, old)| value > old) {
            best = Some((position(i), value));
        }
    }
    best.map(|(pos, _)| pos)
}
fn choose(targets: &[Target], shapes: &[PlantThreatShape; CANDIDATES], legal: [bool; CANDIDATES]) -> Option<Pos> {
    choose_with(targets, shapes, legal, true)
}

pub(super) fn select(
    scene: &Scene, forecast: &Forecast, at: i32, legal: [bool; CANDIDATES], flag: bool, state: &mut State,
) -> Option<Pos> {
    if !(1..=prediction::HORIZON).contains(&at) || !legal.iter().any(|&v| v) {
        return None;
    }
    state.refresh(scene);
    if at <= 100 {
        state.alternative = None;
        state.alternative_targets.clear();
    }
    // Do not let another provisional Cherry make this one's targets look safe:
    // its own position is being resolved too. Planted Cherries remain real damage.
    let without = Forecast {
        events: forecast
            .events
            .iter()
            .copied()
            .filter(|e| e.kind != prediction::EffectKind::Cherry || e.certain)
            .collect(),
    };
    let shapes = std::array::from_fn(|i| grid_explosion_shape(GridExplosionKind::CherryBomb, grid(position(i))));
    let next_doom = without
        .events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == prediction::EffectKind::Doom)
        .min_by_key(|(_, e)| e.at)
        .map(|(i, _)| i);
    let mut targets = Vec::new();
    for z in
        scene.zombies.iter().copied().filter(|z| {
            matches!(z.row, 1..=3) && (z.giant() || matches!(z.kind, Z::Football | Z::Zomboni | Z::Catapult))
        })
    {
        let (base, projected) = prediction::simulate_projected(z, scene, &without, at);
        telemetry::work_prediction(0, base);
        let (hit_count, _) = telemetry::damage_history(z.id);
        if base.safe() && base.death < NEVER {
            state.debts.retain(|d| d.id != z.id || !d.planned);
        }
        let unresolved_before_next_ash =
            z.kind == Z::GigaGargantuar && z.x <= 500. && !base.safe() && base.danger > at && base.death > base.danger;
        if (damage_gap(z, hit_count) || unresolved_before_next_ash)
            && next_doom.is_none_or(|i| base.ash_mask & (1 << i) == 0)
            && !state.required(z.id)
        {
            state.debts.push(Debt {
                id: z.id,
                hits: hit_count,
                planned: !damage_gap(z, hit_count),
            });
            telemetry::record(
                "cherry_obligation",
                &[z.id.raw() as i64, z.row as i64, z.hp as i64, hit_count as i64],
            );
        }
        let required = state.required(z.id);
        if !required && base.safe() && base.death < NEVER {
            continue;
        }
        let Some(projected_x) = projected else {
            continue;
        };
        let priority = if base.danger <= at + 751 {
            3
        } else if z.kind == Z::GigaGargantuar {
            2
        } else if !base.safe() {
            1
        } else {
            0
        };
        targets.push(Target {
            id: z.id,
            required,
            current: z.rect,
            needs_retention: required
                && !(0..CANDIDATES).any(|i| {
                    legal[i]
                        && (z.row - position(i).0).abs() <= shapes[i].row_range
                        && circle_hits_rect(shapes[i].circle, z.rect_at(projected_x))
                }),
            row: z.row,
            rect: z.rect_at(projected_x),
            priority,
            dependency: (z.giant()
                && z.row <= 2
                && z.x <= 500.
                && !base.safe()
                && base.danger > at
                && base.danger <= at + 751)
                .then_some(hit_count),
        });
    }
    // Keep the flag-wave rightward preference when there is no required damage.
    let legal = std::array::from_fn(|i| legal[i] && (!flag || !state.debts.is_empty() || position(i).1 >= 8));
    let pos = choose(&targets, &shapes, legal);
    if let Some(pos) = pos {
        for target in &targets {
            if let Some(count) = target.dependency {
                if hits(target, shapes[index(pos)], pos, false) && !state.required(target.id) {
                    state.debts.push(Debt {
                        id: target.id,
                        hits: count,
                        planned: true,
                    });
                    telemetry::record(
                        "cherry_dependency",
                        &[target.id.raw() as i64, at as i64, pos.0 as i64, pos.1 as i64],
                    );
                }
            }
        }
    }
    if at <= 100 {
        if let (Some(chosen), Some(other)) = (pos, choose_with(&targets, &shapes, legal, false)) {
            if score(&targets, &shapes, index(other), false) > score(&targets, &shapes, index(chosen), false) {
                state.alternative = Some(other);
                state.alternative_targets.extend(
                    targets
                        .iter()
                        .filter(|t| t.required && !hits(t, shapes[index(other)], other, false))
                        .map(|t| t.id),
                );
            }
        }
        if let Some(pos) = pos {
            let shape = shapes[index(pos)];
            let missing = targets
                .iter()
                .filter(|z| {
                    z.priority > 0
                        && ((z.row - pos.0).abs() > shape.row_range || !circle_hits_rect(shape.circle, z.rect))
                })
                .count();
            telemetry::record("cherry_plan", &[at as i64, pos.1 as i64, missing as i64, pos.0 as i64]);
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsvz::core::contact::grid_explosion_shape_from_pixel;
    use rsvz::core::model::PixelPos;

    fn shapes() -> [PlantThreatShape; CANDIDATES] {
        std::array::from_fn(|i| {
            grid_explosion_shape_from_pixel(
                GridExplosionKind::CherryBomb,
                grid(position(i)),
                PixelPos {
                    x: position(i).1 * 80 - 40,
                    y: if position(i).0 == 1 { 80 } else { 165 },
                },
            )
        })
    }
    fn target(x: i32, priority: usize) -> Target {
        Target {
            id: ZombieId::from_raw(65537),
            required: false,
            needs_retention: false,
            current: ContactRect::new(x, 97, x + 20, 251),
            row: 2,
            rect: ContactRect::new(x, 97, x + 20, 251),
            priority,
            dependency: None,
        }
    }
    #[test]
    fn harmless_left_actors_do_not_pull_a_required_red_left() {
        let targets = [target(280, 0), target(300, 0), target(650, 2)];
        assert_eq!(choose(&targets, &shapes(), [true; CANDIDATES]), Some((1, 9)));
    }
    #[test]
    fn equal_damage_counts_keep_the_leftmost_remaining_opportunity() {
        let targets = [target(280, 2), target(650, 2)];
        let pos = choose(&targets, &shapes(), [true; CANDIDATES]).unwrap();
        assert!(hits(&targets[0], shapes()[index(pos)], pos, false));
    }
    #[test]
    fn urgent_left_loss_overrides_rightward_preference() {
        let targets = [target(280, 3), target(650, 2)];
        let pos = choose(&targets, &shapes(), [true; CANDIDATES]).unwrap();
        assert!(pos.1 < 7);
        assert!(circle_hits_rect(shapes()[(pos.1 - 4) as usize].circle, targets[0].rect));
    }
    #[test]
    fn required_red_coverage_precedes_rightmost_column() {
        let targets = [target(440, 2), target(600, 2)];
        assert_eq!(choose(&targets, &shapes(), [true; CANDIDATES]), Some((1, 7)));
        assert_eq!(
            choose(
                &[],
                &shapes(),
                [
                    true, true, true, true, false, false, false, false, false, false, false, false
                ]
            ),
            Some((1, 5))
        );
        assert_eq!(choose(&targets, &shapes(), [false; CANDIDATES]), None);
    }
    #[test]
    fn row_range_and_rightward_preference_survive_row_one_expansion() {
        let third_row = Target {
            id: ZombieId::from_raw(65537),
            required: false,
            needs_retention: false,
            current: ContactRect::new(650, 182, 775, 336),
            row: 3,
            rect: ContactRect::new(650, 182, 775, 336),
            priority: 2,
            dependency: None,
        };
        assert_eq!(choose(&[third_row], &shapes(), [true; CANDIDATES]), Some((2, 9)));
        let mut legal = [true; CANDIDATES];
        legal[index((1, 9))] = false;
        assert_eq!(choose(&[], &shapes(), legal), Some((2, 9)));
    }
    #[test]
    fn left_damage_obligation_beats_many_right_targets() {
        let mut left = target(240, 2);
        left.required = true;
        let targets = [left, target(640, 2), target(650, 2), target(660, 3)];
        let pos = choose(&targets, &shapes(), [true; CANDIDATES]).unwrap();
        assert!(hits(&targets[0], shapes()[index(pos)], pos, false));
        assert!(choose_with(&targets, &shapes(), [true; CANDIDATES], false).unwrap().1 > pos.1);
    }
    #[test]
    fn blocked_row_one_uses_row_two_without_losing_left_coverage() {
        let mut left = target(240, 2);
        left.required = true;
        let mut legal = [true; CANDIDATES];
        for i in 0..CANDIDATES {
            if position(i).0 == 1 {
                legal[i] = false;
            }
        }
        let pos = choose(&[left], &shapes(), legal).unwrap();
        assert_eq!(pos, (2, 4));
    }
    #[test]
    fn current_coverage_does_not_pull_an_already_coverable_target_left() {
        let mut red = target(500, 2);
        red.required = true;
        red.current = ContactRect::new(660, 97, 680, 251);
        let pos = choose(&[red], &shapes(), [true; CANDIDATES]).unwrap();
        assert_eq!(pos, (1, 7));
    }
}
