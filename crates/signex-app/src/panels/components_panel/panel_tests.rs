//! Components panel view tests: the row cap, the Graphite row look, the flat
//! list order, the source chips, the add menu and collapsing.

use super::*;
use signex_i18n::I18n;
use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PinPadOverride, PlmReserved,
};
use uuid::Uuid;

/// The thumbnail look the view tests draw rows with.
pub(super) fn thumb_look() -> thumbs::ThumbLook {
    thumbs::ThumbLook {
        colors: signex_types::theme::canvas_colors(signex_types::theme::ThemeId::GraphiteLight),
        device_px_per_logical_px: 1.0,
    }
}

pub(super) fn fixture_row(pn: &str, mpn: &str, mfr: &str) -> ComponentRow {
    let _ = (PinPadOverride::new("1", "1"),);
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft(mfr, mpn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

/// More rows than the cap: the view builds the cap's worth and says how
/// many it left out; the rest stay reachable by narrowing the filter.
#[test]
fn rows_beyond_the_cap_collapse_into_a_refine_search_row() {
    let total = 250;
    let mut state = LibraryState::default();
    let rows = (0..total)
        .map(|i| fixture_row(&format!("PN-{i}"), "MPN", "Mfr"))
        .collect();
    state.open_libraries.push(OpenLibrary {
        root: PathBuf::from("/tmp/big.snxlib"),
        display_name: "big".into(),
        library_id: uuid::Uuid::now_v7(),
        tables: std::collections::HashMap::from([("t".to_owned(), rows)]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: Default::default(),
        provenance: Default::default(),
    });
    state.touch_data();
    state.sync_view_caches();

    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    let window = iced::Size::new(600.0, 100_000.0);
    fn build<'a>(state: &'a LibraryState, tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
        view_library_block(
            &state.open_libraries[0],
            ComponentsMountSource::Stock,
            &state.components_panel,
            "",
            thumb_look(),
            tokens,
            &signex_i18n::I18n::en_us(),
        )
    }
    let texts = crate::layout_probe::lay_out_texts(build(&state, &tokens), window);
    let rows = texts.iter().filter(|t| t.text.starts_with("PN-")).count();
    assert_eq!(rows, 200);
    // The count is a catalog argument, wrapped in isolation marks.
    crate::layout_probe::find(&texts, "\u{2068}50\u{2069} more — refine search");

    // Narrowing the filter below the cap removes the summary row.
    state.components_panel.filter = "pn-7".into();
    state.sync_view_caches();
    let texts = crate::layout_probe::lay_out_texts(build(&state, &tokens), window);
    assert!(texts.iter().all(|t| !t.text.contains("refine search")));
    assert_eq!(
        texts.iter().filter(|t| t.text.starts_with("PN-")).count(),
        11
    );
}

/// One library holding `rows`, its view caches current.
pub(super) fn library_with(rows: Vec<ComponentRow>) -> LibraryState {
    let mut state = LibraryState::default();
    state.open_libraries.push(OpenLibrary {
        root: PathBuf::from("/tmp/one.snxlib"),
        display_name: "one".into(),
        library_id: uuid::Uuid::now_v7(),
        tables: std::collections::HashMap::from([("t".to_owned(), rows)]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: Default::default(),
        provenance: Default::default(),
    });
    state.touch_data();
    state.sync_view_caches();
    state
}

/// Graphite: the library header and every row are 26 px tall, the rows
/// one indent step in from their library, and the selected row is filled
/// with the theme's selected token, not the accent wash.
#[test]
fn graphite_rows_are_dense_indented_and_select_with_the_selected_token() {
    use signex_types::theme::{ThemeId, theme_tokens};
    let mut state = library_with(vec![
        fixture_row("PN-A", "", "Mfr"),
        fixture_row("PN-B", "", "Mfr"),
    ]);
    let row_id = state.open_libraries[0].tables["t"][0].row_id;
    state.components_panel.selected = Some(crate::library::state::ComponentsPanelRowKey {
        library_path: PathBuf::from("/tmp/one.snxlib"),
        table: "t".into(),
        row_id: signex_library::RowId::from_uuid(row_id),
    });
    let window = iced::Size::new(400.0, 200.0);
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        let build = || {
            view_library_block(
                &state.open_libraries[0],
                ComponentsMountSource::Stock,
                &state.components_panel,
                "",
                thumb_look(),
                &tokens,
                &signex_i18n::I18n::en_us(),
            )
        };
        let texts = crate::layout_probe::lay_out_texts(build(), window);
        let header = crate::layout_probe::find(&texts, "one").bounds;
        let a = crate::layout_probe::find(&texts, "PN-A").bounds;
        let b = crate::layout_probe::find(&texts, "PN-B").bounds;
        assert!(
            (b.y - a.y - look::ROW_HEIGHT).abs() < 0.5,
            "{id:?}: rows are {} px apart",
            b.y - a.y
        );
        assert!(
            (a.x - look::ROW_INSET).abs() < 0.5,
            "{id:?}: row at {}",
            a.x
        );
        assert!(
            (header.x - look::LIBRARY_INSET).abs() < 0.5,
            "{id:?}: header at {}",
            header.x
        );

        let pixels = crate::layout_probe::draw_pixels(build(), window);
        let at = |x: usize, y: usize| {
            let i = (y * window.width as usize + x) * 4;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };
        let sel = tokens.selection;
        let y = (look::ROW_HEIGHT * 1.5) as usize;
        assert_eq!(at(4, y), [sel.r, sel.g, sel.b], "{id:?}: selected fill");
        assert_ne!(
            at(4, y + look::ROW_HEIGHT as usize),
            [sel.r, sel.g, sel.b],
            "{id:?}: the unselected row is not filled"
        );
    }
}

#[test]
fn mount_sources_list_stock_first_then_project_installed_global() {
    assert_eq!(
        ComponentsMountSource::ORDER,
        &[
            ComponentsMountSource::Stock,
            ComponentsMountSource::Project,
            ComponentsMountSource::Installed,
            ComponentsMountSource::Global,
        ]
    );
}

#[test]
fn mount_source_label_and_key_distinct() {
    for src in ComponentsMountSource::ORDER {
        assert!(!src.label().is_empty());
        assert!(!src.key().is_empty());
        assert_ne!(src.label(), src.key());
    }
}

/// A state with a stock, a global and two installed libraries, mounted
/// in that scrambled order: global, installed A, stock, installed B.
fn four_libraries() -> LibraryState {
    let mut state = LibraryState::default();
    for name in ["g", "ia", "s", "ib"] {
        let mut one = library_with(vec![fixture_row("PN", "", "Mfr")]);
        let mut lib = one.open_libraries.remove(0);
        lib.root = PathBuf::from(format!("/tmp/{name}.snxlib"));
        lib.display_name = name.into();
        state.open_libraries.push(lib);
    }
    state.stock_library_root = Some(PathBuf::from("/tmp/s.snxlib"));
    state.installed_libraries = vec![
        PathBuf::from("/tmp/ia.snxlib"),
        PathBuf::from("/tmp/ib.snxlib"),
    ];
    state.global_libraries = vec![global_prefs::GlobalLibraryEntry {
        path: PathBuf::from("/tmp/g.snxlib"),
        remote: None,
        auto_pull: None,
    }];
    // The row caches are keyed by root: rebuild them for the new roots.
    state.touch_data();
    state.sync_view_caches();
    state
}

fn names(listed: &[(ComponentsMountSource, &OpenLibrary)]) -> Vec<String> {
    listed.iter().map(|(_, l)| l.display_name.clone()).collect()
}

/// One flat list, Stock first, then Project, Installed, Global; inside a
/// source, mount order.
#[test]
fn the_flat_list_orders_stock_then_project_installed_global() {
    let state = four_libraries();
    let listed = listed_libraries(&state, &[]);
    assert_eq!(names(&listed), ["s", "ia", "ib", "g"]);
    assert_eq!(listed[0].0, ComponentsMountSource::Stock);
    assert_eq!(listed[3].0, ComponentsMountSource::Global);
}

/// "All" is always a chip; a source with no library has none.
#[test]
fn empty_source_chips_are_hidden_and_all_stays() {
    let state = four_libraries();
    let counts = SourceCounts::of(&listed_libraries(&state, &[]), &state.components_panel.rows);
    assert_eq!(
        counts.chips(),
        [
            None,
            Some(ComponentsMountSource::Stock),
            Some(ComponentsMountSource::Installed),
            Some(ComponentsMountSource::Global),
        ],
        "Project has no library, so no chip"
    );
    assert_eq!(counts.all, 4);
    assert_eq!(counts.of_source(ComponentsMountSource::Installed), 2);

    let nothing = SourceCounts::of(&[], &crate::library::state::PanelRows::default());
    assert_eq!(nothing.chips(), [None], "All shows even with no library");
}

/// A chip filters the list to its source, and a source that lost its
/// last library falls back to All instead of showing an empty list.
#[test]
fn a_source_chip_filters_the_list_and_an_emptied_one_falls_back_to_all() {
    let state = four_libraries();
    let listed = listed_libraries(&state, &[]);
    let counts = SourceCounts::of(&listed, &state.components_panel.rows);
    let shown = |chip| {
        let selected = counts.effective(chip);
        names(
            &listed
                .iter()
                .filter(|(s, _)| selected.is_none_or(|c| c == *s))
                .cloned()
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(shown(None), ["s", "ia", "ib", "g"]);
    assert_eq!(shown(Some(ComponentsMountSource::Installed)), ["ia", "ib"]);
    assert_eq!(shown(Some(ComponentsMountSource::Global)), ["g"]);
    assert_eq!(
        shown(Some(ComponentsMountSource::Project)),
        ["s", "ia", "ib", "g"],
        "Project has no library: the choice no longer applies"
    );
}

/// The header "+ Add" menu offers Project, Installed and Global, each
/// the existing add-library command for that source.
#[test]
fn the_add_menu_offers_project_installed_and_global() {
    assert_eq!(
        ComponentsMountSource::ADDABLE,
        &[
            ComponentsMountSource::Project,
            ComponentsMountSource::Installed,
            ComponentsMountSource::Global,
        ]
    );
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    let texts = crate::layout_probe::lay_out_texts(
        look::add_menu(&tokens, &I18n::en_us()),
        iced::Size::new(300.0, 300.0),
    );
    for source in ComponentsMountSource::ADDABLE {
        crate::layout_probe::find(&texts, source.label());
    }
}

/// A collapsed library keeps its row (name, badge, count) and drops its
/// components; expanded lists them.
#[test]
fn a_collapsed_library_hides_its_components_and_keeps_its_row() {
    let mut state = library_with(vec![fixture_row("PN-A", "", "Mfr")]);
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
    let window = iced::Size::new(400.0, 200.0);
    let probe = |state: &LibraryState| {
        crate::layout_probe::lay_out_texts(
            view_library_block(
                &state.open_libraries[0],
                ComponentsMountSource::Global,
                &state.components_panel,
                "",
                thumb_look(),
                &tokens,
                &signex_i18n::I18n::en_us(),
            ),
            window,
        )
    };
    let open = probe(&state);
    crate::layout_probe::find(&open, "PN-A");
    crate::layout_probe::find(&open, "GLOBAL");

    let root = state.open_libraries[0].root.clone();
    state.components_panel.collapsed_libraries.insert(root);
    let closed = probe(&state);
    assert!(closed.iter().all(|t| t.text != "PN-A"));
    crate::layout_probe::find(&closed, "one");
    crate::layout_probe::find(&closed, "GLOBAL");
}
