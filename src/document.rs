use std::path::PathBuf;

mod rare;

use serde::{Deserialize, Serialize};

use crate::{
    asset_bundle::{AssetBundle, PositionedAsset},
    card_logic::{
        attribute_asset_name, build_effect_line, description_height, description_y, display_stat,
        frame_asset_name, image_frame, localized_brackets, localized_spell_trap_name,
        spell_trap_subtype_icon_asset, split_pendulum_description, uses_rank,
    },
    constants::{BACKGROUND_CREAM, CARD_HEIGHT, CARD_WIDTH, TEXT_COLOR_DARK},
    layout::layout_style,
    model::{
        CardKind, NameColor, OutFrameEffectBox, PositionedRenderImage, RareType, RenderOptions,
        RenderRequest, TextAlignChoice, TextGradient, TextPaint, YgoCardMeta,
    },
    ruby::strip_ruby_markup,
    text::estimate_text_width,
};

// ── RenderDocument ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderDocument {
    pub schema_version: u32,
    pub kind: CardKind,
    pub canvas: RenderCanvas,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub output_scale: f32,
    pub card: YgoCardMeta,
    pub options: RenderOptions,
    pub nodes: Vec<RenderNode>,
}

impl RenderDocument {
    pub const SCHEMA_VERSION: u32 = 4;

    pub fn from_request(request: &RenderRequest, bundle: &AssetBundle) -> Self {
        let language = request.options.language.as_deref();
        let base = &bundle.layout.base;
        let style = layout_style(
            request.kind,
            language,
            &bundle.layout,
            &request.options.layout_overrides,
        );
        let output_scale = effective_output_scale(request);
        let mut nodes = Vec::new();
        let card = &request.card;

        // ── Frame ──────────────────────────────────────────────────────────
        nodes.push(RenderNode::new(
            "frame",
            0,
            RenderOp::ImageAsset {
                asset: frame_asset_name(card).to_string(),
                x: 0.0,
                y: 0.0,
            },
        ));

        // ── Art ────────────────────────────────────────────────────────────
        let (art_x, art_y, art_w, art_h) = image_frame(card, base);
        nodes.push(RenderNode::new(
            "art",
            10,
            RenderOp::ExternalImage {
                path: request.options.art_image.clone(),
                rect: RenderRect::new(art_x, art_y, art_w, art_h),
                fit: ImageFit::Cover,
                align: ImageAlign::Top,
            },
        ));

        // ── Mask ───────────────────────────────────────────────────────────
        let mask = if card.is_pendulum() {
            &base.mask.pendulum
        } else {
            &base.mask.normal
        };
        nodes.push(RenderNode::new(
            "mask",
            20,
            RenderOp::ImageAsset {
                asset: mask.asset.clone(),
                x: mask.x as f32,
                y: mask.y as f32,
            },
        ));

        // ── Rare effects ───────────────────────────────────────────────────
        rare::push_rare_effect_nodes(&mut nodes, card.rare);

        // ── Foreground image ──────────────────────────────────────────────
        if let Some(image) = foreground_image_for_request(request) {
            nodes.push(RenderNode::new(
                "foreground",
                40,
                RenderOp::PositionedImage {
                    image: image.clone(),
                },
            ));
        }

        // ── Out-frame blocks → ImageAsset + FillRect ─────────────────────
        if card.out_frame {
            push_out_frame_nodes(&mut nodes, bundle, request, base);
        }

        // ── Anniversary mark → ImageAsset ─────────────────────────────────
        if card.twenty_fifth || card.twentieth {
            let mark = if card.twenty_fifth {
                Some(&base.twenty_fifth)
            } else {
                Some(&base.twentieth)
            };
            if let Some(mark) = mark {
                nodes.push(RenderNode::new(
                    "anniversary-mark",
                    60,
                    RenderOp::ImageAsset {
                        asset: mark.asset.clone(),
                        x: mark.x as f32,
                        y: mark.y as f32,
                    },
                ));
            }
        }

        // ── Attribute → ImageAsset ────────────────────────────────────────
        if let Some(asset) = attribute_asset_name(card, language) {
            nodes.push(RenderNode::new(
                "attribute",
                70,
                RenderOp::ImageAsset {
                    asset,
                    x: base.attribute.x as f32,
                    y: base.attribute.y as f32,
                },
            ));
        }

        // ── Level / Rank → ImageAsset × N ────────────────────────────────
        if !card.is_link() && card.level > 0 {
            push_level_or_rank_nodes(&mut nodes, card, base);
        }

        // ── Link arrows → ImageAsset × 8 ──────────────────────────────────
        if card.is_link() {
            push_link_arrow_nodes(&mut nodes, card, base);
        }

        // ── Title → TextLine ──────────────────────────────────────────────
        push_title_node(&mut nodes, request, &style, base);

        // ── Spell/Trap line or monster type line → TextLine ─────────────────
        if card.is_spell() || card.is_trap() {
            push_spell_trap_nodes(&mut nodes, bundle, request, &style, language);
        } else if let Some(text) = build_effect_line(card, request.kind, language) {
            push_monster_type_node(&mut nodes, &style, base, &text, &request.options);
        }

        // ── Pendulum description → TextBlock ──────────────────────────────
        let description_text = if card.is_pendulum() {
            push_pendulum_description_node(
                &mut nodes,
                card,
                &style,
                base,
                language,
                &request.options,
            );
            split_pendulum_description(&card.desc, language).monster_effect
        } else {
            card.desc.clone()
        };

        // ── Description → TextBlock ───────────────────────────────────────
        push_description_node(
            &mut nodes,
            card,
            &style,
            base,
            &description_text,
            &request.options,
        );

        // ── Stats → TextLine × N + ImageAsset ─────────────────────────────
        push_stats_nodes(&mut nodes, bundle, request, &style, base, language);

        // ── Password → TextLine ───────────────────────────────────────────
        push_password_node(&mut nodes, request, &style, base);

        // ── Monster footer scale/link-marker line → TextLine ──────────────
        if card.is_monster() {
            push_scale_line_node(&mut nodes, request, &style, base);
        }

        // ── Package → TextLine ────────────────────────────────────────────
        if let Some(package) = &card.package {
            push_package_node(&mut nodes, request, &style, base, package);
        }

        // ── Copyright → ImageAsset or TextLine ────────────────────────────
        if let Some(copyright) = &card.copyright {
            push_copyright_node(
                &mut nodes,
                bundle,
                card,
                copyright,
                base,
                &style,
                &request.options,
            );
        }

        // ── Laser → ImageAsset ────────────────────────────────────────────
        if let Some(asset) = card.laser.as_deref().and_then(laser_asset_name) {
            nodes.push(RenderNode::new(
                "laser",
                180,
                RenderOp::ImageAsset {
                    asset,
                    x: base.laser.x as f32,
                    y: base.laser.y as f32,
                },
            ));
        }

        Self {
            schema_version: Self::SCHEMA_VERSION,
            kind: request.kind,
            canvas: RenderCanvas {
                width: CARD_WIDTH,
                height: CARD_HEIGHT,
                background: Some(format!(
                    "#{:02x}{:02x}{:02x}",
                    BACKGROUND_CREAM.0, BACKGROUND_CREAM.1, BACKGROUND_CREAM.2
                )),
            },
            language: request.options.language.clone(),
            output_scale,
            card: card.clone(),
            options: request.options.clone(),
            nodes,
        }
    }
}

