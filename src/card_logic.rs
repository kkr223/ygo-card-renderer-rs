use ygopro_cdb_encode_rs::{
    CardDataEntry, TYPE_FUSION, TYPE_LINK, TYPE_RITUAL, TYPE_SYNCHRO, TYPE_XYZ,
};

const TYPE_QUICKPLAY: u32 = 0x1_0000;
const TYPE_CONTINUOUS: u32 = 0x2_0000;
const TYPE_EQUIP: u32 = 0x4_0000;
const TYPE_FIELD: u32 = 0x8_0000;
const TYPE_COUNTER: u32 = 0x10_0000;

use crate::{
    asset_bundle::BaseLayout,
    layout::LayoutStyle,
    model::CardKind,
};

pub(crate) fn frame_asset_name(card: &CardDataEntry) -> &'static str {
    if card.is_spell() {
        "card-spell.webp"
    } else if card.is_trap() {
        "card-trap.webp"
    } else if card.is_pendulum() {
        if (card.type_ & TYPE_XYZ) != 0 {
            "card-xyz-pendulum.webp"
        } else if (card.type_ & TYPE_SYNCHRO) != 0 {
            "card-synchro-pendulum.webp"
        } else if (card.type_ & TYPE_FUSION) != 0 {
            "card-fusion-pendulum.webp"
        } else if (card.type_ & TYPE_RITUAL) != 0 {
            "card-ritual-pendulum.webp"
        } else if (card.type_ & 0x20) != 0 {
            "card-effect-pendulum.webp"
        } else {
            "card-normal-pendulum.webp"
        }
    } else if (card.type_ & TYPE_LINK) != 0 {
        "card-link.webp"
    } else if (card.type_ & TYPE_XYZ) != 0 {
        "card-xyz.webp"
    } else if (card.type_ & TYPE_SYNCHRO) != 0 {
        "card-synchro.webp"
    } else if (card.type_ & TYPE_FUSION) != 0 {
        "card-fusion.webp"
    } else if (card.type_ & TYPE_RITUAL) != 0 {
        "card-ritual.webp"
    } else if (card.type_ & 0x20) != 0 {
        "card-effect.webp"
    } else {
        "card-normal.webp"
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

pub(crate) fn image_frame(card: &CardDataEntry, base: &BaseLayout) -> (u32, u32, u32, u32) {
    let rect = if card.is_pendulum() {
        &base.image.pendulum
    } else {
        &base.image.normal
    };
    (rect.x, rect.y, rect.width, rect.height)
}

pub(crate) fn uses_rank(card: &CardDataEntry) -> bool {
    (card.type_ & TYPE_XYZ) != 0
}

pub(crate) fn description_y(card: &CardDataEntry, style: &LayoutStyle) -> u32 {
    if card.is_spell() || card.is_trap() {
        style.effect_top
    } else {
        let effect_height = if has_effect_line(card) {
            (style.effect_size as f32 * style.effect_line_height).round() as u32
        } else {
            style.effect_min_height
        };
        style.effect_top + effect_height
    }
}

pub(crate) fn description_height(card: &CardDataEntry, style: &LayoutStyle, base: &BaseLayout) -> u32 {
    let mut height = base.description.base_height;
    if !card.is_spell() && !card.is_trap() {
        let effect_height = if has_effect_line(card) {
            (style.effect_size as f32 * style.effect_line_height).round() as u32
        } else {
            style.effect_min_height
        };
        height = height.saturating_sub(effect_height);
        height = height.saturating_sub(base.description.atk_bar_height);
    }
    height
}

pub(crate) fn display_stat(value: i32) -> String {
    match value {
        -2 => "INF".to_string(),
        -1 => "?".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn attribute_asset_name(card: &CardDataEntry, language: Option<&str>) -> Option<String> {
    let suffix = match language.unwrap_or("sc") {
        "jp" => "-jp",
        "kr" => "-kr",
        "en" => "-en",
        "astral" => "-astral",
        _ => "",
    };

    if card.is_spell() {
        Some(format!("attribute-spell{suffix}.webp"))
    } else if card.is_trap() {
        Some(format!("attribute-trap{suffix}.webp"))
    } else {
        let name = match card.attribute {
            0x01 => "earth",
            0x02 => "water",
            0x04 => "fire",
            0x08 => "wind",
            0x10 => "light",
            0x20 => "dark",
            0x40 => "divine",
            _ => return None,
        };
        Some(format!("attribute-{name}{suffix}.webp"))
    }
}

pub(crate) fn spell_trap_subtype_icon_asset(card: &CardDataEntry) -> Option<&'static str> {
    if (card.type_ & TYPE_QUICKPLAY) != 0 {
        Some("icon-quick-play.webp")
    } else if (card.type_ & TYPE_CONTINUOUS) != 0 {
        Some("icon-continuous.webp")
    } else if (card.type_ & TYPE_EQUIP) != 0 {
        Some("icon-equip.webp")
    } else if (card.type_ & TYPE_FIELD) != 0 {
        Some("icon-field.webp")
    } else if (card.type_ & TYPE_COUNTER) != 0 {
        Some("icon-counter.webp")
    } else if card.is_spell() && (card.type_ & TYPE_RITUAL) != 0 {
        Some("icon-ritual.webp")
    } else {
        None
    }
}

pub(crate) fn auto_name_light(card: &CardDataEntry) -> bool {
    card.is_spell() || card.is_trap() || (card.type_ & TYPE_XYZ) != 0 || (card.type_ & TYPE_LINK) != 0
}

pub(crate) fn localized_brackets(language: Option<&str>) -> (&'static str, &'static str) {
    match language.unwrap_or("sc") {
        "en" | "kr" => ("[", "]"),
        _ => ("【", "】"),
    }
}

pub(crate) fn localized_spell_trap_name(card: &CardDataEntry, language: Option<&str>) -> &'static str {
    match language.unwrap_or("sc") {
        "sc" | "tc" => {
            if card.is_spell() {
                "魔法卡"
            } else {
                "陷阱卡"
            }
        }
        "jp" => {
            if card.is_spell() {
                "[魔(ま)][法(ほう)]カード"
            } else {
                "[罠(トラップ)]カード"
            }
        }
        "kr" => {
            if card.is_spell() {
                "마법 카드"
            } else {
                "함정 카드"
            }
        }
        "en" => {
            if card.is_spell() {
                "Spell Card"
            } else {
                "Trap Card"
            }
        }
        "astral" => {
            if card.is_spell() {
                "マホウカアド"
            } else {
                "トラププカアド"
            }
        }
        _ => {
            if card.is_spell() {
                "魔法卡"
            } else {
                "陷阱卡"
            }
        }
    }
}

pub(crate) fn has_effect_line(card: &CardDataEntry) -> bool {
    !card.is_spell() && !card.is_trap() && !build_effect_line(card, CardKind::Yugioh).unwrap_or_default().is_empty()
}
