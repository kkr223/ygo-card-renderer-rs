use std::path::PathBuf;

mod layers;
mod paint;
mod rare;

use serde::{Deserialize, Serialize};

use crate::{
    asset_bundle::{AssetBundle, PositionedAsset},
    card_logic::{
        attribute_asset_name, build_effect_line, image_frame, split_pendulum_description,
    },
    constants::{BACKGROUND_CREAM, CARD_HEIGHT, CARD_WIDTH},
    layout::layout_style,
    model::{
        CardKind, PositionedRenderImage, RenderOptions, RenderRequest, TextAlignChoice, TextPaint,
        YgoCardMeta,
    },
};

// ── RenderDocument ────────────────────────────────────────────────────────────

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
    pub const SCHEMA_VERSION: u32 = 4;

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
        let card = &request.card;
        let facts = crate::facts::CardFacts::new(card);

        // ── Frame ──────────────────────────────────────────────────────────
        nodes.push(RenderNode::new(
            "frame",
            0,
            RenderOp::ImageAsset {
                asset: facts.frame_asset.to_string(),
                x: 0.0,
                y: 0.0,
            },
        ));

        // ── Art ────────────────────────────────────────────────────────────
        let (art_x, art_y, art_w, art_h) = image_frame(card, base);
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

        // ── Mask ───────────────────────────────────────────────────────────
        let mask = if card.is_pendulum() {
            &base.mask.pendulum
        } else {
            &base.mask.normal
        };
        nodes.push(RenderNode::new(
            "mask",
            20,
            RenderOp::ImageAsset {
                asset: mask.asset.clone(),
                x: mask.x as f32,
                y: mask.y as f32,
            },
        ));

        // ── Rare effects ───────────────────────────────────────────────────
        rare::push_rare_effect_nodes(&mut nodes, card.rare);

        // ── Foreground image ──────────────────────────────────────────────
        if let Some(image) = foreground_image_for_request(request) {
            nodes.push(RenderNode::new(
                "foreground",
                40,
                RenderOp::PositionedImage {
                    image: image.clone(),
                },
            ));
        }

        // ── Out-frame blocks → ImageAsset + FillRect ─────────────────────
        if card.out_frame {
            layers::frame::push_out_frame_nodes(&mut nodes, bundle, request, base);
        }

        // ── Anniversary mark → ImageAsset ─────────────────────────────────
        if card.twenty_fifth || card.twentieth {
            let mark = if card.twenty_fifth {
                Some(&base.twenty_fifth)
            } else {
                Some(&base.twentieth)
            };
            if let Some(mark) = mark {
                nodes.push(RenderNode::new(
                    "anniversary-mark",
                    60,
                    RenderOp::ImageAsset {
                        asset: mark.asset.clone(),
                        x: mark.x as f32,
                        y: mark.y as f32,
                    },
                ));
            }
        }

        // ── Attribute → ImageAsset ────────────────────────────────────────
        if let Some(asset) = attribute_asset_name(card, language) {
            nodes.push(RenderNode::new(
                "attribute",
                70,
                RenderOp::ImageAsset {
                    asset,
                    x: base.attribute.x as f32,
                    y: base.attribute.y as f32,
                },
            ));
        }

        // ── Level / Rank → ImageAsset × N ────────────────────────────────
        if !card.is_link() && card.level > 0 {
            layers::frame::push_level_or_rank_nodes(&mut nodes, card, base);
        }

        // ── Link arrows → ImageAsset × 8 ──────────────────────────────────
        if card.is_link() {
            layers::frame::push_link_arrow_nodes(&mut nodes, card, base);
        }

        // ── Title → TextLine ──────────────────────────────────────────────
        layers::text::push_title_node(&mut nodes, request, &style, base);

        // ── Spell/Trap line or monster type line → TextLine ─────────────────
        if card.is_spell() || card.is_trap() {
            layers::text::push_spell_trap_nodes(&mut nodes, bundle, request, &style, language);
        } else if let Some(text) = build_effect_line(card, request.kind, language) {
            layers::text::push_monster_type_node(&mut nodes, &style, base, &text, &request.options);
        }

        // ── Pendulum description → TextBlock ──────────────────────────────
        let description_text = if card.is_pendulum() {
            layers::text::push_pendulum_description_node(
                &mut nodes,
                card,
                &style,
                base,
                language,
                &request.options,
            );
            split_pendulum_description(&card.desc, language).monster_effect
        } else {
            card.desc.clone()
        };

        // ── Description → TextBlock ───────────────────────────────────────
        layers::text::push_description_node(
            &mut nodes,
            card,
            &style,
            base,
            &description_text,
            &request.options,
        );

        // ── Stats → TextLine × N + ImageAsset ─────────────────────────────
        layers::frame::push_stats_nodes(&mut nodes, bundle, request, &style, base, language);

        // ── Password → TextLine ───────────────────────────────────────────
        layers::footer::push_password_node(&mut nodes, request, &style, base);

        // ── Monster footer scale/link-marker line → TextLine ──────────────
        if card.is_monster() {
            layers::footer::push_scale_line_node(&mut nodes, request, &style, base);
        }

        // ── Package → TextLine ────────────────────────────────────────────
        if let Some(package) = &card.package {
            layers::footer::push_package_node(&mut nodes, request, &style, base, package);
        }

        // ── Copyright → ImageAsset or TextLine ────────────────────────────
        if let Some(copyright) = &card.copyright {
            layers::footer::push_copyright_node(
                &mut nodes,
                bundle,
                card,
                copyright,
                base,
                &style,
                &request.options,
            );
        }

        // ── Laser → ImageAsset ────────────────────────────────────────────
        if let Some(asset) = card.laser.as_deref().and_then(laser_asset_name) {
            nodes.push(RenderNode::new(
                "laser",
                180,
                RenderOp::ImageAsset {
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
            card: card.clone(),
            options: request.options.clone(),
            nodes,
        }
    }
}

// ── RenderDocument helpers ─────────────────────────────────────────────────────

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

pub(crate) fn laser_asset_name(laser: &str) -> Option<String> {
    let laser = laser.trim();
    if laser.is_empty() {
        None
    } else if laser.ends_with(".webp") {
        Some(laser.to_string())
    } else {
        Some(format!("{laser}.webp"))
    }
}

// ── RenderNode / RenderCanvas / RenderRect / ImageFit / ImageAlign ──────────

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

// ── RenderOp ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RenderOp {
    ImageAsset {
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
    FillRect {
        rect: RenderRect,
        color: String,
        opacity: f32,
    },
    TextLine {
        text: String,
        rect: RenderRect,
        font_family: String,
        font_size: u32,
        letter_spacing: f32,
        align: TextAlignChoice,
        fill: TextPaint,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<TextPaint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ruby: Option<RubyStyle>,
        #[serde(default)]
        width_compress: bool,
    },
    TextBlock {
        text: String,
        rect: RenderRect,
        font_family: String,
        font_size: u32,
        line_height: f32,
        letter_spacing: f32,
        fill: TextPaint,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<TextPaint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ruby: Option<RubyStyle>,
        #[serde(default)]
        first_line_compress: bool,
    },
    VisualEffect {
        target: EffectTarget,
        effect: EffectStyle,
    },
    CompositeVisualEffect {
        effect: EffectStyle,
        targets: Vec<EffectTargetWeight>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectTargetWeight {
    pub target: EffectTarget,
    pub opacity: f32,
}

// ── RubyStyle ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RubyStyle {
    pub rt_font_size: f32,
    pub rt_top: f32,
    pub rt_font_scale_x: f32,
}

// ── RenderRect ───────────────────────────────────────────────────────────────

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

    pub fn from_f32(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

// ── ImageFit / ImageAlign ────────────────────────────────────────────────────

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

// ── EffectTarget / EffectStyle ───────────────────────────────────────────────

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
    LinkArrows,
    EffectBoxBorder,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum EffectStyle {
    RainbowFoil { opacity: f32 },
    DotGrid { opacity: f32 },
    OpticalSer { opacity: f32 },
    OpticalSerSimple { opacity: f32 },
    OpticalScr { opacity: f32 },
    OpticalScrSimple { opacity: f32 },
    SecretWeave { opacity: f32 },
    SecretFoil { opacity: f32 },
    Holographic { opacity: f32 },
    BrightBorder { opacity: f32 },
    GoldWash { opacity: f32 },
    FrostedFoil { opacity: f32 },
    ConcentricEngrave { opacity: f32 },
    ReliefEngrave { opacity: f32 },
    DiamondFoil { opacity: f32 },
}

// ── Internal ─────────────────────────────────────────────────────────────────

fn default_visible() -> bool {
    true
}

impl PositionedAsset {
    fn w(&self, bundle: &AssetBundle) -> u32 {
        bundle
            .image(&self.asset)
            .ok()
            .and_then(|e| e.size.as_ref().map(|s| s.w))
            .or_else(|| {
                bundle
                    .image(&self.asset)
                    .ok()
                    .and_then(|e| e.atlas.as_ref().map(|a| a.w))
            })
            .unwrap_or(0)
    }

    fn h(&self, bundle: &AssetBundle) -> u32 {
        bundle
            .image(&self.asset)
            .ok()
            .and_then(|e| e.size.as_ref().map(|s| s.h))
            .or_else(|| {
                bundle
                    .image(&self.asset)
                    .ok()
                    .and_then(|e| e.atlas.as_ref().map(|a| a.h))
            })
            .unwrap_or(0)
    }
}
