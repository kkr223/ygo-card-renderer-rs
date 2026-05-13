use image::{ImageReader, Limits};
use tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::{
    document::{ImageAlign, ImageFit, RenderRect},
    model::PositionedRenderImage,
    text::TextAlign,
};

const MAX_EXTERNAL_DECODED_PIXELS: u64 = 4096 * 4096;
const MAX_DOCUMENT_RECT_PIXELS: u64 = 4096 * 4096;

pub(super) fn load_external_pixmap(path: &std::path::Path) -> Option<Pixmap> {
    let dimensions_reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let (declared_width, declared_height) = dimensions_reader.into_dimensions().ok()?;
    validate_external_decode_size(declared_width, declared_height).ok()?;

    let mut reader = ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(declared_width);
    limits.max_image_height = Some(declared_height);
    limits.max_alloc = Some(MAX_EXTERNAL_DECODED_PIXELS * 4);
    reader.limits(limits);

    let img = reader.decode().ok()?;
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    validate_external_decode_size(width, height).ok()?;
    let size = tiny_skia::IntSize::from_wh(width, height)?;
    let mut pixmap = Pixmap::from_vec(rgba.into_raw(), size)?;
    premultiply_pixmap_alpha(&mut pixmap);
    Some(pixmap)
}

fn validate_external_decode_size(width: u32, height: u32) -> Result<(), ()> {
    let pixels = width as u64 * height as u64;
    if pixels == 0 || pixels > MAX_EXTERNAL_DECODED_PIXELS {
        return Err(());
    }
    Ok(())
}

pub(super) fn sanitize_render_rect(rect: &RenderRect) -> Option<RenderRect> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
        || rect.width <= 0.0
        || rect.height <= 0.0
    {
        return None;
    }

    let width = rect.width.round().max(1.0);
    let height = rect.height.round().max(1.0);
    if width as u64 * height as u64 > MAX_DOCUMENT_RECT_PIXELS {
        return None;
    }

    if rect.x < i32::MIN as f32
        || rect.x > i32::MAX as f32
        || rect.y < i32::MIN as f32
        || rect.y > i32::MAX as f32
    {
        return None;
    }

    Some(RenderRect {
        x: rect.x,
        y: rect.y,
        width,
        height,
    })
}

pub(super) fn premultiply_pixmap_alpha(pixmap: &mut Pixmap) {
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

pub(super) fn draw_positioned_render_image(target: &mut Pixmap, image: &PositionedRenderImage) {
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

pub(super) fn draw_external_image(
    target: &mut Pixmap,
    path: Option<&std::path::Path>,
    rect: &RenderRect,
    fit: ImageFit,
    align: ImageAlign,
) {
    let Some(path) = path else {
        return;
    };
    let Some(rect) = sanitize_render_rect(rect) else {
        return;
    };
    let Some(pixmap) = load_external_pixmap(path) else {
        return;
    };

    let source_w = pixmap.width() as f32;
    let source_h = pixmap.height() as f32;
    if source_w <= 0.0 || source_h <= 0.0 {
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

pub(super) fn text_align_choice(align: crate::model::TextAlignChoice) -> TextAlign {
    use crate::model::TextAlignChoice;
    match align {
        TextAlignChoice::Left => TextAlign::Left,
        TextAlignChoice::Center => TextAlign::Center,
        TextAlignChoice::Right => TextAlign::Right,
    }
}
