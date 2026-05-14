//! Pser-print bright border effect.

use tiny_skia::Pixmap;

use crate::{
    constants::{CARD_HEIGHT, CARD_WIDTH},
    pixel_ops::pixel_hash,
};

use super::CoverageRect;

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
            pixels[idx] = crate::pixel_ops::screen_pixel(pixels[idx], src_r, src_g, src_b, alpha);
        }
    }
}
