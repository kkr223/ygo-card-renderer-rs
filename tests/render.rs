use std::{fs, path::PathBuf};

use ygo_card_renderer_rs::{CardKind, RenderOptions, RenderRequest, render_png, render_svg};
use ygopro_cdb_encode_rs::CardDataEntry;

fn write_min_png(path: &PathBuf) {
    let bytes: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    fs::write(path, bytes).expect("write png");
}

fn sample_card() -> CardDataEntry {
    CardDataEntry {
        code: 12345678,
        alias: 0,
        setcode: vec![],
        type_: 0x21,
        attack: 1800,
        defense: 1200,
        level: 4,
        race: 0x1,
        attribute: 0x20,
        category: 0,
        ot: 4,
        name: "测试卡".to_string(),
        desc: "这是一张用于测试渲染管线的卡片。".to_string(),
        strings: vec![],
        lscale: 0,
        rscale: 0,
        link_marker: 0,
        rule_code: 0,
    }
}

fn sample_link_card() -> CardDataEntry {
    CardDataEntry {
        code: 87654321,
        alias: 0,
        setcode: vec![],
        type_: 0x1 | 0x20 | 0x4000000,
        attack: 2300,
        defense: 0,
        level: 3,
        race: 0x1,
        attribute: 0x10,
        category: 0,
        ot: 4,
        name: "连接测试卡".to_string(),
        desc: "用于验证连接箭头与效果行布局。".to_string(),
        strings: vec![],
        lscale: 0,
        rscale: 0,
        link_marker: 0x080 | 0x020 | 0x001,
        rule_code: 0,
    }
}

fn artifact_dir() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-artifacts");
    fs::create_dir_all(&path).expect("create artifact dir");
    path
}

#[test]
fn render_svg_and_png_from_card_data_entry() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let resource_root = temp_dir.path().join("resources");
    fs::create_dir_all(resource_root.join("yugioh/image")).expect("mkdir");
    write_min_png(&resource_root.join("yugioh/image/card-effect.png"));

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: sample_card(),
        options: RenderOptions {
            resource_path: resource_root,
            scale: 1.0,
            ..RenderOptions::default()
        },
    };

    let svg = render_svg(&request).expect("render svg");
    assert!(svg.contains("测试卡"));
    assert!(svg.contains("<svg"));

    let png = render_png(&request).expect("render png");
    assert!(png.starts_with(&[0x89, 0x50, 0x4e, 0x47]));
}

#[test]
fn render_uses_bundled_resource_images() {
    let resource_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("yugioh-card");

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: sample_card(),
        options: RenderOptions {
            resource_path: resource_root,
            scale: 1.0,
            ..RenderOptions::default()
        },
    };

    let svg = render_svg(&request).expect("render svg");
    assert!(svg.contains("data:image/png;base64,"));
    assert!(svg.contains("测试卡"));
}

#[test]
fn render_embeds_bundled_font_faces() {
    let resource_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("yugioh-card");

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: sample_card(),
        options: RenderOptions {
            resource_path: resource_root.clone(),
            language: Some("sc".to_string()),
            scale: 1.0,
            ..RenderOptions::default()
        },
    };

    let svg = render_svg(&request).expect("render svg");
    assert!(svg.contains("@font-face"));
    assert!(svg.contains("font-family:'ygo-sc'"));
    assert!(svg.contains("font-family=\"'ygo-atk-def'"));
    assert!(
        resource_root
            .join("yugioh")
            .join("font")
            .join("ygo-sc.woff2")
            .exists()
    );
}

#[test]
fn render_outputs_effect_line_for_monsters() {
    let resource_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("yugioh-card");

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: sample_card(),
        options: RenderOptions {
            resource_path: resource_root,
            language: Some("sc".to_string()),
            scale: 1.0,
            ..RenderOptions::default()
        },
    };

    let svg = render_svg(&request).expect("render svg");
    assert!(svg.contains("【怪兽／效果】"));
    assert!(svg.contains("<tspan x=\"126\""));
    assert!(!svg.contains("<foreignObject"));
}

#[test]
fn render_outputs_link_arrows_and_link_stat_layout() {
    let resource_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("yugioh-card");

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: sample_link_card(),
        options: RenderOptions {
            resource_path: resource_root,
            language: Some("sc".to_string()),
            scale: 1.0,
            ..RenderOptions::default()
        },
    };

    let svg = render_svg(&request).expect("render svg");
    assert!(svg.contains("x=\"555\" y=\"278\""));
    assert!(svg.contains("x=\"1223\" y=\"761\""));
    assert!(svg.contains("x=\"95\" y=\"1336\""));
    assert!(svg.contains("连接测试卡"));
    assert!(svg.contains(">3</text>"));
}

#[test]
fn render_applies_title_compression_and_baseline_positioning() {
    let resource_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("yugioh-card");
    let mut card = sample_card();
    card.name = "超超超超超超超超超超超超超长测试标题".to_string();

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card,
        options: RenderOptions {
            resource_path: resource_root,
            language: Some("sc".to_string()),
            scale: 1.0,
            ..RenderOptions::default()
        },
    };

    let svg = render_svg(&request).expect("render svg");
    assert!(svg.contains("textLength=\"1033\""));
    assert!(svg.contains("y=\"111\""));
    assert!(svg.contains("dominant-baseline=\"text-before-edge\""));
}

#[test]
fn render_preview_image_to_test_artifacts() {
    let resource_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("yugioh-card");
    let artifact_root = artifact_dir();
    let png_path = artifact_root.join("sample-card-preview.png");
    let svg_path = artifact_root.join("sample-card-preview.svg");

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: sample_card(),
        options: RenderOptions {
            resource_path: resource_root,
            language: Some("sc".to_string()),
            scale: 1.0,
            ..RenderOptions::default()
        },
    };

    let svg = render_svg(&request).expect("render svg");
    let png = render_png(&request).expect("render png");

    fs::write(&svg_path, svg).expect("write svg artifact");
    fs::write(&png_path, png).expect("write png artifact");

    assert!(png_path.exists());
    assert!(svg_path.exists());
}
