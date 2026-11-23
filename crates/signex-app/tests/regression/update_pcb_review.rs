//! #1312 review fixes (PR #2271): failures reach Messages, New PCB repairs a
//! named board that is gone, the stale check covers every sheet the read
//! used, the net rows follow a footprint untick, and a new board's save
//! writes its project too. Fixture and helpers: `update_pcb.rs`.

use signex_app::diagnostics::{DiagnosticLevel, recent_entries};

use super::*;

fn warned(parts: &[&str]) -> bool {
    recent_entries()
        .iter()
        .any(|e| e.level == DiagnosticLevel::Warning && parts.iter().all(|p| e.message.contains(p)))
}

fn pad_net_rows(f: &Fixture) -> usize {
    list(f)
        .rows
        .iter()
        .filter(|r| matches!(r.change, EcoChange::PadNet { .. }))
        .count()
}

fn close_tab(f: &mut Fixture, path: &Path) {
    let index = f
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab");
    let window = f.app.ui_state.main_window_id.expect("main window");
    send(
        &mut f.app,
        Message::Tab {
            window_id: window,
            msg: signex_app::tab_bar::TabMessage::CloseClicked(index),
        },
    );
    assert!(
        f.app.document_state.tabs.iter().all(|t| t.path != path),
        "the tab closed"
    );
}

/// A Net Parameters directive on the R1 to R2 wire, asking for `Width_PWR`.
fn fixture_with_a_schematic_rule() -> Fixture {
    fixture_editing(
        |_, sheet| {
            sheet
                .directives
                .push(signex_types::schematic::SchDirective::NetParameters {
                    uuid: Uuid::from_u128(0x2734_0001),
                    position: Point::new(15.0, 0.0),
                    net_class: Some("PWR".to_string()),
                    diff_pair_class: None,
                    rules: signex_types::schematic::NetRules {
                        width_mm: Some(0.5),
                        ..Default::default()
                    },
                    show_on_flag: false,
                });
        },
        Some((board_of(Vec::new()), false)),
        ProjectDefaults::default(),
    )
}

/// #2734: rules this build cannot read refuse Execute before the board step.
#[test]
fn unreadable_rules_refuse_execute_with_nothing_applied() {
    let mut f = fixture_with_a_schematic_rule();
    let stored = serde_json::json!({ "schema": 999, "drc": { "dsl": "from a newer build" } });
    f.app.document_state.projects[0].data.unknown =
        serde_json::from_value(serde_json::json!({ "rules": stored })).expect("unknown keys");
    assert!(f.app.document_state.projects[0].data.has_unreadable_rules());
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    assert!(add_row(&f, R1).on, "one part is ticked");
    let revision = engine(&f).revision();

    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);

    let Some(Err(why)) = state(&f).validation() else {
        panic!("the dialog stays open with the reason");
    };
    assert!(why.contains("newer Signex"), "{why}");
    assert_eq!(engine(&f).revision(), revision, "the board did not move");
    assert_eq!(engine(&f).undo_len(), 0, "nothing in the board history");
    assert!(references(&f).is_empty(), "no part was placed");
    let data = &f.app.document_state.projects[0].data;
    assert_eq!(
        data.unknown.get("rules"),
        Some(&stored),
        "the rules are kept"
    );
}

/// #2734: readable rules still land with the board step.
#[test]
fn execute_writes_the_schematic_rule_with_the_board_step() {
    let mut f = fixture_with_a_schematic_rule();
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert!(f.app.ui_state.update_pcb.is_none(), "Execute closes it");
    assert_eq!(references(&f), ["R1", "R2"]);
    let rules = f.app.document_state.projects[0].data.rules.as_ref();
    let drc = &rules.expect("the rules are stored").drc;
    assert!(drc.dsl.as_deref().is_some_and(|t| t.contains("Width_PWR")));
    assert!(drc.from_schematic.iter().any(|name| name == "Width_PWR"));
}

