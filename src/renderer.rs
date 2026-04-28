use tiny_skia::{Color, Paint, Pixmap, PixmapPaint, Rect, Transform};
use ygopro_cdb_encode_rs::CardDataEntry;

use crate::{
    asset_bundle::{AssetBundle, BaseLayout, get_bundle},
    card_logic::{
        attribute_asset_name, auto_name_light, build_effect_line, build_scale_line,
        description_height, description_y, display_stat, frame_asset_name, image_frame,
        localized_brackets, localized_spell_trap_name, spell_trap_subtype_icon_asset,
        split_pendulum_description, uses_rank,
    },
    constants::{
        BACKGROUND_CREAM, CARD_HEIGHT, CARD_WIDTH, NAME_COLOR_DARK, NAME_COLOR_LIGHT,
        PASSWORD_COLOR, TEXT_COLOR_DARK, TYPE_COLOR,
    },
    document::{
        EffectStyle, EffectTarget, ImageAlign, ImageFit, RenderDocument, RenderOp, RenderRect,
        TextChannel,
    },
    layout::{LayoutStyle, layout_style},
    model::{
        NameColor, OutFrameEffectBox, PositionedRenderImage, RenderError, RenderRequest,
        TextAlignChoice, TextGradient, TextPaint,
    },
    rare_effect::{
        CoverageRect, draw_bright_border, draw_dot_grid, draw_holographic, draw_rainbow_foil,
        draw_rare_effect, draw_secret_weave,
    },
    pixel_ops::{hsv_to_rgb, pixel_hash, screen_pixel},
    ruby::{contains_ruby_markup, parse_ruby_text, strip_ruby_markup},
    text::{
        DrawTextLine, RubyLineParams, RubyMultilineParams, TextAlign, TextBrush,
        draw_multiline_ruby_text, draw_ruby_text_line, draw_text_line, draw_text_line_scaled,
        estimate_text_width, fit_ruby_text_scale, fit_single_line, fit_single_line_compressed,
    },
};

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
                RenderOp::VisualEffect { target: effect_target, effect } => {
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

        draw_title(&mut target, request, &style, base, language);

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

fn draw_external_image(
    target: &mut Pixmap,
    path: Option<&std::path::Path>,
    rect: &RenderRect,
    fit: ImageFit,
    align: ImageAlign,
) {
    let Some(path) = path else {
        return;
    };
    let Some(pixmap) = load_external_pixmap(path) else {
        return;
    };

    let source_w = pixmap.width() as f32;
    let source_h = pixmap.height() as f32;
    if source_w <= 0.0 || source_h <= 0.0 || rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }

    let target_w = rect.width.round().max(1.0) as u32;
    let target_h = rect.height.round().max(1.0) as u32;
    let Some(mut clipped) = Pixmap::new(target_w, target_h) else {
        return;
    };

    let (scale_x, scale_y) = match fit {
        ImageFit::Stretch => (rect.width / source_w, rect.height / source_h),
        ImageFit::Cover => {
            let scale = (rect.width / source_w).max(rect.height / source_h);
            (scale, scale)
        }
        ImageFit::Contain => {
            let scale = (rect.width / source_w).min(rect.height / source_h);
            (scale, scale)
        }
    };

    let drawn_w = source_w * scale_x;
    let drawn_h = source_h * scale_y;
    let dx = (rect.width - drawn_w) / 2.0;
    let dy = match align {
        ImageAlign::Top => 0.0,
        ImageAlign::Center => (rect.height - drawn_h) / 2.0,
    };

    clipped.draw_pixmap(
        0,
        0,
        pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::from_scale(scale_x, scale_y).post_translate(dx, dy),
        None,
    );

    target.draw_pixmap(
        rect.x.round() as i32,
        rect.y.round() as i32,
        clipped.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_document_title(
    target: &mut Pixmap,
    request: &RenderRequest,
    language: Option<&str>,
    text: &str,
    rect: &RenderRect,
    font_family: &str,
    font_size: u32,
    letter_spacing: f32,
    color: &NameColor,
    width_compress: bool,
    align: TextAlignChoice,
    fill: Option<&TextPaint>,
    shadow: Option<&TextPaint>,
) {
    let name_color = resolve_name_color(color, &request.card);
    let name_brush = resolve_title_brush(request, fill, name_color, rect.x, rect.width);
    let name_shadow = resolve_title_shadow_brush(request, shadow, rect.x, rect.width);

    if contains_ruby_markup(text) {
        let tokens = parse_ruby_text(text);
        let rt_font_size = if language == Some("jp") { 30.0 } else { 0.0 };
        if rt_font_size > 0.0 {
            let scale_x = fit_ruby_text_scale(
                &tokens,
                font_family,
                font_size as f32,
                rt_font_size,
                letter_spacing,
                1.0,
                rect.width,
            )
            .max(0.3);
            draw_ruby_text_line(
                target,
                RubyLineParams {
                    tokens: &tokens,
                    x: rect.x,
                    y: rect.y,
                    font_size: font_size as f32,
                    rt_font_size,
                    rt_top: -18.0,
                    rt_font_scale_x_override: 1.0,
                    color: name_brush.color,
                    shadow_color: Color::TRANSPARENT,
                    brush: name_brush.brush,
                    shadow_brush: None,
                    family: font_family,
                    language,
                    letter_spacing,
                    scale_x,
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
            0.3,
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

    let align = text_align_choice(align);
    if name_shadow.color.alpha() > 0.0 || name_shadow.brush.is_some() {
        draw_text_line_scaled(
            target,
            DrawTextLine {
                text: &title_layout.text,
                x: rect.x + 7.0,
                y: rect.y + 7.0,
                font_size: title_layout.font_size as f32,
                max_width: title_layout.max_width as f32,
                color: name_shadow.color,
                shadow_color: Color::TRANSPARENT,
                brush: name_shadow.brush,
                shadow_brush: None,
                family_name: font_family,
                align,
                language,
                letter_spacing: title_layout.letter_spacing,
                scale_x: title_layout.scale_x,
            },
        );
    }

    draw_text_line_scaled(
        target,
        DrawTextLine {
            text: &title_layout.text,
            x: rect.x,
            y: rect.y,
            font_size: title_layout.font_size as f32,
            max_width: title_layout.max_width as f32,
            color: name_brush.color,
            shadow_color: Color::TRANSPARENT,
            brush: name_brush.brush,
            shadow_brush: None,
            family_name: font_family,
            align,
            language,
            letter_spacing: title_layout.letter_spacing,
            scale_x: title_layout.scale_x,
        },
    );
}

fn draw_monster_type_line(
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
    line: &str,
) {
    let line_layout = fit_single_line(
        line,
        language,
        style.effect_size,
        &style.effect_font_family,
        base.effect.width,
        style.effect_letter_spacing,
        style.effect_size.saturating_sub(10),
    );
    draw_text_line(
        target,
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
                Color::from_rgba8(TEXT_COLOR_DARK.0, TEXT_COLOR_DARK.1, TEXT_COLOR_DARK.2, 255),
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

fn draw_document_link_arrows(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    arrows: &[u8],
    base: &BaseLayout,
) -> Result<(), RenderError> {
    const ARROW_KEYS: &[&str] = &[
        "up",
        "right_up",
        "right",
        "right_down",
        "down",
        "left_down",
        "left",
        "left_up",
    ];

    for (index, key) in ARROW_KEYS.iter().enumerate() {
        let pair = match base.link_arrows.get(*key) {
            Some(pair) => pair,
            None => continue,
        };
        let show = arrows.contains(&((index + 1) as u8));
        let state = if show { &pair.on } else { &pair.off };
        bundle
            .draw_image_at(target, &state.asset, state.x as f32, state.y as f32)
            .map_err(RenderError::Backend)?;
    }

    Ok(())
}

fn draw_document_password(
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    language: Option<&str>,
    text: &str,
    x: f32,
    y: f32,
) {
    draw_text_line(
        target,
        DrawTextLine::unscaled(
            text,
            x,
            y,
            40.0,
            260.0,
            Color::from_rgba8(PASSWORD_COLOR.0, PASSWORD_COLOR.1, PASSWORD_COLOR.2, 255),
            Color::TRANSPARENT,
            &style.password_font_family,
            TextAlign::Left,
            language,
            0.0,
        )
        .with_brushes(
            text_brush(
                request.options.text_colors.password.as_ref(),
                None,
                Color::from_rgba8(PASSWORD_COLOR.0, PASSWORD_COLOR.1, PASSWORD_COLOR.2, 255),
                x,
                260.0,
            ),
            text_brush(
                request.options.text_colors.password_shadow.as_ref(),
                None,
                Color::TRANSPARENT,
                x,
                260.0,
            ),
        ),
    );
}

fn draw_copyright_asset(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    asset: &str,
    base: &BaseLayout,
) -> Result<(), RenderError> {
    if !bundle.has_image(asset) {
        return Ok(());
    }

    let Some((width, _)) = image_dimensions(bundle, asset) else {
        return Ok(());
    };
    let x = CARD_WIDTH.saturating_sub(base.copyright.right + width) as f32;
    bundle
        .draw_image_at(target, asset, x, base.copyright.y as f32)
        .map_err(RenderError::Backend)
}

fn draw_document_visual_effect(
    bundle: &AssetBundle,
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
        h: CARD_HEIGHT,
    };
    let art_rect = art_coverage_rect(request, base);

    if let EffectStyle::BrightBorder { opacity } = effect {
        // BrightBorder operates on both the outer card edge and the art frame
        // bevel simultaneously, so it needs both rects and cannot be expressed
        // as a single EffectArea.  Dispatch it here before the area loop.
        draw_bright_border(target, full_rect, art_rect, opacity);
        return;
    }

    for area in effect_target_areas(bundle, request, base, language, effect_target, full_rect, art_rect) {
        draw_visual_effect_area(target, area, effect);
    }
}

#[derive(Debug, Clone)]
enum EffectArea {
    Rect(CoverageRect),
    MaskedRect { rect: CoverageRect, mask: Pixmap },
}

fn draw_visual_effect_area(target: &mut Pixmap, area: EffectArea, effect: EffectStyle) {
    match area {
        EffectArea::Rect(rect) => draw_visual_effect_rect(target, rect, effect),
        EffectArea::MaskedRect { rect, mask } => draw_masked_visual_effect(target, rect, &mask, effect),
    }
}

fn draw_visual_effect_rect(target: &mut Pixmap, rect: CoverageRect, effect: EffectStyle) {
    match effect {
        EffectStyle::RainbowFoil { opacity } => draw_rainbow_foil(target, rect, opacity),
        EffectStyle::DotGrid { opacity } => draw_dot_grid(target, rect, opacity),
        EffectStyle::SecretWeave { opacity } => draw_secret_weave(target, rect, opacity),
        EffectStyle::Holographic { opacity } => draw_holographic(target, rect, opacity),
        EffectStyle::GoldWash { opacity } => draw_gold_wash(target, rect, opacity),
        EffectStyle::FrostedFoil { opacity } => draw_frosted_foil(target, rect, opacity),
        EffectStyle::ConcentricEngrave { opacity } => {
            draw_concentric_engrave(target, rect, opacity)
        }
        EffectStyle::ReliefEngrave { opacity } => draw_relief_engrave(target, rect, opacity),
        EffectStyle::BrightBorder { .. } => {}
    }
}

fn draw_masked_visual_effect(
    target: &mut Pixmap,
    rect: CoverageRect,
    mask: &Pixmap,
    effect: EffectStyle,
) {
    let width = target.width();
    let height = target.height();
    let x_start = rect.x.min(width);
    let y_start = rect.y.min(height);
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let sub_w = (x_end - x_start) as usize;

    // Snapshot only the rect sub-region instead of the full pixmap (~4 MB → KB).
    let mut before_sub: Vec<tiny_skia::PremultipliedColorU8> =
        Vec::with_capacity(sub_w * (y_end - y_start) as usize);
    {
        let src = target.pixels();
        for y in y_start..y_end {
            let row_start = (y * width + x_start) as usize;
            before_sub.extend_from_slice(&src[row_start..row_start + sub_w]);
        }
    }

    draw_visual_effect_rect(target, rect, effect);

    let mask_w = mask.width();
    let mask_h = mask.height();
    let mask_pixels = mask.pixels();
    let pixels = target.pixels_mut();

    for (iy, y) in (y_start..y_end).enumerate() {
        let local_y = y - y_start;
        for (ix, x) in (x_start..x_end).enumerate() {
            let local_x = x - x_start;
            let target_idx = (y * width + x) as usize;
            let before_idx = iy * sub_w + ix;

            if local_x >= mask_w || local_y >= mask_h {
                pixels[target_idx] = before_sub[before_idx];
                continue;
            }

            let mask_idx = (local_y * mask_w + local_x) as usize;
            let alpha = mask_pixels[mask_idx].alpha() as u16;
            if alpha == 0 {
                pixels[target_idx] = before_sub[before_idx];
            } else if alpha < 255 {
                pixels[target_idx] = lerp_premul(before_sub[before_idx], pixels[target_idx], alpha);
            }
        }
    }
}

fn lerp_premul(
    from: tiny_skia::PremultipliedColorU8,
    to: tiny_skia::PremultipliedColorU8,
    alpha: u16,
) -> tiny_skia::PremultipliedColorU8 {
    let inv = 255_u16.saturating_sub(alpha);
    let channel = |a: u8, b: u8| -> u8 { ((a as u16 * inv + b as u16 * alpha) / 255) as u8 };
    tiny_skia::PremultipliedColorU8::from_rgba(
        channel(from.red(), to.red()),
        channel(from.green(), to.green()),
        channel(from.blue(), to.blue()),
        channel(from.alpha(), to.alpha()),
    )
    .unwrap_or(from)
}

fn draw_gold_wash(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let pixels = target.pixels_mut();

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            let local_x = x.saturating_sub(rect.x) as f32;
            let local_y = y.saturating_sub(rect.y) as f32;
            let shimmer = ((local_x * 0.035 - local_y * 0.018).sin() * 0.5 + 0.5).powf(1.6);
            let noise = (pixel_hash(x, y) & 0xff) as f32 / 255.0;
            let alpha = (opacity * (0.72 + shimmer * 0.20 + noise * 0.08)).clamp(0.0, 1.0);
            let gold_r = (206.0 + shimmer * 38.0) as u8;
            let gold_g = (146.0 + shimmer * 70.0) as u8;
            let gold_b = (30.0 + shimmer * 28.0) as u8;

            let idx = (y * width + x) as usize;
            let dst = pixels[idx];
            let mix = |d: u8, s: u8| -> u8 {
                (d as f32 * (1.0 - alpha) + s as f32 * alpha).round() as u8
            };
            pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(
                mix(dst.red(), gold_r),
                mix(dst.green(), gold_g),
                mix(dst.blue(), gold_b),
                dst.alpha(),
            )
            .unwrap_or(dst);
        }
    }
}

fn draw_frosted_foil(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let pixels = target.pixels_mut();

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            let xf = x as f32;
            let yf = y as f32;
            let h0 = pixel_hash(x, y);
            let h1 = pixel_hash(x / 7, y / 7);
            let h2 = pixel_hash(x / 15, y / 15);
            let h3 = pixel_hash(x / 31, y / 31);
            let pin = (h0 & 0xff) as f32 / 255.0;
            let coarse = (h1 & 0xff) as f32 / 255.0;
            let pebble = (h2 & 0xff) as f32 / 255.0;
            let cloud = (h3 & 0xff) as f32 / 255.0;
            let sparkle = if h0 & 0x7ff < 18 || h1 & 0x1ff < 10 {
                1.0
            } else {
                0.0
            };
            let scratch = if ((x + y * 3) % 53 < 3) && (h1 & 0x3f < 8) {
                1.0
            } else {
                0.0
            };
            let diagonal_band = ((xf * 0.0038 - yf * 0.0025).sin() * 0.5 + 0.5).powf(0.70);
            let rainbow_sweep = ((xf * 0.0017 + yf * 0.0007).sin() * 0.5 + 0.5).powf(0.82);
            let grit_edge = ((xf * 0.22 - yf * 0.14).sin() * 0.5 + 0.5).powf(2.4);
            let cluster = if coarse > 0.68 {
                ((coarse - 0.68) / 0.32).powf(0.62)
            } else {
                0.0
            };
            let pebble_high = if pebble > 0.60 {
                ((pebble - 0.60) / 0.40).powf(0.82)
            } else {
                0.0
            };
            let matte = (cluster * 0.38
                + pebble_high * 0.20
                + pin * 0.06
                + cloud * 0.06
                + grit_edge * 0.08)
                .clamp(0.0, 1.0);
            let strength =
                (matte * 0.58
                    + diagonal_band * 0.34
                    + rainbow_sweep * 0.25
                    + sparkle * 0.30
                    + scratch * 0.14)
                    * opacity;
            let hue = (xf * 0.0011 - yf * 0.0016 + diagonal_band * 0.42 + cloud * 0.08)
                .rem_euclid(1.0);
            let (r, g, b) = hsv_to_rgb(hue, 0.94, 1.0);
            let silver = (0.04 + matte * 0.20 + sparkle * 0.14).min(0.42);
            let src_r = ((r * (1.0 - silver) + silver) * 255.0).round() as u8;
            let src_g = ((g * (1.0 - silver) + silver) * 255.0).round() as u8;
            let src_b = ((b * (1.0 - silver) + silver) * 255.0).round() as u8;
            let alpha = (strength.clamp(0.0, 1.0) * 224.0).round() as u8;

            let idx = (y * width + x) as usize;
            pixels[idx] = screen_pixel(pixels[idx], src_r, src_g, src_b, alpha);
        }
    }
}

fn draw_concentric_engrave(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let cx = rect.x as f32 + rect.w as f32 * 0.5;
    let cy = rect.y as f32 + rect.h as f32 * 0.5;
    let pixels = target.pixels_mut();

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);
            let rings = ((dist * 0.36).sin().abs()).powf(9.0);
            let radial = ((angle * 18.0 + dist * 0.04).sin().abs()).powf(8.0);
            let strength = (rings * 0.78 + radial * 0.22) * opacity;
            if strength < 0.015 {
                continue;
            }
            let hue = (0.11 + dist * 0.006 + angle * 0.04).rem_euclid(1.0);
            let (r, g, b) = hsv_to_rgb(hue, 0.9, 1.0);
            let alpha = (strength.clamp(0.0, 1.0) * 220.0).round() as u8;
            let idx = (y * width + x) as usize;
            pixels[idx] = screen_pixel(
                pixels[idx],
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8,
                alpha,
            );
        }
    }
}

