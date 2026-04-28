//! Shared pixel-level primitives for effect rendering.
//!
//! Centralises functions that were previously duplicated between
//! `rare_effect` and `renderer`:
//!
//! | Old name (rare_effect)  | Old name (renderer)    | Unified name        |
//! |-------------------------|------------------------|---------------------|
//! | `screen_over`           | `screen_pixel`         | `screen_pixel`      |
//! | `hsv_to_rgb`            | `hsv_to_rgb_local`     | `hsv_to_rgb`        |
//! | `hash2`                 | `hash_for_effect`      | `pixel_hash`        |

use tiny_skia::PremultipliedColorU8;

// ─────────────────────────────────────────────────────────────────────────────
// Blending
// ─────────────────────────────────────────────────────────────────────────────

/// Screen-blend `(src_r, src_g, src_b)` at `alpha` over `dst`.
///
/// Equivalent to Photoshop/CSS Screen mode at the given alpha.
/// Returns `dst` unchanged when `alpha == 0`.
#[inline]
pub(crate) fn screen_pixel(
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
// Colour conversion
// ─────────────────────────────────────────────────────────────────────────────

/// HSV (all channels 0.0–1.0) → (r, g, b) floats (0.0–1.0).
#[inline]
pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
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

// ─────────────────────────────────────────────────────────────────────────────
// Hashing
// ─────────────────────────────────────────────────────────────────────────────

/// Deterministic 2D integer hash — no external deps.
///
/// Returns a `u32` whose bits are pseudo-random given `(x, y)`.
/// Suitable for procedural noise, sparkle patterns, and similar per-pixel
/// effects.
#[inline]
pub(crate) fn pixel_hash(x: u32, y: u32) -> u32 {
    let mut h = x
        .wrapping_mul(2246822519)
        .wrapping_add(y.wrapping_mul(3266489917));
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    h
}
