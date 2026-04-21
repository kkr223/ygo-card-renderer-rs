use std::{fs, path::PathBuf};
use std::sync::Once;

use ygo_card_renderer_rs::{
    CardKind, RenderOptions, RenderRequest, Renderer, asset_bundle::init_global_bundle,
    model::{LayoutOverrides, YgoCardMeta},
};
use ygopro_cdb_encode_rs::YgoProCdb;

fn init_bundle() {
    static INIT: Once = Once::new();
    let bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("yugioh_bundle.bin");

    INIT.call_once(|| {
        if let Ok(bytes) = fs::read(&bin_path) {
            init_global_bundle(&bytes).expect("initialize yugioh bundle");
        } else {
            panic!("Missing yugioh_bundle.bin at {:?}", bin_path);
        }
    });
}

fn artifact_dir() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("export");
    fs::create_dir_all(&path).expect("create artifact dir");
    path
}

fn env_opt_u32(key: &str) -> Option<u32> {
    std::env::var(key).ok().and_then(|v| v.parse::<u32>().ok())
}

fn env_opt_i32(key: &str) -> Option<i32> {
    std::env::var(key).ok().and_then(|v| v.parse::<i32>().ok())
}

fn env_opt_f32(key: &str) -> Option<f32> {
    std::env::var(key).ok().and_then(|v| v.parse::<f32>().ok())
}

