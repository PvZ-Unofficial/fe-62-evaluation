//! Ordinary fodder, ported from AvZ2's AFodder (shorthand_240713.h).
//! Unlike smart_fodder, this executes at the requested time and never predicts or replans.

use std::cell::{Cell, RefCell};
use std::ops::BitAnd;
use std::rc::Rc;

use rsvz_backend_api::{
    CurrentWaveBackend, GridTerrainBackend, PlantReadBackend, SceneBackend, SeedBankReadBackend,
    SeedCooldownReadBackend, SeedDefinitionBackend, ZombieRawFactsBackend, ZombieReadBackend,
};
use rsvz_current::CurrentBackend;
use rsvz_model::{CardSelection, Grid, PlantKind, ZombieId, ZombieKind};
use rsvz_schedule::state_hook::{StateEvent, StateHookHandle};

use crate::live_value::read_or_abort;
use crate::logic::cards::{CardContext, IntoCardSelections};
use crate::runtime::{RuntimeError, RuntimeResult};

#[derive(Default)]
struct State {
    cards: Vec<CardSelection>,
    manual: bool,
    hook: Option<StateHookHandle>,
}

/// A configurable candidate list. Clones share the list, as AvZ timelines refer to their AFodder.
#[derive(Clone)]
pub struct Fodder {
    state: Option<Rc<RefCell<State>>>,
}

impl Default for Fodder {
    fn default() -> Self {
        Self {
            state: Some(Rc::new(RefCell::new(State::default()))),
        }
    }
}

std::thread_local! {
    static DEFAULT_STATE: Rc<RefCell<State>> = Rc::new(RefCell::new(State::default()));
}

impl Fodder {
    /// Handle used by the default DSL object; concrete state remains function-owned TLS.
    pub const DEFAULT: Self = Self { state: None };

    fn state(&self) -> Rc<RefCell<State>> {
        self.state.clone().unwrap_or_else(|| DEFAULT_STATE.with(Rc::clone))
    }

    pub fn new() -> Self
    where
        CurrentBackend: SeedBankReadBackend + SeedDefinitionBackend,
    {
        let fodder = Self {
            state: Some(Rc::new(RefCell::new(State::default()))),
        };
        fodder.install();
        fodder
    }

    /// Installs the automatic EnterFight selection without retaining dead user managers.
    pub fn install(&self)
    where
        CurrentBackend: SeedBankReadBackend + SeedDefinitionBackend,
    {
        if self
            .state()
            .borrow()
            .hook
            .is_some_and(|handle| rsvz_schedule::state_hook::with_state_hooks(|hooks| hooks.is_registered(handle)))
        {
            return;
        }
        // A cleared registry starts a fresh session; do not leak the previous default list.
        if self.state().borrow().hook.is_some() {
            *self.state().borrow_mut() = State::default();
        }
        let weak = Rc::downgrade(&self.state());
        let handles = Rc::new(Cell::new(None::<(StateHookHandle, StateHookHandle)>));
        let expired_handles = Rc::clone(&handles);
        let hook = crate::state_hook::register_fallible(StateEvent::EnterFight, 0, move || {
            if let Some(state) = weak.upgrade() {
                let automatic = !state.borrow().manual;
                if automatic {
                    Fodder { state: Some(state) }.auto_set_cards()?;
                }
            } else if let Some((enter, exit)) = expired_handles.get() {
                // Timelines from an old script generation may have owned the last strong handle.
                // Retire both weak callbacks at the next safe dispatch instead of accumulating them.
                crate::state_hook::remove_state_hook(enter);
                crate::state_hook::remove_state_hook(exit);
            }
            Ok(())
        });
        self.state().borrow_mut().hook = Some(hook);
        let weak = Rc::downgrade(&self.state());
        let exit = crate::state_hook::register_fallible(StateEvent::BeforeExit, 0, move || {
            if let Some(state) = weak.upgrade() {
                *state.borrow_mut() = State::default();
            }
            Ok(())
        });
        handles.set(Some((hook, exit)));
    }

    pub fn set_cards(&self, cards: impl IntoCardSelections) -> RuntimeResult<()>
    where
        CurrentBackend: SeedBankReadBackend + SeedDefinitionBackend,
    {
        let cards = cards.into_card_selections();
        validate_cards(&cards)?;
        self.install();
        let owner = self.state();
        let mut state = owner.borrow_mut();
        state.cards = cards;
        state.manual = true;
        Ok(())
    }

