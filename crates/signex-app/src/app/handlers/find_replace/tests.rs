//! Project-scope Find/Replace (#1268).

use std::sync::Arc;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::current_placement::{
    PLACEMENT_A, PLACEMENT_B, placed_twice_on_disk,
};
use crate::find_replace::{FindMatch, FindReplaceMsg, FindScope, ProjectScan, ReplacePrep};
use std::path::{Path, PathBuf};

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

fn replace_in_project(app: &mut Signex, query: &str, replacement: &str) {
    project_search(app, query);
    send(app, FindReplaceMsg::ReplacementChanged(replacement.into()));
    let matches = app.ui_state.find_replace.matches.clone();
    app.run_project_replace_blocking(matches);
}

fn first_label(app: &Signex, path: &Path) -> String {
    app.document_state.engines[path].document().labels[0]
        .text
        .clone()
}

fn undo(app: &mut Signex) {
    let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));
}

fn logged(fragment: &str) -> bool {
    crate::diagnostics::recent_entries()
        .iter()
        .any(|entry| entry.message.contains(fragment))
}

/// A project where the replace has run: `top` (open) and `child` (closed
/// beforehand, now a tabless engine) both carry a `NET` label.
fn after_replace() -> (Signex, tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    replace_in_project(&mut app, "net", "X");
    (app, dir, top, child)
}

#[test]
fn project_replace_all_is_one_undo_step_across_sheets() {
    let (mut app, _dir, top, child) = after_replace();
    assert_eq!(first_label(&app, &top), "TOP_X");
    assert_eq!(first_label(&app, &child), "CHILD_X");
    undo(&mut app);
    assert_eq!(first_label(&app, &top), "TOP_NET");
    assert_eq!(first_label(&app, &child), "CHILD_NET");

    // The same step, undone from the other sheet.
    let (mut app, _dir, top, child) = after_replace();
    assert!(app.open_or_focus_sheet_path(child.clone()));
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(child.as_path())
    );
    undo(&mut app);
    assert_eq!(first_label(&app, &child), "CHILD_NET");
    assert_eq!(first_label(&app, &top), "TOP_NET");
}

#[test]
fn project_replace_leaves_closed_sheets_dirty_and_unwritten() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    let before = std::fs::read(&child).unwrap();

    replace_in_project(&mut app, "child_net", "X");

    assert_eq!(first_label(&app, &child), "X");
    assert!(app.document_state.dirty_paths.contains(&child));
    assert_eq!(std::fs::read(&child).unwrap(), before, "disk is untouched");
    assert!(logged("Replaced 1 on 1 sheet: child.snxsch"));
}

#[test]
fn project_replace_recomputes_from_the_live_document_not_the_scan() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, _child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "top_net");
    let label = app.document_state.engines[&top].document().labels[0].uuid;
    assert_eq!(app.ui_state.find_replace.matches[0].hit.text, "TOP_NET");

    // An edit lands after the scan.
    assert!(app.apply_engine_command(
        signex_engine::Command::UpdateText {
            target: signex_engine::TextTarget::Label(label),
            value: "TOP_NET_LIVE".into(),
        },
        true,
        true,
    ));
    send(&mut app, FindReplaceMsg::ReplacementChanged("X".into()));
    let matches = app.ui_state.find_replace.matches.clone();
    app.run_project_replace_blocking(matches);

    // A stale scan text ("TOP_NET") would have written plain "X".
    assert_eq!(first_label(&app, &top), "X_LIVE");
}

