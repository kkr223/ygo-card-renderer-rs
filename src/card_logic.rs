use ygopro_cdb_encode_rs::{
    CardDataEntry, TYPE_FUSION, TYPE_LINK, TYPE_MONSTER, TYPE_PENDULUM, TYPE_RITUAL, TYPE_SYNCHRO,
    TYPE_XYZ,
};

use crate::{
    constants::CARD_WIDTH,
    layout::LayoutStyle,
    model::{CardKind, RenderError},
};

pub(crate) fn background_rel_path_yugioh(card: &CardDataEntry) -> &'static str {
    if card.is_spell() {
        "yugioh/image/card-spell.png"
    } else if card.is_trap() {
        "yugioh/image/card-trap.png"
    } else if (card.type_ & TYPE_LINK) != 0 {
        "yugioh/image/card-link.png"
    } else if (card.type_ & TYPE_XYZ) != 0 && (card.type_ & TYPE_PENDULUM) != 0 {
        "yugioh/image/card-xyz-pendulum.png"
    } else if (card.type_ & TYPE_XYZ) != 0 {
        "yugioh/image/card-xyz.png"
    } else if (card.type_ & TYPE_SYNCHRO) != 0 && (card.type_ & TYPE_PENDULUM) != 0 {
        "yugioh/image/card-synchro-pendulum.png"
    } else if (card.type_ & TYPE_SYNCHRO) != 0 {
        "yugioh/image/card-synchro.png"
    } else if (card.type_ & TYPE_FUSION) != 0 && (card.type_ & TYPE_PENDULUM) != 0 {
        "yugioh/image/card-fusion-pendulum.png"
    } else if (card.type_ & TYPE_FUSION) != 0 {
        "yugioh/image/card-fusion.png"
    } else if (card.type_ & TYPE_RITUAL) != 0 && (card.type_ & TYPE_PENDULUM) != 0 {
        "yugioh/image/card-ritual-pendulum.png"
    } else if (card.type_ & TYPE_RITUAL) != 0 {
        "yugioh/image/card-ritual.png"
    } else if (card.type_ & TYPE_PENDULUM) != 0 {
        "yugioh/image/card-effect-pendulum.png"
    } else if (card.type_ & TYPE_MONSTER) != 0 {
        "yugioh/image/card-effect.png"
    } else {
        "yugioh/image/card-normal.png"
    }
}

pub(crate) fn background_rel_path_rush(card: &CardDataEntry) -> &'static str {
    if card.is_spell() {
        "rush-duel/image/card-spell.png"
    } else if card.is_trap() {
        "rush-duel/image/card-trap.png"
    } else if (card.type_ & TYPE_FUSION) != 0 {
        "rush-duel/image/card-fusion.png"
    } else if (card.type_ & TYPE_RITUAL) != 0 {
        "rush-duel/image/card-ritual.png"
    } else if (card.type_ & TYPE_MONSTER) != 0 && (card.type_ & 0x20) != 0 {
        "rush-duel/image/card-effect.png"
    } else {
        "rush-duel/image/card-normal.png"
    }
}

pub(crate) fn build_primary_line(card: &CardDataEntry, kind: CardKind) -> String {
    if card.is_spell() {
        "【魔法卡】".to_string()
    } else if card.is_trap() {
        "【陷阱卡】".to_string()
    } else {
        let mut tags = Vec::new();
        if (card.type_ & TYPE_LINK) != 0 {
            tags.push("连接");
        } else if (card.type_ & TYPE_XYZ) != 0 {
            tags.push("超量");
        } else if (card.type_ & TYPE_SYNCHRO) != 0 {
            tags.push("同调");
        } else if (card.type_ & TYPE_FUSION) != 0 {
            tags.push("融合");
        } else if (card.type_ & TYPE_RITUAL) != 0 {
            tags.push("仪式");
        }
        if card.is_pendulum() {
            tags.push("灵摆");
        }
        tags.push("怪兽");
        match kind {
            CardKind::Yugioh | CardKind::RushDuel => format!("【{}】", tags.join("／")),
        }
    }
}

pub(crate) fn build_effect_line(card: &CardDataEntry, kind: CardKind) -> Option<String> {
    if card.is_spell() || card.is_trap() {
        return None;
    }
    let mut tags = Vec::new();
    if matches!(kind, CardKind::RushDuel) {
        tags.push("怪兽");
    } else {
        if (card.type_ & TYPE_FUSION) != 0 {
            tags.push("融合");
        }
        if (card.type_ & TYPE_SYNCHRO) != 0 {
            tags.push("同调");
        }
        if (card.type_ & TYPE_XYZ) != 0 {
            tags.push("超量");
        }
        if (card.type_ & TYPE_LINK) != 0 {
            tags.push("连接");
        }
        if (card.type_ & TYPE_RITUAL) != 0 {
            tags.push("仪式");
        }
        if card.is_pendulum() {
            tags.push("灵摆");
        }
        tags.push("怪兽");
        if (card.type_ & 0x20) != 0 {
            tags.push("效果");
        } else {
            tags.push("通常");
        }
    }
    Some(format!("【{}】", tags.join("／")))
}

