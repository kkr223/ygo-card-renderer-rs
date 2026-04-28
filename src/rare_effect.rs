//! Algorithmic rare/foil effect rendering.
//!
//! Each [`RareType`] variant maps to zero or more composable [`EffectLayer`]s.
//! Layers are drawn directly onto the target [`Pixmap`] using tiny-skia
//! primitives (gradients, pattern tiles) and [`BlendMode::Screen`].
//!
//! No external noise crates are used; all procedural math is inline.

use std::sync::OnceLock;

use tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, GradientStop, LinearGradient, Paint, PathBuilder,
    Pattern, Pixmap, Point, SpreadMode, Transform,
};

use crate::{
    asset_bundle::BaseLayout,
    card_logic::image_frame,
    constants::{CARD_HEIGHT, CARD_WIDTH},
    model::{RareType, YgoCardMeta},
    pixel_ops::{hsv_to_rgb, pixel_hash, screen_pixel},
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
///
/// # Coverage note
///
/// The variants [`RareType::Gr`], [`RareType::Ur`], [`RareType::Utr`], and
/// [`RareType::Dt`] rely on image assets and per-region masking that are only
/// available through the full document render pipeline
/// (`RenderDocument` → `Renderer`).  Calling this function directly for those
/// variants is a no-op — use `Renderer::render_request_png` instead.
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
            LayerKind::BrightBorder { opacity } => {
                draw_bright_border(target, full_rect, art_rect, opacity)
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal layer model
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub(crate) struct CoverageRect {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) w: u32,
    pub(crate) h: u32,
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
    /// Silver-blue bright border used by pser-print.
    BrightBorder { opacity: f32 },
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
        RareType::Sr => vec![EffectLayer::art(LayerKind::RainbowFoil { opacity: 0.46 })],

        RareType::Hr => vec![EffectLayer::full(LayerKind::Holographic { opacity: 0.45 })],

        RareType::Ser => vec![EffectLayer::full(LayerKind::SecretWeave { opacity: 0.66 })],

        RareType::Gser => vec![
            EffectLayer::full(LayerKind::SecretWeave { opacity: 0.58 }),
            EffectLayer::art(LayerKind::RainbowFoil { opacity: 0.18 }),
        ],

        RareType::Pser => vec![
            EffectLayer::art(LayerKind::RainbowFoil { opacity: 0.50 }),
            EffectLayer::art(LayerKind::DotGrid { opacity: 0.60 }),
        ],

        RareType::PserPrint => vec![EffectLayer::full(LayerKind::BrightBorder { opacity: 0.72 })],

        // Gr / Ur / Utr / Dt: effects depend on image assets and masked per-region
        // compositing that require the full document render pipeline.
        // See [`draw_rare_effect`] doc comment for details.
        RareType::Gr | RareType::Ur | RareType::Utr | RareType::Dt => vec![],
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
pub(crate) fn draw_secret_weave(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
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

            let h = pixel_hash(x, y);
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
            pixels[idx] = screen_pixel(pixels[idx], src_r, src_g, src_b, alpha);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Primitive: Rainbow Foil
// ─────────────────────────────────────────────────────────────────────────────

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
pub(crate) fn draw_dot_grid(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    // The tile is constant — build it once, cache it for all subsequent calls.
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
            // White dot with full alpha – Screen blend will lighten the card.
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
pub(crate) fn draw_holographic(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    // ── Layer A: horizontal full-spectrum gradient ────────────────────────────
    draw_rainbow_foil(target, rect, opacity);

    // ── Layer B: procedural shimmer tile ────────────────────────────────────
    // The shimmer pattern is content-independent (hash of tile coordinates only).
    // Build the tile once and reuse it across all calls.
    static SHIMMER_TILE: OnceLock<Pixmap> = OnceLock::new();
    let noise_tile = SHIMMER_TILE.get_or_init(|| {
        let nt = NOISE_TILE;
        let mut tile = Pixmap::new(nt, nt).expect("shimmer tile allocation must succeed");
        let pixels = tile.pixels_mut();
        for py in 0..nt {
            for px in 0..nt {
                let h = pixel_hash(px, py);
                // Only ~15 % of pixels become bright sparkle dots.
                let brightness = if h & 0xFF < 38 {
                    // Intensity varies smoothly within that 15 %.
                    ((h >> 8) & 0xFF) as u8
                } else {
                    0
                };
                // Premultiply: for Screen blend a white sparkle is sufficient.
                // Bake full opacity into the tile; the Pattern opacity handles
                // per-call scaling.
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
        opacity, // scale shimmer brightness by the per-call opacity
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
// Primitive: Bright Border
// ─────────────────────────────────────────────────────────────────────────────

/// Pser-print is a printed bright frame: keep the card surface mostly clean and
/// concentrate the shimmer on the outer card edge and illustration bevel.
pub(crate) fn draw_bright_border(
    target: &mut Pixmap,
    full_rect: CoverageRect,
    art_rect: CoverageRect,
    opacity: f32,
) {
    let width = target.width();
    let height = target.height();
    let x_end = full_rect.x.saturating_add(full_rect.w).min(width);
    let y_end = full_rect.y.saturating_add(full_rect.h).min(height);
    let pixels = target.pixels_mut();

    let outer_band = 56.0_f32;
    let art_band = 34.0_f32;
    let art_left = art_rect.x as f32;
    let art_top = art_rect.y as f32;
    let art_right = art_rect.x.saturating_add(art_rect.w) as f32;
    let art_bottom = art_rect.y.saturating_add(art_rect.h) as f32;

    for y in full_rect.y.min(height)..y_end {
        for x in full_rect.x.min(width)..x_end {
            let xf = x as f32;
            let yf = y as f32;
            let outer_d = xf
                .min(yf)
                .min((CARD_WIDTH - 1) as f32 - xf)
                .min((CARD_HEIGHT - 1) as f32 - yf);
            let mut strength = if outer_d < outer_band {
                let t = 1.0 - outer_d / outer_band;
                t.powf(1.55) * 0.72
            } else {
                0.0
            };

            let near_art_vertical = yf >= art_top - art_band
                && yf <= art_bottom + art_band
                && ((xf - art_left).abs() < art_band || (xf - art_right).abs() < art_band);
            let near_art_horizontal = xf >= art_left - art_band
                && xf <= art_right + art_band
                && ((yf - art_top).abs() < art_band || (yf - art_bottom).abs() < art_band);
            if near_art_vertical || near_art_horizontal {
                let d = (xf - art_left)
                    .abs()
                    .min((xf - art_right).abs())
                    .min((yf - art_top).abs())
                    .min((yf - art_bottom).abs());
                let t = 1.0 - (d / art_band).clamp(0.0, 1.0);
                strength = strength.max(t.powf(1.25) * 0.78);
            }

            if strength <= 0.0 {
                continue;
            }

            let shimmer = ((xf * 0.028 + yf * 0.007).sin() * 0.5 + 0.5).powf(2.0);
            let noise = (pixel_hash(x, y) & 0xff) as f32 / 255.0;
            let strength = (strength * (0.70 + shimmer * 0.24 + noise * 0.08)).min(1.0);
            let blue = (0.72 + shimmer * 0.22).min(1.0);
            let src_r = (205.0 + shimmer * 35.0) as u8;
            let src_g = (218.0 + shimmer * 30.0) as u8;
            let src_b = (235.0 + blue * 20.0) as u8;
            let alpha = (strength * opacity * 255.0).round() as u8;

            let idx = (y * width + x) as usize;
            pixels[idx] = screen_pixel(pixels[idx], src_r, src_g, src_b, alpha);
        }
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
    fn sr_maps_to_art_rainbow_only() {
        let layers = layers_for(RareType::Sr);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].coverage, RareCoverage::Art);
        assert!(matches!(layers[0].kind, LayerKind::RainbowFoil { .. }));
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
    fn pser_print_maps_to_bright_border_only() {
        let layers = layers_for(RareType::PserPrint);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].coverage, RareCoverage::FullCard);
        assert!(matches!(layers[0].kind, LayerKind::BrightBorder { .. }));
    }

    #[test]
    fn gr_ur_dt_no_layers() {
        assert!(layers_for(RareType::Gr).is_empty());
        assert!(layers_for(RareType::Ur).is_empty());
        assert!(layers_for(RareType::Utr).is_empty());
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
    fn bright_border_prefers_edges_over_center() {
        let mut px = make_card_pixmap();
        let full = CoverageRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let art = CoverageRect {
            x: 28,
            y: 28,
            w: 44,
            h: 44,
        };
        let before = px.pixels().to_vec();
        draw_bright_border(&mut px, full, art, 0.8);

        let edge_idx = 5;
        let center_idx = 50 * 100 + 50;
        let edge_delta = px.pixels()[edge_idx].blue() as i16 - before[edge_idx].blue() as i16;
        let center_delta = px.pixels()[center_idx].blue() as i16 - before[center_idx].blue() as i16;
        assert!(edge_delta > center_delta);
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
