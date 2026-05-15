//! Pre-computed card type facts, derived once from `CardDataEntry`.
//! Consolidates bit-flag checks and asset name resolution scattered across
//! `card_logic.rs`, `document/paint.rs`, and layer functions.

use ygopro_cdb_encode_rs::{
    CardDataEntry, TYPE_FUSION, TYPE_LINK, TYPE_RITUAL, TYPE_SYNCHRO, TYPE_TOKEN, TYPE_XYZ,
};

const TYPE_NORMAL: u32 = 0x10;
const TYPE_EFFECT: u32 = 0x20;

/// All boolean / derived facts about a card that are cheap to compute once
/// and queried repeatedly during document construction and rendering.
#[allow(dead_code)]
pub(crate) struct CardFacts {
    pub is_monster: bool,
    pub is_spell: bool,
    pub is_trap: bool,
    pub is_pendulum: bool,
    pub is_link: bool,
    pub is_xyz: bool,
    pub is_synchro: bool,
    pub is_fusion: bool,
    pub is_ritual: bool,
    pub is_token: bool,
    pub is_effect: bool,
    pub is_normal: bool,
    pub has_effect_line: bool,
    pub uses_rank: bool,
    pub name_is_light: bool,
    pub footer_is_light: bool,
    pub frame_asset: &'static str,
}

impl CardFacts {
    pub fn new(card: &CardDataEntry) -> Self {
        let t = card.type_;
        let is_monster = card.is_monster();
        let is_spell = card.is_spell();
        let is_trap = card.is_trap();
        let is_pendulum = card.is_pendulum();
        let is_link = (t & TYPE_LINK) != 0;
        let is_xyz = (t & TYPE_XYZ) != 0;
        let is_synchro = (t & TYPE_SYNCHRO) != 0;
        let is_fusion = (t & TYPE_FUSION) != 0;
        let is_ritual = (t & TYPE_RITUAL) != 0;
        let is_token = (t & TYPE_TOKEN) != 0;
        let is_effect = (t & TYPE_EFFECT) != 0;
        let is_normal = (t & TYPE_NORMAL) != 0;
        let has_effect_line = is_monster;
        let uses_rank = is_xyz;

        let name_is_light = is_spell || is_trap || is_xyz || is_link;
        let footer_is_light = is_monster && is_xyz;

        let frame_asset = frame_asset_name_inner(
            is_spell,
            is_trap,
            is_token,
            is_pendulum,
            is_xyz,
            is_synchro,
            is_fusion,
            is_ritual,
            is_effect,
            is_link,
        );

        Self {
            is_monster,
            is_spell,
            is_trap,
            is_pendulum,
            is_link,
            is_xyz,
            is_synchro,
            is_fusion,
            is_ritual,
            is_token,
            is_effect,
            is_normal,
            has_effect_line,
            uses_rank,
            name_is_light,
            footer_is_light,
            frame_asset,
        }
    }
}

/// Pure function version of frame_asset_name, taking pre-computed booleans.
fn frame_asset_name_inner(
    is_spell: bool,
    is_trap: bool,
    is_token: bool,
    is_pendulum: bool,
    is_xyz: bool,
    is_synchro: bool,
    is_fusion: bool,
    is_ritual: bool,
    is_effect: bool,
    is_link: bool,
) -> &'static str {
    if is_spell {
        "card-spell.webp"
    } else if is_trap {
        "card-trap.webp"
    } else if is_token {
        "card-token.webp"
    } else if is_pendulum {
        if is_xyz {
            "card-xyz-pendulum.webp"
        } else if is_synchro {
            "card-synchro-pendulum.webp"
        } else if is_fusion {
            "card-fusion-pendulum.webp"
        } else if is_ritual {
            "card-ritual-pendulum.webp"
        } else if is_effect {
            "card-effect-pendulum.webp"
        } else {
            "card-normal-pendulum.webp"
        }
    } else if is_link {
        "card-link.webp"
    } else if is_xyz {
        "card-xyz.webp"
    } else if is_synchro {
        "card-synchro.webp"
    } else if is_fusion {
        "card-fusion.webp"
    } else if is_ritual {
        "card-ritual.webp"
    } else if is_effect {
        "card-effect.webp"
    } else {
        "card-normal.webp"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(type_: u32) -> CardFacts {
        CardFacts::new(&CardDataEntry {
            code: 1,
            name: "test".into(),
            desc: "test".into(),
            type_,
            ..CardDataEntry::default()
        })
    }

    #[test]
    fn normal_monster() {
        let f = facts(0x11);
        assert!(f.is_monster);
        assert!(!f.is_spell);
        assert!(!f.is_trap);
        assert!(!f.is_effect);
        assert_eq!(f.frame_asset, "card-normal.webp");
        assert!(!f.name_is_light);
        assert!(!f.footer_is_light);
        assert!(f.has_effect_line);
    }

    #[test]
    fn effect_monster() {
        let f = facts(0x21);
        assert!(f.is_effect);
        assert_eq!(f.frame_asset, "card-effect.webp");
    }

    #[test]
    fn xyz_monster() {
        let f = facts(TYPE_XYZ | 0x1);
        assert!(f.is_xyz);
        assert!(f.uses_rank);
        assert_eq!(f.frame_asset, "card-xyz.webp");
        assert!(f.name_is_light);
        assert!(f.footer_is_light);
    }

    #[test]
    fn link_monster() {
        let f = facts(TYPE_LINK | 0x1);
        assert!(f.is_link);
        assert_eq!(f.frame_asset, "card-link.webp");
        assert!(f.name_is_light);
        assert!(!f.footer_is_light);
    }

    #[test]
    fn spell_card() {
        let f = facts(0x2);
        assert!(f.is_spell);
        assert_eq!(f.frame_asset, "card-spell.webp");
        assert!(f.name_is_light);
        assert!(!f.footer_is_light);
    }

    #[test]
    fn trap_card() {
        let f = facts(0x4);
        assert!(f.is_trap);
        assert_eq!(f.frame_asset, "card-trap.webp");
    }

    #[test]
    fn pendulum_effect() {
        let f = facts(ygopro_cdb_encode_rs::TYPE_PENDULUM | TYPE_EFFECT | 0x1);
        assert!(f.is_pendulum);
        assert_eq!(f.frame_asset, "card-effect-pendulum.webp");
    }

    #[test]
    fn token_card() {
        let f = facts(TYPE_TOKEN | 0x1);
        assert!(f.is_token);
        assert_eq!(f.frame_asset, "card-token.webp");
    }
}
