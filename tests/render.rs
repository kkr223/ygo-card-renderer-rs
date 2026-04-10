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