#[test]
fn project_replace_never_shadows_an_open_tab_under_another_spelling() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    app.document_state.projects[0].case_fold = crate::app::state::scope::CaseFold::from_probe(true);
    assert!(app.open_or_focus_sheet_path(child.clone()));
    let label = app.document_state.engines[&child].document().labels[0].uuid;
    let hit = signex_engine::scan_sheet(
        app.document_state.engines[&child].document(),
        "child_net",
        &signex_engine::DesignatorScope::default(),
    )
    .remove(0);
    assert_eq!(hit.item.uuid, label);
    let other_spelling = dir.path().join("CHILD.snxsch");
    send(&mut app, FindReplaceMsg::QueryChanged("child_net".into()));
    send(&mut app, FindReplaceMsg::ReplacementChanged("X".into()));

    app.run_project_replace_blocking(vec![FindMatch {
        sheet: Some(other_spelling.clone()),
        hit,
        placement_label: None,
    }]);

    assert!(!app.document_state.engines.contains_key(&other_spelling));
    assert_eq!(
        first_label(&app, &child),
        "X",
        "the open tab's engine took it"
    );
    assert_eq!(app.document_state.engines.len(), 2, "top and child only");
}

#[test]
fn grouped_replace_undo_refused_after_a_later_edit_names_replace() {
    let (mut app, _dir, top, child) = after_replace();
    let label = app.document_state.engines[&child].document().labels[0].uuid;
    app.document_state
        .engines
        .get_mut(&child)
        .unwrap()
        .execute(signex_engine::Command::UpdateText {
            target: signex_engine::TextTarget::Label(label),
            value: "CHILD_Y".into(),
        })
        .unwrap();

    undo(&mut app);

    assert_eq!(first_label(&app, &top), "TOP_X", "nothing moved");
    assert_eq!(first_label(&app, &child), "CHILD_Y");
    assert!(logged("Cannot undo the project-wide replace"));
}

#[test]
fn designator_replace_in_a_twice_placed_sheet_changes_only_that_placement() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);

    replace_in_project(&mut app, "R101", "R300");

    let chain =
        |n: u128| signex_types::schematic::InstancePath::from_chain(vec![uuid::Uuid::from_u128(n)]);
    let symbol = &app.document_state.engines[&child].document().symbols[0];
    assert_eq!(symbol.designator_in(&chain(PLACEMENT_B)), "R300");
    assert_eq!(symbol.designator_in(&chain(PLACEMENT_A)), "R1");
    assert_eq!(symbol.reference, "R1");
}

#[test]
fn grouped_undo_of_a_background_label_refreshes_the_netlist() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    // The child as a tabless engine, as a project replace leaves it.
    let sheet =
        signex_types::format::SnxSchematic::parse(&std::fs::read_to_string(&child).unwrap())
            .unwrap()
            .sheet;
    let label = sheet.labels[0].uuid;
    app.document_state.engines.insert(
        child.clone(),
        signex_engine::Engine::new_with_path(sheet, Some(child.clone())).unwrap(),
    );
    app.document_state.dirty_paths.insert(child.clone());
    // One group: a child label rename, and on the active sheet an edit that
    // changes no connectivity, so the active sheet alone never refreshes the
    // netlist.
    let group = app.mint_undo_group();
    app.document_state
        .engines
        .get_mut(&child)
        .unwrap()
        .execute_batch_grouped(
            [signex_engine::Command::UpdateText {
                target: signex_engine::TextTarget::Label(label),
                value: "CHILD_X".into(),
            }],
            Some(group),
        )
        .unwrap();
    app.document_state
        .engines
        .get_mut(&top)
        .unwrap()
        .execute_batch_grouped(
            [signex_engine::Command::SetTitleBlockField {
                key: "title".into(),
                value: "t".into(),
            }],
            Some(group),
        )
        .unwrap();
    app.set_undo_group_sheets(
        group,
        crate::app::UndoGroupSheets {
            label: "replace",
            live: vec![top.clone(), child.clone()],
            ..Default::default()
        },
    );
    let net_names = |app: &mut Signex| -> Vec<String> {
        app.ui_state.project_netlist = None;
        app.refresh_project_netlist();
        app.ui_state
            .project_netlist
            .as_ref()
            .unwrap()
            .netlist
            .nets
            .iter()
            .map(|n| n.name.clone())
            .collect()
    };
    assert!(net_names(&mut app).iter().any(|n| n.contains("CHILD_X")));

    undo(&mut app);

    let names: Vec<String> = app
        .ui_state
        .project_netlist
        .as_ref()
        .expect("the netlist was re-derived")
        .netlist
        .nets
        .iter()
        .map(|n| n.name.clone())
        .collect();
    assert!(names.iter().any(|n| n.contains("CHILD_NET")), "{names:?}");
    assert!(!names.iter().any(|n| n.contains("CHILD_X")), "{names:?}");
}

