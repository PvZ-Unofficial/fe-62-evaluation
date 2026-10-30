//! Six-flower 13-grid autonomous prototype; observable-state decisions, bounded diagnostics.
use PlantKind::*;
use ZombieKind as Z;
use rsvz::core::cards::{PlantSeedOutcome, find_usable_seed, try_plant_seed};
use rsvz::core::model::{ActivationKind, GameEvent, PlantEffectOutcome, Plantability, ZombiePhase};
use rsvz::prelude::{CardSelection, Grid, PlantKind, Wave, ZombieKind, on_before_exit, on_enter_fight};
use rsvz::runtime::{RuntimeError, RuntimeResult};
use rsvz::tick::{TickControl, TickOptions};
use std::cell::RefCell;

mod audit;
mod combat;
mod diagnostics;
mod dolphin;
mod effects;
#[cfg(test)]
mod first_wave_tests;
#[cfg(feature = "live-display")]
mod live_display;
mod perf;
mod planner;
mod policy;
mod prediction;
mod squash;
mod strategy;
mod telemetry;

type Pos = (i32, i32);
const LINEUP: &str = "LI4h32VZVBRckH61gyPX0MOjRkwacPgUPIbxHfUnIAHf7amAtT1/hPsdVP+UXwtX";
const COPY: CardSelection = CardSelection::Imitator(IceShroom);
const PUMPKINS: [Pos; 5] = [(2, 1), (2, 2), (3, 6), (4, 6), (5, 2)];
thread_local! {
    static CONTROL: RefCell<strategy::Control> = RefCell::new(strategy::Control::default());
}
fn err(e: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::new(e.to_string())
}
fn event_cache_mode() -> u32 {
    // The native live entry cannot subscribe to PE-only motion instrumentation.
    if cfg!(feature = "live-display") {
        0
    } else {
        setting("SIX_EVENT_CACHE", 1)
    }
}
fn grid(p: Pos) -> Grid {
    Grid::from_one_based(p.0, p.1).expect("script grid")
}
fn sel(k: PlantKind) -> CardSelection {
    CardSelection::Plant(k)
}
fn cd(c: CardSelection) -> i32 {
    rsvz::cards::card_cd(c)
}
fn ready(c: CardSelection) -> bool {
    find_usable_seed([c]).is_some()
}
fn setting(name: &str, default: u32) -> u32 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}
fn time() -> (i32, i32, i32) {
    let now = rsvz::timing::clock();
    let (w, t) = rsvz::core::timeline::with_timeline_ref(|tl| {
        (1..=20)
            .filter_map(|w| {
                tl.wave_clocks()
                    .refresh_clock(Wave(w))
                    .filter(|&start| start <= now)
                    .map(|start| (w, now - start))
            })
            .last()
            .unwrap_or_else(|| (0, now - tl.wave_clocks().refresh_clock(Wave(1)).unwrap_or(now)))
    });
    (now, w, t)
}
fn main_kind(p: Pos) -> Option<PlantKind> {
    rsvz::with_frame(|f| {
        let mut result = None;
        f.for_each_plant_at_anchor_grid(grid(p), |p| {
            if p.is_alive() && !matches!(p.raw_kind(), Pumpkin | LilyPad | FlowerPot) {
                result = Some(p.raw_kind());
            }
        });
        result
    })
}
fn hp(kind: PlantKind, p: Pos) -> Option<i32> {
    rsvz::with_frame(|f| {
        let mut result = None;
        f.for_each_plant_at_anchor_grid(grid(p), |p| {
            if p.is_alive() && p.raw_kind() == kind {
                result = Some(p.hp());
            }
        });
        result
    })
}
fn legal(c: CardSelection, p: Pos) -> bool {
    matches!(rsvz::core::cards::plantability(c, grid(p)), Ok(Plantability::Allowed))
}
fn play(c: CardSelection, p: Pos, reason: i32) -> bool {
    let Some(slot) = find_usable_seed([c]) else {
        return false;
    };
    if !legal(c, p) {
        return false;
    }
    let shadow = policy::shadow_prepare(c, p);
    let success = match try_plant_seed(slot, grid(p)) {
        Ok(PlantSeedOutcome::Planted(plant_id)) => {
            effects::planted(plant_id, c, p, reason);
            diagnostics::action(c, p, reason);
            telemetry::planted(plant_id, c, p, reason);
            audit::record(1, c, p);
            true
        }
        Ok(_) => {
            diagnostics::mark(diagnostics::Condition::PlantRejected);
            false
        }
        Err(e) => {
            rsvz::fail_script(err(e));
            false
        }
    };
    if shadow {
        policy::shadow_finish(success);
    }
    success
}
// Planner replacement candidates explicitly compare removing the old Puff.
fn play_puff(p: Pos, reason: i32) -> bool {
    place(sel(PuffShroom), p, reason, true)
}
fn placement_budget(c: CardSelection, p: Pos) -> bool {
    use rsvz::core::backend::PlantCostBackend;
    let lily = matches!(p.0, 3 | 4) && hp(LilyPad, p).is_none();
    if lily && (!ready(sel(LilyPad)) || !legal(sel(LilyPad), p)) {
        return false;
    }
    let cost = rsvz::with_backend(|b| {
        let main = b.current_plant_cost(c.checked().expect("card")).map_err(err)?.get();
        let extra = if lily {
            b.current_plant_cost(sel(LilyPad).checked().expect("lily"))
                .map_err(err)?
                .get()
        } else {
            0
        };
        Ok::<_, RuntimeError>(main + extra)
    });
    match cost {
        Ok(cost) => rsvz::board::sun() as i32 >= cost,
        Err(e) => {
            rsvz::fail_script(e);
            false
        }
    }
}
fn place(c: CardSelection, p: Pos, reason: i32, clear_puff: bool) -> bool {
    if !ready(c) {
        return false;
    }
    // Validate the combined transaction before removing a useful blocker or
    // spending the Lily's cooldown. Execution can still reject a changed site.
    if !combat::candidate(c, p) || !placement_budget(c, p) {
        return false;
    }
    if c == sel(CherryBomb) && !combat::cherry_preserves_ladders(p) {
        return false;
    }
    if c == sel(Jalapeno) && !rsvz::is_safe_jalapeno(grid(p)).unwrap_or(false) {
        return false;
    }
    let shadow = policy::shadow_prepare(c, p);
    if clear_puff && main_kind(p) == Some(PuffShroom) {
        telemetry::record("shovel", &[p.0 as i64, p.1 as i64]);
        rsvz::shovel::shovel(p.0, p.1);
        audit::record(2, sel(PuffShroom), p);
    }
    if p.0 == 3 || p.0 == 4 {
        if hp(LilyPad, p).is_none() && !play(sel(LilyPad), p, reason) {
            diagnostics::mark(diagnostics::Condition::LilyUnavailable);
            if shadow {
                policy::shadow_finish(false);
            }
            return false;
        }
    }
    let success = play(c, p, reason);
    if shadow {
        policy::shadow_finish(success);
    }
    success
}
fn choose_cards() -> RuntimeResult<Vec<CardSelection>> {
    // Reroll only the declared opening constraint, never inspect future waves in combat.
    use rsvz::core::model::{SPAWN_SLOTS_PER_WAVE, SpawnWaveSlot};
    use rsvz::core::zombies::{pick_spawn_list, refresh_spawn_preview, spawn_entry, spawn_wave_count};
    let mut rerolls = 0;
    loop {
        let bungees = (0..spawn_wave_count()).any(|w| {
            (0..SPAWN_SLOTS_PER_WAVE).any(|s| spawn_entry(w, SpawnWaveSlot::new(s).expect("slot")) == Some(Z::Bungee))
        });
        if !bungees {
            break;
        }
        if rerolls >= 100_000 {
            return Err(err("bungee reroll limit reached"));
        }
        pick_spawn_list();
        rerolls += 1;
    }
    if rerolls > 0 {
        refresh_spawn_preview();
    }
    diagnostics::rerolls(rerolls);
    Ok(vec![
        sel(IceShroom),
        COPY,
        sel(LilyPad),
        sel(DoomShroom),
        sel(CherryBomb),
        sel(Jalapeno),
        sel(Squash),
        sel(Pumpkin),
        sel(PuffShroom),
        sel(Blover),
    ])
}
#[rsvz::state_hooks]
fn hooks() {
    on_enter_fight(|| {
        if diagnostics::begin() {
            telemetry::next_round();
            CONTROL.with_borrow_mut(|control| control.next_round());
        } else {
            CONTROL.with_borrow_mut(|control| *control = strategy::Control::default());
            effects::clear();
            telemetry::clear();
        }
    });
    on_before_exit(|| {
        policy::replay_exit();
        if let Err(e) = diagnostics::publish() {
            rsvz::fail_script(e);
        }
    });
}
#[rsvz::script]
fn script() -> RuntimeResult<()> {
    if std::env::var_os("SIX_SUN_LIMIT").is_some() {
        rsvz::core::modifier::set_sunlight_limit(setting("SIX_SUN_LIMIT", 9990))?;
    }
    #[cfg(feature = "live-display")]
    live_display::install()?;
    reload(MainUiOrFightUi);
    lineup(LINEUP);
    skip_seed_chooser();
    // Include balloons from the demonstrated roster; preserve natural spawn weights and refresh.
    set_zombies(
        [
            Z::Normal,
            Z::GigaGargantuar,
            Z::Gargantuar,
            Z::Zomboni,
            Z::JackInTheBox,
            Z::Catapult,
            Z::Conehead,
            Z::Football,
            Z::Balloon,
            Z::Bungee,
        ],
        ZombieSpawnMode::Natural,
    );
    rsvz::setup::select_cards_with(choose_cards);
    rsvz::event::reserve(1024).map_err(err)?;
    if event_cache_mode() != 0 {
        rsvz::event::on_gargantuar_motion(rsvz::event::EventOptions::new(), |fact| {
            CONTROL.with_borrow_mut(|control| control.motion_changed(fact));
        })
        .map_err(err)?;
    }
    rsvz::event::on(rsvz::event::EventOptions::new(), |event| match event {
        GameEvent::Activation(_) => {}
        GameEvent::PlantEffect(e) => {
            effects::damaged(e);
            telemetry::plant_effect(e);
            if matches!(
                e.outcome,
                PlantEffectOutcome::Killed | PlantEffectOutcome::Squished | PlantEffectOutcome::Stolen
            ) {
                diagnostics::loss(e);
            }
        }
        GameEvent::HomeEntry(e) => diagnostics::home(e),
        _ => {}
    })
    .map_err(err)?;
    rsvz::tick::spawn(TickOptions::playing_frame(), |_| {
        if diagnostics::active() {
            effects::poll();
            CONTROL.with_borrow_mut(|control| control.tick());
        }
        Ok(TickControl::Continue)
    });
    diagnostics::install()?;
    Ok(())
}
