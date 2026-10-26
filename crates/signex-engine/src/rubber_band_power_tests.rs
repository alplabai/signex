//! Rubber-band moves of power ports (#1031, #1177): every port of one value
//! is one net, so the guard judges a kept port's net together with the other
//! nets that value names, and names it as the netlist does.

use crate::Engine;
use crate::rubber_band_tests::{
    add_label, add_resistor, add_wire, assert_clean, has, lib_pin, lib_symbol, mv, net_shape,
    orthogonal, place, segments, symbol_item,
};
use crate::test_support::test_sheet;
use signex_types::schematic::{NoConnect, Point, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

fn add_power_port(sheet: &mut SchematicSheet, reference: &str, value: &str, at: Point) -> Uuid {
    sheet
        .lib_symbols
        .entry(value.to_string())
        .or_insert_with(|| lib_symbol(value, vec![lib_pin("1", 0.0)]));
    let uuid = place(sheet, reference, value, at);
    let sym = sheet.symbols.iter_mut().find(|s| s.uuid == uuid).unwrap();
    sym.is_power = true;
    sym.value = value.to_string();
    uuid
}

/// The netlist name and terminals of the net holding `symbol`'s `pin`.
fn net_of(sheet: &SchematicSheet, symbol: Uuid, pin: &str) -> (String, Vec<(Uuid, String)>) {
    let netlist = signex_net::build_netlist(sheet);
    let net = netlist
        .nets
        .iter()
        .find(|n| {
            n.terminals
                .iter()
                .any(|t| t.symbol == symbol && t.pin == pin)
        })
        .unwrap_or_else(|| panic!("{symbol} pin {pin} is on no net"));
    let members = net
        .terminals
        .iter()
        .map(|t| (t.symbol, t.pin.clone()))
        .collect();
    (net.name.clone(), members)
}

/// A GND port at (30, 50) wired to pin 1 of R1 at (50, 50): `(port, R1)`.
fn wired_gnd_port(sheet: &mut SchematicSheet) -> (Uuid, Uuid) {
    let port = add_power_port(sheet, "#PWR1", "GND", Point::new(30.0, 50.0));
    let r1 = add_resistor(sheet, "R1", Point::new(50.0, 50.0)); // pins 45, 55
    add_wire(sheet, (30.0, 50.0), (45.0, 50.0));
    (port, r1)
}

/// Move `port` together with an unrelated labelled wire's label by (0, 10),
/// keeping wires. The moved label makes the guard compare against the
/// user's intent rather than the pre-move nets.
fn move_with_unrelated_label(mut sheet: SchematicSheet, port: Uuid) -> Engine {
    add_wire(&mut sheet, (100.0, 100.0), (120.0, 100.0));
    add_label(&mut sheet, "X", (120.0, 100.0));
    let label = sheet.labels.last().unwrap().uuid;
    let mut e = Engine::new(sheet).unwrap();
    mv(
        &mut e,
        vec![
            symbol_item(port),
            SelectedItem::new(label, SelectedKind::Label),
        ],
        0.0,
        10.0,
        true,
    );
    e
}

/// No notice, orthogonal wires, and R1 pin 1 on a net named `name` that also
/// holds pin 1 of each of `with`. Returns that net's terminals.
fn assert_kept(e: &mut Engine, r1: Uuid, name: &str, with: &[Uuid]) -> Vec<(Uuid, String)> {
    let warnings = e.take_warnings();
    assert!(warnings.is_empty(), "false warning: {warnings:?}");
    let segs = segments(e);
    assert!(segs.iter().all(orthogonal), "diagonal wire: {segs:?}");
    let (got, members) = net_of(e.document(), r1, "1");
    assert_eq!(got, name, "{members:?}");
    for symbol in with {
        assert!(
            members.contains(&(*symbol, "1".to_string())),
            "{symbol} not on R1's net: {members:?}"
        );
    }
    members
}

/// R1, its wires, a wired GND port and the VCC label are all in the move. The
/// port stays in the move, so the guard used to fall back to the pre-move
/// nets, which a moved sheet cannot match: R1's wires were dropped and false
/// warnings logged.
fn power_port_fixture() -> (Engine, Vec<SelectedItem>) {
    let mut sheet = test_sheet();
    let gnd = add_power_port(&mut sheet, "#PWR1", "GND", Point::new(30.0, 50.0));
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0)); // pins 45, 55
    let w1 = add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (55.0, 50.0), (70.0, 50.0));
    add_label(&mut sheet, "VCC", (70.0, 50.0));
    let label = sheet.labels[0].uuid;
    let items = vec![
        symbol_item(r1),
        symbol_item(gnd),
        SelectedItem::new(w1, SelectedKind::Wire),
        SelectedItem::new(label, SelectedKind::Label),
    ];
    (Engine::new(sheet).unwrap(), items)
}

