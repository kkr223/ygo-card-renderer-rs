//! Full-card diamond / sparkle foil effect ("全钻膜 / 爆闪膜").
//!
//! ## Physical description
//! Same 7×7 px staggered micro-facet lattice as the SER optical foil,
//! but with **horizontal** elongation instead of vertical.  Each cell is a
//! tiny domed micro-facet whose long axis is horizontal ("短横线"), unlike
//! SER's vertical "短竖线".  The "#" band pattern further gates which cells
//! are active.
//!
//! ## Algorithm
//! - Cell grid: `cell_w = 7`, `cell_h = 7`, `cell_gap = 2` → pitch = 9 px,
//!   odd columns offset vertically by 4 px.
//! - Micro-facet: same dome + tilt + hash-driven elongation as
//!   `optical_micro_facet`, but `dx` is divided by `line_len` instead of
//!   `dy` so the facet stretches **horizontally**.
//! - "#" band gate: only cells whose centre falls within `band_presence` are
//!   rendered (same band positions as SER: u = 0.30, 0.68; v = 0.32, 0.66).
//! - Colour from a physically-based spectral wavelength LUT (380–720 nm),
//!   swept diagonally with per-cell hue jitter.
//! - Screen-blended.

use tiny_skia::Pixmap;

use crate::pixel_ops::screen_pixel;

use super::{
    CoverageRect,
    math::{
        SPECTRAL_LAM_MAX, SPECTRAL_LAM_MIN, cell_hash01, ser_hash01, smoothstep, spectral_lookup,
    },
};

// ── Cell grid & hashing (same as SER optical_micro_facet) ─────────────────────

const CELL_W: i32 = 7;
const CELL_H: i32 = 7;
const CELL_GAP: i32 = 2;
const PITCH: i32 = CELL_W + CELL_GAP; // 9

/// Same per-cell hash as `optical_cell_hash`.  Returns [0, 1).
#[inline]
fn cell_hash(col: i32, row: i32, seed: u32) -> f32 {
    cell_hash01(col, row, seed)
}

/// Column, row, horizontal-stagger for pixel at `(local_x, local_y)`.
///
/// Unlike SER which staggers odd **columns** vertically, NPR staggers odd
/// **rows** horizontally so that a horizontally-elongated facet can bridge
/// two adjacent columns.
#[inline]
fn cell_coord(local_x: i32, local_y: i32) -> (i32, i32, i32) {
    let row = local_y.div_euclid(PITCH);
    let stagger_x = if (row & 1) == 1 { PITCH / 2 } else { 0 };
    let col = (local_x - stagger_x).div_euclid(PITCH);
    (col, row, stagger_x)
}

// ── "#" band pattern ──────────────────────────────────────────────────────────
//
// Band positions are anchored to the illustration (art) frame, not the full
// card.  Art layout for the standard 1394×2031 card:
//   art = (170, 375), size 1054×1054
// Ratios are used so the bands scale with any card size.
//
// Horizontal bands (relative to art area):
//   H1  just below art top border
//   H2  art vertical centre
//   H3  art bottom border
//
// Vertical bands (relative to art area):
//   V1  art left + ¼ art width
//   V2  art left + ¾ art width

/// Art-area ratios (from standard 1394×2031 card layout).
const ART_LEFT_RATIO: f32 = 170.0 / 1394.0;
const ART_TOP_RATIO: f32 = 375.0 / 2031.0;
const ART_WIDTH_RATIO: f32 = 1054.0 / 1394.0;
const ART_HEIGHT_RATIO: f32 = 1054.0 / 2031.0;

