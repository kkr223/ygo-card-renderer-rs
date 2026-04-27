use ygopro_cdb_encode_rs::{
    CardDataEntry, TYPE_FUSION, TYPE_LINK, TYPE_RITUAL, TYPE_SYNCHRO, TYPE_TOKEN, TYPE_XYZ,
};

const TYPE_NORMAL: u32 = 0x10;
const TYPE_EFFECT: u32 = 0x20;
const TYPE_SPIRIT: u32 = 0x200;
const TYPE_UNION: u32 = 0x400;
const TYPE_GEMINI: u32 = 0x800;
const TYPE_TUNER: u32 = 0x1000;
const TYPE_QUICKPLAY: u32 = 0x1_0000;
const TYPE_CONTINUOUS: u32 = 0x2_0000;
const TYPE_EQUIP: u32 = 0x4_0000;
const TYPE_FIELD: u32 = 0x8_0000;
const TYPE_COUNTER: u32 = 0x10_0000;
const TYPE_FLIP: u32 = 0x20_0000;
const TYPE_TOON: u32 = 0x40_0000;
const TYPE_SPS_SUMMON: u32 = 0x200_0000;

use crate::{asset_bundle::BaseLayout, layout::LayoutStyle, model::CardKind};

pub(crate) fn frame_asset_name(card: &CardDataEntry) -> &'static str {
    if card.is_spell() {
        "card-spell.webp"
    } else if card.is_trap() {
        "card-trap.webp"
    } else if (card.type_ & TYPE_TOKEN) != 0 {
        "card-token.webp"
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

pub(crate) fn build_effect_line(
    card: &CardDataEntry,
    kind: CardKind,
    language: Option<&str>,
) -> Option<String> {
    if card.is_spell() || card.is_trap() {
        return None;
    }
    if matches!(kind, CardKind::RushDuel) {
        return Some("【怪兽】".to_string());
    }

    let language = normalized_language(language);
    let (left_bracket, right_bracket) = localized_brackets(Some(language));
    let mut tags = Vec::new();

    if let Some(race) = localized_race_name(card.race, language) {
        tags.push(race);
    } else {
        tags.push(localized_monster_word(language));
    }

    for &(key, bit) in MONSTER_TYPE_ORDER {
        if (card.type_ & bit) != 0 {
            tags.push(localized_monster_type(key, language));
        }
    }

    if (card.type_ & TYPE_EFFECT) != 0 {
        tags.push(localized_monster_type(MonsterTypeKey::Effect, language));
    } else if (card.type_ & TYPE_NORMAL) != 0 {
        tags.push(localized_monster_type(MonsterTypeKey::Normal, language));
    }

    Some(format!(
        "{left_bracket}{}{right_bracket}",
        tags.join(monster_type_separator(language))
    ))
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

pub(crate) fn description_height(
    card: &CardDataEntry,
    style: &LayoutStyle,
    base: &BaseLayout,
) -> u32 {
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
    card.is_spell()
        || card.is_trap()
        || (card.type_ & TYPE_XYZ) != 0
        || (card.type_ & TYPE_LINK) != 0
}

pub(crate) fn localized_brackets(language: Option<&str>) -> (&'static str, &'static str) {
    match language.unwrap_or("sc") {
        "en" | "kr" => ("[", "]"),
        _ => ("【", "】"),
    }
}

pub(crate) fn localized_spell_trap_name(
    card: &CardDataEntry,
    language: Option<&str>,
) -> &'static str {
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
    // `build_effect_line` returns None for spells/traps and always builds a
    // non-empty string for monsters, so `.is_some()` is sufficient here.
    build_effect_line(card, CardKind::Yugioh, None).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonsterTypeKey {
    Fusion,
    Synchro,
    Link,
    Xyz,
    Ritual,
    SpecialSummon,
    Pendulum,
    Spirit,
    Gemini,
    Union,
    Flip,
    Toon,
    Tuner,
    Effect,
    Normal,
}

const MONSTER_TYPE_ORDER: &[(MonsterTypeKey, u32)] = &[
    (MonsterTypeKey::Fusion, TYPE_FUSION),
    (MonsterTypeKey::Synchro, TYPE_SYNCHRO),
    (MonsterTypeKey::Link, TYPE_LINK),
    (MonsterTypeKey::Xyz, TYPE_XYZ),
    (MonsterTypeKey::Ritual, TYPE_RITUAL),
    (MonsterTypeKey::SpecialSummon, TYPE_SPS_SUMMON),
    (
        MonsterTypeKey::Pendulum,
        ygopro_cdb_encode_rs::TYPE_PENDULUM,
    ),
    (MonsterTypeKey::Spirit, TYPE_SPIRIT),
    (MonsterTypeKey::Gemini, TYPE_GEMINI),
    (MonsterTypeKey::Union, TYPE_UNION),
    (MonsterTypeKey::Flip, TYPE_FLIP),
    (MonsterTypeKey::Toon, TYPE_TOON),
    (MonsterTypeKey::Tuner, TYPE_TUNER),
];

fn normalized_language(language: Option<&str>) -> &'static str {
    match language.unwrap_or("sc") {
        "tc" => "tc",
        "jp" => "jp",
        "kr" => "kr",
        "en" => "en",
        "astral" => "astral",
        _ => "sc",
    }
}

fn monster_type_separator(language: &str) -> &'static str {
    match language {
        "tc" | "jp" => "／",
        "kr" => " / ",
        _ => "/",
    }
}

