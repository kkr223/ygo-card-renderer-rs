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

        RareType::Ser => vec![EffectLayer::art(LayerKind::SecretWeave { opacity: 1.0 })],

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

const SECRET_CELL: u32 = 6;

/// SeR micro-grating foil: art area densely covered with circular dots, short
/// vertical flakes, prism rain, and sparse warm-white sparkles. A virtual point
/// light creates a loose “#” diffraction pattern whose colour shifts from
/// orange to blue-purple, while the whole surface keeps the secret-rare grain.
pub(crate) fn draw_secret_weave(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let pixels = target.pixels_mut();
    let rect_w = rect.w.max(1) as f32;
    let rect_h = rect.h.max(1) as f32;
    let max_dist = (rect_w * rect_w + rect_h * rect_h).sqrt();
    let sigma_basis = rect_w.min(rect_h);

    let light_x = rect_w * 0.76;
    let light_y = rect_h * 0.22;
    let band_y0 = rect_h * 0.220;
    let band_y1 = rect_h * 0.735;
    let band_x0 = rect_w * 0.245;
    let band_x1 = rect_w * 0.820;
    let hue_origin_x = rect_w * 0.245;
    let hue_origin_y = rect_h * 0.280;

    let core_sigma = 0.010_f32 * sigma_basis;
    let core_sigma_far = 0.034_f32 * sigma_basis;
    let halo_sigma = 0.040_f32 * sigma_basis;
    let halo_sigma_far = 0.105_f32 * sigma_basis;
    let line_core_width_mult = 0.78_f32;
    let line_core_width_mult_far = 1.18_f32;
    let line_core_lift = 0.52_f32;
    let line_core_lift_far = 0.34_f32;

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            let local_x = x.saturating_sub(rect.x);
            let local_y = y.saturating_sub(rect.y);
            let xf = local_x as f32;
            let yf = local_y as f32;
            let cell_x = local_x / SECRET_CELL;
            let stagger_y = local_y + if (cell_x & 1) != 0 { SECRET_CELL / 2 } else { 0 };
            let cell_y = stagger_y / SECRET_CELL;
            let in_cell_x = local_x % SECRET_CELL;
            let in_cell_y = stagger_y % SECRET_CELL;
            let ch = ser_pixel_hash(cell_x, cell_y);
            let cx = SECRET_CELL as f32 * 0.5;
            let cy = SECRET_CELL as f32 * 0.5;
            let in_xf = in_cell_x as f32;
            let _in_yf = in_cell_y as f32;

            let radius = 2.5_f32;
            let capsule_half_segment = 5.0_f32;
            let mut elem_mask = 0.0_f32;
            for cxi in cell_x.saturating_sub(1)..=(cell_x + 1) {
                let offset_y = if (cxi & 1) != 0 { SECRET_CELL / 2 } else { 0 };
                let center_x = cxi as f32 * SECRET_CELL as f32 + cx;
                for cyi in cell_y.saturating_sub(2)..=(cell_y + 2) {
                    let unit_hash = ser_pixel_hash(cxi, cyi);
                    let center_y = cyi as f32 * SECRET_CELL as f32 + cy - offset_y as f32;
                    let dx = (xf - center_x).abs();
                    let dist = if unit_hash % 9 != 0 {
                        (dx * dx + (yf - center_y) * (yf - center_y)).sqrt()
                    } else {
                        let dy = ((yf - center_y).abs() - capsule_half_segment).max(0.0);
                        (dx * dx + dy * dy).sqrt()
                    };
                    if hard_unit_mask(radius, dist) > 0.0 {
                        elem_mask = 1.0;
                        break;
                    }
                }
                if elem_mask >= 1.0 { break; }
            }

            let u = xf / rect_w;
            let v = yf / rect_h;
            let vertical_grating = 1.0 - smoothstep(0.68, 1.55, (in_xf - cx).abs());
            let diagonal_grating = (ser_sin((xf + yf) * 0.11) * 0.5 + 0.5).powf(10.0);
            let pin_hash = ser_pixel_hash(local_x / 2, local_y / 2);
            let pin_spark = if (pin_hash & 0x1ff) < 11 { 1.0 } else { 0.0 };
            let broad_wave = (ser_sin(u * 8.0 - v * 5.3 + ser_sin(v * 17.0) * 0.35) * 0.5 + 0.5).powf(1.35);
            let cloud_a = value_noise(u, v, 2.4, 3, 11);
            let cloud_b = value_noise(u, v, 5.6, 7, 13);
            let cloud_c = value_noise(u, v, 9.0, 17, 5);
            let cloud = (cloud_a * 0.55 + cloud_b * 0.30 + cloud_c * 0.15).clamp(0.0, 1.0);
            let patch = smoothstep(0.28, 0.76, cloud);

            let dy0 = (yf - band_y0).abs();
            let dy1 = (yf - band_y1).abs();
            let dx0 = (xf - band_x0).abs();
            let dx1 = (xf - band_x1).abs();
            let to_band = dy0.min(dy1).min(dx0).min(dx1);
            let hue_dx = xf - hue_origin_x;
            let hue_dy = yf - hue_origin_y;
            let hue_dist = (hue_dx * hue_dx + hue_dy * hue_dy).sqrt();
            let mut hue_t = smoothstep(0.0, 1.0, (hue_dist / (max_dist * 0.74)).min(1.0));
            hue_t = (hue_t + 0.34 * hue_t * (1.0 - hue_t)).min(1.0);
            let width_t = hue_t.max((((u - 0.18) / 0.82).clamp(0.0, 1.0)) * 0.72);
            let core_s = lerp_f32(core_sigma, core_sigma_far, width_t);
            let halo_s = lerp_f32(halo_sigma, halo_sigma_far, width_t);
            let line_core_width = lerp_f32(line_core_width_mult, line_core_width_mult_far, width_t);
            let line_core_lift = lerp_f32(line_core_lift, line_core_lift_far, width_t);
            let line_core_s = core_s * line_core_width;
            let preset_core = gauss(to_band, core_s);
            let preset_line_core = gauss(to_band, line_core_s);
            let preset_halo = gauss(to_band, halo_s);
            let preset_h = gauss(dy0.min(dy1), line_core_s);
            let preset_v = gauss(dx0.min(dx1), line_core_s);

            let to_lx = light_x - xf;
            let to_ly = light_y - yf;
            let light_dist = (to_lx * to_lx + to_ly * to_ly).sqrt();
            let near_light = 1.0 - (light_dist / (max_dist * 0.56)).clamp(0.0, 1.0);
            let glow = near_light.powi(3) * 0.20;
            let glow_skirt = (1.0 - (light_dist / (max_dist * 0.72)).clamp(0.0, 1.0)).powi(4) * 0.08;
            let glow_angle = to_ly.atan2(to_lx);
            let glow_phase = (glow_angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI);
            let hue_dist = light_dist / max_dist;
            let _hue_diff = 0.035 + hue_dist * (0.780 - 0.035);
            let h_prism = prism_peak(u * 31.0 + v * 1.6 + ((ch >> 17) & 0xffff) as f32 / 65535.0 * 1.7 + cloud_a * 0.42, 8.0)
                + prism_peak(u * 53.0 - v * 2.4 + ((ch >> 33) & 0xffff) as f32 / 65535.0 * 1.9 + cloud_c * 0.35, 10.0) * 0.58;
            let v_prism = prism_peak(v * 35.0 - u * 1.9 + ((ch >> 33) & 0xffff) as f32 / 65535.0 * 1.6 + cloud_b * 0.40, 8.0)
                + prism_peak(v * 59.0 + u * 2.1 + ((ch >> 49) & 0x7fff) as f32 / 32767.0 * 2.0 + cloud_a * 0.36, 10.0) * 0.52;
            let diag_prism = prism_peak((u + v) * 26.0 + ((ch >> 49) & 0x7fff) as f32 / 32767.0 * 1.8 + cloud_c * 0.50, 9.0);

            let h_center0 = 0.165 + ser_sin(u * 9.0 + 0.8) * 0.018 + (cloud_a - 0.5) * 0.030;
            let h_center1 = 0.318 + ser_sin(u * 7.4 + 2.1) * 0.024 + (cloud_b - 0.5) * 0.034;
            let h_center2 = 0.545 + ser_sin(u * 7.8 + 1.4) * 0.030 + (cloud_c - 0.5) * 0.038;
            let h_center3 = 0.725 + ser_sin(u * 8.7 + 3.2) * 0.022 + (cloud_a - 0.5) * 0.030;
            let h_center4 = 0.875 + ser_sin(u * 8.1 + 5.0) * 0.020 + (cloud_b - 0.5) * 0.028;
            let h_band0 = gauss(v - h_center0, 0.030);
            let h_band1 = gauss(v - h_center1, 0.038);
            let h_band2 = gauss(v - h_center2, 0.050);
            let h_band3 = gauss(v - h_center3, 0.042);
            let h_band4 = gauss(v - h_center4, 0.032);
            let slant = v - (0.84 - u * 0.48 + ser_sin(u * 8.4) * 0.026);
            let slant_band = gauss(slant, 0.046);
            let v_lobe0 = gauss(u - (0.185 + ser_sin(v * 6.2) * 0.026), 0.052);
            let v_lobe1 = gauss(u - (0.525 + ser_sin(v * 5.8 + 1.1) * 0.035), 0.072);
            let v_lobe2 = gauss(u - (0.815 + ser_sin(v * 6.8 + 2.4) * 0.034), 0.066);
            let h_cloud = (h_band0 * 0.28 + h_band1 * 0.40 + h_band2 * 0.44 + h_band3 * 0.36 + h_band4 * 0.24 + slant_band * 0.22).min(1.0);
            let v_cloud = (v_lobe0 * 0.22 + v_lobe1 * 0.28 + v_lobe2 * 0.24 + slant_band * 0.16).min(1.0);
            let granular = ((ch >> 10) & 0xff) as f32 / 255.0;
            let fine_hash = ((ser_pixel_hash(local_x / 2, local_y / 2) >> 11) & 0xff) as f32 / 255.0;
            let speckle = smoothstep(0.50, 0.96, granular * 0.36 + fine_hash * 0.30 + cloud_c * 0.34);
            let cloud_gate = 0.36 + patch * 0.64;
            let unit_gate = 0.46 + elem_mask * 0.54 + vertical_grating * 0.14 + pin_spark * 0.18;
            let h_scan_gate = 0.18 + h_prism.max(speckle).min(1.0) * 0.82;
            let v_scan_gate = 0.18 + v_prism.max(speckle).min(1.0) * 0.82;
            let line_foil_gate = (elem_mask * 0.72 + vertical_grating * 0.18 + diagonal_grating * 0.08 + speckle * 0.34 + h_prism.max(v_prism) * 0.24 + pin_spark * 0.30).min(1.0);
            let line_reflect_gate = 0.08 + line_foil_gate * 0.92;
            let h_response = (h_cloud * cloud_gate * h_scan_gate * 0.48 + h_prism * unit_gate * 0.64 + diag_prism * 0.16 + speckle * 0.48 + preset_h * 0.78 * line_reflect_gate).min(1.0);
            let v_response = (v_cloud * cloud_gate * v_scan_gate * 0.39 + v_prism * unit_gate * 0.5632 + diag_prism * 0.18 + speckle * 0.3552 + preset_v * 0.6084 * line_reflect_gate).min(1.0);
            let grain_response = (speckle * 0.72 + h_prism.max(v_prism) * 0.42 + pin_spark * 0.34 + diag_prism * 0.22).min(1.0);
            let line_bright = ((preset_line_core * 0.82 + preset_h.max(preset_v) * 0.42) * line_reflect_gate).min(1.0);
            let warm_warm = smoothstep(0.20, 0.78, near_light);
            let warm_response = ((h_band1 * 0.30 + h_band2 * 0.22 + preset_h * 0.22 * line_reflect_gate + preset_v * 0.12 * line_reflect_gate + warm_warm * 0.22) * (0.38 + h_prism * 0.48 + speckle * 0.14) * (1.06 - width_t * 0.24)).min(1.0);
            let grid_rggb = ser_grid_distribution_rgb(u, v, band_x0 / rect_w, band_x1 / rect_w, band_y0 / rect_h, band_y1 / rect_h, h_response + preset_h * 0.55 * line_reflect_gate, v_response + preset_v * 0.55 * line_reflect_gate, grain_response * 0.72 + fine_hash * 0.28);
            let speckle_rgb = ser_independent_speckle_rgb(u, v, ((ch >> 17) & 0xffff) as f32 / 65535.0, ((ch >> 33) & 0xffff) as f32 / 65535.0, ((ch >> 49) & 0x7fff) as f32 / 32767.0, warm_warm * 0.45 + warm_response * 0.35);
            // Hash-driven colour pops that activate existing foil fragments with
            // an opposite colour temperature. They should not create extra dot
            // geometry; they only re-colour already-visible foil units/grains.
            let loose_hash = ser_pixel_hash(local_x / 2 + 19, local_y / 2 + 43);
            let loose_grain = (loose_hash & 0xffff) as f32 / 65535.0;
            let loose_phase = avoid_magenta_phase(
                (((loose_hash >> 16) & 0xffff) as f32 / 65535.0
                    + u * 0.23
                    - v * 0.17
                    + cloud_b * 0.11)
                    .rem_euclid(1.0),
                warm_warm * 0.25,
            );
            let mut loose_rgb = spectral_phase_rgb(loose_phase, 0.99, 1.0);
            let temp_hash = ((loose_hash >> 32) & 0xffff) as f32 / 65535.0;
            let temp_flip_gate = smoothstep(0.66, 0.965, temp_hash);
            let cool_temp_hue = 0.535 + fine_hash * 0.115; // cyan → blue
            let warm_temp_hue = 0.030 + fine_hash * 0.075; // red-orange → gold
            let opposite_temp_hue = if warm_warm + warm_response * 0.35 > 0.42 {
                cool_temp_hue
            } else {
                warm_temp_hue
            };
            let opposite_temp_rgb = spectral_phase_rgb(opposite_temp_hue, 0.98, 1.0);
            loose_rgb = lerp_rgb(loose_rgb, opposite_temp_rgb, temp_flip_gate * 0.82);
            let loose_gate = smoothstep(0.60, 0.965, loose_grain);
            let foil_colour_gate = (line_foil_gate * 0.62
                + elem_mask * 0.24
                + vertical_grating * 0.18
                + grain_response * 0.34
                + speckle * 0.22)
                .min(1.0);
            let opposite_activation = temp_flip_gate * (0.30 + foil_colour_gate * 0.70);
            let speckle_mix = (grain_response * (0.22 + speckle * 0.30)
                + pin_spark * 0.40
                + h_prism.max(v_prism) * 0.050
                + loose_gate * foil_colour_gate * 0.34
                + opposite_activation * 0.42)
                .min(0.76);
            let loose_mix = (loose_gate * foil_colour_gate * 0.30 + opposite_activation * 0.50).min(0.68);
            let independent_r = speckle_rgb.0 * (1.0 - loose_mix) + loose_rgb.0 * loose_mix;
            let independent_g = speckle_rgb.1 * (1.0 - loose_mix) + loose_rgb.1 * loose_mix;
            let independent_b = speckle_rgb.2 * (1.0 - loose_mix) + loose_rgb.2 * loose_mix;
            let diff_r = grid_rggb.0 * (1.0 - speckle_mix) + independent_r * speckle_mix;
            let diff_g = grid_rggb.1 * (1.0 - speckle_mix) + independent_g * speckle_mix;
            let diff_b = grid_rggb.2 * (1.0 - speckle_mix) + independent_b * speckle_mix;
            let line_core = (preset_line_core * 0.62 * line_reflect_gate + h_response.max(v_response) * 0.64 + grain_response * 0.26).min(1.0);
            let core = (h_response.max(v_response) * 0.88 + grain_response * 0.28 + warm_response * 0.16 + preset_core * 0.045 + line_bright * 0.16).min(1.0);
            let halo = (h_cloud.max(v_cloud) * 0.20 + patch * 0.08 + preset_halo * 0.035).min(1.0);
            let mut strength = core * 0.48 + halo * 0.36;
            let in_band = strength;
            let ambient_glow = if in_band < 0.01 { glow * 0.16 + glow_skirt * 0.10 } else { (glow + glow_skirt) * 0.16 };
            let mut surface = (0.040 + elem_mask * 0.28 + vertical_grating * 0.060 + diagonal_grating * 0.018 + pin_spark * 0.16) * (0.72 + broad_wave * 0.34);
            let lit_gate = (line_core * 0.70 + core * 0.55 + halo * 0.18 + ambient_glow * 1.10).min(1.0);
            surface *= 0.88 + lit_gate * (1.08 - 0.88);
            strength = (surface * (0.54 + strength * 1.34) + ambient_glow + line_core.powf(1.32) * line_core_lift).min(0.92);
            strength = (strength + line_bright * 0.055).min(0.94);
            if elem_mask > 0.2 { strength *= 1.08; }
            let unit_mask = (elem_mask + vertical_grating * 0.26 + diagonal_grating * 0.14 + pin_spark * 0.22).min(1.0);
            if strength < 0.012 && unit_mask < 0.01 { continue; }
            let spark_hash = ser_pixel_hash(local_x / 3, local_y / 3);
            let spark_on = core > 0.58 && (spark_hash & 0x7ff) < 58;
            if strength <= 0.0 && !spark_on { continue; }
            let idx = (y * width + x) as usize;
            let dst = pixels[idx];
            let luminance = (dst.red() as f32 * 0.2126 + dst.green() as f32 * 0.7152 + dst.blue() as f32 * 0.0722) / 255.0;
            let ink_visibility = (1.16 - luminance).clamp(0.34, 1.0);
            let dark_foil_vis = smoothstep(0.92, 0.24, luminance);
            let base_unit_alpha = (unit_mask * 0.52 * (0.70 + broad_wave * 0.30)).min(1.0);
            let base_keep = 1.0 - base_unit_alpha * 0.46;
            let br = dst.red() as f32 / 255.0 * base_keep;
            let bg = dst.green() as f32 / 255.0 * base_keep;
            let bb = dst.blue() as f32 / 255.0 * base_keep;

            let (r, g, b) = if spark_on {
                (0.97, 0.94, 0.76) // warm white sparkle
            } else {
                let gw = (ambient_glow / strength.max(0.001)).clamp(0.0, 1.0);
                let bw = 1.0 - gw;
                let cell_hue = ((ch >> 24) & 0xff) as f32 / 255.0;
                let texture_hue = avoid_magenta_phase((cell_hue * 0.62 + u * 0.16 - v * 0.12 + broad_wave * 0.18 + vertical_grating * 0.08).rem_euclid(1.0), 0.08);
                let band_sat = 0.98 + (0.92 - 0.98) * width_t;
                let band_val_cap = 1.0 + (0.88 - 1.0) * width_t;
                let (tex_r, tex_g, tex_b) = hsv_to_rgb(texture_hue, band_sat, 1.0);
                let band_val = (strength * 1.16 * (1.10 - width_t * 0.08)).max(0.36 * (0.22 + line_core.max(core).max(halo) * 0.78)).min(band_val_cap).min(1.0);
                let band_r = diff_r * (1.0 - 0.10) + tex_r * 0.10;
                let band_g = diff_g * (1.0 - 0.10) + tex_g * 0.10;
                let band_b = diff_b * (1.0 - 0.10) + tex_b * 0.10;
                let rainbow_glow_hue = avoid_magenta_phase(0.040 + bw * 0.04 + glow_phase * 0.34, 0.5);
                let warm_glow_hue = 0.030 + broad_wave * 0.030 + ((ch >> 17) & 0xffff) as f32 / 65535.0 * 0.020;
                let glow_hue = lerp_f32(rainbow_glow_hue, warm_glow_hue, warm_warm);
                let (glow_r, glow_g, glow_b) = hsv_to_rgb(glow_hue, 0.88 + near_light * 0.12, (strength * 0.92).min(0.92));
                ((band_r * band_val) * bw + glow_r * gw, (band_g * band_val) * bw + glow_g * gw, (band_b * band_val) * bw + glow_b * gw)
            };
            let mut alpha = (strength * ink_visibility * (0.64 + dark_foil_vis * 0.42 + line_bright * 0.18)).min(1.0) * opacity;
            alpha *= 1.0 + lit_gate * 0.66 + line_core * 1.08;
            alpha *= 1.0 + line_bright * 0.18;
            alpha = alpha.min(0.99);
            if spark_on { alpha = 0.82 * opacity; }
            let screen_alpha = (alpha * 0.66).min(1.0);
            let src_lum = (0.2126 * r + 0.7152 * g + 0.0722 * b).max(0.001);
            let reflect_lum = (luminance * (0.70 + line_core * 0.10) + strength * 0.22 + grain_response * 0.08)
                .max(0.16 + line_core * 0.14 + line_bright * 0.18)
                .min(0.98);
            let reflect_scale = reflect_lum / src_lum;
            let rr = clamp01(r * reflect_scale);
            let rg = clamp01(g * reflect_scale);
            let rb = clamp01(b * reflect_scale);
            let color_overlay = (alpha * (0.94 * (0.35 + lit_gate * 0.65) + line_core * 1.58 + lit_gate * 0.54 + line_bright * 0.52)).min(0.98);
            let sr = screen_channel_float(br, r, screen_alpha);
            let sg = screen_channel_float(bg, g, screen_alpha);
            let sb = screen_channel_float(bb, b, screen_alpha);
            let out_r = clamp01(sr * (1.0 - color_overlay) + rr * color_overlay);
            let out_g = clamp01(sg * (1.0 - color_overlay) + rg * color_overlay);
            let out_b = clamp01(sb * (1.0 - color_overlay) + rb * color_overlay);
            pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(
                (out_r * 255.0).round() as u8,
                (out_g * 255.0).round() as u8,
                (out_b * 255.0).round() as u8,
                dst.alpha(),
            ).unwrap_or(dst);
        }
    }
}

