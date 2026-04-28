mod color;
mod draw_card;
mod effect_areas;
mod visual_effects;

use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

use crate::{
    asset_bundle::{BaseLayout, get_bundle},
    card_logic::{
        build_effect_line, description_height, description_y, frame_asset_name,
        split_pendulum_description,
    },
    constants::{BACKGROUND_CREAM, CARD_HEIGHT, CARD_WIDTH, TEXT_COLOR_DARK},
    document::{EffectStyle, EffectTarget, RenderDocument, RenderOp},
    layout::layout_style,
    model::{RenderError, RenderRequest},
    rare_effect::{CoverageRect, draw_bright_border, draw_rare_effect},
    text::{
        DrawTextLine, RubyMultilineParams, TextAlign, draw_multiline_ruby_text, draw_text_line,
        fit_single_line,
    },
};

use color::text_brush;
use draw_card::{
    draw_anniversary_mark, draw_art, draw_attribute, draw_copyright_asset, draw_copyright_text,
    draw_document_link_arrows, draw_document_password, draw_document_text_block,
    draw_document_title, draw_external_image, draw_foreground_image, draw_frame, draw_laser,
    draw_level_or_rank, draw_link_arrows, draw_mask, draw_monster_type_line, draw_out_frame_blocks,
    draw_package, draw_password, draw_pendulum_description, draw_positioned_render_image,
    draw_spell_trap_line, draw_stats,
};
use effect_areas::{art_coverage_rect, draw_visual_effect_area, effect_target_areas};