/// Check whether the cell at `(col, row)` falls inside a "#" band.
/// `total_cols` and `total_rows` are the number of cells spanning the card.
fn band_presence_cell(col: i32, row: i32, total_cols: i32, total_rows: i32) -> f32 {
    if total_cols <= 0 || total_rows <= 0 {
        return 0.0;
    }

    // Art frame in cell coordinates
    let art_left = (total_cols as f32 * ART_LEFT_RATIO).round() as i32;
    let art_top = (total_rows as f32 * ART_TOP_RATIO).round() as i32;
    let art_w = (total_cols as f32 * ART_WIDTH_RATIO).round() as i32;
    let art_h = (total_rows as f32 * ART_HEIGHT_RATIO).round() as i32;
    let art_bottom = art_top + art_h;
    let art_mid = art_top + art_h / 2;

    // ── Horizontal band centres (art-relative) ──────────────────────────
    let c1 = art_top + 3; // just below art top border
    let c2 = art_mid; // art centre
    let c3 = art_bottom; // art bottom border

    // Per-column organic jitter: shifts the effective band edge ±0–1 rows
    let col_hash = cell_hash(col, 0, 0xBEEF);
    let jitter: i32 = if col_hash > 0.70 {
        1
    } else if col_hash < 0.30 {
        -1
    } else {
        0
    };

    // Per-column width wobble for bands 2 and 3 (±0–1 extra rows)
    let wb_hash = cell_hash(col, 0, 0xCAFE);
    let extra_w2: i32 = if wb_hash > 0.75 {
        1
    } else if wb_hash < 0.25 {
        -1
    } else {
        0
    };
    let extra_w3: i32 = if wb_hash > 0.80 {
        1
    } else if wb_hash < 0.20 {
        -1
    } else {
        0
    };

    // H1: pattern 1-0-0-1-1 (gap widened to 2 rows), with jitter
    let r = row + jitter;
    let in_band1 = r == c1 - 2              // 1
                || r == c1 + 1              // 1
                || r == c1 + 2; // 1
    // rows c1-1 and c1 are the 2-row gap

    // H2: 3 cell-rows (±1) + width wobble
    let half2 = 1 + extra_w2.abs();
    let in_band2 = (r - c2).abs() <= half2;

    // H3: 3 cell-rows (±1) + width wobble
    let half3 = 1 + extra_w3.abs();
    let in_band3 = (r - c3).abs() <= half3;

    let h_active = in_band1 || in_band2 || in_band3;

    // ── Vertical band centres (art-relative) ────────────────────────────
    let v1 = art_left + art_w / 4; // art left + ¼ width
    let v2 = art_left + art_w * 3 / 4; // art left + ¾ width

    // Width fluctuates 3 or 4 cell-columns per row
    let w1: i32 = if (ser_pixel_hash_band(row as u32) & 1) == 0 {
        3
    } else {
        4
    };
    let w2: i32 = if (ser_pixel_hash_band((row as u32) ^ 0xAAAA) & 1) == 0 {
        3
    } else {
        4
    };
    let half1 = w1 / 2;
    let half2 = w2 / 2;
    let in_col1 = (col - v1).abs() <= half1;
    let in_col2 = (col - v2).abs() <= half2;
    let v_active = in_col1 || in_col2;

    // ── Cross-hatch: either direction, boosted at intersections ────────
    let h = if h_active { 1.0_f32 } else { 0.0 };
    let vv = if v_active { 1.0_f32 } else { 0.0 };
    (h.max(vv) + h * vv * 0.50).clamp(0.0, 1.0)
}

