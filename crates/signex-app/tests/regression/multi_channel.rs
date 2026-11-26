//! #1279 / #1308 — multi-channel sheets and the Compiled Design panel: the
//! Repeat switch and the designator write one result, compile_project fills
//! the panel, an edit marks its sheet out of date, and a channel tab is a
//! read-only view.
//!
//! Real temp dir: `root.snxsch` places `child.snxsch` once through a sheet
//! symbol; both are open as tabs of one project.

use signex_app::app::{CompiledDesignMsg, EditMsg, InputMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::compiled_design::{CompiledStatus, NamingTarget};
use signex_app::panels::{PanelKind, PanelMsg};
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
        bom_alternates: Vec::new(),
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

/// The child's channel strip as the panels last saw it.
fn strip(app: &Signex) -> signex_app::panels::compiled_design::ChannelStripView {
    app.document_state
        .panel_ctx
        .compiled_design
        .strip
        .clone()
        .expect("the child is repeated, so it has a channel strip")
}

/// #2658: a click on a channel row opens the child on that channel's tab;
/// a Ctrl+click opens the source sheet itself, on Editor.
#[test]
fn a_ctrl_click_on_a_channel_row_opens_the_source_sheet_on_editor() {
    let mut f = fixture("Repeat(CH, 1, 2)", false);
    send(&mut f.app, CompiledDesignMsg::CompileRequested);
    let ch2 = f
        .app
        .document_state
        .panel_ctx
        .compiled_design
        .panel
        .rows
        .iter()
        .find(|r| r.name == "CH2")
        .map(|r| r.index)
        .expect("the CH2 row");

    send(&mut f.app, CompiledDesignMsg::RowClicked(ch2));
    assert_eq!(
        f.app.document_state.active_path.as_deref(),
        Some(f.child.as_path())
    );
    assert_eq!(strip(&f.app).active, Some(2), "a click: the channel's tab");

    send(&mut f.app, CompiledDesignMsg::RowSourceClicked(ch2));
    assert_eq!(
        f.app.document_state.active_path.as_deref(),
        Some(f.child.as_path())
    );
    assert_eq!(strip(&f.app).active, None, "a Ctrl+click: Editor");
}

/// #2658, mockup row 151: after a compile each channel card has its
/// drawing, with the part named as that channel names it.
#[test]
fn a_compile_draws_each_channel_card_with_its_designators() {
    let mut f = fixture("Repeat(CH, 1, 2)", true);
    send(&mut f.app, CompiledDesignMsg::FindChanged(String::new()));
    assert!(
        strip(&f.app).tabs.iter().all(|t| t.miniature.is_none()),
        "no drawing before a compile"
    );
    send(&mut f.app, CompiledDesignMsg::CompileRequested);
    let tabs = strip(&f.app).tabs;
    assert_eq!(tabs.len(), 2);
    for tab in &tabs {
        let drawing = tab.miniature.as_ref().expect("a drawing after the compile");
        let names: Vec<&str> = drawing
            .designators
            .iter()
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(names, [format!("{}_R5", tab.channel.name)]);
    }
}

fn designator_format(f: &Fixture) -> String {
    f.app.document_state.projects[0]
        .data
        .schematic_design
        .channel_naming
        .designator_format
        .clone()
}

fn naming_menu_open(f: &Fixture) -> bool {
    f.app.ui_state.compiled_design.naming_menu.is_some()
}

/// A repeated sheet symbol selected with Properties in front, so its
/// Channel naming section shows.
fn naming_fixture() -> Fixture {
    let mut f = fixture("Repeat(CH, 1, 4)", false);
    // The first-run tour claims Esc ahead of a panel menu.
    f.app.ui_state.first_run_tour_open = false;
    let id = f.sheet_symbol;
    select(&mut f.app, id, SelectedKind::ChildSheet);
    f.app.document_state.dock.show_panel(PanelKind::Properties);
    f
}

/// #2674: a style picked in the naming menu is written and closes it.
#[test]
fn a_style_picked_in_the_naming_menu_is_the_channel_naming() {
    let mut f = naming_fixture();
    // Not the default `${Channel}_${Designator}`, so the pick is a change.
    let picked = "${Designator}_${Channel}";
    assert_ne!(designator_format(&f), picked, "the pick is a change");

    send(
        &mut f.app,
        CompiledDesignMsg::NamingMenuToggled(NamingTarget::Designators),
    );
    assert!(naming_menu_open(&f));
    send(
        &mut f.app,
        CompiledDesignMsg::NamingPicked {
            target: NamingTarget::Designators,
            format: picked.to_string(),
        },
    );

    assert_eq!(designator_format(&f), picked);
    assert!(!naming_menu_open(&f), "a pick closes the menu");
}

/// #2674: Esc closes the naming menu and leaves the naming as it was, a
/// custom format typed but not submitted included.
#[test]
fn esc_closes_the_naming_menu_without_a_change() {
    let mut f = naming_fixture();
    let before = designator_format(&f);
    send(
        &mut f.app,
        CompiledDesignMsg::NamingMenuToggled(NamingTarget::Designators),
    );
    send(
        &mut f.app,
        CompiledDesignMsg::NamingCustomTyped("${Designator}-${Index}".to_string()),
    );
    assert!(naming_menu_open(&f));

    crate::settle_support::settle(
        &mut f.app,
        Message::Input(InputMsg::EscapePressed { window: None }),
    );

    assert!(!naming_menu_open(&f), "Esc closes the menu");
    assert_eq!(designator_format(&f), before, "Esc changes nothing");
    let selected = &f.app.interaction_state.active_canvas().selected;
    assert_eq!(selected.len(), 1, "Esc goes no further than the menu");
}

/// The designators of the BOM preview's lines, Lite active.
fn bom_designators(app: &mut Signex) -> Vec<String> {
    let _ = app.update(Message::Menu(signex_app::menu_bar::MenuMessage::ExportBom));
    let preview = app.document_state.bom_preview.as_ref().expect("preview");
    preview
        .table
        .rows
        .iter()
        .flat_map(|row| row.references.iter().cloned())
        .collect()
}

/// The Variant Manager's read of the project's parts, with the settings
/// applied as the matrix shows them: `designator` to its fit in Lite.
fn variant_manager_lite(app: &mut Signex) -> Vec<(String, bool)> {
    let project_id = app.document_state.projects[0].id;
    app.document_state
        .project_tabs
        .entry(project_id)
        .or_default();
    crate::settle_support::settle(
        app,
        Message::Project(signex_app::app::ProjectMsg::Tab(
            signex_app::app::ProjectTabMsg::VariantPartsReadRequested { project_id },
        )),
    );
    let tab = &app.document_state.project_tabs[&project_id];
    let read = tab.variant_parts.as_deref().expect("a read landed");
    let matrix = read.as_ref().expect("the parts were read");
    let settings = &app.document_state.projects[0].data.variant_settings;
    let mut rows: Vec<(String, bool)> = matrix
        .with_settings(settings)
        .rows
        .iter()
        .map(|row| (row.designator.clone(), row.fitted[1]))
        .collect();
    rows.sort();
    rows
}

/// #2662, mockup row 151: a card's fit click, from the parts the compiled
/// strip lists, is the fit the BOM and the Variant Manager read too: the
/// channel's copy is left out of the Lite BOM and shows Not fitted in the
/// matrix, the other channel's copy stays fitted.
#[test]
fn a_channel_card_fit_reaches_the_bom_and_the_variant_manager() {
    let mut f = fixture("Repeat(CH, 1, 2)", true);
    let data = &mut f.app.document_state.projects[0].data;
    data.variant_definitions = vec!["Lite".to_string()];
    data.active_variant = Some("Lite".to_string());
    send(&mut f.app, CompiledDesignMsg::CompileRequested);
    let ch1 = strip(&f.app).tabs[0].clone();
    assert_eq!(ch1.channel.name, "CH1");
    assert!(!ch1.fits.is_empty(), "the compile lists CH1's parts");
    assert!(!ch1.is_not_fitted());
    let mut before = bom_designators(&mut f.app);
    before.sort();
    assert_eq!(before, ["CH1_R5", "CH2_R5"]);

    send(
        &mut f.app,
        CompiledDesignMsg::ChannelFitPicked {
            parts: ch1.fits.iter().map(|fit| fit.part.clone()).collect(),
            fitted: Some(false),
        },
    );

    assert!(strip(&f.app).tabs[0].is_not_fitted(), "the card reads it");
    assert!(!strip(&f.app).tabs[1].is_not_fitted());
    assert_eq!(bom_designators(&mut f.app), ["CH2_R5"], "the Lite BOM");
    assert_eq!(
        variant_manager_lite(&mut f.app),
        [("CH1_R5".to_string(), false), ("CH2_R5".to_string(), true)],
        "the Variant Manager's channel rows"
    );
}

/// #2662, mockup rows 151 and 154: once a card leaves CH1 out of Lite, the
/// Compiled Design tags CH1's row "Not fitted (Lite)" and CH2's not, and
/// CH1's tab Properties name CH1 as left out.
#[test]
fn a_channel_left_out_is_tagged_in_the_panel_and_named_in_its_properties() {
    use signex_app::panels::compiled_design::CompiledRowKind;

    let mut f = fixture("Repeat(CH, 1, 2)", true);
    let data = &mut f.app.document_state.projects[0].data;
    data.variant_definitions = vec!["Lite".to_string()];
    data.active_variant = Some("Lite".to_string());
    send(&mut f.app, CompiledDesignMsg::CompileRequested);
    let tags = |app: &Signex| -> Vec<(String, Option<String>)> {
        app.document_state
            .panel_ctx
            .compiled_design
            .panel
            .rows
            .iter()
            .filter(|row| row.kind == CompiledRowKind::Channel)
            .map(|row| (row.name.clone(), row.not_fitted_in.clone()))
            .collect()
    };
    assert!(tags(&f.app).iter().all(|(_, tag)| tag.is_none()));

    let ch1 = strip(&f.app).tabs[0].clone();
    let parts = ch1.fits.iter().map(|fit| fit.part.clone()).collect();
    send(
        &mut f.app,
        CompiledDesignMsg::ChannelFitPicked {
            parts,
            fitted: Some(false),
        },
    );
    assert_eq!(
        tags(&f.app),
        [
            ("CH1".to_string(), Some("Lite".to_string())),
            ("CH2".to_string(), None),
        ]
    );

    send(
        &mut f.app,
        CompiledDesignMsg::ChannelPicked {
            sheet: f.child.clone(),
            channel: Some(1),
        },
    );
    let summary = strip(&f.app).variant_summary().expect("CH1 is shown");
    assert_eq!(summary.left_out, ["CH1"]);
    assert_eq!(summary.not_fitted, summary.parts);
}
