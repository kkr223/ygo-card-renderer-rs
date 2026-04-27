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
    document::{ImageAlign, ImageFit, RenderDocument, RenderOp, RenderRect, TextChannel},
    layout::{LayoutStyle, layout_style},
    model::{
        NameColor, OutFrameEffectBox, PositionedRenderImage, RenderError, RenderRequest,
        TextGradient, TextPaint,
    },
    rare_effect::draw_rare_effect,
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
                RenderOp::RareEffect { rare } => {
                    draw_rare_effect(&mut target, *rare, &request.card, base);
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
                RenderOp::LinkArrows => {
                    draw_link_arrows(bundle, &mut target, &request, base)?;
                }
                RenderOp::Title { .. } => {
                    draw_title(&mut target, &request, &style, base, language);
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
                    draw_stats(bundle, &mut target, &request, &style, base, language);
                }
                RenderOp::Password { .. } => {
                    draw_password(&mut target, &request, &style, base, language);
                }
                RenderOp::Package { text } => {
                    let mut request = request.clone();
                    request.card.package = Some(text.clone());
                    draw_package(&mut target, &request, &style, base, language);
                }
                RenderOp::Copyright { text } => {
                    let mut request = request.clone();
                    request.card.copyright = Some(text.clone());
                    draw_copyright_text(&mut target, &request, &style, base, language);
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
            let sections = split_pendulum_description(&request.card.desc);
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
    let dx = rect.x + (rect.width - drawn_w) / 2.0;
    let dy = match align {
        ImageAlign::Top => rect.y,
        ImageAlign::Center => rect.y + (rect.height - drawn_h) / 2.0,
    };

    target.draw_pixmap(
        0,
        0,
        pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::from_scale(scale_x, scale_y).post_translate(dx, dy),
        None,
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
    use super::{laser_asset_name, premultiply_pixmap_alpha, scale_pixmap};

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
}