#[test]
fn selecting_a_result_on_a_closed_sheet_opens_it_and_zooms() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "child_net");
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(top.as_path())
    );

    send(&mut app, FindReplaceMsg::SelectResult(0));

    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(child.as_path())
    );
    assert!(app.interaction_state.canvas.pending_fit.get().is_some());
    assert_eq!(app.interaction_state.active_canvas().selected.len(), 1);
}

#[test]
fn find_next_and_previous_step_through_the_results_and_wrap() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "net");
    assert_eq!(app.ui_state.find_replace.matches.len(), 2);
    let active = |app: &Signex| app.document_state.active_path.clone();

    send(&mut app, FindReplaceMsg::NextRequested);
    let first = active(&app);
    send(&mut app, FindReplaceMsg::NextRequested);
    let second = active(&app);
    assert_ne!(first, second);
    assert!(
        [&top, &child]
            .iter()
            .all(|p| [&first, &second].contains(&&Some((*p).clone())))
    );
    send(&mut app, FindReplaceMsg::PreviousRequested);
    assert_eq!(active(&app), first);
}

fn deliver(app: &mut Signex, generation: u64, prep: ReplacePrep) {
    send(
        app,
        FindReplaceMsg::ProjectReplacePrepared {
            generation,
            result: Arc::new(Ok(prep)),
        },
    );
}

fn set_replacement(app: &mut Signex, text: &str) {
    send(app, FindReplaceMsg::ReplacementChanged(text.into()));
}

fn make_newer_minor(path: &Path) {
    let text = std::fs::read_to_string(path).unwrap();
    let newer = text.replacen("format = \"snxsch/1\"", "format = \"snxsch/1.7\"", 1);
    assert_ne!(text, newer, "the header was rewritten");
    std::fs::write(path, newer).unwrap();
}

#[test]
fn a_document_opened_mid_flight_refuses_the_whole_replace() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "child_net");
    set_replacement(&mut app, "X");
    let matches = app.ui_state.find_replace.matches.clone();
    let (generation, prep) = app.prepare_project_replace_blocking(matches).unwrap();

    // The child is opened and edited while the disk was being read.
    assert!(app.open_or_focus_sheet_path(child.clone()));
    let label = app.document_state.engines[&child].document().labels[0].uuid;
    assert!(app.apply_engine_command(
        signex_engine::Command::UpdateText {
            target: signex_engine::TextTarget::Label(label),
            value: "EDITED".into(),
        },
        true,
        true,
    ));
    deliver(&mut app, generation, prep);

    assert_eq!(
        first_label(&app, &child),
        "EDITED",
        "the edit was not overwritten"
    );
    assert!(
        app.document_state.undo_groups.is_empty(),
        "no group was made"
    );
    assert!(logged("Replace cancelled: a document changed"));
    assert!(app.ui_state.find_replace.replace_job.is_none());
}

#[test]
fn closing_the_project_mid_flight_cancels_the_replace() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "child_net");
    set_replacement(&mut app, "X");
    let matches = app.ui_state.find_replace.matches.clone();
    let (generation, prep) = app.prepare_project_replace_blocking(matches).unwrap();
    assert!(app.ui_state.find_replace.is_busy());

    let project_id = app.document_state.projects[0].id;
    let result = app.resolve_sheet_refs_blocking();
    let _ = app.update(crate::app::Message::Project(
        crate::app::ProjectMsg::CloseRefsResolved { project_id, result },
    ));
    assert!(app.document_state.projects.is_empty(), "the project closed");
    assert!(
        !app.ui_state.find_replace.is_busy(),
        "the job was cancelled"
    );
    assert!(logged("Replace cancelled: the project was closed"));

    deliver(&mut app, generation, prep);
    assert!(!app.document_state.engines.contains_key(&child));
    assert!(app.document_state.undo_groups.is_empty());
}

