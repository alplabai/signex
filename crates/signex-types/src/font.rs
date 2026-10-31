//! Font family identity for text-bearing document items.
//!
//! A document stores only a family *name*. Whether that family is installed
//! on the machine that opens the file is a render-time question answered by
//! the app, never written back: opening and saving on a machine that lacks
//! the font must not destroy the author's choice. This module owns the part
//! that is pure data: the bundled fallback and the order a name is picked in.

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

/// Millimetres of cap height per em for `family` (`None` is the bundled
/// default). Bundled faces use their measured ratio; any other family keeps
/// [`FALLBACK_MM_PER_EM`].
pub fn mm_per_em(family: Option<&str>) -> f32 {
    let name = family.map_or(BUNDLED_DEFAULT_FAMILY, str::trim);
    BUNDLED_CAP_HEIGHT_PER_EM
        .iter()
        .find(|(bundled, _)| bundled.eq_ignore_ascii_case(name))
        .map_or(FALLBACK_MM_PER_EM, |&(_, ratio)| ratio)
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