/// S3: a failure in dialog mode is in Messages, not only on the dialog.
#[test]
fn a_failed_read_in_the_dialog_reaches_messages() {
    let mut f = fixture();
    f.app.document_state.projects[0].data.pcb_file = Some("s3-gone-2271.snxpcb".to_string());
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    assert!(state(&f).failure().is_some(), "the dialog says why");
    assert!(
        warned(&["Update PCB did not run", "s3-gone-2271.snxpcb"]),
        "the failure is in Messages"
    );
}

/// S4: the project names a board that is neither open nor on disk. New PCB
/// makes it at that path, and the dialog that said it is missing lists.
#[test]
fn new_pcb_remakes_a_named_board_that_is_gone() {
    let mut f = fixture();
    f.app.document_state.projects[0].data.pcb_file = Some("demo.snxpcb".to_string());
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    assert!(state(&f).failure().is_some(), "the board is missing");
    // The user dismisses the open-failed card, if one came up.
    f.app.document_state.error_notice = None;

    send(&mut f.app, menu_row("new_pcb_document"));
    crate::document_setup::create_if_asked(&mut f.app);
    assert!(
        f.app.document_state.error_notice.is_none(),
        "New PCB is not refused"
    );
    assert!(
        engine(&f).board().footprints.is_empty(),
        "the board is made"
    );
    assert_eq!(
        f.app.document_state.projects[0].data.pcb_file.as_deref(),
        Some("demo.snxpcb"),
        "the project keeps the name it had"
    );
    assert!(add_row(&f, R1).on, "the dialog reads the new board");
}

/// S4 minor: a failed disk check refuses New PCB with why, not as "exists".
#[test]
fn a_failed_disk_check_refuses_new_pcb_with_why() {
    let mut f = fixture();
    let project = f.app.document_state.projects[0].id;
    up(
        &mut f.app,
        UpdatePcbMsg::NewPcbPathChecked {
            project,
            path: f.board.clone(),
            exists: Err("s4-denied-2271".to_string()),
            rules: None,
            fab: None,
        },
    );
    assert!(f.app.document_state.tabs.iter().all(|t| t.path != f.board));
    assert!(f.app.document_state.projects[0].data.pcb_file.is_none());
    assert!(warned(&[
        "Could not check whether",
        "demo.snxpcb",
        "s4-denied-2271"
    ]));
}

/// #2311: a fab tier picked for a new board writes its `fab_*` rules into
/// the project's rules, one undo step, nothing on disk until Save.
#[test]
fn a_new_board_with_a_fab_tier_seeds_its_fab_rules() {
    use signex_types::project::FabTemplate;
    let mut f = fixture();
    let project = f.app.document_state.projects[0].id;
    let on_disk = std::fs::read(&f.app.document_state.projects[0].path).ok();
    up(
        &mut f.app,
        UpdatePcbMsg::NewPcbPathChecked {
            project,
            path: f.board.clone(),
            exists: Ok(false),
            rules: None,
            fab: Some(FabTemplate::Hdi),
        },
    );
    let rules = f.app.document_state.projects[0].data.rules.clone();
    let dsl = rules.and_then(|r| r.drc.dsl).unwrap_or_default();
    assert!(
        dsl.contains("rule fab_track on track require width >= 0.075mm then error"),
        "{dsl}"
    );
    assert_eq!(dsl.matches("rule fab_").count(), 7, "{dsl}");
    assert_eq!(
        std::fs::read(&f.app.document_state.projects[0].path).ok(),
        on_disk,
        "nothing is written before Save"
    );
}

/// S5: a sheet edited after the read makes the list stale; Validate and
/// Execute refuse it.
#[test]
fn a_sheet_edited_after_the_read_makes_the_list_stale() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    assert!(!f.app.update_pcb_stale());
    let sheet = f.sheet.clone();
    f.app
        .document_state
        .engines
        .get_mut(&sheet)
        .expect("the sheet is open")
        .execute(signex_engine::Command::SetSymbolDnp {
            symbol_id: R1,
            dnp: true,
        })
        .expect("edit after the read");
    assert!(f.app.update_pcb_stale(), "the sheet moved past the read");

    up(&mut f.app, UpdatePcbMsg::ValidateRequested);
    let verdict = state(&f).validation().cloned().expect("a verdict");
    let why = verdict.expect_err("Validate refuses a stale list");
    assert!(why.contains("changed after this list"), "{why}");
    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert!(references(&f).is_empty(), "nothing was placed");
}

