//! Font resources are owned by the shared paint backend, never by module callbacks.
use super::*;
use font_kit::{
    canvas::{Canvas as RasterCanvas, Format, RasterizationOptions},
    family_name::FamilyName,
    font::Font,
    hinting::HintingOptions,
    properties::{Properties, Weight},
    source::SystemSource,
};
use pathfinder_geometry::transform2d::Transform2F;
use std::{cell::RefCell, collections::HashMap};

struct Face {
    family: String,
    pixels: u16,
    bold: bool,
    font: Font,
    glyphs: HashMap<char, (u32, i32, i32)>,
    digit_bounds: Option<(i32, i32)>,
}
thread_local! { static FONTS: RefCell<Vec<Face>> = const { RefCell::new(Vec::new()) }; }
pub(super) fn clear() {
    FONTS.with_borrow_mut(Vec::clear);
}

fn error(value: impl std::fmt::Debug) -> RuntimeError {
    RuntimeError::new(format!("native font rasterization: {value:?}"))
}

#[cfg(target_os = "android")]
pub(crate) fn button_label(text: &str) -> RuntimeResult<(Vec<u32>, i32, i32)> {
    use pathfinder_geometry::vector::Vector2F;
    let font = SystemSource::new()
        .select_best_match(
            &[FamilyName::Title("Noto Sans CJK SC".into()), FamilyName::SansSerif],
            &Properties::new(),
        )
        .map_err(error)?
        .load()
        .map_err(error)?;
    let width = text.chars().count() as i32 * 20;
    let height = 28;
    let mut pixels = vec![0; (width * height) as usize];
    for (column, character) in text.chars().enumerate() {
        let glyph = font
            .glyph_for_char(character)
            .or_else(|| font.glyph_for_char('?'))
            .ok_or_else(|| error("missing button glyph"))?;
        let bounds = font
            .raster_bounds(
                glyph,
                20.0,
                Transform2F::default(),
                HintingOptions::None,
                RasterizationOptions::GrayscaleAa,
            )
            .map_err(error)?;
        if bounds.width() <= 0 || bounds.height() <= 0 {
            continue;
        }
        let mut raster = RasterCanvas::new(bounds.size(), Format::A8);
        font.rasterize_glyph(
            &mut raster,
            glyph,
            20.0,
            Transform2F::from_translation(Vector2F::new(-bounds.origin().x() as f32, -bounds.origin().y() as f32)),
            HintingOptions::None,
            RasterizationOptions::GrayscaleAa,
        )
        .map_err(error)?;
        for y in 0..bounds.height() {
            for x in 0..bounds.width() {
                let px = column as i32 * 20 + x + bounds.origin().x();
                let py = 23 + y + bounds.origin().y();
                if (0..width).contains(&px) && (0..height).contains(&py) {
                    pixels[(py * width + px) as usize] =
                        u32::from(raster.pixels[y as usize * raster.stride + x as usize]) << 24 | 0x0055e535;
                }
            }
        }
    }
    Ok((pixels, width, height))
}

fn font_face<'a>(fonts: &'a mut Vec<Face>, requested: PaintFont<'_>) -> RuntimeResult<&'a mut Face> {
    let index = if let Some(index) = fonts.iter().position(|font| {
        font.family == requested.family() && font.pixels == requested.pixels() && font.bold == requested.bold()
    }) {
        index
    } else {
        let properties = Properties::new()
            .weight(if requested.bold() { Weight::BOLD } else { Weight::NORMAL })
            .to_owned();
        let handle = SystemSource::new()
            .select_best_match(
                &[
                    FamilyName::Title(requested.family().into()),
                    FamilyName::Title("Noto Sans CJK SC".into()),
                    FamilyName::Title("PingFang SC".into()),
                    FamilyName::SansSerif,
                ],
                &properties,
            )
            .map_err(error)?;
        fonts.push(Face {
            family: requested.family().into(),
            pixels: requested.pixels(),
            bold: requested.bold(),
            font: handle.load().map_err(error)?,
            glyphs: HashMap::new(),
            digit_bounds: None,
        });
        fonts.len() - 1
    };
    Ok(&mut fonts[index])
}

