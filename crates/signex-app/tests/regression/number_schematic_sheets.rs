//! #2247 - Tools ▸ Annotation ▸ Number Schematic Sheets opens an editable
//! numbering dialog. Apply writes the manual numbers into the parent sheets'
//! sheet symbols as one undo step, in memory only.
//!
//! Real temp dir: `root.snxsch` (open, active) places `power.snxsch` (A) and
//! `io.snxsch` (C); `power.snxsch` (closed) places `ldo.snxsch` (L). Pages in
//! hierarchy order: `/` 1, `/A` 2, `/A/L` 3, `/C` 4.

use signex_app::app::command_palette::{CommandAction, build_catalog};
use signex_app::app::{
    CommandPaletteMsg, EditMsg, Message, NumberingProblem, SheetNumberingMsg, Signex, TabInfo,
    TabKind, UiMsg,
};
use signex_app::keymap::{
    AppCommandId, ShortcutBinding, ShortcutBindingAction, ShortcutContext, ShortcutTrigger,
};
use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet, SheetInstance};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const COMMAND: &str = "number_schematic_sheets";
const A: u128 = 0xA;
const C: u128 = 0xC;
const L: u128 = 0x1;

fn symbol(name: &str, filename: &str, id: u128, x: f64) -> ChildSheet {
    ChildSheet {
        uuid: uuid::Uuid::from_u128(id),
        name: name.to_string(),
        filename: filename.to_string(),
        position: Point::new(x, 0.0),
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

fn sheet(children: Vec<ChildSheet>) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        child_sheets: children,
        ..Default::default()
    }
}

fn write_sheet(path: &Path, sheet: &SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet.clone())
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

struct Rig {
    app: Signex,
    root: PathBuf,
    power: PathBuf,
    _tmp: TempDir,
}

