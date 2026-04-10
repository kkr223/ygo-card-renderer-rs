use std::path::Path;

use resvg::{tiny_skia, usvg};

use crate::{
    assets::{
        embed_font_faces, load_font_dirs, read_data_uri, resolve_atk_def_link_data_uri,
        resolve_attribute_data_uri, resolve_background_data_uri, resolve_copyright_data_uri,
        resolve_level_rank_data_uri, resolve_mask_data_uri,
    },
    card_logic::{
        build_effect_line, build_primary_line, build_scale_line, description_height, description_y,
        display_stat, draw_level_or_rank, image_frame, mask_position,
    },
    constants::{CARD_HEIGHT, CARD_WIDTH},
    layout::layout_style,
    model::{CardKind, RenderError, RenderRequest},
    text::{escape_xml, fit_single_line, render_multiline_text, render_single_line_text},
};

pub fn render_svg(request: &RenderRequest) -> Result<String, RenderError> {
    let scale = normalize_scale(request.options.scale);
    let background_href = resolve_background_data_uri(request)?;
    let mask_href = resolve_mask_data_uri(request)?;
    let art_href = request
        .options
        .art_image
        .as_ref()
        .and_then(|path| read_data_uri(path).ok());
    let attribute_href = resolve_attribute_data_uri(request)?;
    let level_star_href = resolve_level_rank_data_uri(request)?;
    let atk_def_link_href = resolve_atk_def_link_data_uri(request)?;
    let copyright_href = resolve_copyright_data_uri(request)?;

    let title_color =
        request
            .options
            .name_color_override
            .as_deref()
            .unwrap_or(match request.kind {
                CardKind::Yugioh => "#16120f",
                CardKind::RushDuel => "#101010",
            });
    let desc_color = request
        .options
        .description_color_override
        .as_deref()
        .unwrap_or("#111111");

    let style = layout_style(request.kind, request.options.language.as_deref());
    let primary_line = build_primary_line(&request.card, request.kind);
    let effect_line = build_effect_line(&request.card, request.kind);
    let desc = if request.card.desc.trim().is_empty() {
        " ".to_string()
    } else {
        request.card.desc.clone()
    };
    let title_y = style.name_top;
    let image_frame = image_frame(&request.card);
    let mask_position = mask_position(&request.card);
    let description_y = description_y(&request.card, &style);
    let description_height = description_height(&request.card, &style);
    let show_attribute = attribute_href.is_some();
    let title_layout = fit_single_line(
        &request.card.name,
        request.options.language.as_deref(),
        style.name_size,
        if show_attribute {
            style.title_max_width_with_attribute
        } else {
            style.title_max_width_without_attribute
        },
        style.title_letter_spacing,
        style.name_size.saturating_sub(26),
    );

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
        scaled_size(CARD_WIDTH, scale),
        scaled_size(CARD_HEIGHT, scale),
        CARD_WIDTH,
        CARD_HEIGHT
    ));
    svg.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#f4efe7\"/>");
    svg.push_str(&embed_font_faces(
        request.options.resource_path.as_path(),
        request.kind,
    )?);

    if let Some(href) = background_href {
        svg.push_str(&format!(
            "<image x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" href=\"{}\" preserveAspectRatio=\"none\"/>",
            CARD_WIDTH, CARD_HEIGHT, href
        ));
    }

    svg.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"12\" fill=\"#e8dcc5\" opacity=\"0.95\"/>",
        image_frame.0, image_frame.1, image_frame.2, image_frame.3
    ));
    if let Some(href) = art_href {
        svg.push_str(&format!(
            "<clipPath id=\"art-clip\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\"/></clipPath>",
            image_frame.0, image_frame.1, image_frame.2, image_frame.3
        ));
        svg.push_str(&format!(
            "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" href=\"{}\" preserveAspectRatio=\"xMidYMid slice\" clip-path=\"url(#art-clip)\"/>",
            image_frame.0, image_frame.1, image_frame.2, image_frame.3, href
        ));
    }

    if let Some(href) = mask_href {
        svg.push_str(&format!(
            "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" href=\"{}\" preserveAspectRatio=\"none\"/>",
            mask_position.0, mask_position.1, mask_position.2, mask_position.3, href
        ));
    }

    svg.push_str(&render_single_line_text(
        style.name_x,
        title_y,
        style.name_font_family,
        title_color,
        &request.card.name,
        request.options.language.as_deref(),
        title_layout.font_size,
        title_layout.max_width,
        title_layout.letter_spacing,
    ));

    if let Some(href) = attribute_href {
        svg.push_str(&format!(
            "<image x=\"1163\" y=\"96\" width=\"120\" height=\"120\" href=\"{}\" preserveAspectRatio=\"xMidYMid meet\"/>",
            href
        ));
    }

    draw_level_or_rank(
        &mut svg,
        &request.card,
        level_star_href.as_deref(),
        request.kind,
    );

    if request.card.is_spell() || request.card.is_trap() {
        let type_layout = fit_single_line(
            &primary_line,
            request.options.language.as_deref(),
            style.type_size,
            1175,
            style.type_letter_spacing,
            style.type_size.saturating_sub(14),
        );
        svg.push_str(&render_single_line_text(
            116,
            style.type_top,
            style.type_font_family,
            "#1d140f",
            &primary_line,
            request.options.language.as_deref(),
            type_layout.font_size,
            type_layout.max_width,
            type_layout.letter_spacing,
        ));
    }

    if request.card.is_pendulum() {
        svg.push_str(&format!(
            "<text x=\"145\" y=\"1370\" text-anchor=\"middle\" dominant-baseline=\"text-before-edge\" font-size=\"98\" font-family=\"{}\" fill=\"#111\" letter-spacing=\"-10\">{}</text>",
            style.stat_font_family,
            request.card.lscale
        ));
        svg.push_str(&format!(
            "<text x=\"1249\" y=\"1370\" text-anchor=\"middle\" dominant-baseline=\"text-before-edge\" font-size=\"98\" font-family=\"{}\" fill=\"#111\" letter-spacing=\"-10\">{}</text>",
            style.stat_font_family,
            request.card.rscale
        ));
        svg.push_str(&render_multiline_text(
            221,
            style.pendulum_description_top,
            950,
            230,
            style.base_font_family,
            &desc_color,
            &desc,
            request.options.language.as_deref(),
            style.pendulum_description_size,
            style.description_line_height,
            style.description_letter_spacing,
            style.pendulum_description_size.saturating_sub(6),
        ));
    }

    if let Some(effect_line) = effect_line {
        let effect_layout = fit_single_line(
            &effect_line,
            request.options.language.as_deref(),
            style.effect_size,
            style.body_max_width,
            style.effect_letter_spacing,
            style.effect_size.saturating_sub(10),
        );
        svg.push_str(&render_single_line_text(
            style.effect_x,
            style.effect_top,
            style.effect_font_family,
            "#111",
            &effect_line,
            request.options.language.as_deref(),
            effect_layout.font_size,
            effect_layout.max_width,
            effect_layout.letter_spacing,
        ));
    }

    svg.push_str(&render_multiline_text(
        style.effect_x,
        description_y,
        style.body_max_width,
        description_height,
        style.base_font_family,
        &desc_color,
        &desc,
        request.options.language.as_deref(),
        style.description_size,
        style.description_line_height,
        style.description_letter_spacing,
        style.description_size.saturating_sub(8),
    ));

    if let Some(href) = atk_def_link_href {
        svg.push_str(&format!(
            "<image x=\"109\" y=\"1844\" width=\"1175\" height=\"64\" href=\"{}\" preserveAspectRatio=\"none\"/>",
            href
        ));
    }

    svg.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" dominant-baseline=\"text-before-edge\" font-size=\"{}\" font-family=\"{}\" fill=\"#111\" letter-spacing=\"{}\">{}</text>",
        style.stat_atk_x,
        style.stat_top,
        style.stat_size,
        style.stat_font_family,
        style.stat_letter_spacing,
        escape_xml(&display_stat(request.card.attack))
    ));
    if request.card.is_link() {
        draw_link_arrows(&mut svg, request, request.options.resource_path.as_path())?;
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" dominant-baseline=\"text-before-edge\" font-size=\"{}\" font-family=\"{}\" fill=\"#111\" letter-spacing=\"{}\">{}</text>",
            style.stat_link_x,
            style.link_top,
            style.link_size,
            style.link_font_family,
            style.stat_letter_spacing,
            request.card.level.max(1)
        ));
    } else {
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" dominant-baseline=\"text-before-edge\" font-size=\"{}\" font-family=\"{}\" fill=\"#111\" letter-spacing=\"{}\">{}</text>",
            style.stat_def_x,
            style.stat_top,
            style.stat_size,
            style.stat_font_family,
            style.stat_letter_spacing,
            escape_xml(&display_stat(request.card.defense))
        ));
    }

    svg.push_str(&format!(
        "<text x=\"66\" y=\"1932\" dominant-baseline=\"text-before-edge\" font-size=\"28\" font-family=\"{}\" fill=\"#5d5146\">ID {}</text>",
        style.password_font_family,
        request.card.code
    ));

    if let Some(href) = copyright_href {
        svg.push_str(&format!(
            "<image x=\"1150\" y=\"1936\" width=\"120\" height=\"24\" href=\"{}\" preserveAspectRatio=\"xMaxYMid meet\"/>",
            href
        ));
    } else if show_attribute {
        svg.push_str(&format!(
            "<text x=\"1284\" y=\"1936\" text-anchor=\"end\" dominant-baseline=\"text-before-edge\" font-size=\"22\" font-family=\"{}\" fill=\"#5d5146\">{}</text>",
            style.base_font_family,
            escape_xml(&build_scale_line(&request.card))
        ));
    }

    svg.push_str("</svg>");
    Ok(svg)
}

