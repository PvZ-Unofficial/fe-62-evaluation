//! Source-owned game overlays. Built-in configuration is a validated no-op on PE.
mod config;
mod hover;
mod indices;
mod info;
mod markers;
mod observation;
mod primitives;
mod waves;
use crate::runtime::{RuntimeError, RuntimeResult};
pub use config::{Colors, IndexMode, InfoMode, Options};
pub use observation::external_wave_display;
pub use primitives::{Bar, Direction};
use rsvz_backend_api::*;
use rsvz_current::CurrentBackend;
use rsvz_model::{PlantId, SceneKind, WorldId};
use rsvz_schedule::tick::with_scheduler;
use rsvz_schedule::{TickControl, TickHandle, TickLifetime, TickOptions, TickTaskState};
use std::{cell::RefCell, collections::BTreeMap};
thread_local! {
    static SETTINGS: RefCell<crate::control::OwnedRule<Options>> = const { RefCell::new(crate::control::OwnedRule::new()) };
}
std::thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}
#[derive(Default)]
struct State {
    paint: Option<TickHandle>,
    tick: Option<TickHandle>,
    activation: Option<rsvz_schedule::event::EventHandle>,
    history: History,
}
struct History {
    world: Option<WorldId>,
    scene: Option<SceneKind>,
    production: BTreeMap<PlantId, (i32, i32)>,
    clocks: Vec<Option<i32>>,
    markers: Vec<markers::Marker>,
    marker_slots: BTreeMap<(i32, i32), u8>,
    front: [i32; 6],
    mouse_row: i32,
    mouse_col: f32,
}
impl Default for History {
    fn default() -> Self {
        Self {
            world: None,
            scene: None,
            production: BTreeMap::new(),
            clocks: Vec::new(),
            markers: Vec::new(),
            marker_slots: BTreeMap::new(),
            front: [10; 6],
            mouse_row: -1,
            mouse_col: -1.0,
        }
    }
}
impl History {
    fn observe_scene(&mut self, scene: SceneKind) {
        if self.scene.is_some_and(|previous| previous != scene) {
            // A scene edit can reuse Board and its clock/wave table. Only
            // spatial history belongs to the cleared terrain and entities.
            self.markers.clear();
            self.marker_slots.clear();
            self.production.clear();
            self.front = [10; 6];
            self.mouse_row = -1;
            self.mouse_col = -1.0;
        }
        self.scene = Some(scene);
    }
}
fn baseline() -> RuntimeResult<Options> {
    Ok(Options {
        show_me: false,
        ..Options::default()
    })
}
fn apply(_: Options) -> RuntimeResult<()> {
    Ok(())
}

/// Enable built-in overlays. The first call belongs in script registration
/// (or BeforeScript) so activation interests are established before native play;
/// subsequent changes reuse those session resources and can run while playing.
pub fn configure(options: Options) -> RuntimeResult<()>
where
    CurrentBackend: OptionalOverlayBackend
        + BoardReadinessBackend
        + GameUiBackend
        + WaveTimingBackend
        + PlantStateBackend
        + PlantPoolBackend
        + SeedCooldownReadBackend
        + CursorQueryBackend
        + ZombieRawFactsBackend
        + ZombieStateBackend
        + GridItemStateBackend
        + SceneBackend
        + BoardStateBackend
        + GridTerrainBackend
        + GameSpeedHintBackend
        + WaveHealthBackend
        + WorldIdentityBackend,
{
    configure_inner(options, false)
}

/// Supplies a fallback without overriding an explicit declaration from either source.
pub fn configure_fallback(options: Options) -> RuntimeResult<()>
where
    CurrentBackend: OptionalOverlayBackend
        + BoardReadinessBackend
        + GameUiBackend
        + WaveTimingBackend
        + PlantStateBackend
        + PlantPoolBackend
        + SeedCooldownReadBackend
        + CursorQueryBackend
        + ZombieRawFactsBackend
        + ZombieStateBackend
        + GridItemStateBackend
        + SceneBackend
        + BoardStateBackend
        + GridTerrainBackend
        + GameSpeedHintBackend
        + WaveHealthBackend
        + WorldIdentityBackend,
{
    configure_inner(options, true)
}

