use std::path::{Path, PathBuf};

use signex_types::theme::{ThemeId, theme_tokens};

use super::*;
use crate::library::messages::LibraryMessage;
use crate::library::state::LibraryState;

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

/// #134: the library list is as tall as its rows, then scrolls.
#[test]
fn the_library_list_follows_its_rows_up_to_the_cap() {
    assert!(list_height(1) < list_height(2));
    assert!((list_height(2) - list_height(1) - view::ROW_H).abs() < 1e-3);
    assert_eq!(
        list_height(view::LIST_MAX_ROWS),
        list_height(view::LIST_MAX_ROWS + 10)
    );
}

/// #134: a click on a library row copies into it at once; there is no
/// separate select step (one-click copy is kept until m134 is decided).
#[test]
fn a_library_row_click_copies_into_that_library_at_once() {
    let root = Path::new("/libs/mine/Mine.snxlib");
    match view::row_message(root) {
        LibraryMessage::StockCopy(StockCopyMsg::TargetPicked(picked)) => {
            assert_eq!(picked, root);
        }
        other => panic!("a row click must copy at once, got {other:?}"),
    }
}

/// #134: the intro reads as the dialog's mockup words it.
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
