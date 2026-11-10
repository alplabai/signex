//! #1312 — the net rows of the change list.

use signex_types::part::PinPadMap;
use signex_types::pcb::PcbBoard;

use super::fixture::*;
use super::*;
use crate::pcb::BoardEngine;

pub(super) fn parts() -> Vec<EcoPart> {
    vec![
        part("R1", &[0x101], "10k", Ok(r0603())),
        part("R2", &[0x102], "4k7", Ok(r0603())),
    ]
}

pub(super) fn wired() -> signex_types::net::Netlist {
    netlist(&[
        ("VCC", &[(0x101, "R1", "1")]),
        ("SIG", &[(0x101, "R1", "2"), (0x102, "R2", "1")]),
        ("GND", &[(0x102, "R2", "2")]),
    ])
}

pub(super) fn placed() -> PcbBoard {
    board(vec![
        board_fp(0xa1, "R1", "10k", &[0x101], &r0603(), (10.0, 10.0)),
        board_fp(0xa2, "R2", "4k7", &[0x102], &r0603(), (20.0, 10.0)),
    ])
}

pub(super) fn execute(design: &mut Design, list: &EcoChangeList) {
    let mut engine = BoardEngine::new(design.board.clone(), design.revision);
    let commands = lower(list, engine.board());
    engine
        .execute_labelled(commands, "Update PCB", list.revision)
        .unwrap();
    design.revision = engine.revision();
    design.board = engine.into_board();
}

pub(super) fn synced() -> Design {
    let mut design = Design::new(placed(), parts(), wired());
    let list = design.diff();
    execute(&mut design, &list);
    assert!(design.diff().rows.is_empty());
    design
}

pub(super) fn net_of<'a>(board: &'a PcbBoard, reference: &str, pad: &str) -> Option<&'a str> {
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

// ---- Review of #2265: no pad may be left naming a net the table lacks ----

/// Every pad's net is in the board's net table under the same name.
pub(super) fn assert_consistent(board: &PcbBoard) {
    for fp in &board.footprints {
        for pad in &fp.pads {
            if let Some(net) = &pad.net {
                let in_table = board.nets.iter().find(|n| n.number == net.number);
                assert_eq!(
                    in_table.map(|n| n.name.as_str()),
                    Some(net.name.as_str()),
                    "{}.{} names net {} {:?}, the table disagrees",
                    fp.reference,
                    pad.number,
                    net.number,
                    net.name
                );
            }
        }
    }
}

pub(super) fn number_of(board: &PcbBoard, name: &str) -> u32 {
    board.nets.iter().find(|n| n.name == name).unwrap().number
}

pub(super) fn pad_key(board: &PcbBoard, reference: &str, number: &str) -> EcoKey {
    let fp = board
        .footprints
        .iter()
        .find(|f| f.reference == reference)
        .unwrap();
    let pad = fp.pads.iter().find(|p| p.number == number).unwrap();
    EcoKey::PadNet {
        footprint: fp.uuid,
        pad: pad.uuid,
    }
}

/// Probe 1: a locked pad still follows a rename of its own net, as net sync
/// renames it, so it never keeps the old name after the table moves on.
#[test]
fn a_locked_pad_follows_a_rename_of_its_own_net() {
    let mut design = synced();
    design.board.footprints[1].pads[0].net_locked = true;
    design.netlist.nets[1].name = "DATA".into();
    let sig = number_of(&design.board, "SIG");
    let list = design.diff();
    let row = list
        .row(&pad_key(&design.board, "R2", "1"))
        .expect("a rename row for the locked pad");
    assert!(row.needs.contains(&EcoKey::RenameNet(sig)));
    execute(&mut design, &list);
    assert_eq!(net_of(&design.board, "R2", "1"), Some("DATA"));
    assert!(
        design.board.footprints[1].pads[0].net_locked,
        "still locked"
    );
    assert_consistent(&design.board);
}

