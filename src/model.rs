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
    /// Super Rare
    Sr,
    /// Holographic Rare
    Hr,
    /// Gold Rare
    Gr,
    /// Ultra Rare
    Ur,
    /// Ultimate Rare / UTR
    Utr,
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

/// Effect text box used by out-frame cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OutFrameEffectBox {
    /// Original out-frame effect box (`eblock-border.webp`).
    #[default]
    #[serde(alias = "original")]
    EblockBorder,
    /// Alternate-color out-frame effect box (`eblock-border-o.webp`).
    #[serde(alias = "colored")]
    EblockBorderO,
}

impl RareType {
    /// Asset filename stem, e.g. `"hr"` or `"pser-print"`.
    pub fn asset_stem(self) -> &'static str {
        match self {
            Self::Sr => "sr",
            Self::Hr => "hr",
            Self::Gr => "gr",
            Self::Ur => "ur",
            Self::Utr => "utr",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TextAlignChoice {
    #[default]
    Left,
    Center,
    Right,
}

/// CSS-style two-stop horizontal gradient for text rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextGradient {
    pub start: String,
    pub end: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub middle: Option<String>,
    #[serde(default, skip_serializing_if = "GradientDirection::is_default")]
    pub direction: GradientDirection,
}

impl TextGradient {
    pub fn new(start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            middle: None,
            direction: GradientDirection::Horizontal,
        }
    }

    pub fn vertical_middle(
        start: impl Into<String>,
        middle: impl Into<String>,
        end: impl Into<String>,
    ) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            middle: Some(middle.into()),
            direction: GradientDirection::Vertical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum GradientDirection {
    #[default]
    Horizontal,
    Vertical,
}

impl GradientDirection {
    fn is_default(&self) -> bool {
        *self == Self::Horizontal
    }
}

/// A serializable text paint descriptor accepted by render options.
///
/// `color` is used as a solid fallback. When `gradient` is present and both
/// stops parse successfully, the renderer uses a horizontal gradient over the
/// target text layout box.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPaint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<TextGradient>,
}

impl TextPaint {
    pub fn solid(color: impl Into<String>) -> Self {
        Self {
            color: Some(color.into()),
            gradient: None,
        }
    }

    pub fn gradient(start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            color: None,
            gradient: Some(TextGradient::new(start, end)),
        }
    }
}

/// Optional color overrides for card text channels.
///
/// This mirrors DataEditorY's card-image controls for name fill/shadow while
/// leaving room for later UI controls on effect, description, stats, and footer
/// text without changing the render request shape again.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TextColorOverrides {
    pub name: Option<TextPaint>,
    pub name_shadow: Option<TextPaint>,
    pub effect: Option<TextPaint>,
    pub effect_shadow: Option<TextPaint>,
    pub description: Option<TextPaint>,
    pub description_shadow: Option<TextPaint>,
    pub type_line: Option<TextPaint>,
    pub type_line_shadow: Option<TextPaint>,
    pub stats: Option<TextPaint>,
    pub stats_shadow: Option<TextPaint>,
    pub password: Option<TextPaint>,
    pub password_shadow: Option<TextPaint>,
    pub package: Option<TextPaint>,
    pub package_shadow: Option<TextPaint>,
    pub copyright: Option<TextPaint>,
    pub copyright_shadow: Option<TextPaint>,
}

/// Additional alpha image composited over the card surface.
///
/// The image is drawn at its authored size; callers are responsible for
/// preparing any scale/rotation externally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionedRenderImage {
    pub path: PathBuf,
    pub x: i32,
    pub y: i32,
}

/// Optional grayscale protection mask for visual/rare effects.
///
/// Mask pixels are interpreted like print white ink: opaque black protects the
/// underlying card from effects, opaque white allows effects, and gray/alpha
/// values create a soft transition.  When `x`/`y` are omitted the renderer
/// places full-card masks at `(0, 0)` and art-sized masks on the illustration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectMask {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
}

fn default_true() -> bool {
    true
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

    /// Optional card-name gradient. Kept near `name_color` to align with
    /// DataEditorY's current card-image form fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_gradient: Option<TextGradient>,

    /// Optional card-name shadow color. `None` means no custom shadow layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_shadow_color: Option<String>,

    /// Optional card-name shadow gradient. Takes precedence over
    /// `name_shadow_color` when both stops are valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_shadow_gradient: Option<TextGradient>,

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

    /// Show the 25th anniversary mark overlay.
    #[serde(default)]
    pub twenty_fifth: bool,

    /// Render as an out-frame card, allowing transparent art to extend beyond
    /// the normal illustration mask.
    #[serde(default)]
    pub out_frame: bool,

    /// Out-frame alpha image drawn at its authored size and position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_frame_image: Option<PositionedRenderImage>,

    /// Whether the out-frame effect box area should be drawn.
    #[serde(default = "default_true")]
    pub out_frame_effect_enabled: bool,

    /// Which out-frame effect box resource to draw.
    #[serde(default)]
    pub out_frame_effect_box: OutFrameEffectBox,

    /// Optional solid color under the out-frame effect box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_frame_effect_background_color: Option<String>,

    /// Opacity multiplier for the out-frame effect background color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_frame_effect_opacity: Option<f32>,

    /// Whether the out-frame card name block should be drawn.
    #[serde(default = "default_true")]
    pub out_frame_name_block_enabled: bool,

    /// Output image scale. `None` falls back to [`RenderOptions::scale`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
}

impl YgoCardMeta {
    /// Wrap a bare `CardDataEntry` with all-default extra metadata.
    pub fn from_entry(entry: CardDataEntry) -> Self {
        Self {
            entry,
            rare: None,
            name_color: NameColor::Auto,
            name_gradient: None,
            name_shadow_color: None,
            name_shadow_gradient: None,
            package: None,
            copyright: None,
            laser: None,
            twentieth: false,
            twenty_fifth: false,
            out_frame: false,
            out_frame_image: None,
            out_frame_effect_enabled: true,
            out_frame_effect_box: OutFrameEffectBox::default(),
            out_frame_effect_background_color: None,
            out_frame_effect_opacity: None,
            out_frame_name_block_enabled: true,
            scale: None,
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
    pub language: Option<String>,
    pub art_image: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_mask: Option<EffectMask>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_image: Option<PositionedRenderImage>,
    pub scale: f32,
    /// Override the description text color (CSS-style hex or named color).
    pub description_color_override: Option<String>,
    /// Color/gradient overrides for text channels.
    #[serde(default)]
    pub text_colors: TextColorOverrides,
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
            language: None,
            art_image: None,
            effect_mask: None,
            foreground_image: None,
            scale: 1.0,
            description_color_override: None,
            text_colors: TextColorOverrides::default(),
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
