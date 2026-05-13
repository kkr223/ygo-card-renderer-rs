//! Optical Secret Rare foil: physically-inspired micro-facet diffraction.
//!
//! Core algorithm: staggered micro-facet normal map, two orthogonal diffraction
//! gratings, FBM energy variation, "#" band efficiency, and Blinn-Phong glints.

use std::sync::OnceLock;

use tiny_skia::Pixmap;

use super::{math::*, CoverageRect};

const OPTICAL_LUT_SIZE: usize = 1024;
const OPTICAL_LAM_MIN: f32 = 380.0;
const OPTICAL_LAM_MAX: f32 = 720.0;

#[derive(Debug, Clone, Copy)]
struct OpticalSerParams {
    seed: u32,
    cell_w: u32,
    cell_h: u32,
    cell_gap: u32,
    tilt_strength: f32,
    dome_depth: f32,
    grating_d_nm: f32,
    diffraction_order: u32,
    blaze1: f32,
    blaze2: f32,
    tilt_factor: f32,
    band_v1: f32,
    band_v2: f32,
    band_h1: f32,
    band_h2: f32,
    band_width: f32,
    band_base: f32,
    sheet_gain: f32,
    micro_gain: f32,
    sparkle_gain: f32,
    light_x: f32,
    light_y: f32,
    light_z: f32,
    light_blaze_shift: f32,
    shininess: f32,
    white_gain: f32,
    darken: f32,
    foil_gain: f32,
    macro_cell: f32,
    cluster_cell: f32,
    energy_floor: f32,
    macro_low: f32,
    macro_high: f32,
    cluster_low: f32,
    cluster_high: f32,
}

const OPTICAL_SER_PARAMS: OpticalSerParams = OpticalSerParams {
    seed: 42,
    cell_w: 7,
    cell_h: 7,
    cell_gap: 2,
    tilt_strength: 0.75,
    dome_depth: 0.06,
    grating_d_nm: 850.0,
    diffraction_order: 1,
    blaze1: 0.65,
    blaze2: 0.30,
    tilt_factor: 0.45,
    band_v1: 0.30,
    band_v2: 0.68,
    band_h1: 0.32,
    band_h2: 0.66,
    band_width: 0.022,
    band_base: 0.02,
    sheet_gain: 3.00,
    micro_gain: 2.50,
    sparkle_gain: 0.90,
    light_x: 0.50,
    light_y: -0.40,
    light_z: 1.00,
    light_blaze_shift: 0.25,
    shininess: 35.0,
    white_gain: 1.00,
    darken: 0.48,
    foil_gain: 1.90,
    macro_cell: 150.0,
    cluster_cell: 24.0,
    energy_floor: 0.28,
    macro_low: 0.15,
    macro_high: 0.92,
    cluster_low: 0.38,
    cluster_high: 0.90,
};

