//! Diagonal rainbow foil gradient.

use tiny_skia::{BlendMode, Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, SpreadMode, Transform};

use super::CoverageRect;

/// Diagonal (top-left → bottom-right) 7-stop rainbow gradient, Screen blend.
pub(crate) fn draw_rainbow_foil(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let alpha = (opacity * 255.0).round() as u8;

    let stops: Vec<GradientStop> = [
        (0.00_f32, [255_u8, 0, 0]),
        (0.17, [255, 165, 0]),
        (0.33, [255, 255, 0]),
        (0.50, [0, 255, 0]),
        (0.67, [0, 0, 255]),
        (0.83, [128, 0, 255]),
        (1.00, [255, 0, 0]),
    ]
    .iter()
    .map(|(pos, [r, g, b])| GradientStop::new(*pos, Color::from_rgba8(*r, *g, *b, alpha)))
    .collect();

    let x0 = rect.x as f32;
    let y0 = rect.y as f32;
    let x1 = (rect.x + rect.w) as f32;
    let y1 = (rect.y + rect.h) as f32;

    let Some(shader) = LinearGradient::new(
        Point::from_xy(x0, y0),
        Point::from_xy(x1, y1),
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    ) else {
        return;
    };

    let paint = Paint {
        shader,
        blend_mode: BlendMode::Screen,
        anti_alias: false,
        ..Paint::default()
    };

    let mut pb = PathBuilder::new();
    pb.push_rect(tiny_skia::Rect::from_xywh(x0, y0, rect.w as f32, rect.h as f32).unwrap());
    if let Some(path) = pb.finish() {
        target.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}
