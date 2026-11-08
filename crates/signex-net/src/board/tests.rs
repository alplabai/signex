//! Net assignment on a small schematic-plus-board fixture.
use super::*;
use crate::build_netlist;
use crate::fixtures::{empty_sheet, label, pt, wire};
use signex_types::net::Net;
use signex_types::pcb::{Footprint, NetDef, Pad, PadShape, PadType};
use signex_types::schematic::{
    InstancePath, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, SchematicSheet,
    Symbol,
};

fn lib_pin(number: &str, x: f64) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: pt(x, 0.0),
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

fn add_resistor_lib(sheet: &mut SchematicSheet) {
    sheet.lib_symbols.insert(
        "R".to_string(),
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
            pins: vec![lib_pin("1", 0.0), lib_pin("2", 10.0)],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
}

fn place(sheet: &mut SchematicSheet, reference: &str, x: f64) {
    sheet.symbols.push(Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "R".to_string(),
        reference: reference.to_string(),
        position: pt(x, 0.0),
        ..Default::default()
    });
}

/// `IN — R1 — mid — R2 — OUT`: three nets, each pin on exactly one.
fn divider_sheet() -> SchematicSheet {
    let mut sheet = empty_sheet();
    add_resistor_lib(&mut sheet);
    place(&mut sheet, "R1", 0.0);
    place(&mut sheet, "R2", 20.0);
    sheet.wires.push(wire(pt(-5.0, 0.0), pt(0.0, 0.0)));
    sheet.wires.push(wire(pt(10.0, 0.0), pt(20.0, 0.0)));
    sheet.wires.push(wire(pt(30.0, 0.0), pt(35.0, 0.0)));
    sheet
        .labels
        .push(label("IN", pt(-5.0, 0.0), LabelType::Net));
    sheet
        .labels
        .push(label("OUT", pt(35.0, 0.0), LabelType::Net));
    sheet
}

