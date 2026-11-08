//! Per-pad net lock: a locked pad is never changed by a sync, a pad the
//! schematic cannot speak for locks itself, and every edit undoes its lock.

use super::*;

fn locked_pad(pcb: &PcbBoard, reference: &str, number: &str) -> bool {
    pcb.footprints
        .iter()
        .find(|f| f.reference == reference)
        .and_then(|f| f.pads.iter().find(|p| p.number == number))
        .expect("pad on board")
        .net_locked
}

fn ids(pcb: &PcbBoard, reference: &str, number: &str) -> (Uuid, Uuid) {
    let fp = pcb
        .footprints
        .iter()
        .find(|f| f.reference == reference)
        .expect("footprint");
    let pad = fp.pads.iter().find(|p| p.number == number).expect("pad");
    (fp.uuid, pad.uuid)
}

#[test]
fn a_locked_matching_pad_is_untouched_across_a_renumbering_resync() {
    let (before, after) = divider_netlists();
    let mut pcb = board(vec![
        footprint("R1", &["1", "2"]),
        footprint("R2", &["1", "2"]),
    ]);
    link_footprints(&mut pcb, &before);
    assign_pad_nets(&before, &pcb, &SymbolPadKey::for_board(&pcb))
        .edit
        .apply(&mut pcb);
    let hand = PadNet {
        number: 9,
        name: "HAND".to_string(),
    };
    let (fp, pad) = ids(&pcb, "R1", "2");
    assign_pad_net(&pcb, fp, pad, hand.clone()).apply(&mut pcb);
    assert!(locked_pad(&pcb, "R1", "2"));
    let mid = pad_net(&pcb, "R2", "1").cloned().expect("mid assigned");

    let again = assign_pad_nets(&after, &pcb, &SymbolPadKey::for_board(&pcb));
    assert!(
        again
            .issues
            .contains(&PadNetIssue::PadNetLocked { footprint: fp, pad }),
        "{:?}",
        again.issues
    );
    assert!(
        !again
            .issues
            .iter()
            .any(|i| matches!(i, PadNetIssue::TerminalWithoutPad { .. })),
        "{:?}",
        again.issues
    );
    assert!(again.edit.pads.iter().all(|c| c.pad != pad));
    again.edit.apply(&mut pcb);

    assert_eq!(pad_net(&pcb, "R1", "2"), Some(&hand));
    assert!(locked_pad(&pcb, "R1", "2"));
    assert_eq!(pad_net(&pcb, "R2", "1"), Some(&mid), "unlocked pad follows");
    assert!(pcb.nets.iter().any(|n| n.number == 9 && n.name == "HAND"));
}

#[test]
fn a_pad_the_schematic_cannot_speak_for_locks_itself_and_the_inverse_unlocks() {
    let netlist = build_netlist(&divider_sheet());
    let mut pcb = board(vec![
        footprint("R1", &["1", "2", "SH", ""]),
        footprint("R2", &["1", "2"]),
        footprint("H1", &["1", "2"]),
    ]);
    let hand = PadNet {
        number: 9,
        name: "GND".to_string(),
    };
    pcb.nets.push(NetDef {
        number: 9,
        name: "GND".to_string(),
    });
    pcb.footprints[0].pads[2].net = Some(hand.clone()); // off the symbol
    pcb.footprints[0].pads[3].net = Some(hand.clone()); // no key
    pcb.footprints[2].pads[0].net = Some(hand.clone()); // footprint not in schematic: kept, not locked
    let before = pcb.clone();

    link_footprints(&mut pcb, &netlist);
    let inverse = assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_board(&pcb))
        .edit
        .apply(&mut pcb);
    assert!(locked_pad(&pcb, "R1", "SH"));
    assert!(locked_pad(&pcb, "R1", ""));
    assert!(
        !locked_pad(&pcb, "H1", "1"),
        "a footprint the schematic lacks is reported, not locked"
    );
    assert!(!locked_pad(&pcb, "H1", "2"), "no net, nothing to lock");
    assert!(!locked_pad(&pcb, "R1", "1"), "schematic-driven pad");
    assert_eq!(pad_net(&pcb, "R1", "SH"), Some(&hand));

    let second = assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_board(&pcb));
    assert!(second.edit.pads.is_empty(), "{:?}", second.edit.pads);
    let locked_issues = second
        .issues
        .iter()
        .filter(|i| matches!(i, PadNetIssue::PadNetLocked { .. }))
        .count();
    assert_eq!(locked_issues, 2);

    assert_eq!(pad_net(&pcb, "H1", "1"), Some(&hand), "net kept");

    inverse.apply(&mut pcb);
    for fp in &before.footprints {
        for p in &fp.pads {
            assert!(!locked_pad(&pcb, &fp.reference, &p.number));
        }
    }
    assert_eq!(pad_net(&pcb, "R1", "SH"), Some(&hand));
    assert_eq!(pad_net(&pcb, "R1", "1"), None);
}

