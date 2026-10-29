//! A library symbol whose normal and De Morgan bodies carry different pins must
//! give a placed symbol only the pins of the body it shows (#1233): the
//! netlist and ERC read the same pin set, so nothing is doubled.

use signex_erc::ErcContext;
use signex_net::build_netlist;
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol, Wire,
};
use uuid::Uuid;

fn lib_pin(body_style: u32, x: f64) -> LibPin {
    LibPin {
        unit: 0,
        body_style,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: "1".to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn two_body_sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let lib = LibSymbol {
        id: "G".to_string(),
        pins: vec![lib_pin(1, 0.0), lib_pin(2, 5.0)],
        ..blank_lib()
    };
    sheet.lib_symbols.insert("G".to_string(), lib);
    sheet.wires.push(Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(0.0, 0.0),
        end: Point::new(5.0, 0.0),
        ..Default::default()
    });
    sheet.symbols.push(Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "G".to_string(),
        reference: "U1".to_string(),
        ..Default::default()
    });
    sheet
}

fn blank_lib() -> LibSymbol {
    LibSymbol {
        id: String::new(),
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
        pins: Vec::new(),
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

#[test]
fn netlist_gives_the_symbol_one_pin_not_one_per_body() {
    let sheet = two_body_sheet();
    let terminals: usize = build_netlist(&sheet)
        .nets
        .iter()
        .map(|n| n.terminals.iter().filter(|t| t.reference == "U1").count())
        .sum();
    assert_eq!(terminals, 1, "only the normal body's pin is a terminal");
}

#[test]
fn erc_gives_the_symbol_one_pin_not_one_per_body() {
    let ctx = ErcContext::from_snapshot(&two_body_sheet());
    let u1 = ctx.symbols.iter().find(|s| s.reference == "U1").unwrap();
    assert_eq!(u1.pins.len(), 1);
}
