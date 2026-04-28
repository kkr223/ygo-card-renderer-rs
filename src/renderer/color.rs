use tiny_skia::Color;
use ygopro_cdb_encode_rs::CardDataEntry;

use crate::{
    card_logic::auto_name_light,
    constants::{NAME_COLOR_DARK, NAME_COLOR_LIGHT},
    model::{NameColor, TextGradient, TextPaint},
    text::TextBrush,
};

use super::ResolvedPaint;

/// Resolve a [`NameColor`] to a concrete `tiny_skia` [`Color`].
///
/// - `Auto` defers to `auto_name_light` which checks card type flags.
/// - `Dark` / `Light` force the standard palette values.
/// - `Custom` parses a CSS hex string (`#rrggbb` or `#rgb`); falls back to
///   `Auto` if parsing fails.
pub(super) fn resolve_name_color(name_color: &NameColor, card: &CardDataEntry) -> Color {
    match name_color {
        NameColor::Auto => {
            if auto_name_light(card) {
                Color::from_rgba8(NAME_COLOR_LIGHT.0, NAME_COLOR_LIGHT.1, NAME_COLOR_LIGHT.2, 255)
            } else {
                Color::from_rgba8(NAME_COLOR_DARK.0, NAME_COLOR_DARK.1, NAME_COLOR_DARK.2, 255)
            }
        }
        NameColor::Dark => {
            Color::from_rgba8(NAME_COLOR_DARK.0, NAME_COLOR_DARK.1, NAME_COLOR_DARK.2, 255)
        }
        NameColor::Light => {
            Color::from_rgba8(NAME_COLOR_LIGHT.0, NAME_COLOR_LIGHT.1, NAME_COLOR_LIGHT.2, 255)
        }
        NameColor::Custom(hex) => parse_hex_color(hex).unwrap_or_else(|| {
            if auto_name_light(card) {
                Color::from_rgba8(NAME_COLOR_LIGHT.0, NAME_COLOR_LIGHT.1, NAME_COLOR_LIGHT.2, 255)
            } else {
                Color::from_rgba8(NAME_COLOR_DARK.0, NAME_COLOR_DARK.1, NAME_COLOR_DARK.2, 255)
            }
        }),
    }
}

pub(super) fn resolve_name_brush(
    request: &crate::model::RenderRequest,
    fallback: Color,
    x: f32,
    width: f32,
) -> ResolvedPaint {
    let paint = request
        .options
        .text_colors
        .name
        .as_ref()
        .cloned()
        .or_else(|| {
            request
                .card
                .name_gradient
                .as_ref()
                .map(|gradient| TextPaint {
                    color: None,
                    gradient: Some(gradient.clone()),
                })
        });
    ResolvedPaint {
        color: paint_color(paint.as_ref(), None, fallback),
        brush: text_brush(paint.as_ref(), None, fallback, x, width),
    }
}

pub(super) fn resolve_title_brush(
    request: &crate::model::RenderRequest,
    document_paint: Option<&TextPaint>,
    fallback: Color,
    x: f32,
    width: f32,
) -> ResolvedPaint {
    if let Some(paint) = document_paint {
        return ResolvedPaint {
            color: paint_color(Some(paint), None, fallback),
            brush: text_brush(Some(paint), None, fallback, x, width),
        };
    }
    resolve_name_brush(request, fallback, x, width)
}

pub(super) fn resolve_name_shadow_brush(
    request: &crate::model::RenderRequest,
    x: f32,
    width: f32,
) -> ResolvedPaint {
    let paint = request
        .options
        .text_colors
        .name_shadow
        .as_ref()
        .cloned()
        .or_else(|| {
            request
                .card
                .name_shadow_gradient
                .as_ref()
                .map(|gradient| TextPaint {
                    color: request.card.name_shadow_color.clone(),
                    gradient: Some(gradient.clone()),
                })
        })
        .or_else(|| {
            request
                .card
                .name_shadow_color
                .as_ref()
                .map(TextPaint::solid)
        });

    ResolvedPaint {
        color: paint_color(paint.as_ref(), None, Color::TRANSPARENT),
        brush: text_brush(paint.as_ref(), None, Color::TRANSPARENT, x, width),
    }
}

pub(super) fn resolve_title_shadow_brush(
    request: &crate::model::RenderRequest,
    document_paint: Option<&TextPaint>,
    x: f32,
    width: f32,
) -> ResolvedPaint {
    if let Some(paint) = document_paint {
        return ResolvedPaint {
            color: paint_color(Some(paint), None, Color::TRANSPARENT),
            brush: text_brush(Some(paint), None, Color::TRANSPARENT, x, width),
        };
    }
    resolve_name_shadow_brush(request, x, width)
}

pub(super) fn text_brush(
    paint: Option<&TextPaint>,
    legacy_color: Option<&str>,
    fallback: Color,
    x: f32,
    width: f32,
) -> Option<TextBrush> {
    let Some(paint) = paint else {
        return legacy_color.and_then(parse_hex_color).map(TextBrush::solid);
    };

    if let Some(brush) = paint
        .gradient
        .as_ref()
        .and_then(|gradient| gradient_brush(gradient, x, width))
    {
        return Some(brush);
    }

    paint
        .color
        .as_deref()
        .or(legacy_color)
        .and_then(parse_hex_color)
        .map(TextBrush::solid)
        .or_else(|| {
            if fallback.alpha() > 0.0 {
                Some(TextBrush::solid(fallback))
            } else {
                None
            }
        })
}

pub(super) fn paint_color(
    paint: Option<&TextPaint>,
    legacy_color: Option<&str>,
    fallback: Color,
) -> Color {
    paint
        .and_then(|paint| paint.color.as_deref())
        .or(legacy_color)
        .and_then(parse_hex_color)
        .or_else(|| {
            paint.and_then(|paint| {
                paint
                    .gradient
                    .as_ref()
                    .and_then(|gradient| parse_hex_color(&gradient.start))
            })
        })
        .unwrap_or(fallback)
}

fn gradient_brush(gradient: &TextGradient, x: f32, width: f32) -> Option<TextBrush> {
    let start = parse_hex_color(&gradient.start)?;
    let end = parse_hex_color(&gradient.end)?;
    Some(TextBrush::horizontal_gradient(start, end, x, width))
}

/// Parse a CSS-style hex color string (`#rrggbb`, `#rrggbbaa`, `#rgb`).
/// Returns `None` if the string is not a recognised hex format.
pub(super) fn parse_hex_color(s: &str) -> Option<Color> {
    let value = s.trim();
    if let Some(color) = parse_named_color(value) {
        return Some(color);
    }

    let hex = value.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some(Color::from_rgba8(r, g, b, 255))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::from_rgba8(r, g, b, 255))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Color::from_rgba8(r, g, b, a))
        }
        _ => None,
    }
}

fn parse_named_color(s: &str) -> Option<Color> {
    let (r, g, b, a) = match s.to_ascii_lowercase().as_str() {
        "black" => (0, 0, 0, 255),
        "white" => (255, 255, 255, 255),
        "silver" => (192, 192, 192, 255),
        "gold" => (255, 215, 0, 255),
        "red" => (255, 0, 0, 255),
        "blue" => (0, 0, 255, 255),
        "green" => (0, 128, 0, 255),
        "purple" => (128, 0, 128, 255),
        "transparent" => (0, 0, 0, 0),
        _ => return None,
    };
    Some(Color::from_rgba8(r, g, b, a))
}
