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
    fn a_blank_name_is_no_name() {
        assert_eq!(requested_family(Some("  "), Some("B")), Some("B"));
        assert_eq!(normalize_family(Some(" \t".into())), None);
        assert_eq!(normalize_family(Some(" X ".into())), Some("X".into()));
    }
}
