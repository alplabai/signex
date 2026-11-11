//! #2248 - Tools ▸ Assign Footprints opens a project-wide table of the
//! placed parts. Each part takes only its own library row's footprints;
//! ticked parts are set at once; Apply is one undo step across the open
//! sheets, in memory only. A closed sheet's parts are listed, not changed.
//!
//! Real temp dir, project `P`: `a.snxsch` (open, active) and `b.snxsch`
//! (open) each hold a Stock resistor; `c.snxsch` (closed) holds a third.

use std::path::{Path, PathBuf};

use signex_app::app::command_palette::{CommandAction, build_catalog};
use signex_app::app::{
    AssignFootprintsMsg, AssignProblem, CommandPaletteMsg, EditMsg, LoadedProject, Message,
    PartAccess, Signex, TabInfo, TabKind, UiMsg,
};
use signex_app::dock::DockMessage;
use signex_app::keymap::{
    AppCommandId, ShortcutBinding, ShortcutBindingAction, ShortcutContext, ShortcutTrigger,
};
use signex_app::panels::PanelMsg;
use signex_library::primitive::PrimitiveRef;
use signex_types::schematic::{SchematicSheet, Symbol};
use tempfile::TempDir;

use crate::library_footprint_pick::{alternates, binding_key, key_of, place_selected, select};
use crate::settle_support::{settle, settle_task};

const COMMAND: &str = "assign_footprints";

struct Rig {
    app: Signex,
    a: PathBuf,
    b: PathBuf,
    c: PathBuf,
    /// The first alternate footprint of the Stock resistor row, and its name.
    alt: (PrimitiveRef, String),
    _tmp: TempDir,
}

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

fn open_tab(
    app: &mut Signex,
    path: &Path,
    sheet: SchematicSheet,
    project: signex_app::app::ProjectId,
) {
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

fn rig() -> Rig {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let (a, b, c) = (
        dir.join("a.snxsch"),
        dir.join("b.snxsch"),
        dir.join("c.snxsch"),
    );
    for path in [&a, &b, &c] {
        write_sheet(path, &empty_sheet());
    }
    let data = signex_types::project::ProjectData {
        document_id: None,
        description: None,
        name: "P".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("a.snxsch".to_string()),
        pcb_file: None,
        sheets: ["a.snxsch", "b.snxsch", "c.snxsch"]
            .iter()
            .map(|f| entry(f))
            .collect(),
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
        agent_journal: Default::default(),
        unknown: Default::default(),
    };
    let prj = dir.join("P.snxprj");
    signex_types::project::write_project(&prj, &data).expect("write project");

    let (mut app, _task) = Signex::new();
    let stock =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib");
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock)
        .expect("mount stock");
    let project = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id: project,
        path: prj,
        data,
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(project);
    open_tab(&mut app, &a, empty_sheet(), project);
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(a.clone());

    // R on `a` through the real place flow, then copies on `b` and `c`.
    let key = key_of(&app, "STOCK-R");
    select(&mut app, &key);
    let alt = alternates(&app).remove(0);
    place_selected(&mut app, &key);
    let placed = app.document_state.engines[&a].document().symbols[0].clone();
    let mut on_b = empty_sheet();
    on_b.symbols.push(copy_of(&placed, "R2"));
    open_tab(&mut app, &b, on_b, project);
    let mut on_c = empty_sheet();
    on_c.symbols.push(copy_of(&placed, "R3"));
    write_sheet(&c, &on_c);
    Rig {
        app,
        a,
        b,
        c,
        alt,
        _tmp: tmp,
    }
}

fn send(app: &mut Signex, msg: AssignFootprintsMsg) {
    settle(app, Message::from(msg));
}

