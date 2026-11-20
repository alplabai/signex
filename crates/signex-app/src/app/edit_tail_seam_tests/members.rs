//! Review round of PR 2.0: the calls the seam moves that touch a sheet other
//! than the active one, or the ERC epoch: the by-path tail's epoch bump, the
//! multi-sheet finishers' dirty mark on non-active members (Number Schematic
//! Sheets, a board swap's linked parts), the grouped step's own epoch bump,
//! and the Close Project prompt.

use signex_engine::Command;

use super::{dirty, paper, quit_prompt_lists, replace_in_project};
use crate::app::command::queries::tests::{WIRE_A, app_with, path, uid};
use crate::app::handlers::editing_commands::net_color_driver as colours;
use crate::app::{Signex, UndoGroupKind, UndoGroupSheets};

fn epoch(app: &Signex) -> u64 {
    app.document_state.sheet_edit_epoch
}

fn colour_wire_a(r: u8) -> Command {
    let mut colors = std::collections::HashMap::new();
    colors.insert(uid(WIRE_A), colours::color(r));
    Command::SetNetColors { colors }
}

// ---- The by-path tail's epoch bump (`after_sheet_change`).

#[test]
fn a_font_or_paper_edit_by_path_bumps_the_erc_epoch_and_a_colour_does_not() {
    let mut app = app_with(&[("a.snxsch", 100), ("b.snxsch", 101)]);
    let a = path("a.snxsch");
    let font = Command::SetDocumentFontFamily {
        font_family: Some("Inter".to_string()),
    };
    for (what, command, bump) in [
        ("font", font, 1),
        ("paper", paper("A3"), 1),
        ("colour", colour_wire_a(7), 0),
    ] {
        let before = epoch(&app);
        assert_eq!(app.execute_on_sheet(&a, command), Ok(true), "{what}");
        assert_eq!(epoch(&app), before + bump, "{what}");
    }
}

// ---- The grouped step's own epoch bump (`apply_grouped_step`).

/// A colour on `a` (active) and `b` under one group: every member's patch is
/// display-only, so the active tail bumps nothing and the epoch moves only
/// by the grouped step's own unconditional bump.
#[test]
fn a_grouped_step_of_display_only_edits_still_bumps_the_erc_epoch_once() {
    let mut app = app_with(&[("b.snxsch", 101), ("a.snxsch", 100)]);
    let (a, b) = (path("a.snxsch"), path("b.snxsch"));
    let group = app.mint_undo_group();
    for sheet in [&a, &b] {
        let engine = app.document_state.engines.get_mut(sheet).unwrap();
        engine.begin_transaction().unwrap();
        engine.execute(colour_wire_a(8)).unwrap();
        engine.commit_transaction(Some(group)).unwrap();
    }
    let record = UndoGroupSheets {
        kind: UndoGroupKind::Replace,
        live: vec![a.clone(), b.clone()],
        ..Default::default()
    };
    app.document_state.undo_groups.insert(group, record);
    assert_eq!(
        app.document_state.engines[&a].next_undo_group(),
        Some(group)
    );

    let before = epoch(&app);
    app.handle_undo_requested();
    assert_eq!(
        app.document_state.engines[&b].next_redo_group(),
        Some(group),
        "both members stepped"
    );
    assert_eq!(epoch(&app), before + 1, "undo: the group's own bump");
    app.handle_redo_requested();
    assert_eq!(epoch(&app), before + 2, "redo: the group's own bump");
}

// ---- A multi-sheet finisher (`commit_sheet_write`) and its grouped undo.

/// Number Schematic Sheets over an open, active parent and a closed second
/// parent the run loads into a parked engine: a non-active member.
fn renumbered() -> crate::app::sheet_numbering::apply_tests::Rig {
    use crate::app::sheet_numbering::apply_tests::{closed_parent_sheet, rig};
    let mut rig = rig(Some(closed_parent_sheet()));
    let open = rig.open_parent.clone();
    let app = &mut rig.app;
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "a.snxsch".into(),
        path: open.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(open.clone());
    app.document_state.mark_document_clean(&open);
    rig
}

