use std::path::{Path, PathBuf};

use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_types::theme::{ThemeId, theme_tokens};

use super::view::{FooterAction, footer_label, footer_message, footer_plan};
use super::*;
use crate::library::messages::LibraryMessage;
use crate::library::state::LibraryState;
use crate::styles::DialogButtonKind;

/// A new library without Git at `path`, called `name`.
fn new_library(path: &Path, name: &str) {
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: uuid::Uuid::now_v7(),
        library: LibrarySection {
            name: name.into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    LocalGitAdapter::init(
        path,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init library");
}

/// Two writable libraries, One and Two, mounted in `library`; returns
/// their `.snxlib` paths.
fn mount_two(dir: &Path, library: &mut LibraryState) -> (PathBuf, PathBuf) {
    let one = dir.join("one/One.snxlib");
    let two = dir.join("two/Two.snxlib");
    new_library(&one, "One");
    new_library(&two, "Two");
    library.open_library(one.clone()).expect("mount one");
    library.open_library(two.clone()).expect("mount two");
    (one, two)
}

fn dialog_state() -> StockCopyState {
    StockCopyState {
        source: PathBuf::from("/stock/symbols/R.snxsym"),
        kind: StockPrimitiveKind::Symbol,
        label: "R".into(),
        previous_unavailable: None,
        error: None,
        selected: None,
    }
}

/// Mockup row 134: the library used last is preselected; a row the user
/// selects wins while it can take the copy; with neither, the first
/// writable library is preselected, and with no writable library nothing.
#[test]
fn the_last_used_library_is_preselected_and_a_selection_wins() {
    assert_eq!(LibraryState::default().stock_copy_choice(None), None);
    let dir = tempfile::tempdir().expect("tempdir");
    let mut library = LibraryState::default();
    let (one, two) = mount_two(dir.path(), &mut library);
    let first = library.writable_libraries()[0].0.to_path_buf();
    let root = |choice: Option<(&Path, &str)>| choice.map(|(root, _)| root.to_path_buf());

    library.stock_copy_target = None;
    assert_eq!(root(library.stock_copy_choice(None)), Some(first));

    library.stock_copy_target = Some(two.clone());
    assert_eq!(root(library.stock_copy_choice(None)), Some(two.clone()));
    assert_eq!(
        library.stock_copy_choice(Some(one.as_path())),
        Some((one.as_path(), "One"))
    );
    let gone = dir.path().join("gone/Gone.snxlib");
    assert_eq!(
        root(library.stock_copy_choice(Some(gone.as_path()))),
        Some(two),
        "a selection that cannot take the copy falls back to the preselect"
    );
}

/// Mockup row 134 (layout): with a library to copy into, the footer is
/// New Library… quiet at the left, then Cancel and the one primary
/// "Copy to <library>", which copies there. With none, New Library… is the
/// primary and nothing sits at the left.
#[test]
fn the_footer_is_cancel_and_one_copy_to_primary_with_new_library_at_the_left() {
    let i18n = signex_i18n::I18n::en_us();
    let root = Path::new("/libs/passives/passives.snxlib");
    let plan = footer_plan(Some((root, "passives")));
    let copy_to = FooterAction::CopyTo(root.to_path_buf(), "passives".into());
    assert_eq!(
        plan.left,
        vec![(FooterAction::NewLibrary, DialogButtonKind::Ghost)]
    );
    assert_eq!(
        plan.right,
        vec![
            (FooterAction::Cancel, DialogButtonKind::Cancel),
            (copy_to.clone(), DialogButtonKind::Primary),
        ]
    );
    assert_eq!(
        footer_label(&copy_to, &i18n)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], ""),
        "Copy to passives"
    );
    match footer_message(&copy_to) {
        LibraryMessage::StockCopy(StockCopyMsg::TargetPicked(picked)) => assert_eq!(picked, root),
        other => panic!("the primary must copy into the selected library, got {other:?}"),
    }

    let empty = footer_plan(None);
    assert!(empty.left.is_empty(), "{:?}", empty.left);
    assert_eq!(
        empty.right,
        vec![
            (FooterAction::Cancel, DialogButtonKind::Cancel),
            (FooterAction::NewLibrary, DialogButtonKind::Primary),
        ]
    );
}

