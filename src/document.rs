use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    asset_bundle::AssetBundle,
    card_logic::{
        attribute_asset_name, build_effect_line, description_height, description_y,
        frame_asset_name, image_frame, localized_spell_trap_name, spell_trap_subtype_icon_asset,
        split_pendulum_description, uses_rank,
    },
    constants::{BACKGROUND_CREAM, CARD_HEIGHT, CARD_WIDTH},
    layout::layout_style,
    model::{
        CardKind, NameColor, PositionedRenderImage, RareType, RenderOptions, RenderRequest,
        TextAlignChoice, TextGradient, TextPaint, YgoCardMeta,
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderDocument {
    pub schema_version: u32,
    pub kind: CardKind,
    pub canvas: RenderCanvas,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub output_scale: f32,
    pub card: YgoCardMeta,
    pub options: RenderOptions,
    pub nodes: Vec<RenderNode>,
}

impl RenderDocument {
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn from_request(request: &RenderRequest, bundle: &AssetBundle) -> Self {
        let language = request.options.language.as_deref();
        let base = &bundle.layout.base;
        let style = layout_style(
            request.kind,
            language,
            &bundle.layout,
            &request.options.layout_overrides,
        );
        let output_scale = effective_output_scale(request);
        let mut nodes = Vec::new();

        nodes.push(RenderNode::new(
            "frame",
            0,
            RenderOp::BundleImage {
                asset: frame_asset_name(&request.card).to_string(),
                x: 0.0,
                y: 0.0,
            },
        ));

        let (art_x, art_y, art_w, art_h) = image_frame(&request.card, base);
        nodes.push(RenderNode::new(
            "art",
            10,
            RenderOp::ExternalImage {
                path: request.options.art_image.clone(),
                rect: RenderRect::new(art_x, art_y, art_w, art_h),
                fit: ImageFit::Cover,
                align: ImageAlign::Top,
            },
        ));

        let mask = if request.card.is_pendulum() {
            &base.mask.pendulum
        } else {
            &base.mask.normal
        };
        nodes.push(RenderNode::new(
            "mask",
            20,
            RenderOp::BundleImage {
                asset: mask.asset.clone(),
                x: mask.x as f32,
                y: mask.y as f32,
            },
        ));

        add_rare_effect_nodes(&mut nodes, request.card.rare);

        if let Some(image) = foreground_image_for_request(request) {
            nodes.push(RenderNode::new(
                "foreground",
                40,
                RenderOp::PositionedImage {
                    image: image.clone(),
                },
            ));
        }

        if request.card.out_frame {
            nodes.push(RenderNode::new("out-frame", 50, RenderOp::OutFrameBlocks));
        }

        if request.card.twenty_fifth || request.card.twentieth {
            nodes.push(RenderNode::new(
                "anniversary-mark",
                60,
                RenderOp::AnniversaryMark,
            ));
        }

        let attribute_asset = attribute_asset_name(&request.card, language);
        nodes.push(RenderNode::new(
            "attribute",
            70,
            RenderOp::Attribute {
                asset: attribute_asset,
                x: base.attribute.x as f32,
                y: base.attribute.y as f32,
            },
        ));

        if !request.card.is_link() && request.card.level > 0 {
            nodes.push(RenderNode::new(
                if uses_rank(&request.card) {
                    "rank"
                } else {
                    "level"
                },
                80,
                RenderOp::LevelOrRank,
            ));
        }

        if request.card.is_link() {
            nodes.push(RenderNode::new(
                "link-arrows",
                90,
                RenderOp::LinkArrows {
                    arrows: link_arrows_for_request(request),
                },
            ));
        }

        let title_width =
            if request.card.attribute != 0 || request.card.is_spell() || request.card.is_trap() {
                base.name.width_with_attribute
            } else {
                base.name.width_without_attribute
            };
        let (title_fill, title_shadow) = rare_title_paints(request.card.rare);
        nodes.push(RenderNode::new(
            "title",
            100,
            RenderOp::Title {
                text: request.card.name.clone(),
                rect: RenderRect::new(style.name_x, style.name_top, title_width, base.name.height),
                font_family: style.name_font_family.clone(),
                font_size: style.name_size,
                letter_spacing: style.title_letter_spacing,
                color: request.card.name_color.clone(),
                width_compress: request.options.title_width_compress,
                align: TextAlignChoice::Left,
                fill: title_fill,
                shadow: title_shadow,
            },
        ));

        if request.card.is_spell() || request.card.is_trap() {
            nodes.push(RenderNode::new(
                "spell-trap-line",
                110,
                RenderOp::SpellTrapLine {
                    label: localized_spell_trap_name(&request.card, language).to_string(),
                    icon_asset: spell_trap_subtype_icon_asset(&request.card).map(str::to_string),
                },
            ));
        } else if let Some(text) = build_effect_line(&request.card, request.kind, language) {
            nodes.push(RenderNode::new(
                "monster-type-line",
                110,
                RenderOp::MonsterTypeLine {
                    text,
                    rect: RenderRect::new(
                        style.effect_x,
                        style.effect_top,
                        base.effect.width,
                        base.effect.height,
                    ),
                    font_family: style.effect_font_family.clone(),
                    font_size: style.effect_size,
                    letter_spacing: style.effect_letter_spacing,
                },
            ));
        }

        let description_text = if request.card.is_pendulum() {
            let sections = split_pendulum_description(&request.card.desc, language);
            if let Some(text) = sections.pendulum_effect {
                nodes.push(RenderNode::new(
                    "pendulum-description",
                    120,
                    RenderOp::TextBlock {
                        text,
                        rect: RenderRect::new(
                            base.pendulum_description.x,
                            style.pendulum_description_top,
                            base.pendulum_description.width,
                            base.pendulum_description.height,
                        ),
                        font_family: style.base_font_family.clone(),
                        font_size: style.pendulum_description_size,
                        line_height: style.pendulum_description_line_height,
                        letter_spacing: style.pendulum_description_letter_spacing,
                        channel: TextChannel::Description,
                    },
                ));
            }
            sections.monster_effect
        } else {
            request.card.desc.clone()
        };

        nodes.push(RenderNode::new(
            "description",
            130,
            RenderOp::TextBlock {
                text: description_text,
                rect: RenderRect::new(
                    style.description_x,
                    description_y(&request.card, &style),
                    style.body_max_width,
                    description_height(&request.card, &style, base),
                ),
                font_family: style.base_font_family.clone(),
                font_size: style.description_size,
                line_height: style.description_line_height,
                letter_spacing: style.description_letter_spacing,
                channel: TextChannel::Description,
            },
        ));

        if request.card.is_monster() || request.card.is_pendulum() {
            nodes.push(RenderNode::new("stats", 140, RenderOp::Stats));
        }

        nodes.push(RenderNode::new(
            "password",
            150,
            RenderOp::Password {
                text: request.card.code.to_string(),
                x: request
                    .options
                    .layout_overrides
                    .password_x
                    .unwrap_or(base.password.x) as f32,
                y: request
                    .options
                    .layout_overrides
                    .password_y
                    .unwrap_or(base.password.y) as f32,
            },
        ));

        if let Some(package) = &request.card.package {
            nodes.push(RenderNode::new(
                "package",
                160,
                RenderOp::Package {
                    text: package.clone(),
                },
            ));
        }

        if let Some(copyright) = &request.card.copyright {
            nodes.push(RenderNode::new(
                "copyright",
                170,
                RenderOp::Copyright {
                    value: copyright.clone(),
                    asset: copyright_asset_name(&request.card, copyright),
                },
            ));
        }

        if let Some(asset) = request.card.laser.as_deref().and_then(laser_asset_name) {
            nodes.push(RenderNode::new(
                "laser",
                180,
                RenderOp::BundleImage {
                    asset,
                    x: base.laser.x as f32,
                    y: base.laser.y as f32,
                },
            ));
        }

        Self {
            schema_version: Self::SCHEMA_VERSION,
            kind: request.kind,
            canvas: RenderCanvas {
                width: CARD_WIDTH,
                height: CARD_HEIGHT,
                background: Some(format!(
                    "#{:02x}{:02x}{:02x}",
                    BACKGROUND_CREAM.0, BACKGROUND_CREAM.1, BACKGROUND_CREAM.2
                )),
            },
            language: request.options.language.clone(),
            output_scale,
            card: request.card.clone(),
            options: request.options.clone(),
            nodes,
        }
    }

    pub fn to_request(&self) -> RenderRequest {
        let mut options = self.options.clone();
        options.language = self.language.clone();
        RenderRequest {
            kind: self.kind,
            card: self.card.clone(),
            options,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderCanvas {
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderNode {
    pub id: String,
    pub z: i32,
    #[serde(default = "default_visible")]
    pub visible: bool,
    #[serde(flatten)]
    pub op: RenderOp,
}

impl RenderNode {
    pub fn new(id: impl Into<String>, z: i32, op: RenderOp) -> Self {
        Self {
            id: id.into(),
            z,
            visible: true,
            op,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RenderOp {
    BundleImage {
        asset: String,
        x: f32,
        y: f32,
    },
    ExternalImage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<PathBuf>,
        rect: RenderRect,
        fit: ImageFit,
        align: ImageAlign,
    },
    PositionedImage {
        image: PositionedRenderImage,
    },
    VisualEffect {
        target: EffectTarget,
        effect: EffectStyle,
    },
    OutFrameBlocks,
    AnniversaryMark,
    Attribute {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        asset: Option<String>,
        x: f32,
        y: f32,
    },
    LevelOrRank,
    LinkArrows {
        arrows: Vec<u8>,
    },
    Title {
        text: String,
        rect: RenderRect,
        font_family: String,
        font_size: u32,
        letter_spacing: f32,
        color: NameColor,
        width_compress: bool,
        align: TextAlignChoice,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<TextPaint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<TextPaint>,
    },
    SpellTrapLine {
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon_asset: Option<String>,
    },
    MonsterTypeLine {
        text: String,
        rect: RenderRect,
        font_family: String,
        font_size: u32,
        letter_spacing: f32,
    },
    TextBlock {
        text: String,
        rect: RenderRect,
        font_family: String,
        font_size: u32,
        line_height: f32,
        letter_spacing: f32,
        channel: TextChannel,
    },
    Stats,
    Password {
        text: String,
        x: f32,
        y: f32,
    },
    Package {
        text: String,
    },
    Copyright {
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        asset: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RenderRect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x: x as f32,
            y: y as f32,
            width: width as f32,
            height: height as f32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageFit {
    Stretch,
    Cover,
    Contain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageAlign {
    Top,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextChannel {
    Title,
    TypeLine,
    Description,
    Stats,
    Footer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EffectTarget {
    Art,
    ArtFrame,
    CardBase,
    CardBorder,
    FullCard,
    Attribute,
    LevelOrRank,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum EffectStyle {
    RainbowFoil { opacity: f32 },
    DotGrid { opacity: f32 },
    SecretWeave { opacity: f32 },
    Holographic { opacity: f32 },
    BrightBorder { opacity: f32 },
    GoldWash { opacity: f32 },
    FrostedFoil { opacity: f32 },
    ConcentricEngrave { opacity: f32 },
    ReliefEngrave { opacity: f32 },
}

fn add_rare_effect_nodes(nodes: &mut Vec<RenderNode>, rare: Option<RareType>) {
    let Some(rare) = rare else {
        return;
    };

    let mut push_effect = |id: &str, z: i32, target: EffectTarget, effect: EffectStyle| {
        nodes.push(RenderNode::new(
            id,
            z,
            RenderOp::VisualEffect { target, effect },
        ));
    };

    match rare {
        RareType::Sr => {
            push_effect(
                "rare-sr-art-foil",
                30,
                EffectTarget::Art,
                EffectStyle::RainbowFoil { opacity: 0.46 },
            );
        }
        RareType::Ur => {
            push_effect(
                "rare-ur-art-foil",
                30,
                EffectTarget::Art,
                EffectStyle::RainbowFoil { opacity: 0.46 },
            );
            push_effect(
                "rare-ur-attribute-foil",
                75,
                EffectTarget::Attribute,
                EffectStyle::Holographic { opacity: 0.62 },
            );
            push_effect(
                "rare-ur-level-rank-foil",
                85,
                EffectTarget::LevelOrRank,
                EffectStyle::Holographic { opacity: 0.58 },
            );
        }
        RareType::Gr => {
            push_effect(
                "rare-gr-art-foil",
                30,
                EffectTarget::Art,
                EffectStyle::RainbowFoil { opacity: 0.46 },
            );
            push_effect(
                "rare-gr-card-border-gold",
                32,
                EffectTarget::CardBorder,
                EffectStyle::GoldWash { opacity: 0.42 },
            );
            push_effect(
                "rare-gr-art-frame-gold",
                33,
                EffectTarget::ArtFrame,
                EffectStyle::GoldWash { opacity: 0.56 },
            );
            push_effect(
                "rare-gr-attribute-foil",
                75,
                EffectTarget::Attribute,
                EffectStyle::Holographic { opacity: 0.62 },
            );
            push_effect(
                "rare-gr-level-rank-foil",
                85,
                EffectTarget::LevelOrRank,
                EffectStyle::Holographic { opacity: 0.58 },
            );
        }
        RareType::Utr => {
            push_effect(
                "rare-utr-frosted-card-base",
                30,
                EffectTarget::CardBase,
                EffectStyle::FrostedFoil { opacity: 0.50 },
            );
            push_effect(
                "rare-utr-art-relief",
                31,
                EffectTarget::Art,
                EffectStyle::ReliefEngrave { opacity: 1.00 },
            );
            push_effect(
                "rare-utr-attribute-concentric-engrave",
                75,
                EffectTarget::Attribute,
                EffectStyle::ConcentricEngrave { opacity: 0.72 },
            );
            push_effect(
                "rare-utr-level-rank-concentric-engrave",
                85,
                EffectTarget::LevelOrRank,
                EffectStyle::ConcentricEngrave { opacity: 0.68 },
            );
        }
        RareType::Hr => push_effect(
            "rare-hr-full-foil",
            30,
            EffectTarget::FullCard,
            EffectStyle::Holographic { opacity: 0.45 },
        ),
        RareType::Ser => {
            push_effect(
                "rare-ser-art-weave",
                30,
                EffectTarget::Art,
                EffectStyle::SecretWeave { opacity: 0.66 },
            );
            push_effect(
                "rare-ser-attribute-weave",
                75,
                EffectTarget::Attribute,
                EffectStyle::SecretWeave { opacity: 0.66 },
            );
            push_effect(
                "rare-ser-level-rank-weave",
                85,
                EffectTarget::LevelOrRank,
                EffectStyle::SecretWeave { opacity: 0.66 },
            );
        }
        RareType::Gser => {
            push_effect(
                "rare-gser-full-weave",
                30,
                EffectTarget::FullCard,
                EffectStyle::SecretWeave { opacity: 0.58 },
            );
            push_effect(
                "rare-gser-art-foil",
                31,
                EffectTarget::Art,
                EffectStyle::RainbowFoil { opacity: 0.18 },
            );
        }
        RareType::Pser => {
            push_effect(
                "rare-pser-art-foil",
                30,
                EffectTarget::Art,
                EffectStyle::RainbowFoil { opacity: 0.50 },
            );
            push_effect(
                "rare-pser-art-dot-grid",
                31,
                EffectTarget::Art,
                EffectStyle::DotGrid { opacity: 0.60 },
            );
        }
        RareType::PserPrint => push_effect(
            "rare-pser-print-border",
            30,
            EffectTarget::FullCard,
            EffectStyle::BrightBorder { opacity: 0.72 },
        ),
        RareType::Dt => {}
    }
}

fn rare_title_paints(rare: Option<RareType>) -> (Option<TextPaint>, Option<TextPaint>) {
    match rare {
        Some(RareType::Ur | RareType::Gr) => (
            Some(TextPaint {
                color: None,
                gradient: Some(TextGradient::vertical_middle(
                    "#9a6718", "#fff0a8", "#6f4208",
                )),
            }),
            Some(TextPaint {
                color: Some("#5a3708".to_string()),
                gradient: Some(TextGradient::vertical_middle(
                    "#2d1903", "#a46a16", "#221103",
                )),
            }),
        ),
        _ => (None, None),
    }
}

fn foreground_image_for_request(request: &RenderRequest) -> Option<&PositionedRenderImage> {
    if request.card.out_frame {
        request
            .card
            .out_frame_image
            .as_ref()
            .or(request.options.foreground_image.as_ref())
    } else {
        request.options.foreground_image.as_ref()
    }
}

fn effective_output_scale(request: &RenderRequest) -> f32 {
    let scale = request.card.scale.unwrap_or(request.options.scale);
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn laser_asset_name(laser: &str) -> Option<String> {
    let laser = laser.trim();
    if laser.is_empty() {
        None
    } else if laser.ends_with(".webp") {
        Some(laser.to_string())
    } else {
        Some(format!("{laser}.webp"))
    }
}

fn copyright_asset_name(card: &YgoCardMeta, value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let value = value.strip_suffix(".svg").unwrap_or(value);
    let color = if card.is_monster() && (card.type_ & ygopro_cdb_encode_rs::TYPE_XYZ) != 0 {
        "white"
    } else {
        "black"
    };
    Some(format!("copyright-{value}-{color}.svg"))
}

fn link_arrows_for_request(request: &RenderRequest) -> Vec<u8> {
    const LINK_MARKER_TO_ARROW: &[(u32, u8)] = &[
        (0x80, 1),
        (0x100, 2),
        (0x20, 3),
        (0x04, 4),
        (0x02, 5),
        (0x01, 6),
        (0x08, 7),
        (0x40, 8),
    ];

    LINK_MARKER_TO_ARROW
        .iter()
        .filter_map(|(bit, arrow)| {
            if (request.card.link_marker & bit) != 0 {
                Some(*arrow)
            } else {
                None
            }
        })
        .collect()
}

fn default_visible() -> bool {
    true
}
