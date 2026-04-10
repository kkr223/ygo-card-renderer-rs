use std::{
    fs,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use resvg::{tiny_skia, usvg};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ygopro_cdb_encode_rs::{
    CardDataEntry, TYPE_FUSION, TYPE_LINK, TYPE_MONSTER, TYPE_PENDULUM, TYPE_RITUAL, TYPE_SYNCHRO,
    TYPE_XYZ,
};

const CARD_WIDTH: u32 = 1394;
const CARD_HEIGHT: u32 = 2031;

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

pub fn render_svg(request: &RenderRequest) -> Result<String, RenderError> {
    let scale = normalize_scale(request.options.scale);
    let background_href = resolve_background_data_uri(request)?;
    let art_href = request
        .options
        .art_image
        .as_ref()
        .and_then(|path| read_data_uri(path).ok());

    let title_color =
        request
            .options
            .name_color_override
            .as_deref()
            .unwrap_or(match request.kind {
                CardKind::Yugioh => "#16120f",
                CardKind::RushDuel => "#101010",
            });
    let desc_color = request
        .options
        .description_color_override
        .as_deref()
        .unwrap_or("#111111");

    let stats_label = if request.card.is_link() {
        format!(
            "ATK/{}  LINK-{}",
            display_stat(request.card.attack),
            request.card.level.max(1)
        )
    } else {
        format!(
            "ATK/{}  DEF/{}",
            display_stat(request.card.attack),
            display_stat(request.card.defense)
        )
    };

    let primary_line = build_primary_line(&request.card, request.kind);
    let desc = if request.card.desc.trim().is_empty() {
        " ".to_string()
    } else {
        request.card.desc.clone()
    };

    let mut svg = String::new();
    svg.push_str(&format!(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
    scaled_size(CARD_WIDTH, scale),
    scaled_size(CARD_HEIGHT, scale),
    CARD_WIDTH,
    CARD_HEIGHT
  ));
    svg.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#f4efe7\"/>");

    if let Some(href) = background_href {
        svg.push_str(&format!(
      "<image x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" href=\"{}\" preserveAspectRatio=\"none\"/>",
      CARD_WIDTH, CARD_HEIGHT, href
    ));
    }

    svg.push_str("<rect x=\"130\" y=\"326\" width=\"1134\" height=\"1134\" rx=\"12\" fill=\"#e8dcc5\" opacity=\"0.95\"/>");
    if let Some(href) = art_href {
        svg.push_str("<clipPath id=\"art-clip\"><rect x=\"142\" y=\"338\" width=\"1110\" height=\"1110\" rx=\"8\"/></clipPath>");
        svg.push_str(&format!(
      "<image x=\"142\" y=\"338\" width=\"1110\" height=\"1110\" href=\"{}\" preserveAspectRatio=\"xMidYMid slice\" clip-path=\"url(#art-clip)\"/>",
      href
    ));
    }

    svg.push_str(&format!(
    "<text x=\"80\" y=\"154\" font-size=\"64\" font-family=\"'Times New Roman', serif\" fill=\"{}\">{}</text>",
    title_color,
    escape_xml(&request.card.name)
  ));
    svg.push_str(&format!(
    "<text x=\"80\" y=\"235\" font-size=\"34\" font-family=\"'Noto Sans CJK SC', 'Microsoft YaHei', sans-serif\" fill=\"#3a2b1f\">{}</text>",
    escape_xml(&primary_line)
  ));
    svg.push_str(&format!(
    "<text x=\"80\" y=\"1530\" font-size=\"36\" font-family=\"'Noto Sans CJK SC', 'Microsoft YaHei', sans-serif\" fill=\"#222\">{}</text>",
    escape_xml(&build_scale_line(&request.card))
  ));
    svg.push_str(&format!(
    "<text x=\"86\" y=\"1606\" font-size=\"34\" font-family=\"'Noto Sans CJK SC', 'Microsoft YaHei', sans-serif\" fill=\"{}\">{}</text>",
    desc_color,
    escape_xml(&summarize_multiline(&desc))
  ));
    svg.push_str(&format!(
    "<text x=\"975\" y=\"1912\" text-anchor=\"end\" font-size=\"40\" font-family=\"'Times New Roman', serif\" fill=\"#18110d\">{}</text>",
    escape_xml(&stats_label)
  ));
    svg.push_str(&format!(
    "<text x=\"86\" y=\"1912\" font-size=\"28\" font-family=\"'Noto Sans CJK SC', 'Microsoft YaHei', sans-serif\" fill=\"#5d5146\">ID {}</text>",
    request.card.code
  ));
    svg.push_str("</svg>");
    Ok(svg)
}

pub fn render_png(request: &RenderRequest) -> Result<Vec<u8>, RenderError> {
    let svg = render_svg(request)?;
    let opt = usvg::Options::default();
    let tree =
        usvg::Tree::from_str(&svg, &opt).map_err(|err| RenderError::SvgParse(err.to_string()))?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| RenderError::PngEncode("failed to allocate pixmap".to_string()))?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    pixmap
        .encode_png()
        .map_err(|err| RenderError::PngEncode(err.to_string()))
}