fn draw_relief_engrave(target: &mut Pixmap, rect: CoverageRect, opacity: f32) {
    let width = target.width();
    let height = target.height();
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);

    // Pre-compute luma for the rect (+ SAMPLE_RADIUS border).
    // A wider radius lets us compute local variance for better flat-area
    // detection — real UTR engraving only appears in genuinely smooth regions.
    const SAMPLE_RADIUS: u32 = 6;
    let luma_x0 = rect.x.saturating_sub(SAMPLE_RADIUS);
    let luma_y0 = rect.y.saturating_sub(SAMPLE_RADIUS);
    let luma_x1 = x_end.saturating_add(SAMPLE_RADIUS).min(width);
    let luma_y1 = y_end.saturating_add(SAMPLE_RADIUS).min(height);
    let luma_w = luma_x1 - luma_x0;

    let mut luma: Vec<f32> =
        Vec::with_capacity(((luma_x1 - luma_x0) * (luma_y1 - luma_y0)) as usize);
    {
        let src = target.pixels();
        for ly in luma_y0..luma_y1 {
            for lx in luma_x0..luma_x1 {
                let p = src[(ly * width + lx) as usize];
                luma.push(
                    (p.red() as f32 * 0.299
                        + p.green() as f32 * 0.587
                        + p.blue() as f32 * 0.114)
                        / 255.0,
                );
            }
        }
    }

    // Sample luma at absolute card coordinates (clamped to the luma buffer).
    let sample = |ax: i32, ay: i32| -> f32 {
        let cx = ax.clamp(luma_x0 as i32, luma_x1 as i32 - 1) as u32;
        let cy = ay.clamp(luma_y0 as i32, luma_y1 as i32 - 1) as u32;
        luma[((cy - luma_y0) * luma_w + (cx - luma_x0)) as usize]
    };

    // ── Pre-compute colour-variance map ──────────────────────────────────
    // Real UTR engraving is absent in areas with rich colour variation.
    // We compute per-pixel local colour variance (R,G,B channels) over a
    // 5×5 neighbourhood and use it to suppress the effect in colourful
    // regions that the luma-only Sobel filter might miss.
    let var_radius: u32 = 2; // 5×5 window
    let var_x0 = rect.x.saturating_sub(var_radius);
    let var_y0 = rect.y.saturating_sub(var_radius);
    let var_x1 = x_end.saturating_add(var_radius).min(width);
    let var_y1 = y_end.saturating_add(var_radius).min(height);
    let var_w = var_x1 - var_x0;

    // Store per-pixel colour variance as a flat Vec aligned to (var_x0, var_y0).
    let color_var: Vec<f32> = {
        let src = target.pixels();
        let mut buf = Vec::with_capacity(((var_x1 - var_x0) * (var_y1 - var_y0)) as usize);
        for vy in var_y0..var_y1 {
            for vx in var_x0..var_x1 {
                let mut sum_r = 0.0_f32;
                let mut sum_g = 0.0_f32;
                let mut sum_b = 0.0_f32;
                let mut sum_r2 = 0.0_f32;
                let mut sum_g2 = 0.0_f32;
                let mut sum_b2 = 0.0_f32;
                let mut count = 0.0_f32;
                let ky0 = vy.saturating_sub(var_radius).max(var_y0);
                let ky1 = (vy + var_radius + 1).min(var_y1);
                let kx0 = vx.saturating_sub(var_radius).max(var_x0);
                let kx1 = (vx + var_radius + 1).min(var_x1);
                for ky in ky0..ky1 {
                    for kx in kx0..kx1 {
                        let p = src[(ky * width + kx) as usize];
                        let r = p.red() as f32 / 255.0;
                        let g = p.green() as f32 / 255.0;
                        let b = p.blue() as f32 / 255.0;
                        sum_r += r;
                        sum_g += g;
                        sum_b += b;
                        sum_r2 += r * r;
                        sum_g2 += g * g;
                        sum_b2 += b * b;
                        count += 1.0;
                    }
                }
                let var = if count > 1.0 {
                    let vr = (sum_r2 / count - (sum_r / count).powi(2)).max(0.0);
                    let vg = (sum_g2 / count - (sum_g / count).powi(2)).max(0.0);
                    let vb = (sum_b2 / count - (sum_b / count).powi(2)).max(0.0);
                    vr + vg + vb
                } else {
                    0.0
                };
                buf.push(var);
            }
        }
        buf
    };

    let sample_var = |ax: u32, ay: u32| -> f32 {
        let cx = ax.clamp(var_x0, var_x1 - 1);
        let cy = ay.clamp(var_y0, var_y1 - 1);
        color_var[((cy - var_y0) * var_w + (cx - var_x0)) as usize]
    };

    let pixels = target.pixels_mut();

    // ── Primary diagonal angle for the parallel scratch lines ────────────
    // Real UTR uses a dominant ~40-50° angle with slight local wobble.
    // We use two main line families at slightly different angles to create
    // the characteristic brushed-metal / fine-engraving look.
    const PRIMARY_ANGLE: f32 = 0.74;   // ~42° in radians
    const SECONDARY_ANGLE: f32 = 0.52; // ~30° — subtle cross-set
    let cos_p = PRIMARY_ANGLE.cos();
    let sin_p = PRIMARY_ANGLE.sin();
    let cos_s = SECONDARY_ANGLE.cos();
    let sin_s = SECONDARY_ANGLE.sin();

    for y in rect.y.min(height)..y_end {
        for x in rect.x.min(width)..x_end {
            if x == 0 || y == 0 || x + 1 >= width || y + 1 >= height {
                continue;
            }

            let xf = x.saturating_sub(rect.x) as f32;
            let yf = y.saturating_sub(rect.y) as f32;

            // ── Flat-area gating ─────────────────────────────────────────
            let tl = sample(x as i32 - 1, y as i32 - 1);
            let tc = sample(x as i32, y as i32 - 1);
            let tr = sample(x as i32 + 1, y as i32 - 1);
            let ml = sample(x as i32 - 1, y as i32);
            let mc = sample(x as i32, y as i32);
            let mr = sample(x as i32 + 1, y as i32);
            let bl = sample(x as i32 - 1, y as i32 + 1);
            let bc = sample(x as i32, y as i32 + 1);
            let br = sample(x as i32 + 1, y as i32 + 1);

            // Sobel edge magnitude
            let sobel_x = (tr + 2.0 * mr + br) - (tl + 2.0 * ml + bl);
            let sobel_y = (bl + 2.0 * bc + br) - (tl + 2.0 * tc + tr);
            let edge = (sobel_x * sobel_x + sobel_y * sobel_y).sqrt().min(1.0);

            // Local luma deviation from neighbourhood mean
            let avg = (tl + tc + tr + ml + mr + bl + bc + br) / 8.0;
            let detail = (mc - avg).abs().min(1.0);

            // Colour variance gate — suppress in colourful regions
            let cvar = sample_var(x, y);
            let color_gate = (1.0 - (cvar * 28.0).clamp(0.0, 1.0)).powf(1.4);

            // Combined flat-area mask: must pass edge, detail, AND colour gates
            let edge_gate = (1.0 - (edge * 6.0).clamp(0.0, 1.0)).powf(2.0);
            let detail_gate = (1.0 - (detail * 14.0).clamp(0.0, 1.0)).powf(1.5);
            let flat_mask = edge_gate * detail_gate * color_gate;
            if flat_mask < 0.02 {
                continue;
            }

            // ── Parallel diagonal scratch lines ──────────────────────────
            // Project pixel position onto the line-perpendicular axis to
            // create evenly-spaced parallel lines.  A small luma-dependent
            // phase shift makes lines wobble slightly with the artwork's
            // tonal contours, mimicking how real engraving follows surfaces.
            let luma_wobble = mc * 1.6;

            // Primary line family — dominant scratches at ~42°
            let proj_p = xf * cos_p + yf * sin_p;
            let line_p1 = ((proj_p * 0.38 + luma_wobble).sin().abs()).powf(18.0);
            let line_p2 = ((proj_p * 0.72 + luma_wobble * 0.7).sin().abs()).powf(22.0);
            let line_p3 = ((proj_p * 1.45 + luma_wobble * 0.4).sin().abs()).powf(28.0);

            // Secondary line family — finer cross-scratches at ~30°
            let proj_s = xf * cos_s + yf * sin_s;
            let line_s1 = ((proj_s * 0.55 + luma_wobble * 0.5).sin().abs()).powf(24.0);
            let line_s2 = ((proj_s * 1.10 + luma_wobble * 0.3).sin().abs()).powf(30.0);

            // Contour lines that follow luma iso-levels (subtle)
            let contour = ((mc * 32.0 + xf * 0.004 - yf * 0.006).sin().abs()).powf(26.0);

            // Combine: primary dominates, secondary adds texture, contour adds depth
            let line = line_p1 * 0.32
                + line_p2 * 0.22
                + line_p3 * 0.10
                + line_s1 * 0.16
                + line_s2 * 0.08
                + contour * 0.12;

            let strength = line * flat_mask * opacity;
            if strength < 0.008 {
                continue;
            }

            // ── Colour: silvery metallic with very subtle hue shift ──────
            // Real UTR engravings are predominantly silver/white with a
            // faint rainbow sheen, not strongly coloured.
            let hue = (0.08 + xf * 0.0005 + yf * 0.0007 + mc * 0.03).rem_euclid(1.0);
            let (r, g, b) = hsv_to_rgb(hue, 0.18, 1.0);

            let idx = (y * width + x) as usize;
            // Slight darken to simulate the engraved groove shadow
            pixels[idx] = darken_pixel(pixels[idx], (strength * 0.12).clamp(0.0, 0.14));
            // Screen-blend the metallic highlight
            let alpha = (strength.clamp(0.0, 1.0) * 145.0).round() as u8;
            pixels[idx] = screen_pixel(
                pixels[idx],
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8,
                alpha,
            );
        }
    }
}

