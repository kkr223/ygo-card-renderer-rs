use tiny_skia::Pixmap;

use crate::{
    asset_bundle::{AssetBundle, BaseLayout},
    card_logic::{attribute_asset_name, image_frame, uses_rank},
    constants::{CARD_HEIGHT, CARD_WIDTH},
    document::{EffectStyle, EffectTarget},
    model::RenderRequest,
    rare_effect::CoverageRect,
};

use super::visual_effects::{
    draw_concentric_engrave, draw_frosted_foil, draw_gold_wash, draw_relief_engrave,
};

#[derive(Debug, Clone)]
pub(super) enum EffectArea {
    Rect(CoverageRect),
    MaskedRect { rect: CoverageRect, mask: Pixmap },
}

pub(super) fn effect_target_areas(
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
        EffectTarget::ArtFrame => art_frame_effect_areas(bundle, request, base, art_rect),
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

pub(super) fn art_frame_effect_areas(
    bundle: &AssetBundle,
    request: &RenderRequest,
    base: &BaseLayout,
    art_rect: CoverageRect,
) -> Vec<EffectArea> {
    let frame_mask = if request.card.is_pendulum() {
        &base.mask.pendulum
    } else {
        &base.mask.normal
    };

    if let Some(mask) = decode_bundle_image(bundle, &frame_mask.asset) {
        return vec![EffectArea::MaskedRect {
            rect: CoverageRect {
                x: frame_mask.x,
                y: frame_mask.y,
                w: mask.width(),
                h: mask.height(),
            },
            mask,
        }];
    }

    frame_ring_areas(art_rect, 28, 28, 28)
        .into_iter()
        .map(EffectArea::Rect)
        .collect()
}

#[cfg(test)]
pub(super) fn art_frame_coverage_rect(
    bundle: &AssetBundle,
    request: &RenderRequest,
    base: &BaseLayout,
) -> Option<CoverageRect> {
    let mask = if request.card.is_pendulum() {
        &base.mask.pendulum
    } else {
        &base.mask.normal
    };
    let mask_image = decode_bundle_image(bundle, &mask.asset)?;
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

pub(super) fn art_coverage_rect(request: &RenderRequest, base: &BaseLayout) -> CoverageRect {
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

pub(super) fn decode_bundle_image(bundle: &AssetBundle, asset: &str) -> Option<Pixmap> {
    let entry = bundle.image(asset).ok()?;
    match entry.kind.as_str() {
        "raster" => bundle.decode_raster(asset).ok(),
        "svg" => bundle.decode_svg(asset).ok(),
        _ => None,
    }
}

// ── Visual effect dispatch ────────────────────────────────────────────────────

pub(super) fn draw_visual_effect_area(
    target: &mut Pixmap,
    area: EffectArea,
    effect: EffectStyle,
) {
    match area {
        EffectArea::Rect(rect) => draw_visual_effect_rect(target, rect, effect),
        EffectArea::MaskedRect { rect, mask } => {
            draw_masked_visual_effect(target, rect, &mask, effect)
        }
    }
}

fn draw_visual_effect_rect(target: &mut Pixmap, rect: CoverageRect, effect: EffectStyle) {
    use crate::rare_effect::{draw_dot_grid, draw_holographic, draw_rainbow_foil, draw_secret_weave};
    match effect {
        EffectStyle::RainbowFoil { opacity } => draw_rainbow_foil(target, rect, opacity),
        EffectStyle::DotGrid { opacity } => draw_dot_grid(target, rect, opacity),
        EffectStyle::SecretWeave { opacity } => draw_secret_weave(target, rect, opacity),
        EffectStyle::Holographic { opacity } => draw_holographic(target, rect, opacity),
        EffectStyle::GoldWash { opacity } => draw_gold_wash(target, rect, opacity),
        EffectStyle::FrostedFoil { opacity } => draw_frosted_foil(target, rect, opacity),
        EffectStyle::ConcentricEngrave { opacity } => draw_concentric_engrave(target, rect, opacity),
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
