//! Presentation-only adapter; the ordinary overlay callback is unchanged.
use super::*;
/// Let an observation player supply its own recorded wave display. The publisher
/// receives effective settings, including later hotkey changes. Install once
/// after overlay bootstrap; normal option updates keep this callback installed.
pub fn external_wave_display(mut publish: impl FnMut(Options) -> RuntimeResult<()> + 'static) -> RuntimeResult<()>
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
    let previous = STATE.with_borrow_mut(|s| s.paint.take());
    if let Some(task) = previous {
        with_scheduler(|s| {
            s.stop(task);
        });
    }
    let task = crate::paint::register(
        TickOptions::active_phase()
            .lifetime(TickLifetime::Session)
            .name("observation_overlays"),
        move |canvas| {
            let configured = SETTINGS.with_borrow(|s| s.effective_declaration());
            publish(configured.unwrap_or_default())?;
            let Some(mut options) = configured else {
                return Ok(());
            };
            options.total_hp = false;
            options.wavelength_records = 0;
            let world = crate::board::world_id()?;
            let scene = crate::access::with_backend(|b| b.scene()).map_err(crate::access::operation_error)?;
            STATE.with_borrow_mut(|s| {
                if s.history.world != world {
                    s.history = History::default();
                    s.history.world = world;
                }
                s.history.observe_scene(scene);
                canvas.set_origin(canvas.board_position()?.unwrap_or(rsvz_model::PixelPos { x: 0, y: 0 }));
                markers::draw(canvas, &s.history)?;
                info::draw(canvas, options, &mut s.history)?;
                indices::draw(canvas, options, &mut s.history)?;
                hover::draw(canvas, options, &mut s.history)
            })
        },
    )?;
    STATE.with_borrow_mut(|s| s.paint = Some(task));
    Ok(())
}