fn darken_pixel(
    dst: tiny_skia::PremultipliedColorU8,
    amount: f32,
) -> tiny_skia::PremultipliedColorU8 {
    let keep = (1.0 - amount.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    tiny_skia::PremultipliedColorU8::from_rgba(
        (dst.red() as f32 * keep).round() as u8,
        (dst.green() as f32 * keep).round() as u8,
        (dst.blue() as f32 * keep).round() as u8,
        dst.alpha(),
    )
    .unwrap_or(dst)
}

fn effect_target_areas(
    bundle: &AssetBundle,
    request: &RenderRequest,
    base: &BaseLayout,
    language: Option<&str>,
    target: EffectTarget,
    full_rect: CoverageRect,
    art_rect: CoverageRect,
) -> Vec<EffectArea> {
    match target {
        EffectTarget::FullCard => vec![EffectArea::Rect(full_rect)],
        EffectTarget::Art => vec![EffectArea::Rect(art_rect)],
        EffectTarget::CardBase => card_base_areas_excluding_art(full_rect, art_rect)
            .into_iter()
            .map(EffectArea::Rect)
            .collect(),
        EffectTarget::ArtFrame => frame_ring_areas(art_rect, 28, 0, 0)
            .into_iter()
            .map(EffectArea::Rect)
            .collect(),
        EffectTarget::CardBorder => card_border_areas()
            .into_iter()
            .map(EffectArea::Rect)
            .collect(),
        EffectTarget::Attribute => attribute_effect_area(bundle, request, base, language)
            .into_iter()
            .collect(),
        EffectTarget::LevelOrRank => level_or_rank_effect_areas(bundle, request, base),
    }
}

fn card_base_areas_excluding_art(full_rect: CoverageRect, art_rect: CoverageRect) -> Vec<CoverageRect> {
    let fx = full_rect.x;
    let fy = full_rect.y;
    let fw = full_rect.w;
    let fh = full_rect.h;
    let ax = art_rect.x.min(fx + fw);
    let ay = art_rect.y.min(fy + fh);
    let ar = art_rect.x.saturating_add(art_rect.w).min(fx + fw);
    let ab = art_rect.y.saturating_add(art_rect.h).min(fy + fh);

    [
        CoverageRect {
            x: fx,
            y: fy,
            w: fw,
            h: ay.saturating_sub(fy),
        },
        CoverageRect {
            x: fx,
            y: ab,
            w: fw,
            h: fy.saturating_add(fh).saturating_sub(ab),
        },
        CoverageRect {
            x: fx,
            y: ay,
            w: ax.saturating_sub(fx),
            h: ab.saturating_sub(ay),
        },
        CoverageRect {
            x: ar,
            y: ay,
            w: fx.saturating_add(fw).saturating_sub(ar),
            h: ab.saturating_sub(ay),
        },
    ]
    .into_iter()
    .filter(|rect| rect.w > 0 && rect.h > 0)
    .collect()
}

fn card_border_areas() -> Vec<CoverageRect> {
    let outer = CoverageRect {
        x: 0,
        y: 0,
        w: CARD_WIDTH,
        h: CARD_HEIGHT,
    };
    frame_ring_areas(outer, 54, 0, 0)
}

fn frame_ring_areas(rect: CoverageRect, thickness: u32, inset_x: u32, inset_y: u32) -> Vec<CoverageRect> {
    let x = rect.x.saturating_sub(inset_x);
    let y = rect.y.saturating_sub(inset_y);
    let w = rect.w.saturating_add(inset_x.saturating_mul(2));
    let h = rect.h.saturating_add(inset_y.saturating_mul(2));
    let t = thickness.min(w / 2).min(h / 2).max(1);
    vec![
        CoverageRect { x, y, w, h: t },
        CoverageRect {
            x,
            y: y + h.saturating_sub(t),
            w,
            h: t,
        },
        CoverageRect {
            x,
            y: y + t,
            w: t,
            h: h.saturating_sub(t.saturating_mul(2)),
        },
        CoverageRect {
            x: x + w.saturating_sub(t),
            y: y + t,
            w: t,
            h: h.saturating_sub(t.saturating_mul(2)),
        },
    ]
}

fn art_coverage_rect(request: &RenderRequest, base: &BaseLayout) -> CoverageRect {
    let (x, y, w, h) = image_frame(&request.card, base);
    CoverageRect { x, y, w, h }
}

fn attribute_effect_area(
    bundle: &AssetBundle,
    request: &RenderRequest,
    base: &BaseLayout,
    language: Option<&str>,
) -> Option<EffectArea> {
    let asset = attribute_asset_name(&request.card, language)?;
    let mask = decode_bundle_image(bundle, &asset)?;
    let rect = CoverageRect {
        x: base.attribute.x,
        y: base.attribute.y,
        w: mask.width(),
        h: mask.height(),
    };
    Some(EffectArea::MaskedRect { rect, mask })
}

fn level_or_rank_effect_areas(
    bundle: &AssetBundle,
    request: &RenderRequest,
    base: &BaseLayout,
) -> Vec<EffectArea> {
    let count = request.card.level.min(13);
    if count == 0 || request.card.is_link() {
        return Vec::new();
    }

    let (layout, left_to_right) = if uses_rank(&request.card) {
        (&base.rank, true)
    } else {
        (&base.level, false)
    };
    let Some(mask) = decode_bundle_image(bundle, &layout.asset) else {
        return Vec::new();
    };
    let h = mask.height();

    let start = if left_to_right {
        if count < 13 {
            layout.left_lt_13.unwrap_or(147)
        } else {
            layout.left_ge_13.unwrap_or(101)
        }
    } else if count < 13 {
        layout.right_lt_13.unwrap_or(147)
    } else {
        layout.right_ge_13.unwrap_or(101)
    };

    (0..count)
        .map(|index| {
            let x = if left_to_right {
                start + index * (layout.star_width + layout.gap)
            } else {
                CARD_WIDTH - start - index * (layout.star_width + layout.gap) - layout.star_width
            };
            let rect = CoverageRect {
                x,
                y: layout.y,
                w: layout.star_width,
                h,
            };
            EffectArea::MaskedRect {
                rect,
                mask: mask.clone(),
            }
        })
        .collect()
}

fn decode_bundle_image(bundle: &AssetBundle, asset: &str) -> Option<Pixmap> {
    let entry = bundle.image(asset).ok()?;
    match entry.kind.as_str() {
        "raster" => bundle.decode_raster(asset).ok(),
        "svg" => bundle.decode_svg(asset).ok(),
        _ => None,
    }
}

fn text_align_choice(align: TextAlignChoice) -> TextAlign {
    match align {
        TextAlignChoice::Left => TextAlign::Left,
        TextAlignChoice::Center => TextAlign::Center,
        TextAlignChoice::Right => TextAlign::Right,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_document_text_block(
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    language: Option<&str>,
    text: &str,
    rect: &RenderRect,
    font_family: &str,
    font_size: u32,
    line_height: f32,
    letter_spacing: f32,
    channel: TextChannel,
) {
    let (brush, shadow_brush) = match channel {
        TextChannel::Description => (
            text_brush(
                request.options.text_colors.description.as_ref(),
                request.options.description_color_override.as_deref(),
                Color::BLACK,
                rect.x,
                rect.width,
            ),
            text_brush(
                request.options.text_colors.description_shadow.as_ref(),
                None,
                Color::TRANSPARENT,
                rect.x,
                rect.width,
            ),
        ),
        _ => (None, None),
    };

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
            rt_font_size: style.description_rt_font_size,
            rt_top: style.description_rt_top,
            rt_font_scale_x: style.description_rt_font_scale_x,
            line_height,
            letter_spacing,
            min_font_size: font_size.saturating_sub(8),
            first_line_compress: request.options.description_first_line_compress,
        },
    );
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

fn draw_pendulum_description(
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
    text: &str,
) {
    draw_multiline_ruby_text(
        target,
        RubyMultilineParams {
            text,
            x: base.pendulum_description.x as f32,
            y: style.pendulum_description_top as f32,
            width: base.pendulum_description.width as f32,
            height: base.pendulum_description.height as f32,
            family: &style.base_font_family,
            color: Color::BLACK,
            shadow_color: Color::TRANSPARENT,
            brush: text_brush(
                request.options.text_colors.description.as_ref(),
                request.options.description_color_override.as_deref(),
                Color::BLACK,
                base.pendulum_description.x as f32,
                base.pendulum_description.width as f32,
            ),
            shadow_brush: text_brush(
                request.options.text_colors.description_shadow.as_ref(),
                None,
                Color::TRANSPARENT,
                base.pendulum_description.x as f32,
                base.pendulum_description.width as f32,
            ),
            language,
            base_font_size: style.pendulum_description_size,
            rt_font_size: style.description_rt_font_size,
            rt_top: style.description_rt_top,
            rt_font_scale_x: style.description_rt_font_scale_x,
            line_height: style.pendulum_description_line_height,
            letter_spacing: style.pendulum_description_letter_spacing,
            min_font_size: style.pendulum_description_size.saturating_sub(8),
            first_line_compress: request.options.description_first_line_compress,
        },
    );
}

fn draw_frame(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    asset_name: &str,
) -> Result<(), RenderError> {
    bundle
        .draw_image_at(target, asset_name, 0.0, 0.0)
        .map_err(RenderError::Backend)
}

fn draw_art(
    _bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    base: &BaseLayout,
) -> Result<(), RenderError> {
    if let Some(art_path) = &request.options.art_image {
        if let Some(art_pixmap) = load_external_pixmap(art_path) {
            let w = art_pixmap.width();
            let h = art_pixmap.height();
            let (art_x, art_y, frame_w, frame_h) = image_frame(&request.card, base);
            let scale_x = frame_w as f32 / w as f32;
            let scale_y = frame_h as f32 / h as f32;
            // tiny-skia's draw_pixmap transform applies to the source pixmap
            // in destination space. Pass x=0/y=0 and encode the full
            // translate+scale in the transform so they don't double-apply.
            target.draw_pixmap(
                0,
                0,
                art_pixmap.as_ref(),
                &PixmapPaint::default(),
                Transform::from_scale(scale_x, scale_y).post_translate(art_x as f32, art_y as f32),
                None,
            );
        }
    }
    Ok(())
}

fn draw_mask(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    base: &BaseLayout,
) -> Result<(), RenderError> {
    let mask = if request.card.is_pendulum() {
        &base.mask.pendulum
    } else {
        &base.mask.normal
    };
    bundle
        .draw_image_at(target, &mask.asset, mask.x as f32, mask.y as f32)
        .map_err(RenderError::Backend)
}

fn draw_foreground_image(target: &mut Pixmap, request: &RenderRequest) -> Result<(), RenderError> {
    let foreground = if request.card.out_frame {
        request
            .card
            .out_frame_image
            .as_ref()
            .or(request.options.foreground_image.as_ref())
    } else {
        request.options.foreground_image.as_ref()
    };

    let Some(foreground) = foreground else {
        return Ok(());
    };

    draw_positioned_render_image(target, foreground);
    Ok(())
}

fn draw_positioned_render_image(target: &mut Pixmap, image: &PositionedRenderImage) {
    let Some(pixmap) = load_external_pixmap(&image.path) else {
        return;
    };

    target.draw_pixmap(
        image.x,
        image.y,
        pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

fn load_external_pixmap(path: &std::path::Path) -> Option<Pixmap> {
    let img = image::open(path).ok()?;
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let mut pixmap = Pixmap::from_vec(
        rgba.into_raw(),
        tiny_skia::IntSize::from_wh(width, height).unwrap(),
    )?;
    premultiply_pixmap_alpha(&mut pixmap);
    Some(pixmap)
}

fn premultiply_pixmap_alpha(pixmap: &mut Pixmap) {
    for pixel in pixmap.pixels_mut() {
        let a = pixel.alpha();
        if a == 255 {
            continue;
        }
        if a == 0 {
            *pixel = tiny_skia::PremultipliedColorU8::TRANSPARENT;
            continue;
        }

        let r = (pixel.red() as u16 * a as u16 / 255) as u8;
        let g = (pixel.green() as u16 * a as u16 / 255) as u8;
        let b = (pixel.blue() as u16 * a as u16 / 255) as u8;
        *pixel = tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a)
            .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
    }
}

fn draw_out_frame_blocks(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    base: &BaseLayout,
) -> Result<(), RenderError> {
    if !request.card.out_frame {
        return Ok(());
    }

    if request.card.out_frame_name_block_enabled {
        let name_block = &base.out_frame.name_block;
        bundle
            .draw_image_at(
                target,
                &name_block.asset,
                name_block.x as f32,
                name_block.y as f32,
            )
            .map_err(RenderError::Backend)?;
    }

    let effect_box = match request.card.out_frame_effect_box {
        OutFrameEffectBox::EblockBorder => &base.out_frame.effect_box,
        OutFrameEffectBox::EblockBorderO => &base.out_frame.effect_box_colored,
    };

    if request.card.out_frame_effect_enabled {
        draw_out_frame_effect_background(bundle, target, request, effect_box);
        bundle
            .draw_image_at(
                target,
                &effect_box.asset,
                effect_box.x as f32,
                effect_box.y as f32,
            )
            .map_err(RenderError::Backend)?;
    }

    Ok(())
}

fn draw_out_frame_effect_background(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    effect_box: &crate::asset_bundle::PositionedAsset,
) {
    let Some(color) = request
        .card
        .out_frame_effect_background_color
        .as_deref()
        .and_then(parse_hex_color)
    else {
        return;
    };

    let opacity = request
        .card
        .out_frame_effect_opacity
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }

    let Some((width, height)) = image_dimensions(bundle, &effect_box.asset) else {
        return;
    };

    let Some(color) = Color::from_rgba(
        color.red(),
        color.green(),
        color.blue(),
        color.alpha() * opacity,
    ) else {
        return;
    };
    let Some(rect) = Rect::from_xywh(
        effect_box.x as f32,
        effect_box.y as f32,
        width as f32,
        height as f32,
    ) else {
        return;
    };

    let mut paint = Paint::default();
    paint.set_color(color);
    target.fill_rect(rect, &paint, Transform::identity(), None);
}

fn image_dimensions(bundle: &AssetBundle, asset: &str) -> Option<(u32, u32)> {
    let image = bundle.image(asset).ok()?;
    if let Some(size) = &image.size {
        Some((size.w, size.h))
    } else {
        image.atlas.as_ref().map(|sprite| (sprite.w, sprite.h))
    }
}

fn draw_anniversary_mark(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    base: &BaseLayout,
) -> Result<(), RenderError> {
    let mark = if request.card.twenty_fifth {
        Some(&base.twenty_fifth)
    } else if request.card.twentieth {
        Some(&base.twentieth)
    } else {
        None
    };

    if let Some(mark) = mark {
        bundle
            .draw_image_at(target, &mark.asset, mark.x as f32, mark.y as f32)
            .map_err(RenderError::Backend)?;
    }

    Ok(())
}

fn draw_attribute(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    base: &BaseLayout,
    language: Option<&str>,
) -> Result<(), RenderError> {
    if let Some(asset) = attribute_asset_name(&request.card, language) {
        if bundle.has_image(&asset) {
            bundle
                .draw_image_at(
                    target,
                    &asset,
                    base.attribute.x as f32,
                    base.attribute.y as f32,
                )
                .map_err(RenderError::Backend)?;
        }
    }
    Ok(())
}

fn draw_level_or_rank(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    base: &BaseLayout,
) -> Result<(), RenderError> {
    let count = request.card.level.min(13);
    if count == 0 || request.card.is_link() {
        return Ok(());
    }

    let (layout, left_to_right) = if uses_rank(&request.card) {
        (&base.rank, true)
    } else {
        (&base.level, false)
    };

    let start = if left_to_right {
        if count < 13 {
            layout.left_lt_13.unwrap_or(147)
        } else {
            layout.left_ge_13.unwrap_or(101)
        }
    } else if count < 13 {
        layout.right_lt_13.unwrap_or(147)
    } else {
        layout.right_ge_13.unwrap_or(101)
    };

    for index in 0..count {
        let x = if left_to_right {
            start + index * (layout.star_width + layout.gap)
        } else {
            CARD_WIDTH - start - index * (layout.star_width + layout.gap) - layout.star_width
        };
        bundle
            .draw_image_at(target, &layout.asset, x as f32, layout.y as f32)
            .map_err(RenderError::Backend)?;
    }
    Ok(())
}

fn draw_link_arrows(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    base: &BaseLayout,
) -> Result<(), RenderError> {
    if !request.card.is_link() {
        return Ok(());
    }

    let arrows = [
        (0x004_u32, "up"),
        (0x080_u32, "right_up"),
        (0x020_u32, "right"),
        (0x100_u32, "right_down"),
        (0x040_u32, "down"),
        (0x008_u32, "left_down"),
        (0x001_u32, "left"),
        (0x002_u32, "left_up"),
    ];

    for (bit, name) in arrows {
        if let Some(pair) = base.link_arrows.get(name) {
            let state = if (request.card.link_marker & bit) != 0 {
                &pair.on
            } else {
                &pair.off
            };
            bundle
                .draw_image_at(target, &state.asset, state.x as f32, state.y as f32)
                .map_err(RenderError::Backend)?;
        }
    }

    Ok(())
}

fn draw_title(
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
) {
    let show_attribute =
        request.card.attribute != 0 || request.card.is_spell() || request.card.is_trap();
    let title_width = if show_attribute {
        base.name.width_with_attribute
    } else {
        base.name.width_without_attribute
    };

    let name_color = resolve_name_color(&request.card.name_color, &request.card);
    let name_brush =
        resolve_name_brush(request, name_color, style.name_x as f32, title_width as f32);
    let name_shadow = resolve_name_shadow_brush(request, style.name_x as f32, title_width as f32);

    // Ruby path: JP language with rt_font_size set and markup present.
    if style.name_rt_font_size > 0 && contains_ruby_markup(&request.card.name) {
        let tokens = parse_ruby_text(&request.card.name);
        let scale_x = fit_ruby_text_scale(
            &tokens,
            &style.name_font_family,
            style.name_size as f32,
            style.name_rt_font_size as f32,
            style.title_letter_spacing,
            style.name_rt_font_scale_x,
            title_width as f32,
        )
        .max(0.3);
        if name_shadow.color.alpha() > 0.0 || name_shadow.brush.is_some() {
            draw_ruby_text_line(
                target,
                RubyLineParams {
                    tokens: &tokens,
                    x: style.name_x as f32 + 7.0,
                    y: style.name_top as f32 + 7.0,
                    font_size: style.name_size as f32,
                    rt_font_size: style.name_rt_font_size as f32,
                    rt_top: style.name_rt_top,
                    rt_font_scale_x_override: style.name_rt_font_scale_x,
                    color: name_shadow.color,
                    shadow_color: Color::TRANSPARENT,
                    brush: name_shadow.brush.clone(),
                    shadow_brush: None,
                    family: &style.name_font_family,
                    language,
                    letter_spacing: style.title_letter_spacing,
                    scale_x,
                },
            );
        }

        draw_ruby_text_line(
            target,
            RubyLineParams {
                tokens: &tokens,
                x: style.name_x as f32,
                y: style.name_top as f32,
                font_size: style.name_size as f32,
                rt_font_size: style.name_rt_font_size as f32,
                rt_top: style.name_rt_top,
                rt_font_scale_x_override: style.name_rt_font_scale_x,
                color: name_brush.color,
                shadow_color: Color::TRANSPARENT,
                brush: name_brush.brush,
                shadow_brush: None,
                family: &style.name_font_family,
                language,
                letter_spacing: style.title_letter_spacing,
                scale_x,
            },
        );
        return;
    }

    // Plain path (all other cases).
    let title_layout = if request.options.title_width_compress {
        fit_single_line_compressed(
            &request.card.name,
            language,
            style.name_size,
            &style.name_font_family,
            title_width,
            style.title_letter_spacing,
            0.3,
        )
    } else {
        fit_single_line(
            &request.card.name,
            language,
            style.name_size,
            &style.name_font_family,
            title_width,
            style.title_letter_spacing,
            style.name_size.saturating_sub(26),
        )
    };

    if name_shadow.color.alpha() > 0.0 || name_shadow.brush.is_some() {
        draw_text_line_scaled(
            target,
            DrawTextLine {
                text: &title_layout.text,
                x: style.name_x as f32 + 7.0,
                y: style.name_top as f32 + 7.0,
                font_size: title_layout.font_size as f32,
                max_width: title_layout.max_width as f32,
                color: name_shadow.color,
                shadow_color: Color::TRANSPARENT,
                brush: name_shadow.brush,
                shadow_brush: None,
                family_name: &style.name_font_family,
                align: TextAlign::Left,
                language,
                letter_spacing: title_layout.letter_spacing,
                scale_x: title_layout.scale_x,
            },
        );
    }

    draw_text_line_scaled(
        target,
        DrawTextLine {
            text: &title_layout.text,
            x: style.name_x as f32,
            y: style.name_top as f32,
            font_size: title_layout.font_size as f32,
            max_width: title_layout.max_width as f32,
            color: name_brush.color,
            shadow_color: Color::TRANSPARENT,
            brush: name_brush.brush,
            shadow_brush: None,
            family_name: &style.name_font_family,
            align: TextAlign::Left,
            language,
            letter_spacing: title_layout.letter_spacing,
            scale_x: title_layout.scale_x,
        },
    );
}

fn draw_spell_trap_line(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
) -> Result<(), RenderError> {
    let (left_bracket, right_bracket) = localized_brackets(language);
    let left_text = format!(
        "{left_bracket}{}",
        localized_spell_trap_name(&request.card, language)
    );
    let right_text = right_bracket;
    let font_size = style.type_size as f32;
    let letter_spacing = style.type_letter_spacing;
    let text_color = Color::from_rgba8(TYPE_COLOR.0, TYPE_COLOR.1, TYPE_COLOR.2, 255);

    let right_margin = style.type_right as f32;
    let right_width = estimate_text_width(
        right_text,
        language,
        &style.type_font_family,
        font_size,
        letter_spacing,
    );
    let right_x = CARD_WIDTH as f32 - right_margin - right_width;

    draw_text_line(
        target,
        DrawTextLine::unscaled(
            right_text,
            right_x,
            style.type_top as f32,
            font_size,
            right_width.ceil().max(32.0),
            text_color,
            Color::TRANSPARENT,
            &style.type_font_family,
            TextAlign::Left,
            language,
            letter_spacing,
        ),
    );

    let icon_asset = spell_trap_subtype_icon_asset(&request.card);
    let icon_margins = bundle_style_icon_margins(language, bundle);
    let icon_width = 72.0_f32;

    let icon_x = if icon_asset.is_some() {
        right_x - icon_margins.right - icon_width
    } else {
        right_x
    };

    if let Some(icon_asset) = icon_asset {
        let text_top_correction = font_size * 0.092;
        let icon_y = style.type_top as f32 + icon_margins.top - text_top_correction;
        bundle
            .draw_image_at(target, icon_asset, icon_x, icon_y)
            .map_err(RenderError::Backend)?;
    }

    // Strip ruby markup when measuring layout width (the plain base text drives spacing).
    let left_text_stripped = strip_ruby_markup(&left_text);
    let left_width = estimate_text_width(
        &left_text_stripped,
        language,
        &style.type_font_family,
        font_size,
        letter_spacing,
    );
    let left_x = icon_x
        - if icon_asset.is_some() {
            icon_margins.left
        } else {
            0.0
        }
        - left_width;

    // Draw: use ruby path when markup is present and rt_font_size is configured.
    if style.type_rt_font_size > 0 && contains_ruby_markup(&left_text) {
        let tokens = parse_ruby_text(&left_text);
        draw_ruby_text_line(
            target,
            RubyLineParams {
                tokens: &tokens,
                x: left_x,
                y: style.type_top as f32,
                font_size,
                rt_font_size: style.type_rt_font_size as f32,
                rt_top: style.type_rt_top,
                rt_font_scale_x_override: style.type_rt_font_scale_x,
                color: text_color,
                shadow_color: Color::TRANSPARENT,
                brush: text_brush(
                    request.options.text_colors.type_line.as_ref(),
                    None,
                    text_color,
                    left_x,
                    left_width,
                ),
                shadow_brush: text_brush(
                    request.options.text_colors.type_line_shadow.as_ref(),
                    None,
                    Color::TRANSPARENT,
                    left_x,
                    left_width,
                ),
                family: &style.type_font_family,
                language,
                letter_spacing,
                scale_x: 1.0,
            },
        );
    } else {
        draw_text_line(
            target,
            DrawTextLine::unscaled(
                &left_text,
                left_x,
                style.type_top as f32,
                font_size,
                left_width.ceil().max(80.0),
                text_color,
                Color::TRANSPARENT,
                &style.type_font_family,
                TextAlign::Left,
                language,
                letter_spacing,
            )
            .with_brushes(
                text_brush(
                    request.options.text_colors.type_line.as_ref(),
                    None,
                    text_color,
                    left_x,
                    left_width,
                ),
                text_brush(
                    request.options.text_colors.type_line_shadow.as_ref(),
                    None,
                    Color::TRANSPARENT,
                    left_x,
                    left_width,
                ),
            ),
        );
    }

    let _ = base;
    Ok(())
}

fn draw_stats(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
) {
    if request.card.is_monster() {
        draw_stat_separator(bundle, target, request, base, language);

        let value_color = Color::BLACK;

        draw_text_line(
            target,
            DrawTextLine::unscaled(
                &display_stat(request.card.attack),
                style.stat_atk_x as f32,
                style.stat_top as f32,
                style.stat_size as f32,
                220.0,
                value_color,
                Color::TRANSPARENT,
                &style.stat_font_family,
                TextAlign::Right,
                language,
                style.stat_letter_spacing,
            )
            .with_brushes(
                text_brush(
                    request.options.text_colors.stats.as_ref(),
                    None,
                    value_color,
                    style.stat_atk_x as f32 - 220.0,
                    220.0,
                ),
                text_brush(
                    request.options.text_colors.stats_shadow.as_ref(),
                    None,
                    Color::TRANSPARENT,
                    style.stat_atk_x as f32 - 220.0,
                    220.0,
                ),
            ),
        );

        if request.card.is_link() {
            let link_text = if language == Some("astral") {
                &base.atk_def_link.link.astral
            } else {
                &base.atk_def_link.link.default
            };

            draw_text_line_scaled(
                target,
                DrawTextLine {
                    text: &request.card.level.max(1).to_string(),
                    x: style.stat_link_x as f32,
                    y: style.link_top as f32,
                    font_size: style.link_size as f32,
                    max_width: 120.0,
                    color: value_color,
                    shadow_color: Color::TRANSPARENT,
                    brush: text_brush(
                        request.options.text_colors.stats.as_ref(),
                        None,
                        value_color,
                        style.stat_link_x as f32 - 120.0,
                        120.0,
                    ),
                    shadow_brush: text_brush(
                        request.options.text_colors.stats_shadow.as_ref(),
                        None,
                        Color::TRANSPARENT,
                        style.stat_link_x as f32 - 120.0,
                        120.0,
                    ),
                    family_name: &style.link_font_family,
                    align: TextAlign::Right,
                    language,
                    letter_spacing: style.stat_letter_spacing,
                    scale_x: link_text.scale_x.unwrap_or(1.0),
                },
            );
        } else {
            draw_text_line(
                target,
                DrawTextLine::unscaled(
                    &display_stat(request.card.defense),
                    style.stat_def_x as f32,
                    style.stat_top as f32,
                    style.stat_size as f32,
                    220.0,
                    value_color,
                    Color::TRANSPARENT,
                    &style.stat_font_family,
                    TextAlign::Right,
                    language,
                    style.stat_letter_spacing,
                )
                .with_brushes(
                    text_brush(
                        request.options.text_colors.stats.as_ref(),
                        None,
                        value_color,
                        style.stat_def_x as f32 - 220.0,
                        220.0,
                    ),
                    text_brush(
                        request.options.text_colors.stats_shadow.as_ref(),
                        None,
                        Color::TRANSPARENT,
                        style.stat_def_x as f32 - 220.0,
                        220.0,
                    ),
                ),
            );
        }
    }

    if request.card.is_pendulum() {
        let left = if language == Some("astral") {
            &base.pendulum_scale.left.astral
        } else {
            &base.pendulum_scale.left.default
        };
        let right = if language == Some("astral") {
            &base.pendulum_scale.right.astral
        } else {
            &base.pendulum_scale.right.default
        };

        draw_text_line(
            target,
            DrawTextLine::unscaled(
                &request.card.lscale.to_string(),
                left.x as f32,
                left.y as f32,
                if language == Some("astral") {
                    84.0
                } else {
                    98.0
                },
                120.0,
                Color::from_rgba8(TEXT_COLOR_DARK.0, TEXT_COLOR_DARK.1, TEXT_COLOR_DARK.2, 255),
                Color::TRANSPARENT,
                &style.stat_font_family,
                TextAlign::Center,
                language,
                if language == Some("astral") {
                    0.0
                } else {
                    -10.0
                },
            )
            .with_brushes(
                text_brush(
                    request.options.text_colors.stats.as_ref(),
                    None,
                    Color::from_rgba8(TEXT_COLOR_DARK.0, TEXT_COLOR_DARK.1, TEXT_COLOR_DARK.2, 255),
                    left.x as f32,
                    120.0,
                ),
                text_brush(
                    request.options.text_colors.stats_shadow.as_ref(),
                    None,
                    Color::TRANSPARENT,
                    left.x as f32,
                    120.0,
                ),
            ),
        );
        draw_text_line(
            target,
            DrawTextLine::unscaled(
                &request.card.rscale.to_string(),
                right.x as f32,
                right.y as f32,
                if language == Some("astral") {
                    84.0
                } else {
                    98.0
                },
                120.0,
                Color::from_rgba8(TEXT_COLOR_DARK.0, TEXT_COLOR_DARK.1, TEXT_COLOR_DARK.2, 255),
                Color::TRANSPARENT,
                &style.stat_font_family,
                TextAlign::Center,
                language,
                if language == Some("astral") {
                    0.0
                } else {
                    -10.0
                },
            )
            .with_brushes(
                text_brush(
                    request.options.text_colors.stats.as_ref(),
                    None,
                    Color::from_rgba8(TEXT_COLOR_DARK.0, TEXT_COLOR_DARK.1, TEXT_COLOR_DARK.2, 255),
                    right.x as f32,
                    120.0,
                ),
                text_brush(
                    request.options.text_colors.stats_shadow.as_ref(),
                    None,
                    Color::TRANSPARENT,
                    right.x as f32,
                    120.0,
                ),
            ),
        );
    }
}

fn draw_stat_separator(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    base: &BaseLayout,
    language: Option<&str>,
) {
    let asset_name = if request.card.is_link() {
        bundle
            .layout
            .resource_rules
            .atk_link_asset
            .get(if language == Some("astral") {
                "astral"
            } else {
                "default"
            })
    } else {
        bundle
            .layout
            .resource_rules
            .atk_def_asset
            .get(if language == Some("astral") {
                "astral"
            } else {
                "default"
            })
    };

    if let Some(asset_name) = asset_name {
        let _ = bundle.draw_image_at(
            target,
            asset_name,
            base.atk_def_link.background.x as f32,
            base.atk_def_link.background.y as f32,
        );
    }
}

/// Resolve a [`NameColor`] to a concrete `tiny_skia` [`Color`].
///
/// - `Auto` defers to `auto_name_light` which checks card type flags.
/// - `Dark` / `Light` force the standard palette values.
/// - `Custom` parses a CSS hex string (`#rrggbb` or `#rgb`); falls back to
///   `Auto` if parsing fails.
fn resolve_name_color(name_color: &NameColor, card: &CardDataEntry) -> Color {
    match name_color {
        NameColor::Auto => {
            if auto_name_light(card) {
                Color::from_rgba8(
                    NAME_COLOR_LIGHT.0,
                    NAME_COLOR_LIGHT.1,
                    NAME_COLOR_LIGHT.2,
                    255,
                )
            } else {
                Color::from_rgba8(NAME_COLOR_DARK.0, NAME_COLOR_DARK.1, NAME_COLOR_DARK.2, 255)
            }
        }
        NameColor::Dark => {
            Color::from_rgba8(NAME_COLOR_DARK.0, NAME_COLOR_DARK.1, NAME_COLOR_DARK.2, 255)
        }
        NameColor::Light => Color::from_rgba8(
            NAME_COLOR_LIGHT.0,
            NAME_COLOR_LIGHT.1,
            NAME_COLOR_LIGHT.2,
            255,
        ),
        NameColor::Custom(hex) => parse_hex_color(hex).unwrap_or_else(|| {
            if auto_name_light(card) {
                Color::from_rgba8(
                    NAME_COLOR_LIGHT.0,
                    NAME_COLOR_LIGHT.1,
                    NAME_COLOR_LIGHT.2,
                    255,
                )
            } else {
                Color::from_rgba8(NAME_COLOR_DARK.0, NAME_COLOR_DARK.1, NAME_COLOR_DARK.2, 255)
            }
        }),
    }
}

#[derive(Debug, Clone)]
struct ResolvedPaint {
    color: Color,
    brush: Option<TextBrush>,
}

fn resolve_name_brush(
    request: &RenderRequest,
    fallback: Color,
    x: f32,
    width: f32,
) -> ResolvedPaint {
    let paint = request
        .options
        .text_colors
        .name
        .as_ref()
        .cloned()
        .or_else(|| {
            request
                .card
                .name_gradient
                .as_ref()
                .map(|gradient| TextPaint {
                    color: None,
                    gradient: Some(gradient.clone()),
                })
        });
    ResolvedPaint {
        color: paint_color(paint.as_ref(), None, fallback),
        brush: text_brush(paint.as_ref(), None, fallback, x, width),
    }
}

fn resolve_title_brush(
    request: &RenderRequest,
    document_paint: Option<&TextPaint>,
    fallback: Color,
    x: f32,
    width: f32,
) -> ResolvedPaint {
    if let Some(paint) = document_paint {
        return ResolvedPaint {
            color: paint_color(Some(paint), None, fallback),
            brush: text_brush(Some(paint), None, fallback, x, width),
        };
    }
    resolve_name_brush(request, fallback, x, width)
}

fn resolve_name_shadow_brush(request: &RenderRequest, x: f32, width: f32) -> ResolvedPaint {
    let paint = request
        .options
        .text_colors
        .name_shadow
        .as_ref()
        .cloned()
        .or_else(|| {
            request
                .card
                .name_shadow_gradient
                .as_ref()
                .map(|gradient| TextPaint {
                    color: request.card.name_shadow_color.clone(),
                    gradient: Some(gradient.clone()),
                })
        })
        .or_else(|| {
            request
                .card
                .name_shadow_color
                .as_ref()
                .map(TextPaint::solid)
        });

    ResolvedPaint {
        color: paint_color(paint.as_ref(), None, Color::TRANSPARENT),
        brush: text_brush(paint.as_ref(), None, Color::TRANSPARENT, x, width),
    }
}

fn resolve_title_shadow_brush(
    request: &RenderRequest,
    document_paint: Option<&TextPaint>,
    x: f32,
    width: f32,
) -> ResolvedPaint {
    if let Some(paint) = document_paint {
        return ResolvedPaint {
            color: paint_color(Some(paint), None, Color::TRANSPARENT),
            brush: text_brush(Some(paint), None, Color::TRANSPARENT, x, width),
        };
    }
    resolve_name_shadow_brush(request, x, width)
}

fn text_brush(
    paint: Option<&TextPaint>,
    legacy_color: Option<&str>,
    fallback: Color,
    x: f32,
    width: f32,
) -> Option<TextBrush> {
    let Some(paint) = paint else {
        return legacy_color.and_then(parse_hex_color).map(TextBrush::solid);
    };

    if let Some(brush) = paint
        .gradient
        .as_ref()
        .and_then(|gradient| gradient_brush(gradient, x, width))
    {
        return Some(brush);
    }

    paint
        .color
        .as_deref()
        .or(legacy_color)
        .and_then(parse_hex_color)
        .map(TextBrush::solid)
        .or_else(|| {
            if fallback.alpha() > 0.0 {
                Some(TextBrush::solid(fallback))
            } else {
                None
            }
        })
}

fn paint_color(paint: Option<&TextPaint>, legacy_color: Option<&str>, fallback: Color) -> Color {
    paint
        .and_then(|paint| paint.color.as_deref())
        .or(legacy_color)
        .and_then(parse_hex_color)
        .or_else(|| {
            paint.and_then(|paint| {
                paint
                    .gradient
                    .as_ref()
                    .and_then(|gradient| parse_hex_color(&gradient.start))
            })
        })
        .unwrap_or(fallback)
}

fn gradient_brush(gradient: &TextGradient, x: f32, width: f32) -> Option<TextBrush> {
    let start = parse_hex_color(&gradient.start)?;
    let end = parse_hex_color(&gradient.end)?;
    Some(TextBrush::horizontal_gradient(start, end, x, width))
}

/// Parse a CSS-style hex color string (`#rrggbb`, `#rrggbbaa`, `#rgb`).
/// Returns `None` if the string is not a recognised hex format.
fn parse_hex_color(s: &str) -> Option<Color> {
    let value = s.trim();
    if let Some(color) = parse_named_color(value) {
        return Some(color);
    }

    let hex = value.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some(Color::from_rgba8(r, g, b, 255))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::from_rgba8(r, g, b, 255))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Color::from_rgba8(r, g, b, a))
        }
        _ => None,
    }
}

