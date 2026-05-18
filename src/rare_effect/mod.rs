//! Algorithmic rare/foil effect rendering.
//!
//! No external noise crates are used; all procedural math is inline.

mod bright_border;
mod diamond_foil;
mod dot_grid;
mod holographic;
pub(crate) mod math;
mod optical;
mod rainbow_foil;
mod secret;

// ── Shared types ──────────────────────────────────────────────────────────────

/// Axis-aligned rectangle in card-local pixel coordinates.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CoverageRect {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

// ── Public API re-exports ─────────────────────────────────────────────────────

pub(crate) use bright_border::draw_bright_border;
pub(crate) use diamond_foil::draw_diamond_foil;
pub(crate) use dot_grid::draw_dot_grid;
pub(crate) use holographic::draw_holographic;
pub(crate) use optical::{
    draw_optical_scr, draw_optical_scr_simple, draw_optical_ser, draw_optical_ser_simple,
};
pub(crate) use rainbow_foil::draw_rainbow_foil;
pub(crate) use secret::{draw_secret_foil, draw_secret_weave};

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
pub(crate) use math::hsv_to_color;

#[cfg(test)]
mod tests {
    use tiny_skia::{Color, Pixmap};

    use super::*;

    // ── hsv_to_color anchor values ────────────────────────────────────────────

    #[test]
    fn hsv_red() {
        let c = hsv_to_color(0.0, 1.0, 1.0, 1.0);
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

    // ── Primitive smoke tests (must not panic, must mutate pixels) ────────────

    fn make_card_pixmap() -> Pixmap {
        let mut p = Pixmap::new(100, 100).unwrap();
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