/// SER micro-foil without the large horizontal/vertical diffraction bands.
///
/// Used for small masked regions such as attribute icons and level/rank stars:
/// they should still read as Secret Rare foil, but the big "#" weave from the
/// illustration would be visually noisy and directionally wrong at this scale.
pub(crate) fn draw_secret_foil(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let rect_w = rect.w.max(1) as f32;
    let rect_h = rect.h.max(1) as f32;
    let pixels = target.pixels_mut();

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            let local_x = x.saturating_sub(rect.x);
            let local_y = y.saturating_sub(rect.y);
            let xf = local_x as f32;
            let yf = local_y as f32;
            let u = xf / rect_w;
            let v = yf / rect_h;

            let cell_x = local_x / SECRET_CELL;
            let stagger_y = local_y + if (cell_x & 1) != 0 { SECRET_CELL / 2 } else { 0 };
            let cell_y = stagger_y / SECRET_CELL;
            let in_cell_x = local_x % SECRET_CELL;
            let ch = ser_pixel_hash(cell_x, cell_y);
            let cx = SECRET_CELL as f32 * 0.5;
            let in_xf = in_cell_x as f32;

            let mut elem_mask = 0.0_f32;
            for cxi in cell_x.saturating_sub(1)..=(cell_x + 1) {
                let offset_y = if (cxi & 1) != 0 { SECRET_CELL / 2 } else { 0 };
                let center_x = cxi as f32 * SECRET_CELL as f32 + cx;
                for cyi in cell_y.saturating_sub(2)..=(cell_y + 2) {
                    let unit_hash = ser_pixel_hash(cxi, cyi);
                    let center_y = cyi as f32 * SECRET_CELL as f32 + cx - offset_y as f32;
                    let dx = (xf - center_x).abs();
                    let dist = if unit_hash % 9 != 0 {
                        (dx * dx + (yf - center_y) * (yf - center_y)).sqrt()
                    } else {
                        let dy = ((yf - center_y).abs() - 5.0).max(0.0);
                        (dx * dx + dy * dy).sqrt()
                    };
                    if hard_unit_mask(2.5, dist) > 0.0 {
                        elem_mask = 1.0;
                        break;
                    }
                }
                if elem_mask >= 1.0 {
                    break;
                }
            }

            let vertical_grating = 1.0 - smoothstep(0.68, 1.55, (in_xf - cx).abs());
            let diagonal_grating = (ser_sin((xf + yf) * 0.11) * 0.5 + 0.5).powf(10.0);
            let pin_hash = ser_pixel_hash(local_x / 2, local_y / 2);
            let pin_spark = if (pin_hash & 0x1ff) < 11 { 1.0 } else { 0.0 };
            let broad_wave = (ser_sin(u * 8.0 - v * 5.3 + ser_sin(v * 17.0) * 0.35) * 0.5 + 0.5).powf(1.35);
            let cloud_a = value_noise(u, v, 2.4, 3, 11);
            let cloud_b = value_noise(u, v, 5.6, 7, 13);
            let cloud_c = value_noise(u, v, 9.0, 17, 5);
            let granular = ((ch >> 10) & 0xff) as f32 / 255.0;
            let fine_hash = ((pin_hash >> 11) & 0xff) as f32 / 255.0;
            let speckle = smoothstep(0.46, 0.94, granular * 0.38 + fine_hash * 0.34 + cloud_c * 0.28);
            let h_prism = prism_peak(u * 31.0 + v * 1.6 + ((ch >> 17) & 0xffff) as f32 / 65535.0 * 1.7 + cloud_a * 0.42, 8.0);
            let v_prism = prism_peak(v * 35.0 - u * 1.9 + ((ch >> 33) & 0xffff) as f32 / 65535.0 * 1.6 + cloud_b * 0.40, 8.0);
            let diag_prism = prism_peak((u + v) * 26.0 + ((ch >> 49) & 0x7fff) as f32 / 32767.0 * 1.8 + cloud_c * 0.50, 9.0);
            let foil_gate = (elem_mask * 0.56
                + vertical_grating * 0.26
                + diagonal_grating * 0.10
                + speckle * 0.36
                + h_prism.max(v_prism) * 0.30
                + pin_spark * 0.32)
                .min(1.0);
            // Roughly a quarter of the existing foil fragments are only lightly
            // activated: they keep the base SER foil texture, but do not take on
            // the full rainbow cover. This breaks regularity without punching
            // visible holes in the foil.
            let activation_hash = ser_pixel_hash(cell_x.wrapping_add(97), cell_y.wrapping_add(193));
            let activation_scale = if (activation_hash & 0xffff) < 16_384 { 0.36 } else { 1.0 };
            if foil_gate < 0.015 {
                continue;
            }

            let phase_a = ((ch >> 17) & 0xffff) as f32 / 65535.0;
            let phase_b = ((ch >> 33) & 0xffff) as f32 / 65535.0;
            let phase_c = ((ch >> 49) & 0x7fff) as f32 / 32767.0;
            let base_rgb = ser_independent_speckle_rgb(u, v, phase_a, phase_b, phase_c, 0.22 + cloud_a * 0.28);

            // Region-local horizontal rainbow activation. Left is red/orange,
            // right is blue/indigo; every attribute/star mask gets the full sweep.
            let gradient_t = smoothstep(0.0, 1.0, u);
            let gradient_hue = if gradient_t < 0.34 {
                lerp_f32(0.030, 0.120, gradient_t / 0.34) // red-orange → gold
            } else if gradient_t < 0.42 {
                lerp_f32(0.120, 0.150, (gradient_t - 0.34) / 0.08) // slightly wider yellow
            } else if gradient_t < 0.62 {
                lerp_f32(0.150, 0.360, (gradient_t - 0.42) / 0.20) // yellow-green → green
            } else if gradient_t < 0.78 {
                lerp_f32(0.360, 0.560, (gradient_t - 0.62) / 0.16) // green → cyan
            } else {
                lerp_f32(0.580, 0.690, (gradient_t - 0.78) / 0.22) // cyan-blue → indigo
            };
            let gradient_rgb = spectral_phase_rgb(gradient_hue, 0.98, 1.0);

            let local_hue = avoid_magenta_phase((u * 0.30 - v * 0.18 + phase_a * 0.42 + broad_wave * 0.16).rem_euclid(1.0), 0.18);
            let local_rgb = spectral_phase_rgb(local_hue, 0.96, 1.0);
            let shimmer_rgb = lerp_rgb(local_rgb, base_rgb, (0.28 + speckle * 0.32 + h_prism.max(v_prism) * 0.14).min(0.66));
            let mut rgb = lerp_rgb(shimmer_rgb, gradient_rgb, (0.58 + foil_gate * 0.28 + speckle * 0.12).min(0.92));

            // Some already-active foil fragments flip to the opposite colour
            // temperature, but this only changes colour, not geometry/mask.
            let temp_hash = ((pin_hash >> 32) & 0xffff) as f32 / 65535.0;
            let temp_flip = smoothstep(0.64, 0.965, temp_hash) * foil_gate;
            let warm_bias = smoothstep(0.58, 0.90, rgb.0 + rgb.1 * 0.35 - rgb.2 * 0.25);
            let opposite_hue = if warm_bias > 0.45 {
                0.600 + fine_hash * 0.090
            } else {
                0.025 + fine_hash * 0.090
            };
            let opposite_rgb = spectral_phase_rgb(opposite_hue, 0.98, 1.0);
            rgb = lerp_rgb(rgb, opposite_rgb, temp_flip * 0.68);

            let idx = (y * width + x) as usize;
            let dst = pixels[idx];
            let lum = (dst.red() as f32 * 0.2126 + dst.green() as f32 * 0.7152 + dst.blue() as f32 * 0.0722) / 255.0;
            let ink_visibility = (1.16 - lum).clamp(0.34, 1.0);
            let strength = (0.12 + foil_gate * 0.88 + speckle * 0.34 + diag_prism * 0.22)
                * (0.88 + broad_wave * 0.38);
            // Fade activation slightly toward the right so the green/blue side
            // does not visually dominate the warm side.
            let right_fade = lerp_f32(1.0, 0.70, smoothstep(0.12, 1.0, u));
            let local_opacity = opacity * right_fade;
            let alpha = (strength * (0.78 + ink_visibility * 0.22) * local_opacity * activation_scale).min(1.0);
            let dark_alpha = (foil_gate * 0.56 * local_opacity * (0.58 + activation_scale * 0.42)).min(0.64);
            let keep = 1.0 - dark_alpha * 0.54;
            let br = dst.red() as f32 / 255.0 * keep;
            let bg = dst.green() as f32 / 255.0 * keep;
            let bb = dst.blue() as f32 / 255.0 * keep;
            let screen_alpha = (alpha * 0.96).min(1.0);
            let sr = screen_channel_float(br, rgb.0, screen_alpha);
            let sg = screen_channel_float(bg, rgb.1, screen_alpha);
            let sb = screen_channel_float(bb, rgb.2, screen_alpha);
            let overlay = (alpha * (0.82 + foil_gate * 0.76) * (0.72 + activation_scale * 0.28)).min(0.98);
            let src_lum = (0.2126 * rgb.0 + 0.7152 * rgb.1 + 0.0722 * rgb.2).max(0.001);
            let reflect_lum = (lum * 0.36 + strength * 0.52 + speckle * 0.12).clamp(0.24, 1.0);
            let scale = reflect_lum / src_lum;
            let rr = clamp01(rgb.0 * scale);
            let rg = clamp01(rgb.1 * scale);
            let rb = clamp01(rgb.2 * scale);

            pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(
                (clamp01(sr * (1.0 - overlay) + rr * overlay) * 255.0).round() as u8,
                (clamp01(sg * (1.0 - overlay) + rg * overlay) * 255.0).round() as u8,
                (clamp01(sb * (1.0 - overlay) + rb * overlay) * 255.0).round() as u8,
                dst.alpha(),
            )
            .unwrap_or(dst);
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

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn clamp01(v: f32) -> f32 { v.clamp(0.0, 1.0) }

#[inline]
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t.clamp(0.0, 1.0) }

