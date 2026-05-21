mod color;
mod draw_card;
mod effect_areas;
mod visual_effects;

use std::sync::Arc;

use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

use crate::{
    asset_bundle::{AssetBundle, BaseLayout, try_get_bundle},
    constants::{BACKGROUND_CREAM, CARD_WIDTH},
    document::{
        EffectStyle, EffectTarget, EffectTargetWeight, RenderDocument, RenderOp, RenderRect,
        RubyStyle,
    },
    model::{FontWeight, RenderError, RenderRequest},
    rare_effect::{CoverageRect, draw_bright_border},
    text::{
        DrawTextLine, RubyLineParams, RubyMultilineParams, draw_multiline_ruby_text,
        draw_ruby_text_line, draw_text_line, fit_ruby_text_scale, fit_single_line,
        fit_single_line_compressed,
    },
};

use color::{parse_hex_color, text_brush_in_box};
use draw_card::{
    draw_external_image, draw_positioned_render_image, sanitize_render_rect, text_align_choice,
};
use effect_areas::{
    art_coverage_rect, build_composite_mask, draw_visual_effect_area, effect_target_areas,
    load_effect_protection_mask, restore_masked_effect_pixels, restore_protected_effect_pixels,
    snapshot_effect_rect,
};

#[derive(Clone)]
pub struct Renderer {
    bundle: Option<Arc<AssetBundle>>,
}

const MAX_RENDER_PIXELS: u64 = 4096 * 4096;
const TEXT_OUTLINE_OFFSETS: [(f32, f32); 8] = [
    (-1.0, 0.0),
    (1.0, 0.0),
    (0.0, -1.0),
    (0.0, 1.0),
    (-1.0, -1.0),
    (1.0, -1.0),
    (-1.0, 1.0),
    (1.0, 1.0),
];

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self { bundle: None }
    }

    pub fn with_bundle(bundle: Arc<AssetBundle>) -> Self {
        Self {
            bundle: Some(bundle),
        }
    }

    pub fn render_png(&self, request: &RenderRequest) -> Result<Vec<u8>, RenderError> {
        let document = self.build_document(request)?;
        self.render_document(&document)
    }

    pub fn build_document(&self, request: &RenderRequest) -> Result<RenderDocument, RenderError> {
        let bundle = self.bundle()?;
        Ok(RenderDocument::from_request(request, bundle))
    }

    pub fn render_document(&self, document: &RenderDocument) -> Result<Vec<u8>, RenderError> {
        if !matches!(document.schema_version, 3 | RenderDocument::SCHEMA_VERSION) {
            return Err(RenderError::Backend(format!(
                "unsupported schema version {} (expected 3 or {})",
                document.schema_version,
                RenderDocument::SCHEMA_VERSION,
            )));
        }

        validate_render_dimensions(document.canvas.width, document.canvas.height)?;
        let mut target = Pixmap::new(document.canvas.width, document.canvas.height)
            .ok_or_else(|| RenderError::Backend("Failed to allocate Pixmap".to_string()))?;
        target.fill(canvas_background_color(document));

        if !document.nodes.is_empty() {
            let bundle = self.bundle()?;
            let base = &bundle.layout.base;
            let language = document.language.as_deref();
            let effect_protection_mask = load_effect_protection_mask(document, base)?;

            for node in sorted_visible_nodes(document) {
                match &node.op {
                    RenderOp::ImageAsset { asset, x, y } => {
                        if bundle.has_image(asset) {
                            bundle
                                .draw_image_at(&mut target, asset, *x, *y)
                                .map_err(RenderError::Backend)?;
                        }
                    }
                    RenderOp::ImageAssetRect { asset, rect } => {
                        if bundle.has_image(asset) {
                            draw_image_asset_rect(bundle, &mut target, asset, rect)?;
                        }
                    }
                    RenderOp::ExternalImage {
                        path,
                        rect,
                        fit,
                        align,
                        crop,
                        scale,
                        offset_x,
                        offset_y,
                    } => draw_external_image(
                        &mut target,
                        path.as_deref(),
                        rect,
                        *fit,
                        *align,
                        *crop,
                        *scale,
                        *offset_x,
                        *offset_y,
                    ),
                    RenderOp::PositionedImage { image } => {
                        draw_positioned_render_image(&mut target, image)
                    }
                    RenderOp::FillRect {
                        rect,
                        color,
                        opacity,
                    } => draw_fill_rect(&mut target, rect, color, *opacity),
                    RenderOp::TextLine {
                        text,
                        rect,
                        font_family,
                        font_size,
                        letter_spacing,
                        align,
                        fill,
                        shadow,
                        ruby,
                        width_compress,
                        font_weight,
                    } => draw_text_line_op(
                        &mut target,
                        language,
                        text,
                        rect,
                        font_family,
                        *font_size,
                        *letter_spacing,
                        *align,
                        fill,
                        shadow.as_ref(),
                        ruby.as_ref(),
                        *width_compress,
                        *font_weight,
                    ),
                    RenderOp::TextBlock {
                        text,
                        rect,
                        font_family,
                        font_size,
                        line_height,
                        letter_spacing,
                        fill,
                        shadow,
                        ruby,
                        first_line_compress,
                        align,
                        font_weight,
                    } => draw_text_block_op(
                        &mut target,
                        language,
                        text,
                        rect,
                        font_family,
                        *font_size,
                        *line_height,
                        *letter_spacing,
                        fill,
                        shadow.as_ref(),
                        ruby.as_ref(),
                        *first_line_compress,
                        *align,
                        *font_weight,
                    ),
                    RenderOp::VisualEffect {
                        target: effect_target,
                        effect,
                    } => draw_document_visual_effect(
                        bundle,
                        &mut target,
                        document,
                        base,
                        language,
                        *effect_target,
                        *effect,
                        effect_protection_mask.as_ref(),
                    ),
                    RenderOp::CompositeVisualEffect { effect, targets } => {
                        draw_document_composite_visual_effect(
                            bundle,
                            &mut target,
                            document,
                            base,
                            language,
                            *effect,
                            targets,
                            effect_protection_mask.as_ref(),
                        )
                    }
                }
            }
        }

        let output_scale = sanitize_output_scale(document.output_scale);
        let output = if (output_scale - 1.0).abs() > f32::EPSILON {
            scale_pixmap(&target, output_scale)?
        } else {
            target
        };

        output
            .encode_png()
            .map_err(|e| RenderError::PngEncode(e.to_string()))
    }

    fn bundle(&self) -> Result<&AssetBundle, RenderError> {
        match &self.bundle {
            Some(bundle) => Ok(bundle.as_ref()),
            None => renderer_bundle(),
        }
    }
}

