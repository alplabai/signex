use std::path::{Path, PathBuf};

use signex_types::theme::{ThemeId, theme_tokens};

use super::*;
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
        drop(view(&library, &state, &tokens, false));
        drop(view(&library, &state, &tokens, true));
    }
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
