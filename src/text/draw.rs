//! Text rasterisation: shadow + base text painting onto a [`Pixmap`].
//!
//! Public entry points are [`draw_text_line`], [`draw_text_line_scaled`],
//! [`draw_multiline_text`], and the lower-level [`draw_text_shadowed`] /
//! [`draw_text_shadowed_scaled`].
//!
//! # Parameter structs
//!
//! Several functions here used to carry 10+ arguments.  They now accept small
//! **parameter structs** that group logically related values, eliminating the
//! `#[allow(clippy::too_many_arguments)]` suppressions.

use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping};
use tiny_skia::{Color, Pixmap, PremultipliedColorU8};

use super::{
    engine::{TextEngine, with_text_engine},
    measure::{
        estimate_text_width, estimate_text_width_scaled, first_line_scale, max_lines_for_height,
        split_first_explicit_line, total_text_height, wrap_text,
    },
    util::font_weight_for_family,
    util::primary_family_name,
};

// ─────────────────────────────────────────────────────────────────────────────
// Public parameter structs
// ─────────────────────────────────────────────────────────────────────────────

/// Parameters for drawing a single styled line of text.
///
/// Used by [`draw_text_line`] and [`draw_text_line_scaled`].
pub struct DrawTextLine<'a> {
    /// Text to render.
    pub text: &'a str,
    /// Top-left origin X of the *layout* region (alignment is applied relative to this).
    pub x: f32,
    /// Baseline-top Y (the renderer adds a `font_size * 0.82` baseline offset).
    pub y: f32,
    pub font_size: f32,
    /// Available width for layout; text is not clipped, but alignment uses this.
    pub max_width: f32,
    pub color: Color,
    pub shadow_color: Color,
    pub brush: Option<TextBrush>,
    pub shadow_brush: Option<TextBrush>,
    pub family_name: &'a str,
    pub align: TextAlign,
    pub language: Option<&'a str>,
    pub letter_spacing: f32,
    /// Horizontal glyph-level scale (1.0 = no compression).
    pub scale_x: f32,
}

impl<'a> DrawTextLine<'a> {
    /// Construct with `scale_x = 1.0` (the common case).
    #[allow(clippy::too_many_arguments)]
    pub fn unscaled(
        text: &'a str,
        x: f32,
        y: f32,
        font_size: f32,
        max_width: f32,
        color: Color,
        shadow_color: Color,
        family_name: &'a str,
        align: TextAlign,
        language: Option<&'a str>,
        letter_spacing: f32,
    ) -> Self {
        Self {
            text,
            x,
            y,
            font_size,
            max_width,
            color,
            shadow_color,
            brush: None,
            shadow_brush: None,
            family_name,
            align,
            language,
            letter_spacing,
            scale_x: 1.0,
        }
    }

    pub fn with_brushes(
        mut self,
        brush: Option<TextBrush>,
        shadow_brush: Option<TextBrush>,
    ) -> Self {
        self.brush = brush;
        self.shadow_brush = shadow_brush;
        self
    }
}

/// Parameters for drawing multi-line text.
///
/// Used by [`draw_multiline_text`] and the internal first-line-compress variant.
pub struct DrawMultiline<'a> {
    pub text: &'a str,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub family_name: &'a str,
    pub color: Color,
    pub shadow_color: Color,
    pub brush: Option<TextBrush>,
    pub shadow_brush: Option<TextBrush>,
    pub language: Option<&'a str>,
    pub base_font_size: u32,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub min_font_size: u32,
    pub first_line_compress: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// TextAlign
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Fill used for text pixels.
#[derive(Debug, Clone)]
pub enum TextBrush {
    Solid(Color),
    LinearGradient {
        start: Color,
        end: Color,
        x0: f32,
        x1: f32,
    },
    VerticalMiddleGradient {
        top: Color,
        middle: Color,
        bottom: Color,
        y0: f32,
        y1: f32,
    },
}

impl TextBrush {
    pub fn solid(color: Color) -> Self {
        Self::Solid(color)
    }