/// Physically-inspired Secret Rare foil for the illustration: a staggered
/// micro-facet normal map, two orthogonal diffraction gratings, broad "#" band
/// efficiency, FBM energy variation, and Blinn-Phong white glints.
pub(crate) fn draw_optical_ser(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= f32::EPSILON {
        return;
    }

    let params = OPTICAL_SER_PARAMS;
    let width = target.width();
    let height = target.height();
    let x_start = rect.x.min(width);
    let y_start = rect.y.min(height);
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let rect_w = rect.w.max(1) as f32;
    let rect_h = rect.h.max(1) as f32;
    let norm_w = (rect_w - 1.0).max(1.0);
    let norm_h = (rect_h - 1.0).max(1.0);
    let light = vec_norm3(params.light_x, params.light_y, params.light_z);
    let half_vec = vec_norm3(light.0, light.1, light.2 + 1.0);
    let light_xy_len = (light.0 * light.0 + light.1 * light.1).sqrt().max(1e-6);
    let pixels = target.pixels_mut();

    for y in y_start..y_end {
        for x in x_start..x_end {
            let local_x = x.saturating_sub(rect.x);
            let local_y = y.saturating_sub(rect.y);
            let xf = local_x as f32;
            let yf = local_y as f32;
            let u = xf / norm_w;
            let v = yf / norm_h;
            let u_centered = u - 0.5;
            let v_centered = v - 0.5;

            let (nx, ny, nz, inside, col, row) = optical_micro_facet(local_x, local_y, params);
            let n_dot_l = clamp01(nx * light.0 + ny * light.1 + nz * light.2);
            let n_dot_h = clamp01(nx * half_vec.0 + ny * half_vec.1 + nz * half_vec.2);
            let macro_axis = (u_centered * light.0 + v_centered * light.1) / light_xy_len;
            let macro_light = smoothstep(-0.70, 0.70, macro_axis);
            let energy = optical_energy(xf, yf, params);

            let light_mod = (n_dot_l - 0.5) * params.light_blaze_shift;
            let delta1 = nx * params.tilt_factor + params.blaze1 + light_mod;
            let delta2 = ny * params.tilt_factor + params.blaze2 - light_mod;
            let order = params.diffraction_order.max(1) as f32;
            let lam1 = params.grating_d_nm * delta1.abs() / order;
            let lam2 = params.grating_d_nm * delta2.abs() / order;
            let rgb1 = optical_lut_lookup(lam1);
            let rgb2 = optical_lut_lookup(lam2);

            let bw = params.band_width.max(0.002);
            let bb = params.band_base;
            let bw_vary = bw * (0.72 + 0.28 * (1.0 - energy));
            let band_intensity = 0.45 + 0.55 * energy;
            let band_v =
                gauss_line(u, params.band_v1, bw_vary).max(gauss_line(u, params.band_v2, bw_vary));
            let band_h =
                gauss_line(v, params.band_h1, bw_vary).max(gauss_line(v, params.band_h2, bw_vary));
            let eff1 = (bb + (1.0 - bb) * band_v * band_intensity).clamp(bb, 1.0);
            let eff2 = (bb + (1.0 - bb) * band_h * band_intensity).clamp(bb, 1.0);

            let light_energy = 0.30 + 0.70 * n_dot_l;
            let combined_energy = energy * inside * light_energy;
            let macro_brightness = 0.25 + 0.75 * macro_light;
            let micro_scale = combined_energy * macro_brightness * params.micro_gain * 0.5;
            let micro_rgb = (
                (rgb1.0 + rgb2.0) * micro_scale,
                (rgb1.1 + rgb2.1) * micro_scale,
                (rgb1.2 + rgb2.2) * micro_scale,
            );

            let band_boost1 = clamp01(eff1 - bb);
            let band_boost2 = clamp01(eff2 - bb);
            let band_rgb = (
                (rgb1.0 * band_boost1 + rgb2.0 * band_boost2) * combined_energy,
                (rgb1.1 * band_boost1 + rgb2.1 * band_boost2) * combined_energy,
                (rgb1.2 * band_boost1 + rgb2.2 * band_boost2) * combined_energy,
            );

            let inv_band_base = (1.0 - bb).max(1e-6);
            let line1 = band_boost1 / inv_band_base;
            let line2 = band_boost2 / inv_band_base;
            let hash_band = (line1 * 0.95).max(line2 * 1.05) + line1 * line2 * 0.70;
            let hash_band = hash_band.clamp(0.0, 1.0);
            let sheet_lam = 440.0
                + 200.0 * macro_light
                + 14.0 * (std::f32::consts::TAU * (u * 11.0 + v * 7.0)).sin();
            let sheet_base = optical_lut_lookup(sheet_lam);
            let sheet_energy = hash_band
                * (0.18 + 0.82 * macro_light)
                * (0.35 + 0.65 * energy)
                * inside
                * params.sheet_gain;
            let sheet_rgb = (
                sheet_base.0 * sheet_energy,
                sheet_base.1 * sheet_energy,
                sheet_base.2 * sheet_energy,
            );

            let diff_r = 1.0 - (-(micro_rgb.0 + band_rgb.0 + sheet_rgb.0) * params.foil_gain).exp();
            let diff_g = 1.0 - (-(micro_rgb.1 + band_rgb.1 + sheet_rgb.1) * params.foil_gain).exp();
            let diff_b = 1.0 - (-(micro_rgb.2 + band_rgb.2 + sheet_rgb.2) * params.foil_gain).exp();

            let mut spec = n_dot_h.powf(params.shininess) * combined_energy;
            let pin = smoothstep(0.84, 0.995, optical_cell_hash(col, row, params.seed + 4242));
            spec += pin
                * (0.18 + 0.82 * hash_band)
                * combined_energy
                * params.sparkle_gain
                * macro_brightness;

            let idx = (y * width + x) as usize;
            let dst = pixels[idx];
            let base_r = dst.red() as f32 / 255.0;
            let base_g = dst.green() as f32 / 255.0;
            let base_b = dst.blue() as f32 / 255.0;

            let dark_r = base_r * params.darken;
            let dark_g = base_g * params.darken;
            let dark_b = base_b * params.darken;
            let reveal = (spec * 0.06).clamp(0.0, 0.06);
            let result_r = clamp01(
                1.0 - (1.0 - dark_r) * (1.0 - diff_r) + spec * params.white_gain + base_r * reveal,
            );
            let result_g = clamp01(
                1.0 - (1.0 - dark_g) * (1.0 - diff_g) + spec * params.white_gain + base_g * reveal,
            );
            let result_b = clamp01(
                1.0 - (1.0 - dark_b) * (1.0 - diff_b) + spec * params.white_gain + base_b * reveal,
            );

            let out_r = lerp_f32(base_r, result_r, opacity);
            let out_g = lerp_f32(base_g, result_g, opacity);
            let out_b = lerp_f32(base_b, result_b, opacity);
            pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(
                (out_r * 255.0).round() as u8,
                (out_g * 255.0).round() as u8,
                (out_b * 255.0).round() as u8,
                dst.alpha(),
            )
            .unwrap_or(dst);
        }
    }
}

