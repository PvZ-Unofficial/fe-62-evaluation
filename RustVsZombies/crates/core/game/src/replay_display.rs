//! Replay presentation, independent of the ordinary information overlay switch.
use crate::overlay::{Bar, Direction};
use crate::paint::{Argb, PaintFont};
use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::{BoardReadinessBackend, GameUiBackend, OptionalOverlayBackend, WaveTimingBackend};
use rsvz_current::CurrentBackend;
use rsvz_model::{PixelPos, ReplayMode};
use rsvz_schedule::{TickHandle, TickLifetime, TickOptions, TickPriority, TickTaskState, tick::with_scheduler};
use std::cell::{Cell, RefCell};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presentation {
    pub show_mouse: bool,
    pub show_info: bool,
}
impl Default for Presentation {
    fn default() -> Self {
        Self {
            show_mouse: true,
            show_info: true,
        }
    }
}
thread_local! {static OPTIONS:RefCell<crate::control::OwnedRule<Presentation>>=const{RefCell::new(crate::control::OwnedRule::new())};}
std::thread_local! {static TASK:Cell<Option<TickHandle>>=const{Cell::new(None)};}
fn baseline() -> RuntimeResult<Presentation> {
    Ok(Presentation {
        show_mouse: false,
        show_info: false,
    })
}
fn apply(_: Presentation) -> RuntimeResult<()> {
    Ok(())
}
pub fn configure(options: Presentation) -> RuntimeResult<()>
where
    CurrentBackend: OptionalOverlayBackend + BoardReadinessBackend + GameUiBackend + WaveTimingBackend,
{
    configure_inner(options, false)
}
pub fn configure_fallback(options: Presentation) -> RuntimeResult<()>
where
    CurrentBackend: OptionalOverlayBackend + BoardReadinessBackend + GameUiBackend + WaveTimingBackend,
{
    configure_inner(options, true)
}
fn configure_inner(options: Presentation, fallback: bool) -> RuntimeResult<()>
where
    CurrentBackend: OptionalOverlayBackend + BoardReadinessBackend + GameUiBackend + WaveTimingBackend,
{
    if CurrentBackend::OVERLAY_IS_NOOP {
        return Ok(());
    }
    if TASK
        .get()
        .is_none_or(|task| with_scheduler(|scheduler| scheduler.state(task)) == TickTaskState::Stopped)
    {
        let task = crate::paint::register(
            TickOptions::active_phase()
                .lifetime(TickLifetime::Session)
                .priority(TickPriority::HIGH)
                .name("replay_presentation"),
            |canvas| {
                let options = OPTIONS.with_borrow(|rule| rule.effective_declaration());
                let Some(options) = options else {
                    return Ok(());
                };
                let status = crate::replay::status();
                if options.show_mouse
                    && status.mode == ReplayMode::Playing
                    && let Some(cursor) = crate::replay::cursor()
                {
                    canvas.cursor(
                        PixelPos {
                            x: cursor.x,
                            y: cursor.y,
                        },
                        cursor.kind == 1,
                    )?;
                }
                if options.show_info && matches!(status.mode, ReplayMode::Recording | ReplayMode::Playing) {
                    let (label, index, color) = if status.mode == ReplayMode::Recording {
                        ("Rec.", status.end_index, Argb(0xffff0000))
                    } else {
                        (
                            if status.tail_pending { "Snap" } else { "Play" },
                            status.position.unwrap_or(status.start_index),
                            Argb(0xff0000ff),
                        )
                    };
                    Bar {
                        x: 685,
                        y: 3,
                        denominator: 1.0,
                        numerator: 0.0,
                        separators: &[],
                        direction: Direction::Right,
                        length: 106,
                        width: 24,
                        color: Argb(0xffffc000),
                        background: Argb(0xc0ffffff),
                        frame: Argb(0xff000000),
                        border: 1,
                    }
                    .draw(canvas)?;
                    let font = PaintFont::new("", 17, false).map_err(|error| RuntimeError::new(error.to_string()))?;
                    canvas.text(PixelPos { x: 689, y: 4 }, label, font, color)?;
                    canvas.text(PixelPos { x: 733, y: 4 }, &index.to_string(), font, color)?;
                    if status.tail_pending {
                        canvas.text(
                            PixelPos { x: 635, y: 30 },
                            "Input tail not played",
                            font,
                            Argb(0xffff8000),
                        )?;
                    }
                }
                Ok(())
            },
        )?;
        TASK.set(Some(task));
    }
    if fallback {
        crate::control::release(&OPTIONS, baseline, apply)?;
        crate::control::declare_fallback(&OPTIONS, options, baseline, apply)?;
    } else {
        crate::control::declare(&OPTIONS, options, baseline, apply)?;
    }
    if let Some(task) = TASK.get() {
        with_scheduler(|scheduler| {
            let _ = scheduler.resume(task);
        });
    }
    Ok(())
}
pub fn release() -> RuntimeResult<()> {
    crate::control::release(&OPTIONS, baseline, apply)
}