// ── RenderDocument helpers ─────────────────────────────────────────────────────

fn push_out_frame_nodes(
    nodes: &mut Vec<RenderNode>,
    bundle: &AssetBundle,
    request: &RenderRequest,
    base: &crate::asset_bundle::BaseLayout,
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

    let effect_box = match card.out_frame_effect_box {
        OutFrameEffectBox::EblockBorder => &base.out_frame.effect_box,
        OutFrameEffectBox::EblockBorderO => &base.out_frame.effect_box_colored,
    };

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
                        rect: RenderRect::new(
                            effect_box.x,
                            effect_box.y,
                            effect_box.w(bundle),
                            effect_box.h(bundle),
                        ),
                        color: color.to_string(),
                        opacity,
                    },
                ));
            }
        }

        nodes.push(RenderNode::new(
            "out-frame-effect-box",
            52,
            RenderOp::ImageAsset {
                asset: effect_box.asset.clone(),
                x: effect_box.x as f32,
                y: effect_box.y as f32,
            },
        ));
    }
}

fn push_level_or_rank_nodes(
    nodes: &mut Vec<RenderNode>,
    card: &YgoCardMeta,
    base: &crate::asset_bundle::BaseLayout,
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

fn push_link_arrow_nodes(
    nodes: &mut Vec<RenderNode>,
    card: &YgoCardMeta,
    base: &crate::asset_bundle::BaseLayout,
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

fn push_title_node(
    nodes: &mut Vec<RenderNode>,
    request: &RenderRequest,
    style: &crate::layout::LayoutStyle,
    base: &crate::asset_bundle::BaseLayout,
) {
    let card = &request.card;
    let show_attr = card.attribute != 0 || card.is_spell() || card.is_trap();
    let title_width = if show_attr {
        style.title_max_width_with_attribute
    } else {
        style.title_max_width_without_attribute
    };

    let (rare_fill, rare_shadow) = rare_title_paints(card.rare);

    // Resolve effective fill
    let fill = resolve_effective_title_fill(
        rare_fill,
        &card.name_color,
        card,
        &request.options.text_colors.name,
        card.name_gradient.as_ref(),
    );

    // Resolve effective shadow
    let shadow = resolve_effective_title_shadow(
        rare_shadow,
        &request.options.text_colors.name_shadow,
        card.name_shadow_color.as_deref(),
        card.name_shadow_gradient.as_ref(),
    );

    let ruby = if style.name_rt_font_size > 0 {
        Some(RubyStyle {
            rt_font_size: style.name_rt_font_size as f32,
            rt_top: style.name_rt_top,
            rt_font_scale_x: style.name_rt_font_scale_x,
        })
    } else {
        None
    };

    nodes.push(RenderNode::new(
        "title",
        100,
        RenderOp::TextLine {
            text: card.name.clone(),
            rect: RenderRect::new(style.name_x, style.name_top, title_width, base.name.height),
            font_family: style.name_font_family.clone(),
            font_size: style.name_size,
            letter_spacing: style.title_letter_spacing,
            align: TextAlignChoice::Left,
            fill,
            shadow,
            ruby,
            width_compress: request.options.title_width_compress,
        },
    ));
}

fn push_spell_trap_nodes(
    nodes: &mut Vec<RenderNode>,
    bundle: &AssetBundle,
    request: &RenderRequest,
    style: &crate::layout::LayoutStyle,
    language: Option<&str>,
) {
    let card = &request.card;
    let (left_bracket, right_bracket) = localized_brackets(language);
    let left_text = format!(
        "{left_bracket}{}",
        localized_spell_trap_name(card, language)
    );
    let font_size = style.type_size;

    // Resolve colour from text_colors override or fallback TYPE_COLOR
    let fill = resolve_text_fill(
        &request.options.text_colors.type_line,
        None,
        solid_color_text_paint(
            crate::constants::TYPE_COLOR.0,
            crate::constants::TYPE_COLOR.1,
            crate::constants::TYPE_COLOR.2,
        ),
    );
    let shadow = resolve_optional_fill(&request.options.text_colors.type_line_shadow, None);

    let ruby = if style.type_rt_font_size > 0 {
        Some(RubyStyle {
            rt_font_size: style.type_rt_font_size as f32,
            rt_top: style.type_rt_top,
            rt_font_scale_x: style.type_rt_font_scale_x,
        })
    } else {
        None
    };

    let right_width = estimate_text_width(
        right_bracket,
        language,
        &style.type_font_family,
        font_size as f32,
        style.type_letter_spacing,
    )
    .ceil()
    .max(32.0);
    let right_x = CARD_WIDTH as f32 - style.type_right as f32 - right_width;

    nodes.push(RenderNode::new(
        "spell-trap-right-bracket",
        110,
        RenderOp::TextLine {
            text: right_bracket.to_string(),
            rect: RenderRect::from_f32(
                right_x,
                style.type_top as f32,
                right_width,
                font_size as f32,
            ),
            font_family: style.type_font_family.clone(),
            font_size,
            letter_spacing: style.type_letter_spacing,
            align: TextAlignChoice::Left,
            fill: fill.clone(),
            shadow: shadow.clone(),
            ruby: None,
            width_compress: false,
        },
    ));

    let icon_asset = spell_trap_subtype_icon_asset(card).filter(|asset| bundle.has_image(asset));
    let icon_margins = spell_trap_icon_margins(language, bundle);
    let icon_width = icon_asset
        .and_then(|asset| image_width(bundle, asset).map(|width| width as f32))
        .unwrap_or(72.0);
    let icon_x = if icon_asset.is_some() {
        right_x - icon_margins.right - icon_width
    } else {
        right_x
    };

    if let Some(icon_asset) = icon_asset {
        let text_top_correction = font_size as f32 * 0.092;
        nodes.push(RenderNode::new(
            "spell-trap-icon",
            111,
            RenderOp::ImageAsset {
                asset: icon_asset.to_string(),
                x: icon_x,
                y: style.type_top as f32 + icon_margins.top - text_top_correction,
            },
        ));
    }

    let left_width = estimate_text_width(
        &strip_ruby_markup(&left_text),
        language,
        &style.type_font_family,
        font_size as f32,
        style.type_letter_spacing,
    );
    let left_x = icon_x
        - if icon_asset.is_some() {
            icon_margins.left
        } else {
            0.0
        }
        - left_width;

    nodes.push(RenderNode::new(
        "spell-trap-label",
        110,
        RenderOp::TextLine {
            text: left_text,
            rect: RenderRect::from_f32(
                left_x,
                style.type_top as f32,
                left_width.ceil().max(80.0),
                font_size as f32,
            ),
            font_family: style.type_font_family.clone(),
            font_size,
            letter_spacing: style.type_letter_spacing,
            align: TextAlignChoice::Left,
            fill,
            shadow,
            ruby,
            width_compress: false,
        },
    ));
}

fn push_monster_type_node(
    nodes: &mut Vec<RenderNode>,
    style: &crate::layout::LayoutStyle,
    base: &crate::asset_bundle::BaseLayout,
    text: &str,
    options: &RenderOptions,
) {
    let fill = resolve_text_fill(
        &options.text_colors.effect,
        None,
        solid_color_text_paint(TEXT_COLOR_DARK.0, TEXT_COLOR_DARK.1, TEXT_COLOR_DARK.2),
    );
    let shadow = resolve_optional_fill(&options.text_colors.effect_shadow, None);

    nodes.push(RenderNode::new(
        "monster-type-line",
        110,
        RenderOp::TextLine {
            text: text.to_string(),
            rect: RenderRect::new(
                style.effect_x,
                style.effect_top,
                base.effect.width,
                base.effect.height,
            ),
            font_family: style.effect_font_family.clone(),
            font_size: style.effect_size,
            letter_spacing: style.effect_letter_spacing,
            align: TextAlignChoice::Left,
            fill,
            shadow,
            ruby: None,
            width_compress: false,
        },
    ));
}

fn push_pendulum_description_node(
    nodes: &mut Vec<RenderNode>,
    card: &YgoCardMeta,
    style: &crate::layout::LayoutStyle,
    base: &crate::asset_bundle::BaseLayout,
    language: Option<&str>,
    options: &RenderOptions,
) -> String {
    let sections = split_pendulum_description(&card.desc, language);
    if let Some(text) = sections.pendulum_effect {
        let fill = resolve_text_fill(
            &options.text_colors.description,
            options.description_color_override.as_deref(),
            solid_color_text_paint(0, 0, 0),
        );
        let shadow = resolve_optional_fill(&options.text_colors.description_shadow, None);

        nodes.push(RenderNode::new(
            "pendulum-description",
            120,
            RenderOp::TextBlock {
                text,
                rect: RenderRect::new(
                    base.pendulum_description.x,
                    style.pendulum_description_top,
                    base.pendulum_description.width,
                    base.pendulum_description.height,
                ),
                font_family: style.base_font_family.clone(),
                font_size: style.pendulum_description_size,
                line_height: style.pendulum_description_line_height,
                letter_spacing: style.pendulum_description_letter_spacing,
                fill,
                shadow,
                ruby: description_ruby_style(style),
                first_line_compress: options.description_first_line_compress,
            },
        ));
    }
    sections.monster_effect
}

fn push_description_node(
    nodes: &mut Vec<RenderNode>,
    card: &YgoCardMeta,
    style: &crate::layout::LayoutStyle,
    base: &crate::asset_bundle::BaseLayout,
    text: &str,
    options: &RenderOptions,
) {
    let fill = resolve_text_fill(
        &options.text_colors.description,
        options.description_color_override.as_deref(),
        solid_color_text_paint(0, 0, 0),
    );
    let shadow = resolve_optional_fill(&options.text_colors.description_shadow, None);

    nodes.push(RenderNode::new(
        "description",
        130,
        RenderOp::TextBlock {
            text: text.to_string(),
            rect: RenderRect::new(
                style.description_x,
                description_y(card, style),
                style.body_max_width,
                description_height(card, style, base),
            ),
            font_family: style.base_font_family.clone(),
            font_size: style.description_size,
            line_height: style.description_line_height,
            letter_spacing: style.description_letter_spacing,
            fill,
            shadow,
            ruby: description_ruby_style(style),
            first_line_compress: options.description_first_line_compress,
        },
    ));
}

fn push_stats_nodes(
    nodes: &mut Vec<RenderNode>,
    bundle: &AssetBundle,
    request: &RenderRequest,
    style: &crate::layout::LayoutStyle,
    base: &crate::asset_bundle::BaseLayout,
    language: Option<&str>,
) {
    let card = &request.card;

    if card.is_monster() || card.is_pendulum() {
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

    if card.is_monster() {
        let stats_fill = resolve_text_fill(
            &request.options.text_colors.stats,
            None,
            solid_color_text_paint(0, 0, 0),
        );
        let stats_shadow = resolve_optional_fill(&request.options.text_colors.stats_shadow, None);

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
            solid_color_text_paint(TEXT_COLOR_DARK.0, TEXT_COLOR_DARK.1, TEXT_COLOR_DARK.2);
        let stats_fill =
            resolve_text_fill(&request.options.text_colors.stats, None, scale_fill.clone());
        let stats_shadow = resolve_optional_fill(&request.options.text_colors.stats_shadow, None);

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
            },
        ));
    }
}

