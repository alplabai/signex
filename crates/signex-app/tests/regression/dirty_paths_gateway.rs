//! #585 — every engine edit must reach `dirty_paths`.
//!
//! `DocumentState.dirty_paths` is the single source of truth for "this
//! file has unsaved edits". It is written only through
//! `with_active_schematic_session_mut`, which only
//! `finish_schematic_mutation` calls — so an edit that runs the engine
//! directly applies and repaints, and since #584 undoes correctly, but
//! leaves the app believing the document is clean.
//!
//! The consequence a user meets is in
//! `quitting_after_a_move_selection_warns_instead_of_discarding_it`:
//! `handle_app_quit_requested` short-circuits on
//! `dirty_paths.is_empty()`, so the unsaved-changes modal never opens
//! and the edit goes out with the process.
//!
//! Sites covered here: `erc/modals.rs` `handle_move_selection_apply` and
//! `handle_parameter_manager_edit`, and `erc/annotate.rs` `handle_annotate`
//! and `handle_reset_duplicate_designators` on the active engine.

use signex_app::app::{
    AnnotateMsg, Message, MoveSelectionMsg, ParameterManagerMsg, Signex, WindowMsg,
};
use signex_app::menu_bar::MenuMessage;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn symbol(uuid: uuid::Uuid, reference: &str, x: f64) -> Symbol {
    Symbol {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid,
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: String::new(),
        datasheet: String::new(),
        position: Point::new(x, 0.0),
        rotation: 0.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        is_power: false,
        ref_text: None,
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: HashMap::new(),
        custom_properties: Vec::new(),
        pin_uuids: HashMap::new(),
        instances: Vec::new(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
    }
}

fn sheet_with(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols,
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
    }
}

/// A loaded, active schematic. The `TabInfo` matters:
/// `finish_schematic_mutation` reaches `dirty_paths` through
/// `with_active_schematic_session_mut`, which needs a tab at
/// `active_tab` or it silently no-ops.
fn app_with(symbols: Vec<Symbol>) -> (Signex, PathBuf) {
    let path = PathBuf::from("dirty-gateway.snxsch");
    let engine = signex_engine::Engine::new(sheet_with(symbols)).expect("engine");

    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "dirty-gateway".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());

    (app, path)
}

fn assert_dirty(app: &Signex, path: &Path, what: &str) {
    assert!(
        app.document_state.dirty_paths.contains(path),
        "{what} must put the sheet in `dirty_paths`; it is the only thing the quit \
         guard (close_project.rs), the tab-close engine parking (document_tabs.rs), \
         the title-bar unsaved count and Save-All read. dirty_paths = {:?}",
        app.document_state.dirty_paths
    );
    assert!(
        app.document_state.tabs[0].dirty,
        "{what} must also flip the tab's own dirty flag"
    );
}

/// Apply a non-zero Move Selection to `uuid` through the dialog.
fn move_selection(app: &mut Signex, uuid: uuid::Uuid) {
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(uuid, SelectedKind::Symbol)];
    app.ui_state.move_selection.dx = "10".to_string();
    app.ui_state.move_selection.dy = "0".to_string();
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));
}

#[test]
fn a_move_selection_marks_the_document_dirty() {
    let moved = uuid::Uuid::new_v4();
    let (mut app, path) = app_with(vec![symbol(moved, "R1", 50.0)]);

    move_selection(&mut app, moved);

    let x = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols[0]
        .position
        .x;
    assert!(
        (x - 60.0).abs() < 1e-9,
        "fixture precondition: the move must have applied (50.0 + 10.0), got {x}"
    );

    assert_dirty(&app, &path, "a Move Selection");
}

/// The user-visible failure: the edit is applied and on screen, but
/// quitting does not warn, so it goes out with the process.
#[test]
fn quitting_after_a_move_selection_warns_instead_of_discarding_it() {
    let moved = uuid::Uuid::new_v4();
    let (mut app, path) = app_with(vec![symbol(moved, "R1", 50.0)]);

    move_selection(&mut app, moved);

    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));

    let modal = app.ui_state.app_quit_confirm.as_ref().expect(
        "quitting with an applied-but-unsaved Move Selection must raise the \
         unsaved-changes modal; `handle_app_quit_requested` short-circuits on \
         `dirty_paths.is_empty()`, so without the dirty entry the app just exits \
         and the move is lost",
    );
    assert!(
        modal.dirty_paths.contains(&path),
        "the modal must name the sheet the user is about to lose"
    );
}

#[test]
fn a_parameter_manager_edit_marks_the_document_dirty() {
    let target = uuid::Uuid::new_v4();
    let (mut app, path) = app_with(vec![symbol(target, "R1", 0.0)]);

    let _ = app.update(Message::ParameterManager(ParameterManagerMsg::Edit {
        symbol_uuid: target,
        key: "Tolerance".to_string(),
        value: "1%".to_string(),
    }));

    assert_eq!(
        app.document_state
            .active_engine()
            .expect("engine")
            .document()
            .symbols[0]
            .fields
            .get("Tolerance")
            .map(String::as_str),
        Some("1%"),
        "fixture precondition: the field edit must have applied"
    );

    assert_dirty(&app, &path, "a Parameter Manager edit");
}

#[test]
fn annotate_marks_the_active_sheet_dirty() {
    let (mut app, path) = app_with(vec![
        symbol(uuid::Uuid::new_v4(), "R?", 0.0),
        symbol(uuid::Uuid::new_v4(), "R?", 10.0),
    ]);

    let _ = app.update(Message::Annotate(AnnotateMsg::Run(
        signex_engine::AnnotateMode::ResetAndRenumber,
    )));

    let references: Vec<String> = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect();
    assert!(
        references.iter().all(|r| r != "R?"),
        "fixture precondition: annotate must have numbered both symbols, got {references:?}"
    );

    assert_dirty(&app, &path, "Annotate");
}

