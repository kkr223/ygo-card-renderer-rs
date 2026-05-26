mod color;
mod draw_card;
mod effect_areas;
mod effect_ops;
#[cfg(test)]
mod tests;
mod text_ops;
mod visual_effects;

use std::sync::Arc;

use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

use crate::{
    asset_bundle::{AssetBundle, BaseLayout, try_get_bundle},
    constants::{BACKGROUND_CREAM, CARD_WIDTH},
    document::{
        EffectStyle, EffectTarget, EffectTargetWeight, RenderDocument, RenderOp, RenderRect,
    },
    model::{RenderError, RenderRequest},
    rare_effect::{CoverageRect},
};

use color::parse_hex_color;
use draw_card::{draw_external_image, draw_positioned_render_image, sanitize_render_rect};
use effect_areas::{
    art_coverage_rect, build_composite_mask, draw_visual_effect_area, effect_target_areas,
    load_effect_protection_mask, restore_masked_effect_pixels,
    snapshot_effect_rect,
};
use effect_ops::{composite_base_opacity, effect_with_opacity, sanitize_effect_style};
use text_ops::{draw_text_block_op, draw_text_line_op};

#[derive(Clone)]
pub struct Renderer {
    bundle: Option<Arc<AssetBundle>>,
}

const MAX_RENDER_PIXELS: u64 = 4096 * 4096;

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
