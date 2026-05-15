use crate::{
    card_logic::auto_name_light,
    constants::{NAME_COLOR_DARK, NAME_COLOR_LIGHT},
    model::{NameColor, TextGradient, TextPaint, YgoCardMeta},
};

use super::RubyStyle;

// ── Title fill/shadow resolution ──────────────────────────────────────────────

pub(super) use super::rare::rare_title_paints;

pub(super) fn resolve_effective_title_fill(
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

pub(super) fn resolve_effective_title_shadow(
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

pub(super) fn resolve_text_fill(
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

pub(super) fn resolve_optional_fill(
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

pub(super) fn solid_color_text_paint(r: u8, g: u8, b: u8) -> TextPaint {
    TextPaint::solid(format!("#{r:02x}{g:02x}{b:02x}"))
}

pub(super) fn footer_text_paint(card: &YgoCardMeta) -> TextPaint {
    if footer_uses_light_text(card) {
        solid_color_text_paint(255, 255, 255)
    } else {
        solid_color_text_paint(0, 0, 0)
    }
}

pub(super) fn footer_uses_light_text(card: &YgoCardMeta) -> bool {
    crate::facts::CardFacts::new(card).footer_is_light
}

pub(super) fn description_ruby_style(style: &crate::layout::LayoutStyle) -> Option<RubyStyle> {
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

// ── TextPaint extension ───────────────────────────────────────────────────────

impl TextPaint {
    fn has_color_or_gradient(&self) -> bool {
        self.color.is_some() || self.gradient.is_some()
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ygopro_cdb_encode_rs::CardDataEntry;

    #[test]
    fn resolve_text_fill_override_takes_highest_priority() {
        let fallback = solid_color_text_paint(0, 0, 0);
        let override_paint = Some(TextPaint::solid("#ff0000"));
        let result = resolve_text_fill(&override_paint, Some("#00ff00"), fallback);
        assert_eq!(result.color.as_deref(), Some("#ff0000"));
    }

    #[test]
    fn resolve_text_fill_legacy_color_second_priority() {
        let fallback = solid_color_text_paint(0, 0, 0);
        let result = resolve_text_fill(&None, Some("#00ff00"), fallback);
        assert_eq!(result.color.as_deref(), Some("#00ff00"));
    }

    #[test]
    fn resolve_text_fill_fallback_last() {
        let fallback = solid_color_text_paint(0, 0, 0);
        let result = resolve_text_fill(&None, None, fallback);
        assert_eq!(result.color.as_deref(), Some("#000000"));
    }

    #[test]
    fn resolve_optional_fill_returns_none_when_no_source() {
        assert_eq!(resolve_optional_fill(&None, None), None);
    }

    #[test]
    fn resolve_optional_fill_returns_some_when_override() {
        let result = resolve_optional_fill(&Some(TextPaint::solid("#ff0000")), None);
        assert!(result.is_some());
    }

    #[test]
    fn footer_uses_light_for_xyz() {
        let card: YgoCardMeta = CardDataEntry {
            code: 1,
            name: "Xyz".to_string(),
            desc: "test".to_string(),
            type_: ygopro_cdb_encode_rs::TYPE_MONSTER | ygopro_cdb_encode_rs::TYPE_XYZ,
            ..CardDataEntry::default()
        }
        .into();
        assert!(footer_uses_light_text(&card));
    }

    #[test]
    fn footer_uses_dark_for_normal_monster() {
        let card: YgoCardMeta = CardDataEntry {
            code: 1,
            name: "Normal".to_string(),
            desc: "test".to_string(),
            type_: ygopro_cdb_encode_rs::TYPE_MONSTER,
            ..CardDataEntry::default()
        }
        .into();
        assert!(!footer_uses_light_text(&card));
    }

    #[test]
    fn footer_text_paint_white_for_xyz_black_for_normal() {
        let xyz: YgoCardMeta = CardDataEntry {
            code: 1,
            name: "X".to_string(),
            desc: "t".to_string(),
            type_: ygopro_cdb_encode_rs::TYPE_MONSTER | ygopro_cdb_encode_rs::TYPE_XYZ,
            ..CardDataEntry::default()
        }
        .into();
        assert_eq!(footer_text_paint(&xyz).color.as_deref(), Some("#ffffff"));

        let normal: YgoCardMeta = CardDataEntry {
            code: 1,
            name: "N".to_string(),
            desc: "t".to_string(),
            type_: ygopro_cdb_encode_rs::TYPE_MONSTER,
            ..CardDataEntry::default()
        }
        .into();
        assert_eq!(footer_text_paint(&normal).color.as_deref(), Some("#000000"));
    }

    #[test]
    fn solid_color_text_paint_produces_hex_color() {
        let paint = solid_color_text_paint(255, 0, 128);
        assert_eq!(paint.color.as_deref(), Some("#ff0080"));
        assert!(paint.gradient.is_none());
    }
}
