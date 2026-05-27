//! Ruby (furigana) annotation rendering.
//!
//! This module handles the layout and drawing of ruby-annotated text: parsing
//! tokens from the markup, measuring each slot, wrapping to fit a width, and
//! rasterising base text + annotation text together.
//!
//! The public API mirrors the pre-split `text.rs` signatures, but without the
//! `#[allow(clippy::too_many_arguments)]` suppressions — argument groups are
//! now encoded in the [`RubyLineParams`] and [`RubyMultilineParams`] structs.

use tiny_skia::{Color, Pixmap};

use crate::model::FontWeight;
use crate::ruby::{
    RT_COMPRESS_RATE, RT_STRETCH_RATE, RUBY_PADDING_MAX, RubyToken, contains_ruby_markup,
    parse_ruby_text,
};

use super::{
    draw::{
        DrawMultiline, ShadowedText, TextAlign, TextBrush, draw_multiline_text,
        draw_text_shadowed_scaled,
    },
    measure::{estimate_text_width, max_lines_for_height, total_text_height},
};

// ─────────────────────────────────────────────────────────────────────────────
// Public parameter structs
// ─────────────────────────────────────────────────────────────────────────────

/// Parameters for drawing a single ruby-annotated line.
pub struct RubyLineParams<'a> {
    pub tokens: &'a [RubyToken],
    pub x: f32,
    /// Visual top of the *base* text line (same convention as [`ShadowedText::y`]).
    pub y: f32,
    pub font_size: f32,
    pub rt_font_size: f32,
    /// Signed vertical offset from `y` to the top of the ruby annotation
    /// (typically negative — i.e. above the base line).
    pub rt_top: f32,
    pub rt_font_scale_x_override: f32,
    pub color: Color,
    pub shadow_color: Color,
    pub brush: Option<TextBrush>,
    pub shadow_brush: Option<TextBrush>,
    pub family: &'a str,
    pub language: Option<&'a str>,
    pub letter_spacing: f32,
    /// Overall horizontal compression factor for the whole line.
    pub scale_x: f32,
    /// Extra gap between slots for justify alignment (0 = disabled).
    pub justify_gap: f32,
    pub font_weight: Option<FontWeight>,
}

/// Parameters for drawing multi-line ruby-annotated text.
pub struct RubyMultilineParams<'a> {
    pub text: &'a str,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub family: &'a str,
    pub color: Color,
    pub shadow_color: Color,
    pub brush: Option<TextBrush>,
    pub shadow_brush: Option<TextBrush>,
    pub language: Option<&'a str>,
    pub base_font_size: u32,
    pub rt_font_size: u32,
    pub rt_top: f32,
    pub rt_font_scale_x: f32,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub min_font_size: u32,
    pub first_line_compress: bool,
    pub align: TextAlign,
    pub font_weight: Option<FontWeight>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Slot geometry
// ─────────────────────────────────────────────────────────────────────────────

struct RubySlot {
    token: RubyToken,
    /// Width of the base text (unscaled, at line font_size).
    base_width: f32,
    /// Natural width of the rt text (unscaled, at rt_font_size, no letter-spacing).
    rt_natural_width: f32,
    padding_left: f32,
    padding_right: f32,
    /// Glyph-level horizontal scale for the ruby annotation.
    rt_scale_x: f32,
    rt_letter_spacing: f32,
}

impl RubySlot {
    fn slot_width(&self) -> f32 {
        self.padding_left + self.base_width + self.padding_right
    }
}

