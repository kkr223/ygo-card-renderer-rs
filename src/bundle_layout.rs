//! Layout types extracted from `src/asset_bundle.rs`.
//!
//! These structs represent the layout JSON embedded inside the bundle.
//! They are deserialised at bundle load time and consumed by
//! `src/layout.rs`, `src/document.rs`, and the layer modules.

use serde::Deserialize;
use std::collections::HashMap;

// ── Text block / style primitives ──────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TextBlock {
    #[serde(rename = "fontFamily")]
    pub font_family: Option<String>,
    pub top: Option<u32>,
    #[serde(rename = "fontSize")]
    pub font_size: Option<u32>,
    #[serde(rename = "rtFontSize")]
    pub rt_font_size: Option<u32>,
    #[serde(rename = "rtTop")]
    pub rt_top: Option<i32>,
    #[serde(rename = "rtFontScaleX")]
    pub rt_font_scale_x: Option<f32>,
    #[serde(rename = "letterSpacing")]
    pub letter_spacing: Option<f32>,
    #[serde(rename = "wordSpacing")]
    pub word_spacing: Option<f32>,
    #[serde(rename = "lineHeight")]
    pub line_height: Option<f32>,
    #[serde(rename = "scaleY")]
    pub scale_y: Option<f32>,
    pub right: Option<u32>,
    #[serde(rename = "textIndent")]
    pub text_indent: Option<f32>,
    #[serde(rename = "minHeight")]
    pub min_height: Option<u32>,
    #[serde(rename = "smallFontSize")]
    pub small_font_size: Option<u32>,
    #[serde(rename = "icon")]
    pub icon: Option<IconMargins>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct IconMargins {
    #[serde(rename = "marginTop")]
    pub margin_top: Option<f32>,
    #[serde(rename = "marginLeft")]
    pub margin_left: Option<f32>,
    #[serde(rename = "marginRight")]
    pub margin_right: Option<f32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StyleDefinition {
    #[serde(rename = "fontFamily")]
    pub font_family: String,
    pub name: TextBlock,
    #[serde(rename = "spellTrap")]
    pub spell_trap: TextBlock,
    #[serde(rename = "pendulumDescription")]
    pub pendulum_description: TextBlock,
    pub effect: TextBlock,
    pub description: TextBlock,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ResourceRules {
    pub base_image: String,
    pub card_asset: String,
    pub pendulum_asset: String,
    pub attribute_asset: String,
    pub spell_trap_attribute_asset: String,
    pub rare_asset: String,
    pub copyright_asset: String,
    pub laser_asset: String,
    pub atk_def_asset: HashMap<String, String>,
    pub atk_link_asset: HashMap<String, String>,
}

// ── Top-level layout ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct LayoutPayload {
    pub card: CardCanvas,
    pub base: BaseLayout,
    pub styles: HashMap<String, StyleDefinition>,
    pub resource_rules: ResourceRules,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CardCanvas {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NameBase {
    pub x: u32,
    pub width_with_attribute: u32,
    pub width_without_attribute: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Position {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PositionedAsset {
    pub asset: String,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LevelRankBase {
    pub asset: String,
    pub star_width: u32,
    pub y: u32,
    pub gap: u32,
    pub max: u32,
    pub right_lt_13: Option<u32>,
    pub right_ge_13: Option<u32>,
    pub left_lt_13: Option<u32>,
    pub left_ge_13: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SpellTrapAssetRule {
    pub icon_asset_prefix: String,
    pub icon_asset_suffix: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FrameRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ImageFrames {
    pub normal: FrameRect,
    pub pendulum: FrameRect,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MaskFrames {
    pub normal: PositionedAsset,
    pub pendulum: PositionedAsset,
    #[serde(default)]
    pub pendulum_art: Option<PositionedAsset>,
    #[serde(default)]
    pub pendulum_border: Option<PositionedAsset>,
    #[serde(default)]
    pub pendulum_effect: Option<PositionedAsset>,
    #[serde(default)]
    pub pendulum_effect_border: Option<PositionedAsset>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OutFrameLayout {
    pub image: FrameRect,
    pub name_block: PositionedAsset,
    pub effect_box: PositionedAsset,
    pub effect_box_colored: PositionedAsset,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PendulumScaleSide {
    pub astral: Position,
    pub default: Position,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PendulumScaleLayout {
    pub left: PendulumScaleSide,
    pub right: PendulumScaleSide,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BoxRect {
    pub x: u32,
    pub y: Option<u32>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PackageVariant {
    pub x: Option<u32>,
    pub y: u32,
    pub right: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PackageLayout {
    pub font_family: String,
    pub font_size: u32,
    pub pendulum: PackageVariant,
    pub default: PackageVariant,
    pub link: PackageVariant,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ArrowState {
    pub asset: String,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ArrowPair {
    pub on: ArrowState,
    pub off: ArrowState,
    #[serde(default)]
    pub red_mask: Option<ArrowState>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AtkDefLinkText {
    pub x: u32,
    pub y: u32,
    pub font_size: u32,
    pub scale_x: Option<f32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LanguageAwareText {
    pub astral: AtkDefLinkText,
    pub default: AtkDefLinkText,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AtkDefLinkLayout {
    pub background: Position,
    pub atk: LanguageAwareText,
    pub def: LanguageAwareText,
    pub link: LanguageAwareText,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DescriptionBase {
    pub x: u32,
    pub width: u32,
    pub base_height: u32,
    pub atk_bar_height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PasswordLayout {
    pub x: u32,
    pub y: u32,
    pub font_family: String,
    pub font_size: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CopyrightLayout {
    pub right: u32,
    pub y: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BaseLayout {
    pub name: NameBase,
    pub attribute: Position,
    pub level: LevelRankBase,
    pub rank: LevelRankBase,
    pub spell_trap: SpellTrapAssetRule,
    pub image: ImageFrames,
    pub mask: MaskFrames,
    pub out_frame: OutFrameLayout,
    pub pendulum_scale: PendulumScaleLayout,
    pub pendulum_description: BoxRect,
    pub package: PackageLayout,
    pub link_arrows: HashMap<String, ArrowPair>,
    pub effect: BoxRect,
    pub description: DescriptionBase,
    pub atk_def_link: AtkDefLinkLayout,
    pub password: PasswordLayout,
    pub copyright: CopyrightLayout,
    pub laser: Position,
    pub attribute_rare: PositionedAsset,
    pub twentieth: PositionedAsset,
    pub twenty_fifth: PositionedAsset,
}
