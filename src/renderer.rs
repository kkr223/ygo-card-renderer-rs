use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};
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
    layout::{LayoutStyle, layout_style},
    model::{NameColor, RenderError, RenderRequest},
    ruby::{contains_ruby_markup, parse_ruby_text, strip_ruby_markup},
    text::{
        DrawTextLine, RubyLineParams, RubyMultilineParams, TextAlign, draw_multiline_ruby_text,
        draw_ruby_text_line, draw_text_line, draw_text_line_scaled, estimate_text_width,
        fit_ruby_text_scale, fit_single_line, fit_single_line_compressed,
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

        let output_scale = effective_output_scale(request);
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

fn effective_output_scale(request: &RenderRequest) -> f32 {
    let scale = request.card.scale.unwrap_or(request.options.scale);
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
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
        if let Ok(img) = image::open(art_path) {
            let rgba = img.into_rgba8();
            let w = rgba.width();
            let h = rgba.height();
            if let Some(art_pixmap) =
                Pixmap::from_vec(rgba.into_raw(), tiny_skia::IntSize::from_wh(w, h).unwrap())
            {
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
                    Transform::from_scale(scale_x, scale_y)
                        .post_translate(art_x as f32, art_y as f32),
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
                color: name_color,
                shadow_color: Color::TRANSPARENT,
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

    draw_text_line_scaled(
        target,
        DrawTextLine {
            text: &title_layout.text,
            x: style.name_x as f32,
            y: style.name_top as f32,
            font_size: title_layout.font_size as f32,
            max_width: title_layout.max_width as f32,
            color: name_color,
            shadow_color: Color::TRANSPARENT,
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

/// Parse a CSS-style hex color string (`#rrggbb`, `#rrggbbaa`, `#rgb`).
/// Returns `None` if the string is not a recognised hex format.
fn parse_hex_color(s: &str) -> Option<Color> {
    let hex = s.trim().strip_prefix('#')?;
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
    use super::{laser_asset_name, scale_pixmap};

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
}
