//! #1836 — project ERC reports the hierarchy mistakes a sheet symbol can hide:
//! a sheet pin with nothing wired to it, a child port the parent does not
//! expose, and two sheet pins driving one net.
//!
//! Real temp dir: `root.snxsch` (open, active) holds the sheet symbols;
//! `child.snxsch` is on disk only.

use signex_app::app::{ErcMsg, Message, Signex};
use signex_erc::Violation;
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Label, LabelType, Point, SchematicSheet, SheetPin, VAlign, Wire,
};
use tempfile::TempDir;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn label(text: &str, at: Point, label_type: LabelType, shape: &str) -> Label {
    Label {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        text: text.to_string(),
        position: at,
        rotation: 0.0,
        label_type,
        shape: shape.to_string(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
    }
}

fn wire(a: Point, b: Point) -> Wire {
    Wire {
        start: a,
        end: b,
        ..Default::default()
    }
}

/// A sheet symbol at `(x, 0)`, 10 mm square, with one pin per `(name, direction)`
/// on its left edge, `pin_y` apart from 2.54 mm down.
fn symbol(name: &str, x: f64, pins: &[(&str, &str)]) -> ChildSheet {
    ChildSheet {
        uuid: uuid::Uuid::now_v7(),
        name: name.to_string(),
        filename: "child.snxsch".to_string(),
        position: pt(x, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: pins
            .iter()
            .enumerate()
            .map(|(i, (pin, direction))| SheetPin {
                uuid: uuid::Uuid::now_v7(),
                name: (*pin).to_string(),
                direction: (*direction).to_string(),
                position: pt(x, 2.54 * (i as f64 + 1.0)),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            })
            .collect(),
        instances: Vec::new(),
    }
}

/// A child sheet with a wired Hierarchical port per name.
fn child_with_ports(ports: &[&str]) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    for (i, port) in ports.iter().enumerate() {
        let y = 2.54 * (i as f64 + 1.0);
        sheet.wires.push(wire(pt(0.0, y), pt(5.0, y)));
        sheet
            .labels
            .push(label(port, pt(0.0, y), LabelType::Hierarchical, "input"));
    }
    sheet
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

/// Which sheet is the active tab when ERC runs.
#[derive(Clone, Copy)]
enum Active {
    Root,
    Child,
}

/// Write `child` and `root` into a saved project, open both as tabs of that
/// project, run ERC with `active` focused and return every finding filed
/// under any sheet.
fn run_erc(root: SchematicSheet, child: SchematicSheet, active: Active) -> Vec<Violation> {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let (root_path, child_path) = (dir.join("root.snxsch"), dir.join("child.snxsch"));
    write_sheet(&child_path, child.clone());
    write_sheet(&root_path, root.clone());
    let data = signex_types::project::ProjectData {
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("root.snxsch".to_string()),
        pcb_file: None,
        sheets: vec![entry("root.snxsch"), entry("child.snxsch")],
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
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
    for (title, path, sheet) in [("root", &root_path, root), ("child", &child_path, child)] {
        let engine =
            signex_engine::Engine::new_with_path(sheet, Some(path.clone())).expect("engine");
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(signex_app::app::TabInfo {
            title: title.to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: Some(project),
            kind: signex_app::app::TabKind::Schematic,
        });
    }
    let (tab, path) = match active {
        Active::Root => (0, root_path),
        Active::Child => (1, child_path),
    };
    app.document_state.active_tab = tab;
    app.document_state.active_path = Some(path);

    crate::settle_support::settle(&mut app, Message::Erc(ErcMsg::Run));
    app.ui_state
        .erc_violations_by_path
        .values()
        .flatten()
        .cloned()
        .collect()
}

/// A root whose sheet symbol pin `IN` is wired to a labelled stub.
fn root_with_wired_pin(extra: impl FnOnce(&mut SchematicSheet)) -> SchematicSheet {
    let mut root = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    root.child_sheets
        .push(symbol("ch", 20.0, &[("IN", "input")]));
    root.wires.push(wire(pt(20.0, 2.54), pt(10.0, 2.54)));
    root.labels
        .push(label("SIG", pt(10.0, 2.54), LabelType::Net, ""));
    extra(&mut root);
    root
}

const UNCONNECTED: &str = "builtin::sheet_pin_not_connected";
const DIRECTION: &str = "builtin::sheet_pin_direction_conflict";

fn by_id<'a>(found: &'a [Violation], rule_id: &str) -> Vec<&'a Violation> {
    found
        .iter()
        .filter(|v| v.rule_id.as_str() == rule_id)
        .collect()
}

#[test]
fn a_sheet_pin_with_its_wire_deleted_is_reported_as_unconnected() {
    let found = run_erc(
        root_with_wired_pin(|root| root.wires.clear()),
        child_with_ports(&["IN"]),
        Active::Root,
    );

    let hits = by_id(&found, UNCONNECTED);
    assert_eq!(hits.len(), 1, "{found:?}");
    assert!(hits[0].message.contains("'IN'"), "{}", hits[0].message);
    assert_eq!(hits[0].severity, signex_erc::Severity::Warning);
}

#[test]
fn a_wired_sheet_pin_is_not_reported() {
    let found = run_erc(
        root_with_wired_pin(|_| {}),
        child_with_ports(&["IN"]),
        Active::Root,
    );

    assert!(by_id(&found, UNCONNECTED).is_empty(), "{found:?}");
    assert!(by_id(&found, DIRECTION).is_empty(), "{found:?}");
}

#[test]
fn a_child_port_the_parent_does_not_expose_is_reported_per_sheet_symbol() {
    let mut root = root_with_wired_pin(|_| {});
    root.child_sheets
        .push(symbol("ch2", 40.0, &[("IN", "input")]));
    root.wires.push(wire(pt(40.0, 2.54), pt(35.0, 2.54)));

    // The child's label was renamed IN -> INX; each parent pin IN is now bad too.
    let found = run_erc(root, child_with_ports(&["INX"]), Active::Child);

    let unexposed: Vec<&Violation> = found
        .iter()
        .filter(|v| v.message.contains("is not exposed as a sheet pin"))
        .collect();
    assert_eq!(unexposed.len(), 2, "{found:?}");
    for symbol in ["'ch'", "'ch2'"] {
        assert!(
            unexposed.iter().any(|v| v.message.contains(symbol)),
            "a finding names sheet symbol {symbol}: {unexposed:?}"
        );
    }
    assert!(
        unexposed
            .iter()
            .all(|v| v.location_space == signex_erc::LocationSpace::ThisSheet),
        "refiled under the child sheet, in its own coordinates"
    );
}

#[test]
fn two_output_sheet_pins_on_one_net_are_reported() {
    let mut root = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    root.child_sheets
        .push(symbol("a", 20.0, &[("OUT", "output")]));
    root.child_sheets
        .push(symbol("b", 40.0, &[("OUT", "output")]));
    // One wire along y = 2.54 from symbol a's pin to symbol b's pin.
    root.wires.push(wire(pt(20.0, 2.54), pt(40.0, 2.54)));

    let found = run_erc(root, child_with_ports(&["OUT"]), Active::Root);

    let hits = by_id(&found, DIRECTION);
    assert_eq!(hits.len(), 1, "{found:?}");
    assert_eq!(hits[0].severity, signex_erc::Severity::Warning);
}