pub(crate) fn build_scale_line(card: &CardDataEntry) -> String {
    if card.is_pendulum() {
        format!(
            "Level {}  Scale {}/{}",
            card.level, card.lscale, card.rscale
        )
    } else if card.is_link() {
        format!("Link Marker {:#x}", card.link_marker)
    } else {
        format!("Level {}", card.level)
    }
}

pub(crate) fn image_frame(card: &CardDataEntry) -> (u32, u32, u32, u32) {
    if card.is_pendulum() {
        (94, 364, 1205, 1205)
    } else {
        (170, 375, 1054, 1054)
    }
}

pub(crate) fn mask_position(card: &CardDataEntry) -> (u32, u32, u32, u32) {
    if card.is_pendulum() {
        (68, 342, 1258, 1258)
    } else {
        (117, 322, 1160, 1160)
    }
}

pub(crate) fn uses_rank(card: &CardDataEntry) -> bool {
    (card.type_ & TYPE_XYZ) != 0
}

pub(crate) fn description_y(card: &CardDataEntry, style: &LayoutStyle) -> u32 {
    if card.is_spell() || card.is_trap() {
        style.effect_top
    } else {
        let effect_height = (style.effect_size as f32 * style.effect_line_height).round() as u32;
        style.effect_top + effect_height.max(16)
    }
}

pub(crate) fn description_height(card: &CardDataEntry, style: &LayoutStyle) -> u32 {
    let mut height = 385_u32;
    if !card.is_spell() && !card.is_trap() {
        let effect_height = (style.effect_size as f32 * style.effect_line_height).round() as u32;
        height = height.saturating_sub(effect_height.max(16));
        height = height.saturating_sub(60);
    }
    height
}

pub(crate) fn draw_level_or_rank(
    svg: &mut String,
    card: &CardDataEntry,
    href: Option<&str>,
    kind: CardKind,
) {
    let Some(href) = href else {
        return;
    };
    if matches!(kind, CardKind::RushDuel) && card.level == 0 {
        return;
    }
    let count = card.level.min(13);
    if count == 0 {
        return;
    }
    let icon_width = 88_u32;
    if uses_rank(card) {
        let left = if count < 13 { 147 } else { 101 };
        for index in 0..count {
            let x = left + index * (icon_width + 4);
            svg.push_str(&format!(
                "<image x=\"{}\" y=\"247\" width=\"88\" height=\"88\" href=\"{}\" preserveAspectRatio=\"xMidYMid meet\"/>",
                x, href
            ));
        }
    } else {
        let right = if count < 13 { 147 } else { 101 };
        for index in 0..count {
            let x = CARD_WIDTH - right - index * (icon_width + 4) - icon_width;
            svg.push_str(&format!(
                "<image x=\"{}\" y=\"247\" width=\"88\" height=\"88\" href=\"{}\" preserveAspectRatio=\"xMidYMid meet\"/>",
                x, href
            ));
        }
    }
}

pub(crate) fn attribute_rel_path(
    card: &CardDataEntry,
) -> Result<Option<&'static str>, RenderError> {
    let relative = if card.is_spell() {
        Some("yugioh/image/attribute-spell.png")
    } else if card.is_trap() {
        Some("yugioh/image/attribute-trap.png")
    } else {
        match card.attribute {
            0x01 => Some("yugioh/image/attribute-earth.png"),
            0x02 => Some("yugioh/image/attribute-water.png"),
            0x04 => Some("yugioh/image/attribute-fire.png"),
            0x08 => Some("yugioh/image/attribute-wind.png"),
            0x10 => Some("yugioh/image/attribute-light.png"),
            0x20 => Some("yugioh/image/attribute-dark.png"),
            0x40 => Some("yugioh/image/attribute-divine.png"),
            0 => None,
            other => {
                return Err(RenderError::SvgParse(format!(
                    "unsupported attribute value: {other:#x}"
                )));
            }
        }
    };
    Ok(relative)
}

pub(crate) fn display_stat(value: i32) -> String {
    match value {
        -2 => "INF".to_string(),
        -1 => "?".to_string(),
        other => other.to_string(),
    }
}