fn localized_monster_word(language: &str) -> &'static str {
    match language {
        "en" => "Monster",
        "jp" => "モンスター",
        "kr" => "몬스터",
        "tc" => "怪獸",
        _ => "怪兽",
    }
}

fn localized_race_name(race: u32, language: &str) -> Option<&'static str> {
    let key = race_key(race)?;
    Some(match language {
        "tc" => match key {
            "warrior" => "戰士族",
            "spellcaster" => "魔法使族",
            "fairy" => "天使族",
            "fiend" => "惡魔族",
            "zombie" => "不死族",
            "machine" => "機械族",
            "aqua" => "水族",
            "pyro" => "炎族",
            "rock" => "岩石族",
            "wingedbeast" => "鳥獸族",
            "plant" => "植物族",
            "insect" => "昆蟲族",
            "thunder" => "雷族",
            "dragon" => "龍族",
            "beast" => "獸族",
            "beastwarrior" => "獸戰士族",
            "dinosaur" => "恐龍族",
            "fish" => "魚族",
            "seaserpent" => "海龍族",
            "reptile" => "爬蟲類族",
            "psychic" => "念動力族",
            "divinebeast" => "幻神獸族",
            "creatorgod" => "創造神族",
            "wyrm" => "幻龍族",
            "cyberse" => "電子界族",
            "illusion" => "幻想魔族",
            _ => return None,
        },
        "jp" => match key {
            "warrior" => "戦士",
            "spellcaster" => "魔法使い",
            "fairy" => "天使",
            "fiend" => "悪魔",
            "zombie" => "アンデット",
            "machine" => "機械",
            "aqua" => "水",
            "pyro" => "炎",
            "rock" => "岩石",
            "wingedbeast" => "鳥獣",
            "plant" => "植物",
            "insect" => "昆虫",
            "thunder" => "雷",
            "dragon" => "ドラゴン",
            "beast" => "獣",
            "beastwarrior" => "獣戦士",
            "dinosaur" => "恐竜",
            "fish" => "魚",
            "seaserpent" => "海竜",
            "reptile" => "爬虫類",
            "psychic" => "サイキック",
            "divinebeast" => "幻神獣",
            "creatorgod" => "創造神",
            "wyrm" => "幻竜",
            "cyberse" => "サイバース",
            "illusion" => "幻想魔",
            _ => return None,
        },
        "kr" => match key {
            "warrior" => "전사",
            "spellcaster" => "마법사",
            "fairy" => "천사",
            "fiend" => "악마",
            "zombie" => "언데드",
            "machine" => "기계",
            "aqua" => "물",
            "pyro" => "화염",
            "rock" => "암석",
            "wingedbeast" => "비행야수",
            "plant" => "식물",
            "insect" => "곤충",
            "thunder" => "번개",
            "dragon" => "드래곤",
            "beast" => "야수",
            "beastwarrior" => "야수전사",
            "dinosaur" => "공룡",
            "fish" => "어류",
            "seaserpent" => "해룡",
            "reptile" => "파충류",
            "psychic" => "사이킥",
            "divinebeast" => "환신야수",
            "creatorgod" => "창조신",
            "wyrm" => "환룡",
            "cyberse" => "사이버스",
            "illusion" => "환상마",
            _ => return None,
        },
        "en" => match key {
            "warrior" => "Warrior",
            "spellcaster" => "Spellcaster",
            "fairy" => "Fairy",
            "fiend" => "Fiend",
            "zombie" => "Zombie",
            "machine" => "Machine",
            "aqua" => "Aqua",
            "pyro" => "Pyro",
            "rock" => "Rock",
            "wingedbeast" => "Winged Beast",
            "plant" => "Plant",
            "insect" => "Insect",
            "thunder" => "Thunder",
            "dragon" => "Dragon",
            "beast" => "Beast",
            "beastwarrior" => "Beast-Warrior",
            "dinosaur" => "Dinosaur",
            "fish" => "Fish",
            "seaserpent" => "Sea Serpent",
            "reptile" => "Reptile",
            "psychic" => "Psychic",
            "divinebeast" => "Divine-Beast",
            "creatorgod" => "Creator God",
            "wyrm" => "Wyrm",
            "cyberse" => "Cyberse",
            "illusion" => "Illusion",
            _ => return None,
        },
        _ => match key {
            "warrior" => "战士族",
            "spellcaster" => "魔法师族",
            "fairy" => "天使族",
            "fiend" => "恶魔族",
            "zombie" => "不死族",
            "machine" => "机械族",
            "aqua" => "水族",
            "pyro" => "炎族",
            "rock" => "岩石族",
            "wingedbeast" => "鸟兽族",
            "plant" => "植物族",
            "insect" => "昆虫族",
            "thunder" => "雷族",
            "dragon" => "龙族",
            "beast" => "兽族",
            "beastwarrior" => "兽战士族",
            "dinosaur" => "恐龙族",
            "fish" => "鱼族",
            "seaserpent" => "海龙族",
            "reptile" => "爬虫类族",
            "psychic" => "念动力族",
            "divinebeast" => "幻神兽族",
            "creatorgod" => "创造神族",
            "wyrm" => "幻龙族",
            "cyberse" => "电子界族",
            "illusion" => "幻想魔族",
            _ => return None,
        },
    })
}