/// S5: a sheet open at the read and closed since makes the list stale.
#[test]
fn a_sheet_closed_after_the_read_makes_the_list_stale() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    let sheet = f.sheet.clone();
    close_tab(&mut f, &sheet);
    assert!(f.app.update_pcb_stale(), "the sheet's engine is gone");
    assert!(f.app.update_pcb_execute_block().is_some());
}

/// S5: a sheet read from disk and opened since makes the list stale.
#[test]
fn a_sheet_opened_after_a_disk_read_makes_the_list_stale() {
    let mut f = fixture();
    send(&mut f.app, menu_row("new_pcb_document"));
    crate::document_setup::create_if_asked(&mut f.app);
    let sheet = f.sheet.clone();
    close_tab(&mut f, &sheet);
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    assert!(add_row(&f, R1).on, "read from disk");
    assert!(!f.app.update_pcb_stale());

    send(&mut f.app, Message::File(FileMsg::Opened(Some(sheet))));
    assert!(f.app.update_pcb_stale(), "an engine the read did not see");
}

/// Unticking a footprint row recomputes the net rows; Report waits for
/// them.
#[test]
fn unticking_a_footprint_row_recomputes_the_net_rows() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    let before = pad_net_rows(&f);
    assert!(before > 0, "R1 and R2 pads get nets");
    let key = add_row(&f, R2).key.clone();
    let refresh = f
        .app
        .update(Message::from(UpdatePcbMsg::RowToggled { key, on: false }));
    assert!(!state(&f).is_ready(), "the net rows are being recomputed");
    let report = f.app.update(Message::from(UpdatePcbMsg::ReportRequested));
    assert_eq!(report.units(), 0, "Report waits for the net rows");

    crate::drive::drive(&mut f.app, refresh);
    assert!(
        pad_net_rows(&f) < before,
        "R2's pads are no longer given nets"
    );
    let report = f.app.update(Message::from(UpdatePcbMsg::ReportRequested));
    assert_eq!(report.units(), 1, "Report asks where to save");
}

/// New PCB then Ctrl+S writes the board and the project naming it.
#[test]
fn new_pcb_then_save_writes_both_files() {
    let mut f = fixture();
    send(&mut f.app, menu_row("new_pcb_document"));
    crate::document_setup::create_if_asked(&mut f.app);
    send(&mut f.app, menu_row("save_document"));
    let project = f.app.document_state.projects[0].path.clone();
    let board = std::fs::read_to_string(&f.board).expect("the board is written");
    assert!(board.contains("snxpcb/"), "a board file");
    let text = std::fs::read_to_string(&project).expect("the project is written");
    assert!(text.contains("demo.snxpcb"), "{text}");
    let docs = &f.app.document_state;
    assert!(!docs.is_document_dirty(&f.board));
    assert!(!docs.is_document_dirty(&project));
}

/// Ctrl+S on a new board of a project that is not the active one saves
/// that project, not only the active one.
#[test]
fn saving_a_new_board_of_another_project_saves_that_project() {
    let mut f = fixture();
    let first = f.app.document_state.projects[0].id;
    let dir = f.sheet.with_file_name("other");
    std::fs::create_dir(&dir).unwrap();
    let other = add_project(&mut f.app, &dir, None, ProjectDefaults::default());
    for tab in &mut f.app.document_state.tabs {
        tab.project_id = Some(first);
    }
    up(&mut f.app, UpdatePcbMsg::NewPcbRequested(Some(other)));
    crate::document_setup::create_if_asked(&mut f.app);
    let board = dir.join("demo.snxpcb");
    assert!(f.app.document_state.tabs.iter().any(|t| t.path == board));
    f.app.document_state.active_project = Some(first);

    send(&mut f.app, menu_row("save_document"));
    assert!(board.exists(), "the board is written");
    let project = dir.join("demo.snxprj");
    let text = std::fs::read_to_string(&project).expect("its project is written");
    assert!(text.contains("demo.snxpcb"), "{text}");
    assert!(!f.app.document_state.is_document_dirty(&project));
}
