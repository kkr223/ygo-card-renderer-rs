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

/// Named color constants used throughout the renderer (R, G, B).
/// All values are in straight (non-premultiplied) sRGB.
pub(crate) const TEXT_COLOR_DARK: (u8, u8, u8) = (17, 17, 17);
pub(crate) const BACKGROUND_CREAM: (u8, u8, u8) = (244, 239, 231);
pub(crate) const PASSWORD_COLOR: (u8, u8, u8) = (93, 81, 70);
pub(crate) const NAME_COLOR_DARK: (u8, u8, u8) = (22, 18, 15);
pub(crate) const NAME_COLOR_LIGHT: (u8, u8, u8) = (245, 245, 245);
pub(crate) const TYPE_COLOR: (u8, u8, u8) = (29, 20, 15);

pub(crate) const RUSH_DUEL_FONT_SPECS: &[(&str, &str)] = &[
    ("rd-atk-def", "rush-duel/font/rd-atk-def.woff2"),
    ("rd-jp", "rush-duel/font/rd-jp.woff2"),
    ("rd-jp-effect", "rush-duel/font/rd-jp-effect.woff2"),
    ("rd-jp-name", "rush-duel/font/rd-jp-name.woff2"),
    ("rd-sc", "rush-duel/font/rd-sc.woff2"),
    ("rd-sc-name", "rush-duel/font/rd-sc-name.woff2"),
    ("rd-tip", "rush-duel/font/rd-tip.woff2"),
];
