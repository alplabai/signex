//! #2367 and #2365: what the board Component view reads from the schematic
//! (Design Item ID, Source, the schematic designator and a mismatch with the
//! board's), and the component Type leaving a part out of the BOM. A project
//! with one sheet, its board and an open library, driven through
//! `Signex::update`.

use std::collections::HashMap;
use std::path::Path;

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{FileMsg, LoadedProject, Message, Signex};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::library::state::{LibraryDisplaySettings, OpenLibrary};
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::{
    PanelMsg, PartIdentity, PcbComponentFacts, PcbComponentMsg, PcbFootprintTextMsg, Shared,
};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_engine::pcb::component_edit::ComponentSetting;
use signex_engine::pcb::designators::FootprintTextField;
use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PlmReserved, PrimitiveRef,
};
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::{
    ComponentType, Footprint, Pad, PadShape, PadType, PcbBoard, Point, SymbolLink,
};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

const R1: Uuid = Uuid::from_u128(0x2367_0001);
const R2: Uuid = Uuid::from_u128(0x2367_0002);
const FP_R1: Uuid = Uuid::from_u128(0x2367_0011);
const FP_R2: Uuid = Uuid::from_u128(0x2367_0012);
const LIBRARY: Uuid = Uuid::from_u128(0x2367_00a0);
const ROW: Uuid = Uuid::from_u128(0x2367_00b0);

/// R1, placed from the library row ROW, and R2, from no library.
fn sheet() -> SchematicSheet {
    let symbol = |uuid, reference: &str, lib_id: &str| Symbol {
        uuid,
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: "R_0603".to_string(),
        ..Default::default()
    };
    SchematicSheet {
        uuid: Uuid::from_u128(0x2367_0000),
        paper_size: "A4".to_string(),
        symbols: vec![
            Symbol {
                library_id: Some(LIBRARY),
                row_id: Some(ROW),
                ..symbol(R1, "R1", "Device:R")
            },
            Symbol {
                position: signex_types::schematic::Point::new(40.0, 20.0),
                ..symbol(R2, "R2", "Device:R")
            },
        ],
        ..Default::default()
    }
}

