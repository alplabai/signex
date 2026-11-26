//! #1312 — the Update PCB dialog's worker results: only the newest job
//! lands, and a headless run that cannot read ends without a dialog.

use std::path::PathBuf;

use super::{Phase, UpdatePcbMsg, UpdatePcbState};
use crate::app::{Message, Signex};

fn app_with_state(apply: bool, job: u64) -> Signex {
    let (mut app, _t) = Signex::new();
    let project = app.document_state.mint_project_id();
    let mut state = UpdatePcbState::new(
        project,
        "p.snxprj".to_string(),
        Some(PathBuf::from("/w/p.snxpcb")),
        apply,
    );
    state.job = job;
    state.phase = Phase::Computing;
    app.ui_state.update_pcb = Some(Box::new(state));
    app
}

fn computed(job: u64, why: &str) -> Message {
    Message::from(UpdatePcbMsg::Computed {
        job,
        result: Err(why.to_string()),
    })
}

#[test]
fn a_result_of_an_older_job_is_dropped() {
    let mut app = app_with_state(false, 2);
    let _ = app.update(computed(1, "old"));
    let state = app.ui_state.update_pcb.as_ref().expect("still open");
    assert!(matches!(state.phase, Phase::Computing));
    let _ = app.update(computed(2, "now"));
    let state = app.ui_state.update_pcb.as_ref().expect("still open");
    assert_eq!(state.failure(), Some("now"));
}

#[test]
fn a_headless_run_that_cannot_read_ends_without_a_dialog() {
    let mut app = app_with_state(true, 1);
    let _ = app.update(computed(1, "the root sheet could not be read"));
    assert!(app.ui_state.update_pcb.is_none());
}

pub(super) fn project(app: &mut Signex, dir: &std::path::Path) {
    use signex_types::project::ProjectData;
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(crate::app::LoadedProject {
        id,
        path: dir.join("demo.snxprj"),
        data: ProjectData {
            document_id: None,
            description: None,
            name: "demo".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("demo.snxsch".to_string()),
            pcb_file: Some("demo.snxpcb".to_string()),
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            variant_settings: Default::default(),
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            project_defaults: Default::default(),
            agent_journal: Default::default(),
            schematic_design: Default::default(),
            fab_outputs: None,
            router_tool_diameter: None,
            bom_alternates: Vec::new(),
            requirements: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    for tab in &mut app.document_state.tabs {
        tab.project_id = Some(id);
    }
}

/// The `document` operand naming the fixture's project in `dir`.
pub(super) fn document(dir: &std::path::Path) -> String {
    format!("document={}", dir.join("demo.snxprj").display())
}

/// Run the command from `operands`, waited for to its answer as a headless
/// run waits (#2470).
pub(super) fn run(app: &mut Signex, operands: &[&str]) -> crate::app::command::CommandOutcome {
    use crate::app::command::CommandArgs;
    let id = crate::keymap::AppCommandId::new("update_pcb_from_schematic").expect("id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands");
    let (outcome, task) = app.run_operands_settled(&id, &args);
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("settled");
    outcome
}

/// A project's sheet in front and its (empty) board open behind it.
pub(super) fn headless_project(dir: &std::path::Path) -> Signex {
    let board = signex_types::project::SetupProfile::Standard
        .pcb(4)
        .new_board();
    headless_project_with(dir, board, Vec::new())
}

/// [`headless_project`] with `board` and the sheet's `symbols`.
pub(super) fn headless_project_with(
    dir: &std::path::Path,
    board: signex_types::pcb::PcbBoard,
    symbols: Vec<signex_types::schematic::Symbol>,
) -> Signex {
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    };
    headless_project_with_sheet(dir, board, sheet)
}

/// [`headless_project`] with `board` and `sheet`.
pub(super) fn headless_project_with_sheet(
    dir: &std::path::Path,
    board: signex_types::pcb::PcbBoard,
    sheet: signex_types::schematic::SchematicSheet,
) -> Signex {
    let (mut app, _t) = Signex::new();
    let board_path = dir.join("demo.snxpcb");
    app.open_pcb_tab(board_path, "demo.snxpcb".to_string(), board);
    crate::app::handlers::editing_commands::net_color_driver::add_sheet_with(
        &mut app,
        &dir.join("demo.snxsch"),
        sheet,
    );
    project(&mut app, dir);
    app
}

/// The headless form needs `apply=true` (or `dry_run=true`); `apply=false`
/// is refused and opens nothing.
#[test]
fn the_headless_form_refuses_apply_false() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = headless_project(tmp.path());
    let outcome = run(&mut app, &["apply=false"]);
    assert!(
        matches!(&outcome, crate::app::command::CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("apply=true")),
        "{outcome:?}"
    );
    assert!(app.ui_state.update_pcb.is_none());
}

/// `apply=true` reads the design, executes the default ticks and leaves no
/// dialog behind; it answers with the list of what it did (#2470). With no
/// `document` it acts on the active project's board (Caner, 2026-11-25).
#[test]
fn the_headless_form_executes_without_a_dialog() {
    let tmp = tempfile::tempdir().expect("tmp");
    let mut app = headless_project(tmp.path());
    let outcome = run(&mut app, &["apply=true"]);
    let crate::app::command::CommandOutcome::Answered(answer) = &outcome else {
        panic!("answered with the change list: {outcome:?}");
    };
    assert_eq!(answer["dry_run"], false, "{answer}");
    let board = tmp.path().join("demo.snxpcb").display().to_string();
    assert_eq!(
        answer["board"], board,
        "the active project's board: {answer}"
    );
    assert!(app.ui_state.update_pcb.is_none(), "no dialog is left open");
}

#[test]
fn close_drops_the_dialog() {
    let mut app = app_with_state(false, 1);
    let _ = app.update(Message::from(UpdatePcbMsg::CloseRequested));
    assert!(app.ui_state.update_pcb.is_none());
}
