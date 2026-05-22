//! Optical Secret Rare foil: physically-inspired micro-facet diffraction.
//!
//! Core algorithm: staggered micro-facet normal map, two orthogonal diffraction
//! gratings, FBM energy variation, "#" band efficiency, and Blinn-Phong glints.

use tiny_skia::Pixmap;

use super::{math::*, CoverageRect};

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
            let n_dot_l = (nx * light.0 + ny * light.1 + nz * light.2).clamp(0.0, 1.0);
            let n_dot_h = (nx * half_vec.0 + ny * half_vec.1 + nz * half_vec.2).clamp(0.0, 1.0);
            let macro_axis = (u_centered * light.0 + v_centered * light.1) / light_xy_len;
            let macro_light = smoothstep(-0.70, 0.70, macro_axis);
            let energy = optical_energy(xf, yf, params);

            let light_mod = (n_dot_l - 0.5) * params.light_blaze_shift;
            let delta1 = nx * params.tilt_factor + params.blaze1 + light_mod;
            let delta2 = ny * params.tilt_factor + params.blaze2 - light_mod;
            let order = params.diffraction_order.max(1) as f32;
            let lam1 = params.grating_d_nm * delta1.abs() / order;
            let lam2 = params.grating_d_nm * delta2.abs() / order;
            let rgb1 = spectral_lookup(lam1);
            let rgb2 = spectral_lookup(lam2);

            let bw = params.band_width.max(0.002);
            let bb = params.band_base;
            let bw_vary = bw * (0.72 + 0.28 * (1.0 - energy));
            let band_intensity = 0.45 + 0.55 * energy;
            let band_v = gauss(u - params.band_v1, bw_vary).max(gauss(u - params.band_v2, bw_vary));
            let band_h = gauss(v - params.band_h1, bw_vary).max(gauss(v - params.band_h2, bw_vary));
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

            let band_boost1 = (eff1 - bb).clamp(0.0, 1.0);
            let band_boost2 = (eff2 - bb).clamp(0.0, 1.0);
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
            let sheet_base = spectral_lookup(sheet_lam);
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
            let pin = smoothstep(0.84, 0.995, cell_hash01(col, row, params.seed + 4242));
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
            let result_r = (1.0 - (1.0 - dark_r) * (1.0 - diff_r)
                + spec * params.white_gain
                + base_r * reveal)
                .clamp(0.0, 1.0);
            let result_g = (1.0 - (1.0 - dark_g) * (1.0 - diff_g)
                + spec * params.white_gain
                + base_g * reveal)
                .clamp(0.0, 1.0);
            let result_b = (1.0 - (1.0 - dark_b) * (1.0 - diff_b)
                + spec * params.white_gain
                + base_b * reveal)
                .clamp(0.0, 1.0);

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
            let n_dot_l = (nx * light.0 + ny * light.1 + nz * light.2).clamp(0.0, 1.0);
            let n_dot_h = (nx * half_vec.0 + ny * half_vec.1 + nz * half_vec.2).clamp(0.0, 1.0);

            let light_mod = (n_dot_l - 0.5) * params.light_blaze_shift;
            let delta1 = nx * params.tilt_factor + params.blaze1 + light_mod;
            let delta2 = ny * params.tilt_factor + params.blaze2 - light_mod;
            let order = params.diffraction_order.max(1) as f32;
            let lam1 = params.grating_d_nm * delta1.abs() / order;
            let lam2 = params.grating_d_nm * delta2.abs() / order;
            let rgb1 = spectral_lookup(lam1);
            let rgb2 = spectral_lookup(lam2);

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
            let pin = smoothstep(0.84, 0.995, cell_hash01(col, row, params.seed + 4242));
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
                ((col as u32)
                    .wrapping_mul(1_664_525)
                    .wrapping_add((row as u32).wrapping_mul(1_013_904_223))
                    as f32
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
            let warm_bias = smoothstep(
                0.58,
                0.90,
                foil_rgb.0 + foil_rgb.1 * 0.35 - foil_rgb.2 * 0.25,
            );
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
            let result_r =
                (base_r * darken + foil_rgb.0 * strength + spec * params.white_gain * opacity)
                    .clamp(0.0, 1.0);
            let result_g =
                (base_g * darken + foil_rgb.1 * strength + spec * params.white_gain * opacity)
                    .clamp(0.0, 1.0);
            let result_b =
                (base_b * darken + foil_rgb.2 * strength + spec * params.white_gain * opacity)
                    .clamp(0.0, 1.0);

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

    let rnd_line = cell_hash01(col, row, params.seed + 5555);
    let line_len = if rnd_line > 0.80 {
        (cell_hash01(col, row, params.seed + 6666) * 3.0 + 2.0).floor()
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
        (cell_hash01(col, row, params.seed + 1337) - 0.5) * 2.0 * params.tilt_strength;
    let tilt_y =
        (cell_hash01(col, row, params.seed + 8128) - 0.5) * 2.0 * params.tilt_strength;
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
    if amp_sum > 1e-6 {
        total / amp_sum
    } else {
        0.0
    }
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
    let a = cell_hash01(ix, iy, seed);
    let b = cell_hash01(ix + 1, iy, seed);
    let c = cell_hash01(ix, iy + 1, seed);
    let d = cell_hash01(ix + 1, iy + 1, seed);
    (a * (1.0 - sx) + b * sx) * (1.0 - sy) + (c * (1.0 - sx) + d * sx) * sy
}


#[inline]
fn vec_norm3(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let inv = 1.0 / (x * x + y * y + z * z).sqrt().max(1e-8);
    (x * inv, y * inv, z * inv)
}

// ═══════════════════════════════════════════════════════════════════════════════
// SCR (Secret Collector's Rare) - diagonal dotted micro-facet diffraction
// ═══════════════════════════════════════════════════════════════════════════════

/// Parameters for the SCR optical foil.
#[derive(Debug, Clone, Copy)]
struct OpticalScrParams {
    seed: u32,
    /// Dot lattice pitch in screen-space x/y coordinates (pixels).
    dot_pitch: f32,
    dot_radius: f32,
    dot_softness: f32,
    dot_jitter: f32,
    tilt_strength: f32,
    dome_depth: f32,
    /// Grating period in nm (determines spectral spread).
    grating_d_nm: f32,
    /// Diffraction order.
    diffraction_order: u32,
    /// Base blaze angle offset (determines centre wavelength).
    blaze_offset: f32,
    /// How much the local dot normal shifts the grating phase.
    tilt_factor: f32,
    angle_spread: f32,
    /// Spacing of bright diagonal dot rows.
    line_pitch: f32,
    line_base: f32,
    line_gain: f32,
    line_sharpness: f32,
    /// Light direction.
    light_x: f32,
    light_y: f32,
    light_z: f32,
    /// Specular shininess.
    shininess: f32,
    white_gain: f32,
    darken: f32,
    foil_gain: f32,
    sparkle_gain: f32,
    /// FBM energy field parameters.
    macro_cell: f32,
    cluster_cell: f32,
    energy_floor: f32,
    macro_low: f32,
    macro_high: f32,
    cluster_low: f32,
    cluster_high: f32,
}

const OPTICAL_SCR_PARAMS: OpticalScrParams = OpticalScrParams {
    seed: 77,
    dot_pitch: 5.0,
    dot_radius: 2.1,
    dot_softness: 0.47,
    dot_jitter: 0.10,
    tilt_strength: 0.50,
    dome_depth: 0.38,
    grating_d_nm: 760.0,
    diffraction_order: 1,
    blaze_offset: 0.62,
    tilt_factor: 0.12,
    angle_spread: 0.58,
    line_pitch: 14.0,
    line_base: 0.32,
    line_gain: 1.68,
    line_sharpness: 8.6,
    light_x: 0.42,
    light_y: -0.34,
    light_z: 1.00,
    shininess: 54.0,
    white_gain: 1.04,
    darken: 0.40,
    foil_gain: 4.20,
    sparkle_gain: 1.34,
    macro_cell: 145.0,
    cluster_cell: 18.0,
    energy_floor: 0.26,
    macro_low: 0.12,
    macro_high: 0.86,
    cluster_low: 0.24,
    cluster_high: 0.82,
};

/// SCR foil for the illustration area.
///
/// Physical model: only small dots are printed. They live on a dense
/// orthogonal lattice, and each dot is a tiny domed grating with the same
/// lower-left to upper-right orientation. The visible diagonal streaks are
/// bright diagonal groups of dots reflecting together, not continuous stroke
/// geometry.
pub(crate) fn draw_optical_scr(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= f32::EPSILON {
        return;
    }

    let params = OPTICAL_SCR_PARAMS;
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

    // Grating direction: dots are arranged along ↗ rows, so the grating normal
    // is the perpendicular ↘ axis. Colour is therefore coherent along each
    // diagonal row.
    let inv_sqrt2: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let grating_nx = inv_sqrt2;
    let grating_ny = inv_sqrt2;

    for y in y_start..y_end {
        for x in x_start..x_end {
            let local_x = x.saturating_sub(rect.x);
            let local_y = y.saturating_sub(rect.y);
            let xf = local_x as f32;
            let yf = local_y as f32;
            let u = xf / norm_w;
            let v = yf / norm_h;

            let (inside, dome_bright, nx, ny, nz, col, row, line_gate, dot_hash) =
                scr_dot_facet(local_x, local_y, params);

            let u_centered = u - 0.5;
            let v_centered = v - 0.5;
            let n_dot_l = (nx * light.0 + ny * light.1 + nz * light.2).clamp(0.0, 1.0);
            let n_dot_h = (nx * half_vec.0 + ny * half_vec.1 + nz * half_vec.2).clamp(0.0, 1.0);
            let normal_phase = (nx * grating_nx + ny * grating_ny) * params.tilt_factor;
            let sin_theta = (u_centered * grating_nx + v_centered * grating_ny)
                * params.angle_spread
                + params.blaze_offset
                + normal_phase;
            let order = params.diffraction_order.max(1) as f32;
            let lam = params.grating_d_nm * sin_theta.abs() / order;
            let diff_rgb = spectral_lookup(lam);
            let warm_near_light =
                smoothstep(575.0, 635.0, lam) * (1.0 - smoothstep(665.0, 725.0, lam));

            let lam2 = params.grating_d_nm * (sin_theta * 0.56 + 0.14).abs() / order;
            let diff_rgb2 = spectral_lookup(lam2);

            let local_phase = avoid_magenta_phase(
                (dot_hash * 0.18 + u * 0.11 - v * 0.08).rem_euclid(1.0),
                line_gate * 0.20,
            );
            let local_rgb = spectral_phase_rgb(local_phase, 0.95, 1.0);
            let raw_combined_r = diff_rgb.0 * 0.72 + diff_rgb2.0 * 0.20 + local_rgb.0 * 0.08;
            let raw_combined_g = diff_rgb.1 * 0.72 + diff_rgb2.1 * 0.20 + local_rgb.1 * 0.08;
            let raw_combined_b = diff_rgb.2 * 0.72 + diff_rgb2.2 * 0.20 + local_rgb.2 * 0.08;
            let red_dominance = smoothstep(
                0.10,
                0.54,
                raw_combined_r - raw_combined_g.max(raw_combined_b),
            );
            let orange_lift = red_dominance * warm_near_light * 0.22;
            let combined_r = raw_combined_r * (1.0 - red_dominance * warm_near_light * 0.18);
            let combined_g = (raw_combined_g + orange_lift).min(1.0);
            let combined_b = raw_combined_b * (1.0 - red_dominance * warm_near_light * 0.10);

            let energy = scr_energy(xf, yf, params);
            let row_hash = cell_hash01(col, row, params.seed + 90_017);
            let row_gate = 0.62 + 0.38 * smoothstep(0.10, 0.92, row_hash);
            let source_dx = u - 0.78;
            let source_dy = v - 0.18;
            let near_source = (1.0
                - ((source_dx * source_dx + source_dy * source_dy).sqrt() / 0.68))
                .clamp(0.0, 1.0)
                .powf(1.85);
            let warm_energy = smoothstep(0.22, 0.92, warm_near_light * 0.78 + near_source * 0.46);
            let cool_near_light =
                smoothstep(420.0, 485.0, lam) * (1.0 - smoothstep(540.0, 610.0, lam));
            let cool_band_hint = (prism_peak((u * 0.74 + v * 0.92 + 0.18) * 2.4, 3.2) * 0.62
                + prism_peak((u * 1.18 - v * 0.36 + 0.41) * 2.0, 2.4) * 0.38)
                .min(1.0);
            let cool_energy =
                smoothstep(0.16, 0.82, cool_near_light * 0.72 + cool_band_hint * 0.36);
            let cold_far = (1.0 - warm_near_light) * (1.0 - near_source * 0.45);
            let diag_perp = (xf + yf) * inv_sqrt2;
            let diag_along = (xf - yf) * inv_sqrt2;
            let sheet_band = (prism_peak(diag_perp / 38.0 + diag_along * 0.008, 4.4) * 0.78
                + prism_peak(diag_perp / 76.0 + 0.31, 3.1) * 0.22)
                .min(1.0);
            let light_energy = 0.28 + 0.72 * n_dot_l;
            let connected_rows = (params.line_base
                + params.line_gain * line_gate * (0.72 + warm_energy * 0.58))
                .min(2.08);
            let chroma_energy = (warm_energy * 0.72 + cool_energy * 0.56).min(1.0);
            let combined_energy = energy
                * inside
                * dome_bright
                * light_energy
                * connected_rows
                * row_gate
                * (0.82 + warm_energy * 0.70 + cool_energy * 0.52)
                * (1.0 - cold_far * 0.08);

            let scale = combined_energy
                * params.foil_gain
                * (0.88 + warm_energy * 0.52 + cool_energy * 0.40);
            let foil_r = 1.0 - (-combined_r * scale).exp();
            let foil_g = 1.0 - (-combined_g * scale).exp();
            let foil_b = 1.0 - (-combined_b * scale).exp();

            let mut spec = n_dot_h.powf(params.shininess)
                * combined_energy
                * (0.28 + warm_energy * 0.38 + cool_energy * 0.24);
            let sparkle_hash = ser_pixel_hash(local_x / 2 + 13, local_y / 2 + 29);
            let sparkle = smoothstep(0.91, 0.999, (sparkle_hash & 0xFFFF) as f32 / 65535.0);
            spec += sparkle
                * inside
                * params.sparkle_gain
                * (0.26 + line_gate * 0.58 + warm_energy * 0.46 + cool_energy * 0.34)
                * row_gate;

            // ── Composite onto base image ────────────────────────────────────
            let idx = (y * width + x) as usize;
            let dst = pixels[idx];
            let base_r = dst.red() as f32 / 255.0;
            let base_g = dst.green() as f32 / 255.0;
            let base_b = dst.blue() as f32 / 255.0;
            let sheet_energy = (0.095 + energy * 0.24)
                * (0.44 + sheet_band * 0.72)
                * (0.64 + near_source * 0.52 + warm_energy * 0.38 + cool_energy * 0.44)
                * (1.0 - cold_far * 0.10);
            let sheet_alpha = (sheet_energy * opacity).min(0.52);
            let sheet_rgb = lerp_rgb(
                (diff_rgb.0, diff_rgb.1, diff_rgb.2),
                (combined_r, combined_g, combined_b),
                0.38 + chroma_energy * 0.20,
            );
            let metal_keep = 1.0 - sheet_alpha * (0.18 + chroma_energy * 0.08);
            let metal_r = screen_channel_float(base_r * metal_keep, sheet_rgb.0, sheet_alpha);
            let metal_g = screen_channel_float(base_g * metal_keep, sheet_rgb.1, sheet_alpha);
            let metal_b = screen_channel_float(base_b * metal_keep, sheet_rgb.2, sheet_alpha);

            let dot_density = (inside * (0.42 + line_gate * 0.70 + chroma_energy * 0.44)).min(1.0);
            let darken = lerp_f32(0.88, params.darken, dot_density);
            let dark_r = metal_r * darken;
            let dark_g = metal_g * darken;
            let dark_b = metal_b * darken;
            let result_r =
                (1.0 - (1.0 - dark_r) * (1.0 - foil_r) + spec * params.white_gain).clamp(0.0, 1.0);
            let result_g =
                (1.0 - (1.0 - dark_g) * (1.0 - foil_g) + spec * params.white_gain).clamp(0.0, 1.0);
            let result_b =
                (1.0 - (1.0 - dark_b) * (1.0 - foil_b) + spec * params.white_gain).clamp(0.0, 1.0);

            let dot_alpha =
                opacity * (inside * (0.64 + line_gate * 0.40 + chroma_energy * 0.34)).min(1.0);
            let out_r = lerp_f32(metal_r, result_r, dot_alpha);
            let out_g = lerp_f32(metal_g, result_g, dot_alpha);
            let out_b = lerp_f32(metal_b, result_b, dot_alpha);
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

/// SCR foil for small masked regions (attribute icons, level/rank stars, link
/// arrows). It intentionally uses the same dotted optical model as the
/// illustration area; the renderer applies the asset mask after drawing, so the
/// only difference is the covered shape.
pub(crate) fn draw_optical_scr_simple(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    draw_optical_scr(target, rect, opacity);
}

/// SCR dot facet: a small circular dot in an orthogonal lattice. The function
/// returns only dot coverage; any diagonal streak must come from lighting
/// diagonal groups of dots, not from stretched geometry.
fn scr_dot_facet(
    local_x: u32,
    local_y: u32,
    params: OpticalScrParams,
) -> (f32, f32, f32, f32, f32, i32, i32, f32, f32) {
    let xf = local_x as f32;
    let yf = local_y as f32;
    let pitch = params.dot_pitch.max(1.0);
    let base_col = (xf / pitch).floor() as i32;
    let base_row = (yf / pitch).floor() as i32;

    let mut best_inside = 0.0_f32;
    let mut best_dx = 0.0_f32;
    let mut best_dy = 0.0_f32;
    let mut best_radius = params.dot_radius;
    let mut best_col = base_col;
    let mut best_row = base_row;
    let mut best_hash = 0.0_f32;
    let mut best_center_x = 0.0_f32;
    let mut best_center_y = 0.0_f32;

    for row in (base_row - 1)..=(base_row + 1) {
        for col in (base_col - 1)..=(base_col + 1) {
            let hash_a = cell_hash01(col, row, params.seed + 10_003);
            let hash_b = cell_hash01(col, row, params.seed + 20_011);
            let hash_c = cell_hash01(col, row, params.seed + 30_019);
            let jitter_x = (hash_a - 0.5) * params.dot_jitter;
            let jitter_y = (hash_b - 0.5) * params.dot_jitter;
            let center_x = (col as f32 + 0.5 + jitter_x) * pitch;
            let center_y = (row as f32 + 0.5 + jitter_y) * pitch;
            let dx = xf - center_x;
            let dy = yf - center_y;
            let radius = params.dot_radius * (0.86 + hash_c * 0.26);
            let dist = (dx * dx + dy * dy).sqrt();
            let inside = 1.0 - smoothstep(radius, radius + params.dot_softness, dist);
            if inside > best_inside {
                best_inside = inside;
                best_dx = dx;
                best_dy = dy;
                best_radius = radius;
                best_col = col;
                best_row = row;
                best_hash = hash_c;
                best_center_x = center_x;
                best_center_y = center_y;
            }
        }
    }

    if best_inside <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0, 1.0, best_col, best_row, 0.0, best_hash);
    }

    let r = (best_dx * best_dx + best_dy * best_dy).sqrt() / best_radius.max(1e-3);
    let dome_bright = 0.62 + 0.38 * (1.0 - r.min(1.0)).powf(0.72);

    let slope_x = best_dx / best_radius.max(1e-3) * params.dome_depth;
    let slope_y = best_dy / best_radius.max(1e-3) * params.dome_depth;
    let tilt_x =
        (cell_hash01(best_col, best_row, params.seed + 40_031) - 0.5) * params.tilt_strength;
    let tilt_y =
        (cell_hash01(best_col, best_row, params.seed + 50_047) - 0.5) * params.tilt_strength;
    let (nx, ny, nz) = vec_norm3(-(slope_x + tilt_x), -(slope_y + tilt_y), 1.0);

    let row_phase_hash = cell_hash01(best_col, best_row, params.seed + 60_059);
    let inv_sqrt2: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let diag_perp = (best_center_x + best_center_y) * inv_sqrt2;
    let diag_along = (best_center_x - best_center_y) * inv_sqrt2;
    let line_phase =
        diag_perp / params.line_pitch.max(1.0) + diag_along * 0.004 + row_phase_hash * 0.055;
    let primary = prism_peak(line_phase, params.line_sharpness);
    let secondary = prism_peak(line_phase * 0.53 + 0.37, params.line_sharpness * 0.78);
    let line_gate = (primary * 0.92 + secondary * 0.36).min(1.0);

    (
        best_inside,
        dome_bright,
        nx,
        ny,
        nz,
        best_col,
        best_row,
        line_gate,
        best_hash,
    )
}

/// FBM energy field for SCR (same structure as Ser but different seed).
fn scr_energy(x: f32, y: f32, params: OpticalScrParams) -> f32 {
    let macro_noise = optical_fbm(x, y, params.macro_cell, params.seed + 30_001, 5);
    let macro_energy = smoothstep(params.macro_low, params.macro_high, macro_noise);
    let cluster_noise = optical_fbm(x, y, params.cluster_cell, params.seed + 40_003, 4);
    let cluster_energy = smoothstep(params.cluster_low, params.cluster_high, cluster_noise);
    params.energy_floor.max(macro_energy * cluster_energy)
}
