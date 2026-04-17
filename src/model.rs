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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
    pub resource_path: PathBuf,
    pub language: Option<String>,
    pub art_image: Option<PathBuf>,
    pub scale: f32,
    pub output_kind: Option<CardKind>,
    pub name_color_override: Option<String>,
    pub description_color_override: Option<String>,
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
            name_color_override: None,
            description_color_override: None,
            layout_overrides: LayoutOverrides::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRequest {
    pub kind: CardKind,
    pub card: CardDataEntry,
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
