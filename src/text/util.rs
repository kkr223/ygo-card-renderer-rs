//! Font-family resolution and weight lookup utilities.
//!
//! These functions are shared by [`super::engine`], [`super::measure`], and
//! [`super::draw`].

use cosmic_text::Weight;

/// Resolve a CSS font-family stack to the first concrete family name that the
/// bundled fonts actually contain.
///
/// Generic names (`sans-serif`, `serif`, `monospace`) are skipped; the
/// project-level aliases `custom1`/`custom2` are mapped to their internal
/// names.  Falls back to `"ygo-sc"` when no concrete name is found.
pub(super) fn primary_family_name(stack: &str) -> String {
    let family = match stack
        .split(',')
        .map(|part| part.trim().trim_matches('\'').trim_matches('"'))
        .find(|name| !name.is_empty() && !matches!(*name, "sans-serif" | "serif" | "monospace"))
    {
        Some("custom1") => "ygo-custom1",
        Some("custom2") => "ygo-custom2",
        Some(other) => other,
        None => "ygo-sc",
    };

    family.to_string()
}

/// Returns the `Weight` appropriate for the given resolved family name.
pub(super) fn font_weight_for_family(family: &str) -> Weight {
    match family {
        "ygo-atk-def" => Weight::BOLD,
        "ygo-password" => Weight::MEDIUM,
        other if other.starts_with("rd-") => Weight::MEDIUM,
        _ => Weight::NORMAL,
    }
}
