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
    ChildSheet, FillType, Point, SchDrawing, SchTable, SchematicSheet, SelectedItem, SelectedKind,
    Symbol, TableSource,
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
    let (prj, data) = write_project_file(&dir, "Proj", &["root.snxsch", "child.snxsch"]);

    let (mut app, _task) = Signex::new();
    let project = load_project(&mut app, prj, data);
    app.document_state.active_project = Some(project);
    for (title, path, sheet) in [("root", &root_path, root), ("child", &child_path, child)] {
        open_tab(&mut app, project, title, path, sheet);
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

/// `<name>.snxprj` in `dir` over `sheets`, the first being the root.
fn write_project_file(
    dir: &std::path::Path,
    name: &str,
    sheets: &[&str],
) -> (std::path::PathBuf, signex_types::project::ProjectData) {
    let data = signex_types::project::ProjectData {
        document_id: None,
        description: None,
        name: name.to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: sheets.first().map(|s| s.to_string()),
        pcb_file: None,
        sheets: sheets.iter().map(|s| entry(s)).collect(),
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
        agent_journal: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: Default::default(),
        parameters: Default::default(),
        schematic_design: Default::default(),
        unknown: Default::default(),
    };
    let prj = dir.join(format!("{name}.snxprj"));
    signex_types::project::write_project(&prj, &data).expect("write project");
    (prj, data)
}

fn load_project(
    app: &mut Signex,
    path: std::path::PathBuf,
    data: signex_types::project::ProjectData,
) -> signex_app::app::ProjectId {
    let project = app.document_state.mint_project_id();
    app.document_state
        .projects
        .push(signex_app::app::LoadedProject {
            id: project,
            path,
            data,
            pending_libraries: std::collections::HashMap::new(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
    project
}

fn open_tab(
    app: &mut Signex,
    project: signex_app::app::ProjectId,
    title: &str,
    path: &std::path::Path,
    sheet: SchematicSheet,
) {
    let engine =
        signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf())).expect("engine");
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: title.to_string(),
        path: path.to_path_buf(),
        project_id: Some(project),
        kind: signex_app::app::TabKind::Schematic,
    });
}

/// A sheet holding one empty contents table (bound to the project's pages).
fn sheet_with_contents_table() -> SchematicSheet {
    let mut table = SchTable::new_typed(Uuid::now_v7(), Point::new(25.4, 35.56));
    table.source = TableSource::ProjectSheets { per_channel: true };
    table.columns = SchTable::columns_for(&table.source);
    table.rows = Vec::new();
    let mut sheet = sheet();
    sheet.drawings.push(SchDrawing::Table(table));
    sheet
}

fn contents_rows(app: &Signex, path: &std::path::Path) -> Vec<Vec<String>> {
    app.document_state.engines[path]
        .document()
        .drawings
        .iter()
        .find_map(|d| match d {
            SchDrawing::Table(t) => Some(t.rows.clone()),
            _ => None,
        })
        .expect("a contents table")
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
    // The fixture writes the state directly; one message refreshes the panel.
    send(&mut f.app, CompiledDesignMsg::FindChanged(String::new()));
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

/// #2741, mockup row 154: a sheet instance lists its parts, and a click on
/// one lands on the part on its sheet.
#[test]
fn a_component_row_opens_its_sheet_and_selects_the_part() {
    let mut f = fixture("child_sheet", false);
    send(&mut f.app, CompiledDesignMsg::CompileRequested);
    let row = |app: &Signex| {
        app.document_state
            .panel_ctx
            .compiled_design
            .panel
            .rows
            .iter()
            .find(|r| r.name == "child_sheet")
            .cloned()
            .expect("the child's row")
    };
    let folded = row(&f.app);
    assert!(folded.has_children && !folded.is_expanded);
    assert!(folded.part_rows.is_empty(), "folded by default");

    send(&mut f.app, CompiledDesignMsg::RowExpanded(folded.index));
    let open = row(&f.app);
    let labels: Vec<&str> = open.part_rows.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["R5"]);

    let target = open.part_rows[0].target.clone();
    send(&mut f.app, CompiledDesignMsg::TargetClicked(target));

    assert_eq!(
        f.app.document_state.active_path.as_deref(),
        Some(f.child.as_path())
    );
    let selected = &f.app.interaction_state.active_canvas().selected;
    assert!(
        selected
            .iter()
            .any(|s| s.uuid == f.part && s.kind == SelectedKind::Symbol),
        "{selected:?}"
    );
}