#[test]
fn the_inverse_restores_a_previous_lock_flag_exactly() {
    let mut pcb = board(vec![footprint("U1", &["1", "2"])]);
    let (fp, locked) = ids(&pcb, "U1", "1");
    let (_, plain) = ids(&pcb, "U1", "2");
    let first = PadNet {
        number: 3,
        name: "A".to_string(),
    };
    let second = PadNet {
        number: 4,
        name: "B".to_string(),
    };
    assign_pad_net(&pcb, fp, locked, first.clone()).apply(&mut pcb);
    let table = net_rows(&pcb);

    let inverse = assign_pad_net(&pcb, fp, locked, second.clone()).apply(&mut pcb);
    let inverse_plain = assign_pad_net(&pcb, fp, plain, second.clone()).apply(&mut pcb);
    assert_eq!(pad_net(&pcb, "U1", "1"), Some(&second));
    assert!(locked_pad(&pcb, "U1", "1") && locked_pad(&pcb, "U1", "2"));

    inverse_plain.apply(&mut pcb);
    assert!(!locked_pad(&pcb, "U1", "2"), "was unlocked");
    assert_eq!(pad_net(&pcb, "U1", "2"), None);
    inverse.apply(&mut pcb);
    assert!(locked_pad(&pcb, "U1", "1"), "was locked");
    assert_eq!(pad_net(&pcb, "U1", "1"), Some(&first));
    assert_eq!(net_rows(&pcb), table);
}

#[test]
fn hand_assigning_a_net_sets_it_locks_the_pad_and_adds_the_net() {
    let mut pcb = board(vec![footprint("U1", &["1"])]);
    let (fp, pad) = ids(&pcb, "U1", "1");
    let net = PadNet {
        number: 5,
        name: "VBUS".to_string(),
    };
    let inverse = assign_pad_net(&pcb, fp, pad, net.clone()).apply(&mut pcb);
    assert_eq!(pad_net(&pcb, "U1", "1"), Some(&net));
    assert!(locked_pad(&pcb, "U1", "1"));
    assert_eq!(net_rows(&pcb), vec![(5, "VBUS".to_string())]);
    inverse.apply(&mut pcb);
    assert_eq!(pad_net(&pcb, "U1", "1"), None);
    assert!(!locked_pad(&pcb, "U1", "1"));
    assert!(pcb.nets.is_empty());
}

#[test]
fn hand_assigning_to_a_missing_pad_changes_nothing_and_adds_no_net() {
    let mut pcb = board(vec![footprint("U1", &["1"])]);
    let (fp, pad) = ids(&pcb, "U1", "1");
    let net = PadNet {
        number: 5,
        name: "VBUS".to_string(),
    };
    for (footprint, pad) in [(Uuid::now_v7(), pad), (fp, Uuid::now_v7())] {
        let edit = assign_pad_net(&pcb, footprint, pad, net.clone());
        assert!(edit.pads.is_empty() && edit.zones.is_empty());
        let inverse = edit.apply(&mut pcb);
        assert!(pcb.nets.is_empty(), "no NetDef for a pad that is not there");
        assert!(inverse.pads.is_empty() && inverse.nets.is_empty());
    }
    let lock = set_pad_net_lock(&pcb, Uuid::now_v7(), pad, true);
    assert!(lock.pads.is_empty() && lock.nets.is_empty());
}

#[test]
fn hand_assigning_a_number_the_table_already_names_takes_the_tables_name() {
    let mut pcb = board(vec![footprint("U1", &["1"])]);
    pcb.nets.push(NetDef {
        number: 5,
        name: "VBUS".to_string(),
    });
    let (fp, pad) = ids(&pcb, "U1", "1");
    let asked = PadNet {
        number: 5,
        name: "OTHER".to_string(),
    };
    assign_pad_net(&pcb, fp, pad, asked).apply(&mut pcb);
    assert_eq!(
        pad_net(&pcb, "U1", "1"),
        Some(&PadNet {
            number: 5,
            name: "VBUS".to_string(),
        })
    );
    assert_eq!(net_rows(&pcb), vec![(5, "VBUS".to_string())]);
}

