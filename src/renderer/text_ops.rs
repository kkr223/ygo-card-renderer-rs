use tiny_skia::{Color, Pixmap};

use crate::{
    document::{RenderRect, RubyStyle},
    model::FontWeight,
    text::{
        DrawTextLine, RubyLineParams, RubyMultilineParams, draw_multiline_ruby_text,
        draw_ruby_text_line, draw_text_line, fit_ruby_text_scale, fit_single_line,
        fit_single_line_compressed,
    },
};

use super::{
    color::text_brush_in_box,
    draw_card::{sanitize_render_rect, text_align_choice},
};

pub(super) const TEXT_OUTLINE_OFFSETS: [(f32, f32); 8] = [
    (-1.0, 0.0),
    (1.0, 0.0),
    (0.0, -1.0),
    (0.0, 1.0),
    (-1.0, -1.0),
    (1.0, -1.0),
    (-1.0, 1.0),
    (1.0, 1.0),
];

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_text_line_op(
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
    let brush = make_text_brush(fill, rect);
    let shadow_brush = shadow.and_then(|s| make_shadow_brush(s, rect));
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
            if shadow_brush.is_some() {
                draw_ruby_outline(
                    target,
                    &tokens,
                    rect,
                    language,
                    font_family,
                    font_size,
                    letter_spacing,
                    ruby,
                    scale_x,
                    shadow_brush.as_ref(),
                    font_weight,
                );
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
        draw_text_outline(
            target,
            &title_layout.text,
            rect,
            language,
            font_family,
            title_layout.font_size as f32,
            title_layout.max_width as f32,
            title_layout.letter_spacing,
            title_layout.scale_x,
            text_align,
            shadow_brush.as_ref(),
            font_weight,
        );
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

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_text_block_op(
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
    let brush = make_text_brush(fill, rect);
    let shadow_brush = shadow.and_then(|s| make_shadow_brush(s, rect));
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

fn make_text_brush(
    fill: &crate::model::TextPaint,
    rect: RenderRect,
) -> Option<crate::text::TextBrush> {
    text_brush_in_box(
        Some(fill),
        None,
        Color::BLACK,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    )
}

fn make_shadow_brush(
    shadow: &crate::model::TextPaint,
    rect: RenderRect,
) -> Option<crate::text::TextBrush> {
    text_brush_in_box(
        Some(shadow),
        None,
        Color::TRANSPARENT,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    )
}

fn draw_text_outline(
    target: &mut Pixmap,
    text: &str,
    rect: RenderRect,
    language: Option<&str>,
    font_family: &str,
    font_size: f32,
    max_width: f32,
    letter_spacing: f32,
    scale_x: f32,
    align: crate::text::TextAlign,
    shadow_brush: Option<&crate::text::TextBrush>,
    font_weight: Option<FontWeight>,
) {
    if let Some(shadow_brush) = shadow_brush {
        for (dx, dy) in TEXT_OUTLINE_OFFSETS {
            draw_text_line(
                target,
                DrawTextLine {
                    text,
                    x: rect.x + dx,
                    y: rect.y + dy,
                    font_size,
                    max_width,
                    color: Color::TRANSPARENT,
                    shadow_color: Color::TRANSPARENT,
                    brush: Some(shadow_brush.clone()),
                    shadow_brush: None,
                    family_name: font_family,
                    align,
                    language,
                    letter_spacing,
                    scale_x,
                    font_weight,
                },
            );
        }
    }
}

fn draw_ruby_outline(
    target: &mut Pixmap,
    tokens: &[crate::ruby::RubyToken],
    rect: RenderRect,
    language: Option<&str>,
    font_family: &str,
    font_size: u32,
    letter_spacing: f32,
    ruby: &RubyStyle,
    scale_x: f32,
    shadow_brush: Option<&crate::text::TextBrush>,
    font_weight: Option<FontWeight>,
) {
    if let Some(shadow_brush) = shadow_brush {
        for (dx, dy) in TEXT_OUTLINE_OFFSETS {
            draw_ruby_text_line(
                target,
                RubyLineParams {
                    tokens,
                    x: rect.x + dx,
                    y: rect.y + dy,
                    font_size: font_size as f32,
                    rt_font_size: ruby.rt_font_size,
                    rt_top: ruby.rt_top,
                    rt_font_scale_x_override: ruby.rt_font_scale_x,
                    color: Color::TRANSPARENT,
                    shadow_color: Color::TRANSPARENT,
                    brush: Some(shadow_brush.clone()),
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
}
