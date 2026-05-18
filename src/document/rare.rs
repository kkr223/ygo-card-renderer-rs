use crate::model::{RareType, TextGradient, TextPaint};

use super::{EffectStyle, EffectTarget, EffectTargetWeight, RenderNode, RenderOp};

pub(super) fn push_rare_effect_nodes(nodes: &mut Vec<RenderNode>, rare: Option<RareType>) {
    nodes.extend(rare_effect_nodes(rare));
}

fn rare_effect_nodes(rare: Option<RareType>) -> Vec<RenderNode> {
    let mut nodes = Vec::new();
    let Some(rare) = rare else {
        return nodes;
    };

    match rare {
        RareType::Sr => push_composite_rare_effect(
            &mut nodes,
            "rare-sr-art-foil",
            30,
            EffectStyle::RainbowFoil { opacity: 0.46 },
            vec![tw(EffectTarget::Art, 0.46)],
        ),
        RareType::Ur => {
            push_composite_rare_effect(
                &mut nodes,
                "rare-ur-art-foil",
                30,
                EffectStyle::RainbowFoil { opacity: 0.46 },
                vec![tw(EffectTarget::Art, 0.46)],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-ur-icon-foil",
                91,
                EffectStyle::Holographic { opacity: 0.62 },
                vec![
                    tw(EffectTarget::Attribute, 0.62),
                    tw(EffectTarget::LevelOrRank, 0.58),
                    tw(EffectTarget::LinkArrows, 0.58),
                ],
            );
        }
        RareType::Gr => {
            push_composite_rare_effect(
                &mut nodes,
                "rare-gr-art-foil",
                30,
                EffectStyle::RainbowFoil { opacity: 0.46 },
                vec![tw(EffectTarget::Art, 0.46)],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-gr-border-gold",
                33,
                EffectStyle::GoldWash { opacity: 0.56 },
                vec![
                    tw(EffectTarget::CardBorder, 0.42),
                    tw(EffectTarget::ArtFrame, 0.56),
                ],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-gr-icon-foil",
                91,
                EffectStyle::Holographic { opacity: 0.62 },
                vec![
                    tw(EffectTarget::Attribute, 0.62),
                    tw(EffectTarget::LevelOrRank, 0.58),
                    tw(EffectTarget::LinkArrows, 0.58),
                ],
            );
        }
        RareType::Utr => {
            push_composite_rare_effect(
                &mut nodes,
                "rare-utr-frosted-card-base",
                30,
                EffectStyle::FrostedFoil { opacity: 0.50 },
                vec![tw(EffectTarget::CardBase, 0.50)],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-utr-art-relief",
                31,
                EffectStyle::ReliefEngrave { opacity: 1.00 },
                vec![tw(EffectTarget::Art, 1.00)],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-utr-icon-concentric-engrave",
                91,
                EffectStyle::ConcentricEngrave { opacity: 0.72 },
                vec![
                    tw(EffectTarget::Attribute, 0.72),
                    tw(EffectTarget::LevelOrRank, 0.68),
                    tw(EffectTarget::LinkArrows, 0.68),
                ],
            );
        }
        RareType::Hr => push_visual_effect(
            &mut nodes,
            "rare-hr-full-foil",
            95,
            EffectTarget::FullCard,
            EffectStyle::Holographic { opacity: 0.45 },
        ),
        RareType::Ser => {
            push_composite_rare_effect(
                &mut nodes,
                "rare-ser-art-optical",
                30,
                EffectStyle::OpticalSer { opacity: 1.00 },
                vec![tw(EffectTarget::Art, 1.00)],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-ser-icon-optical",
                91,
                EffectStyle::OpticalSerSimple { opacity: 0.90 },
                vec![
                    tw(EffectTarget::Attribute, 0.90),
                    tw(EffectTarget::LevelOrRank, 0.90),
                    tw(EffectTarget::LinkArrows, 0.90),
                ],
            );
        }
        RareType::Scr => {
            push_composite_rare_effect(
                &mut nodes,
                "rare-scr-art-optical",
                30,
                EffectStyle::OpticalScr { opacity: 1.00 },
                vec![tw(EffectTarget::Art, 1.00)],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-scr-icon-optical",
                91,
                EffectStyle::OpticalScrSimple { opacity: 0.90 },
                vec![
                    tw(EffectTarget::Attribute, 0.90),
                    tw(EffectTarget::LevelOrRank, 0.90),
                    tw(EffectTarget::LinkArrows, 0.90),
                ],
            );
        }
        RareType::Esr => push_visual_effect(
            &mut nodes,
            "rare-esr-full-optical",
            95,
            EffectTarget::FullCard,
            EffectStyle::OpticalScr { opacity: 0.50 },
        ),
        RareType::Gser => {
            push_composite_rare_effect(
                &mut nodes,
                "rare-gser-art-optical",
                30,
                EffectStyle::OpticalSer { opacity: 0.70 },
                vec![tw(EffectTarget::Art, 0.70)],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-gser-border-gold",
                33,
                EffectStyle::GoldWash { opacity: 0.56 },
                vec![
                    tw(EffectTarget::CardBorder, 0.42),
                    tw(EffectTarget::ArtFrame, 0.56),
                ],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-gser-border-optical",
                36,
                EffectStyle::OpticalSerSimple { opacity: 0.55 },
                vec![
                    tw(EffectTarget::CardBorder, 0.55),
                    tw(EffectTarget::ArtFrame, 0.55),
                    tw(EffectTarget::EffectBoxBorder, 0.55),
                ],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-gser-icon-optical",
                91,
                EffectStyle::OpticalSerSimple { opacity: 0.90 },
                vec![
                    tw(EffectTarget::Attribute, 0.90),
                    tw(EffectTarget::LevelOrRank, 0.90),
                    tw(EffectTarget::LinkArrows, 0.90),
                ],
            );
        }
        RareType::Pser => push_visual_effect(
            &mut nodes,
            "rare-pser-full-optical",
            95,
            EffectTarget::FullCard,
            EffectStyle::OpticalSer { opacity: 0.50 },
        ),
        RareType::Npr => push_visual_effect(
            &mut nodes,
            "rare-npr-diamond-foil",
            95,
            EffectTarget::FullCard,
            EffectStyle::DiamondFoil { opacity: 0.68 },
        ),
        RareType::Upr => {
            push_composite_rare_effect(
                &mut nodes,
                "rare-ur-art-foil",
                30,
                EffectStyle::RainbowFoil { opacity: 0.46 },
                vec![tw(EffectTarget::Art, 0.46)],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-ur-icon-foil",
                91,
                EffectStyle::Holographic { opacity: 0.62 },
                vec![
                    tw(EffectTarget::Attribute, 0.62),
                    tw(EffectTarget::LevelOrRank, 0.58),
                    tw(EffectTarget::LinkArrows, 0.58),
                ],
            );
            push_visual_effect(
                &mut nodes,
                "rare-upr-diamond-foil",
                96,
                EffectTarget::FullCard,
                EffectStyle::DiamondFoil { opacity: 0.68 },
            );
        }
        RareType::Sepr => {
            push_composite_rare_effect(
                &mut nodes,
                "rare-ser-art-optical",
                30,
                EffectStyle::OpticalSer { opacity: 1.00 },
                vec![tw(EffectTarget::Art, 1.00)],
            );
            push_composite_rare_effect(
                &mut nodes,
                "rare-ser-icon-optical",
                91,
                EffectStyle::OpticalSerSimple { opacity: 0.90 },
                vec![
                    tw(EffectTarget::Attribute, 0.90),
                    tw(EffectTarget::LevelOrRank, 0.90),
                    tw(EffectTarget::LinkArrows, 0.90),
                ],
            );
            push_visual_effect(
                &mut nodes,
                "rare-sepr-diamond-foil",
                96,
                EffectTarget::FullCard,
                EffectStyle::DiamondFoil { opacity: 0.68 },
            );
        }
        RareType::PserPrint => push_visual_effect(
            &mut nodes,
            "rare-pser-print-border",
            30,
            EffectTarget::FullCard,
            EffectStyle::BrightBorder { opacity: 0.72 },
        ),
        RareType::Dt => {}
    }

    nodes
}