fn env_opt_bool(key: &str) -> Option<bool> {
    std::env::var(key).ok().and_then(|v| {
        match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

fn env_string(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_opt_string(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn layout_overrides_from_env() -> LayoutOverrides {
    LayoutOverrides {
        name_top: env_opt_u32("YGO_NAME_TOP"),
        name_size: env_opt_u32("YGO_NAME_SIZE"),
        name_x: env_opt_u32("YGO_NAME_X"),
        title_max_width_with_attribute: env_opt_u32("YGO_TITLE_MAX_WIDTH_WITH_ATTRIBUTE"),
        title_max_width_without_attribute: env_opt_u32("YGO_TITLE_MAX_WIDTH_WITHOUT_ATTRIBUTE"),
        title_letter_spacing: env_opt_f32("YGO_TITLE_LETTER_SPACING"),
        type_top: env_opt_u32("YGO_TYPE_TOP"),
        type_size: env_opt_u32("YGO_TYPE_SIZE"),
        type_letter_spacing: env_opt_f32("YGO_TYPE_LETTER_SPACING"),
        effect_top: env_opt_u32("YGO_EFFECT_TOP"),
        effect_size: env_opt_u32("YGO_EFFECT_SIZE"),
        effect_line_height: env_opt_f32("YGO_EFFECT_LINE_HEIGHT"),
        effect_x: env_opt_u32("YGO_EFFECT_X"),
        effect_letter_spacing: env_opt_f32("YGO_EFFECT_LETTER_SPACING"),
        effect_text_indent: env_opt_i32("YGO_EFFECT_TEXT_INDENT"),
        description_size: env_opt_u32("YGO_DESCRIPTION_SIZE"),
        description_line_height: env_opt_f32("YGO_DESCRIPTION_LINE_HEIGHT"),
        description_x: env_opt_u32("YGO_DESCRIPTION_X"),
        description_letter_spacing: env_opt_f32("YGO_DESCRIPTION_LETTER_SPACING"),
        body_max_width: env_opt_u32("YGO_BODY_MAX_WIDTH"),
        pendulum_description_top: env_opt_u32("YGO_PENDULUM_DESCRIPTION_TOP"),
        pendulum_description_size: env_opt_u32("YGO_PENDULUM_DESCRIPTION_SIZE"),
        stat_atk_x: env_opt_u32("YGO_STAT_ATK_X"),
        stat_def_x: env_opt_u32("YGO_STAT_DEF_X"),
        stat_link_x: env_opt_u32("YGO_STAT_LINK_X"),
        stat_top: env_opt_u32("YGO_STAT_TOP"),
        stat_size: env_opt_u32("YGO_STAT_SIZE"),
        stat_letter_spacing: env_opt_f32("YGO_STAT_LETTER_SPACING"),
        link_top: env_opt_u32("YGO_LINK_TOP"),
        link_size: env_opt_u32("YGO_LINK_SIZE"),
        copyright_right: env_opt_u32("YGO_COPYRIGHT_RIGHT"),
        copyright_y: env_opt_u32("YGO_COPYRIGHT_Y"),
        package_y: env_opt_u32("YGO_PACKAGE_Y"),
        package_y_pendulum: env_opt_u32("YGO_PACKAGE_Y_PENDULUM"),
        package_y_link: env_opt_u32("YGO_PACKAGE_Y_LINK"),
        password_x: env_opt_u32("YGO_PASSWORD_X"),
        password_y: env_opt_u32("YGO_PASSWORD_Y"),
    }
}

fn pick_tuning_card(cards: &[ygopro_cdb_encode_rs::CardDataEntry]) -> ygopro_cdb_encode_rs::CardDataEntry {
    if let Some(code) = env_opt_u32("YGO_RENDER_CARD_CODE") {
        if let Some(card) = cards.iter().find(|card| card.code == code) {
            return card.clone();
        }
        panic!("Could not find card with code {}", code);
    }

    if let Some(name_query) = env_opt_string("YGO_RENDER_CARD_NAME") {
        let query = name_query.to_lowercase();
        if let Some(card) = cards
            .iter()
            .find(|card| card.name.to_lowercase().contains(&query))
        {
            return card.clone();
        }
        panic!("Could not find card matching name query {:?}", name_query);
    }

    cards
        .iter()
        .find(|card| !card.desc.trim().is_empty() && !card.is_spell() && !card.is_trap())
        .cloned()
        .or_else(|| cards.first().cloned())
        .expect("cards.cdb did not contain any cards")
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
            card: card.clone().into(),
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

/// 手动调渲染参数用：
/// `cargo test render_single_card_for_tuning -- --ignored --nocapture`
///
/// 常用环境变量示例：
/// `$env:YGO_RENDER_CARD_CODE="46986414"`
/// `$env:YGO_RENDER_CARD_NAME="blue eyes"`
/// `$env:YGO_DESCRIPTION_SIZE="34"`
/// `$env:YGO_EFFECT_TOP="1516"`
/// `$env:YGO_RENDER_LABEL="desc-34"`
#[test]
#[ignore = "manual tuning helper; run explicitly when adjusting render parameters"]
fn render_single_card_for_tuning() {
    init_bundle();

    let cdb_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cards.cdb");
    if !cdb_path.exists() {
        eprintln!("Skipping tuning test: cards.cdb not found at {:?}", cdb_path);
        return;
    }

    let cdb = YgoProCdb::from_path(&cdb_path).expect("open cdb");
    let mut cards = cdb.find_all().expect("read all cards from cdb");
    cards.sort_by_key(|card| card.code);

    let card = pick_tuning_card(&cards);
    let label = env_string("YGO_RENDER_LABEL", "tuning");
    let language = env_string("YGO_LANGUAGE", "sc");
    let scale = std::env::var("YGO_SCALE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0);
    let art_image = env_opt_string("YGO_ART_IMAGE").map(PathBuf::from);
    let layout_overrides = layout_overrides_from_env();
    let title_width_compress = env_opt_bool("YGO_TITLE_WIDTH_COMPRESS").unwrap_or(false);
    let description_first_line_compress =
        env_opt_bool("YGO_DESCRIPTION_FIRST_LINE_COMPRESS").unwrap_or(false);
    let copyright_text = env_opt_string("YGO_COPYRIGHT_TEXT");
    let package_text = env_opt_string("YGO_PACKAGE_TEXT");

    println!("Selected card: [{}] {}", card.code, card.name);
    println!("language={language}, scale={scale}, label={label}");
    println!(
        "title_width_compress={title_width_compress}, description_first_line_compress={description_first_line_compress}"
    );
    println!("copyright_text={copyright_text:?}, package_text={package_text:?}");
    println!("layout_overrides={layout_overrides:#?}");

    let mut card_meta: YgoCardMeta = card.clone().into();
    if copyright_text.is_some() {
        card_meta.copyright = copyright_text;
    }
    if package_text.is_some() {
        card_meta.package = package_text;
    }

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: card_meta,
        options: RenderOptions {
            resource_path: PathBuf::new(),
            language: Some(language.clone()),
            scale,
            art_image,
            title_width_compress,
            description_first_line_compress,
            layout_overrides,
            ..RenderOptions::default()
        },
    };

    let renderer = Renderer::new();
    let png = renderer.render_png(&request).expect("render png");

    let out_dir = artifact_dir();
    let png_path = out_dir.join(format!(
        "{}-{}-{}.png",
        card.code,
        sanitize_name(&card.name),
        sanitize_name(&label)
    ));
    fs::write(&png_path, &png).expect("write png");

    println!("Rendered tuning image to {:?} ({} bytes)", png_path, png.len());
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
