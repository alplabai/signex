//! Mockup rows 119-121: the one Library panel. It opens on the kind of the
//! active editor tab, a chip pick holds while that tab stays active, the
//! Parts filter matches rows, Delete never acts on a row the filter hides,
//! and with nothing open the chips and tools are inert. (#2262, kept from
//! the SCH Library panel: no title row, the symbol count in the right
//! plural.)

use std::path::{Path, PathBuf};

use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PlmReserved,
};

use super::*;
use crate::app::{FootprintDocument, Message, Signex, SymbolDocument, TabInfo, TabKind};
use crate::layout_probe::{lay_out_texts, messages_on_click};
use crate::library::messages::{LibraryMessage, PickerMsg};
use crate::library::state::OpenLibrary;
use crate::panels::PanelMsg;

const WINDOW: iced::Size = iced::Size::new(300.0, 640.0);

fn push_tab(app: &mut Signex, path: &Path, kind: TabKind) {
    app.document_state.tabs.push(TabInfo {
        title: "tab".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

/// Open a one-symbol `.snxsym` named `name` in a symbol editor tab.
fn open_symbol(app: &mut Signex, file: &str, name: &str) {
    let path = PathBuf::from(file);
    let symbol_file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty(name));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), symbol_file));
    push_tab(app, &path, TabKind::SymbolEditor(path.clone()));
    app.refresh_panel_ctx();
}

fn app_with_one_symbol() -> Signex {
    let (mut app, _boot) = Signex::new();
    open_symbol(&mut app, "power.snxsym", "+3V3");
    app
}

fn app_with_one_footprint() -> Signex {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("soic.snxfpt");
    let mut footprint = signex_library::Footprint::empty("SOIC-8");
    footprint.pads = (1..=8)
        .map(|n| signex_library::Pad {
            number: n.to_string(),
            size: [1.0, 1.0],
            position: [f64::from(n), 0.0],
            ..signex_library::Pad::default()
        })
        .collect();
    let file = signex_library::FootprintFile::from_footprint(footprint);
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    push_tab(&mut app, &path, TabKind::FootprintEditor(path.clone()));
    app.refresh_panel_ctx();
    app
}

