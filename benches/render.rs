//! Criterion benchmarks for the card rendering pipeline.
//!
//! Run with:
//! ```sh
//! cargo bench
//! # or with HTML report (requires criterion "html_reports" feature):
//! cargo bench -- --output-format html
//! ```
//!
//! The benchmarks require a populated bundle at `resources/yugioh_bundle.bin`.
//! If the file is absent the benchmark group is skipped gracefully.

use std::{fs, path::PathBuf, sync::Once};

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use tiny_skia::{Color, Pixmap};
use ygo_card_renderer_rs::{
    asset_bundle::init_global_bundle,
    model::{CardKind, RenderOptions, RenderRequest, YgoCardMeta},
    renderer::Renderer,
    text::{
        DrawTextLine, RubyMultilineParams, TextAlign, draw_multiline_ruby_text, draw_text_line,
    },
};
use ygopro_cdb_encode_rs::YgoProCdb;

// ─────────────────────────────────────────────────────────────────────────────
// Bundle initialisation
// ─────────────────────────────────────────────────────────────────────────────

static BUNDLE_INIT: Once = Once::new();
static mut BUNDLE_OK: bool = false;

fn try_init_bundle() -> bool {
    BUNDLE_INIT.call_once(|| {
        let bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("yugioh_bundle.bin");
        if let Ok(bytes) = fs::read(&bin_path) {
            if init_global_bundle(&bytes).is_ok() {
                // SAFETY: written once before any reads.
                unsafe { BUNDLE_OK = true };
            }
        }
    });
    // SAFETY: written in call_once before this read.
    unsafe { BUNDLE_OK }
}

// ─────────────────────────────────────────────────────────────────────────────
// bench_draw_text_line
// ─────────────────────────────────────────────────────────────────────────────

fn bench_draw_text_line(c: &mut Criterion) {
    if !try_init_bundle() {
        eprintln!("bench_draw_text_line: bundle unavailable, skipping");
        return;
    }

    let mut group = c.benchmark_group("draw_text_line");
    let sample_texts = [
        ("short", "Hello"),
        ("medium", "青眼の白龍"),
        ("long", "この宇宙の全ての存在に対して支配権を持つ悪の化身。"),
    ];

    for (label, text) in sample_texts {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(label), text, |b, text| {
            b.iter_batched(
                || Pixmap::new(860, 60).unwrap(),
                |mut pixmap| {
                    draw_text_line(
                        &mut pixmap,
                        DrawTextLine::unscaled(
                            text,
                            10.0,
                            4.0,
                            36.0,
                            800.0,
                            Color::BLACK,
                            Color::TRANSPARENT,
                            "ygo-sc",
                            TextAlign::Left,
                            None,
                            0.0,
                        ),
                    );
                    pixmap
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// bench_draw_multiline_ruby_text
// ─────────────────────────────────────────────────────────────────────────────

fn bench_draw_multiline_ruby_text(c: &mut Criterion) {
    if !try_init_bundle() {
        eprintln!("bench_draw_multiline_ruby_text: bundle unavailable, skipping");
        return;
    }

    let text_plain = "このカードは通常召喚できない。自分フィールドのモンスター２体をリリースした場合に特殊召喚できる。";
    let text_ruby =
        "{青眼:せいがん}の{白龍:はくりゅう}。この宇宙の全ての存在に対して支配権を持つ悪の化身。";

    let mut group = c.benchmark_group("draw_multiline_ruby_text");

    for (label, text) in [("plain", text_plain), ("ruby", text_ruby)] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(label), text, |b, text| {
            b.iter_batched(
                || Pixmap::new(620, 340).unwrap(),
                |mut pixmap| {
                    draw_multiline_ruby_text(
                        &mut pixmap,
                        RubyMultilineParams {
                            text,
                            x: 10.0,
                            y: 10.0,
                            width: 600.0,
                            height: 320.0,
                            family: "ygo-sc",
                            color: Color::BLACK,
                            shadow_color: Color::TRANSPARENT,
                            brush: None,
                            shadow_brush: None,
                            language: None,
                            base_font_size: 18,
                            rt_font_size: 9,
                            rt_top: -10.0,
                            rt_font_scale_x: 1.0,
                            line_height: 1.4,
                            letter_spacing: 0.0,
                            min_font_size: 12,
                            first_line_compress: false,
                            align: TextAlign::Left,
                            font_weight: None,
                        },
                    );
                    pixmap
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// bench_render_card
// ─────────────────────────────────────────────────────────────────────────────
//
// Environment variables (all optional; the bench is skipped if unavailable):
//   YGO_BUNDLE   – path to the binary bundle (default: resources/yugioh_bundle.bin)
//   YGO_CDB      – path to the cards.cdb SQLite database
//   YGO_ART_DIR  – directory containing art images named <card_name>.jpg or
//                  <card_name>.png; images are matched case-sensitively;
//                  cards without a matching image are rendered without art.
//
// Criterion will run render_png many times per card.  To keep wall-clock time
// reasonable the bench samples at most SAMPLE_SIZE cards from the database.

const SAMPLE_SIZE: usize = 20;

fn find_art(art_dir: &Option<PathBuf>, card_code: u32) -> Option<PathBuf> {
    let dir = art_dir.as_ref()?;
    for ext in &["jpg", "png"] {
        let path = dir.join(format!("{card_code}.{ext}"));
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn bench_render_card(c: &mut Criterion) {
    if !try_init_bundle() {
        eprintln!("bench_render_card: bundle unavailable, skipping");
        return;
    }

    // ── Load CDB ──────────────────────────────────────────────────────────────
    let cdb_path = match std::env::var_os("YGO_CDB") {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("bench_render_card: YGO_CDB not set, skipping");
            return;
        }
    };
    let db = match YgoProCdb::from_path(&cdb_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("bench_render_card: failed to open CDB: {e}");
            return;
        }
    };
    let all_cards = match db.find_all() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("bench_render_card: find_all failed: {e}");
            return;
        }
    };

    // ── Art directory (optional) ───────────────────────────────────────────────
    let art_dir: Option<PathBuf> = std::env::var_os("YGO_ART_DIR").map(PathBuf::from);

    // ── Sample cards ──────────────────────────────────────────────────────────
    // Take every N-th card so the sample is spread across the whole database.
    let step = (all_cards.len() / SAMPLE_SIZE).max(1);
    let sample: Vec<YgoCardMeta> = all_cards
        .into_iter()
        .step_by(step)
        .take(SAMPLE_SIZE)
        .map(YgoCardMeta::from_entry)
        .collect();

    if sample.is_empty() {
        eprintln!("bench_render_card: no cards found in CDB, skipping");
        return;
    }

    let renderer = Renderer::new();
    let mut group = c.benchmark_group("render_card");
    group.sample_size(10); // render_png is expensive; 10 samples per card is enough

    for card in &sample {
        let art_image = find_art(&art_dir, card.code);
        let options = RenderOptions {
            art_image,
            ..RenderOptions::default()
        };
        let request = RenderRequest {
            kind: CardKind::Yugioh,
            card: card.clone(),
            options,
        };

        let label = format!(
            "{}({})",
            card.name.chars().take(20).collect::<String>(),
            card.code
        );
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(&label), &request, |b, req| {
            b.iter_batched(
                || req.clone(),
                |r| renderer.render_png(&r),
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// Registration
// ─────────────────────────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_draw_text_line,
    bench_draw_multiline_ruby_text,
    bench_render_card
);
criterion_main!(benches);
