//! Project-scope Find/Replace (#1268).

use std::sync::Arc;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::current_placement::placed_twice_on_disk;
use crate::find_replace::{FindMatch, FindReplaceMsg, FindScope, ProjectScan};

fn send(app: &mut Signex, msg: FindReplaceMsg) {
    let _ = app.update(crate::app::Message::FindReplaceMsg(msg));
}

fn project_search(app: &mut Signex, query: &str) {
    send(app, FindReplaceMsg::ScopeChanged(FindScope::Project));
    send(app, FindReplaceMsg::QueryChanged(query.into()));
    app.run_project_scan_blocking();
}

fn sheet_names(app: &Signex) -> Vec<String> {
    app.ui_state
        .find_replace
        .matches
        .iter()
        .map(|m| {
            m.sheet
                .as_deref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn project_scope_finds_a_label_on_a_closed_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    assert!(
        !app.document_state.engines.contains_key(&child),
        "the child has no engine"
    );

    project_search(&mut app, "child_net");

    assert_eq!(sheet_names(&app), ["child.snxsch"]);
    assert!(
        !app.document_state.engines.contains_key(&child),
        "searching opens nothing"
    );
}

#[test]
fn a_stale_scan_generation_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, _child) = placed_twice_on_disk(&dir);
    send(&mut app, FindReplaceMsg::ScopeChanged(FindScope::Project));
    send(&mut app, FindReplaceMsg::QueryChanged("child_net".into()));
    let stale = app.ui_state.find_replace.scan_generation;
    send(&mut app, FindReplaceMsg::QueryChanged("top_net".into()));
    assert!(app.ui_state.find_replace.scan_generation > stale);

    let bogus = FindMatch {
        sheet: Some(dir.path().join("child.snxsch")),
        hit: signex_engine::FindHit {
            kind: signex_engine::FindKind::NetLabel,
            item: signex_types::schematic::SelectedItem::new(
                uuid::Uuid::nil(),
                signex_types::schematic::SelectedKind::Label,
            ),
            target: signex_engine::TextTarget::Label(uuid::Uuid::nil()),
            text: "STALE".into(),
            placement: None,
            single: false,
            position: signex_types::schematic::Point::new(0.0, 0.0),
        },
        placement_label: None,
    };
    send(
        &mut app,
        FindReplaceMsg::ProjectScanFinished {
            generation: stale,
            result: Arc::new(Ok(ProjectScan {
                matches: vec![bogus],
                ..ProjectScan::default()
            })),
        },
    );

    assert!(app.ui_state.find_replace.matches.is_empty());
    assert!(
        app.ui_state.find_replace.scanning,
        "the newer scan is awaited"
    );
}
