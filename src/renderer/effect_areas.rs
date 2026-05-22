use std::sync::Arc;

use tiny_skia::{Pixmap, PixmapPaint, Transform};

use crate::{
    asset_bundle::{AssetBundle, BaseLayout, PositionedAsset},
    card_logic::{attribute_asset_name, image_frame, uses_rank},
    constants::{CARD_HEIGHT, CARD_WIDTH},
    document::{EffectStyle, EffectTarget, EffectTargetWeight, RenderDocument},
    model::{OutFrameEffectBox, RenderError, YgoCardMeta},
    rare_effect::CoverageRect,
};

use super::{
    draw_card::load_external_pixmap,
    visual_effects::{
        draw_concentric_engrave, draw_frosted_foil, draw_gold_wash, draw_relief_engrave,
    },
};

#[derive(Debug, Clone)]
pub(super) enum EffectArea {
    Rect(CoverageRect),
    MaskedRect {
        rect: CoverageRect,
        mask: Arc<Pixmap>,
    },
}

#[derive(Debug, Clone)]
pub(super) struct EffectProtectionMask {
    x: i32,
    y: i32,
    pixmap: Pixmap,
}

#[derive(Debug, Clone)]
pub(super) struct EffectRectSnapshot {
    rect: CoverageRect,
    x_start: u32,
    y_start: u32,
    x_end: u32,
    y_end: u32,
    sub_w: usize,
    pixels: Vec<tiny_skia::PremultipliedColorU8>,
}

pub(super) fn load_effect_protection_mask(
    document: &RenderDocument,
    base: &BaseLayout,
) -> Result<Option<EffectProtectionMask>, RenderError> {
    let Some(mask) = document.options.effect_mask.as_ref() else {
        return Ok(None);
    };
    let Some(pixmap) = load_external_pixmap(&mask.path) else {
        return Err(RenderError::Backend(format!(
            "Failed to load effect mask {:?}",
            mask.path
        )));
    };
    let art_rect = art_coverage_rect(&document.card, base);
    let full_card_sized = pixmap.width() == CARD_WIDTH && pixmap.height() == CARD_HEIGHT;
    let should_fit_art = !full_card_sized && mask.x.is_none() && mask.y.is_none();
    let pixmap = if should_fit_art {
        scale_effect_mask_to_art(pixmap, art_rect)?
    } else {
        pixmap
    };
    let default_x = if full_card_sized {
        0
    } else {
        art_rect.x as i32
    };
    let default_y = if full_card_sized {
        0
    } else {
        art_rect.y as i32
    };
    Ok(Some(EffectProtectionMask {
        x: mask.x.unwrap_or(default_x),
        y: mask.y.unwrap_or(default_y),
        pixmap,
    }))
}

fn scale_effect_mask_to_art(pixmap: Pixmap, art_rect: CoverageRect) -> Result<Pixmap, RenderError> {
    if pixmap.width() == art_rect.w && pixmap.height() == art_rect.h {
        return Ok(pixmap);
    }

    let target_w = art_rect.w.max(1);
    let target_h = art_rect.h.max(1);
    let mut target = Pixmap::new(target_w, target_h).ok_or_else(|| {
        RenderError::Backend(format!(
            "Failed to allocate scaled effect mask {target_w}x{target_h}"
        ))
    })?;

    let scale_x = target_w as f32 / pixmap.width() as f32;
    let scale_y = target_h as f32 / pixmap.height() as f32;
    target.draw_pixmap(
        0,
        0,
        pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::from_scale(scale_x, scale_y),
        None,
    );

    Ok(target)
}

