//! Full-spectrum horizontal gradient + procedural shimmer tile.

use std::sync::OnceLock;

use tiny_skia::{BlendMode, FillRule, FilterQuality, Paint, PathBuilder, Pattern, Pixmap, SpreadMode, Transform};

use crate::pixel_ops::pixel_hash;

use super::{rainbow_foil::draw_rainbow_foil, CoverageRect};

/// Noise tile size for the holographic shimmer.
const NOISE_TILE: u32 = 64;

/// Full-spectrum horizontal rainbow gradient + procedural shimmer tile.
pub(crate) fn draw_holographic(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    // ── Layer A: horizontal full-spectrum gradient ────────────────────────────
    draw_rainbow_foil(target, rect, opacity);

    // ── Layer B: procedural shimmer tile ────────────────────────────────────
    static SHIMMER_TILE: OnceLock<Pixmap> = OnceLock::new();
    let noise_tile = SHIMMER_TILE.get_or_init(|| {
        let nt = NOISE_TILE;
        let mut tile = Pixmap::new(nt, nt).expect("shimmer tile allocation must succeed");
        let pixels = tile.pixels_mut();
        for py in 0..nt {
            for px in 0..nt {
                let h = pixel_hash(px, py);
                let brightness = if h & 0xFF < 38 {
                    ((h >> 8) & 0xFF) as u8
                } else {
                    0
                };
                let pm_val = brightness;
                pixels[(py * nt + px) as usize] =
                    tiny_skia::PremultipliedColorU8::from_rgba(pm_val, pm_val, pm_val, brightness)
                        .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
            }
        }
        tile
    });
    let tile_ref = noise_tile.as_ref();
    let pattern_shader = Pattern::new(
        tile_ref,
        SpreadMode::Repeat,
        FilterQuality::Nearest,
        opacity,
        Transform::identity(),
    );

    let paint = Paint {
        shader: pattern_shader,
        blend_mode: BlendMode::Screen,
        anti_alias: false,
        ..Paint::default()
    };

    let x0 = rect.x as f32;
    let y0 = rect.y as f32;
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