/// The project on disk, with only the root sheet open.
fn rig() -> Rig {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let root_sheet = sheet(vec![
        symbol("A", "power.snxsch", A, 0.0),
        symbol("C", "io.snxsch", C, 20.0),
    ]);
    let files = [
        ("root.snxsch", root_sheet.clone()),
        (
            "power.snxsch",
            sheet(vec![symbol("L", "ldo.snxsch", L, 0.0)]),
        ),
        ("ldo.snxsch", sheet(Vec::new())),
        ("io.snxsch", sheet(Vec::new())),
    ];
    for (name, content) in &files {
        write_sheet(&dir.join(name), content);
    }
    let data = signex_types::project::ProjectData {
        document_id: None,
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("root.snxsch".to_string()),
        pcb_file: None,
        sheets: files.iter().map(|(name, _)| entry(name)).collect(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        parameters: Default::default(),
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
    let root = dir.join("root.snxsch");
    let engine =
        signex_engine::Engine::new_with_path(root_sheet, Some(root.clone())).expect("engine");
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
        power: dir.join("power.snxsch"),
        _tmp: tmp,
    }
}

/// The entries of sheet symbol `symbol` on the open sheet at `path`.
fn entries(app: &Signex, path: &Path, symbol: u128) -> Vec<SheetInstance> {
    app.document_state.engines[path]
        .document()
        .child_sheets
        .iter()
        .find(|cs| cs.uuid == uuid::Uuid::from_u128(symbol))
        .expect("sheet symbol")
        .instances
        .clone()
}

fn send(app: &mut Signex, msg: SheetNumberingMsg) {
    crate::settle_support::settle(app, Message::from(msg));
}

fn type_number(app: &mut Signex, row: usize, text: &str) {
    send(
        app,
        SheetNumberingMsg::NumberEdited {
            row,
            text: text.to_string(),
        },
    );
}

/// The dialog is open and has read the project's four pages.
fn assert_dialog_read(app: &Signex) {
    let state = app.ui_state.sheet_numbering.as_ref().expect("dialog open");
    assert_eq!(state.page_count(), Some(4), "/, /A, /A/L and /C");
}

#[test]
fn the_menu_row_opens_the_dialog_and_apply_writes_one_undo_step_in_memory() {
    let mut rig = rig();
    let power_on_disk = std::fs::read(&rig.power).expect("power on disk");
    crate::settle_support::settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    assert_dialog_read(&rig.app);

    // `/A/L` (stored in the closed power.snxsch) and `/C` (in the open root).
    type_number(&mut rig.app, 2, "6");
    type_number(&mut rig.app, 3, "7");
    send(&mut rig.app, SheetNumberingMsg::ApplyRequested);

    assert!(
        rig.app.ui_state.sheet_numbering.is_none(),
        "Apply closes it"
    );
    let root_entry = SheetInstance {
        project: "Proj".to_string(),
        path: "/".to_string(),
        page: "7".to_string(),
    };
    assert_eq!(entries(&rig.app, &rig.root, C), vec![root_entry]);
    assert!(entries(&rig.app, &rig.root, A).is_empty(), "A unchanged");
    let ldo = entries(&rig.app, &rig.power, L);
    assert_eq!(ldo.len(), 1);
    assert_eq!(ldo[0].page, "6");
    assert_eq!(ldo[0].path, format!("/{}", uuid::Uuid::from_u128(A)));
    assert_eq!(
        std::fs::read(&rig.power).expect("power on disk"),
        power_on_disk,
        "nothing is written until the user saves"
    );
    let unsaved = rig.app.document_state.dirty_document_paths();
    assert!(unsaved.contains(&rig.root) && unsaved.contains(&rig.power));

    // One undo takes both sheets back.
    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    assert!(entries(&rig.app, &rig.root, C).is_empty());
    let power_back = rig
        .app
        .document_state
        .engines
        .get(&rig.power)
        .map(|engine| engine.document().child_sheets[0].instances.clone());
    assert!(
        power_back.is_none_or(|instances| instances.is_empty()),
        "the closed sheet is back as loaded"
    );
}

#[test]
fn cancel_closes_the_dialog_and_changes_nothing() {
    let mut rig = rig();
    crate::settle_support::settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    type_number(&mut rig.app, 3, "9");
    send(&mut rig.app, SheetNumberingMsg::CancelRequested);
    assert!(rig.app.ui_state.sheet_numbering.is_none());
    assert!(entries(&rig.app, &rig.root, C).is_empty());
    assert!(!rig.app.document_state.engines.contains_key(&rig.power));
}

#[test]
fn auto_number_clears_a_hand_number() {
    let mut rig = rig();
    crate::settle_support::settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    type_number(&mut rig.app, 3, "9");
    send(&mut rig.app, SheetNumberingMsg::ApplyRequested);
    assert_eq!(entries(&rig.app, &rig.root, C).len(), 1);

    crate::settle_support::settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    send(&mut rig.app, SheetNumberingMsg::AutoNumberRequested);
    send(&mut rig.app, SheetNumberingMsg::ApplyRequested);
    assert!(
        entries(&rig.app, &rig.root, C).is_empty(),
        "back to its position, the entry is gone"
    );
}

#[test]
fn the_palette_lists_and_runs_it() {
    let mut rig = rig();
    let listed = build_catalog(&rig.app)
        .iter()
        .any(|entry| matches!(&entry.action, CommandAction::Command(id) if id.as_str() == COMMAND));
    assert!(listed, "the palette offers Number schematic sheets");
    let _ = rig
        .app
        .update(Message::CommandPalette(CommandPaletteMsg::Open));
    let _ = rig
        .app
        .update(Message::CommandPalette(CommandPaletteMsg::QueryChanged(
            "Number schematic sheets".into(),
        )));
    let task = rig
        .app
        .update(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected));
    crate::settle_support::settle_task(&mut rig.app, task);
    assert!(rig.app.ui_state.sheet_numbering.is_some(), "dialog open");
}

