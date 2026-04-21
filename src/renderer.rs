use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

use crate::{
    asset_bundle::{get_bundle, AssetBundle, BaseLayout},
    card_logic::{
        attribute_asset_name, auto_name_light, build_effect_line, build_scale_line,
        description_height, description_y, display_stat, frame_asset_name, image_frame,
        localized_brackets, localized_spell_trap_name, spell_trap_subtype_icon_asset, uses_rank,
    },
    constants::{CARD_HEIGHT, CARD_WIDTH},
    layout::{layout_style, LayoutStyle},
    model::{RenderError, RenderRequest},
    text::{
        draw_multiline_text, draw_text_line, draw_text_line_scaled, estimate_text_width,
        fit_single_line, fit_single_line_compressed, TextAlign,
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
        let bundle = get_bundle();
        let base = &bundle.layout.base;
        let language = request.options.language.as_deref();
        let style = layout_style(request.kind, language, &bundle.layout, &request.options.layout_overrides);

        let mut target = Pixmap::new(CARD_WIDTH, CARD_HEIGHT)
            .ok_or_else(|| RenderError::Backend("Failed to allocate Pixmap".to_string()))?;
        target.fill(Color::from_rgba8(244, 239, 231, 255));

        draw_frame(bundle, &mut target, frame_asset_name(&request.card))?;
        draw_art(bundle, &mut target, request, base)?;
        draw_mask(bundle, &mut target, request, base)?;
        draw_attribute(bundle, &mut target, request, base, language)?;
        draw_level_or_rank(bundle, &mut target, request, base)?;
        draw_link_arrows(bundle, &mut target, request, base)?;

        draw_title(&mut target, request, &style, base, language);

        if request.card.is_spell() || request.card.is_trap() {
            draw_spell_trap_line(bundle, &mut target, request, &style, base, language)?;
        } else if let Some(line) = build_effect_line(&request.card, request.kind) {
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
                &line_layout.text,
                style.effect_x as f32,
                style.effect_top as f32,
                line_layout.font_size as f32,
                line_layout.max_width as f32,
                Color::from_rgba8(17, 17, 17, 255),
                Color::TRANSPARENT,
                &style.effect_font_family,
                TextAlign::Left,
                language,
                line_layout.letter_spacing,
            );
        }

        draw_multiline_text(
            &mut target,
            &request.card.desc,
            style.description_x as f32,
            description_y(&request.card, &style) as f32,
            style.body_max_width as f32,
            description_height(&request.card, &style, base) as f32,
            &style.base_font_family,
            Color::BLACK,
            Color::TRANSPARENT,
            language,
            style.description_size,
            style.description_line_height,
            style.description_letter_spacing,
            style.description_size.saturating_sub(8),
            request.options.description_first_line_compress,
        );

        draw_stats(bundle, &mut target, request, &style, base, language);
        draw_password(&mut target, request, &style, base, language);

        target
            .encode_png()
            .map_err(|e| RenderError::PngEncode(e.to_string()))
    }
}