fn push_password_node(
    nodes: &mut Vec<RenderNode>,
    request: &RenderRequest,
    style: &crate::layout::LayoutStyle,
    base: &crate::asset_bundle::BaseLayout,
) {
    let ov = &request.options.layout_overrides;
    let x = ov.password_x.unwrap_or(base.password.x) as f32;
    let y = ov.password_y.unwrap_or(base.password.y) as f32;

    let fill = resolve_text_fill(
        &request.options.text_colors.password,
        None,
        footer_text_paint(&request.card),
    );
    let shadow = resolve_optional_fill(&request.options.text_colors.password_shadow, None);

    nodes.push(RenderNode::new(
        "password",
        150,
        RenderOp::TextLine {
            text: request.card.code.to_string(),
            rect: RenderRect::from_f32(x, y, 260.0, base.password.font_size as f32),
            font_family: style.password_font_family.clone(),
            font_size: base.password.font_size,
            letter_spacing: 0.0,
            align: TextAlignChoice::Left,
            fill,
            shadow,
            ruby: None,
            width_compress: false,
        },
    ));
}

fn push_scale_line_node(
    nodes: &mut Vec<RenderNode>,
    request: &RenderRequest,
    style: &crate::layout::LayoutStyle,
    base: &crate::asset_bundle::BaseLayout,
) {
    use crate::card_logic::build_scale_line;

    let ov = &request.options.layout_overrides;
    let copyright_right = ov.copyright_right.unwrap_or(base.copyright.right);
    let copyright_y = ov.copyright_y.unwrap_or(base.copyright.y);
    let scale_fill = resolve_text_fill(
        &request.options.text_colors.copyright,
        None,
        footer_text_paint(&request.card),
    );
    let scale_shadow = resolve_optional_fill(&request.options.text_colors.copyright_shadow, None);
    let scale_text = build_scale_line(&request.card);

    nodes.push(RenderNode::new(
        "scale-line",
        148,
        RenderOp::TextLine {
            text: scale_text,
            rect: RenderRect::from_f32(
                (CARD_WIDTH - copyright_right) as f32,
                copyright_y as f32,
                320.0,
                22.0,
            ),
            font_family: style.base_font_family.clone(),
            font_size: 22,
            letter_spacing: 0.0,
            align: TextAlignChoice::Right,
            fill: scale_fill,
            shadow: scale_shadow,
            ruby: None,
            width_compress: false,
        },
    ));
}

