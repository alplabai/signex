//! Which bundled face an exported schematic text uses (#510).
//!
//! The canvas can draw any installed family; an export can only outline the
//! faces compiled into the app (`text::face_for_alias`). So the export honours
//! a stored family when it names a bundled one and otherwise draws the
//! bundled default, Iosevka, which is also what the canvas draws when the
//! family is missing. The app's missing-font notice says so.

use signex_types::font::requested_family;

/// Alias of Roboto Regular in `text::face_for_alias`.
const ROBOTO_REGULAR: &str = "F1";
/// Alias of Iosevka Regular in `text::face_for_alias`.
const IOSEVKA_REGULAR: &str = "F3";

/// The alias to draw an item in. `default_alias` is what the item used before
/// it could name a family, kept for text that names none so existing exports
/// do not change.
pub(super) fn alias_for_family(
    item: Option<&str>,
    document: Option<&str>,
    default_alias: &'static str,
) -> &'static str {
    match requested_family(item, document) {
        None => default_alias,
        Some(name) if name.eq_ignore_ascii_case("Roboto") => ROBOTO_REGULAR,
        // The bundled default itself, or a family that cannot be embedded.
        Some(_) => IOSEVKA_REGULAR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unnamed_text_keeps_its_existing_face() {
        assert_eq!(alias_for_family(None, None, "F1"), "F1");
        assert_eq!(alias_for_family(None, None, "F3"), "F3");
    }

    #[test]
    fn a_bundled_family_selects_its_own_face() {
        assert_eq!(alias_for_family(Some("roboto"), None, "F3"), "F1");
        assert_eq!(alias_for_family(Some("Iosevka"), None, "F1"), "F3");
    }

    #[test]
    fn any_other_family_exports_in_the_bundled_default() {
        assert_eq!(alias_for_family(Some("Fira Mono"), None, "F1"), "F3");
        assert_eq!(alias_for_family(None, Some("Fira Mono"), "F1"), "F3");
    }

    #[test]
    fn the_item_beats_the_document_default() {
        assert_eq!(
            alias_for_family(Some("Roboto"), Some("Fira Mono"), "F3"),
            "F1"
        );
    }
}
