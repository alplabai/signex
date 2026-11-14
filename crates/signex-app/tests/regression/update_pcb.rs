//! #1312 — Update PCB from schematic and New PCB in a project, driven
//! through UI messages: the menu rows, the palette, a key, the Projects tree
//! and the dialog's own buttons, on a project read from disk with a real
//! library whose row binds a two-pad footprint.

use std::path::{Path, PathBuf};

use signex_app::app::{
    CommandPaletteMsg, ContextMenuMsg, FileMsg, LoadedProject, Message, ProjectTreeAction, Signex,
    TabDocument, UiMsg, UpdatePcbMsg, UpdatePcbState, WindowMsg,
};
use signex_app::keymap::{
    AppCommandId, ShortcutBinding, ShortcutBindingAction, ShortcutContext, ShortcutTrigger,
};
use signex_engine::pcb::BoardEngine;
use signex_engine::pcb::eco::{EcoBlock, EcoChange, EcoChangeList, EcoKey, EcoRow};
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::{Footprint, LayerId, Pad, PadKind, PadShape, PrimitiveRef};
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::{PcbBoard, SymbolLink};
use signex_types::project::{PcbDefaults, ProjectData, ProjectDefaults, SheetEntry};
use signex_types::schematic::{
    InstancePath, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    Symbol, Wire,
};
use uuid::Uuid;

use crate::library_place_symbol::{Fixture as Library, fixture as library};
use crate::menu_support::menu_row;

const R1: Uuid = Uuid::from_u128(0x1312_4001);
const R2: Uuid = Uuid::from_u128(0x1312_4002);
const R3: Uuid = Uuid::from_u128(0x1312_4003);
const STRAY: Uuid = Uuid::from_u128(0x1312_40ff);

