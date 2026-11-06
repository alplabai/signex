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

/// Line height as a multiple of the em size, the line box every text replay
/// lays a line in and aligns by. The selection and hit boxes use the same
/// line box, so they cover what is drawn.
pub const TEXT_LINE_HEIGHT_EM: f32 = 1.2;

/// Height of the line box of a text whose cap height is `size_mm`, drawn in
/// `family`: one em ([`mm_per_em`]) times [`TEXT_LINE_HEIGHT_EM`].
#[must_use]
pub fn line_box_mm(size_mm: f64, family: Option<&str>) -> f64 {
    size_mm / f64::from(mm_per_em(family)) * f64::from(TEXT_LINE_HEIGHT_EM)
}

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

/// Em advance per character the width estimate falls back to before the
/// shaper is installed: a slight overestimate of a UI face, so text never clips.
pub const ESTIMATE_FALLBACK_ADVANCE_EM: f32 = 0.58;

/// Margin on a shaped width, for hinting and rounding in the real layout.
pub const ESTIMATE_SHAPED_MARGIN: f32 = 1.05;

/// Width in px of `text` drawn at `size_px` in `family` (`None` is the bundled
/// default): the shaped width times [`ESTIMATE_SHAPED_MARGIN`], or
/// [`ESTIMATE_FALLBACK_ADVANCE_EM`] per character before the shaper is ready.
pub fn estimate_text_width_px(text: &str, size_px: f32, family: Option<&str>) -> f32 {
    match shaped_advance_em(text, family) {
        Some(em) => em * size_px * ESTIMATE_SHAPED_MARGIN,
        None => text.chars().count() as f32 * size_px * ESTIMATE_FALLBACK_ADVANCE_EM,
    }
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
        .map_or(FALLBACK_MM_PER_EM, |ratio| ratio.max(MIN_MM_PER_EM))
}

/// Floor on [`mm_per_em`]: no face is read as having a cap height under a
/// quarter of its em, so the em in mm, and every size derived from it, is
/// bounded for any size a sheet may hold.
pub const MIN_MM_PER_EM: f32 = 0.25;

/// Width in mm of `text` at cap height `size_mm` in `family`: the shaped
/// advance, or a monospace advance before the shaper is ready. `family` is the
/// RESOLVED face (`None` is the bundled default): an uninstalled family is
/// resolved by the caller to the face the renderer substitutes, so the box
/// matches the drawn glyphs.
pub fn family_width_mm(text: &str, family: Option<&str>, size_mm: f64) -> f64 {
    family_width_mm_checked(text, family, size_mm).0
}

/// [`family_width_mm`] and whether it is final: false while the shaper (or,
/// for a system family, the cap-height reader) is not installed yet, so a
/// caller that stores widths does not keep a stand-in.
pub fn family_width_mm_checked(text: &str, family: Option<&str>, size_mm: f64) -> (f64, bool) {
    // Empty text keeps a one-glyph box so it can still be picked.
    let probe = if text.is_empty() { "0" } else { text };
    let shaped = shaped_advance_em(probe, family);
    let em = shaped.unwrap_or_else(|| probe.chars().count() as f32 * BUNDLED_DEFAULT_ADVANCE_EM);
    let name = family.map_or(BUNDLED_DEFAULT_FAMILY, str::trim);
    let ratio_known = BUNDLED_CAP_HEIGHT_PER_EM
        .iter()
        .any(|(bundled, _)| bundled.eq_ignore_ascii_case(name))
        || CAP_HEIGHT_RESOLVER
            .get()
            .and_then(|resolve| resolve(name))
            .is_some();
    (
        f64::from(em) * size_mm / f64::from(mm_per_em(family)),
        shaped.is_some() && ratio_known,
    )
}

/// Picks the face a text draws in: `(item family, document family)` to the
/// installed family, `None` for the bundled default.
pub type FamilyResolver = fn(Option<&str>, Option<&str>) -> Option<String>;

static FAMILY_RESOLVER: OnceLock<FamilyResolver> = OnceLock::new();

/// Register the resolver that knows which families are installed (the app
/// owns the font catalog), so the canvas and every export pick the same face
/// for the same text. First registration wins.
pub fn set_family_resolver(resolver: FamilyResolver) {
    let _ = FAMILY_RESOLVER.set(resolver);
}

/// The face a text draws in: its own family, else the document's, checked
/// against the installed fonts; `None` is the bundled default (a family that
/// is not installed draws in it). Without a registered resolver only the
/// bundled families count as installed, as in the app before its font scan
/// has finished.
pub fn resolve_family(item: Option<&str>, document: Option<&str>) -> Option<String> {
    if let Some(resolve) = FAMILY_RESOLVER.get() {
        return resolve(item, document);
    }
    let requested = requested_family(item, document)?;
    if requested.eq_ignore_ascii_case(BUNDLED_DEFAULT_FAMILY) {
        return None;
    }
    BUNDLED_CAP_HEIGHT_PER_EM
        .iter()
        .find(|(bundled, _)| bundled.eq_ignore_ascii_case(requested))
        .map(|(bundled, _)| (*bundled).to_string())
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
    fn the_width_estimate_scales_with_size_and_length() {
        // No shaper is registered in this crate's tests, so this is the
        // per-character stand-in.
        assert_eq!(estimate_text_width_px("", 13.0, None), 0.0);
        let one = estimate_text_width_px("a", 10.0, None);
        assert_eq!(one, 10.0 * ESTIMATE_FALLBACK_ADVANCE_EM);
        assert_eq!(estimate_text_width_px("abcd", 10.0, None), 4.0 * one);
        assert_eq!(estimate_text_width_px("a", 20.0, None), 2.0 * one);
    }

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

    #[test]
    fn an_uninstalled_family_resolves_to_the_bundled_default() {
        assert_eq!(resolve_family(Some("No Such Face 9f2"), None), None);
        assert_eq!(resolve_family(None, Some("No Such Face 9f2")), None);
        assert_eq!(resolve_family(Some("Iosevka"), None), None);
        assert_eq!(
            resolve_family(Some("roboto"), None).as_deref(),
            Some("Roboto")
        );
        assert_eq!(resolve_family(None, None), None);
    }

    /// A text of the same inputs measures the same in the face it resolves to.
    #[test]
    fn an_unavailable_family_is_measured_in_the_default_face() {
        let missing = resolve_family(Some("No Such Face 9f2"), None);
        assert_eq!(
            family_width_mm("SIGNAL", missing.as_deref(), 1.27),
            family_width_mm("SIGNAL", None, 1.27)
        );
    }
}
