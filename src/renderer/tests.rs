use super::{
    CoverageRect, art_coverage_rect,
    draw_card::premultiply_pixmap_alpha,
    effect_areas::{EffectArea, art_frame_coverage_rect, art_frame_effect_areas},
    scale_pixmap,
    visual_effects::{draw_frosted_foil, draw_relief_engrave},
};
use crate::{
    CardKind, RenderOptions, RenderRequest,
    asset_bundle::{AssetBundle, get_bundle, init_global_bundle},
    document::{RenderDocument, RenderNode, RenderOp, RenderRect, laser_asset_name},
    model::YgoCardMeta,
};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Once},
};
use tiny_skia::PremultipliedColorU8;
use ygopro_cdb_encode_rs::CardDataEntry;

fn init_bundle() {
    static INIT: Once = Once::new();
    let bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("yugioh_bundle.bin");

    INIT.call_once(|| {
        let bytes = fs::read(&bin_path).expect("read yugioh bundle");
        init_global_bundle(&bytes).expect("initialize yugioh bundle");
    });
}

fn load_test_bundle() -> Arc<AssetBundle> {
    let bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("yugioh_bundle.bin");
    let bytes = fs::read(&bin_path).expect("read yugioh bundle");
    Arc::new(AssetBundle::load_from_bytes(&bytes).expect("load explicit bundle"))
}

#[test]
fn sorted_visible_nodes_filters_invisible_and_keeps_order_for_equal_z() {
    let fill = |color: &str| crate::document::RenderOp::FillRect {
        rect: crate::document::RenderRect::new(0, 0, 1, 1),
        color: color.to_string(),
        opacity: 1.0,
    };
    let document = crate::document::RenderDocument {
        schema_version: crate::document::RenderDocument::SCHEMA_VERSION,
        kind: CardKind::Yugioh,
        canvas: crate::document::RenderCanvas {
            width: 1,
            height: 1,
            background: None,
        },
        language: None,
        output_scale: 1.0,
        card: YgoCardMeta::from(CardDataEntry::default()),
        options: RenderOptions::default(),
        nodes: vec![
            crate::document::RenderNode::new("visible-a", 10, fill("#000000")),
            crate::document::RenderNode {
                id: "hidden".to_string(),
                z: 5,
                visible: false,
                op: fill("#111111"),
            },
            crate::document::RenderNode::new("visible-b", 10, fill("#222222")),
            crate::document::RenderNode::new("visible-before", 0, fill("#333333")),
        ],
    };

    let nodes = super::sorted_visible_nodes(&document);
    assert_eq!(
        nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        vec!["visible-before", "visible-a", "visible-b"]
    );
}

#[test]
fn render_document_accepts_schema_version_3_for_compatibility() {
    let document = crate::document::RenderDocument {
        schema_version: 3,
        kind: CardKind::Yugioh,
        canvas: crate::document::RenderCanvas {
            width: 1,
            height: 1,
            background: None,
        },
        language: None,
        output_scale: 1.0,
        card: YgoCardMeta::from(CardDataEntry::default()),
        options: RenderOptions::default(),
        nodes: Vec::new(),
    };

    let png = super::Renderer::new()
        .render_document(&document)
        .expect("schema v3 document should remain renderable");
    assert!(!png.is_empty());
}

#[test]
fn explicit_bundle_renderer_builds_document() {
    let renderer = super::Renderer::with_bundle(load_test_bundle());
    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: YgoCardMeta::from(CardDataEntry {
            code: 46986414,
            name: "Dark Magician".to_string(),
            desc: "The ultimate wizard in terms of attack and defense.".to_string(),
            type_: 0x41,
            attack: 2500,
            defense: 2100,
            level: 7,
            race: 0x1,
            attribute: 0x10,
            ..CardDataEntry::default()
        }),
        options: RenderOptions::default(),
    };

    let document = renderer
        .build_document(&request)
        .expect("build document with explicit bundle");
    assert!(document.nodes.iter().any(|node| node.id == "frame"));
    assert!(document.nodes.iter().any(|node| node.id == "title"));
}