struct Fixture {
    app: Signex,
    sheet: PathBuf,
    board: PathBuf,
    _lib: Library,
    _tmp: tempfile::TempDir,
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

fn up(app: &mut Signex, msg: UpdatePcbMsg) {
    send(app, Message::from(msg));
}

/// The fixture library's row binds a footprint with pads 1 and 2.
fn bind_two_pad_footprint(lib: &Library) {
    let adapter = LocalGitAdapter::open(&lib.snxlib).expect("open");
    let mut footprint = Footprint::empty("R0603");
    for (number, x) in [("1", -0.75), ("2", 0.75)] {
        footprint.pads.push(Pad {
            number: number.to_string(),
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            size: [0.6, 0.9],
            position: [x, 0.0],
            layers: vec![LayerId::new("F.Cu")],
            ..Pad::default()
        });
    }
    let uuid = footprint.uuid;
    adapter
        .save_footprint(footprint, "seed footprint")
        .expect("save footprint");
    let id = signex_library::RowId(lib.row_id);
    let mut row = adapter.read_row("ICs", id).expect("row");
    row.footprint_ref = Some(PrimitiveRef::new(lib.library_id, uuid));
    adapter
        .update_row("ICs", row, "bind footprint")
        .expect("bind");
}

fn lib_pin(number: &str, x: f64) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn resistor_body() -> LibSymbol {
    LibSymbol {
        id: "R".to_string(),
        reference: String::new(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![lib_pin("1", 0.0), lib_pin("2", 10.0)],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// A resistor from the library row at `x`; `linked` false leaves it with no
/// library row, so it has no footprint to place.
fn part(lib: &Library, uuid: Uuid, reference: &str, x: f64, linked: bool) -> Symbol {
    Symbol {
        uuid,
        lib_id: "R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        position: Point::new(x, 0.0),
        library_id: linked.then_some(lib.library_id),
        row_id: linked.then_some(lib.row_id),
        ..Default::default()
    }
}

/// R1 and R2 joined pin 2 to pin 1 by a wire, plus `extra` parts.
fn sheet(lib: &Library, extra: Vec<Symbol>) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::from_u128(0x1312_4000),
        paper_size: "A4".to_string(),
        symbols: vec![
            part(lib, R1, "R1", 0.0, true),
            part(lib, R2, "R2", 20.0, true),
        ],
        wires: vec![Wire {
            uuid: Uuid::from_u128(0x1312_4010),
            start: Point::new(10.0, 0.0),
            end: Point::new(20.0, 0.0),
            stroke_width: 0.0,
            color: None,
            locked: false,
            unknown_columns: Default::default(),
        }],
        ..Default::default()
    };
    sheet.symbols.extend(extra);
    sheet.lib_symbols.insert("R".to_string(), resistor_body());
    sheet
}

fn add_project(
    app: &mut Signex,
    dir: &Path,
    pcb_file: Option<&str>,
    defaults: ProjectDefaults,
) -> signex_app::app::ProjectId {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("demo.snxprj"),
        data: ProjectData {
            document_id: None,
            description: None,
            name: "demo".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("demo.snxsch".to_string()),
            pcb_file: pcb_file.map(str::to_string),
            sheets: vec![SheetEntry {
                name: "demo.snxsch".to_string(),
                filename: "demo.snxsch".to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            project_defaults: defaults,
            agent_journal: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    for tab in &mut app.document_state.tabs {
        tab.project_id = Some(id);
    }
    id
}

/// The project open with its sheet in front. `board`: a board file the
/// project names, written as given (`newer` marks it from a newer format).
fn fixture_with(
    extra: impl FnOnce(&Library) -> Vec<Symbol>,
    board: Option<(PcbBoard, bool)>,
    defaults: ProjectDefaults,
) -> Fixture {
    let _ = signex_app::diagnostics::init_logging();
    let lib = library();
    bind_two_pad_footprint(&lib);
    let tmp = tempfile::tempdir().unwrap();
    let sheet_path = tmp.path().join("demo.snxsch");
    let board_path = tmp.path().join("demo.snxpcb");
    let text = SnxSchematic::new(sheet(&lib, extra(&lib)))
        .write_string()
        .unwrap();
    std::fs::write(&sheet_path, text).unwrap();
    let names_board = board.is_some();
    if let Some((board, newer)) = board {
        let mut text = SnxPcb::new(board).write_string().unwrap();
        if newer {
            text = text.replacen("snxpcb/1.3", "snxpcb/1.9", 1);
        }
        std::fs::write(&board_path, text).unwrap();
    }
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.library
        .open_library(lib.snxlib.clone())
        .expect("mount library");
    send(
        &mut app,
        Message::File(FileMsg::Opened(Some(sheet_path.clone()))),
    );
    add_project(
        &mut app,
        tmp.path(),
        names_board.then_some("demo.snxpcb"),
        defaults,
    );
    Fixture {
        app,
        sheet: sheet_path,
        board: board_path,
        _lib: lib,
        _tmp: tmp,
    }
}

fn fixture() -> Fixture {
    fixture_with(|_| Vec::new(), None, ProjectDefaults::default())
}

fn state(f: &Fixture) -> &UpdatePcbState {
    f.app
        .ui_state
        .update_pcb
        .as_ref()
        .expect("the Update PCB dialog is open")
}

fn list(f: &Fixture) -> &EcoChangeList {
    let state = state(f);
    assert!(state.is_ready(), "the list is ready: {:?}", state.failure());
    state.list().expect("a change list")
}

fn add_row(f: &Fixture, part: Uuid) -> &EcoRow {
    list(f)
        .row(&EcoKey::AddFootprint {
            sheet: InstancePath::root(),
            part,
        })
        .expect("an add row")
}

fn engine(f: &Fixture) -> &BoardEngine {
    f.app
        .document_state
        .tabs
        .iter()
        .find(|t| t.path == f.board)
        .and_then(|t| t.cached_document.as_ref())
        .and_then(TabDocument::as_pcb_engine)
        .expect("the board is open")
}

fn references(f: &Fixture) -> Vec<String> {
    let mut refs: Vec<String> = engine(f)
        .board()
        .footprints
        .iter()
        .map(|fp| fp.reference.clone())
        .collect();
    refs.sort();
    refs
}

fn ratsnest_lines(f: &Fixture) -> usize {
    f.app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("the board is in view")
        .ratsnest_lines
        .len()
}

fn show(f: &mut Fixture, path: &Path) {
    let index = f
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab");
    let window = f.app.ui_state.main_window_id.expect("main window");
    send(
        &mut f.app,
        Message::Tab {
            window_id: window,
            msg: signex_app::tab_bar::TabMessage::Select(index),
        },
    );
}

/// File ▸ New ▸ PCB, then Design ▸ Update PCB Document from the sheet.
fn new_board_and_list(f: &mut Fixture) {
    send(&mut f.app, menu_row("new_pcb_document"));
    crate::document_setup::create_if_asked(&mut f.app);
    let sheet = f.sheet.clone();
    show(f, &sheet);
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
}

/// A board of `fp` footprints, each a two-pad footprint linked as given.
fn board_of(footprints: Vec<signex_types::pcb::Footprint>) -> PcbBoard {
    PcbBoard {
        uuid: Uuid::from_u128(0x1312_4100),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        sketch: None,
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints,
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn board_footprint(
    uuid: u128,
    reference: &str,
    link: Option<Uuid>,
    locked: bool,
) -> signex_types::pcb::Footprint {
    let pad = |n: u128, number: &str, x: f64| signex_types::pcb::Pad {
        uuid: Uuid::from_u128(uuid * 0x10 + n),
        number: number.to_string(),
        pad_type: signex_types::pcb::PadType::Smd,
        shape: signex_types::pcb::PadShape::Rect,
        position: signex_types::pcb::Point::new(x, 0.0),
        size: signex_types::pcb::Point::new(0.6, 0.9),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: locked,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    signex_types::pcb::Footprint {
        uuid: Uuid::from_u128(uuid),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint_id: "R0603".to_string(),
        position: signex_types::pcb::Point::new(uuid as f64, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked,
        pads: vec![pad(1, "1", -0.75), pad(2, "2", 0.75)],
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: link.map(|unit| SymbolLink::new(InstancePath::root(), [unit])),
        library_ref: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

// ─── The full flow, Execute, undo ────────────────────────────────────────

/// Schematic, then File ▸ New ▸ PCB, then Design ▸ Update PCB Document:
/// every footprint is placed and the net joining R1 and R2 shows as
/// ratsnest; Execute is one undo step.
#[test]
fn a_new_board_gets_every_footprint_and_its_nets_as_ratsnest_in_one_undo_step() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    assert!(add_row(&f, R1).blocked.is_none() && add_row(&f, R1).on);
    assert!(add_row(&f, R2).on);
    assert!(
        list(&f)
            .rows
            .iter()
            .any(|r| matches!(r.change, EcoChange::PadNet { .. })),
        "the pads get their nets"
    );
    assert!(!engine(&f).can_undo(), "a new board has nothing to undo");

    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert!(
        f.app.ui_state.update_pcb.is_none(),
        "Execute closes the dialog"
    );
    assert_eq!(references(&f), ["R1", "R2"]);
    let active = &f.app.document_state.tabs[f.app.document_state.active_tab];
    assert_eq!(active.path, f.board, "the board comes forward");
    assert!(ratsnest_lines(&f) >= 1, "R1.2 to R2.1 is a ratsnest line");
    assert_eq!(engine(&f).undo_len(), 1, "one undo step");

    send(&mut f.app, menu_row("undo"));
    assert!(
        references(&f).is_empty(),
        "one undo takes the whole update back"
    );
    assert!(!engine(&f).can_undo());
}

// ─── Edge cases ──────────────────────────────────────────────────────────

/// A part with no footprint gets a blocked row with its reason; the rest is
/// placed.
#[test]
fn a_missing_footprint_blocks_its_row_and_the_rest_still_runs() {
    let mut f = fixture_with(
        |lib| vec![part(lib, R3, "R3", 40.0, false)],
        None,
        ProjectDefaults::default(),
    );
    new_board_and_list(&mut f);
    let r3 = add_row(&f, R3);
    assert!(matches!(r3.blocked, Some(EcoBlock::MissingFootprint(_))));
    assert!(!r3.on);
    let key = r3.key.clone();
    up(&mut f.app, UpdatePcbMsg::RowToggled { key, on: true });
    assert!(!add_row(&f, R3).on, "a blocked row cannot be ticked");
    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert_eq!(references(&f), ["R1", "R2"]);
}

/// A repeated designator and a `?` one are blocked with an Annotate link,
/// which opens Annotate.
#[test]
fn duplicate_and_unannotated_designators_are_blocked_and_annotate_opens() {
    let mut f = fixture_with(
        |lib| {
            vec![
                part(lib, R3, "R2", 40.0, true),
                part(lib, STRAY, "R?", 60.0, true),
            ]
        },
        None,
        ProjectDefaults::default(),
    );
    new_board_and_list(&mut f);
    assert!(matches!(
        &add_row(&f, R2).blocked,
        Some(EcoBlock::DuplicateDesignator(d)) if d == "R2"
    ));
    assert_eq!(add_row(&f, STRAY).blocked, Some(EcoBlock::NeedsAnnotation));
    assert!(add_row(&f, R1).on, "the rest is still ticked");

    up(&mut f.app, UpdatePcbMsg::AnnotateRequested);
    assert!(f.app.ui_state.update_pcb.is_none());
    assert!(f.app.ui_state.annotate_dialog_open, "Annotate opened");
}

/// A blocked row's Cross Probe link shows its part on the sheet.
#[test]
fn the_cross_probe_link_shows_the_part_on_its_sheet() {
    let mut f = fixture_with(
        |lib| vec![part(lib, STRAY, "R?", 60.0, true)],
        None,
        ProjectDefaults::default(),
    );
    new_board_and_list(&mut f);
    let key = add_row(&f, STRAY).key.clone();
    up(&mut f.app, UpdatePcbMsg::CrossProbeRequested(key));
    assert!(f.app.ui_state.update_pcb.is_none());
    let docs = &f.app.document_state;
    assert_eq!(docs.tabs[docs.active_tab].path, f.sheet);
    assert!(
        f.app
            .interaction_state
            .canvas
            .selected
            .iter()
            .any(|item| item.uuid == STRAY),
        "the part is selected on its sheet"
    );
}

/// A locked footprint's remove row starts unticked, and a pad whose net is
/// locked gets no pad net row.
#[test]
fn locked_footprints_start_unticked_and_net_locked_pads_are_left_alone() {
    let board = board_of(vec![
        board_footprint(0xa1, "R1", Some(R1), true),
        board_footprint(0xa9, "R9", Some(STRAY), true),
    ]);
    let mut f = fixture_with(
        |_| Vec::new(),
        Some((board, false)),
        ProjectDefaults::default(),
    );
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    let remove = list(&f)
        .row(&EcoKey::RemoveFootprint(Uuid::from_u128(0xa9)))
        .expect("R9 is listed for removal");
    assert!(!remove.default_on && !remove.on, "locked: unticked");
    let locked_pad_rows = list(&f).rows.iter().any(|r| {
        matches!(r.change, EcoChange::PadNet { footprint, .. } if footprint == Uuid::from_u128(0xa1))
    });
    assert!(!locked_pad_rows, "net-locked pads keep their nets");
}

/// A board closed with unsaved edits is listed but Execute is off; Reopen
/// gives it its tab back and Execute works.
#[test]
fn a_parked_board_disables_execute_and_reopen_brings_it_back() {
    let mut f = fixture_with(
        |_| Vec::new(),
        Some((board_of(Vec::new()), false)),
        ProjectDefaults::default(),
    );
    send(
        &mut f.app,
        Message::File(FileMsg::Opened(Some(f.board.clone()))),
    );
    let board = f.board.clone();
    f.app.document_state.mark_document_dirty(&board);
    send(
        &mut f.app,
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::CloseAllDocuments,
        )),
    );
    assert!(f.app.document_state.tabs.is_empty(), "every tab closed");
    send(&mut f.app, Message::Ui(UiMsg::GridPickerSelect(1.0)));
    let key = signex_widgets::tree_view::key_of(&f.app.document_state.panel_ctx.project_tree, &[0])
        .expect("the project row");
    send(
        &mut f.app,
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::UpdatePcb(key),
        )),
    );
    assert!(add_row(&f, R1).on);
    let why = f.app.update_pcb_execute_block().expect("Execute is off");
    assert!(why.contains("demo.snxpcb"), "{why}");
    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert!(f.app.ui_state.update_pcb.is_some(), "nothing ran");

    up(&mut f.app, UpdatePcbMsg::ReopenRequested);
    assert!(
        f.app.update_pcb_execute_block().is_none(),
        "the board is editable again"
    );
    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert_eq!(references(&f), ["R1", "R2"]);
}

/// A board from a newer format opens read-only: listed, Execute off with the
/// reason.
#[test]
fn a_read_only_board_disables_execute_with_the_reason() {
    let mut f = fixture_with(
        |_| Vec::new(),
        Some((board_of(Vec::new()), true)),
        ProjectDefaults::default(),
    );
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    assert!(add_row(&f, R1).on);
    let why = f.app.update_pcb_execute_block().expect("Execute is off");
    assert!(why.contains("read-only"), "{why}");
    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert!(references(&f).is_empty(), "nothing was placed");
}

/// A list the board moved past shows the stale banner; Refresh reads again
/// and keeps the user's ticks.
#[test]
fn a_stale_list_is_flagged_and_refresh_keeps_the_ticks() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    let r2 = add_row(&f, R2).key.clone();
    up(
        &mut f.app,
        UpdatePcbMsg::RowToggled {
            key: r2.clone(),
            on: false,
        },
    );
    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert_eq!(references(&f), ["R1"], "only R1 was ticked");

    // From the board: Design ▸ Import Changes From the project.
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    assert!(add_row(&f, R2).on, "R2 is listed again, ticked by default");
    up(
        &mut f.app,
        UpdatePcbMsg::RowToggled {
            key: r2.clone(),
            on: false,
        },
    );
    send(&mut f.app, menu_row("undo"));
    assert!(f.app.update_pcb_stale(), "the board moved past the list");
    let why = f.app.update_pcb_execute_block().expect("Execute is off");
    assert!(!why.is_empty());

    up(&mut f.app, UpdatePcbMsg::RefreshRequested);
    assert!(!f.app.update_pcb_stale());
    assert!(add_row(&f, R1).on, "R1 is to be added again, ticked");
    assert!(!add_row(&f, R2).on, "the user's untick survives Refresh");
}

/// With no board in the project the dialog offers Create PCB, which makes
/// the board and reads the list for it.
#[test]
fn no_board_offers_create_pcb() {
    let mut f = fixture();
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    assert!(state(&f).lacks_board());
    up(&mut f.app, UpdatePcbMsg::CreatePcbRequested);
    crate::document_setup::create_if_asked(&mut f.app);
    assert_eq!(state(&f).board(), Some(f.board.as_path()));
    assert!(add_row(&f, R1).on);
}

/// Validate runs the ticked rows on a copy and changes nothing.
#[test]
fn validate_runs_on_a_copy() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    up(&mut f.app, UpdatePcbMsg::ValidateRequested);
    assert_eq!(state(&f).validation(), Some(&Ok(())));
    assert!(references(&f).is_empty());
}

/// The group box ticks or unticks every row of its group.
#[test]
fn the_group_box_unticks_its_whole_group() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    up(
        &mut f.app,
        UpdatePcbMsg::GroupToggled {
            group: signex_engine::pcb::eco::EcoGroup::AddFootprints,
            on: false,
        },
    );
    assert!(!add_row(&f, R1).on && !add_row(&f, R2).on);
}

/// Report... writes the list as text where the user saved it.
#[test]
fn the_report_is_written_where_the_user_saves_it() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    let path = f.sheet.with_file_name("report.txt");
    up(
        &mut f.app,
        UpdatePcbMsg::ReportPathPicked(Some(path.clone())),
    );
    let text = std::fs::read_to_string(&path).expect("the report");
    assert!(text.contains("Add R1"), "{text}");
}

// ─── New PCB ─────────────────────────────────────────────────────────────

/// The new board takes the project defaults and nothing reaches the disk
/// until a save.
#[test]
fn a_new_pcb_takes_the_project_defaults_and_is_not_written() {
    let defaults = ProjectDefaults {
        pcb: Some(PcbDefaults {
            copper_layers: 6,
            thickness_mm: 1.2,
            clearance_mm: 0.1,
            track_width_mm: 0.1,
            via_diameter_mm: 0.3,
            via_drill_mm: 0.15,
        }),
        sheet: None,
        profile: None,
    };
    let mut f = fixture_with(|_| Vec::new(), None, defaults);
    send(&mut f.app, menu_row("new_pcb_document"));
    crate::document_setup::create_if_asked(&mut f.app);
    let board = engine(&f).board();
    let copper = board
        .layers
        .iter()
        .filter(|l| l.layer_type == "copper")
        .count();
    assert_eq!(copper, 6);
    assert_eq!(board.thickness, 1.2);
    assert_eq!(board.setup.as_ref().map(|s| s.clearance), Some(0.1));
    assert!(!f.board.exists(), "nothing is written until Save");
    let docs = &f.app.document_state;
    assert!(docs.is_document_dirty(&f.board), "the board is unsaved");
    assert_eq!(
        docs.projects[0].data.pcb_file.as_deref(),
        Some("demo.snxpcb")
    );
    assert!(
        docs.is_document_dirty(&docs.projects[0].path),
        "the project names it, unsaved"
    );
}

/// A project holds one board: a second New PCB is refused.
#[test]
fn a_second_new_pcb_is_refused() {
    let mut f = fixture();
    send(&mut f.app, menu_row("new_pcb_document"));
    crate::document_setup::create_if_asked(&mut f.app);
    let tabs = f.app.document_state.tabs.len();
    send(&mut f.app, menu_row("new_pcb_document"));
    crate::document_setup::create_if_asked(&mut f.app);
    assert_eq!(f.app.document_state.tabs.len(), tabs);
    assert!(f.app.document_state.error_notice.is_some(), "it says why");
}

// ─── Entry points ────────────────────────────────────────────────────────

/// Projects ▸ Add New ▸ PCB on the project row.
#[test]
fn projects_add_new_pcb_makes_the_board() {
    let mut f = fixture();
    send(&mut f.app, Message::Ui(UiMsg::GridPickerSelect(1.0)));
    let key = signex_widgets::tree_view::key_of(&f.app.document_state.panel_ctx.project_tree, &[0])
        .expect("the project row");
    send(
        &mut f.app,
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::AddNewPcb(key),
        )),
    );
    crate::document_setup::create_if_asked(&mut f.app);
    assert!(engine(&f).board().footprints.is_empty());
}

