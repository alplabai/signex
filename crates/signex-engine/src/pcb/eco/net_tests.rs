//! #1312 — the net rows of the change list.

use signex_types::part::PinPadMap;
use signex_types::pcb::PcbBoard;

use super::fixture::*;
use super::*;
use crate::pcb::BoardEngine;

fn parts() -> Vec<EcoPart> {
    vec![
        part("R1", &[0x101], "10k", Ok(r0603())),
        part("R2", &[0x102], "4k7", Ok(r0603())),
    ]
}

fn wired() -> signex_types::net::Netlist {
    netlist(&[
        ("VCC", &[(0x101, "R1", "1")]),
        ("SIG", &[(0x101, "R1", "2"), (0x102, "R2", "1")]),
        ("GND", &[(0x102, "R2", "2")]),
    ])
}

fn placed() -> PcbBoard {
    board(vec![
        board_fp(0xa1, "R1", "10k", &[0x101], &r0603(), (10.0, 10.0)),
        board_fp(0xa2, "R2", "4k7", &[0x102], &r0603(), (20.0, 10.0)),
    ])
}

fn execute(design: &mut Design, list: &EcoChangeList) {
    let mut engine = BoardEngine::new(design.board.clone(), design.revision);
    let commands = lower(list, engine.board());
    engine
        .execute_labelled(commands, "Update PCB", list.revision)
        .unwrap();
    design.revision = engine.revision();
    design.board = engine.into_board();
}

fn synced() -> Design {
    let mut design = Design::new(placed(), parts(), wired());
    let list = design.diff();
    execute(&mut design, &list);
    assert!(design.diff().rows.is_empty());
    design
}

fn net_of<'a>(board: &'a PcbBoard, reference: &str, pad: &str) -> Option<&'a str> {
    board
        .footprints
        .iter()
        .find(|f| f.reference == reference)?
        .pads
        .iter()
        .find(|p| p.number == pad)?
        .net
        .as_ref()
        .map(|n| n.name.as_str())
}

fn groups(list: &EcoChangeList) -> Vec<(EcoGroup, usize)> {
    let mut out: Vec<(EcoGroup, usize)> = Vec::new();
    for row in &list.rows {
        match out.last_mut() {
            Some((g, n)) if *g == row.group => *n += 1,
            _ => out.push((row.group, 1)),
        }
    }
    out
}

#[test]
fn a_first_sync_adds_every_net_and_assigns_every_pad() {
    let design = Design::new(placed(), parts(), wired());
    let list = design.diff();
    assert_eq!(groups(&list), [(EcoGroup::Nets, 3), (EcoGroup::PadNets, 4)]);
    let pad_rows: Vec<&EcoRow> = list
        .rows
        .iter()
        .filter(|r| r.group == EcoGroup::PadNets)
        .collect();
    assert!(
        pad_rows
            .iter()
            .all(|r| r.needs.iter().all(|k| matches!(k, EcoKey::AddNet(_)))),
        "a pad on a new net needs that net"
    );
    let mut design = design;
    execute(&mut design, &list);
    assert_eq!(net_of(&design.board, "R1", "2"), Some("SIG"));
    assert_eq!(net_of(&design.board, "R2", "2"), Some("GND"));
}

#[test]
fn a_renamed_net_keeps_its_number_and_renames_its_zones() {
    let mut design = synced();
    let sig = design
        .board
        .nets
        .iter()
        .find(|n| n.name == "SIG")
        .unwrap()
        .number;
    design.board.zones = vec![
        serde_json::from_value(serde_json::json!({
            "uuid": u(0x2001), "net": sig, "net_name": "SIG", "layer": "TopCopper",
            "outline": [{"x": 0.0, "y": 0.0}, {"x": 5.0, "y": 0.0}, {"x": 5.0, "y": 5.0}]
        }))
        .unwrap(),
    ];
    design.netlist.nets[1].name = "DATA".into();
    let list = design.diff();
    let row = list.row(&EcoKey::RenameNet(sig)).expect("a rename row");
    assert!(
        matches!(&row.change, EcoChange::RenameNet { from, to, .. } if from == "SIG" && to == "DATA")
    );
    execute(&mut design, &list);
    assert_eq!(net_of(&design.board, "R2", "1"), Some("DATA"));
    assert_eq!(design.board.zones[0].net_name, "DATA");
    assert_eq!(design.board.zones[0].net, sig);
}

