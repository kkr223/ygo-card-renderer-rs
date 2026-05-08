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

        RareType::Ser => vec![EffectLayer::art(LayerKind::SecretWeave { opacity: 0.66 })],

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

const SECRET_FLAKE_CELL: u32 = 6;

/// Dense prismatic foil: dot/short-dash flakes with directional diffraction.
///
/// Real SeR foil is a coated micro-grating over the illustration: tiny dots and
/// short vertical dashes catch light independently, while the ruling direction
/// throws several horizontal/vertical rainbow streaks across the art. This pass
/// models both parts with deterministic procedural facets and a virtual light.
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
            let nx = if rect.w > 1 {
                xf / (rect.w - 1) as f32
            } else {
                0.0
            };
            let ny = if rect.h > 1 {
                yf / (rect.h - 1) as f32
            } else {
                0.0
            };
            let rect_w = rect.w.max(1) as f32;
            let rect_h = rect.h.max(1) as f32;
            let cross = secret_cross_diffraction(xf, yf, rect_w, rect_h);

            let cell_x = local_x / SECRET_FLAKE_CELL;
            let cell_y = local_y / SECRET_FLAKE_CELL;
            let in_cell_x = local_x % SECRET_FLAKE_CELL;
            let in_cell_y = local_y % SECRET_FLAKE_CELL;
            let cell_hash = pixel_hash(cell_x, cell_y);
            let neighbor_hash =
                pixel_hash(cell_x.wrapping_mul(17) + 11, cell_y.wrapping_mul(31) + 7);

            let shape = cell_hash & 0xf;
            let facet_shape = match shape {
                0..=4 => in_cell_x <= 3 && in_cell_y <= 3,
                5..=11 => (2..=3).contains(&in_cell_x) && in_cell_y <= 5,
                12 => in_cell_x <= 3 && in_cell_y <= 5,
                13 => in_cell_x <= 1 && in_cell_y <= 5,
                14 => in_cell_x >= 4 && in_cell_y <= 5,
                _ => in_cell_x + in_cell_y <= 3,
            };
            let facet_shape = facet_shape && (cell_hash & 0xff) < 246;

            let broken_row = (local_y % 12 <= 3 || cross.horizontal > 0.18)
                && in_cell_x <= 5
                && (neighbor_hash & 0x3) != 0
                && (cell_x + ((neighbor_hash >> 5) & 0x3)) % 7 != 0;
            let broken_column = (local_x % 18 <= 1 || cross.vertical > 0.22)
                && in_cell_y <= 5
                && ((neighbor_hash >> 9) & 0x7) < 2
                && (cell_y + ((neighbor_hash >> 13) & 0x3)) % 5 != 0;

            let jitter = (((cell_hash >> 18) & 0xff) as f32 / 255.0 - 0.5) * 0.16;
            let angle = match (cell_hash >> 3) & 0x7 {
                0 | 1 | 2 => jitter,
                3 | 4 | 5 => std::f32::consts::FRAC_PI_2 + jitter,
                6 => std::f32::consts::FRAC_PI_4 + jitter,
                _ => -std::f32::consts::FRAC_PI_4 + jitter,
            };
            let normal_x = angle.cos();
            let normal_y = angle.sin();
            let groove_phase = ((neighbor_hash >> 4) & 0xff) as f32 / 255.0 * 2.35;
            let groove_coord = xf * normal_x + yf * normal_y + groove_phase;
            let groove = diffraction_ridge(groove_coord, 2.35, 0.72);

            let tilt = (((cell_hash >> 24) & 0xff) as f32 / 255.0 - 0.5) * 0.92;
            let light_x = rect_w * 0.72;
            let light_y = rect_h * 0.20;
            let to_light_x = light_x - xf;
            let to_light_y = light_y - yf;
            let light_distance = (to_light_x * to_light_x + to_light_y * to_light_y).sqrt();
            let inv_light_distance = 1.0 / light_distance.max(1.0);
            let light_dir_x = to_light_x * inv_light_distance;
            let light_dir_y = to_light_y * inv_light_distance;
            let view_x = 0.36_f32;
            let view_y = -0.93_f32;
            let alignment = (light_dir_x * normal_x * 0.62
                + light_dir_y * normal_y * 0.62
                + view_x * normal_x * 0.38
                + view_y * normal_y * 0.38
                + tilt)
                .abs()
                .clamp(0.0, 1.0)
                .powf(2.35);
            let random_facet = ((neighbor_hash >> 16) & 0xff) as f32 / 255.0;
            let angular_phase =
                light_distance * 0.035 + alignment * 2.7 + random_facet * 3.1 + cross.phase;
            let distance_gate = diffraction_ridge(angular_phase, 1.0, 0.14);
            let cross_glint =
                (cross.horizontal * 0.66 + cross.vertical * 0.58 + cross.intersection * 0.92)
                    .min(1.0);
            let catch_light = (0.10
                + groove * (0.20 + alignment * 0.56)
                + distance_gate * (0.18 + alignment * 0.30)
                + cross_glint * 0.68)
                * (0.64 + random_facet * 0.36);

            let idx = (y * width + x) as usize;
            let dst = pixels[idx];
            let luminance = (dst.red() as f32 * 0.2126
                + dst.green() as f32 * 0.7152
                + dst.blue() as f32 * 0.0722)
                / 255.0;
            let ink_visibility = (1.12 - luminance).clamp(0.55, 1.0);

            let pixel_spark = pixel_hash(x, y);
            let random_spark = (groove > 0.34 || cross_glint > 0.28 || distance_gate > 0.55)
                && pixel_spark & 0xfff < 28;
            let hot_flake = facet_shape
                && (catch_light > 0.58
                    || (distance_gate > 0.68 && (neighbor_hash & 0x3ff) < 96)
                    || (neighbor_hash & 0x7ff) < 26);

            let mut strength = 0.0_f32;
            if facet_shape {
                strength += 0.095 + catch_light * 0.68 + cross_glint * 0.14;
            }
            if broken_row {
                strength += (groove + cross.horizontal).min(1.0) * (0.055 + alignment * 0.18);
            }
            if broken_column {
                strength += (groove + cross.vertical).min(1.0) * (0.046 + alignment * 0.16);
            }
            if random_spark {
                strength += 0.20 + alignment * 0.20 + distance_gate * 0.20;
            }
            if hot_flake {
                strength += 0.24 + catch_light * 0.30 + cross.intersection * 0.32;
            }
            if strength <= 0.0 {
                continue;
            }

            let groove_phase = (groove_coord / 2.35).rem_euclid(1.0);
            let spectrum = (groove_phase * 0.48
                + alignment * 0.18
                + random_facet * 0.26
                + cross.phase * 0.34
                + light_distance * 0.0018
                + nx * 0.18
                - ny * 0.08)
                .rem_euclid(1.0);
            let hue = (0.72 - spectrum * 0.74).rem_euclid(1.0);
            let hot = hot_flake || random_spark;
            let saturation = if hot { 0.50 } else { 0.96 };
            let value =
                (0.58 + catch_light * 0.42 + alignment * 0.09 + cross_glint * 0.10).min(1.0);
            let (r, g, b) = hsv_to_rgb(hue, saturation, value);
            let silver = if hot_flake {
                0.46 + cross.intersection * 0.18
            } else if broken_row || broken_column {
                0.16 + cross_glint * 0.16
            } else {
                0.06
            };
            let src_r = ((r * (1.0 - silver) + silver) * 255.0).round() as u8;
            let src_g = ((g * (1.0 - silver) + silver) * 255.0).round() as u8;
            let src_b = ((b * (1.0 - silver) + silver) * 255.0).round() as u8;
            let alpha = ((strength * ink_visibility).min(1.0) * opacity * 255.0).round() as u8;

            pixels[idx] = screen_pixel(pixels[idx], src_r, src_g, src_b, alpha);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SecretCross {
    horizontal: f32,
    vertical: f32,
    intersection: f32,
    phase: f32,
}