/// Simplified Ser foil for small masked regions (attribute icons, level/rank
/// stars). Uses the same micro-facet + dual-grating core as
/// [`draw_optical_ser`], but omits the large-scale "#" diffraction bands,
/// FBM energy field, and broad sheet layer. Instead it applies a left-to-right
/// warm→cool gradient sweep with opposite-temperature flips.
pub(crate) fn draw_optical_ser_simple(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= f32::EPSILON {
        return;
    }

    let params = OPTICAL_SER_PARAMS;
    let width = target.width();
    let height = target.height();
    let x_start = rect.x.min(width);
    let y_start = rect.y.min(height);
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let rect_w = rect.w.max(1) as f32;
    let rect_h = rect.h.max(1) as f32;
    let norm_w = (rect_w - 1.0).max(1.0);
    let norm_h = (rect_h - 1.0).max(1.0);
    let light = vec_norm3(params.light_x, params.light_y, params.light_z);
    let half_vec = vec_norm3(light.0, light.1, light.2 + 1.0);
    let pixels = target.pixels_mut();

    for y in y_start..y_end {
        for x in x_start..x_end {
            let local_x = x.saturating_sub(rect.x);
            let local_y = y.saturating_sub(rect.y);
            let xf = local_x as f32;
            let yf = local_y as f32;
            let u = xf / norm_w;
            let v = yf / norm_h;

            // ── Micro-facet + dual grating (same as draw_optical_ser) ──────────
            let (nx, ny, nz, inside, col, row) = optical_micro_facet(local_x, local_y, params);
            let n_dot_l = clamp01(nx * light.0 + ny * light.1 + nz * light.2);
            let n_dot_h = clamp01(nx * half_vec.0 + ny * half_vec.1 + nz * half_vec.2);

            let light_mod = (n_dot_l - 0.5) * params.light_blaze_shift;
            let delta1 = nx * params.tilt_factor + params.blaze1 + light_mod;
            let delta2 = ny * params.tilt_factor + params.blaze2 - light_mod;
            let order = params.diffraction_order.max(1) as f32;
            let lam1 = params.grating_d_nm * delta1.abs() / order;
            let lam2 = params.grating_d_nm * delta2.abs() / order;
            let rgb1 = optical_lut_lookup(lam1);
            let rgb2 = optical_lut_lookup(lam2);

            // Simplified energy: no FBM, no macro_light, no "#" bands
            let light_energy = 0.30 + 0.70 * n_dot_l;
            let combined_energy = inside * light_energy;
            let micro_scale = combined_energy * params.micro_gain * 0.5;
            let micro_r = (rgb1.0 + rgb2.0) * micro_scale;
            let micro_g = (rgb1.1 + rgb2.1) * micro_scale;
            let micro_b = (rgb1.2 + rgb2.2) * micro_scale;

            // Non-linear diffraction colour
            let diff_r = 1.0 - (-micro_r * params.foil_gain).exp();
            let diff_g = 1.0 - (-micro_g * params.foil_gain).exp();
            let diff_b = 1.0 - (-micro_b * params.foil_gain).exp();

            // Simplified specular
            let mut spec = n_dot_h.powf(params.shininess) * combined_energy;
            let pin = smoothstep(0.84, 0.995, optical_cell_hash(col, row, params.seed + 4242));
            spec += pin * combined_energy * params.sparkle_gain * 0.5;

            // ── Left-to-right warm→cool gradient ─────────────────────────────
            let gradient_t = smoothstep(0.0, 1.0, u);
            let gradient_hue = if gradient_t < 0.34 {
                lerp_f32(0.030, 0.120, gradient_t / 0.34)
            } else if gradient_t < 0.42 {
                lerp_f32(0.120, 0.150, (gradient_t - 0.34) / 0.08)
            } else if gradient_t < 0.62 {
                lerp_f32(0.150, 0.360, (gradient_t - 0.42) / 0.20)
            } else if gradient_t < 0.78 {
                lerp_f32(0.360, 0.560, (gradient_t - 0.62) / 0.16)
            } else {
                lerp_f32(0.580, 0.690, (gradient_t - 0.78) / 0.22)
            };
            let gradient_rgb = spectral_phase_rgb(gradient_hue, 0.98, 1.0);

            // Local hue shimmer (hash-driven)
            let local_phase = avoid_magenta_phase(
                ((col as u32).wrapping_mul(1_664_525).wrapping_add(
                    (row as u32).wrapping_mul(1_013_904_223),
                ) as f32
                    / u32::MAX as f32
                    + u * 0.30
                    - v * 0.18)
                    .rem_euclid(1.0),
                0.18,
            );
            let local_rgb = spectral_phase_rgb(local_phase, 0.96, 1.0);

            // Mix diffraction, gradient, and local shimmer
            let foil_strength = inside * (0.20 + n_dot_l * 0.60 + n_dot_h * 0.20);
            let mix_gradient = 0.40 + foil_strength * 0.30;
            let shimmer_rgb = lerp_rgb(
                local_rgb,
                (diff_r, diff_g, diff_b),
                (0.30 + n_dot_l * 0.40).min(0.70),
            );
            let mut foil_rgb = lerp_rgb(shimmer_rgb, gradient_rgb, mix_gradient.min(0.92));

            // ── Opposite temperature flips ────────────────────────────────────
            let pin_hash = ser_pixel_hash(local_x / 2, local_y / 2);
            let temp_hash = ((pin_hash >> 32) & 0xffff) as f32 / 65535.0;
            let temp_flip = smoothstep(0.64, 0.965, temp_hash) * foil_strength;
            let warm_bias = smoothstep(0.58, 0.90, foil_rgb.0 + foil_rgb.1 * 0.35 - foil_rgb.2 * 0.25);
            let fine_hash = ((pin_hash >> 11) & 0xff) as f32 / 255.0;
            let opposite_hue = if warm_bias > 0.45 {
                0.600 + fine_hash * 0.090
            } else {
                0.025 + fine_hash * 0.090
            };
            let opposite_rgb = spectral_phase_rgb(opposite_hue, 0.98, 1.0);
            foil_rgb = lerp_rgb(foil_rgb, opposite_rgb, temp_flip * 0.68);

            // ── Final composition ──────────────────────────────────────────────
            if foil_strength < 0.005 && spec < 0.005 {
                continue;
            }
            let idx = (y * width + x) as usize;
            let dst = pixels[idx];
            let base_r = dst.red() as f32 / 255.0;
            let base_g = dst.green() as f32 / 255.0;
            let base_b = dst.blue() as f32 / 255.0;

            let darken = 0.42;
            let strength = foil_strength * opacity;
            let result_r = clamp01(base_r * darken + foil_rgb.0 * strength + spec * params.white_gain * opacity);
            let result_g = clamp01(base_g * darken + foil_rgb.1 * strength + spec * params.white_gain * opacity);
            let result_b = clamp01(base_b * darken + foil_rgb.2 * strength + spec * params.white_gain * opacity);

            let out_r = lerp_f32(base_r, result_r, opacity);
            let out_g = lerp_f32(base_g, result_g, opacity);
            let out_b = lerp_f32(base_b, result_b, opacity);
            pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(
                (out_r * 255.0).round() as u8,
                (out_g * 255.0).round() as u8,
                (out_b * 255.0).round() as u8,
                dst.alpha(),
            )
            .unwrap_or(dst);
        }
    }
}