fn parse_named_color(s: &str) -> Option<Color> {
    let (r, g, b, a) = match s.to_ascii_lowercase().as_str() {
        "black" => (0, 0, 0, 255),
        "white" => (255, 255, 255, 255),
        "silver" => (192, 192, 192, 255),
        "gold" => (255, 215, 0, 255),
        "red" => (255, 0, 0, 255),
        "blue" => (0, 0, 255, 255),
        "green" => (0, 128, 0, 255),
        "purple" => (128, 0, 128, 255),
        "transparent" => (0, 0, 0, 0),
        _ => return None,
    };
    Some(Color::from_rgba8(r, g, b, a))
}

fn draw_password(
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
) {
    let ov = &request.options.layout_overrides;
    let password_x = ov.password_x.unwrap_or(base.password.x);
    let password_y = ov.password_y.unwrap_or(base.password.y);
    draw_text_line(
        target,
        DrawTextLine::unscaled(
            &request.card.code.to_string(),
            password_x as f32,
            password_y as f32,
            base.password.font_size as f32,
            260.0,
            Color::from_rgba8(PASSWORD_COLOR.0, PASSWORD_COLOR.1, PASSWORD_COLOR.2, 255),
            Color::TRANSPARENT,
            &style.password_font_family,
            TextAlign::Left,
            language,
            0.0,
        )
        .with_brushes(
            text_brush(
                request.options.text_colors.password.as_ref(),
                None,
                Color::from_rgba8(PASSWORD_COLOR.0, PASSWORD_COLOR.1, PASSWORD_COLOR.2, 255),
                password_x as f32,
                260.0,
            ),
            text_brush(
                request.options.text_colors.password_shadow.as_ref(),
                None,
                Color::TRANSPARENT,
                password_x as f32,
                260.0,
            ),
        ),
    );

    if request.card.is_monster() {
        let copyright_right = request
            .options
            .layout_overrides
            .copyright_right
            .unwrap_or(base.copyright.right);
        let copyright_y = request
            .options
            .layout_overrides
            .copyright_y
            .unwrap_or(base.copyright.y);
        draw_text_line(
            target,
            DrawTextLine::unscaled(
                &build_scale_line(&request.card),
                (CARD_WIDTH - copyright_right) as f32,
                copyright_y as f32,
                22.0,
                320.0,
                Color::from_rgba8(PASSWORD_COLOR.0, PASSWORD_COLOR.1, PASSWORD_COLOR.2, 255),
                Color::TRANSPARENT,
                &style.base_font_family,
                TextAlign::Right,
                language,
                0.0,
            )
            .with_brushes(
                text_brush(
                    request.options.text_colors.copyright.as_ref(),
                    None,
                    Color::from_rgba8(PASSWORD_COLOR.0, PASSWORD_COLOR.1, PASSWORD_COLOR.2, 255),
                    (CARD_WIDTH - copyright_right - 320) as f32,
                    320.0,
                ),
                text_brush(
                    request.options.text_colors.copyright_shadow.as_ref(),
                    None,
                    Color::TRANSPARENT,
                    (CARD_WIDTH - copyright_right - 320) as f32,
                    320.0,
                ),
            ),
        );
    }
}

