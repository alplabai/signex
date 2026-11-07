//! `NetBase` must answer what the full derive answers, for the base itself and
//! for any edit of it: a few targeted edits first, then random sheets under
//! random edits.
use super::*;
use crate::{NetBase, net_signatures, net_signatures_at};
use signex_types::schematic::{BodyStyle, ChildSheet, FillType, LabelType, NoConnect, SheetPin};

fn fresh_wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: a,
        end: b,
        ..Default::default()
    }
}

fn fresh_dot(at: Point) -> Junction {
    Junction {
        uuid: Uuid::now_v7(),
        position: at,
        ..Default::default()
    }
}

fn libs(sheet: &mut SchematicSheet) {
    add_lib(
        sheet,
        "R",
        vec![
            lib_pin("1", pt(-2.5, 0.0), PinDirection::Passive),
            lib_pin("2", pt(2.5, 0.0), PinDirection::Passive),
        ],
    );
    add_lib(
        sheet,
        "P",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::PowerInput)],
    );
    add_lib(
        sheet,
        "power:PWR_FLAG",
        vec![lib_pin("1", pt(0.0, 0.0), PinDirection::PowerOutput)],
    );
    // Two units, the first with an alternate body.
    let mut pins = vec![
        lib_pin("1", pt(-2.5, 0.0), PinDirection::Passive),
        lib_pin("2", pt(2.5, 0.0), PinDirection::Passive),
        lib_pin("3", pt(0.0, 2.5), PinDirection::Passive),
        lib_pin("4", pt(0.0, -2.5), PinDirection::Passive),
    ];
    for (pin, (unit, body)) in pins.iter_mut().zip([(1, 1), (2, 1), (1, 2), (2, 2)]) {
        pin.unit = unit;
        pin.body_style = body;
    }
    add_lib(sheet, "U2", pins);
}

