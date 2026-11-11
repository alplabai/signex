//! #1312 PR 2 — Back Annotate driven through UI messages on a project read
//! from disk: the board's designators and values carried to the schematic,
//! across an open root sheet, a closed child sheet placed once and a closed
//! child sheet placed twice. Execute is one undo group across the sheets;
//! nothing is written to disk.

use std::path::{Path, PathBuf};

use signex_app::app::{
    BackAnnotateMsg, CommandPaletteMsg, EditMsg, Message, Signex, TabInfo, TabKind,
};
use signex_engine::pcb::eco::{BackBlock, BackChangeList, BackKey};
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::{Footprint, PcbBoard, Point as BoardPoint, SymbolLink};
use signex_types::schematic::{
    ChildSheet, FillType, InstancePath, Point, SchematicSheet, Symbol, SymbolInstance,
};
use tempfile::TempDir;
use uuid::Uuid;

use crate::menu_support::menu_row;
use crate::settle_support::settle;

/// Sheet symbols: amp.snxsch placed once (A), buf.snxsch placed twice.
const A: Uuid = Uuid::from_u128(0xa);
const B1: Uuid = Uuid::from_u128(0xb1);
const B2: Uuid = Uuid::from_u128(0xb2);
/// Parts: R1 on the root, R10 on amp, C5 on buf: C5 at /B1 from an
/// instance entry, C9 at /B2 from the symbol's reference.
const R1: Uuid = Uuid::from_u128(0x101);
const R10: Uuid = Uuid::from_u128(0x110);
const C5: Uuid = Uuid::from_u128(0x105);
/// Board footprints.
const FP_R1: Uuid = Uuid::from_u128(0xf1);
const FP_R10: Uuid = Uuid::from_u128(0xf2);
const FP_C5: Uuid = Uuid::from_u128(0xf3);
const FP_C6: Uuid = Uuid::from_u128(0xf4);

fn part(uuid: Uuid, reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid,
        lib_id: "R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        in_bom: true,
        on_board: true,
        ..Default::default()
    }
}

