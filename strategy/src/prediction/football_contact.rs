//! Last native bite check that retains contact after the movement update.
use rsvz::core::model::{ZombieMotionError, ZombieMotionState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    Open,
    LastCheckIn(u32),
    Missed,
}

pub fn window(
    state: &ZombieMotionState, age: i32, col: i32, horizon: u32, certain_ice_at: Option<i32>,
) -> Result<Window, ZombieMotionError> {
    let xs = rsvz::core::zombie_motion::predict_stable_zombie_x_trace(state, horizon)?;
    let left = col * 80 - 80;
    let right = left + 40;
    let mut last = None;
    for dt in 1..=horizon {
        // Plant effects precede the zombie update. A known incoming ice while
        // the actor still has contact room leaves a later window after thawing.
        let x = xs[(dt - 1) as usize] as i32;
        if certain_ice_at.is_some_and(|at| at <= dt as i32) && x >= left {
            return Ok(Window::Open);
        }
        if x < left {
            return Ok(last.map_or(Window::Missed, Window::LastCheckIn));
        }
        let chilled = state.counters.chilled > dt as i32;
        let frozen = state.counters.frozen > dt as i32 || state.counters.buttered > dt as i32;
        let interval = if chilled { 8 } else { 4 };
        // Age increments and debuffs decrement first; movement happens before
        // eating, but attack-box int_x is updated only at the end of the tick.
        // Starting to eat is insufficient if this update's movement has
        // already crossed the left edge. The next prey check would release
        // that transient bite. Keep the post-move box in contact as well.
        let stopped_x = xs[dt as usize] as i32;
        if !frozen && (age + dt as i32) % interval == 0 && x <= right && stopped_x >= left {
            last = Some(dt);
        }
    }
    Ok(Window::Open)
}
