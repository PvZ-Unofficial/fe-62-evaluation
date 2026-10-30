//! Static screenshot identity; no per-frame formatting or changes to strategy.
use super::*;
use rsvz::core::model::{PixelPos, Rect};
use rsvz::paint::{Argb, PaintFont};
use rsvz::tick::TickLifetime;

pub fn install() -> RuntimeResult<()> {
    let run = std::env::var("SIX_LIVE_RUN").map_err(err)?;
    let version = std::env::var("SIX_LIVE_VERSION").map_err(err)?;
    let label = format!("62 | Run {run} | {version}");
    rsvz::log(
        rsvz::LogLevel::Info,
        format!("six_two_live_identity run={run} version={version}"),
    );
    let font = PaintFont::new("Consolas", 14, false).map_err(err)?;
    rsvz::paint::on_paint(
        TickOptions::active_phase().lifetime(TickLifetime::Session),
        move |canvas| {
            canvas.fill_rect(
                Rect {
                    x: 4,
                    y: 576,
                    width: 580,
                    height: 22,
                },
                Argb(0xC0000000),
            )?;
            canvas.text(PixelPos { x: 8, y: 579 }, &label, font, Argb(0xFFFFFFFF))?;
            Ok(())
        },
    )?;
    Ok(())
}