fn renderer_bundle() -> Result<&'static AssetBundle, RenderError> {
    try_get_bundle().map_err(RenderError::Backend)
}

fn scale_pixmap(source: &Pixmap, scale: f32) -> Result<Pixmap, RenderError> {
    let width = ((source.width() as f32 * scale).round() as u32).max(1);
    let height = ((source.height() as f32 * scale).round() as u32).max(1);
    validate_render_dimensions(width, height)?;
    let mut target = Pixmap::new(width, height).ok_or_else(|| {
        RenderError::Backend(format!("Failed to allocate scaled Pixmap {width}x{height}"))
    })?;

    target.draw_pixmap(
        0,
        0,
        source.as_ref(),
        &PixmapPaint::default(),
        Transform::from_scale(scale, scale),
        None,
    );

    Ok(target)
}

fn validate_render_dimensions(width: u32, height: u32) -> Result<(), RenderError> {
    let pixels = width as u64 * height as u64;
    if pixels == 0 || pixels > MAX_RENDER_PIXELS {
        return Err(RenderError::Backend(format!(
            "Render dimensions out of bounds: {width}x{height}"
        )));
    }
    Ok(())
}

fn sanitize_output_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn canvas_background_color(document: &RenderDocument) -> Color {
    document
        .canvas
        .background
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or_else(|| {
            Color::from_rgba8(
                BACKGROUND_CREAM.0,
                BACKGROUND_CREAM.1,
                BACKGROUND_CREAM.2,
                255,
            )
        })
}

fn draw_document_visual_effect(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    document: &RenderDocument,
    base: &BaseLayout,
    language: Option<&str>,
    effect_target: EffectTarget,
    effect: EffectStyle,
    protection_mask: Option<&effect_areas::EffectProtectionMask>,
) {
    let effect = sanitize_effect_style(effect);
    let full_rect = CoverageRect {
        x: 0,
        y: 0,
        w: CARD_WIDTH,
        h: crate::constants::CARD_HEIGHT,
    };
    let art_rect = art_coverage_rect(&document.card, base);

    if let EffectStyle::BrightBorder { opacity } = effect {
        // BrightBorder operates on both the outer card edge and the art frame
        // bevel simultaneously, so it needs both rects and cannot be expressed
        // as a single EffectArea.  Dispatch it here before the area loop.
        let before = protection_mask.map(|_| snapshot_effect_rect(target, full_rect));
        draw_bright_border(target, full_rect, art_rect, opacity);
        if let Some(before) = before.as_ref() {
            restore_protected_effect_pixels(target, before, protection_mask);
        }
        return;
    }

    for area in effect_target_areas(
        bundle,
        document,
        base,
        language,
        effect_target,
        full_rect,
        art_rect,
    ) {
        draw_visual_effect_area(target, area, effect, protection_mask);
    }
}

