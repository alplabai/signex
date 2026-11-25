//! Mockup row 80, decided 2026-11-24 (docs/DECISIONS.md): Tools ▸ Assign
//! Footprints starts on the All chip and lists every part; a chip narrows it.
//!
//! Real temp dir, project `P`: `a.snxsch` (open, active) and `b.snxsch`
//! (open) each hold a Stock resistor; `c.snxsch` (closed) holds a third.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{SchematicSheet, Symbol};

use super::{AssignFilter, PartAccess};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::path_ops::settle;
use crate::app::{
    AssignFootprintsMsg, LoadedProject, Message, ProjectId, Signex, TabInfo, TabKind,
};
use crate::keymap::AppCommandId;

fn write_sheet(path: &Path, sheet: &SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet.clone())
        .write_string()
        .expect("serialise sheet");
    std::fs::write(path, text).expect("write sheet");
}

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

/// `part` again under another id and designator.
fn copy_of(part: &Symbol, reference: &str) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        reference: reference.to_string(),
        ..part.clone()
    }
}

fn project_data(dir: &Path) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "P".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("a.snxsch".to_string()),
        pcb_file: None,
        sheets: ["a.snxsch", "b.snxsch", "c.snxsch"]
            .iter()
            .map(|f| SheetEntry {
                name: (*f).to_string(),
                filename: (*f).to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            })
            .collect(),
        variant_definitions: Vec::new(),
        active_variant: None,
        variant_settings: Default::default(),
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        parameters: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        bom_alternates: Vec::new(),
        requirements: Default::default(),
        unknown: Default::default(),
    }
}

fn open_tab(app: &mut Signex, path: &Path, sheet: SchematicSheet, project: ProjectId) {
    let engine =
        signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf())).expect("engine");
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: path.file_stem().unwrap().to_string_lossy().into_owned(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: Some(project),
        kind: TabKind::Schematic,
    });
}

/// The Stock resistor placed on `a` through the `place_component` command,
/// which stamps it with its library row as the Components panel place does.
fn place_stock_resistor(app: &mut Signex, stock: &Path) {
    let id = AppCommandId::new("place_component").expect("a command id");
    let library = format!("library={}", stock.display());
    let call = CommandArgs::parse_operands(&id, &[&library, "part=STOCK-R", "at=254,254"])
        .expect("operands");
    let (outcome, _) = app.run_operands_settled(&id, &call);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "placed: {outcome:?}"
    );
}

/// The app with the dialog's three parts on disk and in memory, and the
/// temp dir that holds them.
fn rig() -> (Signex, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let (a, b, c) = (
        dir.join("a.snxsch"),
        dir.join("b.snxsch"),
        dir.join("c.snxsch"),
    );
    for path in [&a, &b, &c] {
        write_sheet(path, &empty_sheet());
    }
    let data = project_data(&dir);
    let prj = dir.join("P.snxprj");
    signex_types::project::write_project(&prj, &data).expect("write project");

    let (mut app, _task) = Signex::new();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let stock =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib");
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount stock");
    let project = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id: project,
        path: prj,
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(project);
    open_tab(&mut app, &a, empty_sheet(), project);
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(a.clone());

    place_stock_resistor(&mut app, &stock);
    let placed = app.document_state.engines[&a].document().symbols[0].clone();
    let mut on_b = empty_sheet();
    on_b.symbols.push(copy_of(&placed, "R2"));
    open_tab(&mut app, &b, on_b, project);
    let mut on_c = empty_sheet();
    on_c.symbols.push(copy_of(&placed, "R3"));
    write_sheet(&c, &on_c);
    (app, tmp)
}

/// The dialog has read the three parts: two on open sheets, one closed.
fn assert_dialog_read(app: &Signex) {
    let state = app
        .ui_state
        .assign_footprints
        .as_ref()
        .expect("dialog open");
    let parts = state.parts();
    assert_eq!(parts.len(), 3, "{parts:?}");
    let closed: Vec<&str> = parts
        .iter()
        .filter(|p| p.access == PartAccess::SheetClosed)
        .map(|p| p.designator.as_str())
        .collect();
    assert_eq!(closed, ["R3"], "c.snxsch is listed only");
    for part in parts.iter().filter(|p| p.access == PartAccess::Editable) {
        assert_eq!(part.offers.len(), 7, "the row's default and six alternates");
        assert_eq!(part.row_label.as_deref(), Some("STOCK-R"));
    }
}

#[test]
fn assign_footprints_starts_on_all_and_lists_every_part() {
    let (mut app, _tmp) = rig();
    let menu_row = Message::Menu(crate::menu_bar::MenuMessage::Command(
        AppCommandId::new("assign_footprints").expect("a command id"),
    ));
    settle(&mut app, menu_row);
    assert_dialog_read(&app);
    let state = app.ui_state.assign_footprints.as_ref().expect("open");
    let every: Vec<usize> = (0..state.parts().len()).collect();
    assert_eq!(state.visible_rows(), every, "All lists every part");

    let to = AssignFilter::Changed;
    settle(
        &mut app,
        Message::from(AssignFootprintsMsg::FilterPicked(to)),
    );
    let state = app.ui_state.assign_footprints.as_ref().expect("open");
    assert!(
        state.visible_rows().len() < every.len(),
        "a chip narrows it"
    );
}
