//! The net index: partners across sheets, steps, hover facts, highlight sets.
use super::*;
use signex_types::net_class::NetClass;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn key(name: &str) -> SheetKey {
    SheetKey::new(name)
}

fn labelled(text: &str, at: Point, ty: LabelType, uuid: u128) -> Label {
    Label {
        uuid: id(uuid),
        ..label(text, at, ty)
    }
}

/// `root` under `root.sch` plus `children`, indexed.
fn indexed(root: &SchematicSheet, children: &[(&str, SchematicSheet)]) -> NetIndex {
    let mut sheets: HashMap<SheetKey, SchematicSheet> =
        children.iter().map(|(k, v)| (key(k), v.clone())).collect();
    sheets.insert(key("root.sch"), root.clone());
    let resolved = sheets
        .iter()
        .map(|(k, s)| {
            let sub = s
                .child_sheets
                .iter()
                .map(|cs| (cs.filename.clone(), key(&cs.filename)))
                .collect();
            (k.clone(), sub)
        })
        .collect();
    let roots = [ProjectRoot {
        key: key("root.sch"),
    }];
    let graph = ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    };
    build_project_netlist_indexed(&graph).1
}

fn root_with_child(file: &str, pins: Vec<SheetPin>) -> SchematicSheet {
    let mut root = empty_sheet();
    let mut cs = child_sheet("A", file, pins);
    cs.uuid = id(500);
    root.child_sheets.push(cs);
    root
}

fn entry(name: &str, at: Point, uuid: u128) -> SheetPin {
    SheetPin {
        uuid: id(uuid),
        ..sheet_pin(name, at)
    }
}

fn ids(partners: &[&NetMember]) -> Vec<Uuid> {
    partners.iter().map(|m| m.id).collect()
}

#[test]
fn global_labels_on_two_sheets_are_partners() {
    let mut root = root_with_child("a.sch", vec![]);
    root.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    root.labels
        .push(labelled("VCC", pt(0.0, 0.0), LabelType::Global, 1));
    let mut a = empty_sheet();
    a.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    a.labels
        .push(labelled("VCC", pt(0.0, 0.0), LabelType::Global, 2));
    let idx = indexed(&root, &[("a.sch", a)]);

    let from_root = idx.partners(&key("root.sch"), id(1), None);
    assert_eq!(ids(&from_root), [id(2)]);
    assert_eq!(from_root[0].sheet, key("a.sch"));
    let from_child = idx.partners(&key("a.sch"), id(2), None);
    assert_eq!(ids(&from_child), [id(1)]);
}

#[test]
fn a_hierarchical_label_and_its_sheet_entry_are_partners() {
    let mut root = root_with_child("a.sch", vec![entry("SIG", pt(0.0, 0.0), 10)]);
    root.wires.push(wire(pt(0.0, 0.0), pt(-5.0, 0.0)));
    let mut a = empty_sheet();
    a.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    a.labels
        .push(labelled("SIG", pt(0.0, 0.0), LabelType::Hierarchical, 11));
    let idx = indexed(&root, &[("a.sch", a)]);

    let up = idx.partners(&key("a.sch"), id(11), None);
    assert_eq!(ids(&up), [id(10)]);
    assert_eq!(up[0].sheet, key("root.sch"));
    assert!(matches!(up[0].kind, NetMemberKind::SheetPin { .. }));
    let down = idx.partners(&key("root.sch"), id(10), None);
    assert_eq!(ids(&down), [id(11)]);
}

/// Two children whose entries are wired together on the parent: the two child
/// ports are partners of each other, and repeating the jump cycles the ring.
#[test]
fn ports_of_sibling_sheets_wired_together_are_partners_and_cycle() {
    let mut root = empty_sheet();
    for (n, file, x) in [(500, "a.sch", 0.0), (501, "b.sch", 10.0)] {
        let mut cs = child_sheet(file, file, vec![entry("SIG", pt(x, 0.0), 10 + n)]);
        cs.uuid = id(n);
        root.child_sheets.push(cs);
    }
    root.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    let child = |label_uuid| {
        let mut c = empty_sheet();
        c.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
        c.labels.push(labelled(
            "SIG",
            pt(0.0, 0.0),
            LabelType::Hierarchical,
            label_uuid,
        ));
        c
    };
    let idx = indexed(&root, &[("a.sch", child(21)), ("b.sch", child(22))]);

    let from_a = idx.partners(&key("a.sch"), id(21), None);
    assert_eq!(from_a.first().map(|m| m.id), Some(id(22)));
    assert_eq!(from_a.len(), 3, "the other label and both sheet entries");
    let from_b = idx.partners(&key("b.sch"), id(22), None);
    assert_eq!(
        from_b.first().map(|m| m.id),
        Some(id(510)),
        "the ring continues past the last port to the first"
    );
}