fn footprint_on(app: &Signex, path: &Path) -> String {
    app.document_state.engines[path].document().symbols[0]
        .footprint
        .clone()
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
fn the_menu_row_sets_ticked_parts_on_two_sheets_as_one_undo_step_in_memory() {
    let mut rig = rig();
    let c_on_disk = std::fs::read(&rig.c).expect("c on disk");
    let before = footprint_on(&rig.app, &rig.a);
    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    assert_dialog_read(&rig.app);

    send(&mut rig.app, AssignFootprintsMsg::AllToggled(true));
    send(
        &mut rig.app,
        AssignFootprintsMsg::TickedPicked(binding_key(&rig.alt.0)),
    );
    send(&mut rig.app, AssignFootprintsMsg::ApplyRequested);

    assert!(
        rig.app.ui_state.assign_footprints.is_none(),
        "Apply closes it"
    );
    assert_eq!(footprint_on(&rig.app, &rig.a), rig.alt.1);
    assert_eq!(footprint_on(&rig.app, &rig.b), rig.alt.1);
    let stamp = rig.app.document_state.engines[&rig.b].document().symbols[0]
        .valid_stamp()
        .expect("the part keeps its row link");
    assert_eq!(
        stamp.footprint_ref.as_deref(),
        Some(binding_key(&rig.alt.0).as_str())
    );
    assert_eq!(
        std::fs::read(&rig.c).expect("c on disk"),
        c_on_disk,
        "a closed sheet is not changed"
    );
    assert!(!rig.app.document_state.engines.contains_key(&rig.c));
    assert!(
        rig.app
            .document_state
            .dirty_document_paths()
            .contains(&rig.b),
        "b is unsaved"
    );

    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(footprint_on(&rig.app, &rig.a), before);
    assert_eq!(
        footprint_on(&rig.app, &rig.b),
        before,
        "one undo takes both back"
    );

    let _ = rig.app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(footprint_on(&rig.app, &rig.a), rig.alt.1);
    assert_eq!(
        footprint_on(&rig.app, &rig.b),
        rig.alt.1,
        "one redo brings both back"
    );
}

#[test]
fn one_part_takes_a_pick_and_cancel_changes_nothing() {
    let mut rig = rig();
    let before = footprint_on(&rig.app, &rig.a);
    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    let state = rig.app.ui_state.assign_footprints.as_ref().unwrap();
    let row = state
        .parts()
        .iter()
        .position(|p| p.sheet == rig.b)
        .expect("R2 is listed");
    send(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked {
            row,
            binding: binding_key(&rig.alt.0),
        },
    );
    send(&mut rig.app, AssignFootprintsMsg::CancelRequested);
    assert!(rig.app.ui_state.assign_footprints.is_none());
    assert_eq!(footprint_on(&rig.app, &rig.b), before);

    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    send(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked {
            row,
            binding: binding_key(&rig.alt.0),
        },
    );
    send(&mut rig.app, AssignFootprintsMsg::ApplyRequested);
    assert_eq!(footprint_on(&rig.app, &rig.b), rig.alt.1);
    assert_eq!(footprint_on(&rig.app, &rig.a), before, "only R2 was picked");
}

#[test]
fn apply_changes_nothing_when_a_picked_sheet_was_closed_after_the_read() {
    let mut rig = rig();
    let before = footprint_on(&rig.app, &rig.a);
    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    send(&mut rig.app, AssignFootprintsMsg::AllToggled(true));
    send(
        &mut rig.app,
        AssignFootprintsMsg::TickedPicked(binding_key(&rig.alt.0)),
    );
    rig.app.document_state.engines.remove(&rig.b);
    send(&mut rig.app, AssignFootprintsMsg::ApplyRequested);

    let state = rig
        .app
        .ui_state
        .assign_footprints
        .as_ref()
        .expect("still open");
    assert_eq!(
        state.problem(),
        Some(&AssignProblem::SheetClosed(rig.b.clone()))
    );
    assert_eq!(
        footprint_on(&rig.app, &rig.a),
        before,
        "all or nothing: a is not changed either"
    );
}

#[test]
fn a_footprint_the_row_does_not_offer_is_not_taken() {
    let mut rig = rig();
    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    let library = rig.alt.0.library_id;
    let stranger = binding_key(&PrimitiveRef::new(library, uuid::Uuid::now_v7()));
    send(&mut rig.app, AssignFootprintsMsg::AllToggled(true));
    send(&mut rig.app, AssignFootprintsMsg::TickedPicked(stranger));
    let state = rig.app.ui_state.assign_footprints.as_ref().unwrap();
    assert!(state.changes().is_empty());
    assert!(!state.can_apply());
}

#[test]
fn the_palette_lists_and_runs_it() {
    let mut rig = rig();
    let listed = build_catalog(&rig.app)
        .iter()
        .any(|entry| matches!(&entry.action, CommandAction::Command(id) if id.as_str() == COMMAND));
    assert!(listed, "the palette offers Assign footprints");
    let _ = rig
        .app
        .update(Message::CommandPalette(CommandPaletteMsg::Open));
    let _ = rig
        .app
        .update(Message::CommandPalette(CommandPaletteMsg::QueryChanged(
            "Assign footprints".into(),
        )));
    let task = rig
        .app
        .update(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected));
    settle_task(&mut rig.app, task);
    assert_dialog_read(&rig.app);
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
    settle_task(&mut rig.app, task);
    assert_dialog_read(&rig.app);
}