#[test]
fn a_net_with_no_pads_left_is_removed_and_a_moved_pin_gives_a_pad_row() {
    let mut design = synced();
    // R2.2 leaves GND for VCC; GND then has no pad.
    design.netlist = netlist(&[
        ("VCC", &[(0x101, "R1", "1"), (0x102, "R2", "2")]),
        ("SIG", &[(0x101, "R1", "2"), (0x102, "R2", "1")]),
    ]);
    let gnd = design
        .board
        .nets
        .iter()
        .find(|n| n.name == "GND")
        .unwrap()
        .number;
    let list = design.diff();
    assert!(
        list.row(&EcoKey::RemoveNet(gnd)).is_some(),
        "{}",
        report_text(&list)
    );
    let pad = &design.board.footprints[1].pads[1];
    let row = list
        .row(&EcoKey::PadNet {
            footprint: u(0xa2),
            pad: pad.uuid,
        })
        .unwrap();
    assert!(matches!(
        &row.change,
        EcoChange::PadNet { from: Some(f), to: Some(t), .. } if f.name == "GND" && t.name == "VCC"
    ));
    execute(&mut design, &list);
    assert_eq!(net_of(&design.board, "R2", "2"), Some("VCC"));
    assert!(!design.board.nets.iter().any(|n| n.name == "GND"));
}

#[test]
fn a_pad_whose_net_is_locked_gets_no_row() {
    let mut design = synced();
    design.board.footprints[1].pads[1].net_locked = true;
    design.netlist = netlist(&[
        ("VCC", &[(0x101, "R1", "1"), (0x102, "R2", "2")]),
        ("SIG", &[(0x101, "R1", "2"), (0x102, "R2", "1")]),
    ]);
    let list = design.diff();
    let locked = design.board.footprints[1].pads[1].uuid;
    assert!(
        !list
            .rows
            .iter()
            .any(|r| matches!(r.key, EcoKey::PadNet { pad, .. } if pad == locked)),
        "{}",
        report_text(&list)
    );
}

#[test]
fn a_new_parts_pads_need_its_add_and_unticking_the_add_drops_them() {
    let mut design = synced();
    design.parts.push(part("R3", &[0x103], "1k", Ok(r0603())));
    design.netlist.nets[1]
        .terminals
        .push(netlist(&[("SIG", &[(0x103, "R3", "1")])]).nets[0].terminals[0].clone());
    let add = EcoKey::AddFootprint {
        sheet: root(),
        part: u(0x103),
    };
    let mut list = design.diff();
    let pad_rows: Vec<EcoKey> = list
        .rows
        .iter()
        .filter(|r| r.needs.contains(&add))
        .map(|r| r.key.clone())
        .collect();
    assert_eq!(pad_rows.len(), 1, "{}", report_text(&list));

    assert!(list.set_on(&add, false), "a footprint row changed");
    refresh_nets(&mut list, &design.input());
    assert!(
        !list.rows.iter().any(|r| r.needs.contains(&add)),
        "the pad rows of an unticked add are gone"
    );
    assert!(lower(&list, &design.board).is_empty());
}

#[test]
fn a_pin_map_lands_pins_on_the_mapped_pads() {
    let mut design = Design::new(placed(), parts(), wired());
    let mut mapped = r0603();
    mapped.pin_map = PinPadMap::from_pairs([(" 1", "2"), ("2", "1")]);
    design.parts[0].footprint = Ok(mapped);
    let list = design.diff();
    execute(&mut design, &list);
    assert_eq!(
        net_of(&design.board, "R1", "2"),
        Some("VCC"),
        "pin 1 on pad 2"
    );
    assert_eq!(net_of(&design.board, "R1", "1"), Some("SIG"));
}

#[test]
fn the_report_is_stable_text() {
    let mut design = synced();
    design.parts.push(part("C1", &[0x301], "100n", Ok(r0603())));
    design.parts[1].value = "47k".into();
    design.parts.push(part(
        "J1",
        &[0x401],
        "conn",
        Err(signex_types::part::FootprintResolveError::NoFootprint),
    ));
    design
        .board
        .footprints
        .push(board_fp(0xb1, "H1", "", &[], &r0603(), (2.0, 2.0)));
    let list = design.diff();
    let text = report_text(&list);
    assert_eq!(text, report_text(&design.diff()), "deterministic");
    let source = r0603().source;
    assert_eq!(
        text,
        format!(
            "Update PCB from schematic
Board revision: 2
Changes: 2 of 3 ticked

Add footprints
  [x] Add C1 (100n) {source}
  [-] Add J1 (conn) (blocked: no footprint: the part binds no footprint)

Values
  [x] Value of R2: 4k7 -> 47k

Notes
  - H1 is not linked to a part and is kept
"
        )
    );
}