pub(super) fn effect_target_areas(
    bundle: &AssetBundle,
    document: &RenderDocument,
    base: &BaseLayout,
    language: Option<&str>,
    target: EffectTarget,
    full_rect: CoverageRect,
    art_rect: CoverageRect,
) -> Vec<EffectArea> {
    match target {
        EffectTarget::FullCard => vec![EffectArea::Rect(full_rect)],
        EffectTarget::Art => art_effect_areas(bundle, &document.card, base, art_rect),
        EffectTarget::CardBase => card_base_areas_excluding_art(full_rect, art_rect)
            .into_iter()
            .map(EffectArea::Rect)
            .collect(),
        EffectTarget::ArtFrame if document.card.is_pendulum() => {
            pendulum_border_effect_areas(bundle, base)
        }
        EffectTarget::ArtFrame => art_frame_effect_areas(bundle, &document.card, base, art_rect),
        EffectTarget::CardBorder => card_border_areas()
            .into_iter()
            .map(EffectArea::Rect)
            .collect(),
        EffectTarget::Attribute => attribute_effect_area(bundle, &document.card, base, language)
            .into_iter()
            .collect(),
        EffectTarget::LevelOrRank => level_or_rank_effect_areas(bundle, &document.card, base),
        EffectTarget::LinkArrows => link_arrows_effect_areas(bundle, &document.card, base),
        EffectTarget::EffectBoxBorder if document.card.is_pendulum() => {
            pendulum_effect_box_border_areas(bundle, base)
        }
        EffectTarget::EffectBoxBorder => effect_box_border_areas(bundle, &document.card, base),
    }
}

pub(super) fn build_composite_mask(
    bundle: &AssetBundle,
    document: &RenderDocument,
    base: &BaseLayout,
    language: Option<&str>,
    full_rect: CoverageRect,
    art_rect: CoverageRect,
    effect_opacity: f32,
    targets: &[EffectTargetWeight],
) -> Option<Pixmap> {
    if targets.is_empty() || effect_opacity <= 0.0 || !effect_opacity.is_finite() {
        return None;
    }

    let mut mask = Pixmap::new(CARD_WIDTH, CARD_HEIGHT)?;
    let base_opacity = effect_opacity.clamp(0.0, 1.0);
    let mut painted = false;
    for target_weight in targets {
        if target_weight.opacity <= 0.0 || !target_weight.opacity.is_finite() {
            continue;
        }
        let alpha_scale = (target_weight.opacity.clamp(0.0, 1.0) / base_opacity).clamp(0.0, 1.0);
        let areas = effect_target_areas(
            bundle,
            document,
            base,
            language,
            target_weight.target,
            full_rect,
            art_rect,
        );
        for area in areas {
            painted |= paint_effect_area_into_mask(&mut mask, area, alpha_scale);
        }
    }

    painted.then_some(mask)
}

fn paint_effect_area_into_mask(mask: &mut Pixmap, area: EffectArea, alpha_scale: f32) -> bool {
    match area {
        EffectArea::Rect(rect) => paint_rect_into_mask(mask, rect, alpha_scale),
        EffectArea::MaskedRect {
            rect,
            mask: area_mask,
        } => paint_masked_rect_into_mask(mask, rect, area_mask.as_ref(), alpha_scale),
    }
}

fn paint_rect_into_mask(mask: &mut Pixmap, rect: CoverageRect, alpha_scale: f32) -> bool {
    let Some((x_start, y_start, x_end, y_end)) = clipped_rect_bounds(rect, CARD_WIDTH, CARD_HEIGHT)
    else {
        return false;
    };
    let alpha = scaled_alpha(255, alpha_scale);
    if alpha == 0 {
        return false;
    }

    let mut painted = false;
    for y in y_start..y_end {
        for x in x_start..x_end {
            painted |= write_mask_alpha(mask, x, y, alpha);
        }
    }
    painted
}

fn paint_masked_rect_into_mask(
    composite_mask: &mut Pixmap,
    rect: CoverageRect,
    area_mask: &Pixmap,
    alpha_scale: f32,
) -> bool {
    let Some((x_start, y_start, x_end, y_end)) = clipped_rect_bounds(rect, CARD_WIDTH, CARD_HEIGHT)
    else {
        return false;
    };

    let local_x_start = x_start.saturating_sub(rect.x);
    let local_y_start = y_start.saturating_sub(rect.y);
    let local_w = (x_end - x_start).min(area_mask.width().saturating_sub(local_x_start));
    let local_h = (y_end - y_start).min(area_mask.height().saturating_sub(local_y_start));
    if local_w == 0 || local_h == 0 {
        return false;
    }

    let mut painted = false;
    let area_pixels = area_mask.pixels();
    for local_y in 0..local_h {
        let y = y_start + local_y;
        let src_y = local_y_start + local_y;
        for local_x in 0..local_w {
            let x = x_start + local_x;
            let src_x = local_x_start + local_x;
            let area_idx = (src_y * area_mask.width() + src_x) as usize;
            let alpha = scaled_alpha(area_pixels[area_idx].alpha(), alpha_scale);
            if alpha > 0 {
                painted |= write_mask_alpha(composite_mask, x, y, alpha);
            }
        }
    }
    painted
}