fn child_with_pins(sheet: &mut SchematicSheet, at: &[Point]) {
    sheet.child_sheets.push(ChildSheet {
        uuid: Uuid::now_v7(),
        name: "Child".into(),
        filename: "child.snxsch".into(),
        position: pt(30.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: at
            .iter()
            .enumerate()
            .map(|(i, p)| SheetPin {
                uuid: Uuid::now_v7(),
                name: format!("P{i}"),
                direction: "input".into(),
                position: *p,
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            })
            .collect(),
        instances: Vec::new(),
    });
}

fn labelled(text: &str, at: Point, kind: LabelType) -> Label {
    Label::new(text, at, kind)
}

/// The derive of `cand` from a base of `before` equals the full derive, and
/// is not the full derive in disguise.
fn assert_same(before: &SchematicSheet, cand: &SchematicSheet) {
    let base = NetBase::new(before);
    assert_eq!(base.derive(before), net_signatures(before), "the base");
    assert!(
        base.derives_incrementally(cand),
        "fell back to the full derive"
    );
    assert_eq!(base.derive(cand), net_signatures(cand), "the edit");
}

fn two_resistors() -> SchematicSheet {
    let mut sheet = empty_sheet();
    libs(&mut sheet);
    place(&mut sheet, "R1", "R", pt(10.0, 10.0));
    place(&mut sheet, "R2", "R", pt(30.0, 10.0));
    sheet.wires.push(fresh_wire(pt(12.5, 10.0), pt(27.5, 10.0)));
    sheet.wires.push(fresh_wire(pt(7.5, 10.0), pt(0.0, 10.0)));
    sheet
}

#[test]
fn a_rubber_banded_wire_follows_its_pin() {
    let before = two_resistors();
    let mut cand = before.clone();
    cand.symbols[0].position = pt(10.0, 20.0);
    cand.wires[0].start = pt(12.5, 20.0);
    cand.wires[0].end = pt(27.5, 20.0);
    cand.wires.push(fresh_wire(pt(27.5, 20.0), pt(27.5, 10.0)));
    cand.wires[1].start = pt(7.5, 20.0);
    cand.wires[1].end = pt(0.0, 20.0);
    assert_same(&before, &cand);
}

#[test]
fn a_pin_sliding_onto_a_wire_interior_with_and_without_a_dot() {
    let mut before = two_resistors();
    before.wires.push(fresh_wire(pt(20.0, 0.0), pt(20.0, 20.0)));
    let mut cand = before.clone();
    cand.symbols[0].position = pt(17.5, 10.0);
    assert_same(&before, &cand);
    cand.junctions.push(fresh_dot(pt(20.0, 10.0)));
    cand.symbols[0].position = pt(17.5, 10.0);
    assert_same(&before, &cand);
    cand.symbols[0].position = pt(10.0, 10.0);
    assert_same(&before, &cand);
}

#[test]
fn a_dot_creating_a_t_junction_merges_two_nets() {
    let mut before = two_resistors();
    before
        .wires
        .push(fresh_wire(pt(20.0, 10.0), pt(20.0, 20.0)));
    let mut cand = before.clone();
    cand.junctions.push(fresh_dot(pt(20.0, 10.0)));
    assert_same(&before, &cand);
}

#[test]
fn a_moved_wire_end_landing_on_another_nets_pin() {
    let mut before = two_resistors();
    before.wires.push(fresh_wire(pt(0.0, 30.0), pt(10.0, 30.0)));
    let mut cand = before.clone();
    cand.wires[2].end = pt(12.5, 10.0);
    assert_same(&before, &cand);
    cand.wires[2].start = pt(27.5, 10.0);
    assert_same(&before, &cand);
}

#[test]
fn labels_join_nets_by_name_and_leave_them() {
    let mut before = two_resistors();
    before.wires.push(fresh_wire(pt(0.0, 30.0), pt(10.0, 30.0)));
    before
        .labels
        .push(labelled("A", pt(15.0, 10.0), LabelType::Net));
    before
        .labels
        .push(labelled("A", pt(5.0, 30.0), LabelType::Net));
    before
        .labels
        .push(labelled("A", pt(3.0, 30.0), LabelType::Hierarchical));
    let mut cand = before.clone();
    cand.labels[1].position = pt(50.0, 50.0);
    assert_same(&before, &cand);
    cand.labels[1].position = pt(5.0, 30.0);
    cand.labels[0].text = "B".into();
    assert_same(&before, &cand);
    cand.labels[0].label_type = LabelType::Global;
    cand.labels[1].label_type = LabelType::Global;
    cand.labels[0].text = "A".into();
    assert_same(&before, &cand);
    cand.labels.remove(0);
    assert_same(&before, &cand);
}

#[test]
fn power_ports_join_move_and_clash() {
    let mut before = empty_sheet();
    libs(&mut before);
    before.wires.push(fresh_wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    before.wires.push(fresh_wire(pt(0.0, 20.0), pt(10.0, 20.0)));
    place(&mut before, "R1", "R", pt(0.0, 0.0));
    place_power(&mut before, "#1", "P", "GND", pt(10.0, 0.0));
    place_power(&mut before, "#2", "P", "GND", pt(10.0, 20.0));
    place_power(&mut before, "#3", "P", "VCC", pt(50.0, 50.0));
    place_power(&mut before, "#4", "P", "GND", pt(50.0, 50.0));
    place_power(
        &mut before,
        "#5",
        "power:PWR_FLAG",
        "PWR_FLAG",
        pt(10.0, 0.0),
    );
    let mut cand = before.clone();
    cand.symbols[2].position = pt(40.0, 20.0);
    assert_same(&before, &cand);
    // A wire end lands on the clashing tip: the short becomes real.
    let mut cand = before.clone();
    cand.wires.push(fresh_wire(pt(50.0, 50.0), pt(60.0, 50.0)));
    assert_same(&before, &cand);
    let mut cand = before.clone();
    cand.symbols.remove(1);
    cand.symbols[1].value = "VCC".into();
    assert_same(&before, &cand);
}

#[test]
fn child_sheet_pins_anchor_to_wires_and_follow_them() {
    let mut before = empty_sheet();
    libs(&mut before);
    before.wires.push(fresh_wire(pt(0.0, 0.0), pt(20.0, 0.0)));
    before.wires.push(fresh_wire(pt(0.0, 10.0), pt(20.0, 10.0)));
    child_with_pins(&mut before, &[pt(10.0, 0.0), pt(30.0, 10.0)]);
    let mut cand = before.clone();
    cand.child_sheets[0].pins[1].position = pt(10.0, 10.0);
    assert_same(&before, &cand);
    cand.wires[0].end = pt(5.0, 0.0);
    assert_same(&before, &cand);
    cand.child_sheets[0].pins[0].position = pt(5.0, 0.0);
    assert_same(&before, &cand);
}

#[test]
fn buses_and_bus_entries_are_not_nets() {
    let before = two_resistors();
    let mut cand = before.clone();
    cand.buses.push(signex_types::schematic::Bus {
        uuid: Uuid::now_v7(),
        start: pt(12.5, 10.0),
        end: pt(12.5, 40.0),
    });
    cand.bus_entries.push(signex_types::schematic::BusEntry {
        uuid: Uuid::now_v7(),
        position: pt(12.5, 20.0),
        size: (2.5, 2.5),
    });
    assert_same(&before, &cand);
}

#[test]
fn no_connects_gate_a_pin() {
    let before = two_resistors();
    let mut cand = before.clone();
    cand.symbols[0].position = pt(10.0, 30.0);
    cand.no_connects.push(NoConnect {
        uuid: Uuid::now_v7(),
        position: pt(12.5, 30.0),
    });
    assert_same(&before, &cand);
}

/// A queried point that is already a node of another net (a pin tip held by
/// a no-connect, lying inside a wire without a dot) is answered by the wire's
/// net and merges nothing (#2144): the case
/// `random_edits_derive_the_same_nets_as_the_full_derive` met at seed 0xa11ce,
/// round 19, step 2.
#[test]
fn a_queried_pin_tip_inside_a_wire_reports_the_wire_like_the_full_derive() {
    let mut before = empty_sheet();
    libs(&mut before);
    before.wires.push(fresh_wire(pt(0.0, 0.0), pt(20.0, 0.0)));
    before
        .labels
        .push(labelled("B", pt(0.0, 0.0), LabelType::Net));
    place(&mut before, "R1", "R", pt(12.5, 0.0));
    before.no_connects.push(NoConnect {
        uuid: Uuid::now_v7(),
        position: pt(10.0, 0.0),
    });
    let mut cand = before.clone();
    cand.labels[0].text = "C".into();
    let base = NetBase::new(&before);
    let tip = pt_key(&pt(10.0, 0.0));
    let far = pt_key(&pt(15.0, 5.0));
    let (nets, found) = net_signatures_at(&before, &[(tip, true), (tip, false)]);
    let named = |name: &str| nets.iter().position(|n| n.name == name);
    assert_eq!(found[0].1, named("B"), "the tip is on the wire: {nets:?}");
    assert_ne!(
        found[1].1,
        named("B"),
        "off the wire it is the pin's own net"
    );
    for at in [
        vec![(tip, true)],
        vec![(tip, false)],
        vec![(far, true), (tip, true), (pt_key(&pt(5.0, 0.0)), true)],
        vec![(tip, true), (pt_key(&pt(5.0, 0.0)), true)],
    ] {
        for sheet in [&before, &cand] {
            assert_eq!(
                base.derive_at(sheet, &at),
                net_signatures_at(sheet, &at),
                "{at:?}"
            );
        }
    }
}

/// The same by a power-port tip: a port inside a wire with no dot, a second
/// port of its value elsewhere, and queries on the port tip and on the wire.
/// The port's tip, queried as on the wire, reports the wire's net; the GND net
/// keeps its own answer, whatever the query order (#2144).
#[test]
fn a_queried_power_port_tip_inside_a_wire_reports_the_wire_like_the_full_derive() {
    let mut before = empty_sheet();
    libs(&mut before);
    before.wires.push(fresh_wire(pt(0.0, 0.0), pt(20.0, 0.0)));
    before
        .labels
        .push(labelled("B", pt(0.0, 0.0), LabelType::Net));
    place_power(&mut before, "#1", "P", "GND", pt(10.0, 0.0));
    place_power(&mut before, "#2", "P", "GND", pt(30.0, 10.0));
    before
        .wires
        .push(fresh_wire(pt(30.0, 10.0), pt(40.0, 10.0)));
    before
        .labels
        .push(labelled("X", pt(40.0, 10.0), LabelType::Net));
    let mut cand = before.clone();
    cand.symbols[1].position = pt(30.0, 20.0);
    let base = NetBase::new(&before);
    let port = pt_key(&pt(10.0, 0.0));
    let wire = pt_key(&pt(5.0, 0.0));
    let elsewhere = pt_key(&pt(30.0, 10.0));
    let (nets, found) =
        net_signatures_at(&before, &[(port, true), (elsewhere, false), (wire, true)]);
    let named = |name: &str| nets.iter().position(|n| n.name == name);
    assert_eq!(found[0].1, named("B"), "the tip is on the wire: {nets:?}");
    assert_eq!(
        found[1].1,
        named("GND"),
        "the GND net is not merged: {nets:?}"
    );
    assert_eq!(found[2].1, named("B"));
    assert_ne!(named("B"), named("GND"));
    let (_, alone) = net_signatures_at(&before, &[(elsewhere, false)]);
    assert_eq!(
        alone[0], found[1],
        "an earlier query changes no later answer"
    );
    for at in [
        vec![(port, true), (wire, true)],
        vec![(wire, true), (port, true)],
        vec![(port, true), (elsewhere, false), (wire, true)],
        vec![(port, false), (wire, true)],
    ] {
        for sheet in [&before, &cand] {
            assert_eq!(
                base.derive_at(sheet, &at),
                net_signatures_at(sheet, &at),
                "{at:?}"
            );
        }
    }
}

#[test]
fn a_repeated_wire_id_in_the_base_falls_back_to_the_full_derive() {
    let mut before = two_resistors();
    let id = before.wires[0].uuid;
    before.wires[1].uuid = id;
    let mut cand = before.clone();
    cand.symbols[0].position = pt(10.0, 20.0);
    cand.wires[0].start = pt(12.5, 20.0);
    let base = NetBase::new(&before);
    assert!(!base.derives_incrementally(&cand));
    assert_eq!(base.derive(&cand), net_signatures(&cand));
}

#[test]
fn a_changed_library_falls_back_to_the_full_derive() {
    let before = two_resistors();
    let mut cand = before.clone();
    add_lib(
        &mut cand,
        "R",
        vec![lib_pin("9", pt(-2.5, 0.0), PinDirection::Passive)],
    );
    let base = NetBase::new(&before);
    assert!(!base.derives_incrementally(&cand));
    assert_eq!(base.derive(&cand), net_signatures(&cand));
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % bound
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next(100) < percent
    }

    fn spot(&mut self) -> Point {
        pt(2.5 * self.next(9) as f64, 2.5 * self.next(7) as f64)
    }
}

const TEXTS: [&str; 5] = ["A", "B", "GND", "VCC", ""];
const KINDS: [LabelType; 4] = [
    LabelType::Net,
    LabelType::Global,
    LabelType::Power,
    LabelType::Hierarchical,
];

fn add_random_symbol(sheet: &mut SchematicSheet, rng: &mut Lcg, index: usize) {
    let at = rng.spot();
    let reference = format!("X{index}");
    match rng.next(12) {
        0..=4 => place(sheet, &reference, "R", at),
        5..=7 => {
            let value = TEXTS[rng.next(4) as usize];
            place_power(sheet, &reference, "P", value, at);
        }
        8 => place_power(sheet, &reference, "power:PWR_FLAG", "PWR_FLAG", at),
        10 | 11 => {
            place(sheet, &reference, "U2", at);
            let sym = sheet.symbols.last_mut().expect("just placed");
            sym.unit = 1 + rng.next(2) as u32;
        }
        _ => place_power(sheet, &reference, "P", "", at),
    }
    let sym = sheet.symbols.last_mut().expect("just placed");
    sym.rotation = 90.0 * rng.next(4) as f64;
    sym.mirror_x = rng.chance(20);
    sym.mirror_y = rng.chance(20);
}

fn random_sheet(rng: &mut Lcg) -> SchematicSheet {
    let mut sheet = empty_sheet();
    libs(&mut sheet);
    for index in 0..(2 + rng.next(8) as usize) {
        add_random_symbol(&mut sheet, rng, index);
    }
    for _ in 0..(3 + rng.next(10)) {
        let (a, b) = (rng.spot(), rng.spot());
        let elbow = pt(b.x, a.y);
        for (p, q) in [(a, elbow), (elbow, b)] {
            if p != q {
                sheet.wires.push(fresh_wire(p, q));
            }
        }
        if rng.chance(10) && a != b {
            sheet.wires.push(fresh_wire(a, b));
        }
    }
    for _ in 0..rng.next(5) {
        sheet.junctions.push(fresh_dot(rng.spot()));
    }
    for _ in 0..(1 + rng.next(6)) {
        let text = TEXTS[rng.next(5) as usize];
        let kind = KINDS[rng.next(4) as usize];
        sheet.labels.push(labelled(text, rng.spot(), kind));
    }
    for _ in 0..rng.next(3) {
        sheet.no_connects.push(NoConnect {
            uuid: Uuid::now_v7(),
            position: rng.spot(),
        });
    }
    if rng.chance(40) {
        let pins: Vec<Point> = (0..1 + rng.next(3)).map(|_| rng.spot()).collect();
        child_with_pins(&mut sheet, &pins);
    }
    sheet
}

/// One random edit of the kind a move, a reroute or an undo makes.
fn edit(sheet: &mut SchematicSheet, rng: &mut Lcg) {
    let shift = |p: &mut Point, rng: &mut Lcg| {
        p.x += 2.5 * (rng.next(5) as f64 - 2.0);
        p.y += 2.5 * (rng.next(5) as f64 - 2.0);
    };
    let pick = |len: usize, rng: &mut Lcg| (len > 0).then(|| rng.next(len as u64) as usize);
    match rng.next(17) {
        0 | 1 => {
            if let Some(i) = pick(sheet.symbols.len(), rng) {
                shift(&mut sheet.symbols[i].position, rng);
            }
        }
        2 => {
            if let Some(i) = pick(sheet.symbols.len(), rng) {
                sheet.symbols[i].rotation += 90.0;
                sheet.symbols[i].mirror_y ^= rng.chance(50);
            }
        }
        3 => {
            if let Some(i) = pick(sheet.symbols.len(), rng) {
                sheet.symbols.remove(i);
            }
        }
        4 => {
            let index = sheet.symbols.len() + 50;
            add_random_symbol(sheet, rng, index);
        }
        5 | 6 => {
            if let Some(i) = pick(sheet.wires.len(), rng) {
                let w = &mut sheet.wires[i];
                if rng.chance(50) {
                    shift(&mut w.start, rng);
                }
                if rng.chance(50) {
                    shift(&mut w.end, rng);
                }
            }
        }
        7 => {
            if let Some(i) = pick(sheet.wires.len(), rng) {
                sheet.wires.remove(i);
            }
        }
        8 => {
            let (a, b) = (rng.spot(), rng.spot());
            sheet.wires.push(fresh_wire(a, b));
        }
        9 => {
            if let Some(i) = pick(sheet.junctions.len(), rng) {
                if rng.chance(50) {
                    sheet.junctions.remove(i);
                } else {
                    shift(&mut sheet.junctions[i].position, rng);
                }
            } else {
                sheet.junctions.push(fresh_dot(rng.spot()));
            }
        }
        10 | 11 => {
            if let Some(i) = pick(sheet.labels.len(), rng) {
                match rng.next(4) {
                    0 => shift(&mut sheet.labels[i].position, rng),
                    1 => sheet.labels[i].text = TEXTS[rng.next(5) as usize].to_string(),
                    2 => sheet.labels[i].label_type = KINDS[rng.next(4) as usize],
                    _ => {
                        sheet.labels.remove(i);
                    }
                }
            }
        }
        12 => sheet.labels.push(labelled(
            TEXTS[rng.next(5) as usize],
            rng.spot(),
            KINDS[rng.next(4) as usize],
        )),
        13 => {
            if let Some(i) = pick(sheet.no_connects.len(), rng) {
                sheet.no_connects.remove(i);
            } else {
                sheet.no_connects.push(NoConnect {
                    uuid: Uuid::now_v7(),
                    position: rng.spot(),
                });
            }
        }
        14 => {
            if let Some(child) = sheet.child_sheets.first_mut()
                && !child.pins.is_empty()
            {
                let i = rng.next(child.pins.len() as u64) as usize;
                shift(&mut child.pins[i].position, rng);
            }
        }
        16 => {
            if let Some(i) = pick(sheet.symbols.len(), rng) {
                sheet.symbols[i].unit = 1 + rng.next(2) as u32;
                sheet.symbols[i].body_style = if rng.chance(50) {
                    BodyStyle::Normal
                } else {
                    BodyStyle::Alternate
                };
            }
        }
        _ => {
            if let Some(i) = pick(sheet.symbols.len(), rng) {
                sheet.symbols[i].value = TEXTS[rng.next(4) as usize].to_string();
            }
        }
    }
}

fn dump(sheet: &SchematicSheet) -> String {
    let wires: Vec<_> = sheet
        .wires
        .iter()
        .map(|w| format!("({},{})-({},{})", w.start.x, w.start.y, w.end.x, w.end.y))
        .collect();
    let symbols: Vec<_> = sheet
        .symbols
        .iter()
        .map(|s| {
            format!(
                "{}@({},{}) r{} {}{}",
                s.lib_id,
                s.position.x,
                s.position.y,
                s.rotation,
                s.value,
                if s.mirror_y { " my" } else { "" }
            )
        })
        .collect();
    let labels: Vec<_> = sheet
        .labels
        .iter()
        .map(|l| {
            format!(
                "{:?}{:?}@({},{})",
                l.label_type, l.text, l.position.x, l.position.y
            )
        })
        .collect();
    let dots: Vec<_> = sheet
        .junctions
        .iter()
        .map(|j| format!("({},{})", j.position.x, j.position.y))
        .collect();
    format!("wires {wires:?} symbols {symbols:?} labels {labels:?} dots {dots:?}")
}

#[test]
fn random_edits_derive_the_same_nets_as_the_full_derive() {
    for seed in [0xA11CE, 0xB0B, 0x5EED, 0xFEED_F00D] {
        let mut rng = Lcg(seed);
        for round in 0..1500 {
            let before = random_sheet(&mut rng);
            let base = NetBase::new(&before);
            assert_eq!(base.derive(&before), net_signatures(&before), "the base");
            let mut cand = before.clone();
            for step in 0..4 {
                for _ in 0..(1 + rng.next(4)) {
                    edit(&mut cand, &mut rng);
                }
                let at: Vec<_> = (0..4)
                    .map(|_| (pt_key(&rng.spot()), rng.chance(50)))
                    .chain(
                        cand.child_sheets
                            .iter()
                            .flat_map(|c| c.pins.iter().map(|p| (pt_key(&p.position), true))),
                    )
                    .collect();
                let where_ = format!("seed {seed:#x} round {round} step {step}");
                assert!(
                    base.derives_incrementally(&cand),
                    "{where_}: fell back\nbefore {}\nafter {}",
                    dump(&before),
                    dump(&cand)
                );
                assert_eq!(
                    base.derive_at(&cand, &at),
                    net_signatures_at(&cand, &at),
                    "{where_}\nbefore {}\nafter {}",
                    dump(&before),
                    dump(&cand)
                );
            }
        }
    }
}
