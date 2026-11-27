//! Mockup row 208: the picker names the pin whose wiring an alternate would
//! change, from the sheet's connectivity read with the candidates.

use std::sync::Arc;

use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol, Wire,
};
use uuid::Uuid;

use super::candidates::{CandidateQuery, read_candidates};
use super::wiring::{PinProblem, pin_problem, wired_pins};

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

/// C22 at (50, 50) with pins 1 at (45, 50) and 2 at (55, 50); a wire runs
/// into pin 1 only.
fn sheet() -> (SchematicSheet, Uuid) {
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    sheet.lib_symbols.insert(
        "C".into(),
        LibSymbol {
            id: "C".into(),
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
            pins: vec![lib_pin("1", -5.0), lib_pin("2", 5.0)],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
    let c22 = Uuid::now_v7();
    sheet.symbols.push(Symbol {
        uuid: c22,
        lib_id: "C".into(),
        reference: "C22".into(),
        position: Point::new(50.0, 50.0),
        ..Default::default()
    });
    sheet.wires = vec![Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(30.0, 50.0),
        end: Point::new(45.0, 50.0),
        ..Default::default()
    }];
    (sheet, c22)
}

fn pins(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(number, name)| (number.to_string(), name.to_string()))
        .collect()
}

#[test]
fn the_parts_wired_pins_come_from_the_sheets_netlist() {
    let (sheet, c22) = sheet();
    let netlist = signex_net::build_netlist(&sheet);
    assert_eq!(wired_pins(&netlist, &[c22]), ["1"]);
    assert!(wired_pins(&netlist, &[Uuid::now_v7()]).is_empty());
}

/// The read the picker starts works the wiring out with the candidates; a
/// later read (no sheet) leaves it unread.
#[test]
fn the_candidates_read_carries_the_wired_pins() {
    let (sheet, c22) = sheet();
    let query = |sheet: Option<Arc<SchematicSheet>>| CandidateQuery {
        row: None,
        base_mpn: String::new(),
        query: String::new(),
        project_libraries: Vec::new(),
        open_libraries: Vec::new(),
        sheet,
        units: vec![c22],
    };
    let first = read_candidates(&query(Some(Arc::new(sheet))));
    assert_eq!(first.wired, Some(vec!["1".to_string()]));
    assert_eq!(read_candidates(&query(None)).wired, None);
}

#[test]
fn a_wired_pin_the_candidate_lacks_is_named_first() {
    let base = pins(&[("1", "A"), ("2", "K")]);
    let wired = ["1".to_string(), "2".to_string()];
    assert_eq!(
        pin_problem(&base, &wired, &pins(&[("1", "A")])),
        Some(PinProblem::LosesWire {
            pin: "2".into(),
            name: "K".into(),
        })
    );
}

/// Mockup row 208: a three-pin part for a two-pin one leaves its third pin
/// with no wire.
#[test]
fn a_candidate_pin_no_wire_reaches_is_named() {
    let base = pins(&[("1", "~"), ("2", "~")]);
    let wired = ["1".to_string(), "2".to_string()];
    let candidate = pins(&[("1", "~"), ("2", "~"), ("3", "GND")]);
    assert_eq!(
        pin_problem(&base, &wired, &candidate),
        Some(PinProblem::NoWire {
            pin: "3".into(),
            name: "GND".into(),
        })
    );
    assert_eq!(pin_problem(&base, &wired, &base), None);
}
