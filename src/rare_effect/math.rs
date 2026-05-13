//! Shared math utilities for rare/foil effect rendering.

use crate::pixel_ops::hsv_to_rgb;

/// Convert HSV (all 0.0–1.0) + alpha byte to a premultiplied [`tiny_skia::Color`].
#[cfg(test)]
pub(crate) fn hsv_to_color(h: f32, s: f32, v: f32, alpha: f32) -> tiny_skia::Color {
    let (r, g, b) = hsv_to_rgb(h, s, v);
    tiny_skia::Color::from_rgba(r * alpha, g * alpha, b * alpha, alpha)
        .unwrap_or(tiny_skia::Color::TRANSPARENT)
}

pub(crate) fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub(crate) fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

#[inline]
pub(crate) fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[inline]
pub(crate) fn gauss(d: f32, sigma: f32) -> f32 {
    (-d * d / (2.0 * sigma * sigma)).exp()
}

#[inline]
pub(crate) fn ser_sin(v: f32) -> f32 {
    v.sin()
}

#[inline]
pub(crate) fn ser_pixel_hash(x: u32, y: u32) -> u64 {
    let mut h = (x as u64 & 0xFFFF) | ((y as u64 & 0xFFFF) << 16);
    h = ((h ^ (h >> 8)).wrapping_mul(0x006e_ed0e_9da1_f1e3)) & 0xFFFF_FFFF_FFFF_FFFF;
    h ^= h >> 31;
    h = h.wrapping_mul(0x006e_ed0e_9da1_f1e3) & 0xFFFF_FFFF_FFFF_FFFF;
    h ^= h >> 31;
    h
}

#[inline]
pub(crate) fn ser_hash01(x: u32, y: u32) -> f32 {
    (ser_pixel_hash(x, y) & 0xFFFF) as f32 / 65535.0
}

#[inline]
pub(crate) fn value_noise(u: f32, v: f32, scale: f32, seed_x: i32, seed_y: i32) -> f32 {
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
pub(crate) fn prism_peak(phase: f32, power: f32) -> f32 {
    (ser_sin(phase * std::f32::consts::TAU) * 0.5 + 0.5).powf(power)
}

#[inline]
pub(crate) fn ser_independent_speckle_rgb(
    u: f32,
    v: f32,
    phase_a: f32,
    phase_b: f32,
    phase_c: f32,
    warm_bias: f32,
) -> (f32, f32, f32) {
    let mut phase = (phase_a * 0.42
        + phase_b * 0.31
        + phase_c * 0.27
        + ser_sin(u * 38.0 + v * 17.0) * 0.055
        + u * 0.18
        - v * 0.11)
        .rem_euclid(1.0);
    phase = avoid_magenta_phase(phase, warm_bias);
    let r = spectral_phase_rgb(phase, 0.99, 1.0);
    let cool = spectral_phase_rgb(0.57 + phase_b * 0.12, 0.96, 1.0);
    let warm = spectral_phase_rgb(0.045 + phase_c * 0.060, 0.99, 1.0);
    let warm_mix = smoothstep(0.62, 0.94, phase_a) * (0.35 + warm_bias * 0.65);
    let gr = lerp_rgb(cool, warm, warm_mix);
    lerp_rgb(r, gr, 0.38)
}

#[inline]
pub(crate) fn ser_grid_distribution_rgb(
    u: f32,
    v: f32,
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    h_weight: f32,
    v_weight: f32,
    grain: f32,
) -> (f32, f32, f32) {
    let cyan = (0.00, 0.66, 0.96);
    let blue = (0.02, 0.12, 1.00);
    let deep_blue = (0.02, 0.02, 0.92);
    let green = (0.28, 0.92, 0.02);
    let lime = (0.78, 1.00, 0.00);
    let yellow = (1.00, 0.96, 0.00);
    let orange = (1.00, 0.52, 0.00);
    let red_orange = (1.00, 0.18, 0.03);
    let top_h = palette_ramp(
        &[
            (0.0, cyan),
            (x0, green),
            (x0 + (x1 - x0) * 0.42, cyan),
            (x1, blue),
            (1.0, deep_blue),
        ],
        u,
    );
    let bottom_h = palette_ramp(
        &[
            (0.0, yellow),
            (x0 * 0.68, yellow),
            (x0, orange),
            (x0 + (x1 - x0) * 0.52, yellow),
            (x1 - (x1 - x0) * 0.10, lime),
            (x1, green),
            (1.0, cyan),
        ],
        u,
    );
    let left_v = palette_ramp(
        &[
            (0.0, cyan),
            (y0, green),
            (y0 + (y1 - y0) * 0.24, lime),
            (y0 + (y1 - y0) * 0.50, yellow),
            (y0 + (y1 - y0) * 0.78, red_orange),
            (y1, yellow),
            (1.0, yellow),
        ],
        v,
    );
    let right_v = palette_ramp(
        &[
            (0.0, deep_blue),
            (y0, blue),
            (y0 + (y1 - y0) * 0.50, cyan),
            (y0 + (y1 - y0) * 0.70, lime),
            (y1, cyan),
            (1.0, cyan),
        ],
        v,
    );
    let top_near = (-((v - y0) * (v - y0)) / (2.0 * 0.115 * 0.115)).exp();
    let bottom_near = (-((v - y1) * (v - y1)) / (2.0 * 0.115 * 0.115)).exp();
    let left_near = (-((u - x0) * (u - x0)) / (2.0 * 0.105 * 0.105)).exp();
    let right_near = (-((u - x1) * (u - x1)) / (2.0 * 0.105 * 0.105)).exp();
    let h_color = lerp_rgb(
        top_h,
        bottom_h,
        bottom_near / (top_near + bottom_near).max(0.001),
    );
    let v_color = lerp_rgb(
        left_v,
        right_v,
        right_near / (left_near + right_near).max(0.001),
    );
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
pub(crate) fn lerp_rgb(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    let t = t.clamp(0.0, 1.0);
    (
        a.0 * (1.0 - t) + b.0 * t,
        a.1 * (1.0 - t) + b.1 * t,
        a.2 * (1.0 - t) + b.2 * t,
    )
}

#[inline]
pub(crate) fn palette_ramp(stops: &[(f32, (f32, f32, f32))], t: f32) -> (f32, f32, f32) {
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
pub(crate) fn avoid_magenta_phase(phase: f32, warm_bias: f32) -> f32 {
    let h = phase.rem_euclid(1.0);
    if !(0.76..=0.965).contains(&h) {
        return h;
    }
    let t = (h - 0.76) / 0.205;
    if warm_bias > 0.55 {
        return (0.030 + t * 0.070).rem_euclid(1.0);
    }
    let blue_target = 0.610 - t * 0.070;
    let warm_target = 0.045 + t * 0.045;
    let use_warm = smoothstep(0.56, 0.88, t) * (0.38 + warm_bias * 0.62);
    blue_target * (1.0 - use_warm) + warm_target * use_warm
}

#[inline]
pub(crate) fn spectral_phase_rgb(phase: f32, saturation: f32, value: f32) -> (f32, f32, f32) {
    hsv_to_rgb(phase.rem_euclid(1.0), saturation, value)
}

#[inline]
pub(crate) fn screen_channel_float(dst: f32, src: f32, alpha: f32) -> f32 {
    1.0 - (1.0 - dst) * (1.0 - src * alpha)
}

#[inline]
pub(crate) fn hard_unit_mask(radius: f32, dist: f32) -> f32 {
    if dist <= radius { 1.0 } else { 0.0 }
}