/// Mockup row 154: Recompile is the panel's title-bar icon.
#[test]
fn the_title_bar_recompile_compiles_the_project() {
    let mut f = fixture("Repeat(CH, 1, 2)", false);
    send(&mut f.app, CompiledDesignMsg::FindChanged(String::new()));
    let press = signex_app::panels::title_actions(
        signex_app::panels::PanelKind::CompiledDesign,
        &f.app.document_state.panel_ctx,
    )
    .into_iter()
    .next()
    .expect("the panel has Recompile in its title bar")
    .on_press
    .expect("a project is open");
    crate::settle_support::settle(&mut f.app, Message::Dock(DockMessage::Panel(press)));
    assert_eq!(
        f.app.document_state.panel_ctx.compiled_design.panel.status,
        CompiledStatus::Ready
    );
}

/// The compile of one project refreshes that project's contents tables and
/// never another open project's (#1300 review).
#[test]
fn a_compile_refreshes_only_its_own_projects_contents_tables() {
    let mut f = fixture("Repeat(CH, 1, 2)", false);
    // Project A's root gets a contents table.
    let mut root = f.app.document_state.engines[&f.root].document().clone();
    root.drawings.extend(sheet_with_contents_table().drawings);
    let engine = signex_engine::Engine::new_with_path(root, Some(f.root.clone())).expect("engine");
    f.app.document_state.engines.insert(f.root.clone(), engine);
    // Project B, open alongside, with its own contents sheet.
    let other = TempDir::new().expect("tempdir");
    let b_path = other.path().join("contents.snxsch");
    write_sheet(&b_path, sheet_with_contents_table());
    let (b_prj, b_data) = write_project_file(other.path(), "Other", &["contents.snxsch"]);
    let b = load_project(&mut f.app, b_prj, b_data);
    open_tab(
        &mut f.app,
        b,
        "contents",
        &b_path,
        sheet_with_contents_table(),
    );

    send(&mut f.app, CompiledDesignMsg::CompileRequested);

    assert!(
        !contents_rows(&f.app, &f.root).is_empty(),
        "the compiled project's contents table lists its pages"
    );
    assert!(
        contents_rows(&f.app, &b_path).is_empty(),
        "another project's contents table is left alone"
    );
    assert!(
        !f.app.document_state.engines[&b_path].can_undo(),
        "and gets no undo step"
    );
    // The refresh is an edit through the gateway (one undo step), but it is
    // the compile's own output, so the compile is not out of date.
    assert!(f.app.document_state.engines[&f.root].can_undo());
    assert!(
        f.app
            .document_state
            .compiled_design
            .results
            .values()
            .all(|result| result.stale.is_empty()),
        "the table refresh marks no sheet out of date"
    );
}

/// `root.snxsch` holding a contents table and a sheet symbol on
/// `power/buck.snxsch`, titled Buck, rev A. Both are open, the root in front.
struct ContentsProject {
    app: Signex,
    root: std::path::PathBuf,
    buck: std::path::PathBuf,
    _dir: TempDir,
}