#[test]
fn a_sheet_numbering_run_round_trips_the_dirty_mark_of_its_non_active_member() {
    let mut rig = renumbered();
    let other = rig.closed_parent.clone();
    assert!(!dirty(&rig.app, &other), "starts clean");
    crate::app::sheet_numbering::apply_tests::apply(&mut rig);
    let app = &mut rig.app;
    let problem =
        (app.ui_state.sheet_numbering.as_ref()).and_then(|state| state.problem().cloned());
    assert!(problem.is_none(), "Apply ran: {problem:?}");
    assert_eq!(
        app.document_state.engines[&other].document().child_sheets[0].instances[0].page,
        "1",
        "the non-active member was renumbered"
    );
    assert!(dirty(app, &other), "dirty after the run");
    assert!(quit_prompt_lists(app, &other), "quit asks for it");
    app.handle_undo_requested();
    assert!(
        !app.document_state.engines.contains_key(&other),
        "the parked member went back to disk"
    );
    assert!(!dirty(app, &other), "clean after the grouped undo");
}

// ---- A board swap's linked part on a sheet that is not active.

#[test]
fn a_board_gate_swap_leaves_the_linked_non_active_sheet_dirty_and_quit_asks() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board, sheet) =
        crate::app::handlers::pcb_component::part_swap_tests::app_with_part(dir.path());
    assert_ne!(
        app.document_state.active_path.as_deref(),
        Some(sheet.as_path()),
        "the sheet is not the active one"
    );
    assert!(!dirty(&app, &sheet), "starts clean");
    let command = crate::keymap::AppCommandId::new("swap_gates").unwrap();
    let document = format!("document={}", board.display());
    let args = crate::app::command::CommandArgs::parse_operands(
        &command,
        &[document.as_str(), "designator=U1", "units=1,2"],
    )
    .unwrap();
    let ran = app.run_headless_command_in_test(&command, args).0;
    assert!(
        matches!(ran, crate::app::command::CommandOutcome::Answered(_)),
        "{ran:?}"
    );
    assert!(dirty(&app, &sheet), "the linked sheet is unsaved");
    assert!(quit_prompt_lists(&mut app, &sheet), "quit asks for it");
}

// ---- The grouped step's trailing panel refresh, and Close Project.

/// The red dot on `file`'s tree row, searched through nested rows.
fn red_dot(app: &Signex, file: &str) -> bool {
    fn find<'a>(
        rows: &'a [crate::panels::SheetInfo],
        file: &str,
    ) -> Option<&'a crate::panels::SheetInfo> {
        rows.iter().find_map(|row| {
            (row.filename == file)
                .then_some(row)
                .or_else(|| find(&row.children, file))
        })
    }
    let rows = &app.document_state.panel_ctx.projects[0].sheets;
    find(rows, file).expect("the sheet's tree row").is_dirty
}

/// `placed_twice_on_disk` with both sheets project pages and open as tabs,
/// `top` active.
fn both_open(dir: &tempfile::TempDir) -> (Signex, std::path::PathBuf, std::path::PathBuf) {
    use crate::app::handlers::menu::export::tests::current_placement::placed_twice_listing;
    let (mut app, top, child) = placed_twice_listing(dir, &["top.snxsch", "child.snxsch"]);
    assert!(app.open_or_focus_sheet_path(child.clone()));
    assert!(app.open_or_focus_sheet_path(top.clone()));
    (app, top, child)
}

#[test]
fn a_grouped_undo_clears_the_red_dot_of_its_non_active_member() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = both_open(&dir);
    replace_in_project(&mut app, "net", "X");
    assert!(
        red_dot(&app, "child.snxsch"),
        "the replace dirtied the child"
    );
    app.handle_undo_requested();
    assert!(!dirty(&app, &child));
    assert!(
        !red_dot(&app, "child.snxsch"),
        "its dot follows in the same call"
    );
}

