//! #1286: units of one part split across sheets are judged project-wide.

use signex_erc::{RuleKind, project_units_not_placed, run};
use signex_types::schematic::{
    FillType, Graphic, LibGraphic, LibSymbol, Point, SchematicSheet, Symbol,
};
use uuid::Uuid;

const LIB: &str = "Amplifier:LM358";

fn dual_lib() -> LibSymbol {
    let graphic = |unit| LibGraphic {
        unit,
        body_style: 1,
        graphic: Graphic::Polyline {
            points: Vec::new(),
            width: 0.0,
            fill: FillType::default(),
        },
    };
    LibSymbol {
        id: LIB.into(),
        reference: "U".into(),
        value: "LM358".into(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: vec![graphic(1), graphic(2)],
        pins: Vec::new(),
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn sheet_with_units(units: &[u32]) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".into(),
        symbols: units
            .iter()
            .map(|&unit| Symbol {
                uuid: Uuid::new_v4(),
                lib_id: LIB.into(),
                reference: "U1".into(),
                unit,
                position: Point::new(25.4, 25.4),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    sheet.lib_symbols.insert(LIB.into(), dual_lib());
    sheet
}

#[test]
fn a_unit_on_another_sheet_counts_as_placed() {
    let (a, b) = (sheet_with_units(&[1]), sheet_with_units(&[2]));
    assert!(project_units_not_placed(&[("a", &a), ("b", &b)]).is_empty());
    // On its own, each sheet still sees the other unit as missing.
    let own: Vec<_> = run(&a)
        .into_iter()
        .filter(|v| v.rule == RuleKind::UnitsNotPlaced)
        .collect();
    assert_eq!(own.len(), 1);
}

#[test]
fn a_unit_placed_nowhere_is_filed_under_the_first_units_sheet() {
    let (a, empty) = (sheet_with_units(&[2]), sheet_with_units(&[]));
    let found = project_units_not_placed(&[("empty", &empty), ("a", &a)]);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, "a");
    assert_eq!(found[0].1.builtin_kind(), Some(RuleKind::UnitsNotPlaced));
    assert!(
        found[0].1.message.ends_with("unit 1"),
        "{}",
        found[0].1.message
    );
}