fn child(uuid: Uuid, name: &str, filename: &str, x: f64) -> ChildSheet {
    ChildSheet {
        uuid,
        name: name.to_string(),
        filename: filename.to_string(),
        position: Point::new(x, 50.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn sheet(symbols: Vec<Symbol>, children: Vec<ChildSheet>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols,
        child_sheets: children,
        ..Default::default()
    }
}

fn write_sheet(path: &Path, sheet: &SchematicSheet) {
    let text = SnxSchematic::new(sheet.clone()).write_string().unwrap();
    std::fs::write(path, text).unwrap();
}

fn footprint(
    uuid: Uuid,
    reference: &str,
    value: &str,
    sheet: InstancePath,
    unit: Uuid,
) -> Footprint {
    let mut fp: Footprint = serde_json::from_value(serde_json::json!({
        "uuid": uuid, "reference": reference, "value": value,
        "position": {"x": 0.0, "y": 0.0}, "layer": "TopCopper"
    }))
    .unwrap();
    fp.position = BoardPoint::new(uuid.as_u128() as f64, 10.0);
    fp.schematic_link = Some(SymbolLink::new(sheet, [unit]));
    fp
}

/// The board after edits made on it: R1 renamed R101, R10's value 22k, the
/// first C5 renamed C51 and given 1u.
fn board() -> PcbBoard {
    let mut board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).unwrap();
    let root = InstancePath::root();
    board.footprints = vec![
        footprint(FP_R1, "R101", "10k", root.clone(), R1),
        footprint(FP_R10, "R10", "22k", root.child(A), R10),
        footprint(FP_C5, "C51", "1u", root.child(B1), C5),
        footprint(FP_C6, "C9", "100n", root.child(B2), C5),
    ];
    board
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

struct Rig {
    app: Signex,
    root: PathBuf,
    amp: PathBuf,
    buf: PathBuf,
    board: PathBuf,
    _tmp: TempDir,
}

fn project_data(
    dir: &Path,
    name: &str,
    root: &str,
    pcb_file: Option<&str>,
    sheets: &[&str],
) -> signex_types::project::ProjectData {
    signex_types::project::ProjectData {
        document_id: None,
        description: None,
        name: name.to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some(root.to_string()),
        pcb_file: pcb_file.map(str::to_string),
        sheets: sheets.iter().map(|name| entry(name)).collect(),
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
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

fn add_project(
    app: &mut Signex,
    path: PathBuf,
    data: signex_types::project::ProjectData,
) -> signex_app::app::ProjectId {
    let id = app.document_state.mint_project_id();
    app.document_state
        .projects
        .push(signex_app::app::LoadedProject {
            id,
            path,
            data,
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
    id
}

/// The project on disk with only its root sheet open; the board is closed.
fn rig() -> Rig {
    rig_with(board(), false)
}

/// [`rig`] with `board` as the board file; `other_project` also opens a
/// second project, Other, whose root sheet is amp.snxsch, so another open
/// project reaches that sheet (#1224).
fn rig_with(board: PcbBoard, other_project: bool) -> Rig {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    let root_sheet = sheet(
        vec![part(R1, "R1", "10k")],
        vec![
            child(A, "Amp", "amp.snxsch", 0.0),
            child(B1, "Buf1", "buf.snxsch", 20.0),
            child(B2, "Buf2", "buf.snxsch", 40.0),
        ],
    );
    let mut c5 = part(C5, "C9", "100n");
    // Only /B1 has an entry; /B2 reads the symbol's reference.
    c5.instances.push(SymbolInstance {
        project: "Proj".to_string(),
        path: InstancePath::root().child(B1).to_string(),
        reference: "C5".to_string(),
        unit: 1,
    });
    let files = [
        ("root.snxsch", root_sheet.clone()),
        (
            "amp.snxsch",
            sheet(vec![part(R10, "R10", "10k")], Vec::new()),
        ),
        ("buf.snxsch", sheet(vec![c5], Vec::new())),
    ];
    for (name, content) in &files {
        write_sheet(&dir.join(name), content);
    }
    let board_path = dir.join("Proj.snxpcb");
    std::fs::write(&board_path, SnxPcb::new(board).write_string().unwrap()).unwrap();
    let names: Vec<&str> = files.iter().map(|(name, _)| *name).collect();
    let data = project_data(&dir, "Proj", "root.snxsch", Some("Proj.snxpcb"), &names);
    let prj = dir.join("Proj.snxprj");
    signex_types::project::write_project(&prj, &data).unwrap();

    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    if other_project {
        let other = project_data(&dir, "Other", "amp.snxsch", None, &["amp.snxsch"]);
        let path = dir.join("Other.snxprj");
        signex_types::project::write_project(&path, &other).unwrap();
        add_project(&mut app, path, other);
    }
    let project = add_project(&mut app, prj, data);
    app.document_state.active_project = Some(project);
    let root = dir.join("root.snxsch");
    let engine = signex_engine::Engine::new_with_path(root_sheet, Some(root.clone())).unwrap();
    app.document_state.engines.insert(root.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "root".to_string(),
        path: root.clone(),
        cached_document: None,
        project_id: Some(project),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root.clone());
    Rig {
        app,
        root,
        amp: dir.join("amp.snxsch"),
        buf: dir.join("buf.snxsch"),
        board: board_path,
        _tmp: tmp,
    }
}

fn list(rig: &Rig) -> &BackChangeList {
    let state = rig
        .app
        .ui_state
        .back_annotate
        .as_ref()
        .expect("dialog open");
    assert!(state.is_ready(), "the list is ready");
    state.list().expect("a list")
}

fn symbol<'a>(app: &'a Signex, path: &Path, uuid: Uuid) -> &'a Symbol {
    app.document_state.engines[path]
        .document()
        .symbols
        .iter()
        .find(|s| s.uuid == uuid)
        .expect("symbol")
}

/// Bring the tab of `path` to the front.
fn show(app: &mut Signex, path: &Path) {
    let index = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab");
    let window = app.ui_state.main_window_id.expect("main window");
    settle(
        app,
        Message::Tab {
            window_id: window,
            msg: signex_app::tab_bar::TabMessage::Select(index),
        },
    );
}

fn send(rig: &mut Rig, msg: BackAnnotateMsg) {
    settle(&mut rig.app, Message::from(msg));
}

#[test]
fn the_tools_row_lists_the_board_changes_with_shared_sheet_values_read_only() {
    let mut rig = rig();
    settle(&mut rig.app, menu_row("back_annotate_schematics"));
    let list = list(&rig);
    let rows: Vec<(BackKey, Option<BackBlock>, bool)> = list
        .rows
        .iter()
        .map(|r| (r.key, r.blocked.clone(), r.on))
        .collect();
    assert_eq!(
        rows,
        vec![
            (BackKey::Designator(FP_R1), None, true),
            (BackKey::Designator(FP_C5), None, true),
            (BackKey::Value(FP_R10), None, true),
            (BackKey::Value(FP_C5), Some(BackBlock::SharedSheet), false),
        ]
    );
}

#[test]
fn execute_is_one_undo_group_across_open_and_closed_sheets_and_writes_nothing() {
    let mut rig = rig();
    let on_disk: Vec<Vec<u8>> = [&rig.root, &rig.amp, &rig.buf]
        .iter()
        .map(|p| std::fs::read(p).unwrap())
        .collect();
    settle(&mut rig.app, menu_row("back_annotate_schematics"));
    send(&mut rig, BackAnnotateMsg::ExecuteRequested);
    assert!(
        rig.app.ui_state.back_annotate.is_none(),
        "Execute closes it"
    );

    let app = &rig.app;
    assert_eq!(symbol(app, &rig.root, R1).reference, "R101");
    assert_eq!(
        symbol(app, &rig.amp, R10).value,
        "22k",
        "closed sheet loaded"
    );
    let c5 = symbol(app, &rig.buf, C5);
    assert_eq!(c5.designator_in(&InstancePath::root().child(B1)), "C51");
    assert_eq!(
        c5.designator_in(&InstancePath::root().child(B2)),
        "C9",
        "the other placement keeps its designator"
    );
    assert_eq!(c5.value, "100n", "a shared sheet's value is not carried");
    let unsaved = app.document_state.dirty_document_paths();
    for path in [&rig.amp, &rig.buf] {
        assert!(unsaved.contains(path), "{} is unsaved", path.display());
    }
    let now: Vec<Vec<u8>> = [&rig.root, &rig.amp, &rig.buf]
        .iter()
        .map(|p| std::fs::read(p).unwrap())
        .collect();
    assert_eq!(now, on_disk, "nothing is written to disk");

    // The read opened the board as a tab in front; undo from the root sheet.
    show(&mut rig.app, &rig.root);
    settle(&mut rig.app, Message::Edit(EditMsg::Undo));
    let app = &rig.app;
    assert_eq!(symbol(app, &rig.root, R1).reference, "R1", "one undo");
    let amp = app
        .document_state
        .engines
        .get(&rig.amp)
        .map(|e| e.document().symbols[0].value.clone());
    assert!(
        amp.is_none() || amp.as_deref() == Some("10k"),
        "the closed sheet is back as read: {amp:?}"
    );
    let buf = app.document_state.engines.get(&rig.buf).map(|e| {
        e.document().symbols[0]
            .designator_in(&InstancePath::root().child(B1))
            .to_string()
    });
    assert!(
        buf.is_none() || buf.as_deref() == Some("C5"),
        "the shared sheet is back as read: {buf:?}"
    );
}

#[test]
fn unticked_rows_stay_out_and_validate_says_the_rest_applies() {
    let mut rig = rig();
    settle(&mut rig.app, menu_row("back_annotate_schematics"));
    send(
        &mut rig,
        BackAnnotateMsg::RowToggled {
            key: BackKey::Designator(FP_R1),
            on: false,
        },
    );
    send(&mut rig, BackAnnotateMsg::ValidateRequested);
    let state = rig.app.ui_state.back_annotate.as_ref().unwrap();
    assert_eq!(state.validation(), Some(&Ok(())));
    send(&mut rig, BackAnnotateMsg::ExecuteRequested);
    assert_eq!(symbol(&rig.app, &rig.root, R1).reference, "R1");
    assert_eq!(symbol(&rig.app, &rig.amp, R10).value, "22k");
}

#[test]
fn an_edit_after_the_read_makes_the_list_stale_until_refresh() {
    let mut rig = rig();
    settle(&mut rig.app, menu_row("back_annotate_schematics"));
    rig.app
        .document_state
        .engines
        .get_mut(&rig.root)
        .unwrap()
        .execute(signex_engine::Command::UpdateText {
            target: signex_engine::TextTarget::SymbolValue(R1),
            value: "47k".into(),
        })
        .unwrap();
    assert!(rig.app.back_annotate_stale());
    send(&mut rig, BackAnnotateMsg::ExecuteRequested);
    assert_eq!(
        symbol(&rig.app, &rig.root, R1).reference,
        "R1",
        "a stale list does not run"
    );
    let refused = rig
        .app
        .ui_state
        .back_annotate
        .as_ref()
        .unwrap()
        .validation();
    assert!(matches!(refused, Some(Err(_))), "{refused:?}");
    send(&mut rig, BackAnnotateMsg::RefreshRequested);
    assert!(!rig.app.back_annotate_stale());
    assert!(
        list(&rig).row(&BackKey::Value(FP_R1)).is_some_and(|r| r.on),
        "the refreshed list carries the board's 10k back over 47k"
    );
}

#[test]
fn a_closed_sheet_changed_on_disk_refuses_execute() {
    let mut rig = rig();
    settle(&mut rig.app, menu_row("back_annotate_schematics"));
    let mut amp = sheet(vec![part(R10, "R10", "33k")], Vec::new());
    amp.uuid = Uuid::from_u128(0xdead);
    std::thread::sleep(std::time::Duration::from_millis(20));
    write_sheet(&rig.amp, &amp);
    send(&mut rig, BackAnnotateMsg::ExecuteRequested);
    assert_eq!(symbol(&rig.app, &rig.root, R1).reference, "R1");
    let state = rig.app.ui_state.back_annotate.as_ref().expect("still open");
    assert!(
        matches!(state.validation(), Some(Err(why)) if why.contains("changed on disk")),
        "{:?}",
        state.validation()
    );
}

#[test]
fn the_palette_and_the_board_side_open_the_same_dialog() {
    let mut rig = rig();
    settle(
        &mut rig.app,
        Message::CommandPalette(CommandPaletteMsg::Open),
    );
    settle(
        &mut rig.app,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged(
            "Back-annotate the board".into(),
        )),
    );
    settle(
        &mut rig.app,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );
    assert_eq!(list(&rig).rows.len(), 4);
    send(&mut rig, BackAnnotateMsg::CloseRequested);
    assert!(rig.app.ui_state.back_annotate.is_none());

    // The board, now open from the first read, in front: the same command.
    let board = rig.board.clone();
    show(&mut rig.app, &board);
    let docs = &rig.app.document_state;
    assert_eq!(
        docs.tabs[docs.active_tab].path, board,
        "the board is in front"
    );
    settle(&mut rig.app, menu_row("back_annotate_schematics"));
    assert_eq!(list(&rig).rows.len(), 4);
}

/// #1312 review (data loss): Execute writes exactly the plan it stated the
/// closed sheets for. A row ticked again while the check runs is ignored, so
/// a sheet the check never stated (and that changed on disk meanwhile) is
/// not loaded from the read's stale copy.
#[test]
fn execute_writes_the_plan_it_checked_and_ticks_wait_while_it_runs() {
    let mut rig = rig();
    settle(&mut rig.app, menu_row("back_annotate_schematics"));
    // buf.snxsch has one live row; untick it so only amp is checked.
    send(
        &mut rig,
        BackAnnotateMsg::RowToggled {
            key: BackKey::Designator(FP_C5),
            on: false,
        },
    );
    let task = rig
        .app
        .update(Message::from(BackAnnotateMsg::ExecuteRequested));
    assert!(
        rig.app
            .ui_state
            .back_annotate
            .as_ref()
            .unwrap()
            .is_checking()
    );
    // While checking: the tick and Refresh are ignored.
    send(
        &mut rig,
        BackAnnotateMsg::RowToggled {
            key: BackKey::Designator(FP_C5),
            on: true,
        },
    );
    let job = rig.app.ui_state.back_annotate.as_ref().unwrap().job();
    send(&mut rig, BackAnnotateMsg::RefreshRequested);
    let state = rig.app.ui_state.back_annotate.as_ref().unwrap();
    assert!(state.is_checking(), "Refresh waits for the check");
    assert_eq!(state.job(), job);
    assert!(
        !state
            .list()
            .unwrap()
            .row(&BackKey::Designator(FP_C5))
            .unwrap()
            .on,
        "the tick waits for the check"
    );
    // buf changes on disk while the check runs.
    let mut changed = sheet(vec![part(C5, "C9", "220n")], Vec::new());
    changed.uuid = Uuid::from_u128(0xbeef);
    write_sheet(&rig.buf, &changed);
    let external = std::fs::read(&rig.buf).unwrap();
    crate::settle_support::settle_task(&mut rig.app, task);

    assert!(rig.app.ui_state.back_annotate.is_none(), "the plan ran");
    assert_eq!(symbol(&rig.app, &rig.amp, R10).value, "22k");
    assert!(
        !rig.app.document_state.engines.contains_key(&rig.buf),
        "buf was not in the plan, so its stale copy is never loaded"
    );
    assert_eq!(std::fs::read(&rig.buf).unwrap(), external);
}

/// #1312 review: a blocked row ticked by a message stays off, and Execute
/// leaves its value alone.
#[test]
fn a_blocked_row_cannot_be_ticked_and_execute_leaves_it_alone() {
    let mut rig = rig();
    settle(&mut rig.app, menu_row("back_annotate_schematics"));
    send(
        &mut rig,
        BackAnnotateMsg::RowToggled {
            key: BackKey::Value(FP_C5),
            on: true,
        },
    );
    send(
        &mut rig,
        BackAnnotateMsg::GroupToggled {
            group: signex_engine::pcb::eco::BackGroup::Values,
            on: true,
        },
    );
    let row = list(&rig).row(&BackKey::Value(FP_C5)).unwrap().clone();
    assert_eq!(row.blocked, Some(BackBlock::SharedSheet));
    assert!(!row.on);
    send(&mut rig, BackAnnotateMsg::ExecuteRequested);
    assert_eq!(symbol(&rig.app, &rig.buf, C5).value, "100n");
}

/// #1312 review (#1224): on a sheet another open project reaches, a value
/// row is read-only and a designator is written to this placement's entry,
/// never to the shared `reference`.
#[test]
fn a_sheet_another_open_project_reaches_keeps_its_reference_and_value() {
    let mut board = board();
    for fp in &mut board.footprints {
        if fp.uuid == FP_R10 {
            fp.reference = "R11".into();
        }
    }
    let mut rig = rig_with(board, true);
    settle(&mut rig.app, menu_row("back_annotate_schematics"));
    let value = list(&rig).row(&BackKey::Value(FP_R10)).unwrap().clone();
    assert_eq!(value.blocked, Some(BackBlock::SharedSheet));
    send(&mut rig, BackAnnotateMsg::ExecuteRequested);
    let r10 = symbol(&rig.app, &rig.amp, R10);
    assert_eq!(r10.reference, "R10", "Other's designator is untouched");
    assert_eq!(r10.designator_in(&InstancePath::root().child(A)), "R11");
    assert_eq!(r10.value, "10k");
}

/// #1312 review: a key a profile binds to the command opens the dialog.
#[test]
fn a_bound_key_opens_back_annotate() {
    use signex_app::keymap::{
        AppCommandId, ShortcutBinding, ShortcutBindingAction, ShortcutContext, ShortcutTrigger,
    };
    let mut rig = rig();
    let ShortcutTrigger::KeySequence(mut strokes) = ShortcutTrigger::parse("Ctrl+Alt+F10").unwrap()
    else {
        panic!("a key");
    };
    let profiles = &mut rig.app.ui_state.keymap_profiles;
    profiles
        .active_profile_mut()
        .bindings
        .push(ShortcutBinding {
            action: ShortcutBindingAction::Command(
                AppCommandId::new("back_annotate_schematics").unwrap(),
            ),
            context: ShortcutContext::Global,
            triggers: vec![ShortcutTrigger::KeySequence(strokes.clone())],
        });
    rig.app.ui_state.active_keymap = rig.app.ui_state.keymap_profiles.compile_active().into();
    settle(
        &mut rig.app,
        Message::Ui(signex_app::app::UiMsg::KeymapStroke {
            window: None,
            stroke: strokes.remove(0),
        }),
    );
    assert_eq!(list(&rig).rows.len(), 4);
}

/// #1312 review: a Refresh after the project closed says why in the dialog
/// and in Messages instead of keeping the old list.
#[test]
fn a_refresh_after_the_project_closed_fails_with_a_reason() {
    let mut rig = rig();
    settle(&mut rig.app, menu_row("back_annotate_schematics"));
    assert!(rig.app.ui_state.back_annotate.as_ref().unwrap().is_ready());
    rig.app.document_state.projects.clear();
    let since = signex_app::diagnostics::newest_entry_id().unwrap_or(0);
    send(&mut rig, BackAnnotateMsg::RefreshRequested);
    let state = rig.app.ui_state.back_annotate.as_ref().unwrap();
    assert!(!state.is_ready());
    assert!(state.list().is_none(), "the old list is not kept");
    assert_eq!(state.failure(), Some("The project is no longer open."));
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.id > since && e.message.contains("The project is no longer open")),
        "Messages says why"
    );
}
