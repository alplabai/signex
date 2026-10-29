//! #1042 — the canvas draws a net's class style from a snapshot the app
//! refreshes after each update, not from the project: assigning a net to a
//! class (or undoing it) changes what the canvas is handed, with no IO.

use signex_app::app::{LoadedProject, Message, Signex, UiMsg};
use signex_app::dock::DockMessage;
use signex_app::panels::{NetClassMsg, PanelMsg};
use signex_types::net_class::{NetClass, NetLineStyle};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{
    HAlign, Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point,
    SchematicSheet, Symbol, VAlign, Wire,
};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

const WIRE_ID: Uuid = Uuid::from_u128(0x1042);

fn one_pin_part() -> LibSymbol {
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
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// A labelled `VCC` wire that ends on a pin, so the netlist forms the net.
fn vcc_sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        ..Default::default()
    };
    sheet.lib_symbols.insert("R".to_string(), one_pin_part());
    sheet.symbols.push(Symbol {
        uuid: Uuid::from_u128(2),
        lib_id: "R".to_string(),
        reference: "R1".to_string(),
        position: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet.wires.push(Wire {
        uuid: WIRE_ID,
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet.labels.push(Label {
        uuid: Uuid::from_u128(3),
        text: "VCC".to_string(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
    });
    sheet
}

fn fixture() -> Signex {
    let dir = std::env::temp_dir().join("signex-net-class-rendering-1042");
    let sheet_path: PathBuf = dir.join("X.snxsch");
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    let mut power = NetClass::named("Power");
    power.style.line = NetLineStyle::Dashed;
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: ProjectData {
            name: "P".to_string(),
            description: None,
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: "X".to_string(),
                filename: "X.snxsch".to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: vec![power],
            rules: None,
            annotation: None,
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    let mut engine = signex_engine::Engine::new(vcc_sheet()).expect("engine");
    engine.set_path(Some(sheet_path.clone()));
    app.document_state.engines.insert(sheet_path, engine);
    app
}

fn send(app: &mut Signex, msg: NetClassMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::NetClass(msg))));
    // The refresh runs on a worker; here, run its half to completion.
    app.settle_net_class_styles_blocking();
}

#[test]
fn assigning_a_net_to_a_class_styles_its_wire_and_undo_unstyles_it() {
    let mut app = fixture();
    let project = app.document_state.active_project.expect("active project");
    assert!(app.ui_state.net_class_styles.styles().is_empty());

    send(
        &mut app,
        NetClassMsg::AssignNet {
            project,
            net: "VCC".into(),
            class: Some("Power".into()),
        },
    );
    let style = app.ui_state.net_class_styles.styles().get(&WIRE_ID);
    assert_eq!(style.map(|s| s.line), Some(NetLineStyle::Dashed));

    send(&mut app, NetClassMsg::Undo);
    assert!(app.ui_state.net_class_styles.styles().is_empty());
}

#[test]
fn a_net_outside_every_class_is_left_unstyled() {
    let mut app = fixture();
    let project = app.document_state.active_project.expect("active project");

    send(
        &mut app,
        NetClassMsg::AssignNet {
            project,
            net: "SDA".into(),
            class: Some("Power".into()),
        },
    );

    assert!(app.ui_state.net_class_styles.styles().is_empty());
}

/// `send` settles inline and so never goes through the task and its
/// `NetClassStylesResolved` arm; this does, with the answer the production
/// path can hand back when the worker task dies.
#[test]
fn a_failed_background_resolution_keeps_what_is_drawn_and_is_asked_again() {
    let mut app = fixture();
    let project = app.document_state.active_project.expect("active project");
    send(
        &mut app,
        NetClassMsg::AssignNet {
            project,
            net: "VCC".into(),
            class: Some("Power".into()),
        },
    );
    assert!(
        app.ui_state
            .net_class_styles
            .styles()
            .contains_key(&WIRE_ID)
    );

    let failed = app
        .net_class_failure_of_current_request(project, "the net-class style task failed")
        .expect("a request was made");
    let _ = app.update(Message::Ui(UiMsg::NetClassStylesResolved(
        std::sync::Arc::new(Err(failed)),
    )));
    assert!(
        app.ui_state
            .net_class_styles
            .styles()
            .contains_key(&WIRE_ID),
        "the styles already drawn stay"
    );

    // The next edit of the class table plans the work again and lands.
    send(&mut app, NetClassMsg::Undo);
    assert!(app.ui_state.net_class_styles.styles().is_empty());
}