/// Overall ruby-fit strategy: override → stretch → compress → fit.
/// Decide how to fit the rt text over its base text.
///
/// Returns `(padding_left, padding_right, rt_scale_x, rt_letter_spacing)`.
fn compute_rt_strategy(
    base_width: f32,
    rt_natural_width: f32,
    rt_font_scale_x_override: f32,
    font_size: f32,
    rt_font_size: f32,
    rt_char_count: usize,
) -> (f32, f32, f32, f32) {
    // ① Bundle-supplied override — use directly.
    if (rt_font_scale_x_override - 1.0).abs() > f32::EPSILON {
        return (0.0, 0.0, rt_font_scale_x_override, 0.0);
    }

    if rt_natural_width <= 0.0 || base_width <= 0.0 {
        return (0.0, 0.0, 1.0, 0.0);
    }

    let ratio = rt_natural_width / base_width;

    if ratio < RT_STRETCH_RATE && rt_char_count > 1 {
        // ② Stretch: distribute extra space as inter-character letter-spacing.
        let max_ls = font_size - rt_font_size / 2.0;
        let needed = (base_width - rt_natural_width) / (rt_char_count.saturating_sub(1) as f32);
        let ls = needed.min(max_ls).max(0.0);
        (0.0, 0.0, 1.0, ls)
    } else if rt_natural_width > base_width {
        // ③ Compress.
        let compress_ratio = base_width / rt_natural_width;
        if compress_ratio < RT_COMPRESS_RATE {
            let padded_width = rt_natural_width * RT_COMPRESS_RATE;
            let total_pad = padded_width - base_width;
            let pad = (total_pad / 2.0).min(RUBY_PADDING_MAX);
            (pad, pad, RT_COMPRESS_RATE, 0.0)
        } else {
            (0.0, 0.0, compress_ratio, 0.0)
        }
    } else {
        // ④ rt fits — leave as-is.
        (0.0, 0.0, 1.0, 0.0)
    }
}