fn sorted_visible_nodes(document: &RenderDocument) -> Vec<&crate::document::RenderNode> {
    let mut nodes: Vec<_> = document
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.visible)
        .collect();
    nodes.sort_by_key(|(index, node)| (node.z, *index));
    nodes.into_iter().map(|(_, node)| node).collect()
}

fn draw_document_composite_visual_effect(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    document: &RenderDocument,
    base: &BaseLayout,
    language: Option<&str>,
    effect: EffectStyle,
    targets: &[EffectTargetWeight],
    protection_mask: Option<&effect_areas::EffectProtectionMask>,
) {
    let effect = sanitize_effect_style(effect);
    let base_opacity = composite_base_opacity(effect, targets);
    if base_opacity <= 0.0 || targets.is_empty() {
        return;
    }
    let effect = effect_with_opacity(effect, base_opacity);

    let full_rect = CoverageRect {
        x: 0,
        y: 0,
        w: CARD_WIDTH,
        h: crate::constants::CARD_HEIGHT,
    };
    let art_rect = art_coverage_rect(&document.card, base);
    let Some(mask) = build_composite_mask(
        bundle,
        document,
        base,
        language,
        full_rect,
        art_rect,
        base_opacity,
        targets,
    ) else {
        return;
    };

    let before = snapshot_effect_rect(target, full_rect);
    draw_document_visual_effect(
        bundle,
        target,
        document,
        base,
        language,
        EffectTarget::FullCard,
        effect,
        None,
    );
    restore_masked_effect_pixels(target, &before, &mask, protection_mask);
}

fn draw_fill_rect(target: &mut Pixmap, rect: &RenderRect, color: &str, opacity: f32) {
    let Some(rect) = sanitize_render_rect(rect) else {
        return;
    };
    let Some(tiny_color) = parse_hex_color(color) else {
        return;
    };
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }
    let Some(color) = Color::from_rgba(
        tiny_color.red(),
        tiny_color.green(),
        tiny_color.blue(),
        tiny_color.alpha() * opacity,
    ) else {
        return;
    };
    let Some(tiny_rect) = tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
    else {
        return;
    };
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(color);
    target.fill_rect(tiny_rect, &paint, tiny_skia::Transform::identity(), None);
}