fn optical_micro_facet(
    local_x: u32,
    local_y: u32,
    params: OpticalSerParams,
) -> (f32, f32, f32, f32, i32, i32) {
    let pitch_x = (params.cell_w + params.cell_gap).max(1) as i32;
    let pitch_y = (params.cell_h + params.cell_gap).max(1) as i32;
    let px = local_x as i32;
    let py = local_y as i32;
    let col = px.div_euclid(pitch_x);
    let stagger = if (col & 1) == 1 { pitch_y / 2 } else { 0 };
    let row = (py - stagger).div_euclid(pitch_y);

    let rnd_line = optical_cell_hash(col, row, params.seed + 5555);
    let line_len = if rnd_line > 0.80 {
        (optical_cell_hash(col, row, params.seed + 6666) * 3.0 + 2.0).floor()
    } else {
        1.0
    };

    let cx = (col as f32 + 0.5) * pitch_x as f32;
    let cy = (row as f32 + 0.5) * pitch_y as f32 + stagger as f32;
    let dx = (local_x as f32 - cx) / (params.cell_w.max(1) as f32 * 0.5);
    let dy = (local_y as f32 - cy) / (params.cell_h.max(1) as f32 * 0.5);
    let dy_stretched = dy / line_len.max(1.0);
    let r = (dx.abs().powf(3.0) + dy_stretched.abs().powf(3.0)).powf(1.0 / 3.0);
    let inside = 1.0 - smoothstep(0.85, 1.08, r);

    let tilt_x =
        (optical_cell_hash(col, row, params.seed + 1337) - 0.5) * 2.0 * params.tilt_strength;
    let tilt_y =
        (optical_cell_hash(col, row, params.seed + 8128) - 0.5) * 2.0 * params.tilt_strength;
    let r_dist = (dx * dx + dy * dy).sqrt();
    let slope = params.dome_depth * r_dist.clamp(0.0, 1.0);
    let active = if inside > 0.001 { 1.0 } else { 0.0 };
    let nx = (-dx * slope + tilt_x) * active;
    let ny = (-dy * slope + tilt_y) * active;
    let inv_len = 1.0 / (nx * nx + ny * ny + 1.0).sqrt();
    (nx * inv_len, ny * inv_len, inv_len, inside, col, row)
}