/// Probe 1, the other half: a locked pad whose net NUMBER would change still
/// gets no row and keeps its net.
#[test]
fn a_locked_pad_never_moves_to_another_net() {
    let mut design = synced();
    design.board.footprints[1].pads[1].net_locked = true;
    design.netlist = netlist(&[
        ("VCC", &[(0x101, "R1", "1"), (0x102, "R2", "2")]),
        ("SIG", &[(0x101, "R1", "2"), (0x102, "R2", "1")]),
    ]);
    let list = design.diff();
    assert!(list.row(&pad_key(&design.board, "R2", "2")).is_none());
    execute(&mut design, &list);
    assert_eq!(net_of(&design.board, "R2", "2"), Some("GND"));
    assert_consistent(&design.board);
}

/// Probe 2: unticking the pad row that empties a net keeps the net, so the
/// pad never names a net the table dropped.
#[test]
fn an_unticked_pad_row_keeps_the_net_it_would_empty() {
    let mut design = synced();
    design.netlist = netlist(&[
        ("VCC", &[(0x101, "R1", "1"), (0x102, "R2", "2")]),
        ("SIG", &[(0x101, "R1", "2"), (0x102, "R2", "1")]),
    ]);
    let gnd = number_of(&design.board, "GND");
    let mut list = design.diff();
    let remove = list.row(&EcoKey::RemoveNet(gnd)).unwrap().clone();
    assert!(list.applies(&remove));
    list.set_on(&pad_key(&design.board, "R2", "2"), false);
    assert!(!list.applies(&remove), "the remove needs the pad row");
    execute(&mut design, &list);
    assert_eq!(net_of(&design.board, "R2", "2"), Some("GND"));
    assert_eq!(number_of(&design.board, "GND"), gnd);
    assert_consistent(&design.board);
}

/// Probe 2, renames: an unticked rename takes its pad rows with it, and an
/// unticked pad row that only follows the rename takes the rename with it.
#[test]
fn a_rename_and_the_pad_rows_that_follow_it_go_together() {
    let mut design = synced();
    design.netlist.nets[1].name = "DATA".into();
    let sig = number_of(&design.board, "SIG");

    let mut list = design.diff();
    list.set_on(&EcoKey::RenameNet(sig), false);
    let mut unticked_rename = design.clone();
    execute(&mut unticked_rename, &list);
    assert_eq!(net_of(&unticked_rename.board, "R2", "1"), Some("SIG"));
    assert_consistent(&unticked_rename.board);

    let mut list = design.diff();
    list.set_on(&pad_key(&design.board, "R1", "2"), false);
    let rename = list.row(&EcoKey::RenameNet(sig)).unwrap().clone();
    assert!(!list.applies(&rename), "the rename needs its pad rows");
    execute(&mut design, &list);
    assert_eq!(net_of(&design.board, "R1", "2"), Some("SIG"));
    assert_eq!(net_of(&design.board, "R2", "1"), Some("SIG"));
    assert_consistent(&design.board);
}

/// Probe 3: Refresh carries the footprint ticks and then recomputes the net
/// rows for them, so a kept part's only net is not removed under its pad.
#[test]
fn a_refresh_recomputes_the_net_rows_for_the_carried_ticks() {
    let mut design = synced();
    // R2 is not on the board, and its pins are out of the netlist with it:
    // GND, on R2 only, goes.
    design.parts[1].on_board = false;
    design
        .netlist
        .nets
        .iter_mut()
        .for_each(|n| n.terminals.retain(|t| t.reference != "R2"));
    design.netlist.nets.retain(|n| !n.terminals.is_empty());
    let gnd = number_of(&design.board, "GND");
    let mut list = design.diff();
    assert!(list.row(&EcoKey::RemoveNet(gnd)).is_some());
    // The user unticks the removal, then presses Refresh.
    list.set_on(&EcoKey::RemoveFootprint(u(0xa2)), false);
    let mut refreshed = design.diff();
    refreshed.keep_choices_from(&list, &design.input());
    assert!(!refreshed.row(&EcoKey::RemoveFootprint(u(0xa2))).unwrap().on);
    assert!(
        refreshed
            .row(&EcoKey::RemoveNet(gnd))
            .is_none_or(|r| !refreshed.applies(r)),
        "{}",
        report_text(&refreshed)
    );
    execute(&mut design, &refreshed);
    assert_eq!(net_of(&design.board, "R2", "2"), Some("GND"));
    assert_consistent(&design.board);
}