fn secret_cross_diffraction(xf: f32, yf: f32, w: f32, h: f32) -> SecretCross {
    let horizontal = [
        h * 0.105 + (xf * 0.010).sin() * 4.8,
        h * 0.235 + (xf * 0.007).cos() * 6.0,
        h * 0.415 + (xf * 0.009).sin() * 5.4,
        h * 0.635 + (xf * 0.006).cos() * 7.2,
        h * 0.835 + (xf * 0.011).sin() * 4.6,
    ]
    .into_iter()
    .enumerate()
    .fold(0.0_f32, |acc, (i, center)| {
        let width = if i == 2 { 20.0 } else { 13.5 };
        acc.max(smooth_band((yf - center).abs(), width, 1.1))
    });

    let vertical = [
        w * 0.185 + (yf * 0.010).cos() * 4.4,
        w * 0.405 + (yf * 0.007).sin() * 5.8,
        w * 0.680 + (yf * 0.009).cos() * 5.2,
        w * 0.865 + (yf * 0.012).sin() * 3.8,
    ]
    .into_iter()
    .enumerate()
    .fold(0.0_f32, |acc, (i, center)| {
        let width = if i == 2 { 16.5 } else { 10.5 };
        acc.max(smooth_band((xf - center).abs(), width, 0.95))
    });

    let fine_horizontal = smooth_band(
        wrapped_distance(yf + (xf * 0.016).sin() * 2.4, 8.0),
        1.7,
        0.36,
    );
    let fine_vertical = smooth_band(
        wrapped_distance(xf + (yf * 0.014).cos() * 2.0, 11.0),
        1.35,
        0.30,
    );

    let horizontal = (horizontal * 0.78 + fine_horizontal * 0.22).min(1.0);
    let vertical = (vertical * 0.74 + fine_vertical * 0.20).min(1.0);
    let intersection = (horizontal * vertical).sqrt();
    let phase = (xf * 0.0065 + yf * 0.0042 + horizontal * 0.27 - vertical * 0.19).rem_euclid(1.0);

    SecretCross {
        horizontal,
        vertical,
        intersection,
        phase,
    }
}

fn wrapped_distance(value: f32, period: f32) -> f32 {
    let wrapped = value.rem_euclid(period);
    wrapped.min(period - wrapped)
}

fn smooth_band(distance: f32, outer: f32, hot_core: f32) -> f32 {
    if distance <= hot_core {
        1.0
    } else if distance >= outer {
        0.0
    } else {
        let t = 1.0 - (distance - hot_core) / (outer - hot_core);
        t * t * (3.0 - 2.0 * t)
    }
}

fn diffraction_ridge(coord: f32, period: f32, half_width: f32) -> f32 {
    let wrapped = coord.rem_euclid(period);
    let distance = wrapped.min(period - wrapped);
    if distance >= half_width {
        0.0
    } else {
        let t = 1.0 - distance / half_width;
        t * t * (3.0 - 2.0 * t)
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
    fn ser_maps_to_secret_weave_art_only() {
        let layers = layers_for(RareType::Ser);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].coverage, RareCoverage::Art);
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