#[test]
fn unlocking_a_pad_keeps_its_net_lets_the_sync_set_it_and_undoes_exactly() {
    let netlist = build_netlist(&divider_sheet());
    let mut pcb = board(vec![
        footprint("R1", &["1", "2"]),
        footprint("R2", &["1", "2"]),
    ]);
    let (fp, pad) = ids(&pcb, "R1", "1");
    let hand = PadNet {
        number: 9,
        name: "HAND".to_string(),
    };
    assign_pad_net(&pcb, fp, pad, hand.clone()).apply(&mut pcb);
    link_footprints(&mut pcb, &netlist);
    let key = SymbolPadKey::for_board(&pcb);
    assign_pad_nets(&netlist, &pcb, &key).edit.apply(&mut pcb);
    assert_eq!(
        pad_net(&pcb, "R1", "1"),
        Some(&hand),
        "locked pad untouched"
    );

    let locked_state = pcb.clone();
    let unlock = set_pad_net_lock(&pcb, fp, pad, false);
    let relock = unlock.apply(&mut pcb);
    assert!(!locked_pad(&pcb, "R1", "1"));
    assert_eq!(pad_net(&pcb, "R1", "1"), Some(&hand), "net kept");

    let resync = assign_pad_nets(&netlist, &pcb, &key);
    assert!(
        !resync
            .issues
            .iter()
            .any(|i| matches!(i, PadNetIssue::PadNetLocked { footprint, pad: p } if *footprint == fp && *p == pad))
    );
    let undo_sync = resync.edit.apply(&mut pcb);
    assert_ne!(
        pad_net(&pcb, "R1", "1"),
        Some(&hand),
        "schematic sets it now"
    );

    undo_sync.apply(&mut pcb);
    relock.apply(&mut pcb);
    assert_eq!(pcb.footprints, locked_state.footprints);
    assert_eq!(net_rows(&pcb), net_rows(&locked_state));
}

#[test]
fn locking_a_pad_by_hand_keeps_its_net_and_the_inverse_unlocks() {
    let netlist = build_netlist(&divider_sheet());
    let mut pcb = board(vec![
        footprint("R1", &["1", "2"]),
        footprint("R2", &["1", "2"]),
    ]);
    link_footprints(&mut pcb, &netlist);
    assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_board(&pcb))
        .edit
        .apply(&mut pcb);
    let (fp, pad) = ids(&pcb, "R1", "2");
    let net = pad_net(&pcb, "R1", "2").cloned();
    let before = pcb.clone();
    let inverse = set_pad_net_lock(&pcb, fp, pad, true).apply(&mut pcb);
    assert!(locked_pad(&pcb, "R1", "2"));
    assert_eq!(pad_net(&pcb, "R1", "2").cloned(), net);
    inverse.apply(&mut pcb);
    assert_eq!(pcb.footprints, before.footprints);
}

#[test]
fn a_locked_pad_keeps_its_number_but_follows_a_rename_of_it() {
    let before = Netlist {
        nets: vec![net(
            7,
            "OLD",
            vec![terminal("R1", "1"), terminal("R2", "1")],
        )],
    };
    let after = Netlist {
        nets: vec![net(
            7,
            "NEW",
            vec![terminal("R1", "1"), terminal("R2", "1")],
        )],
    };
    let mut pcb = board(vec![footprint("R1", &["1"]), footprint("R2", &["1"])]);
    link_footprints(&mut pcb, &before);
    assign_pad_nets(&before, &pcb, &SymbolPadKey::for_board(&pcb))
        .edit
        .apply(&mut pcb);
    let (fp, pad) = ids(&pcb, "R1", "1");
    set_pad_net_lock(&pcb, fp, pad, true).apply(&mut pcb);
    let old = PadNet {
        number: 7,
        name: "OLD".to_string(),
    };
    assert_eq!(pad_net(&pcb, "R1", "1"), Some(&old));

    let inverse = assign_pad_nets(&after, &pcb, &SymbolPadKey::for_board(&pcb))
        .edit
        .apply(&mut pcb);
    let new = PadNet {
        number: 7,
        name: "NEW".to_string(),
    };
    assert_eq!(
        pad_net(&pcb, "R1", "1"),
        Some(&new),
        "same number, new name"
    );
    assert!(locked_pad(&pcb, "R1", "1"), "still locked");
    assert_eq!(pad_net(&pcb, "R2", "1"), Some(&new));
    assert_eq!(net_rows(&pcb), vec![(7, "NEW".to_string())]);

    inverse.apply(&mut pcb);
    assert_eq!(pad_net(&pcb, "R1", "1"), Some(&old));
    assert!(locked_pad(&pcb, "R1", "1"));
    assert_eq!(pad_net(&pcb, "R2", "1"), Some(&old));
    assert_eq!(net_rows(&pcb), vec![(7, "OLD".to_string())]);
}

#[test]
fn a_locked_pad_still_counts_for_a_designator_shared_by_two_footprints() {
    let netlist = Netlist {
        nets: vec![net(3, "SIG", vec![terminal("R1", "1")])],
    };
    let mut pcb = board(vec![footprint("R1", &["1"]), footprint("R1", &["1"])]);
    let (fp, pad) = ids(&pcb, "R1", "1");
    set_pad_net_lock(&pcb, fp, pad, true).apply(&mut pcb);

    link_footprints(&mut pcb, &netlist);
    let assignment = assign_pad_nets(&netlist, &pcb, &SymbolPadKey::for_board(&pcb));
    assert!(
        assignment
            .issues
            .iter()
            .any(|i| matches!(i, PadNetIssue::KeyOnSeveralFootprints { footprints, .. } if footprints.len() == 2)),
        "{:?}",
        assignment.issues
    );
    assert!(
        assignment.edit.pads.is_empty(),
        "neither footprint is assigned: {:?}",
        assignment.edit.pads
    );
}
