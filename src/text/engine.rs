//! Thread-local `TextEngine` and the global pre-built font database.
//!
//! The [`TextEngine`] bundles together a `cosmic_text::FontSystem`, a
//! `SwashCache`, and the project's own [`GlyphWidthCache`].  One engine lives
//! per OS thread via a `thread_local!` cell; callers reach it through the
//! module-private [`with_text_engine`] helper.
//!
//! The font database ([`GLOBAL_FONT_DB`]) is built **once** on first access:
//! WOFF2 fonts are decoded, loaded into a `fontdb::Database`, and then
//! reference-counted clones are handed to each thread's `FontSystem`.  This
//! eliminates per-thread WOFF2 decoding and filesystem scanning.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::OnceLock;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, Weight};

use crate::asset_bundle::get_bundle;

// ─────────────────────────────────────────────────────────────────────────────
// Glyph-advance cache
// ─────────────────────────────────────────────────────────────────────────────

/// Reference font size used for normalised glyph-advance caching.
///
/// All advance measurements are stored as `advance / REF_FONT_SIZE`.  To get
/// the advance at an arbitrary size, callers multiply by the target size.
/// This lets different font sizes share a single cache entry per `(char, family)`.
pub(super) const REF_FONT_SIZE: f32 = 100.0;

pub(super) struct GlyphWidthCache {
    family_ids: HashMap<String, u32>,
    next_id: u32,
    /// `(char, family_id)` → normalised advance (advance_at_ref / REF_FONT_SIZE).
    advances: HashMap<(char, u32), f32>,
}

impl GlyphWidthCache {
    pub(super) fn new() -> Self {
        Self {
            family_ids: HashMap::new(),
            next_id: 0,
            advances: HashMap::new(),
        }
    }

    /// Returns the numeric id for `name`, inserting one if not yet known.
    pub(super) fn family_id(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.family_ids.get(name) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.family_ids.insert(name.to_string(), id);
        id
    }

    pub(super) fn get(&self, ch: char, family_id: u32) -> Option<f32> {
        self.advances.get(&(ch, family_id)).copied()
    }

    pub(super) fn insert(&mut self, ch: char, family_id: u32, normalised_advance: f32) {
        self.advances.insert((ch, family_id), normalised_advance);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TextEngine
// ─────────────────────────────────────────────────────────────────────────────

pub struct TextEngine {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub(super) glyph_cache: GlyphWidthCache,
}

impl TextEngine {
    /// Returns `(sum_of_glyph_advances, char_count)` for `text`.
    ///
    /// Letter-spacing is intentionally **excluded** so callers can apply it
    /// with the correct span:
    /// ```text
    /// rendered_width = raw + letter_spacing * (count - 1)
    /// ```
    ///
    /// Advances are cached at [`REF_FONT_SIZE`] and scaled linearly, so
    /// the same character at different sizes shares a single cache entry.
    pub(crate) fn measure_raw_advances(
        &mut self,
        text: &str,
        family_name: &str,
        font_size: f32,
    ) -> (f32, usize) {
        let resolved = super::util::primary_family_name(family_name);
        let weight = super::util::font_weight_for_family(resolved.as_str());
        let family_id = self.glyph_cache.family_id(resolved.as_str());

        let mut raw = 0.0_f32;
        let mut count = 0usize;

        for ch in text.chars() {
            let normalised = match self.glyph_cache.get(ch, family_id) {
                Some(n) => n,
                None => {
                    let ref_advance = measure_char_advance(
                        &mut self.font_system,
                        ch,
                        resolved.as_str(),
                        REF_FONT_SIZE,
                        weight,
                    );
                    let n = ref_advance / REF_FONT_SIZE;
                    self.glyph_cache.insert(ch, family_id, n);
                    n
                }
            };
            raw += normalised * font_size;
            count += 1;
        }

        (raw, count)
    }
}

/// Measure the advance width of a single character by building a minimal
/// `cosmic_text::Buffer`.  This is the slow path; results are cached by
/// [`TextEngine::measure_raw_advances`] as normalised advances, so each
/// `(char, family)` pair only passes through here once.
fn measure_char_advance(
    font_system: &mut FontSystem,
    ch: char,
    family: &str,
    font_size: f32,
    weight: Weight,
) -> f32 {
    let mut tmp = [0u8; 4];
    let s = ch.encode_utf8(&mut tmp);

    let metrics = Metrics::new(font_size, font_size);
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(font_system, None, Some(font_size * 2.0));

    let attrs = Attrs::new().family(Family::Name(family)).weight(weight);
    buffer.set_text(font_system, s, &attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, true);

    let mut advance = 0.0_f32;
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            advance += glyph.w;
        }
    }
    advance
}

// ─────────────────────────────────────────────────────────────────────────────
// Global font database & thread-local engine
// ─────────────────────────────────────────────────────────────────────────────

/// A fully-populated `fontdb::Database` built **once** (WOFF2→TTF decode +
/// `load_font_data`).  Each thread-local [`TextEngine`] clones this database
/// — the clone is cheap because font binary data is reference-counted.
static GLOBAL_FONT_DB: OnceLock<cosmic_text::fontdb::Database> = OnceLock::new();

fn get_global_font_db() -> &'static cosmic_text::fontdb::Database {
    GLOBAL_FONT_DB.get_or_init(|| {
        let bundle = get_bundle();
        let mut db = cosmic_text::fontdb::Database::new();
        for font_meta in bundle.index.fonts.values() {
            if let Ok(bytes) = bundle.get_bytes(&font_meta.buffer) {
                let font_data = if ygo_woff2::is_woff2(bytes) {
                    match ygo_woff2::convert_woff2_to_ttf(&mut Cursor::new(bytes)) {
                        Ok(ttf) => ttf,
                        Err(e) => {
                            eprintln!("woff2 decode failed for {:?}: {}", font_meta.buffer, e);
                            continue;
                        }
                    }
                } else {
                    bytes.to_vec()
                };
                db.load_font_data(font_data);
            }
        }
        // Skip load_system_fonts() — all required fonts are bundled.
        db.set_sans_serif_family("ygo-sc");
        db.set_serif_family("ygo-sc");
        db
    })
}

fn build_text_engine() -> TextEngine {
    let db = get_global_font_db().clone();
    let font_system = FontSystem::new_with_locale_and_db("zh-CN".to_string(), db);
    TextEngine {
        font_system,
        swash_cache: SwashCache::new(),
        glyph_cache: GlyphWidthCache::new(),
    }
}

thread_local! {
    static TEXT_ENGINE: RefCell<TextEngine> = RefCell::new(build_text_engine());
}

/// Run `f` with exclusive access to the current thread's [`TextEngine`].
pub(super) fn with_text_engine<R>(f: impl FnOnce(&mut TextEngine) -> R) -> R {
    TEXT_ENGINE.with(|engine| f(&mut engine.borrow_mut()))
}
