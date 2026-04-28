use tiny_skia::Pixmap;

use crate::{
    pixel_ops::{hsv_to_rgb, pixel_hash, screen_pixel},
    rare_effect::CoverageRect,
};

// ── Pixel-level visual effects ────────────────────────────────────────────────

pub(super) fn draw_gold_wash(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let pixels = target.pixels_mut();

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            let local_x = x.saturating_sub(rect.x) as f32;
            let local_y = y.saturating_sub(rect.y) as f32;
            let shimmer = ((local_x * 0.035 - local_y * 0.018).sin() * 0.5 + 0.5).powf(1.6);
            let noise = (pixel_hash(x, y) & 0xff) as f32 / 255.0;
            let alpha = (opacity * (0.72 + shimmer * 0.20 + noise * 0.08)).clamp(0.0, 1.0);
            let gold_r = (206.0 + shimmer * 38.0) as u8;
            let gold_g = (146.0 + shimmer * 70.0) as u8;
            let gold_b = (30.0 + shimmer * 28.0) as u8;

            let idx = (y * width + x) as usize;
            let dst = pixels[idx];
            let mix = |d: u8, s: u8| -> u8 {
                (d as f32 * (1.0 - alpha) + s as f32 * alpha).round() as u8
            };
            pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(
                mix(dst.red(), gold_r),
                mix(dst.green(), gold_g),
                mix(dst.blue(), gold_b),
                dst.alpha(),
            )
            .unwrap_or(dst);
        }
    }
}

pub(super) fn draw_frosted_foil(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let pixels = target.pixels_mut();

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            let xf = x as f32;
            let yf = y as f32;
            let h0 = pixel_hash(x, y);
            let h1 = pixel_hash(x / 7, y / 7);
            let h2 = pixel_hash(x / 15, y / 15);
            let h3 = pixel_hash(x / 31, y / 31);
            let pin = (h0 & 0xff) as f32 / 255.0;
            let coarse = (h1 & 0xff) as f32 / 255.0;
            let pebble = (h2 & 0xff) as f32 / 255.0;
            let cloud = (h3 & 0xff) as f32 / 255.0;
            let sparkle = if h0 & 0x7ff < 18 || h1 & 0x1ff < 10 { 1.0 } else { 0.0 };
            let scratch = if ((x + y * 3) % 53 < 3) && (h1 & 0x3f < 8) { 1.0 } else { 0.0 };
            let diagonal_band = ((xf * 0.0038 - yf * 0.0025).sin() * 0.5 + 0.5).powf(0.70);
            let rainbow_sweep = ((xf * 0.0017 + yf * 0.0007).sin() * 0.5 + 0.5).powf(0.82);
            let grit_edge = ((xf * 0.22 - yf * 0.14).sin() * 0.5 + 0.5).powf(2.4);
            let cluster = if coarse > 0.68 {
                ((coarse - 0.68) / 0.32).powf(0.62)
            } else {
                0.0
            };
            let pebble_high = if pebble > 0.60 {
                ((pebble - 0.60) / 0.40).powf(0.82)
            } else {
                0.0
            };
            let matte = (cluster * 0.38
                + pebble_high * 0.20
                + pin * 0.06
                + cloud * 0.06
                + grit_edge * 0.08)
                .clamp(0.0, 1.0);
            let strength = (matte * 0.58
                + diagonal_band * 0.34
                + rainbow_sweep * 0.25
                + sparkle * 0.30
                + scratch * 0.14)
                * opacity;
            let hue =
                (xf * 0.0011 - yf * 0.0016 + diagonal_band * 0.42 + cloud * 0.08).rem_euclid(1.0);
            let (r, g, b) = hsv_to_rgb(hue, 0.94, 1.0);
            let silver = (0.04 + matte * 0.20 + sparkle * 0.14).min(0.42);
            let src_r = ((r * (1.0 - silver) + silver) * 255.0).round() as u8;
            let src_g = ((g * (1.0 - silver) + silver) * 255.0).round() as u8;
            let src_b = ((b * (1.0 - silver) + silver) * 255.0).round() as u8;
            let alpha = (strength.clamp(0.0, 1.0) * 224.0).round() as u8;

            let idx = (y * width + x) as usize;
            pixels[idx] = screen_pixel(pixels[idx], src_r, src_g, src_b, alpha);
        }
    }
}

pub(super) fn draw_concentric_engrave(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let cx = rect.x as f32 + rect.w as f32 * 0.5;
    let cy = rect.y as f32 + rect.h as f32 * 0.5;
    let pixels = target.pixels_mut();

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);
            let rings = ((dist * 0.36).sin().abs()).powf(9.0);
            let radial = ((angle * 18.0 + dist * 0.04).sin().abs()).powf(8.0);
            let strength = (rings * 0.78 + radial * 0.22) * opacity;
            if strength < 0.015 {
                continue;
            }
            let hue = (0.11 + dist * 0.006 + angle * 0.04).rem_euclid(1.0);
            let (r, g, b) = hsv_to_rgb(hue, 0.9, 1.0);
            let alpha = (strength.clamp(0.0, 1.0) * 220.0).round() as u8;
            let idx = (y * width + x) as usize;
            pixels[idx] = screen_pixel(
                pixels[idx],
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8,
                alpha,
            );
        }
    }
}

