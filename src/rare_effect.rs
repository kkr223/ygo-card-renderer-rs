//! Algorithmic rare/foil effect rendering.
//!
//! Each [`RareType`] variant maps to zero or more composable [`EffectLayer`]s.
//! Layers are drawn directly onto the target [`Pixmap`] using tiny-skia
//! primitives (gradients, pattern tiles) and [`BlendMode::Screen`].
//!
//! No external noise crates are used; all procedural math is inline.

use tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, GradientStop, LinearGradient, Paint, PathBuilder,
    Pattern, Pixmap, Point, PremultipliedColorU8, SpreadMode, Transform,
};

use crate::{
    asset_bundle::BaseLayout,
    card_logic::image_frame,
    constants::{CARD_HEIGHT, CARD_WIDTH},
    model::{RareType, YgoCardMeta},
};

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Which region of the card the effect covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RareCoverage {
    /// Only the illustration frame (from [`image_frame`]).
    Art,
    /// The entire card surface.
    FullCard,
}

/// Apply algorithmic foil/rare effects for `rare` onto `target`.
///
/// Drawing happens in-place; the effect is composited on top of whatever has
/// already been rendered (frame, art, mask).
pub fn draw_rare_effect(
    target: &mut Pixmap,
    rare: RareType,
    card: &YgoCardMeta,
    base: &BaseLayout,
) {
    let art_rect = {
        let (x, y, w, h) = image_frame(card, base);
        CoverageRect { x, y, w, h }
    };
    let full_rect = CoverageRect {
        x: 0,
        y: 0,
        w: CARD_WIDTH,
        h: CARD_HEIGHT,
    };

    for layer in layers_for(rare) {
        let rect = match layer.coverage {
            RareCoverage::Art => art_rect,
            RareCoverage::FullCard => full_rect,
        };
        match layer.kind {
            LayerKind::RainbowFoil { opacity } => draw_rainbow_foil(target, rect, opacity),
            LayerKind::DotGrid { opacity } => draw_dot_grid(target, rect, opacity),
            LayerKind::SecretWeave { opacity } => draw_secret_weave(target, rect, opacity),
            LayerKind::Holographic { opacity } => draw_holographic(target, rect, opacity),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal layer model
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct CoverageRect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Debug, Clone, Copy)]
enum LayerKind {
    /// Diagonal multi-stop rainbow LinearGradient.
    RainbowFoil { opacity: f32 },
    /// Horizontal/vertical grid of rainbow circles via Pattern tile.
    DotGrid { opacity: f32 },
    /// Fine prismatic weave used by Secret Rare style cards.
    SecretWeave { opacity: f32 },
    /// Full-spectrum horizontal gradient + noise tile.
    Holographic { opacity: f32 },
}

#[derive(Debug, Clone, Copy)]
struct EffectLayer {
    coverage: RareCoverage,
    kind: LayerKind,
}

impl EffectLayer {
    const fn art(kind: LayerKind) -> Self {
        Self {
            coverage: RareCoverage::Art,
            kind,
        }
    }
    const fn full(kind: LayerKind) -> Self {
        Self {
            coverage: RareCoverage::FullCard,
            kind,
        }
    }
}

/// Map each [`RareType`] to its effect layers (ordered, front-to-back).
fn layers_for(rare: RareType) -> Vec<EffectLayer> {
    match rare {
        RareType::Hr => vec![EffectLayer::full(LayerKind::Holographic { opacity: 0.45 })],

        RareType::Ser => vec![EffectLayer::full(LayerKind::SecretWeave { opacity: 0.66 })],

        RareType::Gser => vec![
            EffectLayer::full(LayerKind::SecretWeave { opacity: 0.58 }),
            EffectLayer::art(LayerKind::RainbowFoil { opacity: 0.18 }),
        ],

        RareType::Pser | RareType::PserPrint => vec![
            EffectLayer::art(LayerKind::RainbowFoil { opacity: 0.50 }),
            EffectLayer::art(LayerKind::DotGrid { opacity: 0.60 }),
        ],

        // Gr / Ur / Dt: image-asset path handled elsewhere; no algorithmic layers.
        RareType::Gr | RareType::Ur | RareType::Dt => vec![],
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Primitive: Secret Rare Weave
// ─────────────────────────────────────────────────────────────────────────────

/// Dense prismatic foil: micro line weave + broad diagonal colour bands.
///
/// Real SER foil reads less like round dots and more like an embossed
/// lenticular mesh. This direct pixel pass keeps the card art visible while
/// adding the small square/short-line highlights seen across the full card.
fn draw_secret_weave(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let pixels = target.pixels_mut();

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            let local_x = x.saturating_sub(rect.x);
            let local_y = y.saturating_sub(rect.y);

            let xf = local_x as f32;
            let yf = local_y as f32;
            let diagonal = ((xf * 0.018 - yf * 0.026).sin() * 0.5 + 0.5).powf(1.8);
            let cross = (((xf + yf) * 0.012).sin() * 0.5 + 0.5).powf(2.4);

            let vertical = local_x % 5 <= 1 && local_y % 17 < 13;
            let horizontal = local_y % 5 <= 1 && local_x % 19 < 14;
            let stitch = (local_x + local_y * 2) % 11 == 0;
            let cell = local_x % 5 <= 1 && local_y % 5 <= 1;

            let mut strength = 0.035 + diagonal * 0.16 + cross * 0.08;
            if vertical {
                strength += 0.26;
            }
            if horizontal {
                strength += 0.18;
            }
            if cell {
                strength += 0.16;
            }
            if stitch {
                strength += 0.08;
            }

            let h = hash2(x, y);
            if h & 0x3ff < 18 {
                strength += 0.70;
            } else if h & 0xff < 10 {
                strength += 0.24;
            }

            let hue = (xf * 0.0022 - yf * 0.0036 + diagonal * 0.16 + cross * 0.10).rem_euclid(1.0);
            let (r, g, b) = hsv_to_rgb(hue, 0.92, 1.0);
            let silver = if vertical || horizontal { 0.18 } else { 0.04 };
            let src_r = ((r * (1.0 - silver) + silver) * 255.0).round() as u8;
            let src_g = ((g * (1.0 - silver) + silver) * 255.0).round() as u8;
            let src_b = ((b * (1.0 - silver) + silver) * 255.0).round() as u8;
            let alpha = (strength.min(1.0) * opacity * 255.0).round() as u8;

            let idx = (y * width + x) as usize;
            pixels[idx] = screen_over(pixels[idx], src_r, src_g, src_b, alpha);
        }
    }
}

fn screen_over(
    dst: PremultipliedColorU8,
    src_r: u8,
    src_g: u8,
    src_b: u8,
    alpha: u8,
) -> PremultipliedColorU8 {
    if alpha == 0 {
        return dst;
    }

    let blend = |d: u8, s: u8| -> u8 {
        let effective_src = s as u16 * alpha as u16 / 255;
        (255 - ((255 - d as u16) * (255 - effective_src) / 255)) as u8
    };

    PremultipliedColorU8::from_rgba(
        blend(dst.red(), src_r),
        blend(dst.green(), src_g),
        blend(dst.blue(), src_b),
        dst.alpha(),
    )
    .unwrap_or(dst)
}

// ─────────────────────────────────────────────────────────────────────────────
// Primitive: Rainbow Foil
// ─────────────────────────────────────────────────────────────────────────────

/// Diagonal (top-left → bottom-right) 7-stop rainbow gradient, Screen blend.
fn draw_rainbow_foil(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
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

// ─────────────────────────────────────────────────────────────────────────────
// Primitive: Dot Grid
// ─────────────────────────────────────────────────────────────────────────────

/// Spacing between dot centres (px).
const DOT_SPACING: u32 = 12;
/// Dot radius (px).
const DOT_RADIUS: f32 = 3.5;
/// Tile size = one grid cell.
const TILE_SIZE: u32 = DOT_SPACING;

/// Horizontal/vertical grid of rainbow circles, tiled via `Pattern::new`.
fn draw_dot_grid(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    // Build a TILE_SIZE×TILE_SIZE transparent tile with one circle centred in it.
    // The hue cycles once across the full tile width in the horizontal direction.
    let ts = TILE_SIZE;
    let Some(mut tile) = Pixmap::new(ts, ts) else {
        return;
    };

    let cx = ts as f32 / 2.0;
    let cy = ts as f32 / 2.0;

    // We'll draw many tiles – hue changes per column of the coverage rect.
    // Since Pattern::new tiles the same pixmap, we bake a neutral white dot
    // and rely on the gradient overlay that sits next to it (RainbowFoil)
    // for colour. But we want the dots themselves to carry rainbow colour.
    //
    // Strategy: build the tile with a bright white dot (the Screen blend
    // against the dark card will give a "sparkle" effect), then the
    // RainbowFoil layer underneath provides the hue tint.
    // For SER/GSER the RainbowFoil runs first so the order is correct.

    // Draw circle onto tile using PathBuilder.
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, DOT_RADIUS);
    if let Some(circle_path) = pb.finish() {
        // White dot with full alpha – Screen blend will lighten the card.
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

    // Tile the dot pattern across the coverage rect using Pattern.
    let tile_ref = tile.as_ref();
    let pattern_shader = Pattern::new(
        tile_ref,
        SpreadMode::Repeat,
        FilterQuality::Nearest,
        opacity,
        // Translate pattern origin to coverage rect origin so dots align to (rect.x, rect.y).
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

// ─────────────────────────────────────────────────────────────────────────────
// Primitive: Holographic
// ─────────────────────────────────────────────────────────────────────────────

/// Noise tile size for the holographic shimmer.
const NOISE_TILE: u32 = 64;

/// Full-spectrum horizontal rainbow gradient + procedural shimmer tile.
fn draw_holographic(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    // ── Layer A: horizontal full-spectrum gradient ────────────────────────────
    draw_rainbow_foil(target, rect, opacity);

    // ── Layer B: procedural shimmer tile ────────────────────────────────────
    let nt = NOISE_TILE;
    let Some(mut noise_tile) = Pixmap::new(nt, nt) else {
        return;
    };

    // Generate a shimmer pattern: bright pixels scattered using a cheap
    // deterministic hash — no external noise crate required.
    let pixels = noise_tile.pixels_mut();
    for py in 0..nt {
        for px in 0..nt {
            // Cheap spatial hash (integer arithmetic only).
            let h = hash2(px, py);
            // Only ~15% of pixels become bright sparkle dots.
            let brightness = if h & 0xFF < 38 {
                // Intensity varies smoothly within that 15%.
                ((h >> 8) & 0xFF) as u8
            } else {
                0
            };
            let a = ((brightness as f32 / 255.0) * opacity * 255.0).round() as u8;
            // Premultiply: for Screen blend a white sparkle is sufficient.
            let pm_val = (brightness as u16 * a as u16 / 255) as u8;
            pixels[(py * nt + px) as usize] =
                PremultipliedColorU8::from_rgba(pm_val, pm_val, pm_val, a)
                    .unwrap_or(PremultipliedColorU8::TRANSPARENT);
        }
    }

    let tile_ref = noise_tile.as_ref();
    let pattern_shader = Pattern::new(
        tile_ref,
        SpreadMode::Repeat,
        FilterQuality::Nearest,
        1.0, // opacity already baked into the pixel alphas above
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

// ─────────────────────────────────────────────────────────────────────────────
// Math helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Convert HSV (all 0.0–1.0) + alpha byte to a premultiplied [`Color`].
#[allow(dead_code)]
pub(crate) fn hsv_to_color(h: f32, s: f32, v: f32, alpha: f32) -> Color {
    let (r, g, b) = hsv_to_rgb(h, s, v);
    Color::from_rgba(r * alpha, g * alpha, b * alpha, alpha).unwrap_or(Color::TRANSPARENT)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s == 0.0 {
        return (v, v, v);
    }
    let h6 = (h * 6.0).rem_euclid(6.0);
    let i = h6 as u32;
    let f = h6 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

/// Deterministic 2D integer hash — no external deps.
/// Returns a u32 whose bits are pseudo-random given (x, y).
#[inline]
fn hash2(x: u32, y: u32) -> u32 {
    let mut h = x
        .wrapping_mul(2246822519)
        .wrapping_add(y.wrapping_mul(3266489917));
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    h
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── hsv_to_color anchor values ────────────────────────────────────────────

    #[test]
    fn hsv_red() {
        let c = hsv_to_color(0.0, 1.0, 1.0, 1.0);
        // Premultiplied red at full alpha → RGBA (1,0,0,1)
        assert!((c.red() - 1.0).abs() < 0.01, "red channel");
        assert!(c.green() < 0.01, "green channel");
        assert!(c.blue() < 0.01, "blue channel");
    }

    #[test]
    fn hsv_green() {
        let c = hsv_to_color(1.0 / 3.0, 1.0, 1.0, 1.0);
        assert!(c.red() < 0.01, "red");
        assert!((c.green() - 1.0).abs() < 0.01, "green");
        assert!(c.blue() < 0.01, "blue");
    }

    #[test]
    fn hsv_blue() {
        let c = hsv_to_color(2.0 / 3.0, 1.0, 1.0, 1.0);
        assert!(c.red() < 0.01, "red");
        assert!(c.green() < 0.01, "green");
        assert!((c.blue() - 1.0).abs() < 0.01, "blue");
    }

    #[test]
    fn hsv_grey() {
        let c = hsv_to_color(0.0, 0.0, 0.5, 1.0);
        assert!((c.red() - 0.5).abs() < 0.01);
        assert!((c.green() - 0.5).abs() < 0.01);
        assert!((c.blue() - 0.5).abs() < 0.01);
    }

    // ── Layer mapping correctness ─────────────────────────────────────────────

    #[test]
    fn hr_maps_to_holographic_fullcard() {
        let layers = layers_for(RareType::Hr);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].coverage, RareCoverage::FullCard);
        assert!(matches!(layers[0].kind, LayerKind::Holographic { .. }));
    }

    #[test]
    fn ser_maps_to_secret_weave_fullcard() {
        let layers = layers_for(RareType::Ser);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].coverage, RareCoverage::FullCard);
        assert!(matches!(layers[0].kind, LayerKind::SecretWeave { .. }));
    }

    #[test]
    fn gser_adds_art_rainbow_to_secret_weave() {
        let layers = layers_for(RareType::Gser);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].coverage, RareCoverage::FullCard);
        assert_eq!(layers[1].coverage, RareCoverage::Art);
        assert!(matches!(layers[0].kind, LayerKind::SecretWeave { .. }));
        assert!(matches!(layers[1].kind, LayerKind::RainbowFoil { .. }));
    }

    #[test]
    fn pser_keeps_stronger_art_rainbow_than_gser() {
        let gser_foil = layers_for(RareType::Gser)
            .iter()
            .find_map(|l| {
                if let LayerKind::RainbowFoil { opacity } = l.kind {
                    Some(opacity)
                } else {
                    None
                }
            })
            .unwrap();
        let pser_foil = layers_for(RareType::Pser)
            .iter()
            .find_map(|l| {
                if let LayerKind::RainbowFoil { opacity } = l.kind {
                    Some(opacity)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(
            pser_foil > gser_foil,
            "Pser should be brighter than Gser art rainbow"
        );
    }

    #[test]
    fn gr_ur_dt_no_layers() {
        assert!(layers_for(RareType::Gr).is_empty());
        assert!(layers_for(RareType::Ur).is_empty());
        assert!(layers_for(RareType::Dt).is_empty());
    }

    // ── Primitive smoke tests (must not panic, must mutate pixels) ────────────

    fn make_card_pixmap() -> Pixmap {
        let mut p = Pixmap::new(100, 100).unwrap();
        // Fill with a mid-grey so Screen blend is visible.
        p.fill(Color::from_rgba8(80, 80, 80, 255));
        p
    }

    #[test]
    fn rainbow_foil_mutates_pixels() {
        let mut px = make_card_pixmap();
        let original = px.pixels()[50].red();
        let rect = CoverageRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        draw_rainbow_foil(&mut px, rect, 0.5);
        // Screen blend over grey should change at least some pixels.
        let changed = px.pixels().iter().any(|p| p.red() != original);
        assert!(changed, "rainbow_foil should change pixels");
    }

    #[test]
    fn dot_grid_mutates_pixels() {
        let mut px = make_card_pixmap();
        let rect = CoverageRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let before: Vec<_> = px.pixels().to_vec();
        draw_dot_grid(&mut px, rect, 0.5);
        assert!(
            px.pixels()
                .iter()
                .zip(before.iter())
                .any(|(a, b)| a.red() != b.red()),
            "dot_grid should change pixels"
        );
    }

    #[test]
    fn holographic_mutates_pixels() {
        let mut px = make_card_pixmap();
        let rect = CoverageRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let before: Vec<_> = px.pixels().to_vec();
        draw_holographic(&mut px, rect, 0.5);
        assert!(
            px.pixels()
                .iter()
                .zip(before.iter())
                .any(|(a, b)| a.red() != b.red()),
            "holographic should change pixels"
        );
    }

    #[test]
    fn secret_weave_mutates_pixels() {
        let mut px = make_card_pixmap();
        let rect = CoverageRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let before: Vec<_> = px.pixels().to_vec();
        draw_secret_weave(&mut px, rect, 0.5);
        assert!(
            px.pixels()
                .iter()
                .zip(before.iter())
                .any(|(a, b)| a.red() != b.red()),
            "secret_weave should change pixels"
        );
    }

    #[test]
    fn primitives_do_not_panic_on_minimal_rect() {
        let mut px = Pixmap::new(4, 4).unwrap();
        let rect = CoverageRect {
            x: 0,
            y: 0,
            w: 4,
            h: 4,
        };
        draw_rainbow_foil(&mut px, rect, 0.5);
        draw_dot_grid(&mut px, rect, 0.5);
        draw_secret_weave(&mut px, rect, 0.5);
        draw_holographic(&mut px, rect, 0.5);
    }
}
