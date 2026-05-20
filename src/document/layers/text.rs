use crate::{
    asset_bundle::{AssetBundle, BaseLayout},
    card_logic::{description_height, description_y, split_pendulum_description},
    constants::{CARD_WIDTH, TEXT_COLOR_DARK},
    layout::LayoutStyle,
    model::{RenderOptions, RenderRequest, TextAlignChoice, YgoCardMeta},
    ruby::strip_ruby_markup,
    text::estimate_text_width,
};

use ygopro_cdb_encode_rs::{TYPE_FUSION, TYPE_LINK, TYPE_SYNCHRO, TYPE_XYZ};

use super::super::paint;
use super::super::{RenderNode, RenderOp, RenderRect, RubyStyle};
use super::footer;

// ── Title ────────────────────────────────────────────────────────────────────

pub(crate) fn push_title_node(
    nodes: &mut Vec<RenderNode>,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
) {
    let card = &request.card;
    let show_attr = card.attribute != 0 || card.is_spell() || card.is_trap();
    let title_width = if show_attr {
        style.title_max_width_with_attribute
    } else {
        style.title_max_width_without_attribute
    };

    let (rare_fill, rare_shadow) = paint::rare_title_paints(card.rare);

    // Resolve effective fill
    let fill = paint::resolve_effective_title_fill(
        rare_fill,
        &card.name_color,
        card,
        &request.options.text_colors.name,
        card.name_gradient.as_ref(),
    );

    // Resolve effective shadow
    let shadow = paint::resolve_effective_title_shadow(
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
            font_family: request
                .options
                .font
                .as_deref()
                .filter(|font| !font.trim().is_empty())
                .map(|font| format!("'{font}'"))
                .unwrap_or_else(|| style.name_font_family.clone()),
            font_size: style.name_size,
            letter_spacing: style.title_letter_spacing,
            align: request.options.align.unwrap_or(TextAlignChoice::Left),
            fill,
            shadow,
            ruby,
            width_compress: request.options.title_width_compress,
            font_weight: None,
        },
    ));
}

// ── Spell / Trap label + icon ────────────────────────────────────────────────

pub(crate) fn push_spell_trap_nodes(
    nodes: &mut Vec<RenderNode>,
    bundle: &AssetBundle,
    request: &RenderRequest,
    style: &LayoutStyle,
    language: Option<&str>,
) {
    use crate::card_logic::{
        localized_brackets, localized_spell_trap_name, spell_trap_subtype_icon_asset,
    };

    let card = &request.card;
    let (left_bracket, right_bracket) = localized_brackets(language);
    let left_text = format!(
        "{left_bracket}{}",
        localized_spell_trap_name(card, language)
    );
    let font_size = style.type_size;

    let fill = paint::resolve_text_fill(
        &request.options.text_colors.type_line,
        None,
        paint::solid_color_text_paint(
            crate::constants::TYPE_COLOR.0,
            crate::constants::TYPE_COLOR.1,
            crate::constants::TYPE_COLOR.2,
        ),
    );
    let shadow = paint::resolve_optional_fill(&request.options.text_colors.type_line_shadow, None);

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
            font_weight: None,
        },
    ));

    let icon_asset = spell_trap_subtype_icon_asset(card).filter(|asset| bundle.has_image(asset));
    let icon_margins = spell_trap_icon_margins(language, bundle);
    let icon_width = icon_asset
        .and_then(|asset| footer::image_width(bundle, asset).map(|width| width as f32))
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
            font_weight: None,
        },
    ));
}

// ── Monster type line ────────────────────────────────────────────────────────

pub(crate) fn push_monster_type_node(
    nodes: &mut Vec<RenderNode>,
    style: &LayoutStyle,
    base: &BaseLayout,
    text: &str,
    options: &RenderOptions,
) {
    let fill = paint::resolve_text_fill(
        &options.text_colors.effect,
        None,
        paint::solid_color_text_paint(TEXT_COLOR_DARK.0, TEXT_COLOR_DARK.1, TEXT_COLOR_DARK.2),
    );
    let shadow = paint::resolve_optional_fill(&options.text_colors.effect_shadow, None);

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
            font_weight: None,
        },
    ));
}

// ── Pendulum description ─────────────────────────────────────────────────────

