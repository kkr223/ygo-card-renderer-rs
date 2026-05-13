//! Horizontal/vertical grid of rainbow circles.

use std::sync::OnceLock;

use tiny_skia::{BlendMode, FillRule, FilterQuality, Paint, PathBuilder, Pattern, Pixmap, SpreadMode, Transform};

use super::CoverageRect;

/// Spacing between dot centres (px).
const DOT_SPACING: u32 = 12;
/// Dot radius (px).
const DOT_RADIUS: f32 = 3.5;
/// Tile size = one grid cell.
const TILE_SIZE: u32 = DOT_SPACING;

/// Horizontal/vertical grid of rainbow circles, tiled via `Pattern::new`.
pub(crate) fn draw_dot_grid(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    static DOT_TILE: OnceLock<Pixmap> = OnceLock::new();
    let tile = DOT_TILE.get_or_init(|| {
        let ts = TILE_SIZE;
        let mut tile =
            Pixmap::new(ts, ts).expect("dot tile allocation must succeed for reasonable TILE_SIZE");
        let cx = ts as f32 / 2.0;
        let cy = ts as f32 / 2.0;
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, DOT_RADIUS);
        if let Some(circle_path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(255, 255, 255, 220);
            paint.anti_alias = true;
            paint.blend_mode = BlendMode::Source;
            tile.fill_path(
                &circle_path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        tile
    });

    let tile_ref = tile.as_ref();
    let pattern_shader = Pattern::new(
        tile_ref,
        SpreadMode::Repeat,
        FilterQuality::Nearest,
        opacity,
        Transform::from_translate(rect.x as f32, rect.y as f32),
    );

    let paint = Paint {
        shader: pattern_shader,
        blend_mode: BlendMode::Screen,
        anti_alias: false,
        ..Paint::default()
    };

    let x0 = rect.x as f32;
    let y0 = rect.y as f32;
    let mut pb2 = PathBuilder::new();
    pb2.push_rect(tiny_skia::Rect::from_xywh(x0, y0, rect.w as f32, rect.h as f32).unwrap());
    if let Some(path) = pb2.finish() {
        target.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}
