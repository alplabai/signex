//! Refresh All Pricing at the app seam: re-entrancy refusal and what the
//! Messages panel receives.

use iced::Task;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use signex_library::{FailureKind, PricingRefreshReport, RowFailure, StoreOutcome};
use signex_types::project::{LibraryEntry, LibraryEntryKind};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{ContextMenuMsg, LibraryRowRef, Message, ProjectTreeAction, Signex};
use crate::library::messages::LibraryMessage;
use crate::library::state::{LibraryDisplaySettings, OpenLibrary};

fn lines_containing(needle: &str) -> Vec<String> {
    crate::diagnostics::recent_entries()
        .into_iter()
        .map(|e| e.message)
        .filter(|m| m.contains(needle))
        .collect()
}

/// A project listing one open library, plus the row that names it.
fn app_with_open_library(name: &str) -> (Signex, LibraryRowRef) {
    let _ = crate::diagnostics::init_logging();
    let mut app = app_workspace("/proj", &[]);
    let root = PathBuf::from(format!("/libs/{name}/{name}.snxlib"));
    app.document_state.projects[0]
        .data
        .libraries
        .push(LibraryEntry {
            path: root.clone(),
            kind: LibraryEntryKind::Shared,
            library_id: None,
            unknown: Default::default(),
        });
    app.refresh_panel_ctx();
    app.library.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: name.into(),
        library_id: Uuid::new_v4(),
        tables: Default::default(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    });
    let project_id = app.document_state.projects[0].id;
    (app, LibraryRowRef { project_id, root })
}

fn pick(app: &mut Signex, row: &LibraryRowRef) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::RefreshLibraryPricing(row.clone()),
    )));
}

#[test]
fn the_menu_action_starts_a_background_walk_without_blocking_update() {
    let (mut app, row) = app_with_open_library("PxStart");
    pick(&mut app, &row);
    let run = app
        .library
        .pricing_refresh
        .as_ref()
        .expect("a walk is recorded as running");
    assert_eq!(run.library_path, row.root);
    assert!(!run.cancel.load(Ordering::Relaxed));
    assert!(!lines_containing("Refresh All Pricing started for PxStart").is_empty());
}

#[test]
fn a_second_refresh_while_one_runs_is_refused_with_a_message() {
    let (mut app, row) = app_with_open_library("PxBusy");
    pick(&mut app, &row);
    let first = Arc::clone(&app.library.pricing_refresh.as_ref().unwrap().cancel);

    pick(&mut app, &row);

    let refused = lines_containing("Refresh All Pricing refused for PxBusy");
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(refused[0].contains("still running"));
    let still = app
        .library
        .pricing_refresh
        .as_ref()
        .expect("first walk kept");
    assert!(
        Arc::ptr_eq(&still.cancel, &first),
        "the running walk was not replaced"
    );
}

#[test]
fn finishing_frees_the_slot_and_reports_failures_and_a_summary() {
    let (mut app, row) = app_with_open_library("PxDone");
    pick(&mut app, &row);
    let report = PricingRefreshReport {
        rows_considered: 3,
        failures: vec![RowFailure {
            table: "R".into(),
            internal_pn: "R-7".into(),
            distributor: Some("Mouser".into()),
            kind: FailureKind::Auth,
            detail: "no Mouser API key in keyring".into(),
        }],
        store: StoreOutcome {
            stored: 2,
            dropped: Vec::new(),
        },
        ..Default::default()
    };

    let _ = app.update(Message::Library(LibraryMessage::PricingRefreshFinished {
        library_path: row.root.clone(),
        outcome: Arc::new(Ok(report)),
    }));

    assert!(app.library.pricing_refresh.is_none(), "slot freed");
    assert!(
        !lines_containing("Pricing PxDone: R / R-7: no Mouser API key in keyring (Mouser)")
            .is_empty(),
        "the failed row is named"
    );
    assert!(
        !lines_containing("PxDone: Pricing refresh: 2 updated, 1 failed (3 rows)").is_empty(),
        "the summary line"
    );
    // The slot is usable again.
    pick(&mut app, &row);
    assert!(app.library.pricing_refresh.is_some());
}

#[test]
fn a_walk_that_fails_as_a_whole_is_reported_and_frees_the_slot() {
    let (mut app, row) = app_with_open_library("PxFail");
    pick(&mut app, &row);
    let _ = app.update(Message::Library(LibraryMessage::PricingRefreshFinished {
        library_path: row.root.clone(),
        outcome: Arc::new(Err("library file is unreadable".into())),
    }));
    assert!(app.library.pricing_refresh.is_none());
    assert!(
        !lines_containing("Refresh All Pricing failed for PxFail: library file is unreadable")
            .is_empty()
    );
}

fn cancel_flag(app: &Signex) -> Arc<std::sync::atomic::AtomicBool> {
    Arc::clone(
        &app.library
            .pricing_refresh
            .as_ref()
            .expect("a walk is running")
            .cancel,
    )
}

#[test]
fn closing_the_library_cancels_its_walk() {
    let (mut app, row) = app_with_open_library("PxClose");
    pick(&mut app, &row);
    let flag = cancel_flag(&app);
    assert!(!flag.load(Ordering::Relaxed));
    app.library.close_library(&row.root);
    assert!(
        flag.load(Ordering::Relaxed),
        "a closed library must not be written to"
    );
}

#[test]
fn closing_another_library_leaves_the_walk_running() {
    let (mut app, row) = app_with_open_library("PxOther");
    pick(&mut app, &row);
    let flag = cancel_flag(&app);
    app.library
        .close_library(std::path::Path::new("/libs/Elsewhere/Elsewhere.snxlib"));
    assert!(!flag.load(Ordering::Relaxed));
}

#[test]
fn closing_the_project_leaves_its_librarys_walk_running() {
    let (mut app, row) = app_with_open_library("PxProj");
    pick(&mut app, &row);
    let flag = cancel_flag(&app);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::CloseProject(vec![0]),
    )));
    let result = app.resolve_sheet_refs_blocking();
    let _ = app.update(Message::Project(
        crate::app::ProjectMsg::CloseRefsResolved {
            project_id: row.project_id,
            result,
        },
    ));
    assert!(app.document_state.projects.is_empty(), "the project closed");
    assert!(
        !flag.load(Ordering::Relaxed),
        "the library stays mounted (and may be shared), so its walk finishes and saves"
    );
}

#[test]
fn quitting_cancels_the_walk() {
    let (mut app, row) = app_with_open_library("PxQuit");
    pick(&mut app, &row);
    let flag = cancel_flag(&app);
    let _ = app.flush_quit_writes_then(Task::none());
    assert!(flag.load(Ordering::Relaxed));
}