pub struct Renderer;

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self
    }

    pub fn render_png(&self, request: &RenderRequest) -> Result<Vec<u8>, RenderError> {
        let document = self.build_document(request);
        self.render_document(&document)
    }

    pub fn build_document(&self, request: &RenderRequest) -> RenderDocument {
        let bundle = get_bundle();
        RenderDocument::from_request(request, bundle)
    }

    pub fn render_document(&self, document: &RenderDocument) -> Result<Vec<u8>, RenderError> {
        let request = document.to_request();
        if document.nodes.is_empty() {
            return self.render_request_png(&request, document.output_scale);
        }

        let bundle = get_bundle();
        let base = &bundle.layout.base;
        let language = document.language.as_deref();
        let style = layout_style(
            document.kind,
            language,
            &bundle.layout,
            &request.options.layout_overrides,
        );
        let document_link_arrow_count = document.nodes.iter().find_map(|node| {
            if !node.visible {
                return None;
            }
            match &node.op {
                RenderOp::LinkArrows { arrows } => Some(arrows.len().max(1) as u32),
                _ => None,
            }
        });

        let mut target = Pixmap::new(document.canvas.width, document.canvas.height)
            .ok_or_else(|| RenderError::Backend("Failed to allocate Pixmap".to_string()))?;
        target.fill(Color::from_rgba8(
            BACKGROUND_CREAM.0,
            BACKGROUND_CREAM.1,
            BACKGROUND_CREAM.2,
            255,
        ));

        let mut nodes: Vec<_> = document
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.visible)
            .collect();
        nodes.sort_by_key(|(index, node)| (node.z, *index));

        for (_, node) in nodes {
            match &node.op {
                RenderOp::BundleImage { asset, x, y } => {
                    if bundle.has_image(asset) {
                        bundle
                            .draw_image_at(&mut target, asset, *x, *y)
                            .map_err(RenderError::Backend)?;
                    }
                }
                RenderOp::ExternalImage {
                    path,
                    rect,
                    fit,
                    align,
                } => {
                    draw_external_image(&mut target, path.as_deref(), rect, *fit, *align);
                }
                RenderOp::PositionedImage { image } => {
                    draw_positioned_render_image(&mut target, image);
                }
                RenderOp::VisualEffect {
                    target: effect_target,
                    effect,
                } => {
                    draw_document_visual_effect(
                        bundle,
                        &mut target,
                        &request,
                        base,
                        language,
                        *effect_target,
                        *effect,
                    );
                }
                RenderOp::OutFrameBlocks => {
                    draw_out_frame_blocks(bundle, &mut target, &request, base)?;
                }
                RenderOp::AnniversaryMark => {
                    draw_anniversary_mark(bundle, &mut target, &request, base)?;
                }
                RenderOp::Attribute { asset, x, y } => {
                    if let Some(asset) = asset {
                        if bundle.has_image(asset) {
                            bundle
                                .draw_image_at(&mut target, asset, *x, *y)
                                .map_err(RenderError::Backend)?;
                        }
                    }
                }
                RenderOp::LevelOrRank => {
                    draw_level_or_rank(bundle, &mut target, &request, base)?;
                }
                RenderOp::LinkArrows { arrows } => {
                    draw_document_link_arrows(bundle, &mut target, arrows, base)?;
                }
                RenderOp::Title {
                    text,
                    rect,
                    font_family,
                    font_size,
                    letter_spacing,
                    color,
                    width_compress,
                    align,
                    fill,
                    shadow,
                } => {
                    draw_document_title(
                        &mut target,
                        &request,
                        language,
                        text,
                        rect,
                        font_family,
                        *font_size,
                        *letter_spacing,
                        color,
                        *width_compress,
                        *align,
                        fill.as_ref(),
                        shadow.as_ref(),
                    );
                }
                RenderOp::SpellTrapLine { .. } => {
                    draw_spell_trap_line(bundle, &mut target, &request, &style, base, language)?;
                }
                RenderOp::MonsterTypeLine { text, .. } => {
                    draw_monster_type_line(&mut target, &request, &style, base, language, text);
                }
                RenderOp::TextBlock {
                    text,
                    rect,
                    font_family,
                    font_size,
                    line_height,
                    letter_spacing,
                    channel,
                } => {
                    draw_document_text_block(
                        &mut target,
                        &request,
                        &style,
                        language,
                        text,
                        rect,
                        font_family,
                        *font_size,
                        *line_height,
                        *letter_spacing,
                        *channel,
                    );
                }
                RenderOp::Stats => {
                    let mut request = request.clone();
                    if request.card.is_link() {
                        if let Some(count) = document_link_arrow_count {
                            request.card.level = count;
                        }
                    }
                    draw_stats(bundle, &mut target, &request, &style, base, language);
                }
                RenderOp::Password { text, x, y } => {
                    draw_document_password(&mut target, &request, &style, language, text, *x, *y);
                }
                RenderOp::Package { text } => {
                    let mut request = request.clone();
                    request.card.package = Some(text.clone());
                    draw_package(&mut target, &request, &style, base, language);
                }
                RenderOp::Copyright { value, asset } => {
                    if let Some(asset) = asset {
                        draw_copyright_asset(bundle, &mut target, asset, base)?;
                    } else {
                        let mut request = request.clone();
                        request.card.copyright = Some(value.clone());
                        draw_copyright_text(&mut target, &request, &style, base, language);
                    }
                }
            }
        }

        let output = if (document.output_scale - 1.0).abs() > f32::EPSILON {
            scale_pixmap(&target, document.output_scale)?
        } else {
            target
        };

        output
            .encode_png()
            .map_err(|e| RenderError::PngEncode(e.to_string()))
    }

    fn render_request_png(
        &self,
        request: &RenderRequest,
        output_scale: f32,
    ) -> Result<Vec<u8>, RenderError> {
        let bundle = get_bundle();
        let base = &bundle.layout.base;
        let language = request.options.language.as_deref();
        let style = layout_style(
            request.kind,
            language,
            &bundle.layout,
            &request.options.layout_overrides,
        );

        let mut target = Pixmap::new(CARD_WIDTH, CARD_HEIGHT)
            .ok_or_else(|| RenderError::Backend("Failed to allocate Pixmap".to_string()))?;
        target.fill(Color::from_rgba8(
            BACKGROUND_CREAM.0,
            BACKGROUND_CREAM.1,
            BACKGROUND_CREAM.2,
            255,
        ));

        draw_frame(bundle, &mut target, frame_asset_name(&request.card))?;
        draw_art(bundle, &mut target, request, base)?;
        draw_mask(bundle, &mut target, request, base)?;
        if let Some(rare) = request.card.rare {
            draw_rare_effect(&mut target, rare, &request.card, base);
        }
        draw_foreground_image(&mut target, request)?;
        draw_out_frame_blocks(bundle, &mut target, request, base)?;
        draw_anniversary_mark(bundle, &mut target, request, base)?;
        draw_attribute(bundle, &mut target, request, base, language)?;
        draw_level_or_rank(bundle, &mut target, request, base)?;
        draw_link_arrows(bundle, &mut target, request, base)?;

        draw_card::draw_title(&mut target, request, &style, base, language);

        if request.card.is_spell() || request.card.is_trap() {
            draw_spell_trap_line(bundle, &mut target, request, &style, base, language)?;
        } else if let Some(line) = build_effect_line(&request.card, request.kind, language) {
            let line_layout = fit_single_line(
                &line,
                language,
                style.effect_size,
                &style.effect_font_family,
                base.effect.width,
                style.effect_letter_spacing,
                style.effect_size.saturating_sub(10),
            );
            draw_text_line(
                &mut target,
                DrawTextLine::unscaled(
                    &line_layout.text,
                    style.effect_x as f32,
                    style.effect_top as f32,
                    line_layout.font_size as f32,
                    line_layout.max_width as f32,
                    Color::from_rgba8(TEXT_COLOR_DARK.0, TEXT_COLOR_DARK.1, TEXT_COLOR_DARK.2, 255),
                    Color::TRANSPARENT,
                    &style.effect_font_family,
                    TextAlign::Left,
                    language,
                    line_layout.letter_spacing,
                )
                .with_brushes(
                    text_brush(
                        request.options.text_colors.effect.as_ref(),
                        None,
                        Color::from_rgba8(
                            TEXT_COLOR_DARK.0,
                            TEXT_COLOR_DARK.1,
                            TEXT_COLOR_DARK.2,
                            255,
                        ),
                        style.effect_x as f32,
                        line_layout.max_width as f32,
                    ),
                    text_brush(
                        request.options.text_colors.effect_shadow.as_ref(),
                        None,
                        Color::TRANSPARENT,
                        style.effect_x as f32,
                        line_layout.max_width as f32,
                    ),
                ),
            );
        }

        let description_text;
        if request.card.is_pendulum() {
            let sections = split_pendulum_description(&request.card.desc, language);
            if let Some(pendulum_effect) = sections.pendulum_effect.as_deref() {
                draw_pendulum_description(
                    &mut target,
                    request,
                    &style,
                    base,
                    language,
                    pendulum_effect,
                );
            }
            description_text = sections.monster_effect;
        } else {
            description_text = request.card.desc.clone();
        }

        draw_multiline_ruby_text(
            &mut target,
            RubyMultilineParams {
                text: &description_text,
                x: style.description_x as f32,
                y: description_y(&request.card, &style) as f32,
                width: style.body_max_width as f32,
                height: description_height(&request.card, &style, base) as f32,
                family: &style.base_font_family,
                color: Color::BLACK,
                shadow_color: Color::TRANSPARENT,
                brush: text_brush(
                    request.options.text_colors.description.as_ref(),
                    request.options.description_color_override.as_deref(),
                    Color::BLACK,
                    style.description_x as f32,
                    style.body_max_width as f32,
                ),
                shadow_brush: text_brush(
                    request.options.text_colors.description_shadow.as_ref(),
                    None,
                    Color::TRANSPARENT,
                    style.description_x as f32,
                    style.body_max_width as f32,
                ),
                language,
                base_font_size: style.description_size,
                rt_font_size: style.description_rt_font_size,
                rt_top: style.description_rt_top,
                rt_font_scale_x: style.description_rt_font_scale_x,
                line_height: style.description_line_height,
                letter_spacing: style.description_letter_spacing,
                min_font_size: style.description_size.saturating_sub(8),
                first_line_compress: request.options.description_first_line_compress,
            },
        );

        draw_stats(bundle, &mut target, request, &style, base, language);
        draw_password(&mut target, request, &style, base, language);
        draw_package(&mut target, request, &style, base, language);
        draw_copyright_text(&mut target, request, &style, base, language);
        draw_laser(bundle, &mut target, request, base)?;

        let output = if (output_scale - 1.0).abs() > f32::EPSILON {
            scale_pixmap(&target, output_scale)?
        } else {
            target
        };

        output
            .encode_png()
            .map_err(|e| RenderError::PngEncode(e.to_string()))
    }
}