fn push_visual_effect(
    nodes: &mut Vec<RenderNode>,
    id: &str,
    z: i32,
    target: EffectTarget,
    effect: EffectStyle,
) {
    nodes.push(RenderNode::new(
        id,
        z,
        RenderOp::VisualEffect { target, effect },
    ));
}

fn push_composite_rare_effect(
    nodes: &mut Vec<RenderNode>,
    id: &str,
    z: i32,
    effect: EffectStyle,
    targets: Vec<EffectTargetWeight>,
) {
    nodes.push(RenderNode::new(
        id,
        z,
        RenderOp::CompositeVisualEffect { effect, targets },
    ));
}

fn tw(target: EffectTarget, opacity: f32) -> EffectTargetWeight {
    EffectTargetWeight { target, opacity }
}

// ── Title paint preset ──────────────────────────────────────────────────────

pub(super) fn rare_title_paints(rare: Option<RareType>) -> (Option<TextPaint>, Option<TextPaint>) {
    match rare {
        Some(RareType::Ur | RareType::Gr | RareType::Gser) => (
            Some(TextPaint {
                color: None,
                gradient: Some(TextGradient::vertical_middle(
                    "#9a6718", "#fff0a8", "#6f4208",
                )),
            }),
            Some(TextPaint {
                color: Some("#5a3708".to_string()),
                gradient: Some(TextGradient::vertical_middle(
                    "#2d1903", "#a46a16", "#221103",
                )),
            }),
        ),
        Some(RareType::Ser | RareType::Pser | RareType::Scr) => (
            Some(TextPaint {
                color: None,
                gradient: Some(TextGradient::vertical_middle(
                    "#f8fafc", "#94a3b8", "#f1f5f9",
                )),
            }),
            Some(TextPaint {
                color: Some("#94a3b8".to_string()),
                gradient: Some(TextGradient::vertical_middle(
                    "#cbd5e1", "#64748b", "#cbd5e1",
                )),
            }),
        ),
        _ => (None, None),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn assert_composite(
        node: &RenderNode,
        id: &str,
        z: i32,
        expected_effect: EffectStyle,
        expected_targets: &[EffectTargetWeight],
    ) {
        assert_eq!(node.id, id);
        assert_eq!(node.z, z);
        match &node.op {
            RenderOp::CompositeVisualEffect { effect, targets } => {
                assert_eq!(*effect, expected_effect);
                assert_eq!(targets, expected_targets);
            }
            _ => panic!("expected composite visual effect"),
        }
    }

    fn assert_visual(
        node: &RenderNode,
        id: &str,
        z: i32,
        target: EffectTarget,
        expected_effect: EffectStyle,
    ) {
        assert_eq!(node.id, id);
        assert_eq!(node.z, z);
        match &node.op {
            RenderOp::VisualEffect {
                target: actual,
                effect,
            } => {
                assert_eq!(*actual, target);
                assert_eq!(*effect, expected_effect);
            }
            _ => panic!("expected visual effect"),
        }
    }

    #[test]
    fn rare_effect_nodes_lock_contracts() {
        assert!(rare_effect_nodes(None).is_empty());
        assert!(rare_effect_nodes(Some(RareType::Dt)).is_empty());

        let nodes = rare_effect_nodes(Some(RareType::Sr));
        assert_eq!(nodes.len(), 1);
        assert_composite(
            &nodes[0],
            "rare-sr-art-foil",
            30,
            EffectStyle::RainbowFoil { opacity: 0.46 },
            &[tw(EffectTarget::Art, 0.46)],
        );

        let nodes = rare_effect_nodes(Some(RareType::Ur));
        assert_eq!(nodes.len(), 2);
        assert_composite(
            &nodes[0],
            "rare-ur-art-foil",
            30,
            EffectStyle::RainbowFoil { opacity: 0.46 },
            &[tw(EffectTarget::Art, 0.46)],
        );
        assert_composite(
            &nodes[1],
            "rare-ur-icon-foil",
            91,
            EffectStyle::Holographic { opacity: 0.62 },
            &[
                tw(EffectTarget::Attribute, 0.62),
                tw(EffectTarget::LevelOrRank, 0.58),
                tw(EffectTarget::LinkArrows, 0.58),
            ],
        );

        let nodes = rare_effect_nodes(Some(RareType::Gr));
        assert_eq!(nodes.len(), 3);
        assert_composite(
            &nodes[0],
            "rare-gr-art-foil",
            30,
            EffectStyle::RainbowFoil { opacity: 0.46 },
            &[tw(EffectTarget::Art, 0.46)],
        );
        assert_composite(
            &nodes[1],
            "rare-gr-border-gold",
            33,
            EffectStyle::GoldWash { opacity: 0.56 },
            &[
                tw(EffectTarget::CardBorder, 0.42),
                tw(EffectTarget::ArtFrame, 0.56),
            ],
        );
        assert_composite(
            &nodes[2],
            "rare-gr-icon-foil",
            91,
            EffectStyle::Holographic { opacity: 0.62 },
            &[
                tw(EffectTarget::Attribute, 0.62),
                tw(EffectTarget::LevelOrRank, 0.58),
                tw(EffectTarget::LinkArrows, 0.58),
            ],
        );

        let nodes = rare_effect_nodes(Some(RareType::Utr));
        assert_eq!(nodes.len(), 3);
        assert_composite(
            &nodes[0],
            "rare-utr-frosted-card-base",
            30,
            EffectStyle::FrostedFoil { opacity: 0.50 },
            &[tw(EffectTarget::CardBase, 0.50)],
        );
        assert_composite(
            &nodes[1],
            "rare-utr-art-relief",
            31,
            EffectStyle::ReliefEngrave { opacity: 1.00 },
            &[tw(EffectTarget::Art, 1.00)],
        );
        assert_composite(
            &nodes[2],
            "rare-utr-icon-concentric-engrave",
            91,
            EffectStyle::ConcentricEngrave { opacity: 0.72 },
            &[
                tw(EffectTarget::Attribute, 0.72),
                tw(EffectTarget::LevelOrRank, 0.68),
                tw(EffectTarget::LinkArrows, 0.68),
            ],
        );

        let nodes = rare_effect_nodes(Some(RareType::Hr));
        assert_eq!(nodes.len(), 1);
        assert_visual(
            &nodes[0],
            "rare-hr-full-foil",
            95,
            EffectTarget::FullCard,
            EffectStyle::Holographic { opacity: 0.45 },
        );

        let nodes = rare_effect_nodes(Some(RareType::Ser));
        assert_eq!(nodes.len(), 2);
        assert_composite(
            &nodes[0],
            "rare-ser-art-optical",
            30,
            EffectStyle::OpticalSer { opacity: 1.00 },
            &[tw(EffectTarget::Art, 1.00)],
        );
        assert_composite(
            &nodes[1],
            "rare-ser-icon-optical",
            91,
            EffectStyle::OpticalSerSimple { opacity: 0.90 },
            &[
                tw(EffectTarget::Attribute, 0.90),
                tw(EffectTarget::LevelOrRank, 0.90),
                tw(EffectTarget::LinkArrows, 0.90),
            ],
        );

        let nodes = rare_effect_nodes(Some(RareType::Gser));
        assert_eq!(nodes.len(), 4);
        assert_composite(
            &nodes[0],
            "rare-gser-art-optical",
            30,
            EffectStyle::OpticalSer { opacity: 0.70 },
            &[tw(EffectTarget::Art, 0.70)],
        );
        assert_composite(
            &nodes[1],
            "rare-gser-border-gold",
            33,
            EffectStyle::GoldWash { opacity: 0.56 },
            &[
                tw(EffectTarget::CardBorder, 0.42),
                tw(EffectTarget::ArtFrame, 0.56),
            ],
        );
        assert_composite(
            &nodes[2],
            "rare-gser-border-optical",
            36,
            EffectStyle::OpticalSerSimple { opacity: 0.55 },
            &[
                tw(EffectTarget::CardBorder, 0.55),
                tw(EffectTarget::ArtFrame, 0.55),
                tw(EffectTarget::EffectBoxBorder, 0.55),
            ],
        );
        assert_composite(
            &nodes[3],
            "rare-gser-icon-optical",
            91,
            EffectStyle::OpticalSerSimple { opacity: 0.90 },
            &[
                tw(EffectTarget::Attribute, 0.90),
                tw(EffectTarget::LevelOrRank, 0.90),
                tw(EffectTarget::LinkArrows, 0.90),
            ],
        );

        let nodes = rare_effect_nodes(Some(RareType::Pser));
        assert_eq!(nodes.len(), 1);
        assert_visual(
            &nodes[0],
            "rare-pser-full-optical",
            95,
            EffectTarget::FullCard,
            EffectStyle::OpticalSer { opacity: 0.50 },
        );

        let nodes = rare_effect_nodes(Some(RareType::Scr));
        assert_eq!(nodes.len(), 2);
        assert_composite(
            &nodes[0],
            "rare-scr-art-optical",
            30,
            EffectStyle::OpticalScr { opacity: 1.00 },
            &[tw(EffectTarget::Art, 1.00)],
        );
        assert_composite(
            &nodes[1],
            "rare-scr-icon-optical",
            91,
            EffectStyle::OpticalScrSimple { opacity: 0.90 },
            &[
                tw(EffectTarget::Attribute, 0.90),
                tw(EffectTarget::LevelOrRank, 0.90),
                tw(EffectTarget::LinkArrows, 0.90),
            ],
        );

        let nodes = rare_effect_nodes(Some(RareType::Esr));
        assert_eq!(nodes.len(), 1);
        assert_visual(
            &nodes[0],
            "rare-esr-full-optical",
            95,
            EffectTarget::FullCard,
            EffectStyle::OpticalScr { opacity: 0.50 },
        );

        let nodes = rare_effect_nodes(Some(RareType::Npr));
        assert_eq!(nodes.len(), 1);
        assert_visual(
            &nodes[0],
            "rare-npr-diamond-foil",
            95,
            EffectTarget::FullCard,
            EffectStyle::DiamondFoil { opacity: 0.68 },
        );

        let nodes = rare_effect_nodes(Some(RareType::Upr));
        assert_eq!(nodes.len(), 3);
        assert_composite(
            &nodes[0],
            "rare-ur-art-foil",
            30,
            EffectStyle::RainbowFoil { opacity: 0.46 },
            &[tw(EffectTarget::Art, 0.46)],
        );
        assert_composite(
            &nodes[1],
            "rare-ur-icon-foil",
            91,
            EffectStyle::Holographic { opacity: 0.62 },
            &[
                tw(EffectTarget::Attribute, 0.62),
                tw(EffectTarget::LevelOrRank, 0.58),
                tw(EffectTarget::LinkArrows, 0.58),
            ],
        );
        assert_visual(
            &nodes[2],
            "rare-upr-diamond-foil",
            96,
            EffectTarget::FullCard,
            EffectStyle::DiamondFoil { opacity: 0.68 },
        );

        let nodes = rare_effect_nodes(Some(RareType::Sepr));
        assert_eq!(nodes.len(), 3);
        assert_composite(
            &nodes[0],
            "rare-ser-art-optical",
            30,
            EffectStyle::OpticalSer { opacity: 1.00 },
            &[tw(EffectTarget::Art, 1.00)],
        );
        assert_composite(
            &nodes[1],
            "rare-ser-icon-optical",
            91,
            EffectStyle::OpticalSerSimple { opacity: 0.90 },
            &[
                tw(EffectTarget::Attribute, 0.90),
                tw(EffectTarget::LevelOrRank, 0.90),
                tw(EffectTarget::LinkArrows, 0.90),
            ],
        );
        assert_visual(
            &nodes[2],
            "rare-sepr-diamond-foil",
            96,
            EffectTarget::FullCard,
            EffectStyle::DiamondFoil { opacity: 0.68 },
        );

        let nodes = rare_effect_nodes(Some(RareType::PserPrint));
        assert_eq!(nodes.len(), 1);
        assert_visual(
            &nodes[0],
            "rare-pser-print-border",
            30,
            EffectTarget::FullCard,
            EffectStyle::BrightBorder { opacity: 0.72 },
        );
    }

    #[test]
    fn rare_title_paints_returns_none_for_none_rare() {
        assert_eq!(rare_title_paints(None), (None, None));
    }

    #[test]
    fn rare_title_paints_returns_none_for_dt() {
        assert_eq!(rare_title_paints(Some(RareType::Dt)), (None, None));
    }

    #[test]
    fn rare_title_paints_gold_for_ur_gr_gser() {
        for rare in [RareType::Ur, RareType::Gr, RareType::Gser] {
            let (fill, shadow) = rare_title_paints(Some(rare));
            assert!(fill.is_some(), "{rare:?} should have fill");
            assert!(shadow.is_some(), "{rare:?} should have shadow");
            assert!(
                fill.unwrap().gradient.is_some(),
                "{rare:?} fill should be gradient"
            );
            assert!(
                shadow.unwrap().color.is_some(),
                "{rare:?} shadow should have color"
            );
        }
    }

    #[test]
    fn rare_title_paints_silver_for_ser_pser_scr() {
        for rare in [RareType::Ser, RareType::Pser, RareType::Scr] {
            let (fill, shadow) = rare_title_paints(Some(rare));
            assert!(fill.is_some(), "{rare:?} should have fill");
            assert!(shadow.is_some(), "{rare:?} should have shadow");
        }
    }
}