fn measure_ruby_slots(
    tokens: &[RubyToken],
    family: &str,
    font_size: f32,
    rt_font_size: f32,
    letter_spacing: f32,
    rt_font_scale_x_override: f32,
) -> Vec<RubySlot> {
    tokens
        .iter()
        .map(|token| {
            let base_text = token.base_text();
            let base_width =
                estimate_text_width(base_text, None, family, font_size, letter_spacing);

            if let RubyToken::Ruby { rt, .. } = token {
                let rt_w = estimate_text_width(rt, None, family, rt_font_size, 0.0);
                let rt_chars = rt.chars().count();
                let (pl, pr, rs, rls) = compute_rt_strategy(
                    base_width,
                    rt_w,
                    rt_font_scale_x_override,
                    font_size,
                    rt_font_size,
                    rt_chars,
                );
                RubySlot {
                    token: token.clone(),
                    base_width,
                    rt_natural_width: rt_w,
                    padding_left: pl,
                    padding_right: pr,
                    rt_scale_x: rs,
                    rt_letter_spacing: rls,
                }
            } else {
                RubySlot {
                    token: token.clone(),
                    base_width,
                    rt_natural_width: 0.0,
                    padding_left: 0.0,
                    padding_right: 0.0,
                    rt_scale_x: 1.0,
                    rt_letter_spacing: 0.0,
                }
            }
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the `scale_x` required to fit all tokens within `max_width`.
pub fn fit_ruby_text_scale(
    tokens: &[RubyToken],
    family: &str,
    font_size: f32,
    rt_font_size: f32,
    letter_spacing: f32,
    rt_font_scale_x_override: f32,
    max_width: f32,
) -> f32 {
    let slots = measure_ruby_slots(
        tokens,
        family,
        font_size,
        rt_font_size,
        letter_spacing,
        rt_font_scale_x_override,
    );
    let total: f32 = slots.iter().map(|s| s.slot_width()).sum();
    if total <= 0.0 {
        return 1.0;
    }
    (max_width / total).min(1.0)
}

/// Draw one line of ruby-annotated text.
pub fn draw_ruby_text_line(pixmap: &mut Pixmap, p: RubyLineParams<'_>) {
    if p.tokens.is_empty() {
        return;
    }

    let slots = measure_ruby_slots(
        p.tokens,
        p.family,
        p.font_size,
        p.rt_font_size,
        p.letter_spacing,
        p.rt_font_scale_x_override,
    );

    let rt_y = p.y + p.rt_top;
    let mut cursor_x = 0.0_f32;

    for slot in &slots {
        // ── Base text ─────────────────────────────────────────────────────
        let base_text = slot.token.base_text();
        if !base_text.trim().is_empty() {
            let base_draw_x = p.x + (cursor_x + slot.padding_left) * p.scale_x;
            let layout_w = slot.base_width / p.scale_x.max(0.01) + 200.0;
            draw_text_shadowed_scaled(
                pixmap,
                ShadowedText {
                    text: base_text,
                    x: base_draw_x,
                    y: p.y,
                    font_size: p.font_size,
                    width: layout_w,
                    height: p.font_size * 1.4,
                    base_color: p.color,
                    shadow_color: p.shadow_color,
                    base_brush: p.brush.clone(),
                    shadow_brush: p.shadow_brush.clone(),
                    family_name: p.family,
                    letter_spacing: p.letter_spacing,
                    scale_x: p.scale_x,
                    font_weight: p.font_weight,
                },
            );
        }

        // ── Ruby annotation ───────────────────────────────────────────────
        if let RubyToken::Ruby { rt, .. } = &slot.token {
            if !rt.is_empty() && p.rt_font_size > 0.0 {
                let combined_scale = slot.rt_scale_x * p.scale_x;

                let base_draw_x = p.x + (cursor_x + slot.padding_left) * p.scale_x;
                let base_screen_w = slot.base_width * p.scale_x;
                let base_center_screen = base_draw_x + base_screen_w / 2.0;

                let eff_rt_w = if slot.rt_letter_spacing > 0.0 {
                    (slot.rt_natural_width
                        + slot.rt_letter_spacing * (rt.chars().count().saturating_sub(1)) as f32)
                        * combined_scale
                } else {
                    slot.rt_natural_width * combined_scale
                };
                let rt_draw_x = base_center_screen - eff_rt_w / 2.0;

                let rt_layout_w = slot.rt_natural_width / slot.rt_scale_x.max(0.01) + 200.0;
                draw_text_shadowed_scaled(
                    pixmap,
                    ShadowedText {
                        text: rt,
                        x: rt_draw_x,
                        y: rt_y,
                        font_size: p.rt_font_size,
                        width: rt_layout_w,
                        height: p.rt_font_size * 1.4,
                        base_color: p.color,
                        shadow_color: p.shadow_color,
                        base_brush: p.brush.clone(),
                        shadow_brush: p.shadow_brush.clone(),
                        family_name: p.family,
                        letter_spacing: slot.rt_letter_spacing,
                        scale_x: combined_scale,
                        font_weight: p.font_weight,
                    },
                );
            }
        }

        cursor_x += slot.slot_width() + p.justify_gap;
    }
}

/// Wrap ruby-annotated text into lines that fit within `max_width`.
///
/// Ruby tokens are kept atomic; plain tokens are split character-by-character
/// (appropriate for CJK text, the primary ruby use-case here).
pub fn wrap_ruby_tokens(
    text: &str,
    family_name: &str,
    font_size: f32,
    letter_spacing: f32,
    rt_font_size: f32,
    rt_font_scale_x_override: f32,
    max_width: f32,
) -> Vec<Vec<RubyToken>> {
    let all_tokens = parse_ruby_text(text);
    let mut result: Vec<Vec<RubyToken>> = Vec::new();
    let mut current_line: Vec<RubyToken> = Vec::new();
    let mut current_width = 0.0_f32;

    for token in all_tokens {
        match token {
            RubyToken::Newline => {
                result.push(std::mem::take(&mut current_line));
                current_width = 0.0;
            }
            RubyToken::Ruby { .. } => {
                let slot_w = {
                    let slots = measure_ruby_slots(
                        &[token.clone()],
                        family_name,
                        font_size,
                        rt_font_size,
                        letter_spacing,
                        rt_font_scale_x_override,
                    );
                    slots[0].slot_width()
                };
                if !current_line.is_empty() && current_width + slot_w > max_width {
                    result.push(std::mem::take(&mut current_line));
                    current_width = 0.0;
                }
                current_line.push(token);
                current_width += slot_w;
            }
            RubyToken::Plain(ref s) => {
                for ch in s.chars() {
                    let mut ch_buf = [0u8; 4];
                    let ch_str = ch.encode_utf8(&mut ch_buf);
                    let ch_w =
                        estimate_text_width(&ch_str, None, family_name, font_size, letter_spacing);
                    if !current_line.is_empty() && current_width + ch_w > max_width {
                        result.push(std::mem::take(&mut current_line));
                        current_width = 0.0;
                    }
                    match current_line.last_mut() {
                        Some(RubyToken::Plain(last)) => last.push(ch),
                        _ => current_line.push(RubyToken::Plain(ch_str.to_string())),
                    }
                    current_width += ch_w;
                }
            }
        }
    }

    if !current_line.is_empty() {
        result.push(current_line);
    }
    result
}

/// Render multi-line ruby-annotated text.
///
/// Falls back to plain [`draw_multiline_text`] when the text contains no
/// ruby markup or when `rt_font_size == 0`.
pub fn draw_multiline_ruby_text(pixmap: &mut Pixmap, p: RubyMultilineParams<'_>) {
    let text = p.text.trim_end();
    if text.is_empty() {
        return;
    }

    if p.rt_font_size == 0 || !contains_ruby_markup(text) {
        draw_multiline_text(
            pixmap,
            DrawMultiline {
                text,
                x: p.x,
                y: p.y,
                width: p.width,
                height: p.height,
                family_name: p.family,
                color: p.color,
                shadow_color: p.shadow_color,
                brush: p.brush.clone(),
                shadow_brush: p.shadow_brush.clone(),
                language: p.language,
                base_font_size: p.base_font_size,
                line_height: p.line_height,
                letter_spacing: p.letter_spacing,
                min_font_size: p.min_font_size,
                first_line_compress: p.first_line_compress,
                align: p.align,
                font_weight: p.font_weight,
            },
        );
        return;
    }

    if p.first_line_compress {
        if let Some((first_line, rest)) = split_first_explicit_ruby_line(text) {
            draw_multiline_ruby_with_first_line_compress(pixmap, first_line, rest, p);
            return;
        }
    }

    // Binary-search for the largest font size that fits.
    let font_size = {
        let fits = |fs: u32| {
            let lines = wrap_ruby_tokens(
                text,
                p.family,
                fs as f32,
                p.letter_spacing,
                p.rt_font_size as f32,
                p.rt_font_scale_x,
                p.width,
            );
            total_text_height(lines.len(), fs, p.line_height) <= p.height
        };

        if fits(p.base_font_size) {
            p.base_font_size
        } else {
            let mut lo = p.min_font_size;
            let mut hi = p.base_font_size;
            while lo + 1 < hi {
                let mid = (lo + hi) / 2;
                if fits(mid) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            lo
        }
    };

    let mut lines = wrap_ruby_tokens(
        text,
        p.family,
        font_size as f32,
        p.letter_spacing,
        p.rt_font_size as f32,
        p.rt_font_scale_x,
        p.width,
    );
    let max_lines = max_lines_for_height(p.height, font_size, p.line_height);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
    }

    let eff_rt_font_size = if p.rt_font_size > 0 {
        p.rt_font_size as f32
    } else {
        font_size as f32 * 0.5
    };

    let force_last_line = (font_size as u32) < p.base_font_size;

    for (index, line_tokens) in lines.iter().enumerate() {
        let line_y = if index == 0 {
            p.y
        } else {
            p.y + index as f32 * font_size as f32 * p.line_height
        };
        let is_last = index == lines.len() - 1;
        let slots = measure_ruby_slots(
            line_tokens,
            p.family,
            font_size as f32,
            eff_rt_font_size,
            p.letter_spacing,
            p.rt_font_scale_x,
        );
        let line_width = slots
            .iter()
            .map(RubySlot::slot_width)
            .sum::<f32>()
            .min(p.width);

        let (line_x, justify_gap) = if p.align == TextAlign::Justify
            && (!is_last || force_last_line)
            && line_tokens.len() > 1
        {
            let gap = (p.width - line_width).max(0.0) / (line_tokens.len() - 1) as f32;
            (p.x, gap)
        } else {
            let x = match p.align {
                TextAlign::Left | TextAlign::Justify => p.x,
                TextAlign::Center => p.x + (p.width - line_width) / 2.0,
                TextAlign::Right => p.x + p.width - line_width,
            };
            (x, 0.0)
        };

        draw_ruby_text_line(
            pixmap,
            RubyLineParams {
                tokens: line_tokens,
                x: line_x,
                y: line_y,
                font_size: font_size as f32,
                rt_font_size: eff_rt_font_size,
                rt_top: p.rt_top,
                rt_font_scale_x_override: p.rt_font_scale_x,
                color: p.color,
                shadow_color: p.shadow_color,
                brush: p.brush.clone(),
                shadow_brush: p.shadow_brush.clone(),
                family: p.family,
                language: p.language,
                letter_spacing: p.letter_spacing,
                scale_x: 1.0,
                justify_gap,
                font_weight: p.font_weight,
            },
        );
    }
}

fn draw_multiline_ruby_with_first_line_compress(
    pixmap: &mut Pixmap,
    first_line: &str,
    rest: &str,
    p: RubyMultilineParams<'_>,
) {
    let first_line_height = p.base_font_size as f32;
    let remaining_height = (p.height - first_line_height * p.line_height).max(0.0);
    let tokens = parse_ruby_text(first_line);
    let first_line_scale_x = fit_ruby_text_scale(
        &tokens,
        p.family,
        p.base_font_size as f32,
        p.rt_font_size as f32,
        p.letter_spacing,
        p.rt_font_scale_x,
        p.width,
    );
    let slots = measure_ruby_slots(
        &tokens,
        p.family,
        p.base_font_size as f32,
        p.rt_font_size as f32,
        p.letter_spacing,
        p.rt_font_scale_x,
    );
    let line_width = slots
        .iter()
        .map(RubySlot::slot_width)
        .sum::<f32>()
        .min(p.width / first_line_scale_x.max(0.01))
        * first_line_scale_x;
    let line_x = match p.align {
        TextAlign::Left | TextAlign::Justify => p.x,
        TextAlign::Center => p.x + (p.width - line_width) / 2.0,
        TextAlign::Right => p.x + p.width - line_width,
    };

    draw_ruby_text_line(
        pixmap,
        RubyLineParams {
            tokens: &tokens,
            x: line_x,
            y: p.y,
            font_size: p.base_font_size as f32,
            rt_font_size: p.rt_font_size as f32,
            rt_top: p.rt_top,
            rt_font_scale_x_override: p.rt_font_scale_x,
            color: p.color,
            shadow_color: p.shadow_color,
            brush: p.brush.clone(),
            shadow_brush: p.shadow_brush.clone(),
            family: p.family,
            language: p.language,
            letter_spacing: p.letter_spacing,
            scale_x: first_line_scale_x,
            justify_gap: 0.0,
            font_weight: p.font_weight,
        },
    );

    if rest.trim().is_empty() || remaining_height <= 0.0 {
        return;
    }

    draw_multiline_ruby_text(
        pixmap,
        RubyMultilineParams {
            text: rest,
            y: p.y + p.base_font_size as f32 * p.line_height,
            height: remaining_height,
            first_line_compress: false,
            ..p
        },
    );
}

fn split_first_explicit_ruby_line(text: &str) -> Option<(&str, &str)> {
    let newline_index = text.find('\n')?;
    let first_line = text[..newline_index].trim_end_matches('\r');
    let rest = text[newline_index + 1..].trim_start_matches('\n');
    Some((first_line, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_ruby_text_at_first_explicit_newline_for_first_line_compress() {
        let split = split_first_explicit_ruby_line("[青眼(ブルーアイズ)]の白龍\n通常モンスター");

        assert_eq!(split, Some(("[青眼(ブルーアイズ)]の白龍", "通常モンスター")));
    }
}