#[inline]
fn gauss(d: f32, sigma: f32) -> f32 { (-d * d / (2.0 * sigma * sigma)).exp() }

#[inline]
fn ser_sin(v: f32) -> f32 { v.sin() }

#[inline]
fn ser_pixel_hash(x: u32, y: u32) -> u64 {
    let mut h = (x as u64 & 0xFFFF) | ((y as u64 & 0xFFFF) << 16);
    h = ((h ^ (h >> 8)).wrapping_mul(0x006e_ed0e_9da1_f1e3)) & 0xFFFF_FFFF_FFFF_FFFF;
    h ^= h >> 31;
    h = h.wrapping_mul(0x006e_ed0e_9da1_f1e3) & 0xFFFF_FFFF_FFFF_FFFF;
    h ^= h >> 31;
    h
}

#[inline]
fn ser_hash01(x: u32, y: u32) -> f32 { (ser_pixel_hash(x, y) & 0xFFFF) as f32 / 65535.0 }

#[inline]
fn value_noise(u: f32, v: f32, scale: f32, seed_x: i32, seed_y: i32) -> f32 {
    let px = u * scale + seed_x as f32 * 17.0;
    let py = v * scale + seed_y as f32 * 19.0;
    let ix = px.floor() as i32;
    let iy = py.floor() as i32;
    let fx = px - ix as f32;
    let fy = py - iy as f32;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let a = ser_hash01(ix as u32, iy as u32);
    let b = ser_hash01((ix + 1) as u32, iy as u32);
    let c = ser_hash01(ix as u32, (iy + 1) as u32);
    let d = ser_hash01((ix + 1) as u32, (iy + 1) as u32);
    (a * (1.0 - sx) + b * sx) * (1.0 - sy) + (c * (1.0 - sx) + d * sx) * sy
}