/// The command palette opens the dialog.
#[test]
fn the_palette_opens_update_pcb() {
    let mut f = fixture();
    send(&mut f.app, Message::CommandPalette(CommandPaletteMsg::Open));
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged(
            "Update PCB from schematic".into(),
        )),
    );
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );
    assert!(state(&f).lacks_board());
}

/// A key a profile binds to the command opens the dialog.
#[test]
fn a_bound_key_opens_update_pcb() {
    let mut f = fixture();
    let ShortcutTrigger::KeySequence(mut strokes) = ShortcutTrigger::parse("Ctrl+Alt+F11").unwrap()
    else {
        panic!("a key");
    };
    let profiles = &mut f.app.ui_state.keymap_profiles;
    profiles
        .active_profile_mut()
        .bindings
        .push(ShortcutBinding {
            action: ShortcutBindingAction::Command(
                AppCommandId::new("update_pcb_from_schematic").unwrap(),
            ),
            context: ShortcutContext::Global,
            triggers: vec![ShortcutTrigger::KeySequence(strokes.clone())],
            platform: None,
        });
    f.app.ui_state.active_keymap = f.app.ui_state.keymap_profiles.compile_active().into();
    send(
        &mut f.app,
        Message::Ui(UiMsg::KeymapStroke {
            window: None,
            stroke: strokes.remove(0),
        }),
    );
    assert!(state(&f).lacks_board());
}