fn draw_package(
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
) {
    if let Some(package) = &request.card.package {
        let ov = &request.options.layout_overrides;
        let y = if request.card.is_pendulum() {
            ov.package_y_pendulum.unwrap_or(base.package.pendulum.y)
        } else if request.card.is_link() {
            ov.package_y_link.unwrap_or(base.package.link.y)
        } else {
            ov.package_y.unwrap_or(base.package.default.y)
        };

        // Package position is right-aligned in bundle (like copyright)
        let right = if request.card.is_pendulum() {
            base.package
                .pendulum
                .right
                .unwrap_or(base.package.pendulum.x.unwrap_or(116)) // Pendulum often uses x, fallback to 116
        } else if request.card.is_link() {
            base.package.link.right.unwrap_or(252)
        } else {
            base.package.default.right.unwrap_or(148)
        };

        let x = if request.card.is_pendulum() && base.package.pendulum.x.is_some() {
            base.package.pendulum.x.unwrap() as f32
        } else {
            (CARD_WIDTH - right) as f32
        };

        let align = if request.card.is_pendulum() && base.package.pendulum.x.is_some() {
            TextAlign::Left
        } else {
            TextAlign::Right
        };

        draw_text_line(
            target,
            DrawTextLine::unscaled(
                package,
                x,
                y as f32,
                base.package.font_size as f32,
                400.0,
                Color::BLACK,
                Color::TRANSPARENT,
                &style.password_font_family,
                align,
                language,
                0.0,
            )
            .with_brushes(
                text_brush(
                    request.options.text_colors.package.as_ref(),
                    None,
                    Color::BLACK,
                    if matches!(align, TextAlign::Right) {
                        x - 400.0
                    } else {
                        x
                    },
                    400.0,
                ),
                text_brush(
                    request.options.text_colors.package_shadow.as_ref(),
                    None,
                    Color::TRANSPARENT,
                    if matches!(align, TextAlign::Right) {
                        x - 400.0
                    } else {
                        x
                    },
                    400.0,
                ),
            ),
        );
    }
}

