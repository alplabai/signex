//! Library Browser — bound-primitive names for the Symbol and Footprint
//! columns, resolved from the library's cached listings.

use signex_library::PrimitiveRef;

use super::OpenLibrary;

/// The names a grid shows for the primitives its rows bind. Empty when the
/// listings are not loaded, so every cell falls back to the short id.
#[derive(Clone, Copy)]
pub(super) struct PrimitiveNames<'a> {
    lib: Option<&'a OpenLibrary>,
}

impl<'a> PrimitiveNames<'a> {
    #[cfg(test)]
    pub(super) fn none() -> Self {
        Self { lib: None }
    }

    pub(super) fn of(lib: &'a OpenLibrary) -> Self {
        Self { lib: Some(lib) }
    }

    pub(super) fn symbol(&self, r: &PrimitiveRef) -> Option<&'a str> {
        let lib = self.lib?;
        listed_name(&lib.cached_symbols, r)
    }

    pub(super) fn footprint(&self, r: &PrimitiveRef) -> Option<&'a str> {
        let lib = self.lib?;
        listed_name(&lib.cached_footprints, r)
    }
}

fn listed_name<'a>(
    listing: &'a [signex_library::PrimitiveSummary],
    r: &PrimitiveRef,
) -> Option<&'a str> {
    listing
        .iter()
        .find(|s| s.uuid == r.uuid)
        .map(|s| s.name.as_str())
        .filter(|n| !n.is_empty())
}

#[cfg(test)]
mod tests {
    use super::super::columns::ColumnKind;
    use crate::library::state::LibraryState;

    #[test]
    fn a_stock_rows_symbol_and_footprint_cells_show_the_primitive_names() {
        let stock = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/stock-library/Stock.snxlib");
        let mut state = LibraryState::default();
        state.open_library_read_only(stock.clone()).expect("mount");
        let lib = state.library_at(&stock).expect("mounted");
        let row = lib
            .tables
            .values()
            .flatten()
            .find(|r| r.internal_pn.as_str() == "STOCK-R")
            .expect("STOCK-R");
        let names = super::PrimitiveNames::of(lib);

        let symbol = ColumnKind::Symbol.cell_value_named(row, &names);
        let footprint = ColumnKind::Footprint.cell_value_named(row, &names);
        assert!(!symbol.starts_with('•'), "symbol cell: {symbol}");
        assert!(!footprint.starts_with('•'), "footprint cell: {footprint}");
        assert_eq!(symbol, "R");
        assert!(
            footprint.contains("0805") || footprint.contains("Chip"),
            "{footprint}"
        );
        assert!(ColumnKind::Symbol.cell_value(row).starts_with('•'));
    }

    #[test]
    fn a_listing_loaded_late_invalidates_the_cache_and_name_search_finds_the_row() {
        use super::super::{BrowserViewCache, active_table};
        use crate::library::state::LibraryBrowserState;

        let stock = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/stock-library/Stock.snxlib");
        let mut state = LibraryState::default();
        state.open_library_read_only(stock.clone()).expect("mount");
        let footprints = std::mem::take(
            &mut state
                .open_libraries
                .iter_mut()
                .find(|l| l.root == stock)
                .expect("mounted")
                .cached_footprints,
        );
        let name = footprints
            .iter()
            .map(|f| f.name.clone())
            .find(|n| n.contains("0805"))
            .expect("a 0805 footprint is listed");
        let mut browser = LibraryBrowserState::new(stock.clone());
        browser.search = name.clone();

        let build = |state: &LibraryState, browser: &LibraryBrowserState| {
            let lib = state.library_at(&stock).expect("mounted");
            BrowserViewCache::build(
                lib,
                &state.template_registry,
                state.clock,
                state.data_version,
                browser,
            )
        };
        let before = build(&state, &browser);
        assert!(before.visible_for_test().is_empty(), "no listing, no name");

        state
            .open_libraries
            .iter_mut()
            .find(|l| l.root == stock)
            .expect("mounted")
            .cached_footprints = footprints;
        state.refresh_loaded_previews(&stock);

        let lib = state.library_at(&stock).expect("mounted");
        let table = active_table(lib, &browser);
        assert!(
            !before.matches(state.data_version, state.clock, table, &browser),
            "the listing reload must invalidate the cache"
        );
        assert!(!build(&state, &browser).visible_for_test().is_empty());
    }
}