fn pad(number: &str) -> Pad {
    Pad {
        uuid: Uuid::now_v7(),
        number: number.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: pt(0.0, 0.0),
        size: pt(1.0, 1.0),
        drill: None,
        layers: Vec::new(),
        net: None,
        roundrect_ratio: 0.0,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn footprint(reference: &str, pads: &[&str]) -> Footprint {
    Footprint {
        uuid: Uuid::now_v7(),
        reference: reference.to_string(),
        value: String::new(),
        footprint_id: String::new(),
        position: pt(0.0, 0.0),
        rotation: 0.0,
        layer: String::new(),
        locked: false,
        pads: pads.iter().map(|n| pad(n)).collect(),
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn board(footprints: Vec<Footprint>) -> PcbBoard {
    PcbBoard {
        uuid: Uuid::now_v7(),
        version: 0,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints,
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

/// The net a schematic terminal is on, as the board should record it.
fn net_of(netlist: &Netlist, reference: &str, pin: &str) -> PadNet {
    let net = netlist
        .nets
        .iter()
        .find(|n| {
            n.terminals
                .iter()
                .any(|t| t.reference == reference && t.pin == pin)
        })
        .expect("terminal on a net");
    PadNet {
        number: net.id.0,
        name: net.name.clone(),
    }
}

fn pad_net<'a>(board: &'a PcbBoard, reference: &str, number: &str) -> Option<&'a PadNet> {
    board
        .footprints
        .iter()
        .find(|f| f.reference == reference)?
        .pads
        .iter()
        .find(|p| p.number == number)?
        .net
        .as_ref()
}

fn net_rows(board: &PcbBoard) -> Vec<(u32, String)> {
    board
        .nets
        .iter()
        .map(|n| (n.number, n.name.clone()))
        .collect()
}

fn terminal(reference: &str, pin: &str) -> Terminal {
    Terminal {
        symbol: Uuid::now_v7(),
        reference: reference.to_string(),
        pin: pin.to_string(),
        occurrence: InstancePath::root(),
    }
}

fn net(id: u32, name: &str, terminals: Vec<Terminal>) -> Net {
    Net {
        id: NetId(id),
        name: name.to_string(),
        class: None,
        wires: Vec::new(),
        junctions: Vec::new(),
        terminals,
    }
}

#[test]
fn every_pad_gets_the_net_of_its_schematic_pin() {
    let netlist = build_netlist(&divider_sheet());
    assert_eq!(netlist.nets.len(), 3, "fixture: IN, mid and OUT");
    let mut pcb = board(vec![
        footprint("R1", &["1", "2"]),
        footprint("R2", &["1", "2"]),
    ]);

    let assignment = assign_pad_nets(&netlist, &pcb, &ReferencePadKey);
    assert!(assignment.issues.is_empty(), "{:?}", assignment.issues);
    assignment.edit.apply(&mut pcb);

    for (reference, pin) in [("R1", "1"), ("R1", "2"), ("R2", "1"), ("R2", "2")] {
        assert_eq!(
            pad_net(&pcb, reference, pin),
            Some(&net_of(&netlist, reference, pin)),
            "{reference}.{pin}"
        );
    }
    assert_eq!(pad_net(&pcb, "R1", "1").unwrap().name, "IN");
    assert_eq!(pad_net(&pcb, "R2", "2").unwrap().name, "OUT");
    assert_eq!(pad_net(&pcb, "R1", "2"), pad_net(&pcb, "R2", "1"));
    let expected: Vec<(u32, String)> = netlist
        .nets
        .iter()
        .map(|n| (n.id.0, n.name.clone()))
        .collect();
    assert_eq!(net_rows(&pcb), expected);
    assert!(pcb.nets.iter().all(|n| n.number != 0), "0 means no net");
}

#[test]
fn the_inverse_edit_restores_the_board() {
    let netlist = build_netlist(&divider_sheet());
    let mut pcb = board(vec![footprint("R1", &["1", "2"]), footprint("R3", &["1"])]);
    pcb.nets.push(NetDef {
        number: 7,
        name: "OLD".to_string(),
    });
    let old = PadNet {
        number: 7,
        name: "OLD".to_string(),
    };
    pcb.footprints[1].pads[0].net = Some(old.clone());

    let inverse = assign_pad_nets(&netlist, &pcb, &ReferencePadKey)
        .edit
        .apply(&mut pcb);
    assert_eq!(pad_net(&pcb, "R3", "1"), Some(&old), "R3 is not in the schematic");
    assert!(pad_net(&pcb, "R1", "1").is_some());

    inverse.apply(&mut pcb);
    assert_eq!(net_rows(&pcb), vec![(7, "OLD".to_string())]);
    assert_eq!(pad_net(&pcb, "R3", "1"), Some(&old));
    assert_eq!(pad_net(&pcb, "R1", "1"), None);
    assert_eq!(pad_net(&pcb, "R1", "2"), None);
}

#[test]
fn an_assigned_board_needs_no_pad_change() {
    let netlist = build_netlist(&divider_sheet());
    let mut pcb = board(vec![
        footprint("R1", &["1", "2"]),
        footprint("R2", &["1", "2"]),
    ]);
    assign_pad_nets(&netlist, &pcb, &ReferencePadKey)
        .edit
        .apply(&mut pcb);

    let again = assign_pad_nets(&netlist, &pcb, &ReferencePadKey);
    assert!(again.edit.pads.is_empty(), "{:?}", again.edit.pads);
}

#[test]
fn a_terminal_with_no_footprint_is_reported() {
    let netlist = build_netlist(&divider_sheet());
    let mut pcb = board(vec![footprint("R1", &["1", "2"])]);

    let assignment = assign_pad_nets(&netlist, &pcb, &ReferencePadKey);
    let missing: Vec<(&str, &str)> = assignment
        .issues
        .iter()
        .map(|issue| match issue {
            PadNetIssue::TerminalWithoutPad { terminal, .. } => {
                (terminal.reference.as_str(), terminal.pin.as_str())
            }
            other => panic!("unexpected issue {other:?}"),
        })
        .collect();
    assert_eq!(missing.len(), 2);
    assert!(missing.contains(&("R2", "1")) && missing.contains(&("R2", "2")));

    assignment.edit.apply(&mut pcb);
    assert_eq!(pad_net(&pcb, "R1", "2"), Some(&net_of(&netlist, "R1", "2")));
}

#[test]
fn a_missing_pad_number_is_reported() {
    let netlist = build_netlist(&divider_sheet());
    let pcb = board(vec![footprint("R1", &["1"]), footprint("R2", &["1", "2"])]);

    let issues = assign_pad_nets(&netlist, &pcb, &ReferencePadKey).issues;
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert!(matches!(
        &issues[0],
        PadNetIssue::TerminalWithoutPad { terminal, .. } if terminal.reference == "R1" && terminal.pin == "2"
    ));
}

#[test]
fn a_reference_on_two_footprints_assigns_neither() {
    let netlist = build_netlist(&divider_sheet());
    let mut pcb = board(vec![
        footprint("R1", &["1", "2"]),
        footprint("R1", &["1", "2"]),
        footprint("R2", &["1", "2"]),
    ]);
    let twins = [pcb.footprints[0].uuid, pcb.footprints[1].uuid];

    let assignment = assign_pad_nets(&netlist, &pcb, &ReferencePadKey);
    assert_eq!(assignment.issues.len(), 2, "{:?}", assignment.issues);
    for issue in &assignment.issues {
        let PadNetIssue::KeyOnSeveralFootprints { footprints, .. } = issue else {
            panic!("unexpected issue {issue:?}");
        };
        assert_eq!(footprints.as_slice(), twins.as_slice());
    }
    assignment.edit.apply(&mut pcb);
    assert!(
        pcb.footprints[..2]
            .iter()
            .flat_map(|f| &f.pads)
            .all(|p| p.net.is_none())
    );
    assert!(pad_net(&pcb, "R2", "1").is_some());
}

#[test]
fn a_pad_on_two_nets_is_left_unassigned() {
    let netlist = Netlist {
        nets: vec![
            net(1, "A", vec![terminal("U1", "1"), terminal("U1", "2")]),
            net(2, "B", vec![terminal("U1", "1")]),
        ],
    };
    let mut pcb = board(vec![footprint("U1", &["1", "2"])]);
    let pad_one = pcb.footprints[0].pads[0].uuid;

    let assignment = assign_pad_nets(&netlist, &pcb, &ReferencePadKey);
    assert_eq!(
        assignment.issues,
        vec![PadNetIssue::PadOnSeveralNets {
            footprint: pcb.footprints[0].uuid,
            pad: pad_one,
            nets: vec![NetId(1), NetId(2)],
        }]
    );
    assignment.edit.apply(&mut pcb);
    assert_eq!(pad_net(&pcb, "U1", "1"), None);
    assert_eq!(pad_net(&pcb, "U1", "2").map(|n| n.name.as_str()), Some("A"));
}

#[test]
fn an_unannotated_terminal_never_links() {
    let netlist = Netlist {
        nets: vec![net(1, "A", vec![terminal("R?", "1")])],
    };
    let mut pcb = board(vec![footprint("R?", &["1"])]);

    let assignment = assign_pad_nets(&netlist, &pcb, &ReferencePadKey);
    assert!(matches!(
        assignment.issues.as_slice(),
        [
            PadNetIssue::TerminalWithoutPad { net: NetId(1), .. },
            PadNetIssue::PadWithoutTerminal { .. }
        ]
    ));
    assignment.edit.apply(&mut pcb);
    assert_eq!(pad_net(&pcb, "R?", "1"), None);
}

/// A link rule other than the designator: the key is swappable without
/// touching the assignment.
struct SymbolKey(HashMap<Uuid, Uuid>);

impl PadLinkKey for SymbolKey {
    type Key = (Uuid, String);

    fn terminal_key(&self, terminal: &Terminal) -> Option<Self::Key> {
        Some((terminal.symbol, terminal.pin.clone()))
    }

    fn pad_key(&self, footprint: &Footprint, pad: &Pad) -> Option<Self::Key> {
        Some((*self.0.get(&footprint.uuid)?, pad.number.clone()))
    }
}

#[test]
fn another_link_key_drives_the_same_assignment() {
    let t = terminal("R?", "1");
    let symbol = t.symbol;
    let netlist = Netlist {
        nets: vec![net(4, "SIG", vec![t])],
    };
    let mut pcb = board(vec![footprint("", &["1"])]);
    let key = SymbolKey(HashMap::from([(pcb.footprints[0].uuid, symbol)]));

    let assignment = assign_pad_nets(&netlist, &pcb, &key);
    assert!(assignment.issues.is_empty(), "{:?}", assignment.issues);
    assignment.edit.apply(&mut pcb);
    assert_eq!(
        pcb.footprints[0].pads[0].net,
        Some(PadNet {
            number: 4,
            name: "SIG".to_string(),
        })
    );
}

use signex_types::pcb::{Segment, Via, ViaType, Zone};

fn mid_pads() -> Vec<Terminal> {
    vec![terminal("R1", "2"), terminal("R2", "1")]
}

/// The divider with ids as the netlist numbers them before and after a wire
/// move re-sorts the roots: the unlabelled middle net goes 2 -> 1.
fn divider_netlists() -> (Netlist, Netlist) {
    let before = Netlist {
        nets: vec![
            net(1, "IN", vec![terminal("R1", "1")]),
            net(2, "N$2", mid_pads()),
            net(3, "OUT", vec![terminal("R2", "2")]),
        ],
    };
    let after = Netlist {
        nets: vec![
            net(1, "N$1", mid_pads()),
            net(2, "IN", vec![terminal("R1", "1")]),
            net(3, "OUT", vec![terminal("R2", "2")]),
        ],
    };
    (before, after)
}

fn copper_on(pcb: &mut PcbBoard, number: u32, name: &str) {
    pcb.segments.push(Segment {
        uuid: Uuid::now_v7(),
        start: pt(0.0, 0.0),
        end: pt(1.0, 0.0),
        width: 0.2,
        layer: "F.Cu".to_string(),
        net: number,
        unknown_columns: Default::default(),
    });
    pcb.vias.push(Via {
        uuid: Uuid::now_v7(),
        position: pt(1.0, 0.0),
        diameter: 0.6,
        drill: 0.3,
        layers: Vec::new(),
        net: number,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
    });
    pcb.zones.push(Zone {
        uuid: Uuid::now_v7(),
        net: number,
        net_name: name.to_string(),
        layer: "F.Cu".to_string(),
        outline: Vec::new(),
        holes: Vec::new(),
        priority: 0,
        fill_type: String::new(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        unknown: Default::default(),
    });
}

#[test]
fn copper_keeps_its_net_when_the_netlist_renumbers() {
    let (before, after) = divider_netlists();
    let mut pcb = board(vec![
        footprint("R1", &["1", "2"]),
        footprint("R2", &["1", "2"]),
    ]);
    assign_pad_nets(&before, &pcb, &ReferencePadKey)
        .edit
        .apply(&mut pcb);
    let mid = pad_net(&pcb, "R1", "2").cloned().expect("mid assigned");
    assert_eq!(mid.number, 2);
    copper_on(&mut pcb, mid.number, &mid.name);
    let copper = (pcb.segments.clone(), pcb.vias.clone(), pcb.zones.clone());

    let again = assign_pad_nets(&after, &pcb, &ReferencePadKey);
    assert!(again.issues.is_empty(), "{:?}", again.issues);
    again.edit.apply(&mut pcb);

    assert_eq!(pad_net(&pcb, "R1", "2"), Some(&mid), "number and name kept");
    assert_eq!(pad_net(&pcb, "R2", "1"), Some(&mid));
    assert_eq!(copper, (pcb.segments.clone(), pcb.vias.clone(), pcb.zones.clone()));
    let rows = net_rows(&pcb);
    assert_eq!(rows.iter().filter(|(n, _)| *n == mid.number).count(), 1);
    assert!(rows.contains(&(mid.number, "N$2".to_string())));
    let numbers: Vec<u32> = rows.iter().map(|(n, _)| *n).collect();
    let mut unique = numbers.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), numbers.len(), "numbers are distinct");
}

#[test]
fn a_net_with_routed_copper_stays_in_the_table_when_it_loses_its_pads() {
    let (before, _) = divider_netlists();
    let mut pcb = board(vec![
        footprint("R1", &["1", "2"]),
        footprint("R2", &["1", "2"]),
    ]);
    assign_pad_nets(&before, &pcb, &ReferencePadKey)
        .edit
        .apply(&mut pcb);
    copper_on(&mut pcb, 2, "N$2");

    let only_in = Netlist {
        nets: vec![
            net(1, "IN", vec![terminal("R1", "1")]),
            net(3, "OUT", vec![terminal("R2", "2")]),
        ],
    };
    assign_pad_nets(&only_in, &pcb, &ReferencePadKey)
        .edit
        .apply(&mut pcb);
    assert!(pcb.nets.iter().any(|n| n.number == 2), "{:?}", pcb.nets);
    assert_eq!(pad_net(&pcb, "R1", "2"), None, "its pads are cleared");
    assert_eq!(pad_net(&pcb, "R2", "1"), None);
}

#[test]
fn a_new_net_never_takes_a_number_copper_uses() {
    let mut pcb = board(vec![footprint("U1", &["1"])]);
    pcb.nets.push(NetDef {
        number: 1,
        name: "OLD".to_string(),
    });
    copper_on(&mut pcb, 1, "OLD");
    let netlist = Netlist {
        nets: vec![net(1, "NEW", vec![terminal("U1", "1")])],
    };

    assign_pad_nets(&netlist, &pcb, &ReferencePadKey)
        .edit
        .apply(&mut pcb);
    let pad = pad_net(&pcb, "U1", "1").expect("assigned");
    assert_ne!(pad.number, 1);
    assert!(pcb.nets.iter().any(|n| n.number == 1 && n.name == "OLD"));
}

#[test]
fn a_keyless_pad_and_an_unknown_footprint_keep_their_nets_and_are_reported() {
    let netlist = build_netlist(&divider_sheet());
    let mut pcb = board(vec![
        footprint("R1", &["1", "2", ""]),
        footprint("R2", &["1", "2"]),
        footprint("H1", &["1"]),
    ]);
    let hand = PadNet {
        number: 9,
        name: "SHIELD".to_string(),
    };
    pcb.nets.push(NetDef {
        number: 9,
        name: "SHIELD".to_string(),
    });
    pcb.footprints[0].pads[2].net = Some(hand.clone());
    pcb.footprints[2].pads[0].net = Some(hand.clone());
    let hole = (pcb.footprints[0].uuid, pcb.footprints[0].pads[2].uuid);
    let h1 = pcb.footprints[2].uuid;

    let assignment = assign_pad_nets(&netlist, &pcb, &ReferencePadKey);
    assert!(assignment.issues.contains(&PadNetIssue::PadWithoutTerminal {
        footprint: hole.0,
        pad: hole.1
    }));
    assert!(
        assignment
            .issues
            .contains(&PadNetIssue::FootprintNotInSchematic { footprint: h1 })
    );
    assignment.edit.apply(&mut pcb);
    assert_eq!(pcb.footprints[0].pads[2].net, Some(hand.clone()));
    assert_eq!(pad_net(&pcb, "H1", "1"), Some(&hand));
    assert!(pcb.nets.iter().any(|n| n.number == 9), "{:?}", pcb.nets);
    assert!(pad_net(&pcb, "R1", "1").is_some());
}

#[test]
fn a_conflicting_pad_keeps_its_existing_net() {
    let netlist = Netlist {
        nets: vec![
            net(1, "A", vec![terminal("U1", "1")]),
            net(2, "B", vec![terminal("U1", "1")]),
        ],
    };
    let mut pcb = board(vec![footprint("U1", &["1"])]);
    let old = PadNet {
        number: 5,
        name: "KEEP".to_string(),
    };
    pcb.footprints[0].pads[0].net = Some(old.clone());
    assign_pad_nets(&netlist, &pcb, &ReferencePadKey)
        .edit
        .apply(&mut pcb);
    assert_eq!(pad_net(&pcb, "U1", "1"), Some(&old));
}
