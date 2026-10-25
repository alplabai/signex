//! Which family a text item draws in (#510).
//!
//! The one place the "installed or fall back" decision is made. Both render
//! paths receive its answer on `TextItem::family` and read it through
//! `TextItem::family_or`, and the missing-font report uses the same
//! predicate, so what the Messages panel says is what the canvas does.

use std::sync::Arc;

use signex_types::font::{BUNDLED_DEFAULT_FAMILY, requested_family};

use super::catalog::FontCatalog;

/// The family to draw an item in, or `None` for the canvas default
/// ([`BUNDLED_DEFAULT_FAMILY`]).
///
/// Order: the item's own family, then the document default; the first one
/// named is the only one considered. A named family that is not installed
/// falls to the bundled default, not on to the document default: the author
/// chose that face for this item, and the substitution must be the same
/// bundled face on every machine. With no catalog yet (the scan has not
/// finished) only the bundled families count as installed, so the first
/// frames are the same on every machine.
pub fn resolve_family(
    item: Option<&str>,
    document: Option<&str>,
    catalog: &FontCatalog,
) -> Option<Arc<str>> {
    let requested = requested_family(item, document)?;
    if requested.eq_ignore_ascii_case(BUNDLED_DEFAULT_FAMILY) {
        return None;
    }
    catalog.has(requested).then(|| Arc::from(requested))
}

/// [`resolve_family`] against the process-wide installed-font cache. What the
/// snapshot builders call.
pub fn resolve_installed(item: Option<&str>, document: Option<&str>) -> Option<Arc<str>> {
    resolve_family(item, document, super::catalog::installed_or_bundled())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> FontCatalog {
        FontCatalog::from_families(["Fira Mono", "Other Face"])
    }

    fn name(resolved: Option<Arc<str>>) -> Option<String> {
        resolved.map(|family| family.to_string())
    }

    #[test]
    fn resolution_order_is_item_then_document_then_the_bundled_default() {
        let c = catalog();
        assert_eq!(
            name(resolve_family(Some("Fira Mono"), Some("Other Face"), &c)),
            Some("Fira Mono".into())
        );
        assert_eq!(
            name(resolve_family(None, Some("Other Face"), &c)),
            Some("Other Face".into())
        );
        assert_eq!(name(resolve_family(None, None, &c)), None);
    }

    #[test]
    fn a_missing_item_family_falls_to_the_bundled_default_not_the_document_family() {
        let c = catalog();
        assert_eq!(
            name(resolve_family(
                Some("Not Installed"),
                Some("Other Face"),
                &c
            )),
            None,
            "the author picked that face for this item; substituting a different \
             document face would differ from the documented fallback"
        );
    }

    #[test]
    fn naming_the_bundled_default_resolves_to_the_default() {
        assert_eq!(
            name(resolve_family(Some("iosevka"), None, &catalog())),
            None
        );
    }

    #[test]
    fn before_the_scan_lands_only_the_bundled_families_count_as_installed() {
        let before_scan = FontCatalog::from_families::<_, &str>([]);
        assert_eq!(
            name(resolve_family(Some("Fira Mono"), None, &before_scan)),
            None
        );
        assert_eq!(
            name(resolve_family(Some("Roboto"), None, &before_scan)),
            Some("Roboto".into())
        );
    }
}
