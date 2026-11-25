//! #2658: a channel card's miniature draws the sheet with the designators
//! and net names of that channel.
use signex_types::project::ChannelNaming;
use signex_types::schematic::{FillType, Graphic, LibGraphic, Point, SchematicSheet};

use crate::fixtures::multi_channel::{BUCK, BUCK_PARTS, project};
use crate::{CompiledSheet, SheetKey, compile_design, sheet_miniature};

/// Row 4 of the compiled buck project is channel CH2.
const CH2_ROW: usize = 4;

fn ch2(project: &crate::fixtures::multi_channel::Project) -> (SchematicSheet, CompiledSheet) {
    let design = compile_design(&project.graph(), &ChannelNaming::default());
    let row = design.sheets[CH2_ROW].clone();
    assert_eq!(row.channel.as_ref().map(|c| c.index), Some(2));
    (project.sheets[&SheetKey::new(BUCK)].clone(), row)
}

#[test]
fn the_miniature_names_parts_and_nets_as_its_channel_does() {
    let (sheet, row) = ch2(&project());
    let mini = sheet_miniature(&sheet, &row).expect("a drawing");
    let mut names: Vec<&str> = mini.designators.iter().map(|t| t.text.as_str()).collect();
    names.sort_unstable();
    let mut expected: Vec<String> = BUCK_PARTS.iter().map(|p| format!("CH2_{p}")).collect();
    expected.sort_unstable();
    assert_eq!(names, expected);
    let nets: Vec<&str> = mini.nets.iter().map(|t| t.text.as_str()).collect();
    assert!(nets.contains(&"VOUT_3V3_CH2"), "{nets:?}");
    assert!(!nets.iter().any(|n| n.ends_with("_CH1")), "{nets:?}");
    assert!(mini.nets.iter().any(|t| t.text == "GND" && t.is_power));
}

#[test]
fn the_miniature_draws_every_wire_inside_its_bounds() {
    let (sheet, row) = ch2(&project());
    let mini = sheet_miniature(&sheet, &row).expect("a drawing");
    assert_eq!(mini.wires.len(), sheet.wires.len());
    for (a, b) in &mini.wires {
        assert!(mini.bounds.contains(a.x, a.y) && mini.bounds.contains(b.x, b.y));
    }
}

/// A part that draws a body has its box drawn, and its designator sits in
/// the middle of it.
#[test]
fn a_part_body_is_drawn_with_its_designator_in_the_middle() {
    let mut p = project();
    let lib = p
        .sheet_mut(BUCK)
        .lib_symbols
        .get_mut("P")
        .expect("the fixture's part");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-1.0, -1.0),
            end: Point::new(1.0, 1.0),
            width: 0.0,
            fill: FillType::None,
        },
    });
    let (sheet, row) = ch2(&p);
    let mini = sheet_miniature(&sheet, &row).expect("a drawing");
    assert_eq!(mini.bodies.len(), BUCK_PARTS.len());
    let u1 = sheet
        .symbols
        .iter()
        .find(|s| s.reference == "U1")
        .expect("U1");
    let label = mini
        .designators
        .iter()
        .find(|t| t.text == "CH2_U1")
        .expect("CH2_U1");
    assert!((label.at.x - u1.position.x).abs() < 1e-9);
    assert!((label.at.y - u1.position.y).abs() < 1e-9);
}

#[test]
fn a_sheet_with_nothing_drawn_has_no_miniature() {
    let (_, mut row) = ch2(&project());
    row.components.clear();
    row.net_names.clear();
    assert!(sheet_miniature(&SchematicSheet::default(), &row).is_none());
}
