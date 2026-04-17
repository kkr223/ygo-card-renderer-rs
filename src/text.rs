use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use std::io::Cursor;
use std::sync::Mutex;
use tiny_skia::{Color, Pixmap, PremultipliedColorU8};

use crate::asset_bundle::get_bundle;

pub struct TextEngine {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SingleLineLayout {
    pub(crate) font_size: u32,
    pub(crate) max_width: u32,
    pub(crate) letter_spacing: f32,
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
            font_size: base_font_size,
            max_width,
            letter_spacing,
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
        font_size: scaled_font.max(min_font_size).min(base_font_size),
        max_width,
        letter_spacing,
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
    if text.trim().is_empty() {
        return;
    }

    let estimated =
        estimate_text_width(text, language, family_name, font_size, letter_spacing).min(max_width);
    let draw_x = match align {
        TextAlign::Left => x,
        TextAlign::Center => x - estimated / 2.0,
        TextAlign::Right => x - estimated,
    };

    draw_text_shadowed(
        pixmap,
        text,
        draw_x,
        y,
        font_size,
        max_width,
        font_size * 1.4,
        color,
        shadow_color,
        family_name,
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
) {
    let text = text.trim_end();
    if text.is_empty() {
        return;
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
        );
    }
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
    let attrs = Attrs::new().family(Family::Name(resolved_family.as_str()));
    buffer.set_text(font_system, text, &attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, true);

    // layout.rs 中的 y 更接近 SVG 的 text-before-edge 坐标，这里补一个基线偏移。
    let baseline_y = y + font_size * 0.82;
    draw_buffer_to_pixmap(
        font_system,
        swash_cache,
        &buffer,
        pixmap,
        x + 1.0,
        baseline_y + 1.0,
        shadow_color,
    );
    draw_buffer_to_pixmap(
        font_system,
        swash_cache,
        &buffer,
        pixmap,
        x,
        baseline_y,
        base_color,
    );
}

fn draw_buffer_to_pixmap(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    buffer: &Buffer,
    pixmap: &mut Pixmap,
    offset_x: f32,
    offset_y: f32,
    color: Color,
) {
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            let physical_glyph = glyph.physical((offset_x, offset_y), 1.0);
            if let Some(image) = swash_cache.get_image(font_system, physical_glyph.cache_key) {
                let glyph_width = image.placement.width as usize;
                let glyph_height = image.placement.height as usize;
                if glyph_width == 0 || glyph_height == 0 || image.data.is_empty() {
                    continue;
                }

                let x = physical_glyph.x + image.placement.left;
                let y = physical_glyph.y - image.placement.top;
                if x < 0 || y < 0 {
                    continue;
                }

                for (cy, row) in image.data.chunks(glyph_width).enumerate() {
                    for (cx, &alpha) in row.iter().enumerate() {
                        if alpha == 0 {
                            continue;
                        }

                        let px = x + cx as i32;
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

pub(crate) fn estimate_text_width(
    text: &str,
    _language: Option<&str>,
    family_name: &str,
    font_size: f32,
    letter_spacing: f32,
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
    let attrs = Attrs::new().family(Family::Name(resolved_family.as_str()));
    buffer.set_text(font_system, text, &attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, true);

    let mut width = 0.0_f32;
    for run in buffer.layout_runs() {
        width = width.max(run.line_w);
    }

    let char_count = text.chars().count();
    if char_count > 1 {
        width += letter_spacing * (char_count.saturating_sub(1) as f32);
    }

    width.max(0.0)
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
        "ygo-password" => "ygo-password 常规".to_string(),
        other => other.to_string(),
    }
}
