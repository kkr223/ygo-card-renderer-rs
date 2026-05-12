use crate::asset_bundle::{LayoutPayload, StyleDefinition};
use crate::model::{CardKind, LayoutOverrides};

#[derive(Debug, Clone)]
pub(crate) struct LayoutStyle {
    pub(crate) base_font_family: String,
    pub(crate) name_font_family: String,
    pub(crate) type_font_family: String,
    pub(crate) effect_font_family: String,
    pub(crate) stat_font_family: String,
    pub(crate) link_font_family: String,
    pub(crate) password_font_family: String,
    pub(crate) name_top: u32,
    pub(crate) name_size: u32,
    pub(crate) type_top: u32,
    pub(crate) type_size: u32,
    pub(crate) type_right: u32,
    pub(crate) effect_top: u32,
    pub(crate) effect_size: u32,
    pub(crate) effect_line_height: f32,
    pub(crate) effect_min_height: u32,
    pub(crate) description_size: u32,
    pub(crate) description_line_height: f32,
    pub(crate) pendulum_description_top: u32,
    pub(crate) pendulum_description_size: u32,
    pub(crate) pendulum_description_line_height: f32,
    pub(crate) pendulum_description_letter_spacing: f32,
    pub(crate) title_max_width_with_attribute: u32,
    pub(crate) title_max_width_without_attribute: u32,
    pub(crate) body_max_width: u32,
    pub(crate) title_letter_spacing: f32,
    pub(crate) type_letter_spacing: f32,
    pub(crate) effect_letter_spacing: f32,
    pub(crate) description_letter_spacing: f32,
    pub(crate) name_x: u32,
    pub(crate) description_x: u32,
    pub(crate) effect_x: u32,
    pub(crate) effect_text_indent: i32,
    pub(crate) stat_atk_x: u32,
    pub(crate) stat_def_x: u32,
    pub(crate) stat_link_x: u32,
    pub(crate) stat_top: u32,
    pub(crate) stat_size: u32,
    pub(crate) link_top: u32,
    pub(crate) link_size: u32,
    pub(crate) stat_letter_spacing: f32,

    // Ruby/furigana annotation parameters (0 = disabled / non-JP language).
    pub(crate) name_rt_font_size: u32,
    pub(crate) name_rt_top: f32,
    pub(crate) name_rt_font_scale_x: f32,
    pub(crate) type_rt_font_size: u32,
    pub(crate) type_rt_top: f32,
    pub(crate) type_rt_font_scale_x: f32,
    pub(crate) effect_rt_font_size: u32,
    pub(crate) effect_rt_top: f32,
    pub(crate) effect_rt_font_scale_x: f32,
    pub(crate) description_rt_font_size: u32,
    pub(crate) description_rt_top: f32,
    pub(crate) description_rt_font_scale_x: f32,
}

fn quoted_font(name: &str) -> String {
    format!("'{name}'")
}

fn style_for_language<'a>(layout: &'a LayoutPayload, language: &str) -> &'a StyleDefinition {
    layout
        .styles
        .get(language)
        .or_else(|| layout.styles.get("sc"))
        .expect("layout styles missing default 'sc'")
}

fn text_indent_px(value: Option<f32>) -> i32 {
    value.unwrap_or(0.0).round() as i32
}

/// Extract the three RT (ruby text) layout parameters from a `TextBlock`.
fn text_block_rt(block: &crate::asset_bundle::TextBlock) -> (u32, f32, f32) {
    (
        block.rt_font_size.unwrap_or(0),
        block.rt_top.unwrap_or(0) as f32,
        block.rt_font_scale_x.unwrap_or(1.0),
    )
}

