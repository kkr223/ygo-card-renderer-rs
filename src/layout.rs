#![allow(dead_code)]

use crate::asset_bundle::TextLayout;
use crate::model::{CardKind, LayoutOverrides};

#[derive(Debug, Clone, Copy)]
pub(crate) struct LayoutStyle {
    pub(crate) base_font_family: &'static str,
    pub(crate) name_font_family: &'static str,
    pub(crate) type_font_family: &'static str,
    pub(crate) effect_font_family: &'static str,
    pub(crate) stat_font_family: &'static str,
    pub(crate) link_font_family: &'static str,
    pub(crate) password_font_family: &'static str,
    pub(crate) name_top: u32,
    pub(crate) name_size: u32,
    pub(crate) type_top: u32,
    pub(crate) type_size: u32,
    pub(crate) effect_top: u32,
    pub(crate) effect_size: u32,
    pub(crate) effect_line_height: f32,
    pub(crate) description_size: u32,
    pub(crate) description_line_height: f32,
    pub(crate) pendulum_description_top: u32,
    pub(crate) pendulum_description_size: u32,
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
}

/// RushDuel 族基准（sc）
const fn rush_duel_base() -> LayoutStyle {
    LayoutStyle {
        base_font_family: "'rd-sc', 'Microsoft YaHei', sans-serif",
        name_font_family: "'rd-sc-name', 'rd-sc', 'Microsoft YaHei', sans-serif",
        type_font_family: "'rd-sc', 'Microsoft YaHei', sans-serif",
        effect_font_family: "'rd-sc', 'Microsoft YaHei', sans-serif",
        stat_font_family: "'rd-atk-def', 'Times New Roman', serif",
        link_font_family: "'rd-atk-def', 'Times New Roman', serif",
        password_font_family: "'rd-tip', 'rd-sc', monospace",
        name_top: 71,
        name_size: 92,
        type_top: 1476,
        type_size: 46,
        effect_top: 1476,
        effect_size: 46,
        effect_line_height: 1.15,
        description_size: 39,
        description_line_height: 1.39,
        pendulum_description_top: 1282,
        pendulum_description_size: 36,
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
    }
}

/// YuGiOh 族基准（sc）
const fn yugioh_base() -> LayoutStyle {
    LayoutStyle {
        base_font_family: "'ygo-sc', 'Microsoft YaHei', sans-serif",
        name_font_family: "'ygo-sc', 'Microsoft YaHei', sans-serif",
        type_font_family: "'ygo-sc', 'Microsoft YaHei', sans-serif",
        effect_font_family: "'ygo-sc', 'Microsoft YaHei', sans-serif",
        stat_font_family: "'ygo-atk-def', 'Times New Roman', serif",
        link_font_family: "'ygo-link', 'ygo-atk-def', 'Times New Roman', serif",
        password_font_family: "'ygo-password', 'ygo-sc', monospace",
        name_top: 108,
        name_size: 108,
        type_top: 254,
        type_size: 76,
        effect_top: 1528,
        effect_size: 44,
        effect_line_height: 1.2,
        description_size: 36,
        description_line_height: 1.2,
        pendulum_description_top: 1282,
        pendulum_description_size: 36,
        title_max_width_with_attribute: 1033,
        title_max_width_without_attribute: 1161,
        body_max_width: 1196,
        title_letter_spacing: 0.0,
        type_letter_spacing: 2.0,
        effect_letter_spacing: 2.0,
        description_letter_spacing: 2.0,
        name_x: 116,
        description_x: 99,
        effect_x: 99,
        effect_text_indent: 0,
        stat_atk_x: 1003,
        stat_def_x: 1286,
        stat_link_x: 1280,
        stat_top: 1833,
        stat_size: 62,
        link_top: 1849,
        link_size: 44,
        stat_letter_spacing: 2.0,
    }
}

/// PSD pt → 像素（基于 96dpi 屏幕，PSD 使用 72pt = 1 inch）
#[inline]
fn pt_to_px(pt: f32) -> u32 {
    (pt * 96.0 / 72.0).round() as u32
}