#[test]
fn a_double_replace_all_replaces_once_and_a_stale_prepared_is_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "child_net");
    set_replacement(&mut app, "X");
    send(&mut app, FindReplaceMsg::ReplaceAll);
    let live = app
        .ui_state
        .find_replace
        .replace_job
        .clone()
        .expect("a job");
    send(&mut app, FindReplaceMsg::ReplaceAll);
    assert_eq!(
        app.ui_state
            .find_replace
            .replace_job
            .as_ref()
            .map(|j| j.generation),
        Some(live.generation),
        "the second click did not start another job"
    );

    // An answer to some older job does not steal the live one.
    deliver(
        &mut app,
        live.generation - 1,
        ReplacePrep {
            hierarchy: None,
            loaded: Default::default(),
            unreadable: Vec::new(),
        },
    );
    assert!(app.ui_state.find_replace.replace_job.is_some());

    let matches = app
        .ui_state
        .find_replace
        .replace_job
        .take()
        .unwrap()
        .matches;
    app.run_project_replace_blocking(matches);
    assert_eq!(first_label(&app, &child), "X");
    assert_eq!(
        app.document_state.undo_groups.len(),
        1,
        "one group, replaced once"
    );
}

#[test]
fn undo_then_redo_of_a_replace_that_parked_a_closed_sheet() {
    let (mut app, _dir, top, child) = after_replace();
    assert!(app.document_state.dirty_paths.contains(&child));

    undo(&mut app);
    assert_eq!(first_label(&app, &child), "CHILD_NET");
    assert!(
        !app.document_state.dirty_paths.contains(&child),
        "back at what was loaded from disk, so nothing to save"
    );

    let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Redo));
    assert_eq!(first_label(&app, &child), "CHILD_X");
    assert_eq!(first_label(&app, &top), "TOP_X");
    assert!(
        app.document_state.dirty_paths.contains(&child),
        "redo is an unsaved edit"
    );
}

#[test]
fn a_result_added_after_the_scan_is_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, _child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "top_net");
    let mut extra = app.document_state.engines[&top].document().labels[0].clone();
    extra.uuid = uuid::Uuid::new_v4();
    extra.text = "EXTRA_TOP_NET".into();
    assert!(app.apply_engine_command(
        signex_engine::Command::PlaceLabel { label: extra },
        true,
        true,
    ));
    set_replacement(&mut app, "X");
    let matches = app.ui_state.find_replace.matches.clone();
    app.run_project_replace_blocking(matches);

    let texts: Vec<String> = app.document_state.engines[&top]
        .document()
        .labels
        .iter()
        .map(|l| l.text.clone())
        .collect();
    assert!(texts.contains(&"X".to_string()), "{texts:?}");
    assert!(texts.contains(&"EXTRA_TOP_NET".to_string()), "{texts:?}");
}

#[test]
fn a_shown_result_that_changed_since_the_search_is_reported_and_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, _child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "top_net");
    let label = app.document_state.engines[&top].document().labels[0].uuid;
    assert!(app.apply_engine_command(
        signex_engine::Command::UpdateText {
            target: signex_engine::TextTarget::Label(label),
            value: "VCC".into(),
        },
        true,
        true,
    ));
    set_replacement(&mut app, "X");
    let matches = app.ui_state.find_replace.matches.clone();
    app.run_project_replace_blocking(matches);

    assert_eq!(first_label(&app, &top), "VCC");
    assert!(logged(
        "1 result changed since the search and was left alone"
    ));
}

#[test]
fn read_only_and_unreadable_sheets_are_skipped_by_the_scan_and_named() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    make_newer_minor(&child);
    project_search(&mut app, "child_net");
    assert!(app.ui_state.find_replace.matches.is_empty());
    assert!(
        app.ui_state
            .find_replace
            .skipped_note
            .as_deref()
            .is_some_and(|n| n.contains("child.snxsch") && n.contains("read-only"))
    );
    assert!(logged("Find: not searched, read-only"));

    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    std::fs::write(&child, "not a sheet").unwrap();
    project_search(&mut app, "child_net");
    assert!(app.ui_state.find_replace.matches.is_empty());
    assert!(logged("Find: not searched, could not be parsed"));
}