fn push_package_node(
    nodes: &mut Vec<RenderNode>,
    request: &RenderRequest,
    style: &crate::layout::LayoutStyle,
    base: &crate::asset_bundle::BaseLayout,
    text: &str,
) {
    let card = &request.card;
    let ov = &request.options.layout_overrides;

    let y = if card.is_pendulum() {
        ov.package_y_pendulum.unwrap_or(base.package.pendulum.y)
    } else if card.is_link() {
        ov.package_y_link.unwrap_or(base.package.link.y)
    } else {
        ov.package_y.unwrap_or(base.package.default.y)
    };

    let right = if card.is_pendulum() {
        base.package
            .pendulum
            .right
            .unwrap_or(base.package.pendulum.x.unwrap_or(116))
    } else if card.is_link() {
        base.package.link.right.unwrap_or(252)
    } else {
        base.package.default.right.unwrap_or(148)
    };

    let explicit_pendulum_x = if card.is_pendulum() {
        base.package.pendulum.x
    } else {
        None
    };

    let x = if let Some(x) = explicit_pendulum_x {
        x as f32
    } else {
        (CARD_WIDTH - right) as f32
    };

    let align = if explicit_pendulum_x.is_some() {
        TextAlignChoice::Left
    } else {
        TextAlignChoice::Right
    };

    let fill = resolve_text_fill(
        &request.options.text_colors.package,
        None,
        solid_color_text_paint(0, 0, 0),
    );
    let shadow = resolve_optional_fill(&request.options.text_colors.package_shadow, None);

    nodes.push(RenderNode::new(
        "package",
        160,
        RenderOp::TextLine {
            text: text.to_string(),
            rect: RenderRect::from_f32(x, y as f32, 400.0, base.package.font_size as f32),
            font_family: style.password_font_family.clone(),
            font_size: base.package.font_size,
            letter_spacing: 0.0,
            align,
            fill,
            shadow,
            ruby: None,
            width_compress: false,
        },
    ));
}