/// 将 bundle 的 TextLayout 数据应用到 style 上（硬编码值已在 style 中，bundle 覆盖有值的字段）。
fn apply_bundle_text_layout(style: &mut LayoutStyle, tl: &TextLayout) {
    if let Some(name) = &tl.card_name {
        if let Some(x) = name.x {
            style.name_x = x as u32;
        }
        if let Some(w) = name.width {
            // 卡名宽度对应两个 title_max_width；带属性图标时略窄，这里用 PSD 值作为不带属性的上限
            style.title_max_width_without_attribute = w as u32;
            // 带属性版本保持与无属性的比例关系（原硬编码 1033/1161 ≈ 0.89）
            style.title_max_width_with_attribute = (w * 1033.0 / 1161.0).round() as u32;
        }
    }
    if let Some(eff) = &tl.effect_text {
        if let Some(x) = eff.x {
            style.effect_x = x as u32;
        }
        // NOTE: PSD の effect_text.y は文字枠上辺（行距込み）のため JS 参考値より約 16px 下にずれる。
        // effect_top は各言語のハードコード値を維持し、PSD の y 値では上書きしない。
        if let Some(w) = eff.width {
            style.body_max_width = w as u32;
        }
        if let Some(lhr) = eff.line_height_ratio {
            style.effect_line_height = lhr;
        }
    }
    if let Some(desc) = &tl.description_text {
        if let Some(x) = desc.x {
            style.description_x = x as u32;
        }
        if let Some(lhr) = desc.line_height_ratio {
            style.description_line_height = lhr;
        }
        // body_max_width 以 effect_text 为准（更宽），description 宽度略有差异时不覆盖
    }
    // NOTE: PSD の type_line は効果枠内の種族行（y≈1464）であり、
    // spell/trap カード名直下の type_top（y≈253）とは別物なので、ここでは適用しない。
    // type_top は各言語の hardcoded 値を維持する。
    let _ = &tl.type_line; // suppress unused warning
}

/// 将外部 LayoutOverrides 应用到 style 上（最高优先级）。
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

/// 构建最终 LayoutStyle。优先级：overrides > bundle text_layout > 硬编码默认值。
pub(crate) fn layout_style(
    kind: CardKind,
    language: Option<&str>,
    bundle_text_layout: Option<&TextLayout>,
    overrides: &LayoutOverrides,
) -> LayoutStyle {
    let mut style = match (kind, language.unwrap_or("sc")) {
        (CardKind::RushDuel, "jp") => {
            let mut s = rush_duel_base();
            s.base_font_family = "'rd-jp', 'Yu Gothic', sans-serif";
            s.name_font_family = "'rd-jp-name', 'rd-jp', 'Yu Gothic', sans-serif";
            s.type_font_family = "'rd-jp-effect', 'rd-jp', 'Yu Gothic', sans-serif";
            s.effect_font_family = "'rd-jp-effect', 'rd-jp', 'Yu Gothic', sans-serif";
            s.password_font_family = "'rd-tip', 'rd-jp', monospace";
            s
        }
        (CardKind::RushDuel, _) => rush_duel_base(),
        (_, "en") => {
            let mut s = yugioh_base();
            s.base_font_family = "'ygo-en', 'Arial', sans-serif";
            s.name_font_family = "'ygo-en-name', 'ygo-en', 'Arial', sans-serif";
            s.type_font_family = "'ygo-en-race', 'ygo-en', 'Arial', sans-serif";
            s.effect_font_family = "'ygo-en-race', 'ygo-en', 'Arial', sans-serif";
            s.password_font_family = "'ygo-password', 'ygo-en', monospace";
            s.name_top = 52;
            s.name_size = 158;
            s.type_size = 74;
            s.effect_top = 1527;
            s.effect_size = 56;
            s.effect_line_height = 1.02;
            s.description_size = 42;
            s.description_line_height = 1.02;
            s.pendulum_description_size = 42;
            s.body_max_width = 1175;
            s.title_letter_spacing = 1.0;
            s.type_letter_spacing = 1.0;
            s.effect_letter_spacing = 1.0;
            s.description_letter_spacing = 0.0;
            s.description_x = 109;
            s.effect_x = 109;
            s.stat_size = 58;
            s
        }
        (_, "jp") => {
            let mut s = yugioh_base();
            s.base_font_family = "'ygo-jp', 'Yu Gothic', sans-serif";
            s.name_font_family = "'ygo-jp', 'Yu Gothic', sans-serif";
            s.type_font_family = "'ygo-jp', 'Yu Gothic', sans-serif";
            s.effect_font_family = "'ygo-jp', 'Yu Gothic', sans-serif";
            s.password_font_family = "'ygo-password', 'ygo-jp', monospace";
            s.name_top = 98;
            s.type_top = 253;
            s.type_size = 80;
            s.effect_size = 46;
            s.effect_line_height = 1.17;
            s.description_size = 38;
            s.description_line_height = 1.17;
            s.pendulum_description_top = 1288;
            s.body_max_width = 1175;
            s.title_letter_spacing = 0.0;
            s.type_letter_spacing = 0.0;
            s.effect_letter_spacing = 0.0;
            s.description_letter_spacing = 0.0;
            s.description_x = 109;
            s.effect_x = 109;
            s.effect_text_indent = -9;
            s.stat_size = 58;
            s
        }
        _ => yugioh_base(),
    };

    // 第二层：bundle 中从 PSD 提取的数据
    if let Some(tl) = bundle_text_layout {
        apply_bundle_text_layout(&mut style, tl);
    }

    // 第三层：调用方显式覆盖（最高优先级）
    apply_overrides(&mut style, overrides);

    style
}