/// Mockup row 134 (update): selecting a row only moves the selection, and
/// nothing is copied; the footer's Copy to <library> then starts the copy
/// into the selected library and closes the dialog.
#[test]
fn selecting_a_library_copies_nothing_until_the_copy_is_confirmed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _boot) = crate::app::Signex::new();
    app.library.stock_copy_target = None;
    let (_one, two) = mount_two(dir.path(), &mut app.library);
    let two_dir = two.parent().expect("dir").to_path_buf();
    app.library.stock_copy = Some(dialog_state());
    let stock = |msg| crate::app::Message::Library(LibraryMessage::StockCopy(msg));

    drop(app.update(stock(StockCopyMsg::TargetSelected(two.clone()))));

    let open = app
        .library
        .stock_copy
        .as_ref()
        .expect("the dialog stays open");
    assert_eq!(open.selected.as_deref(), Some(two.as_path()));
    assert_eq!(
        app.library.stock_copy_choice(open.selected.as_deref()),
        Some((two.as_path(), "Two"))
    );
    assert!(app.library.stock_copies_in_flight.is_empty(), "no copy yet");
    assert!(!two_dir.join("symbols/R.snxsym").exists());

    let plan = footer_plan(app.library.stock_copy_choice(open.selected.as_deref()));
    let (confirm, _) = plan.right.last().expect("primary").clone();
    drop(app.update(crate::app::Message::Library(footer_message(&confirm))));

    assert!(
        app.library.stock_copy.is_none(),
        "confirming closes the dialog"
    );
    assert!(
        app.library
            .stock_copies_in_flight
            .contains(Path::new("/stock/symbols/R.snxsym")),
        "the copy into the selected library has started"
    );
}

#[test]
fn the_kind_comes_from_the_extension() {
    assert_eq!(
        StockPrimitiveKind::of_path(Path::new("/s/symbols/R.snxsym")),
        Some(StockPrimitiveKind::Symbol)
    );
    assert_eq!(
        StockPrimitiveKind::of_path(Path::new("/s/footprints/x.SNXFPT")),
        Some(StockPrimitiveKind::Footprint)
    );
    assert_eq!(
        StockPrimitiveKind::of_path(Path::new("/s/Stock.snxlib")),
        None
    );
}

#[test]
fn the_dialog_draws_in_every_theme_from_tokens_alone() {
    let library = LibraryState::default();
    let state = StockCopyState {
        source: PathBuf::from("/stock/symbols/R.snxsym"),
        kind: StockPrimitiveKind::Symbol,
        label: "R".into(),
        previous_unavailable: Some(PathBuf::from("/gone/Gone.snxlib")),
        error: Some("not writable".into()),
        selected: None,
    };
    for id in ThemeId::BUILTINS {
        let tokens = theme_tokens(*id);
        drop(view(
            &library,
            &state,
            &tokens,
            &signex_i18n::I18n::en_us(),
            false,
        ));
        drop(view(
            &library,
            &state,
            &tokens,
            &signex_i18n::I18n::en_us(),
            true,
        ));
    }
}

/// Mockup row 134: the library list is as tall as its rows, then scrolls.
#[test]
fn the_library_list_follows_its_rows_up_to_the_cap() {
    assert!(list_height(1) < list_height(2));
    assert!((list_height(2) - list_height(1) - view::ROW_H).abs() < 1e-3);
    assert_eq!(
        list_height(view::LIST_MAX_ROWS),
        list_height(view::LIST_MAX_ROWS + 10)
    );
}

/// Mockup row 134: a click on a library row selects it; the copy waits for
/// the footer's Copy to <library> (select, then confirm).
#[test]
fn a_library_row_click_selects_that_library() {
    let root = Path::new("/libs/mine/Mine.snxlib");
    match view::row_message(root) {
        LibraryMessage::StockCopy(StockCopyMsg::TargetSelected(picked)) => {
            assert_eq!(picked, root);
        }
        other => panic!("a row click must only select, got {other:?}"),
    }
}

/// Mockup row 134: the intro reads as the dialog's mockup words it.
#[test]
fn the_intro_names_the_primitive_and_says_the_stock_stays() {
    let i18n = signex_i18n::I18n::en_us();
    let (title, text) = intro_texts(&i18n, "R");
    assert_eq!(
        title.replace(['\u{2068}', '\u{2069}'], ""),
        "Copy R to a library"
    );
    assert_eq!(
        text,
        "The stock library is read-only. The copy opens for editing; the stock original is not changed."
    );
}

#[test]
fn the_remembered_target_round_trips_through_prefs_json() {
    let dir = tempfile::tempdir().expect("tempdir");
    let prefs = dir.path().join("prefs.json");
    std::fs::write(&prefs, r#"{"unit":"Mm"}"#).expect("seed prefs");
    let target = dir.path().join("Mine/Mine.snxlib");

    crate::fonts::write_stock_copy_target_pref_at(&prefs, &target);

    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&prefs).expect("read")).expect("json");
    assert_eq!(
        crate::fonts::read_stock_copy_target_pref_from(Some(&json)),
        Some(target)
    );
    assert_eq!(json["unit"], "Mm", "other keys survive the write");
    assert_eq!(crate::fonts::read_stock_copy_target_pref_from(None), None);
    assert_eq!(
        crate::fonts::read_stock_copy_target_pref_from(Some(&serde_json::json!({
            "stock_copy_library": ""
        }))),
        None
    );
}
