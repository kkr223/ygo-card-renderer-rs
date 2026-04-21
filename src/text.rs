use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};
use std::io::Cursor;
use std::sync::Mutex;
use tiny_skia::{Color, Pixmap, PremultipliedColorU8};

use crate::asset_bundle::get_bundle;

pub struct TextEngine {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
}

#[derive(Debug, Clone)]
pub(crate) struct SingleLineLayout {
    pub(crate) text: String,
    pub(crate) font_size: u32,
    pub(crate) max_width: u32,
    pub(crate) letter_spacing: f32,
    pub(crate) scale_x: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum TextAlign {
    Left,
    Center,
    Right,
}

pub static TEXT_ENGINE: std::sync::LazyLock<Mutex<TextEngine>> = std::sync::LazyLock::new(|| {
    let mut db = cosmic_text::fontdb::Database::new();

    let bundle = get_bundle();
    for font_meta in bundle.index.fonts.values() {
        if let Ok(bytes) = bundle.get_bytes(&font_meta.buffer) {
            let font_data = if ygo_woff2::is_woff2(bytes) {
                match ygo_woff2::convert_woff2_to_ttf(&mut Cursor::new(bytes)) {
                    Ok(ttf) => ttf,
                    Err(e) => {
                        eprintln!("woff2 decode failed for {:?}: {}", font_meta.buffer, e);
                        continue;
                    }
                }
            } else {
                bytes.to_vec()
            };
            db.load_font_data(font_data);
        }
    }

    db.load_system_fonts();
    db.set_sans_serif_family("ygo-sc");
    db.set_serif_family("ygo-sc");

    let font_system = FontSystem::new_with_locale_and_db("zh-CN".to_string(), db);
    let swash_cache = SwashCache::new();

    Mutex::new(TextEngine {
        font_system,
        swash_cache,
    })
});

pub(crate) fn fit_single_line(
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

pub(crate) fn fit_single_line_compressed(
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

    let max_width = max_width as f32;
    let fit_ratio = if estimated > 0.0 {
        (max_width / estimated).min(1.0)
    } else {
        1.0
    };
    let mut scale_x = if estimated > max_width {
        fit_ratio.max(min_scale_x).min(1.0)
    } else {
        1.0
    };
    let mut fitted_text = text.to_string();

    if estimated > max_width && fit_ratio < min_scale_x {
        scale_x = min_scale_x;
        let unscaled_limit = max_width / scale_x;
        fitted_text = truncate_text_to_width(
            text,
            language,
            family_name,
            base_font_size as f32,
            letter_spacing,
            unscaled_limit,
        );
    }

    SingleLineLayout {
        text: fitted_text,
        font_size: base_font_size,
        max_width: max_width as u32,
        letter_spacing,
        scale_x: scale_x.max(0.0),
    }
}

pub(crate) fn draw_text_line(
    pixmap: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    max_width: f32,
    color: Color,
    shadow_color: Color,
    family_name: &str,
    align: TextAlign,
    language: Option<&str>,
    letter_spacing: f32,
) {
    draw_text_line_scaled(
        pixmap,
        text,
        x,
        y,
        font_size,
        max_width,
        color,
        shadow_color,
        family_name,
        align,
        language,
        letter_spacing,
        1.0,
    );
}

pub(crate) fn draw_text_line_scaled(
    pixmap: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    max_width: f32,
    color: Color,
    shadow_color: Color,
    family_name: &str,
    align: TextAlign,
    language: Option<&str>,
    letter_spacing: f32,
    scale_x: f32,
) {
    if text.trim().is_empty() {
        return;
    }

    let unscaled_width = estimate_text_width(
        text,
        language,
        family_name,
        font_size,
        letter_spacing,
    );
    let estimated = estimate_text_width_scaled(
        text,
        language,
        family_name,
        font_size,
        letter_spacing,
        scale_x,
    )
    .min(max_width);
    let draw_x = match align {
        TextAlign::Left => x,
        TextAlign::Center => x - estimated / 2.0,
        TextAlign::Right => x - estimated,
    };
    let layout_width = if scale_x > 0.0 && scale_x != 1.0 {
        (max_width / scale_x).max(unscaled_width).ceil()
    } else {
        max_width.max(unscaled_width).ceil()
    };

    draw_text_shadowed_scaled(
        pixmap,
        text,
        draw_x,
        y,
        font_size,
        layout_width,
        font_size * 1.4,
        color,
        shadow_color,
        family_name,
        letter_spacing,
        scale_x,
    );
}

pub(crate) fn draw_multiline_text(
    pixmap: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    family_name: &str,
    color: Color,
    shadow_color: Color,
    language: Option<&str>,
    base_font_size: u32,
    line_height: f32,
    letter_spacing: f32,
    min_font_size: u32,
    first_line_compress: bool,
) {
    let text = text.trim_end();
    if text.is_empty() {
        return;
    }

    if first_line_compress {
        if let Some((first_line, rest)) = split_first_explicit_line(text) {
            draw_multiline_text_with_first_line_compress(
                pixmap,
                &first_line,
                &rest,
                x,
                y,
                width,
                height,
                family_name,
                color,
                shadow_color,
                language,
                base_font_size,
                line_height,
                letter_spacing,
                min_font_size,
            );
            return;
        }
    }

    let mut font_size = base_font_size;
    let mut lines = wrap_text(
        text,
        language,
        family_name,
        width,
        font_size as f32,
        letter_spacing,
    );
    while font_size > min_font_size
        && total_text_height(lines.len(), font_size, line_height) > height
    {
        font_size -= 1;
        lines = wrap_text(
            text,
            language,
            family_name,
            width,
            font_size as f32,
            letter_spacing,
        );
    }

    let max_lines = max_lines_for_height(height, font_size, line_height);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
    }

    for (index, line) in lines.iter().enumerate() {
        let line_y = if index == 0 {
            y
        } else {
            y + (index as f32 * font_size as f32 * line_height)
        };
        draw_text_shadowed(
            pixmap,
            line,
            x,
            line_y,
            font_size as f32,
            width,
            font_size as f32 * 1.4,
            color,
            shadow_color,
            family_name,
            letter_spacing,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_multiline_text_with_first_line_compress(
    pixmap: &mut Pixmap,
    first_line: &str,
    rest: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    family_name: &str,
    color: Color,
    shadow_color: Color,
    language: Option<&str>,
    base_font_size: u32,
    line_height: f32,
    letter_spacing: f32,
    min_font_size: u32,
) {
    let first_line_height = base_font_size as f32;
    let remaining_height = (height - first_line_height * line_height).max(0.0);
    let first_line_scale_x = first_line_scale(
        first_line,
        language,
        family_name,
        base_font_size as f32,
        width,
        letter_spacing,
    );

    draw_text_line_scaled(
        pixmap,
        first_line,
        x,
        y,
        base_font_size as f32,
        width,
        color,
        shadow_color,
        family_name,
        TextAlign::Left,
        language,
        letter_spacing,
        first_line_scale_x,
    );

    if rest.trim().is_empty() || remaining_height <= 0.0 {
        return;
    }

    let rest_y = y + base_font_size as f32 * line_height;
    draw_multiline_text(
        pixmap,
        rest,
        x,
        rest_y,
        width,
        remaining_height,
        family_name,
        color,
        shadow_color,
        language,
        base_font_size,
        line_height,
        letter_spacing,
        min_font_size,
        false,
    );
}

pub fn draw_text_shadowed(
    pixmap: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    width: f32,
    height: f32,
    base_color: Color,
    shadow_color: Color,
    family_name: &str,
    letter_spacing: f32,
) {
    draw_text_shadowed_scaled(
        pixmap,
        text,
        x,
        y,
        font_size,
        width,
        height,
        base_color,
        shadow_color,
        family_name,
        letter_spacing,
        1.0,
    );
}

pub fn draw_text_shadowed_scaled(
    pixmap: &mut Pixmap,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    width: f32,
    height: f32,
    base_color: Color,
    shadow_color: Color,
    family_name: &str,
    letter_spacing: f32,
    scale_x: f32,
) {
    if text.trim().is_empty() {
        return;
    }

    let mut engine = TEXT_ENGINE.lock().unwrap();
    let TextEngine {
        font_system,
        swash_cache,
    } = &mut *engine;

    let metrics = Metrics::new(font_size, font_size);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, Some(width), Some(height));

    let resolved_family = primary_family_name(family_name);
    let attrs = Attrs::new()
        .family(Family::Name(resolved_family.as_str()))
        .weight(font_weight_for_family(resolved_family.as_str()));
    buffer.set_text(font_system, text, &attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, true);

    // layout.rs 中的 y 更接近 SVG 的 text-before-edge 坐标，这里补一个基线偏移。
    let baseline_y = y + font_size * 0.82;
    draw_buffer_to_pixmap(
        font_system,
        swash_cache,
        &buffer,
        text,
        pixmap,
        x + 1.0,
        baseline_y + 1.0,
        shadow_color,
        letter_spacing,
        scale_x,
    );
    draw_buffer_to_pixmap(
        font_system,
        swash_cache,
        &buffer,
        text,
        pixmap,
        x,
        baseline_y,
        base_color,
        letter_spacing,
        scale_x,
    );
}

fn draw_buffer_to_pixmap(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    buffer: &Buffer,
    text: &str,
    pixmap: &mut Pixmap,
    offset_x: f32,
    offset_y: f32,
    color: Color,
    letter_spacing: f32,
    scale_x: f32,
) {
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let letter_spacing_offset = cluster_spacing_offset(text, glyph.start, letter_spacing);
            let physical_glyph =
                glyph.physical((offset_x + letter_spacing_offset, offset_y), 1.0);
            if let Some(image) = swash_cache.get_image(font_system, physical_glyph.cache_key) {
                let glyph_width = image.placement.width as usize;
                let glyph_height = image.placement.height as usize;
                if glyph_width == 0 || glyph_height == 0 || image.data.is_empty() {
                    continue;
                }

                let base_x = offset_x;
                let glyph_left = physical_glyph.x + image.placement.left;
                let scaled_left = base_x + (glyph_left as f32 - base_x) * scale_x;
                let x = scaled_left.round() as i32;
                let y = physical_glyph.y - image.placement.top;
                if x < 0 || y < 0 {
                    continue;
                }

                let scaled_glyph_width = ((glyph_width as f32) * scale_x).ceil().max(1.0) as usize;

                for (cy, row) in image.data.chunks(glyph_width).enumerate() {
                    for dest_cx in 0..scaled_glyph_width {
                        let src_cx =
                            ((dest_cx as f32) / scale_x).floor().min((glyph_width - 1) as f32)
                                as usize;
                        let alpha = row[src_cx];
                        if alpha == 0 {
                            continue;
                        }

                        let px = x + dest_cx as i32;
                        let py = y + cy as i32;
                        if px < 0
                            || px >= pixmap.width() as i32
                            || py < 0
                            || py >= pixmap.height() as i32
                        {
                            continue;
                        }

                        let existing = pixmap.pixel(px as u32, py as u32).unwrap().demultiply();
                        let alpha_f = alpha as f32 / 255.0 * color.alpha();

                        let blended_r =
                            color.red() * 255.0 * alpha_f + existing.red() as f32 * (1.0 - alpha_f);
                        let blended_g = color.green() * 255.0 * alpha_f
                            + existing.green() as f32 * (1.0 - alpha_f);
                        let blended_b = color.blue() * 255.0 * alpha_f
                            + existing.blue() as f32 * (1.0 - alpha_f);
                        let blended_a = alpha_f * 255.0 + existing.alpha() as f32 * (1.0 - alpha_f);

                        let width = pixmap.width() as i32;
                        if let Some(pm) = PremultipliedColorU8::from_rgba(
                            blended_r as u8,
                            blended_g as u8,
                            blended_b as u8,
                            blended_a as u8,
                        ) {
                            pixmap.pixels_mut()[(py * width + px) as usize] = pm;
                        }
                    }
                }
            }
        }
    }
}

fn wrap_text(
    text: &str,
    language: Option<&str>,
    family_name: &str,
    max_width: f32,
    font_size: f32,
    letter_spacing: f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for raw_line in text.replace("\r\n", "\n").split('\n') {
        if raw_line.is_empty() {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            lines.push(String::new());
            continue;
        }

        for token in tokenize_line(raw_line) {
            let candidate = if current.is_empty() {
                token.clone()
            } else {
                format!("{current}{token}")
            };

            let width =
                estimate_text_width(&candidate, language, family_name, font_size, letter_spacing);
            if !current.is_empty() && width > max_width {
                lines.push(std::mem::take(&mut current));
                if token.trim().is_empty() {
                    continue;
                }
                current.push_str(token.trim_start());
            } else {
                current.push_str(&token);
            }
        }

        if !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

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

fn total_text_height(line_count: usize, font_size: u32, line_height: f32) -> f32 {
    if line_count == 0 {
        0.0
    } else {
        font_size as f32 + (line_count.saturating_sub(1) as f32 * font_size as f32 * line_height)
    }
}

fn max_lines_for_height(height: f32, font_size: u32, line_height: f32) -> usize {
    if height <= 0.0 || font_size == 0 {
        return 0;
    }

    let line_step = font_size as f32 * line_height;
    if line_step <= 0.0 {
        return 1;
    }

    let additional_lines = ((height - font_size as f32).max(0.0) / line_step).floor() as usize;
    1 + additional_lines
}

fn split_first_explicit_line(text: &str) -> Option<(String, String)> {
    let normalized = text.replace("\r\n", "\n");
    let newline_index = normalized.find('\n')?;
    let first = normalized[..newline_index].trim_end().to_string();
    let rest = normalized[newline_index + 1..].trim_start_matches('\n').to_string();
    Some((first, rest))
}

fn first_line_scale(
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

fn truncate_text_to_width(
    text: &str,
    language: Option<&str>,
    family_name: &str,
    font_size: f32,
    letter_spacing: f32,
    max_width: f32,
) -> String {
    if estimate_text_width(text, language, family_name, font_size, letter_spacing) <= max_width {
        return text.to_string();
    }

    let mut fitted = String::new();
    for ch in text.chars() {
        let candidate = format!("{fitted}{ch}");
        if estimate_text_width(&candidate, language, family_name, font_size, letter_spacing)
            > max_width
        {
            break;
        }
        fitted.push(ch);
    }

    fitted
}

pub(crate) fn estimate_text_width(
    text: &str,
    _language: Option<&str>,
    family_name: &str,
    font_size: f32,
    letter_spacing: f32,
) -> f32 {
    estimate_text_width_scaled(text, _language, family_name, font_size, letter_spacing, 1.0)
}

pub(crate) fn estimate_text_width_scaled(
    text: &str,
    _language: Option<&str>,
    family_name: &str,
    font_size: f32,
    letter_spacing: f32,
    scale_x: f32,
) -> f32 {
    if text.is_empty() {
        return 0.0;
    }

    let mut engine = TEXT_ENGINE.lock().unwrap();
    let TextEngine { font_system, .. } = &mut *engine;

    let metrics = Metrics::new(font_size, font_size);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, None, Some(font_size * 2.0));

    let resolved_family = primary_family_name(family_name);
    let attrs = Attrs::new()
        .family(Family::Name(resolved_family.as_str()))
        .weight(font_weight_for_family(resolved_family.as_str()));
    buffer.set_text(font_system, text, &attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, true);

    let mut width = 0.0_f32;
    for run in buffer.layout_runs() {
        width = width.max(layout_run_width(text, &run.glyphs, letter_spacing));
    }

    (width * scale_x).max(0.0)
}

fn cluster_spacing_offset(text: &str, cluster_start: usize, letter_spacing: f32) -> f32 {
    if letter_spacing == 0.0 || cluster_start == 0 {
        return 0.0;
    }

    let preceding_chars = text
        .get(..cluster_start)
        .map(|prefix| prefix.chars().count())
        .unwrap_or(0);
    letter_spacing * preceding_chars as f32
}

fn layout_run_width(text: &str, glyphs: &[cosmic_text::LayoutGlyph], letter_spacing: f32) -> f32 {
    let mut width = 0.0_f32;

    for glyph in glyphs {
        let glyph_right = glyph.x + glyph.w + cluster_spacing_offset(text, glyph.start, letter_spacing);
        width = width.max(glyph_right);
    }

    width
}

fn primary_family_name(stack: &str) -> String {
    let family = stack
        .split(',')
        .map(|part| part.trim().trim_matches('\'').trim_matches('"'))
        .find(|name| !name.is_empty() && !matches!(*name, "sans-serif" | "serif" | "monospace"))
        .unwrap_or("ygo-sc")
        .to_string();

    match family.as_str() {
        // 打包后的字体内部 family 名与前端别名不完全一致，这里做一层兼容映射。
        "custom1" => "ygo-custom1".to_string(),
        "custom2" => "ygo-custom2".to_string(),
        other => other.to_string(),
    }
}

fn font_weight_for_family(family: &str) -> Weight {
    match family {
        "ygo-atk-def" => Weight::BOLD,
        "ygo-password" => Weight::MEDIUM,
        other if other.starts_with("rd-") => Weight::MEDIUM,
        _ => Weight::NORMAL,
    }
}