fn race_key(race: u32) -> Option<&'static str> {
    const RACES: &[(&str, u32)] = &[
        ("warrior", 0x1),
        ("spellcaster", 0x2),
        ("fairy", 0x4),
        ("fiend", 0x8),
        ("zombie", 0x10),
        ("machine", 0x20),
        ("aqua", 0x40),
        ("pyro", 0x80),
        ("rock", 0x100),
        ("wingedbeast", 0x200),
        ("plant", 0x400),
        ("insect", 0x800),
        ("thunder", 0x1000),
        ("dragon", 0x2000),
        ("beast", 0x4000),
        ("beastwarrior", 0x8000),
        ("dinosaur", 0x10000),
        ("fish", 0x20000),
        ("seaserpent", 0x40000),
        ("reptile", 0x80000),
        ("psychic", 0x100000),
        ("divinebeast", 0x200000),
        ("creatorgod", 0x400000),
        ("wyrm", 0x800000),
        ("cyberse", 0x1000000),
        ("illusion", 0x2000000),
    ];

    RACES
        .iter()
        .find_map(|(key, bit)| if (race & bit) != 0 { Some(*key) } else { None })
}

fn localized_monster_type(key: MonsterTypeKey, language: &str) -> &'static str {
    match language {
        "tc" => match key {
            MonsterTypeKey::Fusion => "融合",
            MonsterTypeKey::Synchro => "同步",
            MonsterTypeKey::Link => "連結",
            MonsterTypeKey::Xyz => "超量",
            MonsterTypeKey::Ritual => "儀式",
            MonsterTypeKey::SpecialSummon => "特殊召喚",
            MonsterTypeKey::Pendulum => "靈擺",
            MonsterTypeKey::Spirit => "靈魂",
            MonsterTypeKey::Gemini => "二重",
            MonsterTypeKey::Union => "同盟",
            MonsterTypeKey::Flip => "反轉",
            MonsterTypeKey::Toon => "卡通",
            MonsterTypeKey::Tuner => "調整",
            MonsterTypeKey::Effect => "效果",
            MonsterTypeKey::Normal => "通常",
        },
        "jp" => match key {
            MonsterTypeKey::Fusion => "融合",
            MonsterTypeKey::Synchro => "シンクロ",
            MonsterTypeKey::Link => "リンク",
            MonsterTypeKey::Xyz => "エクシーズ",
            MonsterTypeKey::Ritual => "儀式",
            MonsterTypeKey::SpecialSummon => "特殊召喚",
            MonsterTypeKey::Pendulum => "ペンデュラム",
            MonsterTypeKey::Spirit => "スピリット",
            MonsterTypeKey::Gemini => "デュアル",
            MonsterTypeKey::Union => "ユニオン",
            MonsterTypeKey::Flip => "リバース",
            MonsterTypeKey::Toon => "トゥーン",
            MonsterTypeKey::Tuner => "チューナー",
            MonsterTypeKey::Effect => "効果",
            MonsterTypeKey::Normal => "通常",
        },
        "kr" => match key {
            MonsterTypeKey::Fusion => "융합",
            MonsterTypeKey::Synchro => "싱크로",
            MonsterTypeKey::Link => "링크",
            MonsterTypeKey::Xyz => "엑시즈",
            MonsterTypeKey::Ritual => "의식",
            MonsterTypeKey::SpecialSummon => "특수 소환",
            MonsterTypeKey::Pendulum => "펜듈럼",
            MonsterTypeKey::Spirit => "스피릿",
            MonsterTypeKey::Gemini => "듀얼",
            MonsterTypeKey::Union => "유니온",
            MonsterTypeKey::Flip => "리버스",
            MonsterTypeKey::Toon => "툰",
            MonsterTypeKey::Tuner => "튜너",
            MonsterTypeKey::Effect => "효과",
            MonsterTypeKey::Normal => "일반",
        },
        "en" => match key {
            MonsterTypeKey::Fusion => "Fusion",
            MonsterTypeKey::Synchro => "Synchro",
            MonsterTypeKey::Link => "Link",
            MonsterTypeKey::Xyz => "Xyz",
            MonsterTypeKey::Ritual => "Ritual",
            MonsterTypeKey::SpecialSummon => "Sp. Summon",
            MonsterTypeKey::Pendulum => "Pendulum",
            MonsterTypeKey::Spirit => "Spirit",
            MonsterTypeKey::Gemini => "Gemini",
            MonsterTypeKey::Union => "Union",
            MonsterTypeKey::Flip => "Flip",
            MonsterTypeKey::Toon => "Toon",
            MonsterTypeKey::Tuner => "Tuner",
            MonsterTypeKey::Effect => "Effect",
            MonsterTypeKey::Normal => "Normal",
        },
        _ => match key {
            MonsterTypeKey::Fusion => "融合",
            MonsterTypeKey::Synchro => "同调",
            MonsterTypeKey::Link => "连接",
            MonsterTypeKey::Xyz => "超量",
            MonsterTypeKey::Ritual => "仪式",
            MonsterTypeKey::SpecialSummon => "特殊召唤",
            MonsterTypeKey::Pendulum => "灵摆",
            MonsterTypeKey::Spirit => "灵魂",
            MonsterTypeKey::Gemini => "二重",
            MonsterTypeKey::Union => "同盟",
            MonsterTypeKey::Flip => "反转",
            MonsterTypeKey::Toon => "卡通",
            MonsterTypeKey::Tuner => "调整",
            MonsterTypeKey::Effect => "效果",
            MonsterTypeKey::Normal => "通常",
        },
    }
}