pub(super) fn draw_relief_engrave(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);

    // Pre-compute luma for the rect (+ SAMPLE_RADIUS border).
    // A wider radius lets us compute local variance for better flat-area
    // detection — real UTR engraving only appears in genuinely smooth regions.
    const SAMPLE_RADIUS: u32 = 6;
    let luma_x0 = rect.x.saturating_sub(SAMPLE_RADIUS);
    let luma_y0 = rect.y.saturating_sub(SAMPLE_RADIUS);
    let luma_x1 = x_end.saturating_add(SAMPLE_RADIUS).min(width);
    let luma_y1 = y_end.saturating_add(SAMPLE_RADIUS).min(height);
    let luma_w = luma_x1 - luma_x0;

    let mut luma: Vec<f32> =
        Vec::with_capacity(((luma_x1 - luma_x0) * (luma_y1 - luma_y0)) as usize);
    {
        let src = target.pixels();
        for ly in luma_y0..luma_y1 {
            for lx in luma_x0..luma_x1 {
                let p = src[(ly * width + lx) as usize];
                luma.push(
                    (p.red() as f32 * 0.299
                        + p.green() as f32 * 0.587
                        + p.blue() as f32 * 0.114)
                        / 255.0,
                );
            }
        }
    }

    // Sample luma at absolute card coordinates (clamped to the luma buffer).
    let sample = |ax: i32, ay: i32| -> f32 {
        let cx = ax.clamp(luma_x0 as i32, luma_x1 as i32 - 1) as u32;
        let cy = ay.clamp(luma_y0 as i32, luma_y1 as i32 - 1) as u32;
        luma[((cy - luma_y0) * luma_w + (cx - luma_x0)) as usize]
    };

    // ── Pre-compute colour-variance map ──────────────────────────────────
    // Real UTR engraving is absent in areas with rich colour variation.
    let var_radius: u32 = 2; // 5×5 window
    let var_x0 = rect.x.saturating_sub(var_radius);
    let var_y0 = rect.y.saturating_sub(var_radius);
    let var_x1 = x_end.saturating_add(var_radius).min(width);
    let var_y1 = y_end.saturating_add(var_radius).min(height);
    let var_w = var_x1 - var_x0;

    let color_var: Vec<f32> = {
        let src = target.pixels();
        let mut buf = Vec::with_capacity(((var_x1 - var_x0) * (var_y1 - var_y0)) as usize);
        for vy in var_y0..var_y1 {
            for vx in var_x0..var_x1 {
                let mut sum_r = 0.0_f32;
                let mut sum_g = 0.0_f32;
                let mut sum_b = 0.0_f32;
                let mut sum_r2 = 0.0_f32;
                let mut sum_g2 = 0.0_f32;
                let mut sum_b2 = 0.0_f32;
                let mut count = 0.0_f32;
                let ky0 = vy.saturating_sub(var_radius).max(var_y0);
                let ky1 = (vy + var_radius + 1).min(var_y1);
                let kx0 = vx.saturating_sub(var_radius).max(var_x0);
                let kx1 = (vx + var_radius + 1).min(var_x1);
                for ky in ky0..ky1 {
                    for kx in kx0..kx1 {
                        let p = src[(ky * width + kx) as usize];
                        let r = p.red() as f32 / 255.0;
                        let g = p.green() as f32 / 255.0;
                        let b = p.blue() as f32 / 255.0;
                        sum_r += r;
                        sum_g += g;
                        sum_b += b;
                        sum_r2 += r * r;
                        sum_g2 += g * g;
                        sum_b2 += b * b;
                        count += 1.0;
                    }
                }
                let var = if count > 1.0 {
                    let vr = (sum_r2 / count - (sum_r / count).powi(2)).max(0.0);
                    let vg = (sum_g2 / count - (sum_g / count).powi(2)).max(0.0);
                    let vb = (sum_b2 / count - (sum_b / count).powi(2)).max(0.0);
                    vr + vg + vb
                } else {
                    0.0
                };
                buf.push(var);
            }
        }
        buf
    };

    let sample_var = |ax: u32, ay: u32| -> f32 {
        let cx = ax.clamp(var_x0, var_x1 - 1);
        let cy = ay.clamp(var_y0, var_y1 - 1);
        color_var[((cy - var_y0) * var_w + (cx - var_x0)) as usize]
    };

    let pixels = target.pixels_mut();

    // ── Primary diagonal angle for the parallel scratch lines ────────────
    // Real UTR uses a dominant ~40-50° angle with slight local wobble.
    const PRIMARY_ANGLE: f32 = 0.74; // ~42° in radians
    const SECONDARY_ANGLE: f32 = 0.52; // ~30° — subtle cross-set
    let cos_p = PRIMARY_ANGLE.cos();
    let sin_p = PRIMARY_ANGLE.sin();
    let cos_s = SECONDARY_ANGLE.cos();
    let sin_s = SECONDARY_ANGLE.sin();

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            if x == 0 || y == 0 || x + 1 >= width || y + 1 >= height {
                continue;
            }

            let xf = x.saturating_sub(rect.x) as f32;
            let yf = y.saturating_sub(rect.y) as f32;

            // ── Flat-area gating ─────────────────────────────────────────
            let tl = sample(x as i32 - 1, y as i32 - 1);
            let tc = sample(x as i32, y as i32 - 1);
            let tr = sample(x as i32 + 1, y as i32 - 1);
            let ml = sample(x as i32 - 1, y as i32);
            let mc = sample(x as i32, y as i32);
            let mr = sample(x as i32 + 1, y as i32);
            let bl = sample(x as i32 - 1, y as i32 + 1);
            let bc = sample(x as i32, y as i32 + 1);
            let br = sample(x as i32 + 1, y as i32 + 1);

            // Sobel edge magnitude
            let sobel_x = (tr + 2.0 * mr + br) - (tl + 2.0 * ml + bl);
            let sobel_y = (bl + 2.0 * bc + br) - (tl + 2.0 * tc + tr);
            let edge = (sobel_x * sobel_x + sobel_y * sobel_y).sqrt().min(1.0);

            // Local luma deviation from neighbourhood mean
            let avg = (tl + tc + tr + ml + mr + bl + bc + br) / 8.0;
            let detail = (mc - avg).abs().min(1.0);

            // Colour variance gate — suppress in colourful regions
            let cvar = sample_var(x, y);
            let color_gate = (1.0 - (cvar * 28.0).clamp(0.0, 1.0)).powf(1.4);

            // Combined flat-area mask: must pass edge, detail, AND colour gates
            let edge_gate = (1.0 - (edge * 6.0).clamp(0.0, 1.0)).powf(2.0);
            let detail_gate = (1.0 - (detail * 14.0).clamp(0.0, 1.0)).powf(1.5);
            let flat_mask = edge_gate * detail_gate * color_gate;
            if flat_mask < 0.02 {
                continue;
            }

            // ── Parallel diagonal scratch lines ──────────────────────────
            let luma_wobble = mc * 1.6;

            // Primary line family — dominant scratches at ~42°
            let proj_p = xf * cos_p + yf * sin_p;
            let line_p1 = ((proj_p * 0.38 + luma_wobble).sin().abs()).powf(18.0);
            let line_p2 = ((proj_p * 0.72 + luma_wobble * 0.7).sin().abs()).powf(22.0);
            let line_p3 = ((proj_p * 1.45 + luma_wobble * 0.4).sin().abs()).powf(28.0);

            // Secondary line family — finer cross-scratches at ~30°
            let proj_s = xf * cos_s + yf * sin_s;
            let line_s1 = ((proj_s * 0.55 + luma_wobble * 0.5).sin().abs()).powf(24.0);
            let line_s2 = ((proj_s * 1.10 + luma_wobble * 0.3).sin().abs()).powf(30.0);

            // Contour lines that follow luma iso-levels (subtle)
            let contour = ((mc * 32.0 + xf * 0.004 - yf * 0.006).sin().abs()).powf(26.0);

            // Combine: primary dominates, secondary adds texture, contour adds depth
            let line = line_p1 * 0.32
                + line_p2 * 0.22
                + line_p3 * 0.10
                + line_s1 * 0.16
                + line_s2 * 0.08
                + contour * 0.12;

            let strength = line * flat_mask * opacity;
            if strength < 0.008 {
                continue;
            }

            // ── Colour: silvery metallic with very subtle hue shift ──────
            let hue = (0.08 + xf * 0.0005 + yf * 0.0007 + mc * 0.03).rem_euclid(1.0);
            let (r, g, b) = hsv_to_rgb(hue, 0.18, 1.0);

            let idx = (y * width + x) as usize;
            // Slight darken to simulate the engraved groove shadow
            pixels[idx] = darken_pixel(pixels[idx], (strength * 0.12).clamp(0.0, 0.14));
            // Screen-blend the metallic highlight
            let alpha = (strength.clamp(0.0, 1.0) * 145.0).round() as u8;
            pixels[idx] = screen_pixel(
                pixels[idx],
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8,
                alpha,
            );
        }
    }
}

fn darken_pixel(
    dst: tiny_skia::PremultipliedColorU8,
    amount: f32,
) -> tiny_skia::PremultipliedColorU8 {
    let keep = (1.0 - amount.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    tiny_skia::PremultipliedColorU8::from_rgba(
        (dst.red() as f32 * keep).round() as u8,
        (dst.green() as f32 * keep).round() as u8,
        (dst.blue() as f32 * keep).round() as u8,
        dst.alpha(),
    )
    .unwrap_or(dst)
}