fn footprint(uuid: Uuid, symbol: Uuid, reference: &str, x: f64) -> Footprint {
    let pad = |n: u128, dx: f64| Pad {
        uuid: Uuid::from_u128(uuid.as_u128() * 16 + n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(dx, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    Footprint {
        uuid,
        reference: reference.into(),
        value: "10k".into(),
        footprint_id: "R_0603".into(),
        position: Point::new(x, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: vec![pad(1, -2.0), pad(2, 2.0)],
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: Some(SymbolLink::new(InstancePath::root(), [symbol])),
        library_ref: None,
        component: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![
            footprint(FP_R1, R1, "R1", 10.0),
            footprint(FP_R2, R2, "R2", 20.0),
        ],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        stackup: None,
        rooms: Vec::new(),
        free: Default::default(),
        options: Default::default(),
        unknown: Default::default(),
    }
}

fn row() -> ComponentRow {
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: ROW,
        internal_pn: InternalPn::new("RES-10K-0603"),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::from_u128(1)),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Yageo", "RC0603"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

fn open_library(app: &mut Signex) {
    app.library.open_libraries.push(OpenLibrary {
        root: "passives.snxlib".into(),
        display_name: "Passives".into(),
        library_id: LIBRARY,
        tables: HashMap::from([("resistors".to_string(), vec![row()])]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
        dangling: Default::default(),
        pricing: Default::default(),
        provenance: Default::default(),
    });
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
            pcb_file: Some("demo.snxpcb".to_string()),
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
            variant_settings: Default::default(),
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
            bom_alternates: Vec::new(),
            requirements: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    for tab in &mut app.document_state.tabs {
        tab.project_id = Some(id);
    }
}

struct Fixture {
    app: Signex,
    board: std::path::PathBuf,
    sheet: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

/// The sheet and the board open in one project, the board in view; the
/// sheet only when `with_sheet`.
fn fixture(with_sheet: bool) -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let sheet = tmp.path().join("demo.snxsch");
    let board_path = tmp.path().join("demo.snxpcb");
    std::fs::write(
        &sheet,
        SnxSchematic::new(self::sheet()).write_string().unwrap(),
    )
    .unwrap();
    std::fs::write(&board_path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    open_library(&mut app);
    if with_sheet {
        send(
            &mut app,
            Message::File(FileMsg::Opened(Some(sheet.clone()))),
        );
    }
    send(
        &mut app,
        Message::File(FileMsg::Opened(Some(board_path.clone()))),
    );
    add_project(&mut app, tmp.path());
    Fixture {
        app,
        board: board_path,
        sheet,
        _tmp: tmp,
    }
}

fn select(app: &mut Signex, x: f64) {
    send(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(x, 10.0),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
    let selected: Vec<ItemRef> = app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(selected.len(), 1, "{selected:?}");
}

fn facts(app: &Signex) -> PcbComponentFacts {
    app.document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.component.clone())
        .expect("the Component view")
}

#[test]
fn a_placed_part_shows_its_design_item_id_source_and_schematic_designator() {
    let mut f = fixture(true);
    select(&mut f.app, 10.0);
    let shown = facts(&f.app);
    assert_eq!(
        shown.identity,
        Shared::Same(PartIdentity::Read {
            designator: "R1".into(),
            design_item_id: Some("RES-10K-0603".into()),
            source: Some("Passives".into()),
        })
    );
    assert_eq!(shown.designator_mismatches, 0);
}

#[test]
fn a_part_from_no_library_shows_its_symbol_and_no_source() {
    let mut f = fixture(true);
    select(&mut f.app, 20.0);
    assert_eq!(
        facts(&f.app).identity,
        Shared::Same(PartIdentity::Read {
            designator: "R2".into(),
            design_item_id: Some("Device:R".into()),
            source: None,
        })
    );
}

#[test]
fn without_the_sheet_open_the_part_is_not_read() {
    let mut f = fixture(false);
    select(&mut f.app, 10.0);
    assert_eq!(facts(&f.app).identity, Shared::Same(PartIdentity::NotRead));
}

#[test]
fn a_renamed_board_designator_shows_the_mismatch() {
    let mut f = fixture(true);
    select(&mut f.app, 10.0);
    for message in [
        PcbFootprintTextMsg::Typed {
            board: f.board.clone(),
            footprint: FP_R1,
            field: FootprintTextField::Designator,
            text: "R5".into(),
        },
        PcbFootprintTextMsg::Submitted {
            board: f.board.clone(),
            footprint: FP_R1,
            field: FootprintTextField::Designator,
        },
    ] {
        send(
            &mut f.app,
            Message::Dock(DockMessage::Panel(PanelMsg::PcbFootprintText(message))),
        );
    }
    let shown = facts(&f.app);
    assert_eq!(shown.designator, Shared::Same("R5".to_string()));
    assert_eq!(shown.designator_mismatches, 1);
}

/// The mismatch note's Update PCB button (Graphite mockup row 94) runs Design ▸
/// Update PCB Document: the Update PCB dialog opens for the project, and
/// nothing changes on the board until it is executed.
#[test]
fn the_update_pcb_button_of_a_mismatch_opens_update_pcb() {
    let mut f = fixture(true);
    select(&mut f.app, 10.0);
    for message in [
        PcbFootprintTextMsg::Typed {
            board: f.board.clone(),
            footprint: FP_R1,
            field: FootprintTextField::Designator,
            text: "R5".into(),
        },
        PcbFootprintTextMsg::Submitted {
            board: f.board.clone(),
            footprint: FP_R1,
            field: FootprintTextField::Designator,
        },
    ] {
        send(
            &mut f.app,
            Message::Dock(DockMessage::Panel(PanelMsg::PcbFootprintText(message))),
        );
    }
    assert_eq!(facts(&f.app).designator_mismatches, 1);
    assert!(f.app.ui_state.update_pcb.is_none());
    // Update PCB runs for the open project; the fixture adds the project
    // without opening it, so make it the open one as opening it would.
    f.app.document_state.active_project = Some(f.app.document_state.projects[0].id);
    send(
        &mut f.app,
        Message::Dock(DockMessage::Panel(PanelMsg::PcbComponent(
            PcbComponentMsg::UpdatePcbPressed,
        ))),
    );
    assert!(
        f.app.ui_state.update_pcb.is_some(),
        "the Update PCB dialog is open"
    );
    assert_eq!(
        facts(&f.app).designator,
        Shared::Same("R5".to_string()),
        "the board keeps its designator until Update PCB runs"
    );
}

#[test]
fn a_mechanical_type_on_the_board_leaves_the_part_out_of_the_bom() {
    let mut f = fixture(true);
    select(&mut f.app, 20.0);
    send(
        &mut f.app,
        Message::Dock(DockMessage::Panel(PanelMsg::PcbComponent(
            PcbComponentMsg::SettingPicked {
                board: f.board.clone(),
                footprints: vec![FP_R2],
                setting: ComponentSetting::Type(ComponentType::Mechanical),
            },
        ))),
    );
    let index = f
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == f.sheet)
        .expect("the sheet's tab");
    f.app.document_state.active_tab = index;
    f.app.document_state.active_path = Some(f.sheet.clone());
    send(&mut f.app, Message::Menu(MenuMessage::ExportBom));
    let preview = f
        .app
        .document_state
        .bom_preview
        .as_ref()
        .expect("the BOM preview");
    let listed: Vec<&str> = preview
        .table
        .rows
        .iter()
        .flat_map(|row| row.references.iter().map(String::as_str))
        .collect();
    assert_eq!(listed, ["R1"], "R2 is mechanical on the board");
}
