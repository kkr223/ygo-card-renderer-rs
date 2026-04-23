//! Text rendering subsystem.
//!
//! This module is the public façade over four implementation sub-modules:
//!
//! | Sub-module | Responsibility |
//! |---|---|
//! | [`engine`] | Thread-local `TextEngine`, global `FontDB` |
//! | [`util`] | Font-family helpers (`primary_family_name`, `font_weight_for_family`) |
//! | [`measure`] | Width estimation, line wrapping, single-line fitting |
//! | [`draw`] | Pixel-level text rasterisation (shadowed, multi-line) |
//! | [`ruby`] | Ruby / furigana annotation rendering |
//!
//! All public items that `renderer.rs` (and any future callers) need are
//! re-exported here so the import path stays `crate::text::*`.

mod engine;
mod util;
pub mod measure;
pub mod draw;
pub mod ruby;

// ── measure ──────────────────────────────────────────────────────────────────
pub use measure::{
    estimate_text_width,
    fit_single_line,
    fit_single_line_compressed,
};

// ── draw ─────────────────────────────────────────────────────────────────────
pub use draw::{
    TextAlign,
    DrawTextLine,
    draw_text_line,
    draw_text_line_scaled,
    draw_multiline_text,
};

// ── ruby ─────────────────────────────────────────────────────────────────────
pub use ruby::{
    RubyLineParams,
    RubyMultilineParams,
    fit_ruby_text_scale,
    draw_ruby_text_line,
    draw_multiline_ruby_text,
};
