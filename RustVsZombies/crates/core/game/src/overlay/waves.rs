use super::primitives::{Bar, Direction, number};
use super::{History, InfoMode, Options};
use crate::paint::{Argb, PaintCanvas, PaintFont};
use crate::runtime::RuntimeResult;
use rsvz_backend_api::{ClockBackend, CurrentWaveBackend, WaveHealthBackend, WaveTimingBackend};
use rsvz_current::CurrentBackend;

fn text(canvas: &mut dyn PaintCanvas, x: i32, value: &str, color: Argb) -> RuntimeResult<()> {
    number(canvas, x, 574, 24, value, PaintFont::new("", 17, false).unwrap(), color)
}
pub(super) fn draw(canvas: &mut dyn PaintCanvas, o: Options, history: &History) -> RuntimeResult<()>
where
    CurrentBackend: WaveTimingBackend + WaveHealthBackend,
{
    crate::access::with_backend(|b| -> RuntimeResult<()> {
        let wave = b.current_wave()?.0;
        let total = b.total_waves()?;
        if total == 0 {
            return Ok(());
        }
        let clock = b.clock()?;
        let countdown = b.refresh_countdown()?;
        let remaining = if wave == total {
            b.level_end_countdown()?
        } else if countdown > 200 {
            0
        } else if wave % 10 == 9 {
            if countdown <= 5 {
                b.huge_wave_countdown()?
            } else {
                countdown + 745
            }
        } else {
            countdown
        };
        let advanced = o.info == InfoMode::Advanced || wave % 10 == 9 || wave == total;
        if o.total_hp {
            let maximum = if wave == 0 { 1 } else { b.zombie_health_wave_start()? };
            let health = if wave == 0 {
                0
            } else {
                b.total_zombies_health_in_wave(wave - 1)?
            };
            let threshold = b.zombie_refresh_health_threshold()?;
            let marks = [
                if advanced {
                    f64::from(threshold)
                } else {
                    f64::from(maximum * 13 / 20)
                },
                f64::from(maximum / 2),
            ];
            Bar {
                x: 58,
                y: 574,
                denominator: f64::from(maximum),
                numerator: f64::from(health),
                separators: if wave == 0 {
                    &[]
                } else if advanced {
                    &marks[..1]
                } else {
                    &marks
                },
                direction: Direction::Right,
                length: 127,
                width: 24,
                color: if advanced && remaining != 0 {
                    o.colors.refresh_hp
                } else {
                    o.colors.total_hp
                },
                background: Argb(0xc0ffffff),
                frame: Argb(0xff000000),
                border: 1,
            }
            .draw(canvas)?;
            text(canvas, 59, &format!("{:02},", wave.max(1)), Argb(0xff0000ff))?;
            let elapsed = if wave == 0 {
                Some(-countdown)
            } else {
                history
                    .clocks
                    .get((wave - 1) as usize)
                    .copied()
                    .flatten()
                    .map(|start| clock - start)
            };
            let value = elapsed.map_or_else(|| "?".to_owned(), |time| time.to_string());
            text(canvas, 82, &value, Argb(0xff0000ff))?;
            if remaining != 0 && advanced {
                text(canvas, 145, &(-remaining).to_string(), Argb(0xffff0000))?;
            }
        }
        for i in 0..i32::from(o.wavelength_records) {
            let next = if remaining != 0 && advanced {
                wave - i
            } else {
                wave - 1 - i
            };
            if next <= 0 {
                continue;
            }
            let Some(end) = history.clocks.get(next as usize).copied().flatten() else {
                continue;
            };
            let Some(start) = history.clocks.get((next - 1) as usize).copied().flatten() else {
                continue;
            };
            Bar {
                x: 191 + 71 * i,
                y: 574,
                denominator: 1.0,
                numerator: 0.0,
                separators: &[],
                direction: Direction::Right,
                length: 65,
                width: 24,
                color: Argb(0xffffc000),
                background: Argb(0xc0ffffff),
                frame: Argb(0xff000000),
                border: 1,
            }
            .draw(canvas)?;
            text(canvas, 193 + 71 * i, &format!("{next:02},"), Argb(0xff0000ff))?;
            text(canvas, 216 + 71 * i, &(end - start).to_string(), Argb(0xff0000ff))?;
        }
        Ok(())
    })
}
