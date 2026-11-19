//! #2530 — the Rules constraint grid end to end through `update`: a value
//! typed in a cell of the All row becomes one rule of the project's rule
//! text, the other rules stay as typed, and one Undo takes it back.

use signex_app::app::{LoadedProject, Message, ProjectMsg, RulesTabMsg, Signex};
use signex_drc_dsl::table::catalog::Domain;
use signex_types::project::{DrcRuleSet, ProjectData, ProjectRules};
use signex_widgets::constraint_grid::{CellRef, ColumnId, GridMessage, RowId};

const TEXT: &str =
    "rule sig on clearance when net == \"SIG\" require clearance >= 0.3mm then error\n";

fn project(app: &mut Signex, dir: &std::path::Path) -> signex_app::app::ProjectId {
    let data = ProjectData {
        document_id: None,
        name: "grid".to_string(),
        description: None,
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: None,
        pcb_file: None,
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: Some(ProjectRules {
            drc: DrcRuleSet {
                dsl: Some(TEXT.to_string()),
                ..Default::default()
            },
            ..ProjectRules::default()
        }),
        annotation: None,
        parameters: Default::default(),
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        fab_outputs: None,
        requirements: Default::default(),
        unknown: Default::default(),
    };
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("grid.snxprj"),
        data,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    id
}

fn send(app: &mut Signex, msg: RulesTabMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Rules(msg)));
}

fn dsl(app: &Signex) -> String {
    app.document_state.projects[0]
        .data
        .rules
        .as_ref()
        .and_then(|r| r.drc.dsl.clone())
        .unwrap_or_default()
}

#[test]
fn a_grid_cell_typed_in_writes_one_rule_and_one_undo_takes_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _task) = Signex::new();
    let id = project(&mut app, dir.path());
    send(&mut app, RulesTabMsg::OpenRequested { project: Some(id) });
    send(
        &mut app,
        RulesTabMsg::DomainPicked {
            project_id: id,
            domain: Domain::Clearance,
        },
    );
    // The All row is the first row; the Clearance group's Min the first
    // column of the Clearance domain.
    let cell = CellRef::new(RowId(0), ColumnId(0));
    for msg in [
        GridMessage::CellPressed(cell),
        GridMessage::EditTextChanged("0.25".to_string()),
        GridMessage::EditSubmitted,
    ] {
        send(
            &mut app,
            RulesTabMsg::GridActed {
                project_id: id,
                msg,
            },
        );
    }
    let text = dsl(&app);
    assert!(
        text.starts_with(TEXT),
        "the written rule stays as typed: {text}"
    );
    assert!(
        text.contains("on clearance require clearance >= 0.25mm"),
        "the All row's value is a board-wide rule: {text}"
    );
    let _ = app.update(crate::menu_support::menu_row("undo"));
    assert_eq!(dsl(&app), TEXT, "one step: the rule text is back");
}