fn draw_image_asset_rect(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    asset: &str,
    rect: &RenderRect,
) -> Result<(), RenderError> {
    let Some(rect) = sanitize_render_rect(rect) else {
        return Ok(());
    };
    let pixmap = bundle
        .decoded_image_for_render(asset)
        .map_err(RenderError::Backend)?;
    let source_w = pixmap.width() as f32;
    let source_h = pixmap.height() as f32;
    if source_w <= 0.0 || source_h <= 0.0 {
        return Ok(());
    }
    target.draw_pixmap(
        0,
        0,
        pixmap.as_ref().as_ref(),
        &PixmapPaint::default(),
        Transform::from_scale(rect.width / source_w, rect.height / source_h)
            .post_translate(rect.x, rect.y),
        None,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_text_line_op(
    target: &mut Pixmap,
    language: Option<&str>,
    text: &str,
    rect: &RenderRect,
    font_family: &str,
    font_size: u32,
    letter_spacing: f32,
    align: crate::model::TextAlignChoice,
    fill: &crate::model::TextPaint,
    shadow: Option<&crate::model::TextPaint>,
    ruby: Option<&RubyStyle>,
    width_compress: bool,
    font_weight: Option<FontWeight>,
) {
    let Some(rect) = sanitize_render_rect(rect) else {
        return;
    };
    let text_align = text_align_choice(align);
    let brush = text_brush_in_box(
        Some(fill),
        None,
        Color::BLACK,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    );
    let shadow_brush = shadow.and_then(|s| {
        text_brush_in_box(
            Some(s),
            None,
            Color::TRANSPARENT,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        )
    });
    if let Some(ruby) = ruby {
        if ruby.rt_font_size > 0.0 && crate::ruby::contains_ruby_markup(text) {
            let tokens = crate::ruby::parse_ruby_text(text);
            let scale_x = fit_ruby_text_scale(
                &tokens,
                font_family,
                font_size as f32,
                ruby.rt_font_size,
                letter_spacing,
                ruby.rt_font_scale_x,
                rect.width,
            )
            .max(0.3);
            let shadow_color = Color::TRANSPARENT;
            if shadow_brush.is_some() {
                for (dx, dy) in TEXT_OUTLINE_OFFSETS {
                    draw_ruby_text_line(
                        target,
                        RubyLineParams {
                            tokens: &tokens,
                            x: rect.x + dx,
                            y: rect.y + dy,
                            font_size: font_size as f32,
                            rt_font_size: ruby.rt_font_size,
                            rt_top: ruby.rt_top,
                            rt_font_scale_x_override: ruby.rt_font_scale_x,
                            color: shadow_color,
                            shadow_color: Color::TRANSPARENT,
                            brush: shadow_brush.clone(),
                            shadow_brush: None,
                            family: font_family,
                            language,
                            letter_spacing,
                            scale_x,
                            justify_gap: 0.0,
                            font_weight,
                        },
                    );
                }
            }
            draw_ruby_text_line(
                target,
                RubyLineParams {
                    tokens: &tokens,
                    x: rect.x,
                    y: rect.y,
                    font_size: font_size as f32,
                    rt_font_size: ruby.rt_font_size,
                    rt_top: ruby.rt_top,
                    rt_font_scale_x_override: ruby.rt_font_scale_x,
                    color: Color::BLACK,
                    shadow_color: Color::TRANSPARENT,
                    brush,
                    shadow_brush: None,
                    family: font_family,
                    language,
                    letter_spacing,
                    scale_x,
                    justify_gap: 0.0,
                    font_weight,
                },
            );
            return;
        }
    }
    let title_layout = if width_compress {
        fit_single_line_compressed(
            text,
            language,
            font_size,
            font_family,
            rect.width.round() as u32,
            letter_spacing,
            0.2,
        )
    } else {
        fit_single_line(
            text,
            language,
            font_size,
            font_family,
            rect.width.round() as u32,
            letter_spacing,
            font_size.saturating_sub(26),
        )
    };
    if shadow_brush.is_some() {
        for (dx, dy) in TEXT_OUTLINE_OFFSETS {
            draw_text_line(
                target,
                DrawTextLine {
                    text: &title_layout.text,
                    x: rect.x + dx,
                    y: rect.y + dy,
                    font_size: title_layout.font_size as f32,
                    max_width: title_layout.max_width as f32,
                    color: Color::TRANSPARENT,
                    shadow_color: Color::TRANSPARENT,
                    brush: shadow_brush.clone(),
                    shadow_brush: None,
                    family_name: font_family,
                    align: text_align,
                    language,
                    letter_spacing: title_layout.letter_spacing,
                    scale_x: title_layout.scale_x,
                    font_weight,
                },
            );
        }
    }
    draw_text_line(
        target,
        DrawTextLine {
            text: &title_layout.text,
            x: rect.x,
            y: rect.y,
            font_size: title_layout.font_size as f32,
            max_width: title_layout.max_width as f32,
            color: Color::BLACK,
            shadow_color: Color::TRANSPARENT,
            brush,
            shadow_brush: None,
            family_name: font_family,
            align: text_align,
            language,
            letter_spacing: title_layout.letter_spacing,
            scale_x: title_layout.scale_x,
            font_weight,
        },
    );
}

fn draw_text_block_op(
    target: &mut Pixmap,
    language: Option<&str>,
    text: &str,
    rect: &RenderRect,
    font_family: &str,
    font_size: u32,
    line_height: f32,
    letter_spacing: f32,
    fill: &crate::model::TextPaint,
    shadow: Option<&crate::model::TextPaint>,
    ruby: Option<&RubyStyle>,
    first_line_compress: bool,
    align: crate::model::TextAlignChoice,
    font_weight: Option<FontWeight>,
) {
    let Some(rect) = sanitize_render_rect(rect) else {
        return;
    };
    let brush = text_brush_in_box(
        Some(fill),
        None,
        Color::BLACK,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    );
    let shadow_brush = shadow.and_then(|s| {
        text_brush_in_box(
            Some(s),
            None,
            Color::TRANSPARENT,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        )
    });
    let rt_font_size = ruby.map(|r| r.rt_font_size as u32).unwrap_or(0);
    let rt_top = ruby.map(|r| r.rt_top).unwrap_or(0.0);
    let rt_font_scale_x = ruby.map(|r| r.rt_font_scale_x).unwrap_or(1.0);
    draw_multiline_ruby_text(
        target,
        RubyMultilineParams {
            text,
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            family: font_family,
            color: Color::BLACK,
            shadow_color: Color::TRANSPARENT,
            brush,
            shadow_brush,
            language,
            base_font_size: font_size,
            rt_font_size,
            rt_top,
            rt_font_scale_x,
            line_height,
            letter_spacing,
            min_font_size: font_size.saturating_sub(10),
            first_line_compress,
            align: text_align_choice(align),
            font_weight,
        },
    );
}

fn sanitize_effect_style(effect: EffectStyle) -> EffectStyle {
    match effect {
        EffectStyle::RainbowFoil { opacity } => EffectStyle::RainbowFoil {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::DotGrid { opacity } => EffectStyle::DotGrid {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::OpticalSer { opacity } => EffectStyle::OpticalSer {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::OpticalSerSimple { opacity } => EffectStyle::OpticalSerSimple {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::OpticalScr { opacity } => EffectStyle::OpticalScr {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::OpticalScrSimple { opacity } => EffectStyle::OpticalScrSimple {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::SecretWeave { opacity } => EffectStyle::SecretWeave {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::SecretFoil { opacity } => EffectStyle::SecretFoil {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::Holographic { opacity } => EffectStyle::Holographic {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::BrightBorder { opacity } => EffectStyle::BrightBorder {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::GoldWash { opacity } => EffectStyle::GoldWash {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::FrostedFoil { opacity } => EffectStyle::FrostedFoil {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::ConcentricEngrave { opacity } => EffectStyle::ConcentricEngrave {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::ReliefEngrave { opacity } => EffectStyle::ReliefEngrave {
            opacity: sanitize_opacity(opacity),
        },
        EffectStyle::DiamondFoil { opacity } => EffectStyle::DiamondFoil {
            opacity: sanitize_opacity(opacity),
        },
    }
}

fn effect_opacity(effect: EffectStyle) -> f32 {
    match effect {
        EffectStyle::RainbowFoil { opacity }
        | EffectStyle::DotGrid { opacity }
        | EffectStyle::OpticalSer { opacity }
        | EffectStyle::OpticalSerSimple { opacity }
        | EffectStyle::OpticalScr { opacity }
        | EffectStyle::OpticalScrSimple { opacity }
        | EffectStyle::SecretWeave { opacity }
        | EffectStyle::SecretFoil { opacity }
        | EffectStyle::Holographic { opacity }
        | EffectStyle::BrightBorder { opacity }
        | EffectStyle::GoldWash { opacity }
        | EffectStyle::FrostedFoil { opacity }
        | EffectStyle::ConcentricEngrave { opacity }
        | EffectStyle::ReliefEngrave { opacity }
        | EffectStyle::DiamondFoil { opacity } => opacity,
    }
}

fn composite_base_opacity(effect: EffectStyle, targets: &[EffectTargetWeight]) -> f32 {
    targets
        .iter()
        .map(|target| sanitize_opacity(target.opacity))
        .fold(effect_opacity(effect), f32::max)
}

fn effect_with_opacity(effect: EffectStyle, opacity: f32) -> EffectStyle {
    let opacity = sanitize_opacity(opacity);
    match effect {
        EffectStyle::RainbowFoil { .. } => EffectStyle::RainbowFoil { opacity },
        EffectStyle::DotGrid { .. } => EffectStyle::DotGrid { opacity },
        EffectStyle::OpticalSer { .. } => EffectStyle::OpticalSer { opacity },
        EffectStyle::OpticalSerSimple { .. } => EffectStyle::OpticalSerSimple { opacity },
        EffectStyle::OpticalScr { .. } => EffectStyle::OpticalScr { opacity },
        EffectStyle::OpticalScrSimple { .. } => EffectStyle::OpticalScrSimple { opacity },
        EffectStyle::SecretWeave { .. } => EffectStyle::SecretWeave { opacity },
        EffectStyle::SecretFoil { .. } => EffectStyle::SecretFoil { opacity },
        EffectStyle::Holographic { .. } => EffectStyle::Holographic { opacity },
        EffectStyle::BrightBorder { .. } => EffectStyle::BrightBorder { opacity },
        EffectStyle::GoldWash { .. } => EffectStyle::GoldWash { opacity },
        EffectStyle::FrostedFoil { .. } => EffectStyle::FrostedFoil { opacity },
        EffectStyle::ConcentricEngrave { .. } => EffectStyle::ConcentricEngrave { opacity },
        EffectStyle::ReliefEngrave { .. } => EffectStyle::ReliefEngrave { opacity },
        EffectStyle::DiamondFoil { .. } => EffectStyle::DiamondFoil { opacity },
    }
}

fn sanitize_opacity(opacity: f32) -> f32 {
    if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CoverageRect, art_coverage_rect,
        draw_card::premultiply_pixmap_alpha,
        effect_areas::{EffectArea, art_frame_coverage_rect, art_frame_effect_areas},
        scale_pixmap,
        visual_effects::{draw_frosted_foil, draw_relief_engrave},
    };
    use crate::{
        CardKind, RenderOptions, RenderRequest,
        asset_bundle::{AssetBundle, get_bundle, init_global_bundle},
        document::{RenderDocument, RenderNode, RenderOp, RenderRect, laser_asset_name},
        model::YgoCardMeta,
    };
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, Once},
    };
    use tiny_skia::PremultipliedColorU8;
    use ygopro_cdb_encode_rs::CardDataEntry;

    fn init_bundle() {
        static INIT: Once = Once::new();
        let bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("yugioh_bundle.bin");

        INIT.call_once(|| {
            let bytes = fs::read(&bin_path).expect("read yugioh bundle");
            init_global_bundle(&bytes).expect("initialize yugioh bundle");
        });
    }

    fn load_test_bundle() -> Arc<AssetBundle> {
        let bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("yugioh_bundle.bin");
        let bytes = fs::read(&bin_path).expect("read yugioh bundle");
        Arc::new(AssetBundle::load_from_bytes(&bytes).expect("load explicit bundle"))
    }

    #[test]
    fn sorted_visible_nodes_filters_invisible_and_keeps_order_for_equal_z() {
        let fill = |color: &str| crate::document::RenderOp::FillRect {
            rect: crate::document::RenderRect::new(0, 0, 1, 1),
            color: color.to_string(),
            opacity: 1.0,
        };
        let document = crate::document::RenderDocument {
            schema_version: crate::document::RenderDocument::SCHEMA_VERSION,
            kind: CardKind::Yugioh,
            canvas: crate::document::RenderCanvas {
                width: 1,
                height: 1,
                background: None,
            },
            language: None,
            output_scale: 1.0,
            card: YgoCardMeta::from(CardDataEntry::default()),
            options: RenderOptions::default(),
            nodes: vec![
                crate::document::RenderNode::new("visible-a", 10, fill("#000000")),
                crate::document::RenderNode {
                    id: "hidden".to_string(),
                    z: 5,
                    visible: false,
                    op: fill("#111111"),
                },
                crate::document::RenderNode::new("visible-b", 10, fill("#222222")),
                crate::document::RenderNode::new("visible-before", 0, fill("#333333")),
            ],
        };

        let nodes = super::sorted_visible_nodes(&document);
        assert_eq!(
            nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>(),
            vec!["visible-before", "visible-a", "visible-b"]
        );
    }

    #[test]
    fn render_document_accepts_schema_version_3_for_compatibility() {
        let document = crate::document::RenderDocument {
            schema_version: 3,
            kind: CardKind::Yugioh,
            canvas: crate::document::RenderCanvas {
                width: 1,
                height: 1,
                background: None,
            },
            language: None,
            output_scale: 1.0,
            card: YgoCardMeta::from(CardDataEntry::default()),
            options: RenderOptions::default(),
            nodes: Vec::new(),
        };

        let png = super::Renderer::new()
            .render_document(&document)
            .expect("schema v3 document should remain renderable");
        assert!(!png.is_empty());
    }

    #[test]
    fn explicit_bundle_renderer_builds_document() {
        let renderer = super::Renderer::with_bundle(load_test_bundle());
        let request = RenderRequest {
            kind: CardKind::Yugioh,
            card: YgoCardMeta::from(CardDataEntry {
                code: 46986414,
                name: "Dark Magician".to_string(),
                desc: "The ultimate wizard in terms of attack and defense.".to_string(),
                type_: 0x41,
                attack: 2500,
                defense: 2100,
                level: 7,
                race: 0x1,
                attribute: 0x10,
                ..CardDataEntry::default()
            }),
            options: RenderOptions::default(),
        };

        let document = renderer
            .build_document(&request)
            .expect("build document with explicit bundle");
        assert!(document.nodes.iter().any(|node| node.id == "frame"));
        assert!(document.nodes.iter().any(|node| node.id == "title"));
    }

    #[test]
    fn explicit_bundle_renderer_renders_non_empty_document() {
        let renderer = super::Renderer::with_bundle(load_test_bundle());
        let document = RenderDocument {
            schema_version: RenderDocument::SCHEMA_VERSION,
            kind: CardKind::Yugioh,
            canvas: crate::document::RenderCanvas {
                width: 4,
                height: 4,
                background: None,
            },
            language: None,
            output_scale: 1.0,
            card: YgoCardMeta::from(CardDataEntry::default()),
            options: RenderOptions::default(),
            nodes: vec![RenderNode::new(
                "test-fill",
                0,
                RenderOp::FillRect {
                    rect: RenderRect::new(0, 0, 4, 4),
                    color: "#ff0000".to_string(),
                    opacity: 1.0,
                },
            )],
        };

        let png = renderer
            .render_document(&document)
            .expect("render document with explicit bundle");
        assert!(!png.is_empty());
    }

    #[test]
    fn explicit_bundle_renderer_draws_bundle_image_assets() {
        let bundle = load_test_bundle();
        assert!(bundle.has_image("card-normal.webp"));
        let renderer = super::Renderer::with_bundle(bundle);
        let document = RenderDocument {
            schema_version: RenderDocument::SCHEMA_VERSION,
            kind: CardKind::Yugioh,
            canvas: crate::document::RenderCanvas {
                width: 16,
                height: 16,
                background: None,
            },
            language: None,
            output_scale: 1.0,
            card: YgoCardMeta::from(CardDataEntry::default()),
            options: RenderOptions::default(),
            nodes: vec![RenderNode::new(
                "test-image",
                0,
                RenderOp::ImageAsset {
                    asset: "card-normal.webp".to_string(),
                    x: 0.0,
                    y: 0.0,
                },
            )],
        };

        let png = renderer
            .render_document(&document)
            .expect("render image asset with explicit bundle");
        assert!(!png.is_empty());
    }

    #[test]
    fn builds_laser_asset_names() {
        assert_eq!(laser_asset_name("laser1").as_deref(), Some("laser1.webp"));
        assert_eq!(
            laser_asset_name("laser2.webp").as_deref(),
            Some("laser2.webp")
        );
        assert_eq!(laser_asset_name("  ").as_deref(), None);
    }

    #[test]
    fn scales_pixmap_dimensions() {
        let source = tiny_skia::Pixmap::new(10, 20).expect("source pixmap");
        let scaled = scale_pixmap(&source, 0.5).expect("scale pixmap");

        assert_eq!(scaled.width(), 5);
        assert_eq!(scaled.height(), 10);
    }

    #[test]
    fn premultiplies_external_image_alpha() {
        let mut pixmap = tiny_skia::Pixmap::from_vec(
            vec![255, 255, 255, 0, 200, 100, 50, 128],
            tiny_skia::IntSize::from_wh(2, 1).unwrap(),
        )
        .expect("pixmap");

        premultiply_pixmap_alpha(&mut pixmap);

        let transparent = pixmap.pixel(0, 0).expect("transparent pixel");
        assert_eq!(transparent.alpha(), 0);
        assert_eq!(transparent.red(), 0);
        assert_eq!(transparent.green(), 0);
        assert_eq!(transparent.blue(), 0);

        let partial = pixmap.pixel(1, 0).expect("partial pixel");
        assert_eq!(partial.alpha(), 128);
        assert_eq!(partial.red(), 100);
        assert_eq!(partial.green(), 50);
        assert_eq!(partial.blue(), 25);
    }

    #[test]
    fn sanitizes_effect_style_opacity() {
        let nan = f32::NAN;

        assert!(matches!(
            super::sanitize_effect_style(crate::document::EffectStyle::RainbowFoil {
                opacity: nan,
            }),
            crate::document::EffectStyle::RainbowFoil { opacity } if opacity == 0.0
        ));

        assert!(matches!(
            super::sanitize_effect_style(crate::document::EffectStyle::BrightBorder {
                opacity: 1.7,
            }),
            crate::document::EffectStyle::BrightBorder { opacity } if opacity == 1.0
        ));

        assert!(matches!(
            super::sanitize_effect_style(crate::document::EffectStyle::ReliefEngrave {
                opacity: -0.25,
            }),
            crate::document::EffectStyle::ReliefEngrave { opacity } if opacity == 0.0
        ));
    }

    #[test]
    fn validates_render_dimensions_bounds() {
        assert!(matches!(
            super::validate_render_dimensions(0, 1),
            Err(crate::model::RenderError::Backend(_))
        ));
        assert!(matches!(
            super::validate_render_dimensions(4097, 4097),
            Err(crate::model::RenderError::Backend(_))
        ));
        assert!(super::validate_render_dimensions(1, 1).is_ok());
    }

    #[test]
    fn relief_engrave_prefers_flat_height_map_regions() {
        let mut pixmap = tiny_skia::Pixmap::new(64, 32).expect("pixmap");
        {
            let pixels = pixmap.pixels_mut();
            for y in 0..32 {
                for x in 0..64 {
                    let value = if x < 32 { 45 } else { 180 };
                    pixels[(y * 64 + x) as usize] =
                        PremultipliedColorU8::from_rgba(value, value, value, 255).unwrap();
                }
            }
        }
        let before = pixmap.pixels().to_vec();
        draw_relief_engrave(
            &mut pixmap,
            CoverageRect {
                x: 0,
                y: 0,
                w: 64,
                h: 32,
            },
            0.7,
        );

        let avg_delta = |x0: u32, x1: u32| -> f32 {
            let mut total = 0.0_f32;
            let mut count = 0_u32;
            for y in 4..28 {
                for x in x0..x1 {
                    let idx = (y * 64 + x) as usize;
                    total += (pixmap.pixels()[idx].red() as i16 - before[idx].red() as i16)
                        .unsigned_abs() as f32;
                    count += 1;
                }
            }
            total / count as f32
        };

        let flat_delta = avg_delta(6, 24);
        let edge_delta = avg_delta(30, 34);
        assert!(flat_delta > edge_delta);
    }

    #[test]
    fn frosted_foil_is_continuous_across_split_rects() {
        let mut whole = tiny_skia::Pixmap::new(64, 64).expect("whole pixmap");
        whole.fill(tiny_skia::Color::from_rgba8(40, 55, 70, 255));
        draw_frosted_foil(
            &mut whole,
            CoverageRect {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
            },
            0.5,
        );

        let mut split = tiny_skia::Pixmap::new(64, 64).expect("split pixmap");
        split.fill(tiny_skia::Color::from_rgba8(40, 55, 70, 255));
        draw_frosted_foil(
            &mut split,
            CoverageRect {
                x: 0,
                y: 0,
                w: 64,
                h: 21,
            },
            0.5,
        );
        draw_frosted_foil(
            &mut split,
            CoverageRect {
                x: 0,
                y: 21,
                w: 64,
                h: 43,
            },
            0.5,
        );

        assert_eq!(split.pixels(), whole.pixels());
    }

    #[test]
    fn art_frame_coverage_rect_expands_beyond_art_rect() {
        init_bundle();

        let request = RenderRequest {
            kind: CardKind::Yugioh,
            card: YgoCardMeta::from(CardDataEntry {
                code: 46986414,
                name: "ブラック・マジシャン".to_string(),
                desc: "test".to_string(),
                type_: 0x41,
                attack: 2500,
                defense: 2100,
                level: 7,
                race: 0x1,
                attribute: 0x10,
                ..CardDataEntry::default()
            }),
            options: RenderOptions::default(),
        };

        let bundle = get_bundle();
        let art_rect = art_coverage_rect(&request.card, &bundle.layout.base);
        let frame_rect = art_frame_coverage_rect(bundle, &request.card, &bundle.layout.base)
            .expect("frame rect");

        assert!(frame_rect.x <= art_rect.x);
        assert!(frame_rect.y <= art_rect.y);
        assert!(frame_rect.x + frame_rect.w >= art_rect.x + art_rect.w);
        assert!(frame_rect.y + frame_rect.h >= art_rect.y + art_rect.h);
        assert!(
            frame_rect.x < art_rect.x
                || frame_rect.y < art_rect.y
                || frame_rect.x + frame_rect.w > art_rect.x + art_rect.w
                || frame_rect.y + frame_rect.h > art_rect.y + art_rect.h
        );
    }

    #[test]
    fn art_frame_effect_uses_mask_alpha() {
        init_bundle();

        let request = RenderRequest {
            kind: CardKind::Yugioh,
            card: YgoCardMeta::from(CardDataEntry {
                code: 46986414,
                name: "ブラック・マジシャン".to_string(),
                desc: "test".to_string(),
                type_: 0x41,
                attack: 2500,
                defense: 2100,
                level: 7,
                race: 0x1,
                attribute: 0x10,
                ..CardDataEntry::default()
            }),
            options: RenderOptions::default(),
        };

        let bundle = get_bundle();
        let art_rect = art_coverage_rect(&request.card, &bundle.layout.base);
        let areas = art_frame_effect_areas(bundle, &request.card, &bundle.layout.base, art_rect);

        assert_eq!(areas.len(), 1);
        let EffectArea::MaskedRect { rect, mask } = &areas[0] else {
            panic!("art frame effect should follow the frame mask alpha");
        };

        assert!(rect.x < art_rect.x);
        assert!(rect.y < art_rect.y);

        let art_center_x = art_rect.x + art_rect.w / 2 - rect.x;
        let art_center_y = art_rect.y + art_rect.h / 2 - rect.y;
        let frame_edge_x = art_rect.x - rect.x - 1;
        let frame_edge_y = art_rect.y + art_rect.h / 2 - rect.y;
        let center_alpha =
            mask.pixels()[(art_center_y * mask.width() + art_center_x) as usize].alpha();
        let edge_alpha =
            mask.pixels()[(frame_edge_y * mask.width() + frame_edge_x) as usize].alpha();

        assert_eq!(center_alpha, 0);
        assert!(edge_alpha > 0);
    }
}