pub fn render_png(request: &RenderRequest) -> Result<Vec<u8>, RenderError> {
    let svg = render_svg(request)?;
    let mut opt = usvg::Options {
        resources_dir: Some(request.options.resource_path.clone()),
        ..usvg::Options::default()
    };
    load_font_dirs(
        &mut opt,
        request.options.resource_path.as_path(),
        request.kind,
    );
    let tree =
        usvg::Tree::from_str(&svg, &opt).map_err(|err| RenderError::SvgParse(err.to_string()))?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| RenderError::PngEncode("failed to allocate pixmap".to_string()))?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    pixmap
        .encode_png()
        .map_err(|err| RenderError::PngEncode(err.to_string()))
}

fn normalize_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

fn scaled_size(value: u32, scale: f32) -> u32 {
    ((value as f32) * scale).round().max(1.0) as u32
}

fn draw_link_arrows(
    svg: &mut String,
    request: &RenderRequest,
    resource_root: &Path,
) -> Result<(), RenderError> {
    let arrows = [
        (0x080_u32, 555_u32, 278_u32, "arrow-up"),
        (0x100_u32, 1130, 299, "arrow-right-up"),
        (0x020_u32, 1223, 761, "arrow-right"),
        (0x004_u32, 1130, 1336, "arrow-right-down"),
        (0x002_u32, 555, 1428, "arrow-down"),
        (0x001_u32, 95, 1336, "arrow-left-down"),
        (0x008_u32, 71, 758, "arrow-left"),
        (0x040_u32, 95, 299, "arrow-left-up"),
    ];
    for (bit, x, y, name) in arrows {
        let suffix = if (request.card.link_marker & bit) != 0 {
            "on"
        } else {
            "off"
        };
        let path = resource_root
            .join("yugioh")
            .join("image")
            .join(format!("{name}-{suffix}.png"));
        if path.exists() {
            let href = read_data_uri(&path)?;
            svg.push_str(&format!(
                "<image x=\"{}\" y=\"{}\" width=\"96\" height=\"96\" href=\"{}\" preserveAspectRatio=\"xMidYMid meet\"/>",
                x, y, href
            ));
        }
    }
    Ok(())
}
