#![allow(dead_code)]

pub(crate) const CARD_WIDTH: u32 = 1394;
pub(crate) const CARD_HEIGHT: u32 = 2031;

pub(crate) const YUGIOH_FONT_SPECS: &[(&str, &str)] = &[
    ("ygo-astral", "yugioh/font/ygo-astral.woff2"),
    ("ygo-atk-def", "yugioh/font/ygo-atk-def.woff2"),
    ("ygo-en", "yugioh/font/ygo-en.woff2"),
    ("ygo-en-name", "yugioh/font/ygo-en-name.woff2"),
    ("ygo-en-race", "yugioh/font/ygo-en-race.woff2"),
    ("ygo-jp", "yugioh/font/ygo-jp.woff2"),
    ("ygo-link", "yugioh/font/ygo-link.woff2"),
    ("ygo-password", "yugioh/font/ygo-password.woff2"),
    ("ygo-sc", "yugioh/font/ygo-sc.woff2"),
    ("ygo-tip", "yugioh/font/ygo-tip.woff2"),
];

pub(crate) const RUSH_DUEL_FONT_SPECS: &[(&str, &str)] = &[
    ("rd-atk-def", "rush-duel/font/rd-atk-def.woff2"),
    ("rd-jp", "rush-duel/font/rd-jp.woff2"),
    ("rd-jp-effect", "rush-duel/font/rd-jp-effect.woff2"),
    ("rd-jp-name", "rush-duel/font/rd-jp-name.woff2"),
    ("rd-sc", "rush-duel/font/rd-sc.woff2"),
    ("rd-sc-name", "rush-duel/font/rd-sc-name.woff2"),
    ("rd-tip", "rush-duel/font/rd-tip.woff2"),
];
