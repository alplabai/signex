//! Mockup row 67, decided 2026-11-24 (docs/DECISIONS.md): Replace All runs
//! with no confirmation, also across sheets, and one undo reverts it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{HAlign, Label, LabelType, Point, SchematicSheet, VAlign, Wire};
use uuid::Uuid;

use crate::app::path_ops::settle;
use crate::app::{CaseFold, EditMsg, FileMsg, LoadedProject, Message, Signex};
use crate::find_replace::{FindReplaceMsg, FindScope};

/// A sheet of one wire with the global label `VCC` on it.
fn vcc_sheet(label: u128, wire: u128) -> SchematicSheet {
    let mut s = SchematicSheet {
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    s.wires.push(Wire {
        uuid: Uuid::from_u128(wire),
        start: Point::new(0.0, 0.0),
        end: Point::new(5.0, 0.0),
        ..Default::default()
    });
    s.labels.push(Label {
        uuid: Uuid::from_u128(label),
        justify: HAlign::default(),
        justify_v: VAlign::default(),
        ..Label::new("VCC", Point::new(0.0, 0.0), LabelType::Global)
    });
    s
}

fn project_data(dir: &Path) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "P".into(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("a.snxsch".into()),
        pcb_file: None,
        sheets: ["a.snxsch", "b.snxsch"]
            .iter()
            .map(|f| SheetEntry {
                name: (*f).into(),
                filename: (*f).into(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            })
            .collect(),
        variant_definitions: Vec::new(),
        active_variant: None,
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
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// Two sheets of one project, each a wire with the global label `VCC`,
/// both open as tabs with `a` focused.
fn open_project(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    let write = |name: &str, s: SchematicSheet| {
        let path = dir.join(name);
        let text = signex_types::format::SnxSchematic::new(s)
            .write_string()
            .expect("encode");
        std::fs::write(&path, text).expect("write");
        path
    };
    let a = write("a.snxsch", vcc_sheet(1, 11));
    let b = write("b.snxsch", vcc_sheet(2, 12));

    let (mut app, _boot) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: project_data(dir),
        pending_libraries: HashMap::new(),
        case_fold: CaseFold::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    for path in [&b, &a] {
        let text = std::fs::read_to_string(path).expect("read");
        let sheet = signex_types::format::SnxSchematic::parse(&text)
            .expect("parse")
            .sheet;
        let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
            path: path.clone(),
            title: "t".into(),
            result: Ok(Box::new(sheet)),
            disk: None,
        }));
    }
    assert_eq!(app.document_state.active_path.as_deref(), Some(a.as_path()));
    (app, a, b)
}

fn first_label(app: &Signex, path: &Path) -> String {
    app.document_state.engines[path].document().labels[0]
        .text
        .clone()
}

fn find(app: &mut Signex, msg: FindReplaceMsg) {
    settle(app, Message::FindReplaceMsg(msg));
}

#[test]
fn project_replace_all_changes_every_sheet_with_no_prompt_and_one_undo() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let (mut app, a, b) = open_project(tmp.path());
    find(&mut app, FindReplaceMsg::ScopeChanged(FindScope::Project));
    find(&mut app, FindReplaceMsg::QueryChanged("VCC".into()));
    find(&mut app, FindReplaceMsg::ReplacementChanged("VDD".into()));
    assert_eq!(app.ui_state.find_replace.matches.len(), 2, "both sheets");

    find(&mut app, FindReplaceMsg::ReplaceAll);

    assert!(
        !app.ui_state.find_replace.is_busy(),
        "nothing waits on a confirm"
    );
    assert_eq!(first_label(&app, &a), "VDD");
    assert_eq!(first_label(&app, &b), "VDD");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(first_label(&app, &a), "VCC", "one undo reverts it");
    assert_eq!(first_label(&app, &b), "VCC", "on every sheet");
}