/// Review point 4: the pad rows of a replaced footprint need its change row;
/// review point 7: the nets of pads the new footprint lacks are noted.
#[test]
fn pad_rows_of_a_changed_footprint_need_the_change_and_lost_nets_are_noted() {
    let mut design = synced();
    let mut lettered = resolved("R0603-AB", 0x6ab, "1.0.0", &["A", "B"]);
    lettered.pin_map = PinPadMap::from_pairs([("1", "A"), ("2", "B")]);
    design.parts[0].footprint = Ok(lettered);
    let change = EcoKey::ChangeFootprint(u(0xa1));
    let mut list = design.diff();
    let pad_rows: Vec<&EcoRow> = list
        .rows
        .iter()
        .filter(|r| matches!(r.key, EcoKey::PadNet { footprint, .. } if footprint == u(0xa1)))
        .collect();
    assert_eq!(pad_rows.len(), 2, "{}", report_text(&list));
    assert!(pad_rows.iter().all(|r| r.needs.contains(&change)));
    for (pad, net) in [("1", "VCC"), ("2", "SIG")] {
        assert!(
            list.notes.iter().any(|n| matches!(
                n,
                EcoNote::PadLosesNet { reference, pad_number, net: lost, .. }
                    if reference == "R1" && pad_number == pad && lost == net
            )),
            "{}",
            report_text(&list)
        );
    }
    assert!(report_text(&list).contains("R1.1 is on net VCC and not on the new footprint"));

    assert!(list.set_on(&change, false));
    refresh_nets(&mut list, &design.input());
    assert!(
        !list
            .rows
            .iter()
            .any(|r| matches!(r.key, EcoKey::PadNet { footprint, .. } if footprint == u(0xa1))),
        "the old pads are in step already"
    );
}

/// A one-pin net with its own id (the fixture numbers ids by position).
fn net_with_id(id: u32, name: &str, pin: Pin<'_>) -> signex_types::net::Net {
    let mut net = netlist(&[(name, &[pin])]).nets.remove(0);
    net.id = signex_types::net::NetId(id);
    net
}

/// Review point 5: a new net is keyed by its name, and a carried tick needs
/// the same change.
#[test]
fn a_new_net_is_keyed_by_name_and_its_untick_survives_a_refresh() {
    let mut design = synced();
    design.parts.push(part("C1", &[0x301], "100n", Ok(r0603())));
    design
        .netlist
        .nets
        .push(net_with_id(10, "NEW", (0x301, "C1", "1")));
    let mut list = design.diff();
    let new = EcoKey::AddNet("NEW".into());
    assert!(list.row(&new).is_some(), "{}", report_text(&list));
    list.set_on(&new, false);
    // Another new net arrives before the refresh.
    design
        .netlist
        .nets
        .push(net_with_id(11, "AAA", (0x301, "C1", "2")));
    let mut refreshed = design.diff();
    refreshed.keep_choices_from(&list, &design.input());
    assert!(!refreshed.row(&new).unwrap().on, "the untick is carried");
    assert!(refreshed.row(&EcoKey::AddNet("AAA".into())).unwrap().on);
}

#[test]
fn a_changed_row_does_not_keep_an_old_tick() {
    let mut design = synced();
    design.parts[1].footprint = Ok(resolved("R0603", 0x603, "1.1.0", &["1", "2"]));
    let change = EcoKey::ChangeFootprint(u(0xa2));
    let mut list = design.diff();
    assert!(
        !list.row(&change).unwrap().on,
        "another version starts unticked"
    );
    list.set_on(&change, true);

    let mut same = design.diff();
    same.keep_choices_from(&list, &design.input());
    assert!(
        same.row(&change).unwrap().on,
        "the same change keeps its tick"
    );

    // The library moved on to 1.2.0: the user never ticked that one.
    design.parts[1].footprint = Ok(resolved("R0603", 0x603, "1.2.0", &["1", "2"]));
    let mut moved = design.diff();
    moved.keep_choices_from(&list, &design.input());
    assert!(!moved.row(&change).unwrap().on);
}
