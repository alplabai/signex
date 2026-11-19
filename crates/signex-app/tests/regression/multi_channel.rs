//! #1279 / #1308 — multi-channel sheets and the Compiled Design panel: the
//! Repeat switch and the designator write one result, compile_project fills
//! the panel, an edit marks its sheet out of date, and a channel tab is a
//! read-only view.
//!
//! Real temp dir: `root.snxsch` places `child.snxsch` once through a sheet
//! symbol; both are open as tabs of one project.

use signex_app::app::{CompiledDesignMsg, EditMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_app::panels::compiled_design::CompiledStatus;
use signex_types::schematic::{
    ChildSheet, FillType, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol,
};
use tempfile::TempDir;
use uuid::Uuid;

/// The project and the paths and ids a test drives.
struct Fixture {
    app: Signex,
    root: std::path::PathBuf,
    child: std::path::PathBuf,
    sheet_symbol: Uuid,
    part: Uuid,
    _dir: TempDir,
}

fn sheet() -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

fn write_sheet(path: &std::path::Path, sheet: SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise sheet");
    std::fs::write(path, text).expect("write sheet");
}

fn entry(filename: &str) -> signex_types::project::SheetEntry {
    signex_types::project::SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

/// `root.snxsch` with one sheet symbol named `designator` on
/// `child.snxsch`, which holds one part R5. `active_child` focuses the
/// child's tab, else the root's.
fn fixture(designator: &str, active_child: bool) -> Fixture {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let (root_path, child_path) = (dir.join("root.snxsch"), dir.join("child.snxsch"));
    let sheet_symbol = Uuid::now_v7();
    let part = Uuid::now_v7();
    let mut root = sheet();
    root.child_sheets.push(ChildSheet {
        uuid: sheet_symbol,
        name: designator.to_string(),
        filename: "child.snxsch".to_string(),
        position: Point::new(20.0, 20.0),
        size: (40.64, 43.18),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    });
    let mut child = sheet();
    child.symbols.push(Symbol {
        uuid: part,
        lib_id: "R".to_string(),
        reference: "R5".to_string(),
        value: "31.6k".to_string(),
        position: Point::new(50.8, 50.8),
        ..Default::default()
    });
    write_sheet(&root_path, root.clone());
    write_sheet(&child_path, child.clone());
    let data = signex_types::project::ProjectData {
        document_id: None,
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("root.snxsch".to_string()),
        pcb_file: None,
        sheets: vec![entry("root.snxsch"), entry("child.snxsch")],
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        fab_outputs: None,
        requirements: Default::default(),
        parameters: Default::default(),
        schematic_design: Default::default(),
        unknown: Default::default(),
    };
    let prj = dir.join("Proj.snxprj");
    signex_types::project::write_project(&prj, &data).expect("write project");

    let (mut app, _task) = Signex::new();
    let project = app.document_state.mint_project_id();
    app.document_state
        .projects
        .push(signex_app::app::LoadedProject {
            id: project,
            path: prj,
            data,
            pending_libraries: std::collections::HashMap::new(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
    app.document_state.active_project = Some(project);
    for (title, path, sheet) in [("root", &root_path, root), ("child", &child_path, child)] {
        let engine =
            signex_engine::Engine::new_with_path(sheet, Some(path.clone())).expect("engine");
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(signex_app::app::TabInfo {
            title: title.to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: Some(project),
            kind: signex_app::app::TabKind::Schematic,
        });
    }
    let (tab, path) = if active_child {
        (1, child_path.clone())
    } else {
        (0, root_path.clone())
    };
    app.document_state.active_tab = tab;
    app.document_state.active_path = Some(path);
    Fixture {
        app,
        root: root_path,
        child: child_path,
        sheet_symbol,
        part,
        _dir: tmp,
    }
}

fn send(app: &mut Signex, msg: CompiledDesignMsg) {
    crate::settle_support::settle(app, Message::from(msg));
}

fn designator(f: &Fixture) -> String {
    f.app.document_state.engines[&f.root]
        .document()
        .child_sheets[0]
        .name
        .clone()
}

fn select(app: &mut Signex, id: Uuid, kind: SelectedKind) {
    app.interaction_state.active_canvas_mut().selected = vec![SelectedItem::new(id, kind)];
}

#[test]
fn the_repeat_switch_writes_the_repeat_and_restores_the_plain_name() {
    let mut f = fixture("buck_channel", false);
    let id = f.sheet_symbol;
    select(&mut f.app, id, SelectedKind::ChildSheet);
    send(
        &mut f.app,
        CompiledDesignMsg::RepeatToggled {
            sheet_symbol: id,
            on: true,
        },
    );
    assert_eq!(designator(&f), "Repeat(CH, 1, 4)");
    send(
        &mut f.app,
        CompiledDesignMsg::RepeatToggled {
            sheet_symbol: id,
            on: false,
        },
    );
    assert_eq!(designator(&f), "buck_channel");
}

#[test]
fn typing_a_repeat_as_the_designator_turns_the_switch_on() {
    let mut f = fixture("buck_channel", false);
    let id = f.sheet_symbol;
    select(&mut f.app, id, SelectedKind::ChildSheet);
    for msg in [
        PanelMsg::ChildSheetNameTyping(id, "Repeat(CH, 1, 6)".to_string()),
        PanelMsg::CommitChildSheetName(id),
    ] {
        crate::settle_support::settle(&mut f.app, Message::Dock(DockMessage::Panel(msg)));
    }
    assert_eq!(designator(&f), "Repeat(CH, 1, 6)");
    let view = f
        .app
        .document_state
        .panel_ctx
        .compiled_design
        .sheet_symbol
        .clone()
        .expect("the sheet symbol's rows");
    assert_eq!(view.repeat.map(|r| r.count()), Some(6));
}

#[test]
fn compile_project_fills_the_panel_with_the_channels() {
    let mut f = fixture("Repeat(CH, 1, 2)", false);
    assert_eq!(
        f.app.document_state.panel_ctx.compiled_design.panel.status,
        CompiledStatus::NotCompiled
    );
    send(&mut f.app, CompiledDesignMsg::CompileRequested);
    let panel = &f.app.document_state.panel_ctx.compiled_design.panel;
    assert_eq!(panel.status, CompiledStatus::Ready);
    let names: Vec<&str> = panel.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["root", "child", "CH1", "CH2"]);
    assert_eq!(panel.totals.parts, 2, "CH1_R5 and CH2_R5");
    assert_eq!(panel.totals.channels, 2);
}

#[test]
fn an_edit_after_a_compile_marks_its_sheet_out_of_date() {
    let mut f = fixture("Repeat(CH, 1, 2)", true);
    send(&mut f.app, CompiledDesignMsg::CompileRequested);
    assert_eq!(
        f.app
            .document_state
            .panel_ctx
            .compiled_design
            .panel
            .stale_count,
        0
    );
    let part = f.part;
    select(&mut f.app, part, SelectedKind::Symbol);
    crate::settle_support::settle(&mut f.app, Message::Edit(EditMsg::DeleteSelected));
    let panel = &f.app.document_state.panel_ctx.compiled_design.panel;
    assert_eq!(panel.stale_count, 1);
    assert!(
        panel.rows.iter().any(|r| r.name == "CH1" && r.is_stale),
        "the channel rows of the edited sheet are amber"
    );
}

#[test]
fn a_channel_tab_refuses_an_edit_with_a_message() {
    let mut f = fixture("Repeat(CH, 1, 2)", true);
    let child = f.child.clone();
    send(
        &mut f.app,
        CompiledDesignMsg::ChannelPicked {
            sheet: child.clone(),
            channel: Some(1),
        },
    );
    let strip = f
        .app
        .document_state
        .panel_ctx
        .compiled_design
        .strip
        .clone()
        .expect("the child is repeated, so it has a channel strip");
    assert_eq!(strip.active, Some(1));
    assert_eq!(strip.tabs.len(), 2);
    let part = f.part;
    select(&mut f.app, part, SelectedKind::Symbol);
    crate::settle_support::settle(&mut f.app, Message::Edit(EditMsg::DeleteSelected));
    let still_there = f.app.document_state.engines[&child]
        .document()
        .symbols
        .iter()
        .any(|s| s.uuid == part);
    assert!(still_there, "a channel tab never edits the sheet");
    let said = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.message
            .contains("Channel CH1 is a compiled view; switch to Editor to edit")
    });
    assert!(said, "the refusal is said in Messages");

    // Editor edits again.
    send(
        &mut f.app,
        CompiledDesignMsg::ChannelPicked {
            sheet: child.clone(),
            channel: None,
        },
    );
    select(&mut f.app, part, SelectedKind::Symbol);
    crate::settle_support::settle(&mut f.app, Message::Edit(EditMsg::DeleteSelected));
    let gone = !f.app.document_state.engines[&child]
        .document()
        .symbols
        .iter()
        .any(|s| s.uuid == part);
    assert!(gone, "Editor is the editable sheet");
}