fn push_copyright_node(
    nodes: &mut Vec<RenderNode>,
    bundle: &AssetBundle,
    card: &YgoCardMeta,
    value: &str,
    base: &crate::asset_bundle::BaseLayout,
    style: &crate::layout::LayoutStyle,
    options: &RenderOptions,
) {
    // Try asset path first
    if let Some(asset) = copyright_asset_name(card, value) {
        if let Some(width) = image_width(bundle, &asset) {
            let ov = &options.layout_overrides;
            let right = ov.copyright_right.unwrap_or(base.copyright.right);
            let y = ov.copyright_y.unwrap_or(base.copyright.y);
            nodes.push(RenderNode::new(
                "copyright",
                170,
                RenderOp::ImageAsset {
                    asset,
                    x: CARD_WIDTH.saturating_sub(right + width) as f32,
                    y: y as f32,
                },
            ));
            return;
        }
    }

    // Fallback: render as text
    let ov = &options.layout_overrides;
    let right = ov.copyright_right.unwrap_or(base.copyright.right);
    let y = ov.copyright_y.unwrap_or(base.copyright.y);

    let fill = resolve_text_fill(
        &options.text_colors.copyright,
        None,
        footer_text_paint(card),
    );
    let shadow = resolve_optional_fill(&options.text_colors.copyright_shadow, None);

    nodes.push(RenderNode::new(
        "copyright",
        170,
        RenderOp::TextLine {
            text: value.to_string(),
            rect: RenderRect::from_f32((CARD_WIDTH - right) as f32, y as f32, 500.0, 32.0),
            font_family: style.base_font_family.clone(),
            font_size: 32,
            letter_spacing: 0.0,
            align: TextAlignChoice::Right,
            fill,
            shadow,
            ruby: None,
            width_compress: false,
        },
    ));
}