fn configure_inner(options: Options, fallback: bool) -> RuntimeResult<()>
where
    CurrentBackend: OptionalOverlayBackend
        + BoardReadinessBackend
        + GameUiBackend
        + WaveTimingBackend
        + PlantStateBackend
        + PlantPoolBackend
        + SeedCooldownReadBackend
        + CursorQueryBackend
        + ZombieRawFactsBackend
        + ZombieStateBackend
        + GridItemStateBackend
        + SceneBackend
        + BoardStateBackend
        + GridTerrainBackend
        + GameSpeedHintBackend
        + WaveHealthBackend
        + WorldIdentityBackend,
{
    if options.marker_duration > i32::MAX as u32 {
        return Err(RuntimeError::new(
            "overlay duration or wavelength count is out of range",
        ));
    }
    if CurrentBackend::OVERLAY_IS_NOOP {
        return Ok(());
    }
    let needs_task = STATE.with_borrow(|state| {
        state
            .paint
            .is_none_or(|task| with_scheduler(|s| s.state(task)) == TickTaskState::Stopped)
    });
    if needs_task {
        let previous_activation = STATE.with_borrow(|state| state.activation);
        let activation = match previous_activation {
            Some(handle) => handle,
            None => markers::register()?,
        };
        let task = crate::paint::register(
            TickOptions::active_phase()
                .lifetime(TickLifetime::Session)
                .name("game_overlays"),
            |canvas| {
                let options = SETTINGS.with_borrow(|rule| rule.effective_declaration());
                let Some(options) = options else {
                    return Ok(());
                };
                let world = crate::board::world_id()?;
                let scene = crate::access::with_backend(|b| b.scene()).map_err(crate::access::operation_error)?;
                STATE.with_borrow_mut(|state| {
                    if state.history.world != world {
                        state.history = History::default();
                        state.history.world = world;
                    }
                    state.history.observe_scene(scene);
                    // Retained activation drawings precede PAINT-runner info
                    // and index drawing in AvZ's DrawEveryTick.
                    canvas.set_origin(canvas.board_position()?.unwrap_or(rsvz_model::PixelPos { x: 0, y: 0 }));
                    markers::draw(canvas, &state.history)?;
                    info::draw(canvas, options, &mut state.history)?;
                    indices::draw(canvas, options, &mut state.history)?;
                    hover::draw(canvas, options, &mut state.history)
                })
            },
        );
        let task = match task {
            Ok(task) => task,
            Err(error) => {
                if previous_activation.is_none() {
                    rsvz_schedule::event::with_events(|events| events.remove_public_handler(activation));
                }
                return Err(error);
            }
        };
        STATE.with_borrow_mut(|state| state.activation = Some(activation));
        let tick = crate::tick::spawn(
            TickOptions::active_phase()
                .lifetime(TickLifetime::Session)
                .idle_neutral()
                .name("overlay_clocks"),
            |_| {
                update_history()?;
                Ok(TickControl::Continue)
            },
        );
        STATE.with_borrow_mut(|state| {
            state.paint = Some(task);
            state.tick = Some(tick);
        });
        crate::state_hook::register_fallible(rsvz_schedule::state_hook::StateEvent::ExitFight, 0, || {
            STATE.with_borrow_mut(|state| {
                state.history.markers.clear();
                state.history.marker_slots.clear();
                state.history.clocks.clear();
            });
            Ok(())
        });
        crate::state_hook::register_native_cleanup(i32::MAX, || {
            drop(STATE.take());
            Ok(())
        });
    }
    let previous = SETTINGS.with_borrow(|rule| rule.declaration());
    if fallback {
        crate::control::release(&SETTINGS, baseline, apply)?;
        crate::control::declare_fallback(&SETTINGS, options, baseline, apply)?;
    } else {
        crate::control::declare(&SETTINGS, options, baseline, apply)?;
    }
    if let Some(task) = STATE.with_borrow(|state| state.paint) {
        with_scheduler(|scheduler| {
            let _ = scheduler.resume(task);
        });
    }
    if previous.is_none_or(|old| old.index != options.index) {
        STATE.with_borrow_mut(|state| state.history.front = [10; 6]);
    }
    Ok(())
}
pub fn release() -> RuntimeResult<()> {
    crate::control::release(&SETTINGS, baseline, apply)
}
pub fn settings() -> Options {
    SETTINGS.with_borrow(|rule| rule.declaration()).unwrap_or_default()
}
fn update_history() -> RuntimeResult<()>
where
    CurrentBackend: WorldIdentityBackend + WaveTimingBackend + SceneBackend,
{
    crate::access::with_backend(|b| -> RuntimeResult<()> {
        let world = b.world_identity()?;
        let scene = b.scene()?;
        let total = b.total_waves()?;
        if !(0..=100).contains(&total) {
            return Err(RuntimeError::new("invalid native wave count"));
        }
        let (wave, clock, countdown, huge, end) = (
            b.current_wave()?.0,
            b.clock()?,
            b.refresh_countdown()?,
            b.huge_wave_countdown()?,
            b.level_end_countdown()?,
        );
        let initial = b.initial_countdown()?;
        STATE.with_borrow_mut(|state| {
            let history = &mut state.history;
            if history.world != world {
                *history = History::default();
                history.world = world;
            }
            history.observe_scene(scene);
            history.clocks.resize(total as usize + 1, None);
            markers::advance(history);
            if total == 0 {
                return;
            }
            if wave > 0
                && let Some(slot) = history.clocks.get_mut((wave - 1) as usize)
                && slot.is_none()
                && !(countdown <= 200 && initial - countdown > 400)
            {
                // Same native timer relation used by the existing timeline.
                *slot = Some(clock + countdown - initial);
            }
            let remaining = if wave == total {
                end
            } else if countdown > 200 {
                0
            } else if wave % 10 == 9 {
                if countdown <= 5 { huge } else { countdown + 745 }
            } else {
                countdown
            };
            if remaining != 0
                && let Some(slot) = history.clocks.get_mut(wave as usize)
            {
                *slot = Some(clock + remaining);
            }
        });
        Ok(())
    })
}

/// Resolved configured overlay after source arbitration; not a pixel verification.
pub fn effective_settings() -> Option<Options> {
    SETTINGS.with_borrow(|rule| rule.effective_declaration())
}

pub fn reset_front()
where
    CurrentBackend: OptionalOverlayBackend,
{
    if !CurrentBackend::OVERLAY_IS_NOOP {
        STATE.with_borrow_mut(|state| state.history.front = [10; 6]);
    }
}
