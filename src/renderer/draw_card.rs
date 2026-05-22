use image::{ImageReader, Limits};
use tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::{
    document::{ImageAlign, ImageFit, RenderRect},
    model::{ImageCrop, PositionedRenderImage},
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

    let source_w = pixmap.width() as f32;
    let source_h = pixmap.height() as f32;
    if source_w <= 0.0 || source_h <= 0.0 {
        return;
    }
    let (scale_x, scale_y) = image_scale(source_w, source_h, image);
    let transform = positioned_image_transform(
        image.x as f32,
        image.y as f32,
        source_w,
        source_h,
        scale_x,
        scale_y,
        image.rotation.unwrap_or(0.0),
    );

    target.draw_pixmap(
        0,
        0,
        pixmap.as_ref(),
        &PixmapPaint::default(),
        transform,
        None,
    );
}

pub(super) fn draw_external_image(
    target: &mut Pixmap,
    path: Option<&std::path::Path>,
    rect: &RenderRect,
    fit: ImageFit,
    align: ImageAlign,
    crop: Option<ImageCrop>,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
) {
    let Some(path) = path else {
        return;
    };
    let Some(rect) = sanitize_render_rect(rect) else {
        return;
    };
    let Some(source_pixmap) = load_external_pixmap(path) else {
        return;
    };
    let pixmap = cropped_pixmap(source_pixmap, crop);

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

    let sanitized_scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
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
    let scale_x = scale_x * sanitized_scale;
    let scale_y = scale_y * sanitized_scale;

    let drawn_w = source_w * scale_x;
    let drawn_h = source_h * scale_y;
    let (dx, dy) = aligned_image_offset(rect.width, rect.height, drawn_w, drawn_h, align);
    let dx = dx + finite_or_zero(offset_x);
    let dy = dy + finite_or_zero(offset_y);

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

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn cropped_pixmap(pixmap: Pixmap, crop: Option<ImageCrop>) -> Pixmap {
    let Some(crop) = crop else {
        return pixmap;
    };
    if !crop.x.is_finite()
        || !crop.y.is_finite()
        || !crop.width.is_finite()
        || !crop.height.is_finite()
        || crop.width <= 0.0
        || crop.height <= 0.0
    {
        return pixmap;
    }
    let x = crop.x.round().max(0.0) as i32;
    let y = crop.y.round().max(0.0) as i32;
    let max_w = pixmap.width().saturating_sub(x.max(0) as u32);
    let max_h = pixmap.height().saturating_sub(y.max(0) as u32);
    let w = (crop.width.round().max(1.0) as u32).min(max_w);
    let h = (crop.height.round().max(1.0) as u32).min(max_h);
    if w == 0 || h == 0 {
        return pixmap;
    }
    let Some(rect) = tiny_skia::IntRect::from_xywh(x, y, w, h) else {
        return pixmap;
    };
    pixmap.clone_rect(rect).unwrap_or(pixmap)
}

fn aligned_image_offset(
    target_w: f32,
    target_h: f32,
    drawn_w: f32,
    drawn_h: f32,
    align: ImageAlign,
) -> (f32, f32) {
    let left = 0.0;
    let center_x = (target_w - drawn_w) / 2.0;
    let right = target_w - drawn_w;
    let top = 0.0;
    let center_y = (target_h - drawn_h) / 2.0;
    let bottom = target_h - drawn_h;
    match align {
        ImageAlign::TopLeft => (left, top),
        ImageAlign::Top => (center_x, top),
        ImageAlign::TopRight => (right, top),
        ImageAlign::Left => (left, center_y),
        ImageAlign::Center => (center_x, center_y),
        ImageAlign::Right => (right, center_y),
        ImageAlign::BottomLeft => (left, bottom),
        ImageAlign::Bottom => (center_x, bottom),
        ImageAlign::BottomRight => (right, bottom),
    }
}

fn image_scale(source_w: f32, source_h: f32, image: &PositionedRenderImage) -> (f32, f32) {
    let base = image
        .scale
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0);
    let scale_x = image
        .width
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|w| w / source_w)
        .or_else(|| image.scale_x.filter(|v| v.is_finite() && *v > 0.0))
        .unwrap_or(base);
    let scale_y = image
        .height
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|h| h / source_h)
        .or_else(|| image.scale_y.filter(|v| v.is_finite() && *v > 0.0))
        .unwrap_or(base);
    (scale_x, scale_y)
}

fn positioned_image_transform(
    x: f32,
    y: f32,
    source_w: f32,
    source_h: f32,
    scale_x: f32,
    scale_y: f32,
    rotation: f32,
) -> Transform {
    let drawn_w = source_w * scale_x;
    let drawn_h = source_h * scale_y;
    if !rotation.is_finite() || rotation.abs() <= f32::EPSILON {
        return Transform::from_scale(scale_x, scale_y).post_translate(x, y);
    }
    Transform::from_scale(scale_x, scale_y)
        .post_translate(x, y)
        .post_rotate_at(rotation, x + drawn_w / 2.0, y + drawn_h / 2.0)
}

pub(super) fn text_align_choice(align: crate::model::TextAlignChoice) -> TextAlign {
    use crate::model::TextAlignChoice;
    match align {
        TextAlignChoice::Left => TextAlign::Left,
        TextAlignChoice::Center => TextAlign::Center,
        TextAlignChoice::Right => TextAlign::Right,
        TextAlignChoice::Justify => TextAlign::Justify,
    }
}