#[test]
fn read_only_and_unreadable_sheets_are_skipped_by_the_replace_and_named() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "child_net");
    set_replacement(&mut app, "X");
    let matches = app.ui_state.find_replace.matches.clone();
    make_newer_minor(&child);
    app.run_project_replace_blocking(matches);
    assert!(
        !app.document_state.engines.contains_key(&child),
        "no engine left behind"
    );
    assert!(logged("Replace: left unchanged, read-only"));

    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "child_net");
    set_replacement(&mut app, "X");
    let matches = app.ui_state.find_replace.matches.clone();
    std::fs::write(&child, "not a sheet").unwrap();
    app.run_project_replace_blocking(matches);
    assert!(!app.document_state.engines.contains_key(&child));
    assert!(logged("Replace: skipped, could not be parsed"));
}

#[test]
fn a_root_sheet_designator_replace_writes_the_reference() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, _child) = placed_twice_on_disk(&dir);

    replace_in_project(&mut app, "r_top", "R_NEW");

    let symbol = &app.document_state.engines[&top].document().symbols[0];
    assert_eq!(symbol.reference, "R_NEW");
    assert!(
        symbol.instances.is_empty(),
        "the root chain carries no entries"
    );
}

#[test]
fn a_singly_placed_sheet_designator_replace_writes_reference_and_drops_the_entry() {
    use crate::app::runtime::designators_tests::placed_once_with_an_entry;
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_once_with_an_entry(&dir);

    replace_in_project(&mut app, "R7", "R9");

    let symbol = &app.document_state.engines[&child].document().symbols[0];
    assert_eq!(symbol.reference, "R9");
    assert!(
        symbol.instances.is_empty(),
        "the placement's entry is dropped"
    );
}

#[test]
fn sheet_scope_replace_only_touches_the_active_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    send(&mut app, FindReplaceMsg::QueryChanged("net".into()));
    send(&mut app, FindReplaceMsg::ReplacementChanged("X".into()));
    assert_eq!(
        app.ui_state.find_replace.matches.len(),
        1,
        "the active sheet only"
    );

    send(&mut app, FindReplaceMsg::ReplaceAll);

    assert_eq!(first_label(&app, &top), "TOP_X");
    assert!(!app.document_state.engines.contains_key(&child));
    assert!(
        app.document_state.undo_groups.is_empty(),
        "sheet scope is no group"
    );
    undo(&mut app);
    assert_eq!(first_label(&app, &top), "TOP_NET");
}

#[test]
fn sheet_scope_replace_ignores_results_left_over_from_another_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    project_search(&mut app, "net");
    // Back to sheet scope without a refresh: the list still names the child.
    app.ui_state.find_replace.scope = FindScope::Sheet;
    app.ui_state.find_replace.replacement = "X".into();
    app.ui_state
        .find_replace
        .matches
        .retain(|m| m.sheet.as_deref() == Some(child.as_path()));

    send(&mut app, FindReplaceMsg::ReplaceAll);

    assert_eq!(first_label(&app, &top), "TOP_NET");
    assert!(!app.document_state.engines.contains_key(&child));
}

#[test]
fn f3_and_shift_f3_resolve_to_next_and_previous() {
    use crate::app::command::bridge::core_to_message as resolve;
    let command = |id: &str| crate::keymap::AppCommandId::new(id).expect("a catalog id");
    assert!(matches!(
        resolve(&command("find_next")),
        Some(crate::app::Message::FindReplaceMsg(
            FindReplaceMsg::NextRequested
        ))
    ));
    assert!(matches!(
        resolve(&command("find_previous")),
        Some(crate::app::Message::FindReplaceMsg(
            FindReplaceMsg::PreviousRequested
        ))
    ));
}