/// Fast hash for vertical-band width fluctuation.
fn ser_pixel_hash_band(v: u32) -> u32 {
    let h = v.wrapping_mul(2_246_822_519);
    h ^ (h >> 13)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Horizontal micro-facet (same as SER but dx_stretched instead of dy_stretched)
// ═══════════════════════════════════════════════════════════════════════════════

/// Dome depth (same magnitude as SER).
const DOME_DEPTH: f32 = 0.06;

/// Compute the horizontal micro-facet for a cell.
/// Returns `(inside, brightness)` where `inside` is [0, 1] soft occupancy
/// and `brightness` is a per-cell luminance multiplier.
fn horizontal_micro_facet(
    local_x: i32,
    local_y: i32,
    col: i32,
    row: i32,
    stagger_x: i32,
) -> (f32, f32) {
    // ── Elongation (horizontal, not vertical like SER) ─────────────────
    let rnd_line = cell_hash(col, row, 0x5555);
    let line_len = if rnd_line > 0.80 {
        cell_hash(col, row, 0x6666) * 3.0 + 2.0 // 2.0..5.0
    } else {
        1.0
    };

    // Cell centre — horizontally staggered (SER staggers vertically)
    let cx = (col as f32 + 0.5) * PITCH as f32 + stagger_x as f32;
    let cy = (row as f32 + 0.5) * PITCH as f32;

    // Distance from centre, normalised to cell half-size
    let dx = (local_x as f32 - cx) / (CELL_W as f32 * 0.5);
    let dy = (local_y as f32 - cy) / (CELL_H as f32 * 0.5);

    // *** KEY DIFFERENCE FROM SER: horizontal stretch instead of vertical ***
    let dx_stretched = dx / line_len.max(1.0);
    // dy is not stretched (SER stretches dy here)

    // Rounded-diamond shape (same as SER)
    let r = (dx_stretched.abs().powf(3.0) + dy.abs().powf(3.0)).powf(1.0 / 3.0);
    let inside = 1.0 - smoothstep(0.85, 1.08, r);

    // Dome brightness mimics the facet's surface normal towards viewer
    let r_dist = (dx * dx + dy * dy).sqrt();
    let slope = DOME_DEPTH * r_dist.clamp(0.0, 1.0);
    // Brightness = how much light the dome reflects toward the viewer
    let dome_bright = (1.0 - slope * 0.5).clamp(0.0, 1.0);

    (inside, dome_bright)
}

// ── Main entry ────────────────────────────────────────────────────────────────

pub(crate) fn draw_diamond_foil(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= f32::EPSILON {
        return;
    }

    let width = target.width();
    let height = target.height();
    let y_start = rect.y.min(height);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let x_start = rect.x.min(width);
    let x_end = rect.x.saturating_add(rect.w).min(width);
    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let rect_w = rect.w.max(1) as f32;
    let rect_h = rect.h.max(1) as f32;
    let norm_w = (rect_w - 1.0).max(1.0);
    let norm_h = (rect_h - 1.0).max(1.0);
    // Cell-grid span of the card (for band positioning)
    let total_cols = ((rect_w / PITCH as f32).ceil() as i32).max(1);
    let total_rows = ((rect_h / PITCH as f32).ceil() as i32).max(1);
    let pixels = target.pixels_mut();

    for y in y_start..y_end {
        let local_y = (y - rect.y) as i32;
        let v = local_y as f32 / norm_h;

        for x in x_start..x_end {
            let local_x = (x - rect.x) as i32;
            let u = local_x as f32 / norm_w;

            let (col, row, stagger_x) = cell_coord(local_x, local_y);
            let (inside, dome_bright) =
                horizontal_micro_facet(local_x, local_y, col, row, stagger_x);

            if inside <= 0.001 {
                continue;
            }

            // ── "#" band gate ──────────────────────────────────────────
            let presence = band_presence_cell(col, row, total_cols, total_rows);
            if presence < 0.015 {
                continue;
            }

            // ── Spectral colour ─────────────────────────────────────────
            let h1 = cell_hash(col, row, 0x7777);
            let noise = ser_hash01(x, y);
            let lam = SPECTRAL_LAM_MIN
                + (SPECTRAL_LAM_MAX - SPECTRAL_LAM_MIN)
                    * ((u * 0.45 + v * 0.35 + noise * 0.06 + h1 * 0.14).rem_euclid(1.0));
            let (r, g, b) = spectral_lookup(lam);

            // ── Brightness — mirror-like foil, high reflectance ─────────
            let cell_bright = 0.62 + h1 * 0.28; // 0.62–0.90
            let band_boost = presence * 0.38; // up to +0.38 in band
            let specular = dome_bright.powi(3) * 0.30; // centre-hotspot highlight
            let facet_bright = inside * (cell_bright + band_boost + specular);
            let bright = facet_bright.clamp(0.0, 1.0);
            let alpha = (opacity * bright * 255.0).round() as u8;
            if alpha == 0 {
                continue;
            }

            // ── Screen blend ────────────────────────────────────────────
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