fn draw_copyright_text(
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
) {
    if let Some(copyright) = &request.card.copyright {
        let ov = &request.options.layout_overrides;
        let right = ov.copyright_right.unwrap_or(base.copyright.right);
        let y = ov.copyright_y.unwrap_or(base.copyright.y);

        draw_text_line(
            target,
            DrawTextLine::unscaled(
                copyright,
                (CARD_WIDTH - right) as f32,
                y as f32,
                32.0,
                500.0,
                Color::from_rgba8(PASSWORD_COLOR.0, PASSWORD_COLOR.1, PASSWORD_COLOR.2, 255),
                Color::TRANSPARENT,
                &style.base_font_family,
                TextAlign::Right,
                language,
                0.0,
            )
            .with_brushes(
                text_brush(
                    request.options.text_colors.copyright.as_ref(),
                    None,
                    Color::from_rgba8(PASSWORD_COLOR.0, PASSWORD_COLOR.1, PASSWORD_COLOR.2, 255),
                    (CARD_WIDTH - right - 500) as f32,
                    500.0,
                ),
                text_brush(
                    request.options.text_colors.copyright_shadow.as_ref(),
                    None,
                    Color::TRANSPARENT,
                    (CARD_WIDTH - right - 500) as f32,
                    500.0,
                ),
            ),
        );
    }
}