    pub fn horizontal_gradient(start: Color, end: Color, x: f32, width: f32) -> Self {
        Self::LinearGradient {
            start,
            end,
            x0: x,
            x1: x + width.max(1.0),
        }
    }

    pub fn vertical_middle_gradient(
        top: Color,
        middle: Color,
        bottom: Color,
        y: f32,
        height: f32,
    ) -> Self {
        Self::VerticalMiddleGradient {
            top,
            middle,
            bottom,
            y0: y,
            y1: y + height.max(1.0),
        }
    }

    fn alpha(&self) -> f32 {
        match self {
            Self::Solid(color) => color.alpha(),
            Self::LinearGradient { start, end, .. } => start.alpha().max(end.alpha()),
            Self::VerticalMiddleGradient {
                top,
                middle,
                bottom,
                ..
            } => top.alpha().max(middle.alpha()).max(bottom.alpha()),
        }
    }

    fn sample(&self, x: f32, y: f32) -> Color {
        match self {
            Self::Solid(color) => *color,
            Self::LinearGradient { start, end, x0, x1 } => {
                let span = (*x1 - *x0).abs().max(1.0);
                let t = ((x - *x0) / span).clamp(0.0, 1.0);
                lerp_color(*start, *end, t)
            }
            Self::VerticalMiddleGradient {
                top,
                middle,
                bottom,
                y0,
                y1,
            } => {
                let span = (*y1 - *y0).abs().max(1.0);
                let t = ((y - *y0) / span).clamp(0.0, 1.0);
                if t <= 0.5 {
                    lerp_color(*top, *middle, t * 2.0)
                } else {
                    lerp_color(*middle, *bottom, (t - 0.5) * 2.0)
                }
            }
        }
    }
}

fn lerp_color(start: Color, end: Color, t: f32) -> Color {
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    Color::from_rgba(
        lerp(start.red(), end.red()),
        lerp(start.green(), end.green()),
        lerp(start.blue(), end.blue()),
        lerp(start.alpha(), end.alpha()),
    )
    .unwrap_or(start)
}

// ─────────────────────────────────────────────────────────────────────────────
// Public draw entry points
// ─────────────────────────────────────────────────────────────────────────────

/// Draw a single line of text (scale_x = 1.0).
pub fn draw_text_line(pixmap: &mut Pixmap, p: DrawTextLine<'_>) {
    draw_text_line_inner(pixmap, p);
}

/// Draw a single line of text with optional horizontal compression.
pub fn draw_text_line_scaled(pixmap: &mut Pixmap, p: DrawTextLine<'_>) {
    draw_text_line_inner(pixmap, p);
}

fn draw_text_line_inner(pixmap: &mut Pixmap, p: DrawTextLine<'_>) {
    if p.text.trim().is_empty() {
        return;
    }

    let unscaled_width = estimate_text_width(
        p.text,
        p.language,
        p.family_name,
        p.font_size,
        p.letter_spacing,
    );
    let estimated = estimate_text_width_scaled(
        p.text,
        p.language,
        p.family_name,
        p.font_size,
        p.letter_spacing,
        p.scale_x,
    )
    .min(p.max_width);

    let draw_x = match p.align {
        TextAlign::Left => p.x,
        TextAlign::Center => p.x - estimated / 2.0,
        TextAlign::Right => p.x - estimated,
    };
    let layout_width = if p.scale_x > 0.0 && p.scale_x != 1.0 {
        (p.max_width / p.scale_x).max(unscaled_width).ceil()
    } else {
        p.max_width.max(unscaled_width).ceil()
    };

    draw_text_shadowed_scaled(
        pixmap,
        ShadowedText {
            text: p.text,
            x: draw_x,
            y: p.y,
            font_size: p.font_size,
            width: layout_width,
            height: p.font_size * 1.4,
            base_color: p.color,
            shadow_color: p.shadow_color,
            base_brush: p.brush.clone(),
            shadow_brush: p.shadow_brush.clone(),
            family_name: p.family_name,
            letter_spacing: p.letter_spacing,
            scale_x: p.scale_x,
        },
    );
}