fn scale_pixmap(source: &Pixmap, scale: f32) -> Result<Pixmap, RenderError> {
    let width = ((source.width() as f32 * scale).round() as u32).max(1);
    let height = ((source.height() as f32 * scale).round() as u32).max(1);
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

fn draw_document_visual_effect(
    bundle: &crate::asset_bundle::AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    base: &BaseLayout,
    language: Option<&str>,
    effect_target: EffectTarget,
    effect: EffectStyle,
) {
    let full_rect = CoverageRect {
        x: 0,
        y: 0,
        w: CARD_WIDTH,
        h: crate::constants::CARD_HEIGHT,
    };
    let art_rect = art_coverage_rect(request, base);

    if let EffectStyle::BrightBorder { opacity } = effect {
        // BrightBorder operates on both the outer card edge and the art frame
        // bevel simultaneously, so it needs both rects and cannot be expressed
        // as a single EffectArea.  Dispatch it here before the area loop.
        draw_bright_border(target, full_rect, art_rect, opacity);
        return;
    }

    for area in effect_target_areas(
        bundle,
        request,
        base,
        language,
        effect_target,
        full_rect,
        art_rect,
    ) {
        draw_visual_effect_area(target, area, effect);
    }
}

#[derive(Debug, Clone)]
struct ResolvedPaint {
    color: Color,
    brush: Option<crate::text::TextBrush>,
}

#[cfg(test)]
mod tests {
    use super::{
        CoverageRect, art_coverage_rect,
        draw_card::{laser_asset_name, premultiply_pixmap_alpha},
        effect_areas::{EffectArea, art_frame_coverage_rect, art_frame_effect_areas},
        scale_pixmap,
        visual_effects::{draw_frosted_foil, draw_relief_engrave},
    };
    use crate::{
        CardKind, RenderOptions, RenderRequest,
        asset_bundle::{get_bundle, init_global_bundle},
        model::YgoCardMeta,
    };
    use std::{fs, path::PathBuf, sync::Once};
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
        let art_rect = art_coverage_rect(&request, &bundle.layout.base);
        let frame_rect =
            art_frame_coverage_rect(bundle, &request, &bundle.layout.base).expect("frame rect");

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
        let art_rect = art_coverage_rect(&request, &bundle.layout.base);
        let areas = art_frame_effect_areas(bundle, &request, &bundle.layout.base, art_rect);

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
