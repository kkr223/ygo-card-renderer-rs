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

pub mod draw;
mod engine;
pub mod measure;
pub mod ruby;
mod util;

// ── measure ──────────────────────────────────────────────────────────────────
pub use measure::{estimate_text_width, fit_single_line, fit_single_line_compressed};

// ── draw ─────────────────────────────────────────────────────────────────────
pub use draw::{
    DrawTextLine, TextAlign, TextBrush, draw_multiline_text, draw_text_line,
};

// ── ruby ─────────────────────────────────────────────────────────────────────
pub use ruby::{
    RubyLineParams, RubyMultilineParams, draw_multiline_ruby_text, draw_ruby_text_line,
    fit_ruby_text_scale,
};