/// An undocked sheet window's own menu bar runs the command for the sheet
/// that window shows.
#[test]
fn an_undocked_windows_menu_bar_opens_update_pcb() {
    let mut f = fixture();
    let index = f
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == f.sheet)
        .expect("the sheet tab");
    let before: std::collections::HashSet<iced::window::Id> =
        f.app.ui_state.windows.keys().copied().collect();
    let _ = f.app.update(Message::Window(WindowMsg::UndockTab(index)));
    let window = *f
        .app
        .ui_state
        .windows
        .keys()
        .find(|id| !before.contains(id))
        .expect("an undocked window");
    let _ = f.app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: f.sheet.clone(),
        id: window,
    }));
    send(
        &mut f.app,
        Message::Ui(UiMsg::MenuInWindow {
            window,
            msg: signex_app::menu_bar::MenuMessage::Command(
                AppCommandId::new("update_pcb_from_schematic").unwrap(),
            ),
        }),
    );
    assert!(
        state(&f).lacks_board(),
        "the window's sheet names the project"
    );
}

/// Esc closes the dialog.
#[test]
fn esc_closes_the_dialog() {
    let mut f = fixture();
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    assert!(f.app.ui_state.update_pcb.is_some());
    send(
        &mut f.app,
        Message::Input(signex_app::app::InputMsg::EscapePressed { window: None }),
    );
    assert!(f.app.ui_state.update_pcb.is_none());
}

#[path = "update_pcb_review.rs"]
mod review;

#[path = "update_pcb_unresolved.rs"]
mod unresolved;
