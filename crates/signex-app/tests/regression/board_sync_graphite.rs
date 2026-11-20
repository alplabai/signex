//! #2533 — the Graphite Update PCB dialog, driven through UI messages: a
//! part with no footprint is lifted into the Problems card, stays in its
//! group unticked, and Execute places every other part on the board.

use std::path::{Path, PathBuf};

use signex_app::app::{
    FileMsg, LoadedProject, Message, RowFilter, Signex, TabDocument, UpdatePcbMsg, UpdatePcbState,
};
use signex_engine::pcb::eco::{EcoBlock, EcoKey};
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::{Footprint, LayerId, Pad, PadKind, PadShape, PrimitiveRef};
use signex_types::format::SnxSchematic;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{
    InstancePath, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet,
    Symbol,
};
use uuid::Uuid;

use crate::library_place_symbol::{Fixture as Library, fixture as library};
use crate::menu_support::menu_row;

const R1: Uuid = Uuid::from_u128(0x2533_0001);
const R2: Uuid = Uuid::from_u128(0x2533_0002);
const R3: Uuid = Uuid::from_u128(0x2533_0003);

struct Rig {
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

/// The library row binds a footprint with pads 1 and 2.
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

/// A resistor at `x`; `linked` false leaves it without a library row, so it
/// has no footprint to place.
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

fn add_project(app: &mut Signex, dir: &Path) {
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
            pcb_file: None,
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
            project_defaults: Default::default(),
            agent_journal: Default::default(),
            fab_outputs: None,
            requirements: Default::default(),
            parameters: Default::default(),
            schematic_design: Default::default(),
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
}

/// R1 and R2 from the library row, R3 with no row (no footprint), in a
/// project with no board yet.
fn rig() -> Rig {
    let _ = signex_app::diagnostics::init_logging();
    let lib = library();
    bind_two_pad_footprint(&lib);
    let tmp = tempfile::tempdir().unwrap();
    let sheet_path = tmp.path().join("demo.snxsch");
    let mut sheet = SchematicSheet {
        uuid: Uuid::from_u128(0x2533_0000),
        paper_size: "A4".to_string(),
        symbols: vec![
            part(&lib, R1, "R1", 0.0, true),
            part(&lib, R2, "R2", 20.0, true),
            part(&lib, R3, "R3", 40.0, false),
        ],
        ..Default::default()
    };
    sheet.lib_symbols.insert("R".to_string(), resistor_body());
    std::fs::write(
        &sheet_path,
        SnxSchematic::new(sheet).write_string().unwrap(),
    )
    .unwrap();
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
    add_project(&mut app, tmp.path());
    Rig {
        app,
        sheet: sheet_path,
        board: tmp.path().join("demo.snxpcb"),
        _lib: lib,
        _tmp: tmp,
    }
}

fn state(rig: &Rig) -> &UpdatePcbState {
    rig.app
        .ui_state
        .update_pcb
        .as_ref()
        .expect("the Update PCB dialog is open")
}

fn add_key(part: Uuid) -> EcoKey {
    EcoKey::AddFootprint {
        sheet: InstancePath::root(),
        part,
    }
}

/// File ▸ New ▸ PCB, then Design ▸ Update PCB Document from the sheet.
fn open_update_pcb(rig: &mut Rig) {
    send(&mut rig.app, menu_row("new_pcb_document"));
    crate::document_setup::create_if_asked(&mut rig.app);
    let index = rig
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == rig.sheet)
        .expect("the sheet tab");
    let window = rig.app.ui_state.main_window_id.expect("main window");
    send(
        &mut rig.app,
        Message::Tab {
            window_id: window,
            msg: signex_app::tab_bar::TabMessage::Select(index),
        },
    );
    send(&mut rig.app, menu_row("update_pcb_from_schematic"));
}

fn board_references(rig: &Rig) -> Vec<String> {
    let mut refs: Vec<String> = rig
        .app
        .document_state
        .tabs
        .iter()
        .find(|t| t.path == rig.board)
        .and_then(|t| t.cached_document.as_ref())
        .and_then(TabDocument::as_pcb_engine)
        .expect("the board is open")
        .board()
        .footprints
        .iter()
        .map(|fp| fp.reference.clone())
        .collect();
    refs.sort();
    refs
}

#[test]
fn a_blocked_row_is_lifted_to_the_problems_card_and_the_board_gets_the_rest() {
    let mut rig = rig();
    open_update_pcb(&mut rig);
    let s = state(&rig);
    assert!(s.is_ready(), "the list is ready: {:?}", s.failure());
    let list = s.list().expect("a change list");
    let r3 = list.row(&add_key(R3)).expect("R3 is listed");
    assert!(
        matches!(r3.blocked, Some(EcoBlock::MissingFootprint(_))),
        "{:?}",
        r3.blocked
    );
    assert!(!r3.on, "the blocked row stays unticked in its group");
    let lifted: Vec<EcoKey> = s
        .problem_rows()
        .into_iter()
        .map(|r| r.key.clone())
        .collect();
    assert_eq!(lifted, [add_key(R3)], "only R3 is in the Problems card");

    // The Problems tile lists only R3; Clear filter lists every row again.
    up(
        &mut rig.app,
        UpdatePcbMsg::FilterChipClicked(RowFilter::Problems),
    );
    let shown: Vec<EcoKey> = state(&rig)
        .shown_rows()
        .into_iter()
        .map(|r| r.key.clone())
        .collect();
    assert_eq!(shown, [add_key(R3)]);
    up(&mut rig.app, UpdatePcbMsg::FilterCleared);
    let all = state(&rig).list().map_or(0, |l| l.rows.len());
    assert_eq!(state(&rig).shown_rows().len(), all);

    // The filter text narrows to a designator.
    up(&mut rig.app, UpdatePcbMsg::FilterQueryChanged("r2".into()));
    let named: Vec<EcoKey> = state(&rig)
        .shown_rows()
        .into_iter()
        .map(|r| r.key.clone())
        .collect();
    assert!(named.contains(&add_key(R2)), "{named:?}");
    assert!(
        !named.contains(&add_key(R1)) && !named.contains(&add_key(R3)),
        "{named:?}"
    );
    up(&mut rig.app, UpdatePcbMsg::FilterCleared);

    up(&mut rig.app, UpdatePcbMsg::ExecuteRequested);
    assert!(rig.app.ui_state.update_pcb.is_none(), "Execute closes it");
    assert_eq!(
        board_references(&rig),
        ["R1", "R2"],
        "every part but the blocked R3 is on the board"
    );
}