#[inline]
fn prism_peak(phase: f32, power: f32) -> f32 { (ser_sin(phase * std::f32::consts::TAU) * 0.5 + 0.5).powf(power) }

#[inline]
fn ser_independent_speckle_rgb(u: f32, v: f32, phase_a: f32, phase_b: f32, phase_c: f32, warm_bias: f32) -> (f32, f32, f32) {
    let mut phase = (phase_a * 0.42 + phase_b * 0.31 + phase_c * 0.27 + ser_sin(u * 38.0 + v * 17.0) * 0.055 + u * 0.18 - v * 0.11).rem_euclid(1.0);
    phase = avoid_magenta_phase(phase, warm_bias);
    let r = spectral_phase_rgb(phase, 0.99, 1.0);
    let cool = spectral_phase_rgb(0.57 + phase_b * 0.12, 0.96, 1.0);
    let warm = spectral_phase_rgb(0.045 + phase_c * 0.060, 0.99, 1.0);
    let warm_mix = smoothstep(0.62, 0.94, phase_a) * (0.35 + warm_bias * 0.65);
    let gr = lerp_rgb(cool, warm, warm_mix);
    lerp_rgb(r, gr, 0.38)
}

#[inline]
fn ser_grid_distribution_rgb(u: f32, v: f32, x0: f32, x1: f32, y0: f32, y1: f32, h_weight: f32, v_weight: f32, grain: f32) -> (f32, f32, f32) {
    let cyan = (0.00, 0.66, 0.96);
    let blue = (0.02, 0.12, 1.00);
    let deep_blue = (0.02, 0.02, 0.92);
    let green = (0.28, 0.92, 0.02);
    let lime = (0.78, 1.00, 0.00);
    let yellow = (1.00, 0.96, 0.00);
    let orange = (1.00, 0.52, 0.00);
    let red_orange = (1.00, 0.18, 0.03);
    let top_h = palette_ramp(&[(0.0, cyan), (x0, green), (x0 + (x1 - x0) * 0.42, cyan), (x1, blue), (1.0, deep_blue)], u);
    let bottom_h = palette_ramp(&[(0.0, yellow), (x0 * 0.68, yellow), (x0, orange), (x0 + (x1 - x0) * 0.52, yellow), (x1 - (x1 - x0) * 0.10, lime), (x1, green), (1.0, cyan)], u);
    let left_v = palette_ramp(&[(0.0, cyan), (y0, green), (y0 + (y1 - y0) * 0.24, lime), (y0 + (y1 - y0) * 0.50, yellow), (y0 + (y1 - y0) * 0.78, red_orange), (y1, yellow), (1.0, yellow)], v);
    let right_v = palette_ramp(&[(0.0, deep_blue), (y0, blue), (y0 + (y1 - y0) * 0.50, cyan), (y0 + (y1 - y0) * 0.70, lime), (y1, cyan), (1.0, cyan)], v);
    let top_near = (-((v - y0) * (v - y0)) / (2.0 * 0.115 * 0.115)).exp();
    let bottom_near = (-((v - y1) * (v - y1)) / (2.0 * 0.115 * 0.115)).exp();
    let left_near = (-((u - x0) * (u - x0)) / (2.0 * 0.105 * 0.105)).exp();
    let right_near = (-((u - x1) * (u - x1)) / (2.0 * 0.105 * 0.105)).exp();
    let h_color = lerp_rgb(top_h, bottom_h, bottom_near / (top_near + bottom_near).max(0.001));
    let v_color = lerp_rgb(left_v, right_v, right_near / (left_near + right_near).max(0.001));
    let hw = h_weight.max(0.0) * (0.35 + top_near + bottom_near);
    let vw = v_weight.max(0.0) * (0.35 + left_near + right_near);
    let vertical_mix = vw / (hw + vw).max(0.001);
    let rgb = lerp_rgb(h_color, v_color, vertical_mix);
    let cool_glint = lerp_rgb(cyan, blue, smoothstep(0.35, 0.86, grain));
    let warm_glint = lerp_rgb(yellow, red_orange, smoothstep(0.68, 1.0, grain));
    let warm_gate = left_near * bottom_near + bottom_near * (1.0 - right_near) * 0.45;
    let glint = lerp_rgb(cool_glint, warm_glint, warm_gate.min(1.0));
    lerp_rgb(rgb, glint, 0.045 + grain * 0.035)
}

