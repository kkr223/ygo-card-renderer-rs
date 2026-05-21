use crate::{
    asset_bundle::{AssetBundle, BaseLayout},
    constants::CARD_WIDTH,
    layout::LayoutStyle,
    model::{RenderOptions, RenderRequest, TextAlignChoice, YgoCardMeta},
};

use super::super::paint;
use super::super::{RenderNode, RenderOp, RenderRect};

// ── Password ─────────────────────────────────────────────────────────────────

pub(crate) fn push_password_node(
    nodes: &mut Vec<RenderNode>,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
) {
    let ov = &request.options.layout_overrides;
    let x = ov.password_x.unwrap_or(base.password.x) as f32;
    let y = ov.password_y.unwrap_or(base.password.y) as f32;

    let fill = paint::resolve_text_fill(
        &request.options.text_colors.password,
        None,
        paint::footer_text_paint(&request.card),
    );
    let shadow = paint::resolve_optional_fill(&request.options.text_colors.password_shadow, None);

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
            font_weight: None,
        },
    ));
}

// ── Package ──────────────────────────────────────────────────────────────────

pub(crate) fn push_package_node(
    nodes: &mut Vec<RenderNode>,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
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

    let fill = paint::resolve_text_fill(
        &request.options.text_colors.package,
        None,
        paint::solid_color_text_paint(0, 0, 0),
    );
    let shadow = paint::resolve_optional_fill(&request.options.text_colors.package_shadow, None);

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
            font_weight: None,
        },
    ));
}

// ── Copyright ────────────────────────────────────────────────────────────────

pub(crate) fn push_copyright_node(
    nodes: &mut Vec<RenderNode>,
    bundle: &AssetBundle,
    card: &YgoCardMeta,
    value: &str,
    base: &BaseLayout,
    style: &LayoutStyle,
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

    let fill = paint::resolve_text_fill(
        &options.text_colors.copyright,
        None,
        paint::footer_text_paint(card),
    );
    let shadow = paint::resolve_optional_fill(&options.text_colors.copyright_shadow, None);

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
            font_weight: None,
        },
    ));
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn copyright_asset_name(card: &YgoCardMeta, value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let value = value.strip_suffix(".svg").unwrap_or(value);
    let color = if paint::footer_uses_light_text(card) {
        "white"
    } else {
        "black"
    };
    Some(format!("copyright-{value}-{color}.svg"))
}

pub(crate) fn image_width(bundle: &AssetBundle, asset: &str) -> Option<u32> {
    let image = bundle.image(asset).ok()?;
    image
        .size
        .as_ref()
        .map(|size| size.w)
        .or_else(|| image.atlas.as_ref().map(|sprite| sprite.w))
}