pub(super) fn measure_digit_bounds(requested: PaintFont<'_>) -> RuntimeResult<(i32, i32)> {
    FONTS.with_borrow_mut(|fonts| {
        let face = font_face(fonts, requested)?;
        if let Some(bounds) = face.digit_bounds {
            return Ok(bounds);
        }
        let metrics = face.font.metrics();
        let baseline = (metrics.ascent * f32::from(face.pixels) / metrics.units_per_em as f32).round() as i32;
        let (mut top, mut bottom) = (i32::MAX, i32::MIN);
        for character in '0'..='9' {
            let glyph = face
                .font
                .glyph_for_char(character)
                .ok_or_else(|| error("missing digit glyph"))?;
            let bounds = face
                .font
                .raster_bounds(
                    glyph,
                    f32::from(face.pixels),
                    Transform2F::default(),
                    HintingOptions::None,
                    RasterizationOptions::GrayscaleAa,
                )
                .map_err(error)?;
            top = top.min(baseline + bounds.origin().y());
            bottom = bottom.max(baseline + bounds.origin().y() + bounds.height());
        }
        face.digit_bounds = Some((top, bottom));
        Ok((top, bottom))
    })
}

pub(super) fn draw_text(
    position: PixelPos, text: &str, requested: PaintFont<'_>, color: Argb, measure: bool,
) -> RuntimeResult<(u32, u32)> {
    FONTS.with_borrow_mut(|fonts| {
        let face = font_face(fonts, requested)?;
        let pixels = i32::from(face.pixels);
        let (mut x, mut y, mut longest) = (0i32, 0i32, 0i32);
        for character in text.chars() {
            if character == '\r' {
                continue;
            }
            if character == '\n' {
                longest = longest.max(x);
                x = 0;
                y = y.checked_add(pixels).ok_or_else(|| error("text height overflow"))?;
                continue;
            }
            if !measure && character != ' ' {
                if !face.glyphs.contains_key(&character) {
                    let glyph = face
                        .font
                        .glyph_for_char(character)
                        .or_else(|| face.font.glyph_for_char('?'))
                        .ok_or_else(|| error("missing glyph"))?;
                    let bounds = face
                        .font
                        .raster_bounds(
                            glyph,
                            pixels as f32,
                            Transform2F::default(),
                            HintingOptions::None,
                            RasterizationOptions::GrayscaleAa,
                        )
                        .map_err(error)?;
                    let id = if bounds.width() <= 0 || bounds.height() <= 0 {
                        0
                    } else {
                        let mut raster = RasterCanvas::new(bounds.size(), Format::A8);
                        face.font
                            .rasterize_glyph(
                                &mut raster,
                                glyph,
                                pixels as f32,
                                Transform2F::from_translation(-bounds.origin().to_f32()),
                                HintingOptions::None,
                                RasterizationOptions::GrayscaleAa,
                            )
                            .map_err(error)?;
                        let mut rgba = Vec::with_capacity((bounds.width() * bounds.height()) as usize);
                        for row in raster.pixels.chunks(raster.stride).take(bounds.height() as usize) {
                            rgba.extend(
                                row[..bounds.width() as usize]
                                    .iter()
                                    .map(|alpha| u32::from(*alpha) << 24 | 0x00ff_ffff),
                            );
                        }
                        // SAFETY: native copies the bounded pixels into its owning MemoryImage.
                        let id =
                            unsafe { raw::pvzp_rs_paint_create_image(rgba.as_ptr(), bounds.width(), bounds.height()) };
                        if id == 0 {
                            return Err(error("native image creation failed"));
                        }
                        id
                    };
                    let metrics = face.font.metrics();
                    let baseline = (metrics.ascent * pixels as f32 / metrics.units_per_em as f32).round() as i32;
                    face.glyphs
                        .insert(character, (id, bounds.origin().x(), baseline + bounds.origin().y()));
                }
                let (id, dx, dy) = face.glyphs[&character];
                if id != 0 {
                    let px = position
                        .x
                        .checked_add(x)
                        .and_then(|x| x.checked_add(dx))
                        .ok_or_else(|| error("text x overflow"))?;
                    let py = position
                        .y
                        .checked_add(y)
                        .and_then(|y| y.checked_add(dy))
                        .ok_or_else(|| error("text y overflow"))?;
                    checked(unsafe { raw::pvzp_rs_paint_image(id, px, py, color.0) })?;
                }
            }
            x = x
                .checked_add(if character <= '\u{ff}' { pixels / 2 + 1 } else { pixels })
                .ok_or_else(|| error("text width overflow"))?;
        }
        longest = longest.max(x);
        let height = if text.is_empty() {
            0
        } else if text.ends_with('\n') {
            y
        } else {
            y.checked_add(pixels).ok_or_else(|| error("text height overflow"))?
        };
        Ok((longest as u32, height as u32))
    })
}