#[test]
fn a_lone_or_local_label_has_no_partner() {
    let mut root = root_with_child("a.sch", vec![]);
    root.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    root.labels
        .push(labelled("ALONE", pt(0.0, 0.0), LabelType::Global, 1));
    root.wires.push(wire(pt(20.0, 0.0), pt(25.0, 0.0)));
    root.labels
        .push(labelled("LOCAL", pt(20.0, 0.0), LabelType::Net, 3));
    let mut a = empty_sheet();
    a.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    a.labels
        .push(labelled("LOCAL", pt(0.0, 0.0), LabelType::Net, 4));
    let idx = indexed(&root, &[("a.sch", a)]);

    assert!(idx.partners(&key("root.sch"), id(1), None).is_empty());
    assert!(
        idx.partners(&key("root.sch"), id(3), None).is_empty(),
        "local labels never cross"
    );
    assert!(
        idx.partners(&key("a.sch"), id(99), None).is_empty(),
        "unknown element"
    );
}

/// R1 on the root and R2 on the child share `VCC` through two global labels.
fn two_sheet_vcc() -> NetIndex {
    let mut root = root_with_child("a.sch", vec![]);
    add_lib(&mut root, "R");
    place(&mut root, "R1", "R", pt(0.0, 0.0));
    root.symbols[0].uuid = id(100);
    root.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    root.wires[0].uuid = id(300);
    root.labels
        .push(labelled("VCC", pt(5.0, 0.0), LabelType::Global, 1));
    let mut a = empty_sheet();
    add_lib(&mut a, "R");
    place(&mut a, "R2", "R", pt(0.0, 0.0));
    a.symbols[0].uuid = id(101);
    a.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    a.labels
        .push(labelled("VCC", pt(5.0, 0.0), LabelType::Global, 2));
    indexed(&root, &[("a.sch", a)])
}

#[test]
fn steps_visit_pins_and_labels_across_sheets_but_not_wiring() {
    let idx = two_sheet_vcc();
    let net = idx.nets_of(&key("root.sch"), id(100))[0];
    let steps = idx.steps(net);
    let at: Vec<(String, Uuid)> = steps.iter().map(|m| (m.sheet.to_string(), m.id)).collect();
    assert_eq!(
        at,
        [
            ("root.sch".to_string(), id(100)),
            ("root.sch".to_string(), id(1)),
            ("a.sch".to_string(), id(101)),
            ("a.sch".to_string(), id(2)),
        ]
    );
    assert_eq!(
        idx.nets_of(&key("a.sch"), id(101))[0],
        net,
        "one net on both sheets"
    );
}

#[test]
fn highlight_elements_cover_every_sheet_the_net_reaches() {
    let idx = two_sheet_vcc();
    let net = idx.nets_of(&key("root.sch"), id(100))[0];
    let sets = idx.highlight_elements(net);
    assert!(sets[&key("root.sch")].contains(&id(300)), "the wire");
    assert!(sets[&key("root.sch")].contains(&id(100)), "the symbol");
    assert!(sets[&key("a.sch")].contains(&id(2)), "the far label");
}

#[test]
fn hover_names_the_net_its_class_and_every_pin_with_its_sheet() {
    let idx = two_sheet_vcc();
    let net = idx.nets_of(&key("root.sch"), id(1))[0];
    let class = NetClass {
        nets: vec!["VCC".to_string()],
        ..NetClass::named("Power")
    };
    let hover = idx.hover(net, &[class]).expect("net exists");
    assert_eq!(hover.name, "VCC");
    assert_eq!(hover.class.as_deref(), Some("Power"));
    let pins: Vec<(String, String, String)> = hover
        .pins
        .iter()
        .map(|p| (p.reference.clone(), p.pin.clone(), p.sheet.to_string()))
        .collect();
    assert_eq!(
        pins,
        [
            ("R1".into(), "1".into(), "root.sch".into()),
            ("R2".into(), "1".into(), "a.sch".into()),
        ]
    );
    assert_eq!(idx.hover(net, &[]).expect("net").class, None);
}