fn optical_energy(x: f32, y: f32, params: OpticalSerParams) -> f32 {
    let macro_noise = optical_fbm(x, y, params.macro_cell, params.seed + 10_001, 5);
    let macro_energy = smoothstep(params.macro_low, params.macro_high, macro_noise);
    let cluster_noise = optical_fbm(x, y, params.cluster_cell, params.seed + 20_003, 4);
    let cluster_energy = smoothstep(params.cluster_low, params.cluster_high, cluster_noise);
    params.energy_floor.max(macro_energy * cluster_energy)
}

fn optical_fbm(x: f32, y: f32, base_cell: f32, seed: u32, octaves: u32) -> f32 {
    let mut total = 0.0;
    let mut amp_sum = 0.0;
    let mut amp = 1.0;
    let mut cell = base_cell.max(1.0);
    for octave in 0..octaves {
        total += optical_smooth_noise(x, y, cell, seed + octave * 7919) * amp;
        amp_sum += amp;
        amp *= 0.5;
        cell = (cell * 0.5).max(1.0);
    }
    if amp_sum > 1e-6 { total / amp_sum } else { 0.0 }
}

fn optical_smooth_noise(x: f32, y: f32, cell: f32, seed: u32) -> f32 {
    let px = x / cell.max(1.0);
    let py = y / cell.max(1.0);
    let ix = px.floor() as i32;
    let iy = py.floor() as i32;
    let fx = px - ix as f32;
    let fy = py - iy as f32;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let a = optical_cell_hash(ix, iy, seed);
    let b = optical_cell_hash(ix + 1, iy, seed);
    let c = optical_cell_hash(ix, iy + 1, seed);
    let d = optical_cell_hash(ix + 1, iy + 1, seed);
    (a * (1.0 - sx) + b * sx) * (1.0 - sy) + (c * (1.0 - sx) + d * sx) * sy
}

