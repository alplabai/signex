//! Mockup row 128 (#3010): the Parameters tab shows the same count in a
//! read-only library as in an editable one. The row is opened through
//! `Signex::update`, as the runtime does, then the tab strip is laid out.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{ComponentClass, LibraryAdapter, PrimitiveRef, RowId};
use signex_types::theme::{ThemeId, theme_tokens};
use uuid::Uuid;

use super::{PreviewTab, graphite_badge, view_tabs};
use crate::app::path_ops::settle_task;
use crate::app::{Message, Signex};
use crate::layout_probe::{LaidOutText, find, lay_out_texts};
use crate::library::component_preview::test_row;
use crate::library::messages::LibraryMessage;
use crate::library::state::EditorAddress;
use crate::styles::PillTone;

const TABLE: &str = "R";

/// A library on disk at `path` holding one resistor row with no
/// parameters, so its three required template slots are missing.
fn library_on_disk(path: &Path) -> RowId {
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "parts".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let options = LibraryInitOptions {
        enable_git: false,
        use_lfs: false,
    };
    let adapter = LocalGitAdapter::init(path, manifest, options).expect("init library");
    let mut row = test_row();
    row.class = ComponentClass::new("resistor");
    row.symbol_ref = PrimitiveRef::new(adapter.library_id(), Uuid::now_v7());
    let id = RowId::from_uuid(row.row_id);
    adapter.insert_row(TABLE, row, "seed").expect("insert row");
    id
}

/// An app with the library mounted (read-only when `read_only`) and the
/// row opened in the Component editor through `update`.
fn opened(read_only: bool) -> (tempfile::TempDir, EditorAddress, Signex) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root: PathBuf = dir.path().join("parts.snxlib");
    let row_id = library_on_disk(&root);
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.library.pending_mounts.clear();
    let mounted = if read_only {
        app.library.open_library_read_only(root.clone())
    } else {
        app.library.open_library(root.clone())
    };
    mounted.expect("mount");
    assert_eq!(app.library.is_read_only(&root), read_only);
    let task = app.update(Message::Library(LibraryMessage::OpenComponentRow {
        library_path: root.clone(),
        table: TABLE.into(),
        row_id,
    }));
    settle_task(&mut app, task);
    let address = EditorAddress::new(root, TABLE.into(), row_id);
    assert!(
        app.library.editors.contains_key(&address),
        "the row is open in the editor"
    );
    (dir, address, app)
}

/// The tab strip's texts, without Fluent's isolation marks.
fn tab_texts(app: &Signex, address: &EditorAddress) -> Vec<LaidOutText> {
    let state = &app.library.editors[address];
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let i18n = signex_i18n::I18n::en_us_ref();
    lay_out_texts(
        view_tabs(state, &app.library, &tokens, i18n, address.clone()),
        iced::Size::new(1400.0, 100.0),
    )
    .into_iter()
    .map(|t| LaidOutText {
        text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
        bounds: t.bounds,
    })
    .collect()
}

/// The count drawn between the Parameters and Supply tab labels, if any.
fn parameters_count(texts: &[LaidOutText]) -> Option<String> {
    let parameters = find(texts, "Parameters").bounds;
    let supply = find(texts, "Supply").bounds;
    texts
        .iter()
        .find(|t| t.bounds.x >= parameters.x + parameters.width && t.bounds.x < supply.x)
        .map(|t| t.text.clone())
}

/// Headless: the badge the Parameters tab carries is the same for a
/// read-only and an editable library: the missing required slots, error
/// toned.
#[test]
fn the_parameters_badge_is_the_same_in_a_read_only_library() {
    let badge = |read_only| {
        let (_dir, address, app) = opened(read_only);
        let state = &app.library.editors[&address];
        graphite_badge(PreviewTab::Parameters, state, &app.library)
    };
    let editable = badge(false);
    assert_eq!(editable, Some(("3".to_string(), PillTone::Err)));
    assert_eq!(badge(true), editable, "read-only keeps the count");
}

/// Mockup row 128: the laid-out Parameters tab reads "Parameters 3" in a
/// read-only library as in an editable one.
#[test]
fn the_parameters_tab_shows_its_count_in_a_read_only_library() {
    for read_only in [false, true] {
        let (_dir, address, app) = opened(read_only);
        let texts = tab_texts(&app, &address);
        assert_eq!(
            parameters_count(&texts).as_deref(),
            Some("3"),
            "read-only {read_only}: {texts:#?}"
        );
    }
}