fn scaled_alpha(alpha: u8, scale: f32) -> u8 {
    ((alpha as f32 * scale.clamp(0.0, 1.0)).round()).clamp(0.0, 255.0) as u8
}

fn write_mask_alpha(mask: &mut Pixmap, x: u32, y: u32, alpha: u8) -> bool {
    let idx = (y * mask.width() + x) as usize;
    let pixels = mask.pixels_mut();
    if alpha <= pixels[idx].alpha() {
        return false;
    }
    pixels[idx] = tiny_skia::PremultipliedColorU8::from_rgba(alpha, alpha, alpha, alpha)
        .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
    true
}

fn card_base_areas_excluding_art(
    full_rect: CoverageRect,
    art_rect: CoverageRect,
) -> Vec<CoverageRect> {
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

fn pendulum_border_effect_areas(bundle: &AssetBundle, base: &BaseLayout) -> Vec<EffectArea> {
    if let Some(border) = &base.mask.pendulum_border {
        if let Some(area) = masked_area(bundle, &border.asset, border.x, border.y) {
            return vec![area];
        }
    }

    Vec::new()
}

fn pendulum_effect_box_border_areas(bundle: &AssetBundle, base: &BaseLayout) -> Vec<EffectArea> {
    let Some(border) = &base.mask.pendulum_effect_border else {
        return Vec::new();
    };
    masked_area(bundle, &border.asset, border.x, border.y)
        .into_iter()
        .collect()
}

fn pendulum_frame_mask(base: &BaseLayout) -> &PositionedAsset {
    base.mask
        .pendulum_border
        .as_ref()
        .unwrap_or(&base.mask.pendulum)
}

fn art_effect_areas(
    bundle: &AssetBundle,
    card: &YgoCardMeta,
    base: &BaseLayout,
    art_rect: CoverageRect,
) -> Vec<EffectArea> {
    if !card.is_pendulum() {
        return vec![EffectArea::Rect(art_rect)];
    }

    if let Some(art_mask) = &base.mask.pendulum_art {
        if let Some(area) = masked_area(bundle, &art_mask.asset, art_mask.x, art_mask.y) {
            return vec![area];
        }
    }

    let frame_mask = pendulum_frame_mask(base);
    let Some(mask) = bundle.decoded_image_for_render(&frame_mask.asset).ok() else {
        return vec![EffectArea::Rect(art_rect)];
    };

    let mask_rect = CoverageRect {
        x: frame_mask.x,
        y: frame_mask.y,
        w: mask.width(),
        h: mask.height(),
    };
    let Some((rect, mask)) = visible_pendulum_art_mask(art_rect, mask_rect, &mask) else {
        return Vec::new();
    };

    vec![EffectArea::MaskedRect {
        rect,
        mask: Arc::new(mask),
    }]
}

fn visible_pendulum_art_mask(
    art_rect: CoverageRect,
    frame_mask_rect: CoverageRect,
    frame_mask: &Pixmap,
) -> Option<(CoverageRect, Pixmap)> {
    let x0 = art_rect.x.max(frame_mask_rect.x);
    let y0 = art_rect.y.max(frame_mask_rect.y);
    let x1 = art_rect
        .x
        .saturating_add(art_rect.w)
        .min(frame_mask_rect.x.saturating_add(frame_mask_rect.w));
    let y1 = art_rect
        .y
        .saturating_add(art_rect.h)
        .min(frame_mask_rect.y.saturating_add(frame_mask_rect.h));
    if x0 >= x1 || y0 >= y1 {
        return None;
    }

    let w = x1 - x0;
    let h = y1 - y0;
    let mut mask = Pixmap::new(w, h)?;
    let src_pixels = frame_mask.pixels();
    let dst_pixels = mask.pixels_mut();

    for local_y in 0..h {
        let src_y = y0 + local_y - frame_mask_rect.y;
        for local_x in 0..w {
            let src_x = x0 + local_x - frame_mask_rect.x;
            let src_idx = (src_y * frame_mask.width() + src_x) as usize;
            let dst_idx = (local_y * w + local_x) as usize;
            let frame_alpha = src_pixels[src_idx].alpha();
            // Only the transparent hole of card-mask-pendulum is real art.
            // Semi-transparent pendulum scale/effect panels are visual frame
            // pixels, so they must not receive Art-target rare effects.
            let allow = if frame_alpha <= 8 { 255 } else { 0 };
            dst_pixels[dst_idx] =
                tiny_skia::PremultipliedColorU8::from_rgba(allow, allow, allow, allow)
                    .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
        }
    }

    Some((CoverageRect { x: x0, y: y0, w, h }, mask))
}

pub(super) fn art_frame_effect_areas(
    bundle: &AssetBundle,
    card: &YgoCardMeta,
    base: &BaseLayout,
    art_rect: CoverageRect,
) -> Vec<EffectArea> {
    let frame_mask = if card.is_pendulum() {
        // Split bundles expose only the upper illustration frame here, so
        // frame-targeted effects don't wash over the pendulum effect panel,
        // scale boxes, or lower text-box background.
        pendulum_frame_mask(base)
    } else {
        &base.mask.normal
    };

    if let Some(area) = masked_area(bundle, &frame_mask.asset, frame_mask.x, frame_mask.y) {
        return vec![area];
    }

    frame_ring_areas(art_rect, 28, 28, 28)
        .into_iter()
        .map(EffectArea::Rect)
        .collect()
}

#[cfg(test)]
pub(super) fn art_frame_coverage_rect(
    bundle: &AssetBundle,
    card: &YgoCardMeta,
    base: &BaseLayout,
) -> Option<CoverageRect> {
    let mask = if card.is_pendulum() {
        pendulum_frame_mask(base)
    } else {
        &base.mask.normal
    };
    let mask_image = bundle.decoded_image_for_render(&mask.asset).ok()?;
    Some(CoverageRect {
        x: mask.x,
        y: mask.y,
        w: mask_image.width(),
        h: mask_image.height(),
    })
}

fn frame_ring_areas(
    rect: CoverageRect,
    thickness: u32,
    inset_x: u32,
    inset_y: u32,
) -> Vec<CoverageRect> {
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

pub(super) fn art_coverage_rect(card: &YgoCardMeta, base: &BaseLayout) -> CoverageRect {
    let (x, y, w, h) = image_frame(card, base);
    CoverageRect { x, y, w, h }
}

fn attribute_effect_area(
    bundle: &AssetBundle,
    card: &YgoCardMeta,
    base: &BaseLayout,
    language: Option<&str>,
) -> Option<EffectArea> {
    let asset = attribute_asset_name(card, language)?;
    masked_area(bundle, &asset, base.attribute.x, base.attribute.y)
}

fn level_or_rank_effect_areas(
    bundle: &AssetBundle,
    card: &YgoCardMeta,
    base: &BaseLayout,
) -> Vec<EffectArea> {
    let count = card.level.min(13);
    if count == 0 || card.is_link() {
        return Vec::new();
    }

    let (layout, left_to_right) = if uses_rank(card) {
        (&base.rank, true)
    } else {
        (&base.level, false)
    };
    let Some(mask) = bundle.decoded_image_for_render(&layout.asset).ok() else {
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

fn link_arrows_effect_areas(
    bundle: &AssetBundle,
    card: &YgoCardMeta,
    base: &BaseLayout,
) -> Vec<EffectArea> {
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
    const ARROW_BITS: &[u32] = &[0x004, 0x080, 0x020, 0x100, 0x040, 0x008, 0x001, 0x002];

    let mut areas = Vec::new();
    for (key, bit) in ARROW_KEYS.iter().zip(ARROW_BITS.iter()) {
        if (card.link_marker & bit) == 0 {
            continue;
        }
        let Some(pair) = base.link_arrows.get(*key) else {
            continue;
        };
        // Pre-computed red mask from build_bundle (preferred).
        if let Some(ref red) = pair.red_mask {
            if let Some(area) = masked_area(bundle, &red.asset, red.x, red.y) {
                areas.push(area);
                continue;
            }
        }
        // Fallback: compute red mask at runtime by diffing on/off images.
        let Some(on_img) = bundle.decoded_image_for_render(&pair.on.asset).ok() else {
            continue;
        };
        let Some(off_img) = bundle.decoded_image_for_render(&pair.off.asset).ok() else {
            continue;
        };
        let Some(red_mask) = arrow_red_region_mask(&on_img, &off_img) else {
            continue;
        };
        areas.push(EffectArea::MaskedRect {
            rect: CoverageRect {
                x: pair.on.x,
                y: pair.on.y,
                w: red_mask.width(),
                h: red_mask.height(),
            },
            mask: Arc::new(red_mask),
        });
    }
    areas
}

/// Build a mask isolating the red fill region of a link arrow by comparing the
/// active (`on`) and inactive (`off`) arrow images. Pixels where the two images
/// differ substantially (the red centre) receive full alpha; the shared frame
/// pixels are left transparent.
fn arrow_red_region_mask(on: &Pixmap, off: &Pixmap) -> Option<Pixmap> {
    if on.width() != off.width() || on.height() != off.height() {
        return None;
    }
    let mut mask = Pixmap::new(on.width(), on.height())?;
    let mask_pixels = mask.pixels_mut();
    let on_pixels = on.pixels();
    let off_pixels = off.pixels();

    for i in 0..on_pixels.len() {
        let o = on_pixels[i];
        let f = off_pixels[i];
        let dr = o.red() as i32 - f.red() as i32;
        let dg = o.green() as i32 - f.green() as i32;
        let db = o.blue() as i32 - f.blue() as i32;
        let diff = dr.abs() + dg.abs() + db.abs();
        let alpha = if diff > 120 { 255u8 } else { 0u8 };
        mask_pixels[i] = tiny_skia::PremultipliedColorU8::from_rgba(alpha, alpha, alpha, alpha)
            .unwrap_or(tiny_skia::PremultipliedColorU8::TRANSPARENT);
    }
    Some(mask)
}

fn effect_box_border_areas(
    bundle: &AssetBundle,
    card: &YgoCardMeta,
    base: &BaseLayout,
) -> Vec<EffectArea> {
    let effect_box = match card.out_frame_effect_box {
        OutFrameEffectBox::EblockBorder => &base.out_frame.effect_box,
        OutFrameEffectBox::EblockBorderO => &base.out_frame.effect_box_colored,
    };
    masked_area(bundle, &effect_box.asset, effect_box.x, effect_box.y)
        .into_iter()
        .collect()
}

fn masked_area(bundle: &AssetBundle, asset_name: &str, x: u32, y: u32) -> Option<EffectArea> {
    let mask = bundle.decoded_image_for_render(asset_name).ok()?;
    Some(EffectArea::MaskedRect {
        rect: CoverageRect {
            x,
            y,
            w: mask.width(),
            h: mask.height(),
        },
        mask,
    })
}

fn clipped_rect_bounds(rect: CoverageRect, max_w: u32, max_h: u32) -> Option<(u32, u32, u32, u32)> {
    let x_start = rect.x.min(max_w);
    let y_start = rect.y.min(max_h);
    let x_end = rect.x.saturating_add(rect.w).min(max_w);
    let y_end = rect.y.saturating_add(rect.h).min(max_h);
    (x_start < x_end && y_start < y_end).then_some((x_start, y_start, x_end, y_end))
}

// ── Visual effect dispatch ────────────────────────────────────────────────────

pub(super) fn draw_visual_effect_area(
    target: &mut Pixmap,
    area: EffectArea,
    effect: EffectStyle,
    protection_mask: Option<&EffectProtectionMask>,
) {
    match area {
        EffectArea::Rect(rect) if protection_mask.is_none() => {
            draw_visual_effect_rect(target, rect, effect)
        }
        EffectArea::Rect(rect) => {
            draw_masked_visual_effect(target, rect, None, effect, protection_mask)
        }
        EffectArea::MaskedRect { rect, mask } => {
            draw_masked_visual_effect(target, rect, Some(mask.as_ref()), effect, protection_mask)
        }
    }
}

fn draw_visual_effect_rect(target: &mut Pixmap, rect: CoverageRect, effect: EffectStyle) {
    use crate::rare_effect::{
        draw_diamond_foil, draw_dot_grid, draw_holographic, draw_optical_scr,
        draw_optical_scr_simple, draw_optical_ser, draw_optical_ser_simple, draw_rainbow_foil,
        draw_secret_foil, draw_secret_weave,
    };
    match effect {
        EffectStyle::RainbowFoil { opacity } => draw_rainbow_foil(target, rect, opacity),
        EffectStyle::DotGrid { opacity } => draw_dot_grid(target, rect, opacity),
        EffectStyle::OpticalSer { opacity } => draw_optical_ser(target, rect, opacity),
        EffectStyle::OpticalSerSimple { opacity } => draw_optical_ser_simple(target, rect, opacity),
        EffectStyle::OpticalScr { opacity } => draw_optical_scr(target, rect, opacity),
        EffectStyle::OpticalScrSimple { opacity } => draw_optical_scr_simple(target, rect, opacity),
        EffectStyle::SecretWeave { opacity } => draw_secret_weave(target, rect, opacity),
        EffectStyle::SecretFoil { opacity } => draw_secret_foil(target, rect, opacity),
        EffectStyle::Holographic { opacity } => draw_holographic(target, rect, opacity),
        EffectStyle::GoldWash { opacity } => draw_gold_wash(target, rect, opacity),
        EffectStyle::FrostedFoil { opacity } => draw_frosted_foil(target, rect, opacity),
        EffectStyle::ConcentricEngrave { opacity } => {
            draw_concentric_engrave(target, rect, opacity)
        }
        EffectStyle::ReliefEngrave { opacity } => draw_relief_engrave(target, rect, opacity),
        EffectStyle::DiamondFoil { opacity } => draw_diamond_foil(target, rect, opacity),
        EffectStyle::BrightBorder { .. } => {}
    }
}

fn draw_masked_visual_effect(
    target: &mut Pixmap,
    rect: CoverageRect,
    mask: Option<&Pixmap>,
    effect: EffectStyle,
    protection_mask: Option<&EffectProtectionMask>,
) {
    let before = snapshot_effect_rect(target, rect);

    draw_visual_effect_rect(target, rect, effect);

    restore_effect_pixels(target, &before, mask, protection_mask);
}

pub(super) fn snapshot_effect_rect(target: &Pixmap, rect: CoverageRect) -> EffectRectSnapshot {
    let width = target.width();
    let height = target.height();
    let x_start = rect.x.min(width);
    let y_start = rect.y.min(height);
    let x_end = rect.x.saturating_add(rect.w).min(width);
    let y_end = rect.y.saturating_add(rect.h).min(height);
    let sub_w = (x_end - x_start) as usize;

    let mut pixels: Vec<tiny_skia::PremultipliedColorU8> =
        Vec::with_capacity(sub_w * (y_end - y_start) as usize);
    let src = target.pixels();
    for y in y_start..y_end {
        let row_start = (y * width + x_start) as usize;
        pixels.extend_from_slice(&src[row_start..row_start + sub_w]);
    }

    EffectRectSnapshot {
        rect,
        x_start,
        y_start,
        x_end,
        y_end,
        sub_w,
        pixels,
    }
}

pub(super) fn restore_protected_effect_pixels(
    target: &mut Pixmap,
    before: &EffectRectSnapshot,
    protection_mask: Option<&EffectProtectionMask>,
) {
    restore_effect_pixels(target, before, None, protection_mask);
}

pub(super) fn restore_masked_effect_pixels(
    target: &mut Pixmap,
    before: &EffectRectSnapshot,
    mask: &Pixmap,
    protection_mask: Option<&EffectProtectionMask>,
) {
    restore_effect_pixels(target, before, Some(mask), protection_mask);
}

fn restore_effect_pixels(
    target: &mut Pixmap,
    before: &EffectRectSnapshot,
    mask: Option<&Pixmap>,
    protection_mask: Option<&EffectProtectionMask>,
) {
    if before.x_start >= before.x_end || before.y_start >= before.y_end {
        return;
    }

    let mask_w = mask.map(Pixmap::width).unwrap_or(0);
    let mask_h = mask.map(Pixmap::height).unwrap_or(0);
    let mask_pixels = mask.map(Pixmap::pixels);
    let width = target.width();
    let pixels = target.pixels_mut();

    for (iy, y) in (before.y_start..before.y_end).enumerate() {
        for (ix, x) in (before.x_start..before.x_end).enumerate() {
            let target_idx = (y * width + x) as usize;
            let before_idx = iy * before.sub_w + ix;

            let mut alpha = 255_u16;

            if let Some(mask_pixels) = mask_pixels {
                let local_x = x.saturating_sub(before.rect.x);
                let local_y = y.saturating_sub(before.rect.y);
                if local_x >= mask_w || local_y >= mask_h {
                    alpha = 0;
                } else {
                    let mask_idx = (local_y * mask_w + local_x) as usize;
                    alpha = mask_pixels[mask_idx].alpha() as u16;
                }
            }

            if let Some(protection_mask) = protection_mask {
                alpha = alpha * protection_mask.coverage_alpha(x, y) / 255;
            }

            if alpha == 0 {
                pixels[target_idx] = before.pixels[before_idx];
            } else if alpha < 255 {
                pixels[target_idx] =
                    lerp_premul(before.pixels[before_idx], pixels[target_idx], alpha);
            }
        }
    }
}

impl EffectProtectionMask {
    fn coverage_alpha(&self, x: u32, y: u32) -> u16 {
        let local_x = x as i32 - self.x;
        let local_y = y as i32 - self.y;
        if local_x < 0
            || local_y < 0
            || local_x >= self.pixmap.width() as i32
            || local_y >= self.pixmap.height() as i32
        {
            return 255;
        }

        let idx = (local_y as u32 * self.pixmap.width() + local_x as u32) as usize;
        let pixel = self.pixmap.pixels()[idx];
        let alpha = pixel.alpha() as u16;
        if alpha == 0 {
            return 255;
        }

        // External masks are premultiplied when loaded. Un-premultiply before
        // luminance so semi-transparent white remains “allow effects”.
        let channel = |c: u8| -> u32 { (c as u32 * 255 / alpha as u32).min(255) };
        let r = channel(pixel.red());
        let g = channel(pixel.green());
        let b = channel(pixel.blue());
        let luma = (r * 2126 + g * 7152 + b * 722 + 5000) / 10_000;
        let protect = alpha as u32 * (255 - luma.min(255)) / 255;
        (255 - protect.min(255)) as u16
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

#[cfg(test)]
mod tests {
    use super::*;

    fn px(r: u8, g: u8, b: u8, a: u8) -> tiny_skia::PremultipliedColorU8 {
        tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, a).unwrap()
    }

    #[test]
    fn black_effect_mask_restores_pixels_and_white_keeps_effects() {
        let mut target = Pixmap::new(2, 1).unwrap();
        target.pixels_mut()[0] = px(10, 20, 30, 255);
        target.pixels_mut()[1] = px(40, 50, 60, 255);

        let before = snapshot_effect_rect(
            &target,
            CoverageRect {
                x: 0,
                y: 0,
                w: 2,
                h: 1,
            },
        );

        target.pixels_mut()[0] = px(200, 0, 0, 255);
        target.pixels_mut()[1] = px(0, 200, 0, 255);

        let mut mask = Pixmap::new(2, 1).unwrap();
        mask.pixels_mut()[0] = px(0, 0, 0, 255);
        mask.pixels_mut()[1] = px(255, 255, 255, 255);
        let protection_mask = EffectProtectionMask {
            x: 0,
            y: 0,
            pixmap: mask,
        };

        restore_protected_effect_pixels(&mut target, &before, Some(&protection_mask));

        assert_eq!(target.pixels()[0], px(10, 20, 30, 255));
        assert_eq!(target.pixels()[1], px(0, 200, 0, 255));
    }

    #[test]
    fn pendulum_art_mask_only_allows_transparent_hole() {
        let mut frame_mask = Pixmap::new(3, 1).unwrap();
        frame_mask.pixels_mut()[0] = px(0, 0, 0, 0);
        frame_mask.pixels_mut()[1] = px(0, 0, 0, 9);
        frame_mask.pixels_mut()[2] = px(0, 0, 0, 255);

        let (_, art_mask) = visible_pendulum_art_mask(
            CoverageRect {
                x: 10,
                y: 20,
                w: 3,
                h: 1,
            },
            CoverageRect {
                x: 10,
                y: 20,
                w: 3,
                h: 1,
            },
            &frame_mask,
        )
        .expect("build pendulum art mask");

        assert_eq!(art_mask.pixels()[0].alpha(), 255);
        assert_eq!(art_mask.pixels()[1].alpha(), 0);
        assert_eq!(art_mask.pixels()[2].alpha(), 0);
    }
}