#[test]
fn explicit_bundle_renderer_renders_non_empty_document() {
    let renderer = super::Renderer::with_bundle(load_test_bundle());
    let document = RenderDocument {
        schema_version: RenderDocument::SCHEMA_VERSION,
        kind: CardKind::Yugioh,
        canvas: crate::document::RenderCanvas {
            width: 4,
            height: 4,
            background: None,
        },
        language: None,
        output_scale: 1.0,
        card: YgoCardMeta::from(CardDataEntry::default()),
        options: RenderOptions::default(),
        nodes: vec![RenderNode::new(
            "test-fill",
            0,
            RenderOp::FillRect {
                rect: RenderRect::new(0, 0, 4, 4),
                color: "#ff0000".to_string(),
                opacity: 1.0,
            },
        )],
    };

    let png = renderer
        .render_document(&document)
        .expect("render document with explicit bundle");
    assert!(!png.is_empty());
}

#[test]
fn explicit_bundle_renderer_draws_bundle_image_assets() {
    let bundle = load_test_bundle();
    assert!(bundle.has_image("card-normal.webp"));
    let renderer = super::Renderer::with_bundle(bundle);
    let document = RenderDocument {
        schema_version: RenderDocument::SCHEMA_VERSION,
        kind: CardKind::Yugioh,
        canvas: crate::document::RenderCanvas {
            width: 16,
            height: 16,
            background: None,
        },
        language: None,
        output_scale: 1.0,
        card: YgoCardMeta::from(CardDataEntry::default()),
        options: RenderOptions::default(),
        nodes: vec![RenderNode::new(
            "test-image",
            0,
            RenderOp::ImageAsset {
                asset: "card-normal.webp".to_string(),
                x: 0.0,
                y: 0.0,
            },
        )],
    };

    let png = renderer
        .render_document(&document)
        .expect("render image asset with explicit bundle");
    assert!(!png.is_empty());
}

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

#[test]
fn sanitizes_effect_style_opacity() {
    let nan = f32::NAN;

    assert!(matches!(
        super::sanitize_effect_style(crate::document::EffectStyle::RainbowFoil {
            opacity: nan,
        }),
        crate::document::EffectStyle::RainbowFoil { opacity } if opacity == 0.0
    ));

    assert!(matches!(
        super::sanitize_effect_style(crate::document::EffectStyle::ReliefEngrave {
            opacity: -0.25,
        }),
        crate::document::EffectStyle::ReliefEngrave { opacity } if opacity == 0.0
    ));
}

#[test]
fn validates_render_dimensions_bounds() {
    assert!(matches!(
        super::validate_render_dimensions(0, 1),
        Err(crate::model::RenderError::Backend(_))
    ));
    assert!(matches!(
        super::validate_render_dimensions(4097, 4097),
        Err(crate::model::RenderError::Backend(_))
    ));
    assert!(super::validate_render_dimensions(1, 1).is_ok());
}

#[test]
fn relief_engrave_prefers_flat_height_map_regions() {
    let mut pixmap = tiny_skia::Pixmap::new(64, 32).expect("pixmap");
    {
        let pixels = pixmap.pixels_mut();
        for y in 0..32 {
            for x in 0..64 {
                let value = if x < 32 { 45 } else { 180 };
                pixels[(y * 64 + x) as usize] =
                    PremultipliedColorU8::from_rgba(value, value, value, 255).unwrap();
            }
        }
    }
    let before = pixmap.pixels().to_vec();
    draw_relief_engrave(
        &mut pixmap,
        CoverageRect {
            x: 0,
            y: 0,
            w: 64,
            h: 32,
        },
        0.7,
    );

    let avg_delta = |x0: u32, x1: u32| -> f32 {
        let mut total = 0.0_f32;
        let mut count = 0_u32;
        for y in 4..28 {
            for x in x0..x1 {
                let idx = (y * 64 + x) as usize;
                total += (pixmap.pixels()[idx].red() as i16 - before[idx].red() as i16)
                    .unsigned_abs() as f32;
                count += 1;
            }
        }
        total / count as f32
    };

    let flat_delta = avg_delta(6, 24);
    let edge_delta = avg_delta(30, 34);
    assert!(flat_delta > edge_delta);
}

