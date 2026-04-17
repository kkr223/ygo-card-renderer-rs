use tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::{
    asset_bundle::get_bundle,
    card_logic::{
        build_effect_line, build_primary_line, build_scale_line, description_height, description_y,
        display_stat, get_frame_name, uses_rank,
    },
    constants::{CARD_HEIGHT, CARD_WIDTH},
    layout::layout_style,
    model::{RenderError, RenderRequest},
    text::{draw_multiline_text, draw_text_line, estimate_text_width, fit_single_line, TextAlign},
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
        let mut target = Pixmap::new(CARD_WIDTH, CARD_HEIGHT)
            .ok_or_else(|| RenderError::Backend("Failed to allocate Pixmap".to_string()))?;
        let language = request.options.language.as_deref();
        let style = layout_style(
            request.kind,
            language,
            Some(&bundle.index.text_layout),
            &request.options.layout_overrides,
        );

        // 1. Fill base background color
        target.fill(tiny_skia::Color::from_rgba8(244, 239, 231, 255));

        let frame_name = get_frame_name(&request.card);
        let frames_cat = if request.card.is_pendulum() {
            &bundle.index.frames.pendulum
        } else {
            &bundle.index.frames.normal
        };

        // 2. Load and draw the frame
        if let Some(frame_meta) = frames_cat.get(frame_name) {
            let pixmap = bundle
                .decode_frame(&frame_meta.buffer)
                .map_err(|e| RenderError::Backend(e))?;
            target.draw_pixmap(
                frame_meta.x as i32,
                frame_meta.y as i32,
                pixmap.as_ref(),
                &PixmapPaint::default(),
                Transform::default(),
                None,
            );
        } else {
            // fallback to normal
            if let Some(frame_meta) = bundle.index.frames.normal.get("通常") {
                let pixmap = bundle
                    .decode_frame(&frame_meta.buffer)
                    .map_err(|e| RenderError::Backend(e))?;
                target.draw_pixmap(
                    frame_meta.x as i32,
                    frame_meta.y as i32,
                    pixmap.as_ref(),
                    &PixmapPaint::default(),
                    Transform::default(),
                    None,
                );
            }
        }

        // 3. Draw Attribute
        let attr_key = get_attribute_key(request.card.attribute);
        if let Some(key) = attr_key {
            bundle.draw_sprite(&mut target, key, 0.0, 0.0);
        } else if request.card.is_spell() {
            bundle.draw_sprite(&mut target, "common/spell", 0.0, 0.0);
        } else if request.card.is_trap() {
            bundle.draw_sprite(&mut target, "common/trap", 0.0, 0.0);
        }

        // 4. Draw Stars (Level / Rank)
        let star_count = request.card.level.min(13);
        if star_count > 0 && !request.card.is_link() {
            let (is_rank, positions) = if uses_rank(&request.card) {
                (true, bundle.index.meta.star_rank_pos.as_ref())
            } else {
                (false, bundle.index.meta.star_level_pos.as_ref())
            };

            if let Some(pos_map) = positions {
                let star_type = if is_rank {
                    "common/star_rank"
                } else {
                    "common/star_level"
                };
                // Using 1, 2, 3 as keys in the JSON for star positions
                for i in 1..=star_count {
                    if let Some(pos) = pos_map.get(&i.to_string()) {
                        bundle.draw_sprite_at(&mut target, star_type, pos.x, pos.y);
                    }
                }
            }
        }

        // 5. Draw Link Arrows
        if request.card.is_link() {
            draw_link_arrows(bundle, &mut target, request.card.link_marker);
        }

        // 6. Draw Card Art
        if let Some(art_path) = &request.options.art_image {
            match image::open(art_path) {
                Ok(img) => {
                    let rgba = img.into_rgba8();
                    let w = rgba.width();
                    let h = rgba.height();
                    if let Some(art_pixmap) = Pixmap::from_vec(
                        rgba.into_raw(),
                        tiny_skia::IntSize::from_wh(w, h).unwrap(),
                    ) {
                        // 因为外部已经裁剪好，我们直接取其卡框预设定的左上角锚点。
                        let (art_x, art_y, _w, _h) = crate::card_logic::image_frame(&request.card);
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
                Err(e) => {
                    // Fail silently or log error depending on convention.
                    // For now, continue without art.
                    eprintln!("Warning: could not open art image: {}", e);
                }
            }
        }

        // 7. Text Overlay (Name, type/effect line, description, stats)
        let show_attribute =
            request.card.attribute != 0 || request.card.is_spell() || request.card.is_trap();
        let title_layout = fit_single_line(
            &request.card.name,
            language,
            style.name_size,
            style.name_font_family,
            if show_attribute {
                style.title_max_width_with_attribute
            } else {
                style.title_max_width_without_attribute
            },
            style.title_letter_spacing,
            style.name_size.saturating_sub(26),
        );

        draw_text_line(
            &mut target,
            &request.card.name,
            style.name_x as f32,
            style.name_top as f32,
            title_layout.font_size as f32,
            title_layout.max_width as f32,
            auto_name_color(request),
            tiny_skia::Color::TRANSPARENT,
            style.name_font_family,
            TextAlign::Left,
            language,
            title_layout.letter_spacing,
        );

        if request.card.is_spell() || request.card.is_trap() {
            draw_spell_trap_line(&mut target, request, &style, language);
        } else if let Some(line) = build_effect_line(&request.card, request.kind) {
            let line_layout = fit_single_line(
                &line,
                language,
                style.effect_size,
                style.effect_font_family,
                style.body_max_width,
                style.effect_letter_spacing,
                style.effect_size.saturating_sub(10),
            );
            draw_text_line(
                &mut target,
                &line,
                style.effect_x as f32,
                style.effect_top as f32,
                line_layout.font_size as f32,
                line_layout.max_width as f32,
                tiny_skia::Color::from_rgba8(17, 17, 17, 255),
                tiny_skia::Color::TRANSPARENT,
                style.effect_font_family,
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
            description_height(&request.card, &style) as f32,
            style.base_font_family,
            tiny_skia::Color::BLACK,
            tiny_skia::Color::TRANSPARENT,
            language,
            style.description_size,
            style.description_line_height,
            style.description_letter_spacing,
            style.description_size.saturating_sub(8),
        );

        if request.card.is_monster() {
            draw_text_line(
                &mut target,
                &display_stat(request.card.attack),
                style.stat_atk_x as f32,
                style.stat_top as f32,
                style.stat_size as f32,
                220.0,
                tiny_skia::Color::from_rgba8(17, 17, 17, 255),
                tiny_skia::Color::TRANSPARENT,
                style.stat_font_family,
                TextAlign::Right,
                language,
                style.stat_letter_spacing,
            );

            if request.card.is_link() {
                draw_text_line(
                    &mut target,
                    &request.card.level.max(1).to_string(),
                    style.stat_link_x as f32,
                    style.link_top as f32,
                    style.link_size as f32,
                    120.0,
                    tiny_skia::Color::from_rgba8(17, 17, 17, 255),
                    tiny_skia::Color::TRANSPARENT,
                    style.link_font_family,
                    TextAlign::Right,
                    language,
                    style.stat_letter_spacing,
                );
            } else {
                draw_text_line(
                    &mut target,
                    &display_stat(request.card.defense),
                    style.stat_def_x as f32,
                    style.stat_top as f32,
                    style.stat_size as f32,
                    220.0,
                    tiny_skia::Color::from_rgba8(17, 17, 17, 255),
                    tiny_skia::Color::TRANSPARENT,
                    style.stat_font_family,
                    TextAlign::Right,
                    language,
                    style.stat_letter_spacing,
                );
            }
        }

        if request.card.is_pendulum() {
            draw_text_line(
                &mut target,
                &request.card.lscale.to_string(),
                145.0,
                1370.0,
                98.0,
                120.0,
                tiny_skia::Color::from_rgba8(17, 17, 17, 255),
                tiny_skia::Color::TRANSPARENT,
                style.stat_font_family,
                TextAlign::Center,
                language,
                -10.0,
            );
            draw_text_line(
                &mut target,
                &request.card.rscale.to_string(),
                1249.0,
                1370.0,
                98.0,
                120.0,
                tiny_skia::Color::from_rgba8(17, 17, 17, 255),
                tiny_skia::Color::TRANSPARENT,
                style.stat_font_family,
                TextAlign::Center,
                language,
                -10.0,
            );
        }

        draw_text_line(
            &mut target,
            &format!("ID {}", request.card.code),
            66.0,
            1932.0,
            28.0,
            260.0,
            tiny_skia::Color::from_rgba8(93, 81, 70, 255),
            tiny_skia::Color::TRANSPARENT,
            style.password_font_family,
            TextAlign::Left,
            language,
            0.0,
        );

        if request.card.is_monster() {
            draw_text_line(
                &mut target,
                &build_scale_line(&request.card),
                1284.0,
                1936.0,
                22.0,
                320.0,
                tiny_skia::Color::from_rgba8(93, 81, 70, 255),
                tiny_skia::Color::TRANSPARENT,
                style.base_font_family,
                TextAlign::Right,
                language,
                0.0,
            );
        }

        // Encode to PNG
        target
            .encode_png()
            .map_err(|e| RenderError::PngEncode(e.to_string()))
    }
}

fn auto_name_color(request: &RenderRequest) -> tiny_skia::Color {
    if request.card.is_spell() || request.card.is_trap() {
        tiny_skia::Color::from_rgba8(245, 245, 245, 255)
    } else {
        tiny_skia::Color::from_rgba8(22, 18, 15, 255)
    }
}

fn draw_spell_trap_line(
    target: &mut Pixmap,
    request: &RenderRequest,
    style: &crate::layout::LayoutStyle,
    language: Option<&str>,
) {
    let line = build_primary_line(&request.card, request.kind);
    let left_text = line.trim_end_matches('】');
    let right_text = "】";
    let font_size = style.type_size as f32;
    let letter_spacing = style.type_letter_spacing;
    let text_color = tiny_skia::Color::from_rgba8(29, 20, 15, 255);

    // 贴近 JS 参考实现：右括号贴右侧，左半段和图标再向左回推。
    let right_margin = 134.0_f32;
    let icon_width = 72.0_f32;
    let icon_margin_top = 8.0_f32;
    let icon_margin_left = 4.0_f32;
    let icon_margin_right = 0.0_f32;

    let right_width = estimate_text_width(
        right_text,
        language,
        style.type_font_family,
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
        tiny_skia::Color::TRANSPARENT,
        style.type_font_family,
        TextAlign::Left,
        language,
        letter_spacing,
    );

    let icon_key = spell_trap_subtype_icon_key(&request.card);
    let icon_x = if icon_key.is_some() {
        right_x - icon_margin_right - icon_width
    } else {
        right_x
    };

    if let Some(icon_key) = icon_key {
        get_bundle().draw_sprite_at(
            target,
            icon_key,
            icon_x,
            style.type_top as f32 + icon_margin_top,
        );
    }

    let left_width = estimate_text_width(
        left_text,
        language,
        style.type_font_family,
        font_size,
        letter_spacing,
    );
    let left_x = icon_x
        - if icon_key.is_some() {
            icon_margin_left
        } else {
            0.0
        }
        - left_width;

    draw_text_line(
        target,
        left_text,
        left_x,
        style.type_top as f32,
        font_size,
        left_width.ceil().max(80.0),
        text_color,
        tiny_skia::Color::TRANSPARENT,
        style.type_font_family,
        TextAlign::Left,
        language,
        letter_spacing,
    );
}

fn draw_link_arrows(
    bundle: &crate::asset_bundle::AssetBundle,
    target: &mut Pixmap,
    link_marker: u32,
) {
    let arrows = [
        (0x080_u32, "links/arrow_6"),
        (0x100_u32, "links/arrow_5"),
        (0x020_u32, "links/arrow_4"),
        (0x004_u32, "links/arrow_0"),
        (0x002_u32, "links/arrow_1"),
        (0x001_u32, "links/arrow_2"),
        (0x008_u32, "links/arrow_3"),
        (0x040_u32, "links/arrow_7"),
    ];

    for (bit, sprite_name) in arrows {
        if (link_marker & bit) != 0 {
            bundle.draw_sprite(target, sprite_name, 0.0, 0.0);
        }
    }
}

fn get_attribute_key(attribute: u32) -> Option<&'static str> {
    match attribute {
        0x01 => Some("common/earth"),
        0x02 => Some("common/water"),
        0x04 => Some("common/fire"),
        0x08 => Some("common/wind"),
        0x10 => Some("common/light"),
        0x20 => Some("common/dark"),
        0x40 => Some("common/divine"),
        _ => None,
    }
}

fn spell_trap_subtype_icon_key(card: &ygopro_cdb_encode_rs::CardDataEntry) -> Option<&'static str> {
    const TYPE_QUICKPLAY: u32 = 0x1_0000;
    const TYPE_CONTINUOUS: u32 = 0x2_0000;
    const TYPE_EQUIP: u32 = 0x4_0000;
    const TYPE_FIELD: u32 = 0x8_0000;
    const TYPE_COUNTER: u32 = 0x10_0000;
    const TYPE_RITUAL: u32 = 0x80;

    if (card.type_ & TYPE_QUICKPLAY) != 0 {
        Some("common/icon_quick-play")
    } else if (card.type_ & TYPE_CONTINUOUS) != 0 {
        Some("common/icon_continuous")
    } else if (card.type_ & TYPE_EQUIP) != 0 {
        Some("common/icon_equip")
    } else if (card.type_ & TYPE_FIELD) != 0 {
        Some("common/icon_field")
    } else if (card.type_ & TYPE_COUNTER) != 0 {
        Some("common/icon_counter")
    } else if card.is_spell() && (card.type_ & TYPE_RITUAL) != 0 {
        Some("common/icon_ritual")
    } else {
        None
    }
}
