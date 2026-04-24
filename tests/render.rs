use std::sync::Once;
use std::time::{Duration, Instant};
use std::{fs, path::PathBuf};

use ygo_card_renderer_rs::{
    CardKind, RenderOptions, RenderRequest, Renderer,
    asset_bundle::init_global_bundle,
    model::{LayoutOverrides, OutFrameEffectBox, PositionedRenderImage, YgoCardMeta},
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

fn test_cdb_path() -> Option<PathBuf> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for filename in ["cards2.cdb", "cards3.cdb", "cards.cdb"] {
        let path = repo_root.join(filename);
        if path.exists() {
            return Some(path);
        }
    }

    None
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
    std::env::var(key)
        .ok()
        .and_then(|v| match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
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

/// Return the art image path for a card from `YGO_ART_DIR`, if set and found.
/// Tries `<dir>/<code>.jpg` then `<dir>/<code>.png`; returns `None` if either
/// the env var is unset or no matching file exists.
fn find_art(card_code: u32) -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("YGO_ART_DIR")?);
    for ext in &["jpg", "png"] {
        let path = dir.join(format!("{card_code}.{ext}"));
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn foreground_image_from_env() -> Option<PositionedRenderImage> {
    Some(PositionedRenderImage {
        path: PathBuf::from(env_opt_string("YGO_FOREGROUND_IMAGE")?),
        x: env_opt_i32("YGO_FOREGROUND_X").unwrap_or(0),
        y: env_opt_i32("YGO_FOREGROUND_Y").unwrap_or(0),
    })
}

fn out_frame_image_from_env() -> Option<PositionedRenderImage> {
    Some(PositionedRenderImage {
        path: PathBuf::from(env_opt_string("YGO_OUT_FRAME_IMAGE")?),
        x: env_opt_i32("YGO_OUT_FRAME_X").unwrap_or(0),
        y: env_opt_i32("YGO_OUT_FRAME_Y").unwrap_or(0),
    })
}

fn env_opt_out_frame_effect_box(key: &str) -> Option<OutFrameEffectBox> {
    let value = env_opt_string(key)?.to_ascii_lowercase();
    match value.as_str() {
        "eblock-border" | "original" | "origin" | "normal" => Some(OutFrameEffectBox::EblockBorder),
        "eblock-border-o" | "colored" | "colour" | "color" | "o" => {
            Some(OutFrameEffectBox::EblockBorderO)
        }
        _ => panic!(
            "{key} must be eblock-border or eblock-border-o, got {:?}",
            value
        ),
    }
}

fn apply_out_frame_env(card_meta: &mut YgoCardMeta) {
    let out_frame_image = out_frame_image_from_env();
    if let Some(enabled) = env_opt_bool("YGO_OUT_FRAME") {
        card_meta.out_frame = enabled;
    } else if out_frame_image.is_some() {
        card_meta.out_frame = true;
    }

    card_meta.out_frame_image = out_frame_image;

    if let Some(enabled) = env_opt_bool("YGO_OUT_FRAME_EFFECT_ENABLED") {
        card_meta.out_frame_effect_enabled = enabled;
    }
    if let Some(effect_box) = env_opt_out_frame_effect_box("YGO_OUT_FRAME_EFFECT_BOX") {
        card_meta.out_frame_effect_box = effect_box;
    }
    if let Some(color) = env_opt_string("YGO_OUT_FRAME_EFFECT_BACKGROUND_COLOR") {
        card_meta.out_frame_effect_background_color = Some(color);
    }
    if let Some(opacity) = env_opt_f32("YGO_OUT_FRAME_EFFECT_OPACITY") {
        card_meta.out_frame_effect_opacity = Some(opacity);
    }
    if let Some(enabled) = env_opt_bool("YGO_OUT_FRAME_NAME_BLOCK_ENABLED") {
        card_meta.out_frame_name_block_enabled = enabled;
    }
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

fn pick_tuning_card(
    cards: &[ygopro_cdb_encode_rs::CardDataEntry],
) -> ygopro_cdb_encode_rs::CardDataEntry {
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
        .expect("selected cdb did not contain any cards")
}

/// 单卡绘制测试。
///
/// 运行方式：
/// `cargo test render_single_card_from_cdb -- --nocapture`
///
/// 常用环境变量：
/// `$env:YGO_RENDER_CARD_CODE="41546"`
/// `$env:YGO_RENDER_CARD_NAME="托马斯"`
/// `$env:YGO_RENDER_LABEL="out-frame"`
/// `$env:YGO_ART_IMAGE="D:\path\art.png"`
/// `$env:YGO_OUT_FRAME="true"`
/// `$env:YGO_OUT_FRAME_IMAGE="D:\path\foreground.png"`
/// `$env:YGO_OUT_FRAME_X="0"`
/// `$env:YGO_OUT_FRAME_Y="0"`
/// `$env:YGO_OUT_FRAME_EFFECT_ENABLED="true"`
/// `$env:YGO_OUT_FRAME_EFFECT_BOX="eblock-border-o"`
/// `$env:YGO_OUT_FRAME_EFFECT_BACKGROUND_COLOR="#ffffff"`
/// `$env:YGO_OUT_FRAME_EFFECT_OPACITY="0.75"`
/// `$env:YGO_OUT_FRAME_NAME_BLOCK_ENABLED="true"`
#[test]
fn render_single_card_from_cdb() {
    init_bundle();

    let Some(cdb_path) = test_cdb_path() else {
        eprintln!("Skipping single-card render test: no CDB exists in repo root");
        return;
    };

    let cdb = YgoProCdb::from_path(&cdb_path).expect("open cdb");
    let mut cards = cdb.find_all().expect("read all cards from cdb");
    cards.sort_by_key(|card| card.code);

    let card = pick_tuning_card(&cards);
    let label = env_string("YGO_RENDER_LABEL", "single");
    let language = env_string("YGO_LANGUAGE", "sc");
    let art_image = env_opt_string("YGO_ART_IMAGE")
        .map(PathBuf::from)
        .or_else(|| find_art(card.code));
    let foreground_image = foreground_image_from_env();
    let mut card_meta: YgoCardMeta = card.clone().into();
    apply_out_frame_env(&mut card_meta);

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: card_meta,
        options: RenderOptions {
            language: Some(language.clone()),
            scale: 1.0,
            art_image,
            foreground_image,
            ..RenderOptions::default()
        },
    };

    let renderer = Renderer::new();
    let png = renderer.render_png(&request).expect("render png");

    let out_dir = artifact_dir();
    let png_path = out_dir.join(format!(
        "single-{}-{}-{}.png",
        card.code,
        sanitize_name(&card.name),
        sanitize_name(&label)
    ));
    fs::write(&png_path, &png).expect("write png");

    println!("Selected CDB: {:?}", cdb_path);
    println!("Selected card: [{}] {}", card.code, card.name);
    println!("language={language}, label={label}");
    println!(
        "Rendered single-card image to {:?} ({} bytes)",
        png_path,
        png.len()
    );
    assert!(
        !png.is_empty(),
        "single-card render should produce png bytes"
    );
}

/// 从 CDB 中读取几张经典卡，覆盖各种类型。
#[test]
fn render_cards_from_cdb() {
    init_bundle();

    let Some(cdb_path) = test_cdb_path() else {
        eprintln!("Skipping CDB test: neither cards3.cdb nor cards.cdb exists in repo root");
        return;
    };

    let cdb = YgoProCdb::from_path(&cdb_path).expect("open cdb");
    let mut cards = cdb.find_all().expect("read all cards from cdb");
    cards.sort_by_key(|card| card.code);

    let renderer = Renderer::new();
    let out_dir = artifact_dir();

    assert!(
        !cards.is_empty(),
        "selected cdb did not contain any cards to render"
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
                language: Some("sc".to_string()),
                scale: 1.0,
                art_image: find_art(card.code),
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

    let Some(cdb_path) = test_cdb_path() else {
        eprintln!("Skipping tuning test: neither cards3.cdb nor cards.cdb exists in repo root");
        return;
    };

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
    let foreground_image = foreground_image_from_env();
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
    apply_out_frame_env(&mut card_meta);
    if copyright_text.is_some() {
        card_meta.copyright = copyright_text;
    }
    if package_text.is_some() {
        card_meta.package = package_text;
    }
    card_meta.scale = Some(scale);

    let request = RenderRequest {
        kind: CardKind::Yugioh,
        card: card_meta,
        options: RenderOptions {
            language: Some(language.clone()),
            scale: 1.0,
            art_image,
            foreground_image,
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

    println!(
        "Rendered tuning image to {:?} ({} bytes)",
        png_path,
        png.len()
    );
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

/// 大批量渲染性能测试。
///
/// 运行方式：
///   `cargo test bench_bulk_render -- --ignored --nocapture`
///
/// 控制参数（环境变量）：
///   YGO_BENCH_REPEAT   每张卡重复渲染次数（默认 3）
///   YGO_BENCH_THREADS  并行线程数（默认 1，单线程）；设为 0 使用所有 CPU 核心
///   YGO_BENCH_WRITE    是否将 PNG 写入磁盘（默认 false，纯内存测速）
#[test]
#[ignore = "performance benchmark; run explicitly with --ignored"]
fn bench_bulk_render() {
    use std::sync::Arc;
    use std::thread;

    init_bundle();

    let Some(cdb_path) = test_cdb_path() else {
        eprintln!("Skipping bench: neither cards3.cdb nor cards.cdb exists in repo root");
        return;
    };

    let repeat: usize = std::env::var("YGO_BENCH_REPEAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    let thread_count: usize = {
        let raw = std::env::var("YGO_BENCH_THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1);
        if raw == 0 {
            thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        } else {
            raw
        }
    };

    let write_png = std::env::var("YGO_BENCH_WRITE")
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);

    let cdb = YgoProCdb::from_path(&cdb_path).expect("open cdb");
    let mut cards = cdb.find_all().expect("read all cards from cdb");
    cards.sort_by_key(|card| card.code);

    assert!(!cards.is_empty(), "selected cdb contains no cards");

    let total_renders = cards.len() * repeat;
    let out_dir = if write_png {
        Some(artifact_dir())
    } else {
        None
    };

    println!("\n=== bench_bulk_render ===");
    println!("  cards in CDB  : {}", cards.len());
    println!("  repeat        : {repeat}");
    println!("  total renders : {total_renders}");
    println!("  threads       : {thread_count}");
    println!("  write PNG     : {write_png}");
    println!();

    // Build the full work list: (card, repeat_index)
    let work: Arc<Vec<_>> = Arc::new(
        cards
            .iter()
            .flat_map(|c| std::iter::repeat_n(c.clone(), repeat))
            .collect(),
    );
    let out_dir = Arc::new(out_dir);

    let wall_start = Instant::now();

    if thread_count <= 1 {
        // ── single-threaded ─────────────────────────────────────────────────
        let renderer = Renderer::new();
        let mut per_card: Vec<Duration> = Vec::with_capacity(work.len());

        for (i, card) in work.iter().enumerate() {
            let request = RenderRequest {
                kind: CardKind::Yugioh,
                card: card.clone().into(),
                options: RenderOptions {
                    language: Some("sc".to_string()),
                    scale: 1.0,
                    art_image: find_art(card.code),
                    ..RenderOptions::default()
                },
            };

            let t = Instant::now();
            let png = renderer.render_png(&request).expect("render_png failed");
            per_card.push(t.elapsed());

            if let Some(dir) = out_dir.as_ref() {
                let name = sanitize_name(&card.name);
                let path = dir.join(format!("bench-{i:05}-{}-{name}.png", card.code));
                fs::write(path, &png).expect("write png");
            }
        }

        let wall = wall_start.elapsed();
        print_bench_stats(&per_card, wall, total_renders);
    } else {
        // ── multi-threaded ──────────────────────────────────────────────────
        let chunk_size = (work.len() + thread_count - 1) / thread_count;
        let mut handles = Vec::with_capacity(thread_count);

        for chunk in work.chunks(chunk_size) {
            let chunk: Vec<_> = chunk.to_vec();
            let out_dir = Arc::clone(&out_dir);
            let offset = handles.len() * chunk_size;

            let handle = thread::spawn(move || {
                let renderer = Renderer::new();
                let mut durations = Vec::with_capacity(chunk.len());

                for (j, card) in chunk.iter().enumerate() {
                    let request = RenderRequest {
                        kind: CardKind::Yugioh,
                        card: card.clone().into(),
                        options: RenderOptions {
                            language: Some("sc".to_string()),
                            scale: 1.0,
                            art_image: find_art(card.code),
                            ..RenderOptions::default()
                        },
                    };

                    let t = Instant::now();
                    let png = renderer.render_png(&request).expect("render_png failed");
                    durations.push(t.elapsed());

                    if let Some(dir) = out_dir.as_ref() {
                        let i = offset + j;
                        let name = sanitize_name(&card.name);
                        let path = dir.join(format!("bench-{i:05}-{}-{name}.png", card.code));
                        fs::write(path, &png).expect("write png");
                    }
                }

                durations
            });

            handles.push(handle);
        }

        let per_card: Vec<Duration> = handles
            .into_iter()
            .flat_map(|h| h.join().expect("thread panicked"))
            .collect();

        let wall = wall_start.elapsed();
        print_bench_stats(&per_card, wall, total_renders);
    }
}

fn print_bench_stats(per_card: &[Duration], wall: Duration, total: usize) {
    let total_cpu_ms: f64 = per_card.iter().map(|d| d.as_secs_f64() * 1000.0).sum();
    let mean_ms = total_cpu_ms / total as f64;

    let mut sorted = per_card.to_vec();
    sorted.sort_unstable();
    let p50 = sorted[sorted.len() / 2].as_secs_f64() * 1000.0;
    let p95 = sorted[(sorted.len() as f64 * 0.95) as usize].as_secs_f64() * 1000.0;
    let p99 = sorted[(sorted.len() as f64 * 0.99).min((sorted.len() - 1) as f64) as usize]
        .as_secs_f64()
        * 1000.0;
    let min_ms = sorted.first().unwrap().as_secs_f64() * 1000.0;
    let max_ms = sorted.last().unwrap().as_secs_f64() * 1000.0;
    let throughput = total as f64 / wall.as_secs_f64();

    println!("=== results ===");
    println!("  wall time     : {:.2}s", wall.as_secs_f64());
    println!("  throughput    : {throughput:.1} cards/s");
    println!("  mean          : {mean_ms:.1}ms");
    println!("  min           : {min_ms:.1}ms");
    println!("  p50           : {p50:.1}ms");
    println!("  p95           : {p95:.1}ms");
    println!("  p99           : {p99:.1}ms");
    println!("  max           : {max_ms:.1}ms");
    println!();
}