#[test]
fn pin_near_finds_the_pin_tip_under_a_point_on_its_own_sheet_only() {
    let idx = two_sheet_vcc();
    let (net, member) = idx
        .pin_near(&key("root.sch"), pt(0.2, 0.1), 0.5, None)
        .expect("R1 pin tip");
    assert_eq!(member.id, id(100));
    assert_eq!(idx.net(net).map(|n| n.name.as_str()), Some("VCC"));
    assert!(
        idx.pin_near(&key("root.sch"), pt(3.0, 3.0), 0.5, None)
            .is_none()
    );
    assert!(
        idx.pin_near(&key("a.sch"), pt(9.0, 9.0), 0.5, None)
            .is_none()
    );
}

#[test]
fn resolve_follows_a_net_by_name_or_anchor_after_a_rebuild() {
    let idx = two_sheet_vcc();
    let anchor = (key("root.sch"), id(1));
    let found = idx.resolve("VCC", &anchor).expect("by anchor");
    assert_eq!(idx.net(found).map(|n| n.name.as_str()), Some("VCC"));
    let gone = (key("root.sch"), id(404));
    assert_eq!(idx.resolve("VCC", &gone), Some(found), "by name");
    assert_eq!(idx.resolve("NOPE", &gone), None);
}

#[test]
fn a_member_says_whether_it_is_a_port() {
    let idx = two_sheet_vcc();
    let label = idx
        .member(&key("root.sch"), id(1), None)
        .expect("global label");
    assert!(label.is_port());
    assert_eq!(label.text, "VCC");
    let pin = idx.member(&key("root.sch"), id(100), None).expect("R1");
    assert!(!pin.is_port());
    assert!(idx.member(&key("root.sch"), id(404), None).is_none());
}

#[test]
fn partners_of_a_child_placed_twice_follow_the_placement() {
    // Entries on the parent wire to distinct parent nets; the child's
    // hierarchical label joins whichever entry it is placed through.
    let mut root = empty_sheet();
    for (n, file, y) in [(500u128, "c.sch", 0.0), (501, "c.sch", 20.0)] {
        let mut cs = child_sheet(
            if n == 500 { "A" } else { "B" },
            file,
            vec![entry("SIG", pt(0.0, y), 600 + n)],
        );
        cs.uuid = id(n);
        root.child_sheets.push(cs);
        root.wires.push(wire(pt(0.0, y), pt(5.0, y)));
    }
    let mut c = empty_sheet();
    c.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    c.labels
        .push(labelled("SIG", pt(0.0, 0.0), LabelType::Hierarchical, 21));
    let idx = indexed(&root, &[("c.sch", c)]);

    let via = |n: u128| InstancePath::from_chain(vec![id(n)]);
    let a = idx.partners(&key("c.sch"), id(21), Some(&via(500)));
    let b = idx.partners(&key("c.sch"), id(21), Some(&via(501)));
    assert_eq!(ids(&a), [id(1100)], "instance A reaches A's entry");
    assert_eq!(ids(&b), [id(1101)], "instance B reaches B's entry");
    assert_ne!(
        idx.nets_in(&key("c.sch"), id(21), Some(&via(500))),
        idx.nets_in(&key("c.sch"), id(21), Some(&via(501)))
    );
}

#[test]
fn a_power_port_is_a_port_and_its_partner_is_the_same_rail_elsewhere() {
    let mut root = root_with_child("a.sch", vec![]);
    add_lib(&mut root, "P");
    place_power(&mut root, "#PWR1", "P", "VCC", pt(0.0, 0.0));
    root.symbols[0].uuid = id(70);
    root.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    let mut a = empty_sheet();
    add_lib(&mut a, "P");
    place_power(&mut a, "#PWR2", "P", "VCC", pt(0.0, 0.0));
    a.symbols[0].uuid = id(71);
    a.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    let idx = indexed(&root, &[("a.sch", a)]);

    assert!(
        idx.member(&key("root.sch"), id(70), None)
            .expect("power port")
            .is_port()
    );
    assert_eq!(ids(&idx.partners(&key("root.sch"), id(70), None)), [id(71)]);
}

