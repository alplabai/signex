//! `change_blocker`: which sheets it must see before a part is changed.

use std::path::PathBuf;

use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{
    ChildSheet, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol,
};
use uuid::Uuid;

use crate::app::state::{CaseFold, LoadedProject, Signex};
use crate::library::messages::PlacedPartTarget;

fn two_unit_lib() -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "L:S" })).unwrap();
    for unit in [1, 2] {
        lib.pins.push(LibPin {
            unit,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 2.54,
                name: unit.to_string(),
                number: unit.to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        });
    }
    lib
}

fn child(filename: &str) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::new_v4(),
        name: "C".into(),
        filename: filename.into(),
        position: Point::new(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: Default::default(),
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

/// An app with `/p/root.snxsch` (U1 unit 1 of a two-unit drawing, one child
/// reference `reference`) in project `/p` under `fold`, plus the empty sheet
/// `/p/sheet-two.snxsch` open.
fn app_with_child_reference(reference: &str, fold: CaseFold) -> (Signex, PlacedPartTarget, Symbol) {
    let (mut app, _task) = Signex::new();
    let root = PathBuf::from("/p/root.snxsch");
    let symbol = Symbol {
        uuid: Uuid::from_u128(1),
        lib_id: "L:S".into(),
        reference: "U1".into(),
        unit: 1,
        ..Default::default()
    };
    let mut sheet = SchematicSheet {
        uuid: Uuid::new_v4(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    sheet.lib_symbols.insert("L:S".into(), two_unit_lib());
    sheet.symbols.push(symbol.clone());
    sheet.child_sheets.push(child(reference));
    let engine = signex_engine::Engine::new(sheet).unwrap();
    let target = PlacedPartTarget {
        sheet: root.clone(),
        engine_instance: engine.instance_id(),
        symbol_id: symbol.uuid,
    };
    app.document_state.engines.insert(root.clone(), engine);
    let other = SchematicSheet {
        uuid: Uuid::new_v4(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    app.document_state.engines.insert(
        PathBuf::from("/p/sheet-two.snxsch"),
        signex_engine::Engine::new(other).unwrap(),
    );
    app.document_state.active_path = Some(root);
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: PathBuf::from("/p/p.snxprj"),
        data: ProjectData {
            description: None,
            name: "p".into(),
            dir: "/p".into(),
            schematic_root: Some("root.snxsch".into()),
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: "root".into(),
                filename: "root.snxsch".into(),
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
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: fold,
        case_probed: true,
        case_decided: true,
    });
    (app, target, symbol)
}

#[test]
fn a_child_reference_that_resolves_to_an_open_sheet_is_not_reported_as_not_open() {
    // `sub/../sheet-two.snxsch` and a padded spelling both lead to the open
    // sheet by the stitcher's own resolution.
    for reference in ["sub/../sheet-two.snxsch", "  sheet-two.snxsch "] {
        let (app, target, symbol) =
            app_with_child_reference(reference, CaseFold::from_probe(false));
        assert_eq!(app.change_blocker(&target, &symbol), None, "{reference:?}");
    }
}

#[test]
fn a_child_reference_to_a_sheet_that_is_not_open_is_reported() {
    let (app, target, symbol) =
        app_with_child_reference("sub/../missing.snxsch", CaseFold::from_probe(false));
    let advice = app.change_blocker(&target, &symbol).expect("refused");
    assert!(
        advice.contains("Open every sheet") && advice.contains("missing.snxsch"),
        "{advice}"
    );
}

#[test]
fn a_case_variant_of_an_open_sheet_is_found_through_the_folded_key() {
    let variant = "SHEET-TWO.SNXSCH";
    let (app, target, symbol) = app_with_child_reference(variant, CaseFold::FOLD);
    assert_eq!(app.change_blocker(&target, &symbol), None);
    // Where case matters it is a different file, and it is not open.
    let (app, target, symbol) = app_with_child_reference(variant, CaseFold::from_probe(false));
    assert!(app.change_blocker(&target, &symbol).is_some());
}
