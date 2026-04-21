use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ygopro_cdb_encode_rs::CardDataEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardKind {
    Yugioh,
    RushDuel,
}

// ── Extra display metadata (not stored in CDB) ─────────────────────────────

/// Rare/foil stamp overlaid in the card art area.
/// Variant names mirror the asset filename stems (`rare-{variant}[‑pendulum].webp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RareType {
    /// Holographic Rare
    Hr,
    /// Gold Rare
    Gr,
    /// Ultimate Rare
    Ur,
    /// Secret Rare
    Ser,
    /// Gold Secret Rare
    Gser,
    /// Prismatic Secret Rare
    Pser,
    /// Prismatic Secret Rare (print)
    PserPrint,
    /// Duel Terminal parallel rare
    Dt,
}

impl RareType {
    /// Asset filename stem, e.g. `"hr"` or `"pser-print"`.
    pub fn asset_stem(self) -> &'static str {
        match self {
            Self::Hr => "hr",
            Self::Gr => "gr",
            Self::Ur => "ur",
            Self::Ser => "ser",
            Self::Gser => "gser",
            Self::Pser => "pser",
            Self::PserPrint => "pser-print",
            Self::Dt => "dt",
        }
    }

    /// Whether this rare type also shows the attribute-rare overlay
    /// (holographic border around the attribute icon).
    pub fn shows_attribute_rare(self) -> bool {
        matches!(self, Self::Hr | Self::Ser | Self::Gser | Self::Pser)
    }
}

/// How to color the card name text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum NameColor {
    /// Automatically choose dark/light based on card type:
    /// dark for normal/effect/ritual/fusion/synchro/token,
    /// light for xyz/link and spell/trap.
    #[default]
    Auto,
    /// Force the standard dark color.
    Dark,
    /// Force the standard light (white) color.
    Light,
    /// Arbitrary CSS-style hex or named color string.
    Custom(String),
}

/// Extended card data: wraps a [`CardDataEntry`] with display metadata that
/// is not stored in the CDB format.
///
/// `Deref` to `CardDataEntry` is implemented so that all helper methods
/// (`is_spell`, `is_link`, field access, …) work directly on this type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YgoCardMeta {
    /// Core CDB card data (flattened into the same JSON object).
    #[serde(flatten)]
    pub entry: CardDataEntry,

    /// Rare/foil stamp overlay. `None` means no stamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rare: Option<RareType>,

    /// Card name color. Defaults to [`NameColor::Auto`].
    #[serde(default)]
    pub name_color: NameColor,

    /// Card set / package label shown near the bottom of the card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,

    /// Copyright line (right side of bottom bar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,

    /// Laser hologram asset identifier (e.g. `"laser1"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub laser: Option<String>,

    /// Show the 20th anniversary mark overlay.
    #[serde(default)]
    pub twentieth: bool,
}

impl YgoCardMeta {
    /// Wrap a bare `CardDataEntry` with all-default extra metadata.
    pub fn from_entry(entry: CardDataEntry) -> Self {
        Self {
            entry,
            rare: None,
            name_color: NameColor::Auto,
            package: None,
            copyright: None,
            laser: None,
            twentieth: false,
        }
    }
}

impl std::ops::Deref for YgoCardMeta {
    type Target = CardDataEntry;

    fn deref(&self) -> &CardDataEntry {
        &self.entry
    }
}

impl std::ops::DerefMut for YgoCardMeta {
    fn deref_mut(&mut self) -> &mut CardDataEntry {
        &mut self.entry
    }
}

impl From<CardDataEntry> for YgoCardMeta {
    fn from(entry: CardDataEntry) -> Self {
        Self::from_entry(entry)
    }
}

/// 允许调用方对任意布局参数进行精确覆盖。
/// 字段名与 `LayoutStyle` 一一对应，`None` 表示"使用默认值"。
/// 单位：像素（与 PSD 画布坐标系一致，卡片基准尺寸 1394×2031）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutOverrides {
    pub name_top: Option<u32>,
    pub name_size: Option<u32>,
    pub name_x: Option<u32>,
    pub title_max_width_with_attribute: Option<u32>,
    pub title_max_width_without_attribute: Option<u32>,
    pub title_letter_spacing: Option<f32>,

    pub type_top: Option<u32>,
    pub type_size: Option<u32>,
    pub type_letter_spacing: Option<f32>,

    pub effect_top: Option<u32>,
    pub effect_size: Option<u32>,
    pub effect_line_height: Option<f32>,
    pub effect_x: Option<u32>,
    pub effect_letter_spacing: Option<f32>,
    pub effect_text_indent: Option<i32>,

    pub description_size: Option<u32>,
    pub description_line_height: Option<f32>,
    pub description_x: Option<u32>,
    pub description_letter_spacing: Option<f32>,

    pub body_max_width: Option<u32>,

    pub pendulum_description_top: Option<u32>,
    pub pendulum_description_size: Option<u32>,

    pub stat_atk_x: Option<u32>,
    pub stat_def_x: Option<u32>,
    pub stat_link_x: Option<u32>,
    pub stat_top: Option<u32>,
    pub stat_size: Option<u32>,
    pub stat_letter_spacing: Option<f32>,

    pub link_top: Option<u32>,
    pub link_size: Option<u32>,

    // copyright 文本位置（right = 距卡片右边缘距离，y = 顶部偏移）
    pub copyright_right: Option<u32>,
    pub copyright_y: Option<u32>,

    // 卡包编码文本 y 坐标（三种变体：普通/灵摆/link）
    pub package_y: Option<u32>,
    pub package_y_pendulum: Option<u32>,
    pub package_y_link: Option<u32>,

    // 左下角密码/ID 文本位置
    pub password_x: Option<u32>,
    pub password_y: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
    pub resource_path: PathBuf,
    pub language: Option<String>,
    pub art_image: Option<PathBuf>,
    pub scale: f32,
    pub output_kind: Option<CardKind>,
    /// Override the description text color (CSS-style hex or named color).
    pub description_color_override: Option<String>,
    #[serde(default)]
    pub title_width_compress: bool,
    #[serde(default)]
    pub description_first_line_compress: bool,
    /// 逐字段覆盖布局参数。优先级：此字段 > bundle text_layout > 硬编码默认值。
    #[serde(default)]
    pub layout_overrides: LayoutOverrides,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            resource_path: PathBuf::new(),
            language: None,
            art_image: None,
            scale: 1.0,
            output_kind: None,
            description_color_override: None,
            title_width_compress: false,
            description_first_line_compress: false,
            layout_overrides: LayoutOverrides::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRequest {
    pub kind: CardKind,
    /// Extended card data: CDB entry + display metadata not stored in CDB.
    pub card: YgoCardMeta,
    pub options: RenderOptions,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("render backend error: {0}")]
    Backend(String),
    #[error("svg parse error: {0}")]
    SvgParse(String),
    #[error("png encode error: {0}")]
    PngEncode(String),
}