    /// Select carried 750cs-definition cards by (base sun cost, AvZ card number).
    /// Excludes GraveBuster, LilyPad and CoffeeBean, including their imitators.
    /// As in AvZ, this does not clear a previous manual-selection flag.
    pub fn auto_set_cards(&self) -> RuntimeResult<()>
    where
        CurrentBackend: SeedBankReadBackend + SeedDefinitionBackend,
    {
        self.install();
        let mut candidates = crate::access::with_backend(|backend| {
            let mut candidates = Vec::new();
            for seed in read_or_abort(backend.seeds(), "fodder seed bank") {
                let selection = read_or_abort(backend.seed_selection(seed), "fodder seed selection");
                if matches!(
                    selection.effective_kind(),
                    PlantKind::GraveBuster | PlantKind::LilyPad | PlantKind::CoffeeBean
                ) {
                    continue;
                }
                if read_or_abort(backend.seed_base_recharge(selection), "fodder recharge") == 750 {
                    candidates.push((
                        read_or_abort(backend.seed_base_cost(selection), "fodder base cost"),
                        card_number(selection.selection()),
                        selection.selection(),
                    ));
                }
            }
            candidates
        });
        candidates.sort_by_key(|&(cost, number, _)| (cost, number));
        self.state().borrow_mut().cards = candidates.into_iter().map(|(_, _, selection)| selection).collect();
        Ok(())
    }

    pub fn cards(&self) -> Vec<CardSelection> {
        self.state().borrow().cards.clone()
    }

    pub fn erase_from_list(&self, cards: impl IntoCardSelections) {
        let cards = cards.into_card_selections();
        self.state().borrow_mut().cards.retain(|card| !cards.contains(card));
    }

    pub fn move_to_list_top(&self, cards: impl IntoCardSelections) -> RuntimeResult<()> {
        self.move_cards(cards.into_card_selections(), true)
    }

    pub fn move_to_list_bottom(&self, cards: impl IntoCardSelections) -> RuntimeResult<()> {
        self.move_cards(cards.into_card_selections(), false)
    }

    fn move_cards(&self, cards: Vec<CardSelection>, front: bool) -> RuntimeResult<()> {
        validate_cards(&cards)?;
        let owner = self.state();
        let mut state = owner.borrow_mut();
        state.cards.retain(|card| !cards.contains(card));
        if front {
            state.cards.splice(..0, cards);
        } else {
            state.cards.extend(cards);
        }
        Ok(())
    }

    pub fn available_count(&self, max_cd: i32) -> usize
    where
        CurrentBackend: SeedBankReadBackend + SeedCooldownReadBackend,
    {
        crate::access::with_backend(|backend| {
            let owner = self.state();
            let state = owner.borrow();
            state
                .cards
                .iter()
                .filter(|&&selection| {
                    read_or_abort(backend.seeds(), "fodder seed bank").any(|seed| {
                        read_or_abort(backend.seed_selection(seed), "fodder seed selection").selection() == selection
                            && (backend.seed_is_usable(seed) || backend.seed_cooldown_remaining(seed) <= max_cd)
                    })
                })
                .count()
        })
    }