#[inline]
fn lerp_rgb(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    let t = t.clamp(0.0, 1.0);
    (a.0 * (1.0 - t) + b.0 * t, a.1 * (1.0 - t) + b.1 * t, a.2 * (1.0 - t) + b.2 * t)
}

#[inline]
fn palette_ramp(stops: &[(f32, (f32, f32, f32))], t: f32) -> (f32, f32, f32) {
    let t = t.clamp(0.0, 1.0);
    for i in 0..stops.len() - 1 {
        let (p0, c0) = stops[i];
        let (p1, c1) = stops[i + 1];
        if t <= p1 {
            let local = if p1 <= p0 { 0.0 } else { (t - p0) / (p1 - p0) };
            return lerp_rgb(c0, c1, smoothstep(0.0, 1.0, local));
        }
    }
    stops[stops.len() - 1].1
}

#[inline]
fn avoid_magenta_phase(phase: f32, warm_bias: f32) -> f32 {
    let h = phase.rem_euclid(1.0);
    if !(0.76..=0.965).contains(&h) { return h; }
    let t = (h - 0.76) / 0.205;
    if warm_bias > 0.55 { return (0.030 + t * 0.070).rem_euclid(1.0); }
    let blue_target = 0.610 - t * 0.070;
    let warm_target = 0.045 + t * 0.045;
    let use_warm = smoothstep(0.56, 0.88, t) * (0.38 + warm_bias * 0.62);
    blue_target * (1.0 - use_warm) + warm_target * use_warm
}

#[inline]
fn spectral_phase_rgb(phase: f32, saturation: f32, value: f32) -> (f32, f32, f32) { hsv_to_rgb(phase.rem_euclid(1.0), saturation, value) }

#[inline]
fn screen_channel_float(dst: f32, src: f32, alpha: f32) -> f32 { 1.0 - (1.0 - dst) * (1.0 - src * alpha) }

#[inline]
fn hard_unit_mask(radius: f32, dist: f32) -> f32 { if dist <= radius { 1.0 } else { 0.0 } }


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