fn contents_project() -> ContentsProject {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    std::fs::create_dir(dir.join("power")).expect("subfolder");
    let (root_path, buck_path) = (
        dir.join("root.snxsch"),
        dir.join("power").join("buck.snxsch"),
    );
    let mut root = sheet_with_contents_table();
    root.child_sheets.push(ChildSheet {
        uuid: Uuid::now_v7(),
        name: "U_BUCK".to_string(),
        filename: "power/buck.snxsch".to_string(),
        // Clear of the table, so a double-click on a row hits the row.
        position: Point::new(200.0, 200.0),
        size: (40.64, 43.18),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    });
    let mut buck = sheet();
    buck.title_block.insert("title".into(), "Buck".into());
    buck.title_block.insert("revision".into(), "A".into());
    write_sheet(&root_path, root.clone());
    write_sheet(&buck_path, buck.clone());
    let (prj, data) = write_project_file(&dir, "Proj", &["root.snxsch", "power/buck.snxsch"]);
    let (mut app, _task) = Signex::new();
    let project = load_project(&mut app, prj, data);
    app.document_state.active_project = Some(project);
    open_tab(&mut app, project, "root", &root_path, root);
    open_tab(&mut app, project, "buck", &buck_path, buck);
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root_path.clone());
    ContentsProject {
        app,
        root: root_path,
        buck: buck_path,
        _dir: tmp,
    }
}

fn contents_table(app: &Signex, path: &std::path::Path) -> SchTable {
    app.document_state.engines[path]
        .document()
        .drawings
        .iter()
        .find_map(|d| match d {
            SchDrawing::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("a contents table")
}

/// A point on body row `row` of `table`.
fn point_on_row(table: &SchTable, row: usize) -> Point {
    (0..2000_i32)
        .map(|step| {
            Point::new(
                table.position.x + 0.5,
                table.position.y + f64::from(step) * 0.05,
            )
        })
        .find(|at| table.cell_at(*at).and_then(|cell| cell.row) == Some(row))
        .expect("the row is on the sheet")
}

/// #2742: the link is the real path, and a double-click opens that file.
#[test]
fn a_contents_row_opens_its_sheet_in_a_subfolder() {
    let mut f = contents_project();
    send(&mut f.app, CompiledDesignMsg::CompileRequested);
    let table = contents_table(&f.app, &f.root);
    assert_eq!(table.links, ["root.snxsch", "power/buck.snxsch"]);

    let buck = f.buck.clone();
    f.app.document_state.engines.remove(&buck);
    f.app.document_state.tabs.retain(|tab| tab.path != buck);
    let at = point_on_row(&table, 1);
    crate::settle_support::settle(
        &mut f.app,
        Message::CanvasEvent(signex_app::canvas::CanvasEvent::DoubleClicked {
            world_x: at.x,
            world_y: at.y,
            screen_x: 0.0,
            screen_y: 0.0,
        }),
    );
    assert_eq!(f.app.document_state.active_path.as_ref(), Some(&buck));
}

/// #2742: closing a sheet leaves its Title and Rev, and the next compile
/// pushes no undo step.
#[test]
fn a_closed_sheet_keeps_its_title_and_rev_with_no_undo_step() {
    let mut f = contents_project();
    send(&mut f.app, CompiledDesignMsg::CompileRequested);
    let rows = contents_rows(&f.app, &f.root);
    assert_eq!((rows[1][2].as_str(), rows[1][3].as_str()), ("Buck", "A"));
    let revision = f.app.document_state.engines[&f.root].revision();

    let buck = f.buck.clone();
    f.app.document_state.engines.remove(&buck);
    f.app.document_state.tabs.retain(|tab| tab.path != buck);
    send(&mut f.app, CompiledDesignMsg::CompileRequested);

    let rows = contents_rows(&f.app, &f.root);
    assert_eq!((rows[1][2].as_str(), rows[1][3].as_str()), ("Buck", "A"));
    assert_eq!(
        f.app.document_state.engines[&f.root].revision(),
        revision,
        "the second refresh changed nothing"
    );
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
    // Messages reads the diagnostics ring, which fills once the logger is in.
    let _ = signex_app::diagnostics::init_logging();
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
            .replace(['\u{2068}', '\u{2069}'], "")
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