    /// Executes one batch. `targets` stores attempted coordinates/types, not successful plant IDs:
    /// AvZ cleanup intentionally also sees replacement plants and existing container layers.
    pub fn plant(
        &self, positions: &[Grid], constraints: &[Constraint], targets: &mut Vec<(Grid, CardSelection)>,
    ) -> RuntimeResult<()>
    where
        CurrentBackend: CardContext + SceneBackend + ZombieRawFactsBackend + CurrentWaveBackend,
    {
        targets.clear();
        let rows = triggered_rows(constraints);
        let row_count = crate::access::with_backend(|b| read_or_abort(b.scene(), "fodder scene").row_count());
        if positions
            .iter()
            .any(|grid| grid.row < 0 || grid.row >= row_count as i32)
        {
            return Err(RuntimeError::new("fodder row is outside the current scene"));
        }
        let mut next = 0;
        for &grid in positions {
            if !constraints.is_empty() && !rows[grid.row as usize] {
                continue;
            }
            let selection = loop {
                let selection = self.state().borrow().cards.get(next).copied();
                let Some(selection) = selection else {
                    let cooldown = crate::access::with_backend(|backend| {
                        let owner = self.state();
                        let state = owner.borrow();
                        let mut cooldown = 9999;
                        for seed in read_or_abort(backend.seeds(), "fodder cooldown bank") {
                            if state.cards.contains(
                                &read_or_abort(backend.seed_selection(seed), "fodder cooldown selection").selection(),
                            ) {
                                cooldown = cooldown.min(backend.seed_cooldown_remaining(seed));
                            }
                        }
                        cooldown
                    });
                    crate::diagnostics::report_runtime_error(&RuntimeError::new(format!(
                        "AFodder: 距下一个垫材可用还有 {cooldown} cs"
                    )));
                    return Ok(());
                };
                next += 1;
                if crate::logic::cards::find_usable_seed([selection]).is_some() {
                    break selection;
                }
            };
            // ACard does not install automatic containers: use the exact packet path, not Card.
            let slot = crate::logic::cards::find_seed_slot(selection).expect("selected fodder packet exists");
            let result = crate::logic::cards::card_slot(slot, grid.row + 1, grid.col + 1);
            targets.push((grid, selection));
            if let Err(error) = result {
                crate::diagnostics::report_runtime_error(error);
            }
        }
        Ok(())
    }
}

fn validate_cards(cards: &[CardSelection]) -> RuntimeResult<()> {
    for &selection in cards {
        selection.checked().map_err(|e| RuntimeError::new(e.to_string()))?;
    }
    Ok(())
}

fn card_number(selection: CardSelection) -> i32 {
    match selection {
        CardSelection::Plant(kind) => kind as i32,
        CardSelection::Imitator(kind) => kind as i32 + 49,
    }
}

/// Conjunction inside one constraint; the list passed to TriggerBy is a disjunction.
#[derive(Clone)]
pub enum Constraint {
    Kind(ZombieKind),
    CurrentWave,
    PreviousWaves,
    RelativeWaves(Vec<i32>),
    Abscissa(i32, i32),
    Predicate(Rc<dyn Fn(ZombieId) -> bool>),
    And(Rc<Constraint>, Rc<Constraint>),
}

impl From<ZombieKind> for Constraint {
    fn from(kind: ZombieKind) -> Self {
        Self::Kind(kind)
    }
}
impl BitAnd for Constraint {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self::And(Rc::new(self), Rc::new(rhs))
    }
}
impl BitAnd<Constraint> for ZombieKind {
    type Output = Constraint;
    fn bitand(self, rhs: Constraint) -> Constraint {
        Constraint::Kind(self) & rhs
    }
}
impl Constraint {
    /// Receives an ID valid for this invocation; normal public Zombie operations can query it.
    pub fn predicate(f: impl Fn(ZombieId) -> bool + 'static) -> Self {
        Self::Predicate(Rc::new(f))
    }
    fn matches<'a>(
        &self, backend: &'a CurrentBackend, zombie: <CurrentBackend as ZombieReadBackend>::ZombieHandle<'a>, wave: i32,
    ) -> bool
    where
        CurrentBackend: ZombieRawFactsBackend,
    {
        match self {
            Self::Kind(kind) => read_or_abort(backend.zombie_kind(zombie), "fodder zombie kind") == *kind,
            Self::CurrentWave => backend.zombie_from_wave(zombie) + 1 == wave,
            Self::PreviousWaves => backend.zombie_from_wave(zombie) + 1 < wave,
            Self::RelativeWaves(waves) => waves.contains(&(backend.zombie_from_wave(zombie) + 1 - wave)),
            Self::Abscissa(left, right) => {
                let x = backend.zombie_pos_x(zombie) as i32;
                *left <= x && x <= *right
            }
            Self::Predicate(f) => f(backend.zombie_id(zombie)),
            Self::And(left, right) => left.matches(backend, zombie, wave) && right.matches(backend, zombie, wave),
        }
    }
}

