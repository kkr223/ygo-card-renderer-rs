use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ygopro_cdb_encode_rs::CardDataEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardKind {
    Yugioh,
    RushDuel,
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
    #[error("svg parse error: {0}")]
    SvgParse(String),
    #[error("png encode error: {0}")]
    PngEncode(String),
}