#[inline]
fn optical_cell_hash(col: i32, row: i32, seed: u32) -> f32 {
    let mut x = (col as u32).wrapping_mul(1_664_525).wrapping_add(seed)
        ^ (row as u32).wrapping_mul(1_013_904_223);
    x ^= x >> 16;
    x = x.wrapping_mul(2_246_822_519);
    x ^= x >> 13;
    x = x.wrapping_mul(3_266_489_917);
    x ^= x >> 16;
    x as f32 / u32::MAX as f32
}

fn wavelength_lut() -> &'static [(f32, f32, f32)] {
    static LUT: OnceLock<Vec<(f32, f32, f32)>> = OnceLock::new();
    LUT.get_or_init(|| {
        (0..OPTICAL_LUT_SIZE)
            .map(|i| {
                let t = i as f32 / (OPTICAL_LUT_SIZE - 1) as f32;
                wavelength_rgb(OPTICAL_LAM_MIN + (OPTICAL_LAM_MAX - OPTICAL_LAM_MIN) * t)
            })
            .collect()
    })
}

fn optical_lut_lookup(lam_nm: f32) -> (f32, f32, f32) {
    if !(OPTICAL_LAM_MIN..=OPTICAL_LAM_MAX).contains(&lam_nm) {
        return (0.0, 0.0, 0.0);
    }
    let lut = wavelength_lut();
    let idx = (lam_nm - OPTICAL_LAM_MIN) / (OPTICAL_LAM_MAX - OPTICAL_LAM_MIN)
        * (OPTICAL_LUT_SIZE - 1) as f32;
    let i0 = idx.floor() as usize;
    let i1 = (i0 + 1).min(OPTICAL_LUT_SIZE - 1);
    let t = idx - i0 as f32;
    lerp_rgb(lut[i0], lut[i1], t)
}

fn wavelength_rgb(lam: f32) -> (f32, f32, f32) {
    let x = 1.056 * gauss_asym(lam, 599.8, 37.9, 31.0) + 0.362 * gauss_asym(lam, 442.0, 16.0, 26.7)
        - 0.065 * gauss_asym(lam, 501.1, 20.4, 26.2);
    let y = 0.821 * gauss_asym(lam, 568.8, 46.9, 40.5) + 0.286 * gauss_asym(lam, 530.9, 16.3, 31.1);
    let z = 1.217 * gauss_asym(lam, 437.0, 11.8, 36.0) + 0.681 * gauss_asym(lam, 459.0, 26.0, 13.8);
    let mut r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
    let mut g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
    let mut b = 0.0557 * x - 0.2040 * y + 1.0570 * z;
    r = r.max(0.0);
    g = g.max(0.0);
    b = b.max(0.0);
    let peak = r.max(g).max(b);
    if peak > 1e-6 {
        (r / peak, g / peak, b / peak)
    } else {
        (0.0, 0.0, 0.0)
    }
}

#[inline]
fn gauss_asym(x: f32, mu: f32, s1: f32, s2: f32) -> f32 {
    let s = if x < mu { s1 } else { s2 };
    (-0.5 * ((x - mu) / s).powi(2)).exp()
}

#[inline]
fn gauss_line(x: f32, centre: f32, sigma: f32) -> f32 {
    (-0.5 * ((x - centre) / sigma.max(1e-6)).powi(2)).exp()
}

#[inline]
fn vec_norm3(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let inv = 1.0 / (x * x + y * y + z * z).sqrt().max(1e-8);
    (x * inv, y * inv, z * inv)
}