pub(crate) fn push_pendulum_description_node(
    nodes: &mut Vec<RenderNode>,
    card: &YgoCardMeta,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
    options: &RenderOptions,
) -> String {
    let sections = split_pendulum_description(&card.desc, language);
    if let Some(text) = sections.pendulum_effect {
        let fill = paint::resolve_text_fill(
            &options.text_colors.description,
            options.description_color_override.as_deref(),
            paint::solid_color_text_paint(0, 0, 0),
        );
        let shadow = paint::resolve_optional_fill(&options.text_colors.description_shadow, None);

        let mut tmp = String::new();
        let (formatted_text, first_line_compress) = apply_format_mode(&text, &mut tmp, card, options);

        nodes.push(RenderNode::new(
            "pendulum-description",
            120,
            RenderOp::TextBlock {
                text: formatted_text.to_string(),
                rect: RenderRect::new(
                    base.pendulum_description.x,
                    style.pendulum_description_top,
                    base.pendulum_description.width,
                    base.pendulum_description.height,
                ),
                font_family: style.base_font_family.clone(),
                font_size: zoomed_font_size(
                    style.pendulum_description_size,
                    options.description_zoom,
                ),
                line_height: style.pendulum_description_line_height,
                letter_spacing: style.pendulum_description_letter_spacing,
                fill,
                shadow,
                ruby: paint::description_ruby_style(style),
                first_line_compress,
                align: options.description_align.unwrap_or(TextAlignChoice::Left),
                font_weight: options.description_weight,
            },
        ));
    }
    sections.monster_effect
}

// ── Effect / description ─────────────────────────────────────────────────────

/// Apply `format_text` mode: compact newlines and determine `first_line_compress`.
///
/// When `options.format_text` is true:
/// - Extra-deck monsters (Fusion/Synchro/Xyz/Link): keep the first newline,
///   strip the rest, and enable first-line compression.
/// - All other cards: strip all newlines.
fn apply_format_mode<'a>(
    text: &'a str,
    tmp: &'a mut String,
    card: &YgoCardMeta,
    options: &RenderOptions,
) -> (&'a str, bool) {
    if !options.format_text {
        return (text, options.description_first_line_compress);
    }
    let t = card.type_;
    let is_extra =
        (t & TYPE_FUSION) != 0 || (t & TYPE_SYNCHRO) != 0 || (t & TYPE_XYZ) != 0 || (t & TYPE_LINK) != 0;

    tmp.clear();
    if is_extra {
        // Keep first newline, remove the rest.
        if let Some(pos) = text.find('\n') {
            tmp.push_str(&text[..=pos]);
            tmp.push_str(&text[pos + 1..].replace('\n', ""));
        } else {
            tmp.push_str(text);
        }
    } else {
        // Remove all newlines.
        tmp.push_str(&text.replace('\n', ""));
    }
    (tmp.as_str(), is_extra)
}

pub(crate) fn push_description_node(
    nodes: &mut Vec<RenderNode>,
    card: &YgoCardMeta,
    style: &LayoutStyle,
    base: &BaseLayout,
    text: &str,
    options: &RenderOptions,
) {
    let fill = paint::resolve_text_fill(
        &options.text_colors.description,
        options.description_color_override.as_deref(),
        paint::solid_color_text_paint(0, 0, 0),
    );
    let shadow = paint::resolve_optional_fill(&options.text_colors.description_shadow, None);

    let mut tmp = String::new();
    let (formatted_text, first_line_compress) = apply_format_mode(text, &mut tmp, card, options);

    nodes.push(RenderNode::new(
        "description",
        130,
        RenderOp::TextBlock {
            text: formatted_text.to_string(),
            rect: RenderRect::new(
                style.description_x,
                description_y(card, style),
                style.body_max_width,
                description_height(card, style, base),
            ),
            font_family: style.base_font_family.clone(),
            font_size: zoomed_font_size(style.description_size, options.description_zoom),
            line_height: style.description_line_height,
            letter_spacing: style.description_letter_spacing,
            fill,
            shadow,
            ruby: paint::description_ruby_style(style),
            first_line_compress,
            align: options.description_align.unwrap_or(TextAlignChoice::Left),
            font_weight: options.description_weight,
        },
    ));
}

fn zoomed_font_size(base: u32, zoom: Option<f32>) -> u32 {
    let zoom = zoom
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1.0);
    ((base as f32 * zoom).round() as u32).max(1)
}

// ── Spell/trap icon margins helper ───────────────────────────────────────────

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