fn row(pn: &str, mpn: &str, mfr: &str) -> ComponentRow {
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: uuid::Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: signex_library::PrimitiveRef::new(uuid::Uuid::nil(), uuid::Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft(mfr, mpn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "1".into(),
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

const LIB_ROOT: &str = "/libs/passives/passives.snxlib";

fn passives() -> OpenLibrary {
    OpenLibrary {
        root: PathBuf::from(LIB_ROOT),
        display_name: "passives".into(),
        library_id: uuid::Uuid::now_v7(),
        tables: std::collections::HashMap::from([
            (
                "res".to_owned(),
                vec![row("R-10K", "ERA-3AED", "Panasonic")],
            ),
            ("cap".to_owned(), vec![row("C-100N", "GRM21", "Murata")]),
        ]),
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
    }
}

fn texts(app: &Signex) -> Vec<String> {
    lay_out_texts(view(&app.library, &app.document_state.panel_ctx), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn clicked(app: &Signex, label: &str) -> Vec<DockMessage> {
    messages_on_click(
        view(&app.library, &app.document_state.panel_ctx),
        WINDOW,
        label,
    )
}

fn picked(kind: LibraryPanelKind) -> Message {
    Message::Library(LibraryMessage::Picker(PickerMsg::PanelKindPicked(kind)))
}

#[test]
fn a_symbol_tab_opens_on_symbols_with_no_title_row_and_a_singular_count() {
    let app = app_with_one_symbol();
    let ctx = &app.document_state.panel_ctx;
    assert!(
        ctx.symbol_editor.is_some(),
        "the symbol editor reaches the panel"
    );
    assert_eq!(shown_kind(&app.library, ctx), LibraryPanelKind::Symbols);
    let shown = texts(&app);
    assert!(
        shown.iter().all(|t| t != "SCH Library" && t != "Library"),
        "the dock tab already names the panel: {shown:#?}"
    );
    assert!(
        shown.iter().any(|t| t == "· 1 symbol"),
        "singular count: {shown:#?}"
    );
    assert!(shown.iter().any(|t| t == "power.snxsym"), "{shown:#?}");
    assert!(shown.iter().any(|t| t == "+3V3"), "{shown:#?}");
    for chip in ["Symbols", "Footprints", "Parts"] {
        assert!(shown.iter().any(|t| t == chip), "{chip} chip: {shown:#?}");
    }
}

#[test]
fn a_footprint_tab_opens_on_footprints() {
    let app = app_with_one_footprint();
    assert_eq!(
        shown_kind(&app.library, &app.document_state.panel_ctx),
        LibraryPanelKind::Footprints
    );
    let shown = texts(&app);
    assert!(shown.iter().any(|t| t == "· 1 footprint"), "{shown:#?}");
    assert!(shown.iter().any(|t| t == "SOIC-8"), "{shown:#?}");
    assert!(shown.iter().any(|t| t == "8 pads"), "{shown:#?}");
}

#[test]
fn any_other_tab_opens_on_parts_and_the_filter_matches_rows() {
    let (mut app, _boot) = Signex::new();
    app.library.open_libraries.push(passives());
    app.library.touch_data();
    app.library.sync_view_caches();
    app.refresh_panel_ctx();
    assert_eq!(
        shown_kind(&app.library, &app.document_state.panel_ctx),
        LibraryPanelKind::Parts
    );
    let shown = texts(&app);
    assert!(shown.iter().any(|t| t == "passives.snxlib"), "{shown:#?}");
    assert!(shown.iter().any(|t| t == "· 2 parts"), "{shown:#?}");
    assert!(
        !shown.iter().any(|t| t == "R-10K"),
        "a closed library lists no rows: {shown:#?}"
    );

    // A filter opens every library with a matching row on its matches.
    app.library.panel_search = "era-3".into();
    app.library.sync_view_caches();
    let shown = texts(&app);
    assert!(shown.iter().any(|t| t == "R-10K"), "{shown:#?}");
    assert!(!shown.iter().any(|t| t == "C-100N"), "{shown:#?}");

    app.library.panel_search = "nothing-like-it".into();
    app.library.sync_view_caches();
    let shown = texts(&app);
    assert!(
        !shown
            .iter()
            .any(|t| t == "R-10K" || t == "res" || t == "Open"),
        "no library row and no table: {shown:#?}"
    );
    assert!(
        shown.iter().any(|t| t == "Nothing matches the filter."),
        "{shown:#?}"
    );
}

#[test]
fn a_chip_pick_holds_while_its_tab_stays_active() {
    let mut app = app_with_one_symbol();
    assert!(
        clicked(&app, "Parts").iter().any(|m| matches!(
            m,
            DockMessage::Library(LibraryMessage::Picker(PickerMsg::PanelKindPicked(
                LibraryPanelKind::Parts
            )))
        )),
        "the Parts chip picks Parts"
    );
    let _task = app.update(picked(LibraryPanelKind::Parts));
    assert_eq!(app.library_panel_kind(), LibraryPanelKind::Parts);

    // Another symbol tab opens on its own kind.
    open_symbol(&mut app, "logic.snxsym", "NAND");
    assert_eq!(app.library_panel_kind(), LibraryPanelKind::Symbols);

    // Back on the first tab, its pick shows again.
    app.document_state.active_tab = app.document_state.tabs.len() - 2;
    app.refresh_panel_ctx();
    assert_eq!(app.library_panel_kind(), LibraryPanelKind::Parts);
}

#[test]
fn with_nothing_open_the_chips_and_tools_are_inert() {
    let (mut app, _boot) = Signex::new();
    app.refresh_panel_ctx();
    assert!(has_nothing_open(
        &app.library,
        &app.document_state.panel_ctx
    ));
    let shown = texts(&app);
    for label in ["No libraries open", "No library open", "Open Library…"] {
        assert!(shown.iter().any(|t| t == label), "{label}: {shown:#?}");
    }
    for inert in ["Symbols", "Footprints", "Parts", "Place", "Add", "Edit"] {
        assert!(
            clicked(&app, inert).is_empty(),
            "{inert} takes no click while nothing is open"
        );
    }
    assert!(
        clicked(&app, "Open Library…")
            .iter()
            .any(|m| matches!(m, DockMessage::Library(LibraryMessage::OpenLibraryDialog))),
        "the empty card opens a library"
    );
}

#[test]
fn delete_is_off_while_the_filter_hides_the_active_symbol() {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("logic.snxsym");
    let mut file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("+3V3"));
    file.symbols.push(signex_library::Symbol::empty("NAND"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    push_tab(&mut app, &path, TabKind::SymbolEditor(path.clone()));
    app.refresh_panel_ctx();
    let delete = |app: &Signex| {
        symbols::view(&app.library, &app.document_state.panel_ctx)
            .tools
            .delete
    };
    assert!(
        matches!(
            delete(&app),
            Some(DockMessage::Panel(PanelMsg::SchLibraryDeleteSymbol(0)))
        ),
        "with no filter Delete targets the active +3V3"
    );
    app.library.panel_search = "NAND".into();
    assert!(
        delete(&app).is_none(),
        "the filter hides +3V3: Delete must not hit a row nobody sees"
    );
    app.library.panel_search = "3v3".into();
    assert!(delete(&app).is_some(), "the active row shows again");
}

#[test]
fn edit_and_delete_are_off_while_the_filter_hides_the_highlighted_footprint() {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("ics.snxfpt");
    let mut file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("SOIC-8"));
    file.footprints
        .push(signex_library::Footprint::empty("QFN-16"));
    let mut doc = FootprintDocument::new(path.clone(), file);
    doc.panel_selected_idx = Some(0);
    app.document_state
        .footprint_editors
        .insert(path.clone(), doc);
    push_tab(&mut app, &path, TabKind::FootprintEditor(path.clone()));
    app.refresh_panel_ctx();
    let tools = |app: &Signex| {
        let tools = footprints::view(&app.library, &app.document_state.panel_ctx).tools;
        (tools.edit, tools.delete)
    };
    assert!(
        matches!(
            tools(&app),
            (
                Some(DockMessage::Panel(PanelMsg::FpLibraryEditInternal(0))),
                Some(DockMessage::Panel(PanelMsg::FpLibraryDeleteInternal(0)))
            )
        ),
        "with no filter Edit and Delete target the highlighted SOIC-8"
    );
    app.library.panel_search = "QFN".into();
    assert!(
        matches!(tools(&app), (None, None)),
        "the filter hides SOIC-8: Edit and Delete are off"
    );
    app.library.panel_search = "soic".into();
    assert!(matches!(tools(&app), (Some(_), Some(_))));
}

/// Mockup row 119: the row the editor shows draws selected, so Edit is live
/// on it with no separate highlight; Delete still waits for one.
#[test]
fn edit_follows_the_footprint_the_editor_shows() {
    let app = app_with_one_footprint();
    let ctx = &app.document_state.panel_ctx;
    let fp = ctx.footprint_editor.as_ref().expect("footprint editor");
    assert_eq!(fp.internal_selected_idx, None, "nothing highlighted");
    assert!(fp.internal_footprints[0].is_active);
    let tools = footprints::view(&app.library, ctx).tools;
    assert!(matches!(
        tools.edit,
        Some(DockMessage::Panel(PanelMsg::FpLibraryEditInternal(0)))
    ));
    assert!(tools.delete.is_none());
}

/// Mockup row 119: with no way to place, Place sends nothing and its tooltip
/// gives the reason, in Footprints and in Symbols.
#[test]
fn place_is_off_with_its_reason_in_footprints_and_symbols() {
    let app = app_with_one_footprint();
    let ctx = &app.document_state.panel_ctx;
    let tools = footprints::view(&app.library, ctx).tools;
    assert!(tools.place.is_none());
    assert_eq!(
        tools.place_blocked,
        Some(ids::LIBRARY_PANEL_PLACE_NOT_IN_LIBRARY),
        "this file is in no open library"
    );

    let app = app_with_one_symbol();
    let ctx = &app.document_state.panel_ctx;
    let tools = symbols::view(&app.library, ctx).tools;
    assert!(tools.place.is_none());
    assert_eq!(tools.place_blocked, Some(ids::LIBRARY_PANEL_PLACE_SYMBOLS));
}

#[test]
fn a_file_is_named_by_the_open_library_that_holds_it() {
    let mut state = LibraryState::default();
    assert_eq!(
        frame::library_name_of(Path::new("/libs/old.snxlib/symbols/a.snxsym"), &state),
        Some("old".into()),
        "a `.snxlib` folder among the ancestors"
    );
    assert_eq!(
        frame::library_name_of(Path::new("/elsewhere/a.snxsym"), &state),
        None
    );
    state.open_libraries.push(passives());
    assert_eq!(
        frame::library_name_of(Path::new("/libs/passives/symbols/opamps.snxsym"), &state),
        Some("passives".into())
    );
}