#[test]
fn a_user_key_binding_runs_it() {
    let mut rig = rig();
    bind(&mut rig.app, COMMAND, "Ctrl+Alt+Shift+F11");
    let ShortcutTrigger::KeySequence(mut strokes) =
        ShortcutTrigger::parse("Ctrl+Alt+Shift+F11").unwrap()
    else {
        panic!("a key");
    };
    let task = rig.app.update(Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: strokes.remove(0),
    }));
    crate::settle_support::settle_task(&mut rig.app, task);
    assert!(rig.app.ui_state.sheet_numbering.is_some(), "dialog open");
}

#[test]
fn a_loose_sheet_without_a_project_opens_nothing() {
    let mut rig = rig();
    rig.app.document_state.projects.clear();
    rig.app.document_state.active_project = None;
    rig.app.document_state.tabs[0].project_id = None;
    crate::settle_support::settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    assert!(rig.app.ui_state.sheet_numbering.is_none());
}

/// The dialog read, `/A/L` (stored in the closed power.snxsch) and `/C` (in
/// the open root) typed, ready to Apply.
fn typed(rig: &mut Rig) {
    crate::settle_support::settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    assert_dialog_read(&rig.app);
    type_number(&mut rig.app, 2, "6");
    type_number(&mut rig.app, 3, "7");
}

/// Apply was refused with `problem`: the dialog is still open and says so,
/// and no sheet changed.
fn assert_refused(rig: &Rig, problem: &NumberingProblem) {
    let state = rig
        .app
        .ui_state
        .sheet_numbering
        .as_ref()
        .expect("still open");
    assert_eq!(state.problem(), Some(problem));
    assert!(entries(&rig.app, &rig.root, C).is_empty(), "root unchanged");
    assert!(
        !rig.app.document_state.engines.contains_key(&rig.power),
        "the closed sheet was not loaded"
    );
}

#[test]
fn apply_refuses_when_a_sheet_was_opened_or_closed_since_the_read() {
    let mut rig = rig();
    typed(&mut rig);
    // An engine came or went (`insert_engine` / `remove_engine` move it).
    rig.app.document_state.documents_epoch += 1;
    send(&mut rig.app, SheetNumberingMsg::ApplyRequested);
    assert_refused(&rig, &NumberingProblem::Stale);
}

#[test]
fn apply_refuses_when_a_sheet_symbol_changed_since_the_read() {
    let mut rig = rig();
    typed(&mut rig);
    rig.app
        .document_state
        .engines
        .get_mut(&rig.root)
        .expect("root engine")
        .execute(signex_engine::Command::ModifyChildSheet {
            sheet_id: uuid::Uuid::from_u128(A),
            name: Some("Power".into()),
            filename: None,
        })
        .expect("rename the sheet symbol");
    send(&mut rig.app, SheetNumberingMsg::ApplyRequested);
    assert_refused(&rig, &NumberingProblem::Stale);
}

#[test]
fn apply_refuses_while_a_closed_parent_sheet_is_being_moved() {
    let mut rig = rig();
    typed(&mut rig);
    let _op = rig.app.document_state.pending_path_ops.begin(
        signex_app::app::path_ops::PathOpKind::Rename,
        vec![rig.power.clone()],
        signex_app::app::CaseFold::default(),
    );
    send(&mut rig.app, SheetNumberingMsg::ApplyRequested);
    assert_refused(&rig, &NumberingProblem::Busy(rig.power.clone()));
}

/// Give `command` the key `keys` in a custom copy of the active profile.
pub fn bind(app: &mut Signex, command: &str, keys: &str) {
    let profiles = &mut app.ui_state.keymap_profiles;
    let mut custom = profiles
        .active_profile()
        .copy_as_custom("user-2247", "User 2247")
        .expect("custom profile");
    custom.bindings.push(ShortcutBinding {
        action: ShortcutBindingAction::Command(AppCommandId::new(command).expect("id")),
        context: ShortcutContext::Global,
        triggers: vec![ShortcutTrigger::parse(keys).expect("trigger")],
    });
    profiles.insert_custom_profile(custom).expect("insert");
    profiles.set_active_profile("user-2247").expect("activate");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
}