#[test]
fn reset_duplicate_designators_marks_the_active_sheet_dirty() {
    let (mut app, path) = app_with(vec![
        symbol(uuid::Uuid::new_v4(), "R1", 0.0),
        symbol(uuid::Uuid::new_v4(), "R1", 10.0),
    ]);

    let _ = app.update(Message::Menu(MenuMessage::AnnotateResetDuplicates));

    let references: Vec<String> = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect();
    assert!(
        references.iter().any(|r| r == "R?"),
        "fixture precondition: the duplicate R1 must have been reset, got {references:?}"
    );

    assert_dirty(&app, &path, "Reset Duplicate Designators");
}

/// A project whose root sheet is the one open tab and whose child sheet is
/// parked: a dirty engine with no tab (`close_tab_now`), carrying
/// `parked_symbols` as its unsaved edit while the file on disk holds a single
/// `R1`. Returns the saved bytes so a test can prove nothing was written.
fn app_with_parked_sheet(
    parked_symbols: Vec<Symbol>,
) -> (Signex, PathBuf, Vec<u8>, tempfile::TempDir) {
    let dir = tempfile::Builder::new()
        .prefix("parked-project")
        .tempdir()
        .expect("tempdir");
    let root_path = dir.path().join("root.snxsch");
    let parked_path = dir.path().join("parked.snxsch");

    // The saved copy of the parked sheet: what is on disk and must stay there.
    let mut on_disk =
        signex_engine::Engine::new(sheet_with(vec![symbol(uuid::Uuid::new_v4(), "R1", 0.0)]))
            .expect("engine");
    on_disk.set_path(Some(parked_path.clone()));
    on_disk.save().expect("seed the saved copy");
    let saved_bytes = std::fs::read(&parked_path).expect("read saved copy");

    // The unsaved edit the user parked: an extra, not-yet-annotated symbol.
    let parked = signex_engine::Engine::new(sheet_with(parked_symbols)).expect("engine");

    let mut root_sheet = sheet_with(vec![symbol(uuid::Uuid::new_v4(), "C?", 0.0)]);
    root_sheet
        .child_sheets
        .push(signex_types::schematic::ChildSheet {
            uuid: uuid::Uuid::nil(),
            name: String::new(),
            filename: "parked.snxsch".to_string(),
            position: Point::new(0.0, 0.0),
            size: (0.0, 0.0),
            stroke_width: 0.0,
            fill: signex_types::schematic::FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        });
    let root = signex_engine::Engine::new(root_sheet).expect("engine");

    let (mut app, _initial_task) = Signex::new();
    app.document_state.engines.insert(root_path.clone(), root);
    app.document_state
        .engines
        .insert(parked_path.clone(), parked);
    app.document_state.dirty_paths.insert(parked_path.clone());
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "root".to_string(),
        path: root_path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root_path);

    (app, parked_path, saved_bytes, dir)
}

/// #669 — a dirty sheet whose tab was closed stays parked in `engines`
/// (`close_tab_now`). Annotate must treat it as the open document it still
/// is: number the parked engine in memory and keep it dirty, never write the
/// user's unsaved edits to disk behind their back.
#[test]
fn annotate_numbers_a_parked_sheet_in_memory_and_never_writes_it() {
    let (mut app, parked_path, saved_bytes, _dir) = app_with_parked_sheet(vec![
        symbol(uuid::Uuid::new_v4(), "R1", 0.0),
        symbol(uuid::Uuid::new_v4(), "R?", 10.0),
    ]);

    let _ = app.update(Message::Annotate(AnnotateMsg::Run(
        signex_engine::AnnotateMode::ResetAndRenumber,
    )));

    let parked_refs = parked_references(&app, &parked_path);
    assert!(
        parked_refs.iter().all(|r| !r.ends_with('?')),
        "the parked engine must carry the annotation, got {parked_refs:?}"
    );
    assert_parked_untouched(&app, &parked_path, &saved_bytes, "Annotate");
}

/// #669 — Reset Duplicate Designators counts a parked sheet (the project set
/// prefers `engines`), so it must reset it too, in memory, instead of
/// reporting duplicates it never touched.
#[test]
fn reset_duplicates_resets_a_parked_sheet_in_memory_and_never_writes_it() {
    let (mut app, parked_path, saved_bytes, _dir) = app_with_parked_sheet(vec![
        symbol(uuid::Uuid::new_v4(), "R1", 0.0),
        symbol(uuid::Uuid::new_v4(), "R1", 10.0),
    ]);

    let _ = app.update(Message::Menu(MenuMessage::AnnotateResetDuplicates));

    let parked_refs = parked_references(&app, &parked_path);
    assert!(
        parked_refs.iter().filter(|r| *r == "R1").count() < 2,
        "the parked engine must no longer hold a duplicate R1, got {parked_refs:?}"
    );
    assert_parked_untouched(
        &app,
        &parked_path,
        &saved_bytes,
        "Reset Duplicate Designators",
    );
}

fn parked_references(app: &Signex, parked_path: &Path) -> Vec<String> {
    app.document_state.engines[parked_path]
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect()
}

fn assert_parked_untouched(app: &Signex, parked_path: &Path, saved_bytes: &[u8], what: &str) {
    assert!(
        app.document_state.dirty_paths.contains(parked_path),
        "the parked sheet still holds unsaved edits and must stay in `dirty_paths`"
    );
    assert_eq!(
        std::fs::read(parked_path).expect("read parked file"),
        saved_bytes,
        "{what} must not write a parked sheet's unsaved edits to disk unasked"
    );
}