// ── Title fill/shadow resolution ──────────────────────────────────────────────

fn rare_title_paints(rare: Option<RareType>) -> (Option<TextPaint>, Option<TextPaint>) {
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

fn resolve_effective_title_fill(
    rare_fill: Option<TextPaint>,
    name_color: &NameColor,
    card: &YgoCardMeta,
    override_paint: &Option<TextPaint>,
    legacy_gradient: Option<&TextGradient>,
) -> TextPaint {
    // Rare preset takes highest priority
    if let Some(fill) = rare_fill {
        return fill;
    }
    // User override via text_colors.name
    if let Some(paint) = override_paint {
        return paint.clone();
    }
    // Legacy name_gradient
    if let Some(gradient) = legacy_gradient {
        return TextPaint {
            color: None,
            gradient: Some(gradient.clone()),
        };
    }
    // Fallback to NameColor resolution
    name_color_to_text_paint(name_color, card)
}

fn resolve_effective_title_shadow(
    rare_shadow: Option<TextPaint>,
    override_paint: &Option<TextPaint>,
    legacy_color: Option<&str>,
    legacy_gradient: Option<&TextGradient>,
) -> Option<TextPaint> {
    if let Some(shadow) = rare_shadow {
        return Some(shadow);
    }
    if let Some(paint) = override_paint {
        return Some(paint.clone());
    }
    if let Some(gradient) = legacy_gradient {
        return Some(TextPaint {
            color: legacy_color.map(str::to_string),
            gradient: Some(gradient.clone()),
        });
    }
    legacy_color.map(|c| TextPaint::solid(c))
}

fn name_color_to_text_paint(name_color: &NameColor, card: &YgoCardMeta) -> TextPaint {
    use crate::card_logic::auto_name_light;
    use crate::constants::{NAME_COLOR_DARK, NAME_COLOR_LIGHT};

    match name_color {
        NameColor::Auto => {
            if auto_name_light(card) {
                solid_color_text_paint(NAME_COLOR_LIGHT.0, NAME_COLOR_LIGHT.1, NAME_COLOR_LIGHT.2)
            } else {
                solid_color_text_paint(NAME_COLOR_DARK.0, NAME_COLOR_DARK.1, NAME_COLOR_DARK.2)
            }
        }
        NameColor::Dark => {
            solid_color_text_paint(NAME_COLOR_DARK.0, NAME_COLOR_DARK.1, NAME_COLOR_DARK.2)
        }
        NameColor::Light => {
            solid_color_text_paint(NAME_COLOR_LIGHT.0, NAME_COLOR_LIGHT.1, NAME_COLOR_LIGHT.2)
        }
        NameColor::Custom(hex) => TextPaint::solid(hex),
    }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn resolve_text_fill(
    override_paint: &Option<TextPaint>,
    legacy_color: Option<&str>,
    fallback: TextPaint,
) -> TextPaint {
    if let Some(paint) = override_paint {
        return paint.clone();
    }
    if let Some(color) = legacy_color {
        return TextPaint::solid(color);
    }
    if fallback.has_color_or_gradient() {
        return fallback;
    }
    fallback
}

fn resolve_optional_fill(
    override_paint: &Option<TextPaint>,
    legacy_color: Option<&str>,
) -> Option<TextPaint> {
    let resolved = resolve_text_fill(
        override_paint,
        legacy_color,
        TextPaint {
            color: None,
            gradient: None,
        },
    );
    if resolved.has_color_or_gradient() {
        Some(resolved)
    } else {
        None
    }
}

fn solid_color_text_paint(r: u8, g: u8, b: u8) -> TextPaint {
    TextPaint::solid(format!("#{r:02x}{g:02x}{b:02x}"))
}

fn footer_text_paint(card: &YgoCardMeta) -> TextPaint {
    if footer_uses_light_text(card) {
        solid_color_text_paint(255, 255, 255)
    } else {
        solid_color_text_paint(0, 0, 0)
    }
}

fn footer_uses_light_text(card: &YgoCardMeta) -> bool {
    card.is_monster() && (card.type_ & ygopro_cdb_encode_rs::TYPE_XYZ) != 0
}

fn description_ruby_style(style: &crate::layout::LayoutStyle) -> Option<RubyStyle> {
    if style.description_rt_font_size > 0 {
        Some(RubyStyle {
            rt_font_size: style.description_rt_font_size as f32,
            rt_top: style.description_rt_top,
            rt_font_scale_x: style.description_rt_font_scale_x,
        })
    } else {
        None
    }
}

fn foreground_image_for_request(request: &RenderRequest) -> Option<&PositionedRenderImage> {
    if request.card.out_frame {
        request
            .card
            .out_frame_image
            .as_ref()
            .or(request.options.foreground_image.as_ref())
    } else {
        request.options.foreground_image.as_ref()
    }
}

fn effective_output_scale(request: &RenderRequest) -> f32 {
    let scale = request.card.scale.unwrap_or(request.options.scale);
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

pub(crate) fn laser_asset_name(laser: &str) -> Option<String> {
    let laser = laser.trim();
    if laser.is_empty() {
        None
    } else if laser.ends_with(".webp") {
        Some(laser.to_string())
    } else {
        Some(format!("{laser}.webp"))
    }
}

fn copyright_asset_name(card: &YgoCardMeta, value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let value = value.strip_suffix(".svg").unwrap_or(value);
    let color = if footer_uses_light_text(card) {
        "white"
    } else {
        "black"
    };
    Some(format!("copyright-{value}-{color}.svg"))
}

fn image_width(bundle: &AssetBundle, asset: &str) -> Option<u32> {
    let image = bundle.image(asset).ok()?;
    image
        .size
        .as_ref()
        .map(|size| size.w)
        .or_else(|| image.atlas.as_ref().map(|sprite| sprite.w))
}

struct IconMargins {
    top: f32,
    left: f32,
    right: f32,
}

fn spell_trap_icon_margins(language: Option<&str>, bundle: &AssetBundle) -> IconMargins {
    let icon = bundle
        .layout
        .styles
        .get(language.unwrap_or("sc"))
        .or_else(|| bundle.layout.styles.get("sc"))
        .and_then(|style| style.spell_trap.icon.as_ref());

    IconMargins {
        top: icon.and_then(|i| i.margin_top).unwrap_or(8.0),
        left: icon.and_then(|i| i.margin_left).unwrap_or(0.0),
        right: icon.and_then(|i| i.margin_right).unwrap_or(0.0),
    }
}

// ── RenderNode / RenderCanvas / RenderRect / ImageFit / ImageAlign ──────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderCanvas {
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderNode {
    pub id: String,
    pub z: i32,
    #[serde(default = "default_visible")]
    pub visible: bool,
    #[serde(flatten)]
    pub op: RenderOp,
}

impl RenderNode {
    pub fn new(id: impl Into<String>, z: i32, op: RenderOp) -> Self {
        Self {
            id: id.into(),
            z,
            visible: true,
            op,
        }
    }
}

// ── RenderOp ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RenderOp {
    ImageAsset {
        asset: String,
        x: f32,
        y: f32,
    },
    ExternalImage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<PathBuf>,
        rect: RenderRect,
        fit: ImageFit,
        align: ImageAlign,
    },
    PositionedImage {
        image: PositionedRenderImage,
    },
    FillRect {
        rect: RenderRect,
        color: String,
        opacity: f32,
    },
    TextLine {
        text: String,
        rect: RenderRect,
        font_family: String,
        font_size: u32,
        letter_spacing: f32,
        align: TextAlignChoice,
        fill: TextPaint,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<TextPaint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ruby: Option<RubyStyle>,
        #[serde(default)]
        width_compress: bool,
    },
    TextBlock {
        text: String,
        rect: RenderRect,
        font_family: String,
        font_size: u32,
        line_height: f32,
        letter_spacing: f32,
        fill: TextPaint,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<TextPaint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ruby: Option<RubyStyle>,
        #[serde(default)]
        first_line_compress: bool,
    },
    VisualEffect {
        target: EffectTarget,
        effect: EffectStyle,
    },
    CompositeVisualEffect {
        effect: EffectStyle,
        targets: Vec<EffectTargetWeight>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectTargetWeight {
    pub target: EffectTarget,
    pub opacity: f32,
}

// ── RubyStyle ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RubyStyle {
    pub rt_font_size: f32,
    pub rt_top: f32,
    pub rt_font_scale_x: f32,
}

// ── RenderRect ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RenderRect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x: x as f32,
            y: y as f32,
            width: width as f32,
            height: height as f32,
        }
    }

    pub fn from_f32(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

// ── ImageFit / ImageAlign ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageFit {
    Stretch,
    Cover,
    Contain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageAlign {
    Top,
    Center,
}

// ── EffectTarget / EffectStyle ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EffectTarget {
    Art,
    ArtFrame,
    CardBase,
    CardBorder,
    FullCard,
    Attribute,
    LevelOrRank,
    LinkArrows,
    EffectBoxBorder,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum EffectStyle {
    RainbowFoil { opacity: f32 },
    DotGrid { opacity: f32 },
    OpticalSer { opacity: f32 },
    OpticalSerSimple { opacity: f32 },
    OpticalScr { opacity: f32 },
    OpticalScrSimple { opacity: f32 },
    SecretWeave { opacity: f32 },
    SecretFoil { opacity: f32 },
    Holographic { opacity: f32 },
    BrightBorder { opacity: f32 },
    GoldWash { opacity: f32 },
    FrostedFoil { opacity: f32 },
    ConcentricEngrave { opacity: f32 },
    ReliefEngrave { opacity: f32 },
    DiamondFoil { opacity: f32 },
}

// ── Internal ─────────────────────────────────────────────────────────────────

fn default_visible() -> bool {
    true
}

impl TextPaint {
    fn has_color_or_gradient(&self) -> bool {
        self.color.is_some() || self.gradient.is_some()
    }
}

impl PositionedAsset {
    fn w(&self, bundle: &AssetBundle) -> u32 {
        bundle
            .image(&self.asset)
            .ok()
            .and_then(|e| e.size.as_ref().map(|s| s.w))
            .or_else(|| {
                bundle
                    .image(&self.asset)
                    .ok()
                    .and_then(|e| e.atlas.as_ref().map(|a| a.w))
            })
            .unwrap_or(0)
    }

    fn h(&self, bundle: &AssetBundle) -> u32 {
        bundle
            .image(&self.asset)
            .ok()
            .and_then(|e| e.size.as_ref().map(|s| s.h))
            .or_else(|| {
                bundle
                    .image(&self.asset)
                    .ok()
                    .and_then(|e| e.atlas.as_ref().map(|a| a.h))
            })
            .unwrap_or(0)
    }
}
