//! #3006 (mockup rows 118 and 127): selecting a Library Browser row starts
//! the preview read and the Properties summary read in the task the
//! selection returns, and running that task draws the row's symbol and
//! footprint. A caller that drops the task (the audit harness did) leaves
//! both on "Loading…" / "Reading symbol…" for good.

use std::path::{Path, PathBuf};

use signex_types::theme::ThemeId;

use crate::app::{Message, Signex};
use crate::library::browser::PreviewLook;
use crate::library::messages::LibraryMessage;
use crate::task_driver::TaskDriver;

/// Dock layouts these tests produce go nowhere: the per-process prefs file
/// they would land in is read back by every other test's `Signex::new`.
struct DiscardLayouts;

impl crate::dock::DockLayoutStore for DiscardLayouts {
    fn write(&self, _layout: &crate::dock::DockLayoutSnapshot) -> Result<(), String> {
        Ok(())
    }
}

fn stock_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

/// The app with the stock library mounted read-only and its Browser tab
/// open and active. Returns the library's root as mounted.
fn app_with_stock_browser() -> (Signex, PathBuf) {
    let (mut app, _boot) = Signex::new();
    app.document_state.dock_layout_sync = crate::dock::DockLayoutSync::new(
        std::sync::Arc::new(DiscardLayouts),
        crate::dock::DockLayoutSnapshot::capture(&app.document_state.dock),
    );
    app.ui_state.first_run_tour_open = false;
    app.library.pending_mounts.clear();
    app.library.open_libraries.clear();
    let stock = stock_root();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock)
        .expect("mount the stock library");
    let root = app
        .library
        .open_libraries
        .last()
        .expect("the stock library is open")
        .root
        .clone();
    TaskDriver::new()
        .send(
            &mut app,
            Message::Library(LibraryMessage::OpenLibraryBrowser(root.clone())),
        )
        .expect("the browser opened");
    assert!(app.library.library_browsers.contains_key(&root));
    (app, root)
}

/// The stock resistor row, which binds a symbol and a footprint.
fn resistor_row(app: &Signex, root: &Path) -> (String, signex_library::RowId) {
    app.library
        .library_at(root)
        .expect("mounted")
        .tables
        .iter()
        .find_map(|(table, rows)| {
            rows.iter()
                .find(|row| row.internal_pn.as_str() == "STOCK-R")
                .map(|row| (table.clone(), signex_library::RowId::from_uuid(row.row_id)))
        })
        .expect("the stock library has STOCK-R")
}

/// The texts of the Library Browser tab body as laid out in `theme`.
fn browser_texts(app: &Signex, root: &Path, theme: ThemeId) -> Vec<String> {
    let tokens = signex_types::theme::theme_tokens(theme);
    let browser = &app.library.library_browsers[root];
    crate::layout_probe::lay_out_texts(
        crate::library::browser::view(
            root,
            &app.library,
            browser,
            &tokens,
            &app.ui_state.i18n,
            PreviewLook::for_tests(),
        ),
        iced::Size::new(1440.0, 900.0),
    )
    .into_iter()
    .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
    .collect()
}

/// The Properties panel's symbol and footprint card lines for the row.
fn binding_summaries(app: &Signex) -> (String, String) {
    let detail = app
        .document_state
        .panel_ctx
        .library_row_detail
        .as_ref()
        .expect("the selected row reaches Properties");
    let plain = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
    (
        plain(detail.symbol_summary.to_string()),
        plain(detail.footprint_summary.to_string()),
    )
}

#[test]
fn a_selected_rows_preview_and_bindings_land_through_the_selections_task() {
    let (mut app, root) = app_with_stock_browser();
    let (table, row_id) = resistor_row(&app, &root);

    let task = app.update(Message::Library(LibraryMessage::BrowserSelectRow {
        library_path: root.clone(),
        table,
        row_id,
    }));

    // Before the returned task runs, both reads are only asked for.
    let preview = &app.library.library_browsers[&root].preview;
    assert!(
        preview.in_flight(),
        "the selection started the preview read"
    );
    assert!(preview.loaded.is_none());
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let texts = browser_texts(&app, &root, theme);
        let loading = texts.iter().filter(|t| *t == "Loading…").count();
        assert_eq!(loading, 2, "{theme:?}: both tiles wait: {texts:#?}");
    }
    assert_eq!(
        binding_summaries(&app),
        (
            "Reading symbol…".to_string(),
            "Reading footprint…".to_string()
        )
    );

    TaskDriver::new()
        .settle(&mut app, task)
        .expect("the selection's task settled");

    let browser = &app.library.library_browsers[&root];
    assert!(!browser.preview.in_flight(), "the preview read landed");
    assert_eq!(browser.preview_error, None);
    let loaded = browser.preview.loaded.as_deref().expect("the row was read");
    assert!(loaded.symbol.is_some(), "STOCK-R's symbol");
    assert!(loaded.footprint.is_some(), "STOCK-R's footprint");
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let texts = browser_texts(&app, &root, theme);
        for word in ["Loading…", "Could not be read"] {
            assert!(
                !texts.iter().any(|t| t == word),
                "{theme:?}: a tile still says {word}: {texts:#?}"
            );
        }
    }
    let (symbol, footprint) = binding_summaries(&app);
    assert!(symbol.starts_with("Symbol bound — "), "{symbol}");
    assert_eq!(footprint, "Footprint bound");
}

/// A selection whose task is dropped never reads: the state the audit
/// captures of mockup rows 118 and 127 showed. Later messages do not start
/// a second read while the first is still owed.
#[test]
fn a_dropped_selection_task_leaves_the_preview_waiting() {
    let (mut app, root) = app_with_stock_browser();
    let (table, row_id) = resistor_row(&app, &root);
    drop(
        app.update(Message::Library(LibraryMessage::BrowserSelectRow {
            library_path: root.clone(),
            table,
            row_id,
        })),
    );
    let generation = app.library.library_browsers[&root].preview.generation;

    drop(
        app.update(Message::Library(LibraryMessage::BrowserMoreMenuToggled {
            library_path: root.clone(),
        })),
    );

    let preview = &app.library.library_browsers[&root].preview;
    assert!(preview.in_flight());
    assert_eq!(preview.generation, generation, "no second read");
    assert!(preview.loaded.is_none());
}