#[test]
fn with_no_schematic_open_nothing_opens() {
    let (mut app, _boot) = Signex::new();
    send(&mut app, AssignFootprintsMsg::Requested);
    assert!(app.ui_state.assign_footprints.is_none());
}

/// The rig with one two-unit part `U1` instead: unit 1 on the open `a`, unit
/// 2 on the open `b`, or on the closed `c` when `second_closed`.
fn multi_unit_rig(second_closed: bool) -> Rig {
    let mut rig = rig();
    let placed = rig.app.document_state.engines[&rig.a].document().symbols[0].clone();
    let unit = |n: u32| Symbol {
        unit: n,
        ..copy_of(&placed, "U1")
    };
    let sheet_with = |symbols: Vec<Symbol>| SchematicSheet {
        symbols,
        ..empty_sheet()
    };
    let engine = |path: &Path, sheet: SchematicSheet| {
        signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf())).expect("engine")
    };
    let a = engine(&rig.a, sheet_with(vec![unit(1)]));
    rig.app.document_state.engines.insert(rig.a.clone(), a);
    let on_b = if second_closed {
        Vec::new()
    } else {
        vec![unit(2)]
    };
    let b = engine(&rig.b, sheet_with(on_b));
    rig.app.document_state.engines.insert(rig.b.clone(), b);
    let on_c = if second_closed {
        vec![unit(2)]
    } else {
        Vec::new()
    };
    write_sheet(&rig.c, &sheet_with(on_c));
    rig
}

/// #2284 review: a part's units on two sheets are one row, and one pick
/// gives every unit the footprint in one undo step.
#[test]
fn a_part_with_units_on_two_sheets_is_one_row_and_one_pick_changes_both() {
    let mut rig = multi_unit_rig(false);
    let before = footprint_on(&rig.app, &rig.a);
    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert_eq!(state.parts().len(), 1, "{:?}", state.parts());
    let part = &state.parts()[0];
    assert_eq!(part.designator, "U1");
    assert_eq!(part.units.len(), 2, "a unit on a and on b");
    assert_eq!(part.access, PartAccess::Editable);

    send(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked {
            row: 0,
            binding: binding_key(&rig.alt.0),
        },
    );
    send(&mut rig.app, AssignFootprintsMsg::ApplyRequested);
    assert!(rig.app.ui_state.assign_footprints.is_none());
    assert_eq!(footprint_on(&rig.app, &rig.a), rig.alt.1);
    assert_eq!(footprint_on(&rig.app, &rig.b), rig.alt.1, "unit B follows");

    let _ = rig.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(footprint_on(&rig.app, &rig.a), before);
    assert_eq!(footprint_on(&rig.app, &rig.b), before, "one undo step");
}