fn normalize_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn scaled_size(value: u32, scale: f32) -> u32 {
    ((value as f32) * scale).round().max(1.0) as u32
}

fn resolve_background_data_uri(request: &RenderRequest) -> Result<Option<String>, RenderError> {
    let relative = match request.kind {
        CardKind::Yugioh => background_rel_path_yugioh(&request.card),
        CardKind::RushDuel => background_rel_path_rush(&request.card),
    };
    let path = request.options.resource_path.join(relative);
    if path.exists() {
        Ok(Some(read_data_uri(&path)?))
    } else {
        Ok(None)
    }
}

fn background_rel_path_yugioh(card: &CardDataEntry) -> &'static str {
    if card.is_spell() {
        "yugioh/image/card-spell.png"
    } else if card.is_trap() {
        "yugioh/image/card-trap.png"
    } else if (card.type_ & TYPE_LINK) != 0 {
        "yugioh/image/card-link.png"
    } else if (card.type_ & TYPE_XYZ) != 0 && (card.type_ & TYPE_PENDULUM) != 0 {
        "yugioh/image/card-xyz-pendulum.png"
    } else if (card.type_ & TYPE_XYZ) != 0 {
        "yugioh/image/card-xyz.png"
    } else if (card.type_ & TYPE_SYNCHRO) != 0 && (card.type_ & TYPE_PENDULUM) != 0 {
        "yugioh/image/card-synchro-pendulum.png"
    } else if (card.type_ & TYPE_SYNCHRO) != 0 {
        "yugioh/image/card-synchro.png"
    } else if (card.type_ & TYPE_FUSION) != 0 && (card.type_ & TYPE_PENDULUM) != 0 {
        "yugioh/image/card-fusion-pendulum.png"
    } else if (card.type_ & TYPE_FUSION) != 0 {
        "yugioh/image/card-fusion.png"
    } else if (card.type_ & TYPE_RITUAL) != 0 && (card.type_ & TYPE_PENDULUM) != 0 {
        "yugioh/image/card-ritual-pendulum.png"
    } else if (card.type_ & TYPE_RITUAL) != 0 {
        "yugioh/image/card-ritual.png"
    } else if (card.type_ & TYPE_PENDULUM) != 0 {
        "yugioh/image/card-effect-pendulum.png"
    } else if (card.type_ & TYPE_MONSTER) != 0 {
        "yugioh/image/card-effect.png"
    } else {
        "yugioh/image/card-normal.png"
    }
}

fn background_rel_path_rush(card: &CardDataEntry) -> &'static str {
    if card.is_spell() {
        "rush-duel/image/card-spell.png"
    } else if card.is_trap() {
        "rush-duel/image/card-trap.png"
    } else if (card.type_ & TYPE_FUSION) != 0 {
        "rush-duel/image/card-fusion.png"
    } else if (card.type_ & TYPE_RITUAL) != 0 {
        "rush-duel/image/card-ritual.png"
    } else if (card.type_ & TYPE_MONSTER) != 0 && (card.type_ & 0x20) != 0 {
        "rush-duel/image/card-effect.png"
    } else {
        "rush-duel/image/card-normal.png"
    }
}

fn build_primary_line(card: &CardDataEntry, kind: CardKind) -> String {
    let card_family = if card.is_spell() {
        "Spell"
    } else if card.is_trap() {
        "Trap"
    } else if card.is_link() {
        "Link Monster"
    } else if (card.type_ & TYPE_XYZ) != 0 {
        "Xyz Monster"
    } else if (card.type_ & TYPE_SYNCHRO) != 0 {
        "Synchro Monster"
    } else if (card.type_ & TYPE_FUSION) != 0 {
        "Fusion Monster"
    } else if (card.type_ & TYPE_RITUAL) != 0 {
        "Ritual Monster"
    } else {
        "Monster"
    };

    let prefix = match kind {
        CardKind::Yugioh => "YGO",
        CardKind::RushDuel => "RD",
    };
    format!("{prefix}  {card_family}  Type={:#x}", card.type_)
}

fn build_scale_line(card: &CardDataEntry) -> String {
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

fn summarize_multiline(value: &str) -> String {
    value.lines().take(5).collect::<Vec<_>>().join(" / ")
}

fn display_stat(value: i32) -> String {
    match value {
        -2 => "INF".to_string(),
        -1 => "?".to_string(),
        other => other.to_string(),
    }
}

fn read_data_uri(path: &Path) -> Result<String, RenderError> {
    let bytes = fs::read(path)?;
    let mime = match path
        .extension()
        .and_then(|item| item.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