pub(crate) struct PendulumTextSections {
    pub(crate) pendulum_effect: Option<String>,
    pub(crate) monster_effect: String,
}

pub(crate) fn split_pendulum_description(desc: &str) -> PendulumTextSections {
    let normalized = desc.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = normalized.lines().collect();

    if is_pendulum_header(lines.first().copied().unwrap_or_default()) {
        lines.remove(0);
    }

    let marker_index = lines.iter().position(|line| is_monster_effect_marker(line));
    let Some(marker_index) = marker_index else {
        return PendulumTextSections {
            pendulum_effect: None,
            monster_effect: desc.to_string(),
        };
    };

    let pendulum_effect = join_trimmed_lines(&lines[..marker_index]);
    let monster_effect = join_trimmed_lines(&lines[marker_index + 1..]);

    PendulumTextSections {
        pendulum_effect: if pendulum_effect.is_empty() {
            None
        } else {
            Some(pendulum_effect)
        },
        monster_effect,
    }
}

fn is_pendulum_header(line: &str) -> bool {
    let trimmed = line.trim();
    (trimmed.contains("【灵摆】")
        || trimmed.contains("[Pendulum")
        || trimmed.contains("ペンデュラム"))
        && (trimmed.contains('←')
            || trimmed.contains('→')
            || trimmed.contains('<')
            || trimmed.contains('>'))
}