#[test]
fn an_auto_name_is_never_trusted_to_find_a_net_again() {
    assert!(is_auto_name("N$12"));
    assert!(!is_auto_name("N$") && !is_auto_name("N$x") && !is_auto_name("VCC"));
    let idx = two_sheet_vcc();
    let net = idx.nets_of(&key("root.sch"), id(1))[0];
    let anchor = idx.anchor_of(net).expect("an anchor");
    assert_eq!(idx.resolve("VCC", &anchor), Some(net));
    let gone = (key("root.sch"), id(404));
    assert_eq!(idx.resolve("N$1", &gone), None);
}

/// Flat pages, none referencing another: each is its own root.
fn indexed_flat(pages: &[(&str, SchematicSheet)]) -> NetIndex {
    let sheets: HashMap<SheetKey, SchematicSheet> =
        pages.iter().map(|(k, v)| (key(k), v.clone())).collect();
    let resolved = HashMap::new();
    let roots: Vec<ProjectRoot> = pages
        .iter()
        .map(|(k, _)| ProjectRoot { key: key(k) })
        .collect();
    let graph = ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    };
    build_project_netlist_indexed(&graph).1
}

/// A resistor on a wire, with a local label `EN` when `labelled`.
fn en_page(wire_uuid: u128, label_uuid: u128, labelled_net: bool) -> SchematicSheet {
    let mut s = empty_sheet();
    add_lib(&mut s, "R");
    place(&mut s, "R1", "R", pt(0.0, 0.0));
    s.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    s.wires[0].uuid = id(wire_uuid);
    if labelled_net {
        s.labels
            .push(labelled("EN", pt(0.0, 0.0), LabelType::Net, label_uuid));
    }
    s
}

#[test]
fn a_live_anchor_beats_an_unrelated_net_that_now_carries_the_name() {
    let before = indexed_flat(&[
        ("p1", en_page(100, 101, true)),
        ("p2", en_page(200, 201, true)),
    ]);
    let net1 = before.nets_of(&key("p1"), id(100))[0];
    assert_eq!(before.net(net1).map(|n| n.name.as_str()), Some("EN"));
    let anchor = (key("p1"), id(100));

    // Page 1 loses its label: its net is auto-named and page 2's net is `EN`.
    let after = indexed_flat(&[
        ("p1", en_page(100, 101, false)),
        ("p2", en_page(200, 201, true)),
    ]);
    let own = after.nets_of(&key("p1"), id(100))[0];
    assert_eq!(after.resolve("EN", &anchor), Some(own));
}

#[test]
fn a_name_that_follows_position_is_not_trusted() {
    assert!(is_positional_name("N$3") && is_positional_name("EN_2"));
    assert!(!is_positional_name("EN") && !is_positional_name("EN_A") && !is_positional_name("VCC"));
}

#[test]
fn power_ports_on_other_sheets_come_before_those_on_the_same_sheet() {
    let mut root = root_with_child("a.sch", vec![]);
    add_lib(&mut root, "P");
    for (n, at) in [(70u128, 0.0), (72, 20.0)] {
        place_power(&mut root, "#PWR", "P", "GND", pt(at, 0.0));
        root.symbols.last_mut().unwrap().uuid = id(n);
        root.wires.push(wire(pt(at, 0.0), pt(at + 5.0, 0.0)));
    }
    let mut a = empty_sheet();
    add_lib(&mut a, "P");
    place_power(&mut a, "#PWR", "P", "GND", pt(0.0, 0.0));
    a.symbols[0].uuid = id(71);
    a.wires.push(wire(pt(0.0, 0.0), pt(5.0, 0.0)));
    let idx = indexed(&root, &[("a.sch", a)]);

    let from_root = idx.partners(&key("root.sch"), id(70), None);
    assert_eq!(ids(&from_root), [id(71), id(72)], "the other sheet first");
}