/// Draw a block of multi-line text, auto-shrinking font size to fit `height`.
pub fn draw_multiline_text(pixmap: &mut Pixmap, p: DrawMultiline<'_>) {
    let text = p.text.trim_end();
    if text.is_empty() {
        return;
    }

    if p.first_line_compress {
        if let Some((first_line, rest)) = split_first_explicit_line(text) {
            draw_multiline_with_first_line_compress(
                pixmap,
                &first_line,
                &rest,
                DrawMultiline {
                    text,
                    first_line_compress: false,
                    ..p
                },
            );
            return;
        }
    }

    // Binary-search for the largest font_size in [min_font_size, base_font_size]
    // whose wrapped text fits within `height`.
    let font_size = binary_search_font_size(
        text,
        p.language,
        p.family_name,
        p.width,
        p.letter_spacing,
        p.line_height,
        p.height,
        p.base_font_size,
        p.min_font_size,
        |fs| {
            wrap_text(
                text,
                p.language,
                p.family_name,
                p.width,
                fs,
                p.letter_spacing,
            )
            .len()
        },
    );

    let mut lines = wrap_text(
        text,
        p.language,
        p.family_name,
        p.width,
        font_size as f32,
        p.letter_spacing,
    );
    let max_lines = max_lines_for_height(p.height, font_size, p.line_height);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
    }

    for (index, line) in lines.iter().enumerate() {
        let line_y = if index == 0 {
            p.y
        } else {
            p.y + index as f32 * font_size as f32 * p.line_height
        };
        draw_text_shadowed(
            pixmap,
            ShadowedText {
                text: line,
                x: p.x,
                y: line_y,
                font_size: font_size as f32,
                width: p.width,
                height: font_size as f32 * 1.4,
                base_color: p.color,
                shadow_color: p.shadow_color,
                base_brush: p.brush.clone(),
                shadow_brush: p.shadow_brush.clone(),
                family_name: p.family_name,
                letter_spacing: p.letter_spacing,
                scale_x: 1.0,
            },
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shadow + base drawing
// ─────────────────────────────────────────────────────────────────────────────

/// Parameters for the shadowed text drawing primitives.
pub struct ShadowedText<'a> {
    pub text: &'a str,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub width: f32,
    pub height: f32,
    pub base_color: Color,
    pub shadow_color: Color,
    pub base_brush: Option<TextBrush>,
    pub shadow_brush: Option<TextBrush>,
    pub family_name: &'a str,
    pub letter_spacing: f32,
    pub scale_x: f32,
}

/// Draw text with a 1px shadow offset (scale_x = 1.0).
pub fn draw_text_shadowed(pixmap: &mut Pixmap, p: ShadowedText<'_>) {
    draw_text_shadowed_scaled(pixmap, p);
}

/// Draw text with a 1px shadow offset and optional horizontal scale.
pub fn draw_text_shadowed_scaled(pixmap: &mut Pixmap, p: ShadowedText<'_>) {
    if p.text.trim().is_empty() {
        return;
    }

    with_text_engine(|engine| {
        let TextEngine {
            font_system,
            swash_cache,
            ..
        } = engine;

        let metrics = Metrics::new(p.font_size, p.font_size);
        let mut buffer = Buffer::new(font_system, metrics);
        buffer.set_size(font_system, Some(p.width), Some(p.height));

        let resolved_family = primary_family_name(p.family_name);
        let attrs = Attrs::new()
            .family(Family::Name(resolved_family.as_str()))
            .weight(font_weight_for_family(resolved_family.as_str()));
        buffer.set_text(font_system, p.text, &attrs, Shaping::Advanced);
        buffer.shape_until_scroll(font_system, true);

        // SVG-style: `y` is the text-before-edge; add a baseline offset.
        let baseline_y = p.y + p.font_size * 0.82;

        // Skip the shadow pass entirely when the shadow colour is fully
        // transparent — the common case (~10-20% time saved).
        let shadow_brush = p
            .shadow_brush
            .as_ref()
            .cloned()
            .unwrap_or_else(|| TextBrush::solid(p.shadow_color));
        if shadow_brush.alpha() > 0.0 {
            draw_buffer_to_pixmap(
                font_system,
                swash_cache,
                &buffer,
                p.text,
                pixmap,
                p.x + 1.0,
                baseline_y + 1.0,
                &shadow_brush,
                p.letter_spacing,
                p.scale_x,
            );
        }
        let base_brush = p
            .base_brush
            .as_ref()
            .cloned()
            .unwrap_or_else(|| TextBrush::solid(p.base_color));
        draw_buffer_to_pixmap(
            font_system,
            swash_cache,
            &buffer,
            p.text,
            pixmap,
            p.x,
            baseline_y,
            &base_brush,
            p.letter_spacing,
            p.scale_x,
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Pixel-level blending
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_buffer_to_pixmap(
    font_system: &mut cosmic_text::FontSystem,
    swash_cache: &mut cosmic_text::SwashCache,
    buffer: &Buffer,
    text: &str,
    pixmap: &mut Pixmap,
    offset_x: f32,
    offset_y: f32,
    brush: &TextBrush,
    letter_spacing: f32,
    scale_x: f32,
) {
    let pm_width = pixmap.width() as i32;
    let pm_height = pixmap.height() as i32;

    // Build a byte-offset → char-index table for O(1) letter-spacing lookup.
    // Only constructed when letter_spacing is non-zero.
    let byte_to_char: Vec<usize> = if letter_spacing != 0.0 {
        let mut table = vec![0usize; text.len() + 1];
        for (char_idx, (byte_idx, _)) in text.char_indices().enumerate() {
            table[byte_idx] = char_idx;
        }
        let mut last = 0;
        for entry in table.iter_mut() {
            if *entry == 0 && last > 0 {
                *entry = last;
            } else {
                last = *entry;
            }
        }
        table
    } else {
        Vec::new()
    };

    let pixels = pixmap.pixels_mut();

    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let letter_spacing_offset = if letter_spacing == 0.0 || glyph.start == 0 {
                0.0
            } else {
                let char_idx = byte_to_char.get(glyph.start).copied().unwrap_or(0);
                letter_spacing * char_idx as f32
            };

            let physical_glyph = glyph.physical((offset_x + letter_spacing_offset, offset_y), 1.0);
            let image = match swash_cache.get_image(font_system, physical_glyph.cache_key) {
                Some(img) => img,
                None => continue,
            };

            let glyph_width = image.placement.width as usize;
            let glyph_height = image.placement.height as usize;
            if glyph_width == 0 || glyph_height == 0 || image.data.is_empty() {
                continue;
            }

            let base_x = offset_x;
            let glyph_left = physical_glyph.x + image.placement.left;
            let scaled_left = base_x + (glyph_left as f32 - base_x) * scale_x;
            let gx = scaled_left.round() as i32;
            let gy = physical_glyph.y - image.placement.top;

            let scaled_glyph_width = ((glyph_width as f32) * scale_x).ceil().max(1.0) as usize;

            let clip_x0 = gx.max(0) as usize;
            let clip_y0 = gy.max(0) as usize;
            let clip_x1 = ((gx + scaled_glyph_width as i32) as usize).min(pm_width as usize);
            let clip_y1 = ((gy + glyph_height as i32) as usize).min(pm_height as usize);

            if clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
                continue;
            }

            let local_y_start = clip_y0 as i32 - gy;
            let local_x_start = clip_x0 as i32 - gx;
            let inv_scale_x = if scale_x > 0.0 { 1.0 / scale_x } else { 1.0 };
            let max_src_cx = (glyph_width - 1) as f32;

            for cy in local_y_start as usize..(clip_y1 - clip_y0 as usize + local_y_start as usize)
            {
                if cy >= glyph_height {
                    break;
                }
                let row = &image.data[cy * glyph_width..(cy + 1) * glyph_width];
                let py = clip_y0 + cy - local_y_start as usize;
                let row_base = py * pm_width as usize;

                for dest_cx in local_x_start as usize..scaled_glyph_width.min(clip_x1 - gx as usize)
                {
                    let src_cx = ((dest_cx as f32) * inv_scale_x).floor().min(max_src_cx) as usize;
                    let alpha = row[src_cx];
                    if alpha == 0 {
                        continue;
                    }

                    let px = clip_x0 + dest_cx - local_x_start as usize;
                    let idx = row_base + px;

                    let color = brush.sample(px as f32, py as f32);
                    let src_r = (color.red() * 255.0) as u32;
                    let src_g = (color.green() * 255.0) as u32;
                    let src_b = (color.blue() * 255.0) as u32;
                    let color_alpha = color.alpha();
                    let sa = (alpha as f32 * color_alpha) as u32;
                    let inv_sa = 255 - sa;

                    let dst = pixels[idx];
                    let r = ((src_r * sa + dst.red() as u32 * inv_sa) / 255) as u8;
                    let g = ((src_g * sa + dst.green() as u32 * inv_sa) / 255) as u8;
                    let b = ((src_b * sa + dst.blue() as u32 * inv_sa) / 255) as u8;
                    let a = (sa + dst.alpha() as u32 * inv_sa / 255) as u8;

                    pixels[idx] = PremultipliedColorU8::from_rgba(r, g, b, a)
                        .unwrap_or(PremultipliedColorU8::TRANSPARENT);
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal: first-line-compress multi-line
// ─────────────────────────────────────────────────────────────────────────────

fn draw_multiline_with_first_line_compress(
    pixmap: &mut Pixmap,
    first_line: &str,
    rest: &str,
    p: DrawMultiline<'_>,
) {
    let first_line_height = p.base_font_size as f32;
    let remaining_height = (p.height - first_line_height * p.line_height).max(0.0);
    let first_line_scale_x = first_line_scale(
        first_line,
        p.language,
        p.family_name,
        p.base_font_size as f32,
        p.width,
        p.letter_spacing,
    );

    draw_text_line_inner(
        pixmap,
        DrawTextLine {
            text: first_line,
            x: p.x,
            y: p.y,
            font_size: p.base_font_size as f32,
            max_width: p.width,
            color: p.color,
            shadow_color: p.shadow_color,
            brush: p.brush.clone(),
            shadow_brush: p.shadow_brush.clone(),
            family_name: p.family_name,
            align: TextAlign::Left,
            language: p.language,
            letter_spacing: p.letter_spacing,
            scale_x: first_line_scale_x,
        },
    );

    if rest.trim().is_empty() || remaining_height <= 0.0 {
        return;
    }

    let rest_y = p.y + p.base_font_size as f32 * p.line_height;
    draw_multiline_text(
        pixmap,
        DrawMultiline {
            text: rest,
            y: rest_y,
            height: remaining_height,
            first_line_compress: false,
            ..p
        },
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal: binary-search for optimal font size
// ─────────────────────────────────────────────────────────────────────────────

/// Return the largest font size in `[min_font_size, base_font_size]` for which
/// `line_count_fn(font_size)` lines fit within `height`.
///
/// Uses binary search — O(log range) wraps instead of O(range).
#[allow(clippy::too_many_arguments)]
fn binary_search_font_size(
    text: &str,
    language: Option<&str>,
    family_name: &str,
    width: f32,
    letter_spacing: f32,
    line_height: f32,
    height: f32,
    base_font_size: u32,
    min_font_size: u32,
    line_count_fn: impl Fn(f32) -> usize,
) -> u32 {
    let fits = |fs: u32| total_text_height(line_count_fn(fs as f32), fs, line_height) <= height;

    // Fast path: base size already fits.
    if fits(base_font_size) {
        return base_font_size;
    }

    // Binary search: lo fits (or is the floor), hi does not fit.
    let mut lo = min_font_size;
    let mut hi = base_font_size;
    while lo + 1 < hi {
        let mid = (lo + hi) / 2;
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    // Suppress unused-variable warnings from the parameters not used in the
    // closure-less search path.
    let _ = (text, language, family_name, width, letter_spacing);
    lo
}
