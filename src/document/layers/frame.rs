use crate::{
    asset_bundle::{AssetBundle, BaseLayout},
    card_logic::{display_stat, uses_rank},
    constants::{CARD_WIDTH, TEXT_COLOR_DARK},
    layout::LayoutStyle,
    model::{OutFrameEffectBox, RenderRequest, TextAlignChoice, YgoCardMeta},
};

use super::super::paint;
use super::super::{RenderNode, RenderOp, RenderRect};

// ── Out-frame blocks ─────────────────────────────────────────────────────────

pub(crate) fn push_out_frame_nodes(
    nodes: &mut Vec<RenderNode>,
    bundle: &AssetBundle,
    request: &RenderRequest,
    base: &BaseLayout,
) {
    let card = &request.card;

    if card.out_frame_name_block_enabled {
        let name_block = &base.out_frame.name_block;
        nodes.push(RenderNode::new(
            "out-frame-name-block",
            50,
            RenderOp::ImageAsset {
                asset: name_block.asset.clone(),
                x: name_block.x as f32,
                y: name_block.y as f32,
            },
        ));
    }

    if card.is_pendulum() {
        // Pendulum cards carry their effect-panel and lower text-box
        // background in the split pendulum mask. Reusing the normal out-frame
        // eblock would wash over the lower half and make scale/text areas hard
        // to read, so only the name block is added here.
        return;
    }

    let effect_box = match card.out_frame_effect_box {
        OutFrameEffectBox::EblockBorder => &base.out_frame.effect_box,
        OutFrameEffectBox::EblockBorderO => &base.out_frame.effect_box_colored,
    };
    let effect_rect = RenderRect::from_f32(
        request
            .options
            .effect_block_x
            .unwrap_or(effect_box.x as f32),
        request
            .options
            .effect_block_y
            .unwrap_or(effect_box.y as f32),
        request
            .options
            .effect_block_width
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or_else(|| effect_box.w(bundle) as f32),
        request
            .options
            .effect_block_height
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or_else(|| effect_box.h(bundle) as f32),
    );

    if card.out_frame_effect_enabled {
        // Effect box background FillRect
        if let Some(color) = card
            .out_frame_effect_background_color
            .as_deref()
            .filter(|c| !c.is_empty())
        {
            let opacity = card.out_frame_effect_opacity.unwrap_or(1.0).clamp(0.0, 1.0);
            if opacity > 0.0 {
                nodes.push(RenderNode::new(
                    "out-frame-effect-bg",
                    51,
                    RenderOp::FillRect {
                        rect: effect_rect,
                        color: color.to_string(),
                        opacity,
                    },
                ));
            }
        }

        nodes.push(RenderNode::new(
            "out-frame-effect-box",
            52,
            RenderOp::ImageAssetRect {
                asset: effect_box.asset.clone(),
                rect: effect_rect,
            },
        ));
    }
}

// ── Level / Rank ─────────────────────────────────────────────────────────────

pub(crate) fn push_level_or_rank_nodes(
    nodes: &mut Vec<RenderNode>,
    card: &YgoCardMeta,
    base: &BaseLayout,
) {
    let count = card.level.min(13);
    if count == 0 {
        return;
    }

    let (layout, left_to_right) = if uses_rank(card) {
        (&base.rank, true)
    } else {
        (&base.level, false)
    };

    let id = if uses_rank(card) { "rank" } else { "level" };
    let start = if left_to_right {
        if count < 13 {
            layout.left_lt_13.unwrap_or(147)
        } else {
            layout.left_ge_13.unwrap_or(101)
        }
    } else if count < 13 {
        layout.right_lt_13.unwrap_or(147)
    } else {
        layout.right_ge_13.unwrap_or(101)
    };

    for index in 0..count {
        let x = if left_to_right {
            start + index * (layout.star_width + layout.gap)
        } else {
            CARD_WIDTH - start - index * (layout.star_width + layout.gap) - layout.star_width
        };
        nodes.push(RenderNode::new(
            id,
            80,
            RenderOp::ImageAsset {
                asset: layout.asset.clone(),
                x: x as f32,
                y: layout.y as f32,
            },
        ));
    }
}

// ── Link arrows ──────────────────────────────────────────────────────────────

pub(crate) fn push_link_arrow_nodes(
    nodes: &mut Vec<RenderNode>,
    card: &YgoCardMeta,
    base: &BaseLayout,
) {
    const ARROW_KEYS: &[&str] = &[
        "up",
        "right_up",
        "right",
        "right_down",
        "down",
        "left_down",
        "left",
        "left_up",
    ];
    const ARROW_BITS: &[u32] = &[0x004, 0x080, 0x020, 0x100, 0x040, 0x008, 0x001, 0x002];

    for (key, bit) in ARROW_KEYS.iter().zip(ARROW_BITS.iter()) {
        let Some(pair) = base.link_arrows.get(*key) else {
            continue;
        };
        let state = if (card.link_marker & bit) != 0 {
            &pair.on
        } else {
            &pair.off
        };
        nodes.push(RenderNode::new(
            "link-arrow",
            90,
            RenderOp::ImageAsset {
                asset: state.asset.clone(),
                x: state.x as f32,
                y: state.y as f32,
            },
        ));
    }
}

// ── Stats (ATK/DEF/LINK + pendulum scales) ─────────────────────────────────