fn triggered_rows(constraints: &[Constraint]) -> [bool; 6]
where
    CurrentBackend: ZombieRawFactsBackend + CurrentWaveBackend,
{
    let mut rows = [false; 6];
    if constraints.is_empty() {
        return rows;
    }
    crate::access::with_backend(|backend| {
        // AvZ ANowWave() clamps the pre-first-wave native value to one.
        let wave = read_or_abort(backend.current_wave(), "fodder current wave").0.max(1);
        for zombie in read_or_abort(backend.zombies(), "fodder zombies") {
            if !backend.zombie_is_alive(zombie) {
                continue;
            }
            let row = backend.zombie_row(zombie);
            if let Some(triggered) = usize::try_from(row).ok().and_then(|row| rows.get_mut(row)) {
                if !*triggered {
                    *triggered = constraints.iter().any(|c| c.matches(backend, zombie, wave));
                }
            }
        }
    });
    rows
}

/// Resolves C(delay)'s default land rows when the expression is constructed, as in AvZ.
pub fn land_positions() -> Vec<Grid>
where
    CurrentBackend: SceneBackend + GridTerrainBackend,
{
    crate::access::with_backend(|backend| {
        let scene = read_or_abort(backend.scene(), "fodder scene");
        (0..scene.row_count() as i32)
            .filter(|&row| {
                read_or_abort(backend.row_can_have_zombies(row), "fodder row")
                    && !read_or_abort(backend.is_pool_square(8, row), "fodder terrain")
            })
            .map(|row| Grid { row, col: 8 })
            .collect()
    })
}

/// AvZ's position/type cleanup, including LilyPad, Pumpkin and FlowerPot layers.
pub fn remove(targets: &[(Grid, CardSelection)]) -> RuntimeResult<()>
where
    CurrentBackend: crate::logic::shovel::ShovelContext,
{
    use crate::logic::shovel::{ShovelTarget, shovel_target_at};
    for &(grid, selection) in targets {
        let kind = match selection {
            CardSelection::Plant(k) | CardSelection::Imitator(k) => k,
        };
        let present = crate::access::with_backend(|backend| {
            read_or_abort(backend.plants(), "fodder cleanup plants").any(|plant| {
                backend.plant_row(plant) == grid.row && backend.plant_col(plant) == grid.col && {
                    let raw = read_or_abort(backend.plant_raw_kind(plant), "fodder cleanup kind");
                    raw == kind || (matches!(selection, CardSelection::Imitator(_)) && raw == PlantKind::Imitator)
                }
            })
        });
        if !present {
            continue;
        }
        if matches!(selection, CardSelection::Imitator(_)) {
            // AShovel(imitator-card) first shovels any untransformed imitator in this cell.
            let placeholder = crate::access::with_backend(|backend| {
                read_or_abort(backend.plants(), "fodder imitator cleanup").find_map(|plant| {
                    (backend.plant_row(plant) == grid.row
                        && backend.plant_col(plant) == grid.col
                        && read_or_abort(backend.plant_raw_kind(plant), "fodder imitator kind") == PlantKind::Imitator)
                        .then(|| backend.plant_id(plant))
                })
            });
            if let Some(id) = placeholder {
                crate::modifier::remove_plant_by_id(id);
            }
        }
        shovel_target_at(grid, ShovelTarget::Plant(CardSelection::Plant(kind)))
            .map_err(|e| RuntimeError::new(e.to_string()))?;
        for kind in [PlantKind::LilyPad, PlantKind::Pumpkin, PlantKind::FlowerPot] {
            shovel_target_at(grid, ShovelTarget::Plant(CardSelection::Plant(kind)))
                .map_err(|e| RuntimeError::new(e.to_string()))?;
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "backend-tests"))]
mod tests {
    use super::*;

    #[test]
    fn expired_managers_unregister_their_hooks_at_the_next_fight() {
        crate::state_hook::clear_state_hooks();
        let manager = Fodder::new();
        let hook = manager.state().borrow().hook.unwrap();
        drop(manager);
        assert!(rsvz_schedule::state_hook::with_state_hooks(
            |hooks| hooks.is_registered(hook)
        ));
        assert!(matches!(
            crate::state_hook::dispatch_state_event(StateEvent::EnterFight),
            rsvz_schedule::state_hook::StateHookDispatchResult::Continue
        ));
        assert!(!rsvz_schedule::state_hook::with_state_hooks(
            |hooks| hooks.is_registered(hook)
        ));
    }
}
