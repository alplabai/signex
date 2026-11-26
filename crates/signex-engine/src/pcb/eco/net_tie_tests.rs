//! #1277: a net-tie part becomes a net-tie footprint on the board, so DRC
//! allows its copper bridge (Caner, 2026-11-25).

use signex_types::pcb::ComponentType;
use signex_types::schematic::{SchematicSheet, Symbol};

use super::fixture::*;
use super::*;

fn sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    for (n, reference, net_tie, in_bom) in [
        (0x1, "NT1", true, true),
        (0x2, "NT2", true, false),
        (0x3, "R1", false, true),
    ] {
        sheet.symbols.push(Symbol {
            uuid: u(n),
            lib_id: "Device:NetTie".into(),
            reference: reference.into(),
            net_tie,
            in_bom,
            ..Symbol::default()
        });
    }
    sheet
}

fn types(parts: &[EcoPart]) -> Vec<(&str, ComponentType)> {
    parts
        .iter()
        .map(|p| {
            let fp = &p.footprint.as_ref().unwrap().footprint;
            (p.part.designator.as_str(), fp.component.component_type)
        })
        .collect()
}

#[test]
fn a_net_tie_parts_footprint_is_typed_a_net_tie_by_its_bom_flag() {
    let parts = design_eco_parts(&[(root(), &sheet())], |_, _| Ok(r0603()));
    assert_eq!(
        types(&parts),
        [
            ("NT1", ComponentType::NetTieInBom),
            ("NT2", ComponentType::NetTie),
            ("R1", ComponentType::Standard),
        ]
    );
}

#[test]
fn a_footprint_the_library_types_as_a_tie_keeps_its_type() {
    let tie = || {
        let mut fp = r0603();
        fp.footprint.component.component_type = ComponentType::NetTie;
        fp
    };
    let parts = design_eco_parts(&[(root(), &sheet())], |_, _| Ok(tie()));
    assert_eq!(
        types(&parts),
        [
            ("NT1", ComponentType::NetTie),
            ("NT2", ComponentType::NetTie),
            ("R1", ComponentType::NetTie),
        ]
    );
}

/// The add row Update PCB executes carries the tie type to the board.
#[test]
fn the_add_row_places_a_net_tie_footprint() {
    let parts = design_eco_parts(&[(root(), &sheet())], |_, _| Ok(r0603()));
    let design = Design::new(board(Vec::new()), parts, netlist(&[]));
    let list = design.diff();
    let row = list
        .row(&EcoKey::AddFootprint {
            sheet: root(),
            part: u(0x2),
        })
        .expect("an add row for NT2");
    let EcoChange::AddFootprint { footprint } = &row.change else {
        panic!("an add row");
    };
    assert_eq!(footprint.component.component_type, ComponentType::NetTie);
}