fn draw_laser(
    bundle: &AssetBundle,
    target: &mut Pixmap,
    request: &RenderRequest,
    base: &BaseLayout,
) -> Result<(), RenderError> {
    let Some(laser) = request.card.laser.as_deref().and_then(laser_asset_name) else {
        return Ok(());
    };

    if bundle.has_image(&laser) {
        bundle
            .draw_image_at(target, &laser, base.laser.x as f32, base.laser.y as f32)
            .map_err(RenderError::Backend)?;
    }

    Ok(())
}

/// Margins for the spell/trap subtype icon, looked up from the bundle's language-specific style.
struct IconMargins {
    top: f32,
    left: f32,
    right: f32,
}

fn bundle_style_icon_margins(language: Option<&str>, bundle: &AssetBundle) -> IconMargins {
    let icon = bundle
        .layout
        .styles
        .get(language.unwrap_or("sc"))
        .or_else(|| bundle.layout.styles.get("sc"))
        .and_then(|style| style.spell_trap.icon.as_ref());

    IconMargins {
        top: icon.and_then(|i| i.margin_top).unwrap_or(8.0),
        left: icon.and_then(|i| i.margin_left).unwrap_or(0.0),
        right: icon.and_then(|i| i.margin_right).unwrap_or(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        draw_frosted_foil, draw_relief_engrave, laser_asset_name, premultiply_pixmap_alpha,
        scale_pixmap, CoverageRect,
    };
    use tiny_skia::PremultipliedColorU8;

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
}
