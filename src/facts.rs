//! Minimal derived card facts used by document construction.
//! Keeps only values consumed outside this module.

use ygopro_cdb_encode_rs::{
    CardDataEntry, TYPE_FUSION, TYPE_LINK, TYPE_RITUAL, TYPE_SYNCHRO, TYPE_TOKEN, TYPE_XYZ,
};

/// Derived facts queried by document construction and rendering.
pub(crate) struct CardFacts {
    pub footer_is_light: bool,
    pub frame_asset: &'static str,
}

impl CardFacts {
    pub fn new(card: &CardDataEntry) -> Self {
        let is_spell = card.is_spell();
        let is_trap = card.is_trap();
        let is_pendulum = card.is_pendulum();
        let is_link = (card.type_ & TYPE_LINK) != 0;
        let is_xyz = (card.type_ & TYPE_XYZ) != 0;
        let is_synchro = (card.type_ & TYPE_SYNCHRO) != 0;
        let is_fusion = (card.type_ & TYPE_FUSION) != 0;
        let is_ritual = (card.type_ & TYPE_RITUAL) != 0;
        let is_token = (card.type_ & TYPE_TOKEN) != 0;
        let is_effect = (card.type_ & 0x20) != 0;

        let footer_is_light = card.is_monster() && is_xyz;

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
        assert_eq!(f.frame_asset, "card-normal.webp");
        assert!(!f.footer_is_light);
    }

    #[test]
    fn effect_monster() {
        let f = facts(0x21);
        assert_eq!(f.frame_asset, "card-effect.webp");
    }

    #[test]
    fn xyz_monster() {
        let f = facts(TYPE_XYZ | 0x1);
        assert_eq!(f.frame_asset, "card-xyz.webp");
        assert!(f.footer_is_light);
    }

    #[test]
    fn link_monster() {
        let f = facts(TYPE_LINK | 0x1);
        assert_eq!(f.frame_asset, "card-link.webp");
        assert!(!f.footer_is_light);
    }

    #[test]
    fn spell_card() {
        let f = facts(0x2);
        assert_eq!(f.frame_asset, "card-spell.webp");
        assert!(!f.footer_is_light);
    }

    #[test]
    fn trap_card() {
        let f = facts(0x4);
        assert_eq!(f.frame_asset, "card-trap.webp");
    }

    #[test]
    fn pendulum_effect() {
        let f = facts(ygopro_cdb_encode_rs::TYPE_PENDULUM | 0x20 | 0x1);
        assert_eq!(f.frame_asset, "card-effect-pendulum.webp");
    }

    #[test]
    fn token_card() {
        let f = facts(TYPE_TOKEN | 0x1);
        assert_eq!(f.frame_asset, "card-token.webp");
    }
}
