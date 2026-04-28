//! Text-width measurement, line-wrapping, and single-line fitting.
//!
//! All functions in this module work purely with advance widths (via the
//! cached [`TextEngine`]) and produce layout data.  They do **not** touch
//! any `Pixmap` — rendering lives in [`super::draw`].

use super::engine::with_text_engine;

// ─────────────────────────────────────────────────────────────────────────────
// Public layout types
// ─────────────────────────────────────────────────────────────────────────────

/// The result of fitting a single line of text into a constrained width.
///
/// Both `font_size` and `scale_x` may be reduced from their initial values
/// to make the text fit.
#[derive(Debug, Clone)]
pub struct SingleLineLayout {
    pub text: String,
    pub font_size: u32,
    pub max_width: u32,
    pub letter_spacing: f32,
    pub scale_x: f32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Public text-width estimators
// ─────────────────────────────────────────────────────────────────────────────

/// Estimate the rendered width of `text` at `font_size` with `letter_spacing`
/// (scale_x = 1.0).
pub fn estimate_text_width(
    text: &str,
    _language: Option<&str>, // TODO: reserved for future per-language shaping
    family_name: &str,
    font_size: f32,
    letter_spacing: f32,
) -> f32 {
    estimate_text_width_scaled(text, _language, family_name, font_size, letter_spacing, 1.0)
}

/// Estimate the rendered width of `text` with a horizontal scale factor applied.
pub fn estimate_text_width_scaled(
    text: &str,
    _language: Option<&str>, // TODO: reserved for future per-language shaping
    family_name: &str,
    font_size: f32,
    letter_spacing: f32,
    scale_x: f32,
) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let (raw, count) =
        with_text_engine(|engine| engine.measure_raw_advances(text, family_name, font_size));
    let width = raw + letter_spacing * count.saturating_sub(1) as f32;
    (width * scale_x).max(0.0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Single-line fitting
// ─────────────────────────────────────────────────────────────────────────────

/// Fit `text` into `max_width` by reducing font size if necessary.
///
/// The font size is scaled by the ratio `max_width / estimated_width` and
/// clamped to `[min_font_size, base_font_size]`.
pub fn fit_single_line(
    text: &str,
    language: Option<&str>,
    base_font_size: u32,
    family_name: &str,
    max_width: u32,
    letter_spacing: f32,
    min_font_size: u32,
) -> SingleLineLayout {
    if text.trim().is_empty() {
        return SingleLineLayout {
            text: text.to_string(),
            font_size: base_font_size,
            max_width,
            letter_spacing,
            scale_x: 1.0,
        };
    }

    let estimated = estimate_text_width(
        text,
        language,
        family_name,
        base_font_size as f32,
        letter_spacing,
    );
    let ratio = (max_width as f32 / estimated).min(1.0);
    let scaled_font = ((base_font_size as f32) * ratio).floor() as u32;

    SingleLineLayout {
        text: text.to_string(),
        font_size: scaled_font.max(min_font_size).min(base_font_size),
        max_width,
        letter_spacing,
        scale_x: 1.0,
    }
}

/// Fit `text` into `max_width` by compressing horizontally (`scale_x < 1.0`).
///
/// Font size stays at `base_font_size`.  If even at `min_scale_x` the text
/// does not fit, it is truncated.
pub fn fit_single_line_compressed(
    text: &str,
    language: Option<&str>,
    base_font_size: u32,
    family_name: &str,
    max_width: u32,
    letter_spacing: f32,
    min_scale_x: f32,
) -> SingleLineLayout {
    if text.trim().is_empty() {
        return SingleLineLayout {
            text: text.to_string(),
            font_size: base_font_size,
            max_width,
            letter_spacing,
            scale_x: 1.0,
        };
    }

    let estimated = estimate_text_width(
        text,
        language,
        family_name,
        base_font_size as f32,
        letter_spacing,
    );

    let max_width_f = max_width as f32;
    let fit_ratio = if estimated > 0.0 {
        (max_width_f / estimated).min(1.0)
    } else {
        1.0
    };
    let mut scale_x = if estimated > max_width_f {
        fit_ratio.max(min_scale_x).min(1.0)
    } else {
        1.0
    };
    let mut fitted_text = text.to_string();

    if estimated > max_width_f && fit_ratio < min_scale_x {
        scale_x = min_scale_x;
        let unscaled_limit = max_width_f / scale_x;
        fitted_text = truncate_text_to_width(
            text,
            family_name,
            base_font_size as f32,
            letter_spacing,
            unscaled_limit,
        );
    }

    SingleLineLayout {
        text: fitted_text,
        font_size: base_font_size,
        max_width,
        letter_spacing,
        scale_x: scale_x.max(0.0),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Line wrapping
// ─────────────────────────────────────────────────────────────────────────────

/// Wrap `text` into lines that fit within `max_width` at the given font size.
///
/// Newlines (`\n`, `\r\n`) are honoured as hard breaks.  ASCII words are kept
/// together; CJK characters each form their own token and wrap freely.
pub fn wrap_text(
    text: &str,
    _language: Option<&str>, // TODO: reserved for future per-language shaping
    family_name: &str,
    max_width: f32,
    font_size: f32,
    letter_spacing: f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_raw = 0.0_f32;
    let mut current_chars = 0usize;

    for raw_line in text.replace("\r\n", "\n").split('\n') {
        if raw_line.is_empty() {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_raw = 0.0;
                current_chars = 0;
            }
            lines.push(String::new());
            continue;
        }

        for token in tokenize_line(raw_line) {
            let (tok_raw, tok_chars) = with_text_engine(|engine| {
                engine.measure_raw_advances(&token, family_name, font_size)
            });

            let proposed_chars = current_chars + tok_chars;
            let proposed_width =
                current_raw + tok_raw + letter_spacing * proposed_chars.saturating_sub(1) as f32;

            if !current.is_empty() && proposed_width > max_width {
                lines.push(std::mem::take(&mut current));
                current_raw = 0.0;
                current_chars = 0;

                if token.trim().is_empty() {
                    continue;
                }

                let trimmed = token.trim_start();
                let (trim_raw, trim_chars) = with_text_engine(|engine| {
                    engine.measure_raw_advances(trimmed, family_name, font_size)
                });

                current.push_str(trimmed);
                current_raw = trim_raw;
                current_chars = trim_chars;
            } else {
                current.push_str(&token);
                current_raw += tok_raw;
                current_chars += tok_chars;
            }
        }

        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_raw = 0.0;
            current_chars = 0;
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

// ─────────────────────────────────────────────────────────────────────────────
// Height helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Total pixel height of `line_count` lines at `font_size` with `line_height`
/// multiplier.
pub fn total_text_height(line_count: usize, font_size: u32, line_height: f32) -> f32 {
    if line_count == 0 {
        0.0
    } else {
        font_size as f32 + (line_count.saturating_sub(1) as f32 * font_size as f32 * line_height)
    }
}

/// Maximum number of complete lines that fit within `height`.
pub fn max_lines_for_height(height: f32, font_size: u32, line_height: f32) -> usize {
    if height <= 0.0 || font_size == 0 {
        return 0;
    }
    let line_step = font_size as f32 * line_height;
    if line_step <= 0.0 {
        return 1;
    }
    let additional = ((height - font_size as f32).max(0.0) / line_step).floor() as usize;
    1 + additional
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers used by draw.rs
// ─────────────────────────────────────────────────────────────────────────────

/// Split `text` at the first explicit newline.
///
/// Returns `None` if the text contains no newline.
pub fn split_first_explicit_line(text: &str) -> Option<(String, String)> {
    let normalized = text.replace("\r\n", "\n");
    let newline_index = normalized.find('\n')?;
    let first = normalized[..newline_index].trim_end().to_string();
    let rest = normalized[newline_index + 1..]
        .trim_start_matches('\n')
        .to_string();
    Some((first, rest))
}

/// Compute the horizontal `scale_x` needed to fit `text` within `max_width`.
pub fn first_line_scale(
    text: &str,
    language: Option<&str>,
    family_name: &str,
    font_size: f32,
    max_width: f32,
    letter_spacing: f32,
) -> f32 {
    let estimated = estimate_text_width(text, language, family_name, font_size, letter_spacing);
    if estimated <= 0.0 {
        1.0
    } else {
        (max_width / estimated).min(1.0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Accumulate characters from `text` until the next one would exceed `max_width`.
fn truncate_text_to_width(
    text: &str,
    family_name: &str,
    font_size: f32,
    letter_spacing: f32,
    max_width: f32,
) -> String {
    with_text_engine(|engine| {
        let (full_raw, full_count) = engine.measure_raw_advances(text, family_name, font_size);
        if full_raw + letter_spacing * full_count.saturating_sub(1) as f32 <= max_width {
            return text.to_string();
        }

        let mut fitted = String::new();
        let mut raw_acc = 0.0_f32;
        let mut char_count = 0usize;

        for ch in text.chars() {
            let (ch_raw, _) = engine.measure_raw_advances(&ch.to_string(), family_name, font_size);
            let new_width = raw_acc + ch_raw + letter_spacing * char_count as f32;
            if new_width > max_width {
                break;
            }
            fitted.push(ch);
            raw_acc += ch_raw;
            char_count += 1;
        }

        fitted
    })
}

/// Tokenize a single line for wrapping.
///
/// ASCII word characters and a small set of punctuation are grouped into
/// word tokens; whitespace becomes a single-space token; all other characters
/// (CJK, etc.) become individual tokens.
fn tokenize_line(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();

    for ch in text.chars() {
        if ch.is_ascii_whitespace() {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            tokens.push(" ".to_string());
        } else if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '\'' | '/' | ':' | ',' | '.') {
            word.push(ch);
        } else {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            tokens.push(ch.to_string());
        }
    }

    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
}