#[test]
fn frosted_foil_is_continuous_across_split_rects() {
    let mut whole = tiny_skia::Pixmap::new(64, 64).expect("whole pixmap");
    whole.fill(tiny_skia::Color::from_rgba8(40, 55, 70, 255));
    draw_frosted_foil(
        &mut whole,
        CoverageRect {
            x: 0,
            y: 0,
            w: 64,
            h: 64,
        },
        0.5,
    );

    let mut split = tiny_skia::Pixmap::new(64, 64).expect("split pixmap");
    split.fill(tiny_skia::Color::from_rgba8(40, 55, 70, 255));
    draw_frosted_foil(
        &mut split,
        CoverageRect {
            x: 0,
            y: 0,
            w: 64,
            h: 21,
        },
        0.5,
    );
    draw_frosted_foil(
        &mut split,
        CoverageRect {
            x: 0,
            y: 21,
            w: 64,
            h: 43,
        },
        0.5,
    );

    assert_eq!(split.pixels(), whole.pixels());
}

#[test]
fn art_frame_coverage_rect_expands_beyond_art_rect() {
    init_bundle();

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: YgoCardMeta::from(CardDataEntry {
            code: 46986414,
            name: "銉栥儵銉冦偗銉汇優銈搞偡銉ｃ兂".to_string(),
            desc: "test".to_string(),
            type_: 0x41,
            attack: 2500,
            defense: 2100,
            level: 7,
            race: 0x1,
            attribute: 0x10,
            ..CardDataEntry::default()
        }),
        options: RenderOptions::default(),
    };

    let bundle = get_bundle();
    let art_rect = art_coverage_rect(&request.card, &bundle.layout.base);
    let frame_rect =
        art_frame_coverage_rect(bundle, &request.card, &bundle.layout.base).expect("frame rect");

    assert!(frame_rect.x <= art_rect.x);
    assert!(frame_rect.y <= art_rect.y);
    assert!(frame_rect.x + frame_rect.w >= art_rect.x + art_rect.w);
    assert!(frame_rect.y + frame_rect.h >= art_rect.y + art_rect.h);
    assert!(
        frame_rect.x < art_rect.x
            || frame_rect.y < art_rect.y
            || frame_rect.x + frame_rect.w > art_rect.x + art_rect.w
            || frame_rect.y + frame_rect.h > art_rect.y + art_rect.h
    );
}

#[test]
fn art_frame_effect_uses_mask_alpha() {
    init_bundle();

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: YgoCardMeta::from(CardDataEntry {
            code: 46986414,
            name: "銉栥儵銉冦偗銉汇優銈搞偡銉ｃ兂".to_string(),
            desc: "test".to_string(),
            type_: 0x41,
            attack: 2500,
            defense: 2100,
            level: 7,
            race: 0x1,
            attribute: 0x10,
            ..CardDataEntry::default()
        }),
        options: RenderOptions::default(),
    };

    let bundle = get_bundle();
    let art_rect = art_coverage_rect(&request.card, &bundle.layout.base);
    let areas = art_frame_effect_areas(bundle, &request.card, &bundle.layout.base, art_rect);

    assert_eq!(areas.len(), 1);
    let EffectArea::MaskedRect { rect, mask } = &areas[0] else {
        panic!("art frame effect should follow the frame mask alpha");
    };

    assert!(rect.x < art_rect.x);
    assert!(rect.y < art_rect.y);

    let art_center_x = art_rect.x + art_rect.w / 2 - rect.x;
    let art_center_y = art_rect.y + art_rect.h / 2 - rect.y;
    let frame_edge_x = art_rect.x - rect.x - 1;
    let frame_edge_y = art_rect.y + art_rect.h / 2 - rect.y;
    let center_alpha = mask.pixels()[(art_center_y * mask.width() + art_center_x) as usize].alpha();
    let edge_alpha = mask.pixels()[(frame_edge_y * mask.width() + frame_edge_x) as usize].alpha();

    assert_eq!(center_alpha, 0);
    assert!(edge_alpha > 0);
}