fn draw_frame(bundle: &AssetBundle, target: &mut Pixmap, asset_name: &str) -> Result<(), RenderError> {
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
        if let Ok(img) = image::open(art_path) {
            let rgba = img.into_rgba8();
            let w = rgba.width();
            let h = rgba.height();
            if let Some(art_pixmap) = Pixmap::from_vec(
                rgba.into_raw(),
                tiny_skia::IntSize::from_wh(w, h).unwrap(),
            ) {
                let (art_x, art_y, _w, _h) = image_frame(&request.card, base);
                target.draw_pixmap(
                    art_x as i32,
                    art_y as i32,
                    art_pixmap.as_ref(),
                    &PixmapPaint::default(),
                    Transform::default(),
                    None,
                );
            }
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
                .draw_image_at(target, &asset, base.attribute.x as f32, base.attribute.y as f32)
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

    let title_layout = if request.options.title_width_compress {
        fit_single_line_compressed(
            &request.card.name,
            language,
            style.name_size,
            &style.name_font_family,
            title_width,
            style.title_letter_spacing,
            0.6,
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

    draw_text_line_scaled(
        target,
        &title_layout.text,
        style.name_x as f32,
        style.name_top as f32,
        title_layout.font_size as f32,
        title_layout.max_width as f32,
        if auto_name_light(&request.card) {
            Color::from_rgba8(245, 245, 245, 255)
        } else {
            Color::from_rgba8(22, 18, 15, 255)
        },
        Color::TRANSPARENT,
        &style.name_font_family,
        TextAlign::Left,
        language,
        title_layout.letter_spacing,
        title_layout.scale_x,
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
    let left_text = format!("{left_bracket}{}", localized_spell_trap_name(&request.card, language));
    let right_text = right_bracket;
    let font_size = style.type_size as f32;
    let letter_spacing = style.type_letter_spacing;
    let text_color = Color::from_rgba8(29, 20, 15, 255);

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
    );

    let icon_asset = spell_trap_subtype_icon_asset(&request.card);
    let icon_margin_top = bundle_style_icon_margin_top(language, bundle);
    let icon_margin_left = bundle_style_icon_margin_left(language, bundle);
    let icon_margin_right = bundle_style_icon_margin_right(language, bundle);
    let icon_width = 72.0_f32;

    let icon_x = if icon_asset.is_some() {
        right_x - icon_margin_right - icon_width
    } else {
        right_x
    };

    if let Some(icon_asset) = icon_asset {
        bundle
            .draw_image_at(target, icon_asset, icon_x, style.type_top as f32 + icon_margin_top)
            .map_err(RenderError::Backend)?;
    }

    let left_width = estimate_text_width(
        &left_text,
        language,
        &style.type_font_family,
        font_size,
        letter_spacing,
    );
    let left_x = icon_x
        - if icon_asset.is_some() {
            icon_margin_left
        } else {
            0.0
        }
        - left_width;

    draw_text_line(
        target,
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
    );

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
        );

        if request.card.is_link() {
            let link_text = if language == Some("astral") {
                &base.atk_def_link.link.astral
            } else {
                &base.atk_def_link.link.default
            };

            draw_text_line_scaled(
                target,
                &request.card.level.max(1).to_string(),
                style.stat_link_x as f32,
                style.link_top as f32,
                style.link_size as f32,
                120.0,
                value_color,
                Color::TRANSPARENT,
                &style.link_font_family,
                TextAlign::Right,
                language,
                style.stat_letter_spacing,
                link_text.scale_x.unwrap_or(1.0),
            );
        } else {
            draw_text_line(
                target,
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
            &request.card.lscale.to_string(),
            left.x as f32,
            left.y as f32,
            if language == Some("astral") { 84.0 } else { 98.0 },
            120.0,
            Color::from_rgba8(17, 17, 17, 255),
            Color::TRANSPARENT,
            &style.stat_font_family,
            TextAlign::Center,
            language,
            if language == Some("astral") { 0.0 } else { -10.0 },
        );
        draw_text_line(
            target,
            &request.card.rscale.to_string(),
            right.x as f32,
            right.y as f32,
            if language == Some("astral") { 84.0 } else { 98.0 },
            120.0,
            Color::from_rgba8(17, 17, 17, 255),
            Color::TRANSPARENT,
            &style.stat_font_family,
            TextAlign::Center,
            language,
            if language == Some("astral") { 0.0 } else { -10.0 },
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

fn draw_password(
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &LayoutStyle,
    base: &BaseLayout,
    language: Option<&str>,
) {
    draw_text_line(
        target,
        &format!("ID {}", request.card.code),
        base.password.x as f32,
        base.password.y as f32,
        base.password.font_size as f32,
        260.0,
        Color::from_rgba8(93, 81, 70, 255),
        Color::TRANSPARENT,
        &style.password_font_family,
        TextAlign::Left,
        language,
        0.0,
    );

    if request.card.is_monster() {
        draw_text_line(
            target,
            &build_scale_line(&request.card),
            (CARD_WIDTH - base.copyright.right) as f32,
            base.copyright.y as f32,
            22.0,
            320.0,
            Color::from_rgba8(93, 81, 70, 255),
            Color::TRANSPARENT,
            &style.base_font_family,
            TextAlign::Right,
            language,
            0.0,
        );
    }
}
fn bundle_style_icon_margin_top(language: Option<&str>, bundle: &AssetBundle) -> f32 {
    bundle
        .layout
        .styles
        .get(language.unwrap_or("sc"))
        .or_else(|| bundle.layout.styles.get("sc"))
        .and_then(|style| style.spell_trap.icon.as_ref())
        .and_then(|icon| icon.margin_top)
        .unwrap_or(8.0)
}

fn bundle_style_icon_margin_left(language: Option<&str>, bundle: &AssetBundle) -> f32 {
    bundle
        .layout
        .styles
        .get(language.unwrap_or("sc"))
        .or_else(|| bundle.layout.styles.get("sc"))
        .and_then(|style| style.spell_trap.icon.as_ref())
        .and_then(|icon| icon.margin_left)
        .unwrap_or(0.0)
}

fn bundle_style_icon_margin_right(language: Option<&str>, bundle: &AssetBundle) -> f32 {
    bundle
        .layout
        .styles
        .get(language.unwrap_or("sc"))
        .or_else(|| bundle.layout.styles.get("sc"))
        .and_then(|style| style.spell_trap.icon.as_ref())
        .and_then(|icon| icon.margin_right)
        .unwrap_or(0.0)
}