pub(crate) fn push_stats_nodes(
    nodes: &mut Vec<RenderNode>,
    bundle: &AssetBundle,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
) {
    let card = &request.card;

    let show_atk_bar = request.options.atk_bar.unwrap_or(true);

    if show_atk_bar && (card.is_monster() || card.is_pendulum()) {
        // Stat separator background
        let sep_asset = if card.is_link() {
            bundle
                .layout
                .resource_rules
                .atk_link_asset
                .get(if language == Some("astral") {
                    "astral"
                } else {
                    "default"
                })
        } else {
            bundle
                .layout
                .resource_rules
                .atk_def_asset
                .get(if language == Some("astral") {
                    "astral"
                } else {
                    "default"
                })
        };
        if let Some(sep_asset) = sep_asset {
            nodes.push(RenderNode::new(
                "stats-separator",
                140,
                RenderOp::ImageAsset {
                    asset: sep_asset.clone(),
                    x: base.atk_def_link.background.x as f32,
                    y: base.atk_def_link.background.y as f32,
                },
            ));
        }
    }

    if show_atk_bar && card.is_monster() {
        let stats_fill = paint::resolve_text_fill(
            &request.options.text_colors.stats,
            None,
            paint::solid_color_text_paint(0, 0, 0),
        );
        let stats_shadow =
            paint::resolve_optional_fill(&request.options.text_colors.stats_shadow, None);

        if card.is_link() {
            // ATK
            nodes.push(RenderNode::new(
                "stats-atk",
                141,
                RenderOp::TextLine {
                    text: display_stat(card.attack),
                    rect: RenderRect::new(style.stat_atk_x, style.stat_top, 220, style.stat_size),
                    font_family: style.stat_font_family.clone(),
                    font_size: style.stat_size,
                    letter_spacing: style.stat_letter_spacing,
                    align: TextAlignChoice::Right,
                    fill: stats_fill.clone(),
                    shadow: stats_shadow.clone(),
                    ruby: None,
                    width_compress: false,
                    font_weight: None,
                },
            ));
            // Link value
            nodes.push(RenderNode::new(
                "stats-link",
                142,
                RenderOp::TextLine {
                    text: card.level.to_string(),
                    rect: RenderRect::new(style.stat_link_x, style.link_top, 120, style.link_size),
                    font_family: style.link_font_family.clone(),
                    font_size: style.link_size,
                    letter_spacing: style.stat_letter_spacing,
                    align: TextAlignChoice::Right,
                    fill: stats_fill,
                    shadow: stats_shadow.clone(),
                    ruby: None,
                    width_compress: false,
                    font_weight: None,
                },
            ));
        } else {
            // ATK
            nodes.push(RenderNode::new(
                "stats-atk",
                141,
                RenderOp::TextLine {
                    text: display_stat(card.attack),
                    rect: RenderRect::new(style.stat_atk_x, style.stat_top, 220, style.stat_size),
                    font_family: style.stat_font_family.clone(),
                    font_size: style.stat_size,
                    letter_spacing: style.stat_letter_spacing,
                    align: TextAlignChoice::Right,
                    fill: stats_fill.clone(),
                    shadow: stats_shadow.clone(),
                    ruby: None,
                    width_compress: false,
                    font_weight: None,
                },
            ));
            // DEF
            nodes.push(RenderNode::new(
                "stats-def",
                142,
                RenderOp::TextLine {
                    text: display_stat(card.defense),
                    rect: RenderRect::new(style.stat_def_x, style.stat_top, 220, style.stat_size),
                    font_family: style.stat_font_family.clone(),
                    font_size: style.stat_size,
                    letter_spacing: style.stat_letter_spacing,
                    align: TextAlignChoice::Right,
                    fill: stats_fill,
                    shadow: stats_shadow,
                    ruby: None,
                    width_compress: false,
                    font_weight: None,
                },
            ));
        }
    }

    if card.is_pendulum() {
        let left = if language == Some("astral") {
            &base.pendulum_scale.left.astral
        } else {
            &base.pendulum_scale.left.default
        };
        let right = if language == Some("astral") {
            &base.pendulum_scale.right.astral
        } else {
            &base.pendulum_scale.right.default
        };

        let scale_font_size = if language == Some("astral") {
            84.0
        } else {
            98.0
        };
        let scale_letter_spacing = if language == Some("astral") {
            0.0
        } else {
            -10.0
        };
        let scale_fill =
            paint::solid_color_text_paint(TEXT_COLOR_DARK.0, TEXT_COLOR_DARK.1, TEXT_COLOR_DARK.2);
        let stats_fill =
            paint::resolve_text_fill(&request.options.text_colors.stats, None, scale_fill.clone());
        let stats_shadow =
            paint::resolve_optional_fill(&request.options.text_colors.stats_shadow, None);

        nodes.push(RenderNode::new(
            "stats-lscale",
            143,
            RenderOp::TextLine {
                text: card.lscale.to_string(),
                rect: RenderRect::from_f32(left.x as f32, left.y as f32, 120.0, scale_font_size),
                font_family: style.stat_font_family.clone(),
                font_size: scale_font_size as u32,
                letter_spacing: scale_letter_spacing,
                align: TextAlignChoice::Center,
                fill: stats_fill.clone(),
                shadow: stats_shadow.clone(),
                ruby: None,
                width_compress: false,
                font_weight: None,
            },
        ));
        nodes.push(RenderNode::new(
            "stats-rscale",
            144,
            RenderOp::TextLine {
                text: card.rscale.to_string(),
                rect: RenderRect::from_f32(right.x as f32, right.y as f32, 120.0, scale_font_size),
                font_family: style.stat_font_family.clone(),
                font_size: scale_font_size as u32,
                letter_spacing: scale_letter_spacing,
                align: TextAlignChoice::Center,
                fill: stats_fill,
                shadow: stats_shadow,
                ruby: None,
                width_compress: false,
                font_weight: None,
            },
        ));
    }
}