/// Press Close Project on the first project and complete its resolve, as
/// `tests/regression/close_support.rs` does. The prompt, taken down again,
/// or `None` when the project closed without asking.
fn close_project(app: &mut Signex) -> Option<Vec<std::path::PathBuf>> {
    use crate::app::{ContextMenuMsg, Message, ProjectMsg, ProjectTreeAction};
    let id = app.document_state.projects[0].id;
    let _resolve = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::CloseProject(vec![id.to_string()]),
    )));
    let result = app.resolve_sheet_refs_blocking();
    let _closed = app.update(Message::Project(ProjectMsg::CloseRefsResolved {
        project_id: id,
        result,
    }));
    let prompt = app.ui_state.project_close_confirm.take()?;
    Some(prompt.dirty_paths)
}

#[test]
fn close_project_asks_for_both_sheets_of_a_replace_and_not_after_its_undo() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = both_open(&dir);
    replace_in_project(&mut app, "net", "X");
    let listed = close_project(&mut app).expect("Close Project asks after the edit");
    assert!(
        listed.contains(&top) && listed.contains(&child),
        "{listed:?}"
    );
    app.handle_undo_requested();
    assert_eq!(
        close_project(&mut app),
        None,
        "nothing to ask after the undo"
    );
    assert!(app.document_state.projects.is_empty(), "the project closed");
}

// ---- Annotate over an open sheet that is not the active one.

/// The open child is annotated in its engine and marked with
/// `mark_document_dirty`, which is not history-derived: the grouped undo
/// steps it back, yet it stays dirty (the root, history-derived, goes clean).
/// Pinned as today's behaviour; closing asks for a child equal to its file.
#[test]
fn an_annotate_leaves_its_open_non_active_child_dirty_after_the_undo_today() {
    use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
    let dir = tempfile::tempdir().unwrap();
    let (root, child) = (
        dir.path().join("a_root.snxsch"),
        dir.path().join("child.snxsch"),
    );
    let unnumbered = |children: &[&str]| {
        let mut sheet = sheet_with_net("R?", "NET", children);
        let mut second = sheet.symbols[0].clone();
        second.uuid = uuid::Uuid::now_v7();
        second.position = signex_types::schematic::Point::new(0.0, 20.0);
        sheet.symbols.push(second);
        sheet
    };
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a_root.snxsch"]);
    open_with(&mut app.document_state, &child, unnumbered(&[]));
    open_with(
        &mut app.document_state,
        &root,
        unnumbered(&["child.snxsch"]),
    );
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "a_root.snxsch".into(),
        path: root.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root.clone());
    for sheet in [&root, &child] {
        app.document_state.mark_document_clean(sheet);
    }
    app.ui_state.annotate_undo = crate::annotate_undo::AnnotateUndo::Project;
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    crate::app::path_ops::settle_task(&mut app, task);
    let record = app.document_state.undo_groups.values().next().cloned();
    assert_eq!(
        record.map(|r| r.live),
        Some(vec![root.clone(), child.clone()]),
        "one group over both sheets"
    );
    assert_eq!((dirty(&app, &root), dirty(&app, &child)), (true, true));
    app.handle_undo_requested();
    assert!(
        app.document_state.engines[&child]
            .next_redo_group()
            .is_some()
    );
    assert_eq!(
        app.document_state.engines[&child].next_redo_group(),
        app.document_state.engines[&root].next_redo_group(),
        "the child was stepped with the root"
    );
    assert_eq!(
        (dirty(&app, &root), dirty(&app, &child)),
        (false, true),
        "after undo"
    );
    app.handle_redo_requested();
    assert_eq!((dirty(&app, &root), dirty(&app, &child)), (true, true));
}
