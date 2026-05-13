use tiny_skia::Color;

use crate::{
    model::{GradientDirection, TextGradient, TextPaint},
    text::TextBrush,
};

pub(super) fn text_brush_in_box(
    paint: Option<&TextPaint>,
    legacy_color: Option<&str>,
    fallback: Color,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Option<TextBrush> {
    let Some(paint) = paint else {
        return legacy_color.and_then(parse_hex_color).map(TextBrush::solid);
    };

    if let Some(brush) = paint
        .gradient
        .as_ref()
        .and_then(|gradient| gradient_brush(gradient, x, y, width, height))
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

fn gradient_brush(
    gradient: &TextGradient,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Option<TextBrush> {
    let start = parse_hex_color(&gradient.start)?;
    let end = parse_hex_color(&gradient.end)?;
    match gradient.direction {
        GradientDirection::Horizontal => Some(TextBrush::horizontal_gradient(start, end, x, width)),
        GradientDirection::Vertical => {
            let middle = match gradient.middle.as_deref() {
                Some(value) => parse_hex_color(value)?,
                None => mix_color(start, end, 0.5),
            };
            Some(TextBrush::vertical_middle_gradient(
                start, middle, end, y, height,
            ))
        }
    }
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    Color::from_rgba(
        a.red() * inv + b.red() * t,
        a.green() * inv + b.green() * t,
        a.blue() * inv + b.blue() * t,
        a.alpha() * inv + b.alpha() * t,
    )
    .unwrap_or(a)
}

pub(super) fn parse_hex_color(value: &str) -> Option<Color> {
    let value = value.trim();
    let hex = value.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
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
