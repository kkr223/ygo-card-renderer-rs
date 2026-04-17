use std::{fs, path::PathBuf};

use ygo_card_renderer_rs::{
    CardKind, RenderOptions, RenderRequest, Renderer, asset_bundle::init_global_bundle,
};
use ygopro_cdb_encode_rs::YgoProCdb;

fn init_bundle() {
    let bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("ygo_assets.bin");

    if let Ok(bytes) = fs::read(&bin_path) {
        let _ = init_global_bundle(&bytes);
    } else {
        panic!("Missing ygo_assets.bin at {:?}", bin_path);
    }
}

fn artifact_dir() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("export");
    fs::create_dir_all(&path).expect("create artifact dir");
    path
}

/// 从 CDB 中读取几张经典卡，覆盖各种类型。
#[test]
fn render_cards_from_cdb() {
    init_bundle();

    let cdb_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cards.cdb");
    if !cdb_path.exists() {
        eprintln!("Skipping CDB test: cards.cdb not found at {:?}", cdb_path);
        return;
    }

    let cdb = YgoProCdb::from_path(&cdb_path).expect("open cdb");
    let mut cards = cdb.find_all().expect("read all cards from cdb");
    cards.sort_by_key(|card| card.code);

    let renderer = Renderer::new();
    let out_dir = artifact_dir();

    assert!(
        !cards.is_empty(),
        "cards.cdb did not contain any cards to render"
    );

    let mut rendered = 0;
    for card in cards {
        println!(
            "[{}] {} type_=0x{:X} level={} attr=0x{:X}",
            card.code, card.name, card.type_, card.level, card.attribute
        );

        let request = RenderRequest {
            kind: CardKind::Yugioh,
            card: card.clone(),
            options: RenderOptions {
                resource_path: PathBuf::new(),
                language: Some("sc".to_string()),
                scale: 1.0,
                art_image: None,
                ..RenderOptions::default()
            },
        };

        let png = renderer.render_png(&request).expect("render png");
        let png_path = out_dir.join(format!("{}-{}.png", card.code, sanitize_name(&card.name)));
        fs::write(&png_path, &png).expect("write png");
        println!("  → Rendered to {:?} ({} bytes)", png_path, png.len());
        rendered += 1;
    }

    println!("\nRendered {rendered} cards successfully.");
    assert!(rendered > 0, "Should have rendered at least one card");
}

fn sanitize_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch,
            _ => '_',
        })
        .collect();

    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "card".to_string()
    } else {
        trimmed.to_string()
    }
}