fn apply_overrides(style: &mut LayoutStyle, ov: &LayoutOverrides) {
    macro_rules! apply {
        ($field:ident) => {
            if let Some(v) = ov.$field {
                style.$field = v;
            }
        };
    }
    apply!(name_top);
    apply!(name_size);
    apply!(name_x);
    apply!(title_max_width_with_attribute);
    apply!(title_max_width_without_attribute);
    apply!(title_letter_spacing);
    apply!(type_top);
    apply!(type_size);
    apply!(type_letter_spacing);
    apply!(effect_top);
    apply!(effect_size);
    apply!(effect_line_height);
    apply!(effect_x);
    apply!(effect_letter_spacing);
    apply!(effect_text_indent);
    apply!(description_size);
    apply!(description_line_height);
    apply!(description_x);
    apply!(description_letter_spacing);
    apply!(body_max_width);
    apply!(pendulum_description_top);
    apply!(pendulum_description_size);
    apply!(stat_atk_x);
    apply!(stat_def_x);
    apply!(stat_link_x);
    apply!(stat_top);
    apply!(stat_size);
    apply!(stat_letter_spacing);
    apply!(link_top);
    apply!(link_size);
}

pub(crate) fn layout_style(
    kind: CardKind,
    language: Option<&str>,
    bundle_layout: &LayoutPayload,
    overrides: &LayoutOverrides,
) -> LayoutStyle {
    let lang = language.unwrap_or("sc");
    let style = style_for_language(bundle_layout, lang);
    let atk_text = if lang == "astral" {
        &bundle_layout.base.atk_def_link.atk.astral
    } else {
        &bundle_layout.base.atk_def_link.atk.default
    };
    let def_text = if lang == "astral" {
        &bundle_layout.base.atk_def_link.def.astral
    } else {
        &bundle_layout.base.atk_def_link.def.default
    };
    let link_text = if lang == "astral" {
        &bundle_layout.base.atk_def_link.link.astral
    } else {
        &bundle_layout.base.atk_def_link.link.default
    };

    let stat_font_family = if lang == "astral" {
        quoted_font("ygo-astral")
    } else {
        quoted_font("ygo-atk-def")
    };
    let link_font_family = if lang == "astral" {
        quoted_font("ygo-astral")
    } else {
        quoted_font("ygo-link")
    };

    // Font families and RT parameters are identical in both card-kind arms; compute them once.
    let base_font_family = quoted_font(&style.font_family);
    let name_font_family = quoted_font(
        style
            .name
            .font_family
            .as_deref()
            .unwrap_or(&style.font_family),
    );
    let type_font_family = quoted_font(
        style
            .spell_trap
            .font_family
            .as_deref()
            .unwrap_or(&style.font_family),
    );
    let effect_font_family = quoted_font(
        style
            .effect
            .font_family
            .as_deref()
            .unwrap_or(&style.font_family),
    );

    let (name_rt_font_size, name_rt_top, name_rt_font_scale_x) = text_block_rt(&style.name);
    let (type_rt_font_size, type_rt_top, type_rt_font_scale_x) = text_block_rt(&style.spell_trap);
    let (effect_rt_font_size, effect_rt_top, effect_rt_font_scale_x) = text_block_rt(&style.effect);
    let (description_rt_font_size, description_rt_top, description_rt_font_scale_x) =
        text_block_rt(&style.description);

    let mut layout = match kind {
        CardKind::Yugioh => LayoutStyle {
            base_font_family,
            name_font_family,
            type_font_family,
            effect_font_family,
            stat_font_family,
            link_font_family,
            password_font_family: quoted_font(&bundle_layout.base.password.font_family),
            name_top: style.name.top.unwrap_or(97),
            name_size: style.name.font_size.unwrap_or(108),
            type_top: style.spell_trap.top.unwrap_or(254),
            type_size: style.spell_trap.font_size.unwrap_or(76),
            type_right: style.spell_trap.right.unwrap_or(134),
            effect_top: style.effect.top.unwrap_or(1528),
            effect_size: style.effect.font_size.unwrap_or(44),
            effect_line_height: style.effect.line_height.unwrap_or(1.2),
            effect_min_height: style.effect.min_height.unwrap_or(0),
            description_size: style.description.font_size.unwrap_or(36),
            description_line_height: style.description.line_height.unwrap_or(1.2),
            pendulum_description_top: style.pendulum_description.top.unwrap_or(1282),
            pendulum_description_size: style.pendulum_description.font_size.unwrap_or(36),
            pendulum_description_line_height: style.pendulum_description.line_height.unwrap_or(1.2),
            pendulum_description_letter_spacing: style
                .pendulum_description
                .letter_spacing
                .unwrap_or(0.0),
            title_max_width_with_attribute: bundle_layout.base.name.width_with_attribute,
            title_max_width_without_attribute: bundle_layout.base.name.width_without_attribute,
            body_max_width: bundle_layout.base.description.width,
            title_letter_spacing: style.name.letter_spacing.unwrap_or(0.0),
            type_letter_spacing: style.spell_trap.letter_spacing.unwrap_or(0.0),
            effect_letter_spacing: style.effect.letter_spacing.unwrap_or(0.0),
            description_letter_spacing: style.description.letter_spacing.unwrap_or(0.0),
            name_x: bundle_layout.base.name.x,
            description_x: bundle_layout.base.description.x,
            effect_x: ((bundle_layout.base.effect.x as i32)
                + text_indent_px(style.effect.text_indent))
            .max(0) as u32,
            effect_text_indent: text_indent_px(style.effect.text_indent),
            stat_atk_x: atk_text.x,
            stat_def_x: def_text.x,
            stat_link_x: link_text.x,
            stat_top: atk_text.y,
            stat_size: atk_text.font_size,
            link_top: link_text.y,
            link_size: link_text.font_size,
            stat_letter_spacing: if lang == "astral" { 0.0 } else { 2.0 },
            name_rt_font_size,
            name_rt_top,
            name_rt_font_scale_x,
            type_rt_font_size,
            type_rt_top,
            type_rt_font_scale_x,
            effect_rt_font_size,
            effect_rt_top,
            effect_rt_font_scale_x,
            description_rt_font_size,
            description_rt_top,
            description_rt_font_scale_x,
        },
        CardKind::RushDuel => LayoutStyle {
            base_font_family,
            name_font_family,
            type_font_family,
            effect_font_family,
            stat_font_family: quoted_font("rd-atk-def"),
            link_font_family: quoted_font("rd-atk-def"),
            password_font_family: quoted_font("rd-tip"),
            name_top: 71,
            name_size: 92,
            type_top: 1476,
            type_size: 46,
            type_right: 134,
            effect_top: 1476,
            effect_size: 46,
            effect_line_height: 1.15,
            effect_min_height: 0,
            description_size: 39,
            description_line_height: 1.39,
            pendulum_description_top: 1282,
            pendulum_description_size: 36,
            pendulum_description_line_height: 1.2,
            pendulum_description_letter_spacing: 0.0,
            title_max_width_with_attribute: 1033,
            title_max_width_without_attribute: 1161,
            body_max_width: 1175,
            title_letter_spacing: 0.0,
            type_letter_spacing: 2.0,
            effect_letter_spacing: 2.0,
            description_letter_spacing: 0.0,
            name_x: 116,
            description_x: 126,
            effect_x: 126,
            effect_text_indent: 0,
            stat_atk_x: 1028,
            stat_def_x: 1306,
            stat_link_x: 1294,
            stat_top: 1804,
            stat_size: 54,
            link_top: 1820,
            link_size: 42,
            stat_letter_spacing: 1.5,
            name_rt_font_size,
            name_rt_top,
            name_rt_font_scale_x,
            type_rt_font_size,
            type_rt_top,
            type_rt_font_scale_x,
            effect_rt_font_size,
            effect_rt_top,
            effect_rt_font_scale_x,
            description_rt_font_size,
            description_rt_top,
            description_rt_font_scale_x,
        },
    };

    apply_overrides(&mut layout, overrides);
    layout
}
