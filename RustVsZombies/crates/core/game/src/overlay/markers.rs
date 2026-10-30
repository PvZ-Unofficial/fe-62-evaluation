//! Activation overlays consume resolved native facts after the native callback,
//! never infer explosion success by dereferencing a disappeared projectile.
use super::info::{grid_y, text};
use super::primitives::rect;
use super::{History, SETTINGS, STATE};
use crate::paint::{Argb, PaintCanvas};
use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::SceneBackend;
use rsvz_current::CurrentBackend;
use rsvz_model::{ActivationFact, ActivationKind, GameEvent};
use rsvz_schedule::event::{EventHandle, EventLifetime, EventOptions, with_events};
pub(super) struct Marker {
    remaining: u32,
    x: i32,
    y: i32,
    time: Option<i32>,
    target_x: i32,
    color: Argb,
}
pub(super) fn register() -> RuntimeResult<EventHandle>
where
    CurrentBackend: SceneBackend,
{
    with_events(|events| {
        events.register_public_handler(
            rsvz_model::EventInterest::ACTIVATION,
            EventOptions::new().lifetime(EventLifetime::Session),
            Box::new(|event| {
                if let GameEvent::Activation(fact) = event
                    && let Err(error) = record(*fact)
                {
                    crate::diagnostics::abort_operation(error);
                }
            }),
        )
    })
    .map_err(|error| RuntimeError::new(error.to_string()))
}
fn record(fact: ActivationFact) -> RuntimeResult<()>
where
    CurrentBackend: SceneBackend,
{
    let options = SETTINGS.with_borrow(|rule| rule.effective_declaration());
    let Some(options) = options.filter(|options| options.activation_time && options.marker_duration != 0) else {
        return Ok(());
    };
    let scene = crate::access::with_backend(|b| b.scene()).map_err(crate::access::operation_error)?;
    STATE.with_borrow_mut(|state| {
        let h = &mut state.history;
        h.observe_scene(scene);
        if h.markers.len() >= 10000 {
            return Err(RuntimeError::new("activation marker queue is full"));
        }
        let slot = h.marker_slots.entry((fact.grid.row, fact.grid.col)).or_default();
        let offset = i32::from(*slot) * 15;
        *slot = (*slot + 1) % 4;
        let (x, y) = match fact.kind {
            ActivationKind::Cob | ActivationKind::Squash => {
                (40 + fact.grid.col * 80, grid_y(scene, fact.grid.row, fact.grid.col))
            }
            _ => (fact.position.x, fact.position.y),
        };
        let c = options.colors;
        let color = match fact.kind {
            ActivationKind::Cob => c.marker_cob,
            ActivationKind::Ice => c.marker_ice,
            ActivationKind::Doom => c.marker_doom,
            ActivationKind::Cherry => c.marker_cherry,
            ActivationKind::Jalapeno => c.marker_jalapeno,
            ActivationKind::Squash => c.marker_squash,
            ActivationKind::Mine => c.marker_mine,
        };
        let time = relative_time(fact, h);
        h.markers.push(Marker {
            remaining: options.marker_duration,
            x,
            y: y + offset,
            time,
            target_x: fact.target_x,
            color,
        });
        Ok(())
    })
}
pub(super) fn draw(canvas: &mut dyn PaintCanvas, history: &History) -> RuntimeResult<()> {
    for marker in &history.markers {
        rect(canvas, marker.x + 4, marker.y + 9, 72, 14, marker.color)?;
        let time = marker.time.map_or_else(|| "?".to_owned(), |t| t.to_string());
        text(
            canvas,
            marker.x + 3,
            marker.y + 5,
            &format!("{time:<4}  {:02}", marker.target_x % 80 * 125 / 100),
            17,
            Argb(0xffffffff),
        )?;
        let col = marker.target_x / 80;
        text(
            canvas,
            marker.x + 44,
            marker.y + 5,
            &if col < 10 { format!("{col}.") } else { "X.".to_owned() },
            17,
            Argb(0xffffffff),
        )?;
    }
    Ok(())
}

fn relative_time(fact: ActivationFact, history: &History) -> Option<i32> {
    if fact.wave == 0 {
        return Some(1 - fact.refresh_countdown);
    }
    if let Some(start) = history.clocks.get((fact.wave - 1) as usize).copied().flatten() {
        return Some(fact.main_counter.saturating_add(1) - start);
    }
    // A mid-wave attach after the HP gate opened cannot reconstruct a lost
    // wave start; keep it visibly unknown rather than inventing an elapsed time.
    (!(fact.refresh_countdown <= 200 && fact.initial_countdown - fact.refresh_countdown > 400))
        .then_some(fact.initial_countdown - fact.refresh_countdown + 1)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scene_edit_clears_old_geometry_but_preserves_wave_history() {
        let mut history = History::default();
        history.observe_scene(rsvz_model::SceneKind::Pool);
        history.clocks = vec![Some(10), Some(610)];
        history.markers.push(Marker {
            remaining: 5000,
            x: 40,
            y: 500,
            time: Some(1),
            target_x: 80,
            color: Argb(0xffffffff),
        });
        history.marker_slots.insert((5, 0), 1);
        history.observe_scene(rsvz_model::SceneKind::Pool);
        assert_eq!(history.markers.len(), 1);
        history.observe_scene(rsvz_model::SceneKind::Day);
        assert!(history.markers.is_empty());
        assert!(history.marker_slots.is_empty());
        assert_eq!(history.clocks, [Some(10), Some(610)]);
    }

    #[test]
    fn duration_advances_even_when_the_board_clock_is_paused() {
        let mut history = History::default();
        history.markers.push(Marker {
            remaining: 1,
            x: 0,
            y: 0,
            time: Some(0),
            target_x: 80,
            color: Argb(0xff000000),
        });
        advance(&mut history);
        assert_eq!(history.markers.len(), 1);
        assert_eq!(history.markers[0].remaining, 0);
        advance(&mut history);
        assert!(history.markers.is_empty());
    }
    #[test]
    fn activation_time_uses_native_phase_and_never_fabricates_lost_history() {
        let mut fact = ActivationFact {
            kind: ActivationKind::Cherry,
            grid: rsvz_model::Grid { row: 0, col: 0 },
            position: rsvz_model::PixelPos { x: 40, y: 80 },
            target_x: 80,
            main_counter: 500,
            wave: 0,
            refresh_countdown: 600,
            initial_countdown: 600,
        };
        let mut history = History::default();
        assert_eq!(relative_time(fact, &history), Some(-599));
        fact.wave = 1;
        fact.refresh_countdown = 450;
        assert_eq!(relative_time(fact, &history), Some(151));
        fact.refresh_countdown = 100;
        assert_eq!(relative_time(fact, &history), None);
        history.clocks = vec![Some(20)];
        assert_eq!(relative_time(fact, &history), Some(481));
    }
}

// AvZ updates multi-tick painter durations on GLOBAL dispatch, including
// advanced pause; this is deliberately independent of the Board clock.
pub(super) fn advance(history: &mut History) {
    history.markers.retain_mut(|marker| {
        if marker.remaining == 0 {
            false
        } else {
            marker.remaining -= 1;
            true
        }
    });
}
