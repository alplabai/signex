//! Font family identity for text-bearing document items.
//!
//! A document stores only a family *name*. Whether that family is installed
//! on the machine that opens the file is a render-time question answered by
//! the app, never written back: opening and saving on a machine that lacks
//! the font must not destroy the author's choice. This module owns the part
//! that is pure data: the bundled fallback and the order a name is picked in.

use std::sync::OnceLock;

/// Family every renderer falls back to. It ships inside the app, so it
/// always exists, on every machine, which is what makes the fallback
/// deterministic.
pub const BUNDLED_DEFAULT_FAMILY: &str = "Iosevka";

/// Millimetres of glyph height per em for a face we know nothing about
/// (a system family, or a bundled face with no cap height).
///
/// A text item's `size_mm` is the height its capitals should measure on the
/// sheet; renderers are sized in em. The two differ by the face's cap height
/// over its em, which is why this is only the *fallback*: use [`mm_per_em`].
pub const FALLBACK_MM_PER_EM: f32 = 0.72;

/// OS/2 `sCapHeight / unitsPerEm` of the bundled faces, by family name.
/// The bold cuts share their regular cut's capital height. A test in
/// `signex-output` pins these to the font files.
const BUNDLED_CAP_HEIGHT_PER_EM: [(&str, f32); 2] =
    [("Iosevka", 735.0 / 1000.0), ("Roboto", 1456.0 / 2048.0)];

/// Reads the cap height over em of an installed family, `None` when the
/// family is unknown or the reader is not ready yet.
pub type CapHeightResolver = fn(&str) -> Option<f32>;

static CAP_HEIGHT_RESOLVER: OnceLock<CapHeightResolver> = OnceLock::new();

/// Register the reader of system families' cap height (#1797). This crate
/// does no font-file IO: the app, which holds the font database, supplies
/// the reader once and owns its per-family cache. First registration wins.
pub fn set_cap_height_resolver(resolver: CapHeightResolver) {
    let _ = CAP_HEIGHT_RESOLVER.set(resolver);
}

/// Shaped width in em of a text in a family, `None` when the shaper is not
/// ready yet.
pub type AdvanceResolver = fn(&str, &str) -> Option<f32>;

static ADVANCE_RESOLVER: OnceLock<AdvanceResolver> = OnceLock::new();

/// Register the text shaper (#2023). Like the cap-height reader, the app
/// owns the font system; this crate only asks. First registration wins.
pub fn set_advance_resolver(resolver: AdvanceResolver) {
    let _ = ADVANCE_RESOLVER.set(resolver);
}

/// Shaped width of `text` in em in `family` (`None` is the bundled default),
/// or `None` before a shaper is registered.
pub fn shaped_advance_em(text: &str, family: Option<&str>) -> Option<f32> {
    let name = family.map_or(BUNDLED_DEFAULT_FAMILY, str::trim);
    ADVANCE_RESOLVER
        .get()
        .and_then(|resolve| resolve(text, name))
}

/// Em advance per character of the bundled default face, which is monospace.
/// Stands in for the shaper until it is ready.
const BUNDLED_DEFAULT_ADVANCE_EM: f32 = 0.5;

/// Millimetres of cap height per em for `family` (`None` is the bundled
/// default). Bundled faces use their measured ratio; a system family uses
/// the registered resolver's reading; anything else keeps
/// [`FALLBACK_MM_PER_EM`].
pub fn mm_per_em(family: Option<&str>) -> f32 {
    let name = family.map_or(BUNDLED_DEFAULT_FAMILY, str::trim);
    if let Some(&(_, ratio)) = BUNDLED_CAP_HEIGHT_PER_EM
        .iter()
        .find(|(bundled, _)| bundled.eq_ignore_ascii_case(name))
    {
        return ratio;
    }
    CAP_HEIGHT_RESOLVER
        .get()
        .and_then(|resolve| resolve(name))
        .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
        .unwrap_or(FALLBACK_MM_PER_EM)
}

/// Width in mm of `text` in the bundled default face at cap height
/// `size_mm`: the shaped advance, or the monospace advance before the shaper
/// is ready.
pub fn default_face_width_mm(text: &str, size_mm: f64) -> f64 {
    let em = shaped_advance_em(text, None)
        .unwrap_or_else(|| text.chars().count().max(1) as f32 * BUNDLED_DEFAULT_ADVANCE_EM);
    f64::from(em) * size_mm / f64::from(mm_per_em(None))
}

/// Cap-height ratio of the bundled `family`, for tests that pin the table.
pub fn bundled_cap_height_per_em(family: &str) -> Option<f32> {
    BUNDLED_CAP_HEIGHT_PER_EM
        .iter()
        .find(|(bundled, _)| bundled.eq_ignore_ascii_case(family))
        .map(|&(_, ratio)| ratio)
}

/// A stored family name, or `None` when it names nothing. Surrounding
/// whitespace is not part of a family name, and a blank name is no name.
pub fn normalize_family(family: Option<String>) -> Option<String> {
    family.and_then(|name| {
        let trimmed = name.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

/// The family a text item asks for: its own, else the document default,
/// else `None` (the caller then uses [`BUNDLED_DEFAULT_FAMILY`]).
///
/// This is the *requested* family. Whether it is installed is the app's
/// call; a missing one resolves to [`BUNDLED_DEFAULT_FAMILY`] there.
pub fn requested_family<'a>(item: Option<&'a str>, document: Option<&'a str>) -> Option<&'a str> {
    [item, document]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_beats_document_beats_nothing() {
        assert_eq!(requested_family(Some("A"), Some("B")), Some("A"));
        assert_eq!(requested_family(None, Some("B")), Some("B"));
        assert_eq!(requested_family(None, None), None);
    }

    #[test]
    fn bundled_families_use_their_cap_height_and_others_the_fallback() {
        assert_eq!(mm_per_em(None), 0.735);
        assert_eq!(mm_per_em(Some(" iosevka ")), 0.735);
        assert!((mm_per_em(Some("Roboto")) - 0.710_937_5).abs() < 1e-6);
        assert_eq!(mm_per_em(Some("Fira Mono")), FALLBACK_MM_PER_EM);
    }

    #[test]
    fn a_blank_name_is_no_name() {
        assert_eq!(requested_family(Some("  "), Some("B")), Some("B"));
        assert_eq!(normalize_family(Some(" \t".into())), None);
        assert_eq!(normalize_family(Some(" X ".into())), Some("X".into()));
    }
}