fn is_monster_effect_marker(line: &str) -> bool {
    matches!(
        line.trim(),
        "【怪兽效果】"
            | "[Monster Effect]"
            | "【Monster Effect】"
            | "【モンスター効果】"
            | "【몬스터 효과】"
    )
}

fn join_trimmed_lines(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{build_effect_line, frame_asset_name, split_pendulum_description};
    use ygopro_cdb_encode_rs::{CardDataEntry, TYPE_MONSTER, TYPE_TOKEN};

    #[test]
    fn token_cards_use_token_frame() {
        let card = CardDataEntry {
            type_: TYPE_MONSTER | TYPE_TOKEN,
            ..CardDataEntry::default()
        };

        assert_eq!(frame_asset_name(&card), "card-token.webp");
    }

    #[test]
    fn builds_english_monster_type_line() {
        let card = CardDataEntry {
            type_: TYPE_MONSTER | 0x10,
            race: 0x2000,
            ..CardDataEntry::default()
        };

        assert_eq!(
            build_effect_line(&card, crate::model::CardKind::Yugioh, Some("en")).as_deref(),
            Some("[Dragon/Normal]")
        );
    }

    #[test]
    fn builds_japanese_monster_type_line() {
        let card = CardDataEntry {
            type_: TYPE_MONSTER | 0x40 | 0x20,
            race: 0x2,
            ..CardDataEntry::default()
        };

        assert_eq!(
            build_effect_line(&card, crate::model::CardKind::Yugioh, Some("jp")).as_deref(),
            Some("【魔法使い／融合／効果】")
        );
    }

    #[test]
    fn builds_korean_monster_type_line() {
        let card = CardDataEntry {
            type_: TYPE_MONSTER | 0x4000000 | 0x1000000 | 0x20,
            race: 0x1000000,
            ..CardDataEntry::default()
        };

        assert_eq!(
            build_effect_line(&card, crate::model::CardKind::Yugioh, Some("kr")).as_deref(),
            Some("[사이버스 / 링크 / 펜듈럼 / 효과]")
        );
    }

    #[test]
    fn splits_sc_pendulum_description() {
        let sections = split_pendulum_description(
            "←6 【灵摆】 6→\r\n灵摆效果。\r\n【怪兽效果】\r\n怪兽效果。",
        );

        assert_eq!(sections.pendulum_effect.as_deref(), Some("灵摆效果。"));
        assert_eq!(sections.monster_effect, "怪兽效果。");
    }

    #[test]
    fn leaves_unmarked_text_unchanged() {
        let sections = split_pendulum_description("没有分隔标记。");

        assert_eq!(sections.pendulum_effect, None);
        assert_eq!(sections.monster_effect, "没有分隔标记。");
    }
}
