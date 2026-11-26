//! One rule for the project a `document` operand names: the board's tab
//! decides between two projects that list the same board, and every
//! command that names a project through a document (Update PCB, Back
//! Annotate, the board against the schematic, the BOM, the Rules and
//! Project tab commands) reads it from `project_named`.

use std::path::Path;

use signex_types::pcb::PcbBoard;
use signex_types::project::ProjectData;

use super::DocumentTarget;
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::state::{LoadedProject, ProjectId};
use crate::keymap::AppCommandId;

/// A project `name` in `dir` whose board is `Shared.snxpcb`.
fn project(app: &mut Signex, dir: &Path, name: &str) -> ProjectId {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join(format!("{name}.snxprj")),
        data: ProjectData {
            document_id: None,
            description: None,
            name: name.into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: Some("Shared.snxpcb".into()),
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
    id
}

fn target(path: &Path) -> DocumentTarget {
    DocumentTarget::Path(path.to_string_lossy().into_owned())
}

#[test]
fn a_board_two_projects_list_resolves_to_its_tabs_project_for_every_caller() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (mut app, _task) = Signex::new();
    let first = project(&mut app, dir.path(), "A");
    let second = project(&mut app, dir.path(), "B");
    let board = dir.path().join("Shared.snxpcb");
    let empty: PcbBoard =
        serde_json::from_value(serde_json::json!({"uuid": uuid::Uuid::from_u128(1)}))
            .expect("a board");
    app.open_pcb_tab(board.clone(), "Shared".into(), empty);
    let tab = (app.document_state.tabs.iter_mut())
        .find(|tab| tab.path == board)
        .expect("the board's tab");
    tab.project_id = Some(second);

    let named = app.project_named(&target(&board)).expect("a project");
    assert_eq!(named.id, second, "the tab's project, not the first listed");
    assert_ne!(named.id, first);

    // The Rules and Project tab commands' entry reads the same rule.
    let rules = AppCommandId::new("rules_list").expect("a catalog id");
    let operand = format!("document={}", board.display());
    let args = CommandArgs::parse_operands(&rules, &[operand.as_str()]).expect("operands");
    assert_eq!(app.rules_project_named(&args), Ok(second));
}

#[test]
fn the_project_file_names_itself_and_a_stray_file_names_none() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (mut app, _task) = Signex::new();
    let first = project(&mut app, dir.path(), "A");
    let file = dir.path().join("A.snxprj");
    assert_eq!(app.project_named(&target(&file)).map(|p| p.id), Ok(first));
    let stray = target(&dir.path().join("elsewhere").join("x.snxsch"));
    assert!(app.project_named(&stray).is_err());
}