/// A part with a unit on a closed sheet cannot be changed here: changing the
/// open unit alone would give one designator two footprints.
#[test]
fn a_part_with_a_unit_on_a_closed_sheet_is_read_only() {
    let mut rig = multi_unit_rig(true);
    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert_eq!(state.parts().len(), 1, "{:?}", state.parts());
    assert_eq!(state.parts()[0].access, PartAccess::SheetClosed);
    send(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked {
            row: 0,
            binding: binding_key(&rig.alt.0),
        },
    );
    let state = rig.app.ui_state.assign_footprints.as_ref().unwrap();
    assert!(!state.can_apply(), "the pick is not taken");
}

/// #2284 review: a part deleted after the read is named by the designator the
/// dialog listed, since the engine has none for it any more.
#[test]
fn a_part_deleted_after_the_read_is_named_in_the_refusal() {
    let mut rig = rig();
    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    let state = rig.app.ui_state.assign_footprints.as_ref().unwrap();
    let row = state
        .parts()
        .iter()
        .position(|p| p.sheet == rig.b)
        .expect("R2 is listed");
    let id = state.parts()[row].symbol_id;
    send(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked {
            row,
            binding: binding_key(&rig.alt.0),
        },
    );
    rig.app
        .document_state
        .engines
        .get_mut(&rig.b)
        .expect("b open")
        .execute(signex_engine::Command::DeleteSelection {
            items: vec![signex_types::schematic::SelectedItem::new(
                id,
                signex_types::schematic::SelectedKind::Symbol,
            )],
        })
        .expect("delete R2");
    send(&mut rig.app, AssignFootprintsMsg::ApplyRequested);
    let state = rig
        .app
        .ui_state
        .assign_footprints
        .as_ref()
        .expect("still open");
    match state.problem() {
        Some(AssignProblem::Refused { reference, .. }) => assert_eq!(reference, "R2"),
        other => panic!("expected a refusal naming R2, got {other:?}"),
    }
}

/// The dialog's row of the part on `sheet`.
fn row_on(rig: &Rig, sheet: &Path) -> usize {
    rig.app
        .ui_state
        .assign_footprints
        .as_ref()
        .expect("open")
        .parts()
        .iter()
        .position(|p| p.sheet == sheet)
        .expect("a part on that sheet")
}

/// #2284 review (hkngln): a footprint changed behind the dialog after the
/// read refuses Apply as stale, instead of applying over a state the dialog
/// never showed.
#[test]
fn apply_refuses_as_stale_when_a_listed_part_changed_after_the_read() {
    let mut rig = rig();
    let before = footprint_on(&rig.app, &rig.a);
    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    let (r1, r2) = (row_on(&rig, &rig.a), row_on(&rig, &rig.b));
    let state = rig.app.ui_state.assign_footprints.as_ref().unwrap();
    let (r1_id, r1_name) = (
        state.parts()[r1].symbol_id,
        state.parts()[r1].designator.clone(),
    );
    let default = state.parts()[r1].offers[0].binding.clone();

    // Behind the dialog, Properties switches R1 to the alternate.
    settle(
        &mut rig.app,
        Message::Dock(DockMessage::Panel(PanelMsg::SwitchFootprintChoice(
            r1_id,
            binding_key(&rig.alt.0),
        ))),
    );
    assert_eq!(footprint_on(&rig.app, &rig.a), rig.alt.1);

    send(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked {
            row: r1,
            binding: default,
        },
    );
    send(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked {
            row: r2,
            binding: binding_key(&rig.alt.0),
        },
    );
    send(&mut rig.app, AssignFootprintsMsg::ApplyRequested);

    let state = rig
        .app
        .ui_state
        .assign_footprints
        .as_ref()
        .expect("still open");
    assert_eq!(state.problem(), Some(&AssignProblem::Stale(r1_name)));
    assert_eq!(footprint_on(&rig.app, &rig.b), before, "R2 not changed");
    assert_eq!(
        footprint_on(&rig.app, &rig.a),
        rig.alt.1,
        "R1 keeps what Properties set"
    );
}

