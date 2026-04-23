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

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use tiny_skia::{Color, Pixmap};
use ygo_card_renderer_rs::{
    asset_bundle::init_global_bundle,
    text::{
        draw_multiline_ruby_text, draw_text_line, DrawTextLine, RubyMultilineParams, TextAlign,
    },
};

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

    let text_plain =
        "このカードは通常召喚できない。自分フィールドのモンスター２体をリリースした場合に特殊召喚できる。";
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
                            language: None,
                            base_font_size: 18,
                            rt_font_size: 9,
                            rt_top: -10.0,
                            rt_font_scale_x: 1.0,
                            line_height: 1.4,
                            letter_spacing: 0.0,
                            min_font_size: 12,
                            first_line_compress: false,
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
// Registration
// ─────────────────────────────────────────────────────────────────────────────

criterion_group!(benches, bench_draw_text_line, bench_draw_multiline_ruby_text);
criterion_main!(benches);