#[test]
fn a_wired_power_port_moving_with_the_symbol_does_not_drop_its_wires() {
    let (mut e, items) = power_port_fixture();
    let (r1, gnd) = (items[0].uuid, items[1].uuid);
    mv(&mut e, items, 0.0, 10.0, true);
    let warnings = e.take_warnings();
    assert!(warnings.is_empty(), "false warning: {warnings:?}");
    let segs = segments(&e);
    assert!(has(&segs, (30.0, 60.0), (45.0, 60.0)), "{segs:?}");
    assert!(
        segs.iter()
            .any(|s| s.0 == (55.0, 60.0) || s.1 == (55.0, 60.0)),
        "{segs:?}"
    );
    let (name, members) = net_of(e.document(), r1, "1");
    assert_eq!(name, "GND", "{members:?}");
    assert!(members.contains(&(gnd, "1".to_string())), "{members:?}");
}

#[test]
fn a_wired_power_port_moving_with_just_the_symbol_keeps_the_nets() {
    let (mut e, items) = power_port_fixture();
    let before = net_shape(e.document());
    mv(&mut e, vec![items[0], items[1]], 0.0, 10.0, true);
    let warnings = e.take_warnings();
    assert!(warnings.is_empty(), "false warning: {warnings:?}");
    assert_clean(&mut e, &before);
}

/// A sheet usually has many GND ports. #PWR1 (wired to R1) moves; #PWR2
/// (wired to R2) stays. The kept port must take R1 back into the one GND
/// net, not leave a second one.
#[test]
fn a_kept_power_port_rejoins_the_other_ports_of_its_value() {
    let mut sheet = test_sheet();
    let (pwr1, r1) = wired_gnd_port(&mut sheet);
    let pwr2 = add_power_port(&mut sheet, "#PWR2", "GND", Point::new(30.0, 150.0));
    let r2 = add_resistor(&mut sheet, "R2", Point::new(50.0, 150.0));
    add_wire(&mut sheet, (30.0, 150.0), (45.0, 150.0));
    let mut e = move_with_unrelated_label(sheet, pwr1);
    assert_kept(&mut e, r1, "GND", &[pwr1, pwr2, r2]);
}

/// The kept port's own net also carries a local label. A power port outranks
/// a `Net` label, so the netlist names the net after the port; the intent
/// used to keep the label's name and the move fell back with a false notice.
#[test]
fn a_kept_power_port_names_its_net_over_a_local_label() {
    let mut sheet = test_sheet();
    let (pwr1, r1) = wired_gnd_port(&mut sheet);
    add_label(&mut sheet, "FOO", (40.0, 50.0));
    let mut e = move_with_unrelated_label(sheet, pwr1);
    assert_kept(&mut e, r1, "GND", &[pwr1]);
}

/// A GND port touching a VCC port with no anchor joins no net by name
/// (`signex_net::touching_port_conflicts`), while a no-connect on the tip
/// still makes its pin a terminal. The intent used to pull it into the kept
/// port's GND net, and the move fell back with a false notice.
#[test]
fn a_conflicted_port_of_the_same_value_is_not_joined() {
    let mut sheet = test_sheet();
    let (pwr1, r1) = wired_gnd_port(&mut sheet);
    let pwr3 = add_power_port(&mut sheet, "#PWR3", "GND", Point::new(200.0, 200.0));
    add_power_port(&mut sheet, "#PWR4", "VCC", Point::new(200.0, 200.0));
    sheet.no_connects.push(NoConnect {
        uuid: Uuid::new_v4(),
        position: Point::new(200.0, 200.0),
    });
    let mut e = move_with_unrelated_label(sheet, pwr1);
    let members = assert_kept(&mut e, r1, "GND", &[pwr1]);
    assert!(!members.contains(&(pwr3, "1".to_string())), "{members:?}");
}
