//! Legacy standalone rare-effect compositor (deprecated).
//!
//! This was the original layer-stacking API. It has been superseded by the
//! document pipeline (`RenderDocument` → `Renderer`), which expresses rarity
//! effects as `EffectTarget` + `EffectStyle` nodes. The direct draw functions
//! (`draw_rainbow_foil`, etc.) remain the pixel-level implementations used by
//! both paths.
//!
//! Kept for reference and its internal tests; not called by any active code path.

use tiny_skia::Pixmap;

use crate::{
    asset_bundle::BaseLayout,
    card_logic::image_frame,
    constants::{CARD_HEIGHT, CARD_WIDTH},
    model::{RareType, YgoCardMeta},
};

use super::{
    CoverageRect, bright_border::draw_bright_border, dot_grid::draw_dot_grid,
    holographic::draw_holographic, optical::draw_optical_ser, rainbow_foil::draw_rainbow_foil,
    secret::draw_secret_weave,
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
/// variants is a no-op — use `Renderer::render_document` instead.
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
            LayerKind::OpticalSer { opacity } => draw_optical_ser(target, rect, opacity),
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
enum LayerKind {
    /// Diagonal multi-stop rainbow LinearGradient.
    RainbowFoil { opacity: f32 },
    /// Horizontal/vertical grid of rainbow circles via Pattern tile.
    DotGrid { opacity: f32 },
    /// Fine prismatic weave used by Secret Rare style cards.
    SecretWeave { opacity: f32 },
    /// Optical diffraction simulation used by modern Secret Rare art foil.
    OpticalSer { opacity: f32 },
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

        RareType::Ser => vec![EffectLayer::art(LayerKind::OpticalSer { opacity: 1.0 })],

        RareType::Gser => vec![
            EffectLayer::full(LayerKind::SecretWeave { opacity: 0.58 }),
            EffectLayer::art(LayerKind::RainbowFoil { opacity: 0.18 }),
        ],

        RareType::Pser => vec![
            EffectLayer::art(LayerKind::RainbowFoil { opacity: 0.50 }),
            EffectLayer::art(LayerKind::DotGrid { opacity: 0.60 }),
        ],

        RareType::PserPrint => vec![EffectLayer::full(LayerKind::BrightBorder { opacity: 0.72 })],

        RareType::Gr | RareType::Ur | RareType::Utr | RareType::Scr | RareType::Dt => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hr_maps_to_holographic_fullcard() {
        let layers = layers_for(RareType::Hr);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].coverage, RareCoverage::FullCard);
        assert!(matches!(layers[0].kind, LayerKind::Holographic { .. }));
    }

    #[test]
    fn ser_maps_to_optical_ser_art_only() {
        let layers = layers_for(RareType::Ser);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].coverage, RareCoverage::Art);
        assert!(matches!(layers[0].kind, LayerKind::OpticalSer { .. }));
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
}