/// An engine refusal on a later sheet puts the earlier sheets back and
/// leaves no undo group: R2 on `b` is locked after the read.
#[test]
fn an_engine_refusal_part_way_puts_the_earlier_sheets_back() {
    let mut rig = rig();
    let before = footprint_on(&rig.app, &rig.a);
    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    let r2 = row_on(&rig, &rig.b);
    let r2_id = rig.app.ui_state.assign_footprints.as_ref().unwrap().parts()[r2].symbol_id;
    rig.app
        .document_state
        .engines
        .get_mut(&rig.b)
        .unwrap()
        .execute(signex_engine::Command::SetSymbolLocked {
            symbol_id: r2_id,
            locked: true,
        })
        .expect("lock R2");
    send(&mut rig.app, AssignFootprintsMsg::AllToggled(true));
    send(
        &mut rig.app,
        AssignFootprintsMsg::TickedPicked(binding_key(&rig.alt.0)),
    );
    send(&mut rig.app, AssignFootprintsMsg::ApplyRequested);

    let state = rig
        .app
        .ui_state
        .assign_footprints
        .as_ref()
        .expect("still open");
    assert_eq!(
        state.problem(),
        Some(&AssignProblem::NotChanged(rig.b.clone()))
    );
    assert_eq!(footprint_on(&rig.app, &rig.a), before, "a was put back");
    assert_eq!(footprint_on(&rig.app, &rig.b), before);
    assert!(
        rig.app.document_state.engines[&rig.a]
            .next_undo_group()
            .is_none(),
        "a's history holds no group entry"
    );
    assert!(
        rig.app.document_state.undo_groups.is_empty(),
        "no group left"
    );
}

/// A sheet that became read-only after the read refuses Apply.
#[test]
fn apply_refuses_a_sheet_that_became_read_only() {
    let mut rig = rig();
    let before = footprint_on(&rig.app, &rig.a);
    settle(&mut rig.app, crate::menu_support::menu_row(COMMAND));
    let mut sheet = rig.app.document_state.engines[&rig.b].document().clone();
    sheet.unknown.newer_minor_source = Some("1.99".to_string());
    let engine = signex_engine::Engine::new_with_path(sheet, Some(rig.b.clone())).expect("engine");
    rig.app.document_state.engines.insert(rig.b.clone(), engine);
    send(&mut rig.app, AssignFootprintsMsg::AllToggled(true));
    send(
        &mut rig.app,
        AssignFootprintsMsg::TickedPicked(binding_key(&rig.alt.0)),
    );
    send(&mut rig.app, AssignFootprintsMsg::ApplyRequested);

    let state = rig
        .app
        .ui_state
        .assign_footprints
        .as_ref()
        .expect("still open");
    assert_eq!(
        state.problem(),
        Some(&AssignProblem::ReadOnly(rig.b.clone()))
    );
    assert_eq!(footprint_on(&rig.app, &rig.a), before, "nothing changed");
}

/// Give `command` the key `keys` in a custom copy of the active profile.
pub fn bind(app: &mut Signex, command: &str, keys: &str) {
    let profiles = &mut app.ui_state.keymap_profiles;
    let mut custom = profiles
        .active_profile()
        .copy_as_custom("user-2248", "User 2248")
        .expect("custom profile");
    custom.bindings.push(ShortcutBinding {
        action: ShortcutBindingAction::Command(AppCommandId::new(command).expect("id")),
        context: ShortcutContext::Global,
        triggers: vec![ShortcutTrigger::parse(keys).expect("trigger")],
        platform: None,
    });
    profiles.insert_custom_profile(custom).expect("insert");
    profiles.set_active_profile("user-2248").expect("activate");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
}
