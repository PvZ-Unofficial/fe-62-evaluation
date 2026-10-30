//! A-TAS bar geometry, including bottom-right anchors and separator rounding.
use crate::paint::{Argb, PaintCanvas, PaintFont};
use crate::runtime::RuntimeResult;
use rsvz_model::{PixelPos, Rect};
#[derive(Clone, Copy)]
pub enum Direction {
    Right,
    Down,
    Left,
    Up,
}
pub struct Bar<'a> {
    pub x: i32,
    pub y: i32,
    pub denominator: f64,
    pub numerator: f64,
    pub separators: &'a [f64],
    pub direction: Direction,
    pub length: i32,
    pub width: i32,
    pub color: Argb,
    pub background: Argb,
    pub frame: Argb,
    pub border: i32,
}
impl Bar<'_> {
    pub fn draw(&self, canvas: &mut dyn PaintCanvas) -> RuntimeResult<()> {
        if !self.denominator.is_finite() || self.denominator <= 0.0 || !self.numerator.is_finite() {
            return Ok(());
        }
        let horizontal = matches!(self.direction, Direction::Right | Direction::Left);
        let reverse = matches!(self.direction, Direction::Left | Direction::Up);
        let (w, h) = if horizontal {
            (self.length, self.width)
        } else {
            (self.width, self.length)
        };
        let (x, y) = if reverse {
            (self.x - w, self.y - h)
        } else {
            (self.x, self.y)
        };
        rect(canvas, x, y, w, h, self.background)?;
        let n = (self.numerator / self.denominator * f64::from(self.length)).ceil() as i32;
        let (nx, ny, nw, nh) = match self.direction {
            Direction::Right => (x, y, n, h),
            Direction::Down => (x, y, w, n),
            Direction::Left => (self.x - n, y, n, h),
            Direction::Up => (x, self.y - n, w, n),
        };
        rect(canvas, nx, ny, nw, nh, self.color)?;
        let b = self.border;
        rect(canvas, x - b, y - b, w + b, b, self.frame)?;
        rect(canvas, x + w, y - b, b, h + b, self.frame)?;
        rect(canvas, x, y + h, w + b, b, self.frame)?;
        rect(canvas, x - b, y, b, h + b, self.frame)?;
        for separator in self.separators {
            let pos = (separator / self.denominator * f64::from(self.length)).ceil() as i32
                - i32::from(*separator != self.denominator);
            let (sx, sy, sw, sh) = match self.direction {
                Direction::Right => (self.x + pos, self.y, b, h),
                Direction::Down => (self.x, self.y + pos, w, b),
                Direction::Left => (self.x - pos - b, y, b, h),
                Direction::Up => (x, self.y - pos - b, w, b),
            };
            rect(canvas, sx, sy, sw, sh, self.frame)?;
        }
        Ok(())
    }
}
pub fn rect(canvas: &mut dyn PaintCanvas, x: i32, y: i32, width: i32, height: i32, color: Argb) -> RuntimeResult<()> {
    canvas.fill_rect(Rect { x, y, width, height }, color)
}
/// Center a fixed digit reference, keeping punctuation and changing values on one baseline.
pub(super) fn number(
    canvas: &mut dyn PaintCanvas, x: i32, top: i32, height: i32, value: &str, font: PaintFont<'_>, color: Argb,
) -> RuntimeResult<()> {
    let (ink_top, ink_bottom) = canvas.measure_digit_bounds(font)?;
    let y = top + (height - (ink_bottom - ink_top)).div_euclid(2) - ink_top;
    canvas.text(PixelPos { x, y }, value, font, color)
}
pub fn bar(
    canvas: &mut dyn PaintCanvas, x: i32, y: i32, total: i32, value: i32, separators: &[f64], direction: Direction,
    length: i32, width: i32, color: Argb,
) -> RuntimeResult<()> {
    Bar {
        x,
        y,
        denominator: f64::from(total),
        numerator: f64::from(value),
        separators,
        direction,
        length,
        width,
        color,
        background: Argb(0xa0ffffff),
        frame: Argb(0xff000000),
        border: 1,
    }
    .draw(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::PaintFont;
    use rsvz_model::PixelPos;
    #[derive(Default)]
    struct Recording(Vec<(Rect, Argb)>, Vec<(PixelPos, String)>, (i32, i32));
    impl PaintCanvas for Recording {
        fn board_position(&self) -> RuntimeResult<Option<PixelPos>> {
            Ok(None)
        }
        fn set_origin(&mut self, _: PixelPos) {}

        fn cursor(&mut self, _: PixelPos, _: bool) -> RuntimeResult<()> {
            unreachable!()
        }
        fn cursor_grid_row(&self) -> RuntimeResult<Option<i32>> {
            unreachable!()
        }
        fn mouse_position(&self) -> RuntimeResult<PixelPos> {
            unreachable!()
        }
        fn cursor_sprite_position(&self) -> RuntimeResult<Option<PixelPos>> {
            unreachable!()
        }

        fn size(&self) -> (u32, u32) {
            (800, 600)
        }
        fn fill_rect(&mut self, rect: Rect, color: Argb) -> RuntimeResult<()> {
            self.0.push((rect, color));
            Ok(())
        }
        fn text(&mut self, position: PixelPos, value: &str, _: PaintFont<'_>, _: Argb) -> RuntimeResult<()> {
            self.1.push((position, value.to_owned()));
            Ok(())
        }
        fn measure_text(&mut self, _: &str, _: PaintFont<'_>) -> RuntimeResult<(u32, u32)> {
            unreachable!()
        }
        fn measure_digit_bounds(&mut self, _: PaintFont<'_>) -> RuntimeResult<(i32, i32)> {
            Ok(self.2)
        }
        fn set_clip(&mut self, _: Option<Rect>) -> RuntimeResult<()> {
            unreachable!()
        }
    }
    #[test]
    fn numeric_labels_share_a_stable_center_with_segment_digits() {
        let font = PaintFont::new("Arial", 12, false).unwrap();
        for bounds in [(4, 13), (14, 23), (5, 15)] {
            let mut canvas = Recording(Vec::new(), Vec::new(), bounds);
            for (top, height) in [(25, 11), (8, 26), (574, 24)] {
                for value in ["1", "888", "-200", "02,"] {
                    number(&mut canvas, 50, top, height, value, font, Argb(0xff000000)).unwrap();
                }
                let row = &canvas.1[canvas.1.len() - 4..];
                assert!(row.iter().all(|(position, _)| *position == row[0].0));
                let ink_top = row[0].0.y + bounds.0;
                let ink_bottom = row[0].0.y + bounds.1;
                assert!(ink_top >= top && ink_bottom <= top + height);
                assert!(((ink_top - top) - (top + height - ink_bottom)).abs() <= 1);
            }
        }
        let mut canvas = Recording::default();
        seven_segment(&mut canvas, 888, 50, 27, Argb(0xff0040ff), Argb(0), false).unwrap();
        let top = canvas.0.iter().map(|(r, _)| r.y).min().unwrap();
        let bottom = canvas.0.iter().map(|(r, _)| r.y + r.height).max().unwrap();
        assert_eq!((top - 25, 36 - bottom), (2, 2));
    }
    #[test]
    fn reverse_bar_anchor_alpha_and_separators_match_native_reference() {
        let mut canvas = Recording::default();
        bar(
            &mut canvas,
            100,
            120,
            3000,
            1501,
            &[1500.0, 1800.0],
            Direction::Up,
            40,
            8,
            Argb(0xff9868bc),
        )
        .unwrap();
        assert_eq!(
            canvas.0[0],
            (
                Rect {
                    x: 92,
                    y: 80,
                    width: 8,
                    height: 40
                },
                Argb(0xa0ffffff)
            )
        );
        assert_eq!(
            canvas.0[1].0,
            Rect {
                x: 92,
                y: 99,
                width: 8,
                height: 21
            }
        );
        assert_eq!(
            canvas.0[6].0,
            Rect {
                x: 92,
                y: 100,
                width: 8,
                height: 1
            }
        );
        assert_eq!(
            canvas.0[7].0,
            Rect {
                x: 92,
                y: 96,
                width: 8,
                height: 1
            }
        );
        for entry in &canvas.0[2..] {
            assert_eq!(entry.1, Argb(0xff000000));
        }
    }
}

// Exact A-TAS Draw.h digit stroke rectangles; intentionally differs from a
// generic seven-segment font (especially the narrow 1 and joined 0 strokes).
pub(super) fn seven_segment(
    canvas: &mut dyn PaintCanvas, value: u32, x: i32, y: i32, color: Argb, background: Argb, show_background: bool,
) -> RuntimeResult<()> {
    let (l, w, space, edge) = (2, 1, 2, 2);
    let text = value.to_string();
    if show_background {
        rect(
            canvas,
            x - edge,
            y - edge,
            2 * edge + text.len() as i32 * (2 * w + l + space) - space,
            2 * edge + 3 * w + 2 * l,
            background,
        )?;
    }
    for (i, digit) in text.bytes().enumerate() {
        let offset = i as i32 * (2 * w + l + space);
        match digit {
            b'-' => {
                rect(canvas, offset + x, y + w + l, 2 * w + l, w, color)?;
            }
            b'0' => {
                rect(canvas, offset + x + w, y, l, w, color)?;
                rect(canvas, offset + x + w, y + 2 * w + 2 * l, l, w, color)?;
                rect(canvas, offset + x, y, w, 3 * w + 2 * l, color)?;
                rect(canvas, offset + x + w + l, y, w, 3 * w + 2 * l, color)?;
            }
            b'1' => {
                rect(canvas, offset + x + l / 2, y + w, w, w, color)?;
                rect(canvas, offset + x + w + l / 2, y, w, 3 * w + 2 * l, color)?;
            }
            b'2' => {
                rect(canvas, offset + x, y, w + l, w, color)?;
                rect(canvas, offset + x + w, y + w + l, l, w, color)?;
                rect(canvas, offset + x + w, y + 2 * w + 2 * l, l + w, w, color)?;
                rect(canvas, offset + x, y + w + l, w, 2 * w + l, color)?;
                rect(canvas, offset + x + w + l, y, w, 2 * w + l, color)?;
            }
            b'3' => {
                rect(canvas, offset + x, y, w + l, w, color)?;
                rect(canvas, offset + x, y + w + l, w + l, w, color)?;
                rect(canvas, offset + x, y + 2 * w + 2 * l, w + l, w, color)?;
                rect(canvas, offset + x + w + l, y, w, 3 * w + 2 * l, color)?;
            }
            b'4' => {
                rect(canvas, offset + x + w, y + w + l, l, w, color)?;
                rect(canvas, offset + x, y, w, 2 * w + l, color)?;
                rect(canvas, offset + x + w + l, y, w, 3 * w + 2 * l, color)?;
            }
            b'5' => {
                rect(canvas, offset + x + w, y, l + w, w, color)?;
                rect(canvas, offset + x + w, y + w + l, l, w, color)?;
                rect(canvas, offset + x, y + 2 * w + 2 * l, w + l, w, color)?;
                rect(canvas, offset + x, y, w, 2 * w + l, color)?;
                rect(canvas, offset + x + w + l, y + w + l, w, 2 * w + l, color)?;
            }
            b'6' => {
                rect(canvas, offset + x + w, y, l + w, w, color)?;
                rect(canvas, offset + x + w, y + w + l, l, w, color)?;
                rect(canvas, offset + x + w, y + 2 * w + 2 * l, l, w, color)?;
                rect(canvas, offset + x, y, w, 3 * w + 2 * l, color)?;
                rect(canvas, offset + x + w + l, y + w + l, w, 2 * w + l, color)?;
            }
            b'7' => {
                rect(canvas, offset + x, y, w + l, w, color)?;
                rect(canvas, offset + x + w + l, y, w, 3 * w + 2 * l, color)?;
            }
            b'8' => {
                rect(canvas, offset + x + w, y, l, w, color)?;
                rect(canvas, offset + x + w, y + w + l, l, w, color)?;
                rect(canvas, offset + x + w, y + 2 * w + 2 * l, l, w, color)?;
                rect(canvas, offset + x, y, w, 3 * w + 2 * l, color)?;
                rect(canvas, offset + x + w + l, y, w, 3 * w + 2 * l, color)?;
            }
            b'9' => {
                rect(canvas, offset + x + w, y, l, w, color)?;
                rect(canvas, offset + x + w, y + w + l, l, w, color)?;
                rect(canvas, offset + x, y + 2 * w + 2 * l, w + l, w, color)?;
                rect(canvas, offset + x, y, w, 2 * w + l, color)?;
                rect(canvas, offset + x + w + l, y, w, 3 * w + 2 * l, color)?;
            }
            _ => {}
        }
    }
    Ok(())
}
