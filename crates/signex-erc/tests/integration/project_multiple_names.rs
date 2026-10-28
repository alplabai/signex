//! #1061: `NetMultipleNames` over the stitched project netlist, so a name that
//! a net gets on another sheet is reported with every name's sheet and
//! position. Rule: `docs/NET_NAMING.md`.

use std::collections::HashMap;

use signex_erc::{ProjectNetNames, RuleKind, Violation, project_net_multiple_names};
use signex_net::{ProjectGraph, ProjectRoot, SheetKey};
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Label, LabelType, LibPin, LibSymbol, NoConnect, Pin,
    PinDirection, PinShapeStyle, Point, SchematicSheet, SelectedKind, SheetPin, Symbol, VAlign,
    Wire,
};
use uuid::Uuid;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn label(text: &str, at: Point, label_type: LabelType) -> Label {
    Label {
        unknown_columns: Default::default(),
        uuid: Uuid::new_v4(),
        text: text.to_string(),
        position: at,
        rotation: 0.0,
        label_type,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

/// A wire from `(0, 0)` to `(10, 0)` with resistor `reference` on its right
/// end and the given labels at the points named.
fn sheet(reference: &str, labels: &[(&str, LabelType, f64)]) -> SchematicSheet {
    let mut s = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    s.wires.push(Wire {
        start: pt(0.0, 0.0),
        end: pt(10.0, 0.0),
        ..Default::default()
    });
    for (text, ty, x) in labels {
        s.labels.push(label(text, pt(*x, 0.0), *ty));
    }
    s.lib_symbols.insert(
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
            pins: vec![LibPin {
                unit: 0,
                body_style: 1,
                pin: Pin {
                    direction: PinDirection::Passive,
                    shape_style: PinShapeStyle::Plain,
                    position: pt(0.0, 0.0),
                    rotation: 0.0,
                    length: 0.0,
                    name: String::new(),
                    number: "1".to_string(),
                    visible: true,
                    name_visible: true,
                    number_visible: true,
                },
            }],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
    s.symbols.push(Symbol {
        lib_id: "R".to_string(),
        reference: reference.to_string(),
        position: pt(10.0, 0.0),
        ..Default::default()
    });
    s
}

fn child(name: &str, filename: &str, pin: Option<&str>) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::nil(),
        name: name.to_string(),
        filename: filename.to_string(),
        position: pt(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: pin
            .map(|n| SheetPin {
                uuid: Uuid::nil(),
                name: n.to_string(),
                direction: String::new(),
                position: pt(0.0, 0.0),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            })
            .into_iter()
            .collect(),
        instances: Vec::new(),
    }
}

/// The project pass's findings over `root` (key `root`) and its `children`
/// (keyed by filename).
fn project(
    root: &SchematicSheet,
    children: &[(&str, SchematicSheet)],
) -> Vec<(SheetKey, Violation)> {
    project_full(root, children).findings
}

/// [`project`], with the covered sources too.
fn project_full(root: &SchematicSheet, children: &[(&str, SchematicSheet)]) -> ProjectNetNames {
    let mut sheets: HashMap<SheetKey, SchematicSheet> = children
        .iter()
        .map(|(f, s)| (SheetKey::new(*f), s.clone()))
        .collect();
    sheets.insert(SheetKey::new("root"), root.clone());
    let resolved = sheets
        .iter()
        .map(|(k, s)| {
            let submap = s
                .child_sheets
                .iter()
                .map(|c| (c.filename.clone(), SheetKey::new(c.filename.clone())))
                .collect();
            (k.clone(), submap)
        })
        .collect();
    let roots = [ProjectRoot {
        key: SheetKey::new("root"),
    }];
    project_net_multiple_names(&ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    })
}

#[test]
fn a_global_on_the_root_and_a_hierarchical_in_the_child_are_one_finding() {
    // The issue's case: Global `I2C_SDA` on the root wire of sheet pin `SDA`;
    // child `charger` calls its side Hierarchical `SDA`. The export says
    // `I2C_SDA`; per-sheet ERC sees one name on each sheet.
    let mut root = sheet("R1", &[("I2C_SDA", LabelType::Global, 0.0)]);
    root.child_sheets
        .push(child("charger", "charger.sch", Some("SDA")));
    let charger = sheet("R2", &[("SDA", LabelType::Hierarchical, 0.0)]);

    let found = project(&root, &[("charger.sch", charger)]);

    assert_eq!(found.len(), 1, "{found:?}");
    let (key, v) = &found[0];
    assert_eq!(key.as_str(), "root", "filed under the winning name's sheet");
    assert_eq!(v.rule, RuleKind::NetMultipleNames);
    assert_eq!(v.builtin_kind(), Some(RuleKind::NetMultipleNames));
    let m = &v.message;
    assert!(m.contains("the netlist uses 'I2C_SDA'"), "{m}");
    assert!(
        m.contains("'I2C_SDA' (global) on sheet root at (0.00, 0.00)"),
        "{m}"
    );
    assert!(
        m.contains("'SDA' (hierarchical) on sheet charger at (0.00, 0.00)"),
        "{m}"
    );
}

#[test]
fn two_children_sharing_a_global_with_different_local_aliases_are_one_finding() {
    let mut root = sheet("R1", &[]);
    root.labels.clear();
    root.child_sheets.push(child("A", "a.sch", None));
    root.child_sheets.push(child("B", "b.sch", None));
    let a = sheet(
        "R2",
        &[
            ("BUS", LabelType::Global, 0.0),
            ("ALIAS_A", LabelType::Hierarchical, 10.0),
        ],
    );
    let b = sheet(
        "R3",
        &[
            ("BUS", LabelType::Global, 0.0),
            ("ALIAS_B", LabelType::Hierarchical, 10.0),
        ],
    );

    let found = project(&root, &[("a.sch", a), ("b.sch", b)]);

    assert_eq!(found.len(), 1, "{found:?}");
    let m = &found[0].1.message;
    assert!(m.contains("the netlist uses 'BUS'"), "{m}");
    for part in ["'ALIAS_A'", "on sheet A", "'ALIAS_B'", "on sheet B"] {
        assert!(m.contains(part), "{part}: {m}");
    }
}

#[test]
fn a_single_sheet_net_is_reported_once() {
    // Two hierarchical names on one wire of a lone sheet: the project pass
    // owns it (the caller drops the sheet's own finding), so there is one.
    let root = sheet(
        "R1",
        &[
            ("SDA", LabelType::Hierarchical, 0.0),
            ("I2C_SDA", LabelType::Hierarchical, 10.0),
        ],
    );
    let found = project(&root, &[]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.message.contains("the netlist uses 'I2C_SDA'"));
}

#[test]
fn one_name_repeated_and_label_conflicts_are_not_reported() {
    let same = sheet(
        "R1",
        &[
            ("SDA", LabelType::Hierarchical, 0.0),
            ("SDA", LabelType::Hierarchical, 10.0),
        ],
    );
    assert!(project(&same, &[]).is_empty());

    // Local `SDA` + global `I2C_SDA`: `NetLabelConflict` reports this one.
    let conflict = sheet(
        "R1",
        &[
            ("SDA", LabelType::Net, 0.0),
            ("I2C_SDA", LabelType::Global, 10.0),
        ],
    );
    assert!(project(&conflict, &[]).is_empty());
}

fn pinless(labels: &[(&str, LabelType, f64)]) -> SchematicSheet {
    let mut s = sheet("R1", &[]);
    s.symbols.clear();
    for (text, ty, x) in labels {
        s.labels.push(label(text, pt(*x, 0.0), *ty));
    }
    s
}

#[test]
fn a_wire_with_two_labels_and_no_pin_is_reported_once() {
    let root = pinless(&[
        ("A", LabelType::Hierarchical, 0.0),
        ("B", LabelType::Hierarchical, 10.0),
    ]);
    let found = project(&root, &[]);
    assert_eq!(found.len(), 1, "{found:?}");
    let m = &found[0].1.message;
    assert!(
        m.contains("no net is exported; it would be named 'A'"),
        "{m}"
    );
    assert!(!m.contains("the netlist uses"), "{m}");
}

#[test]
fn a_power_port_only_rail_selects_the_symbol() {
    // `VCC` port on the wire's right end, no component pin, a second name on
    // the left: the port wins and the finding selects its symbol.
    let mut root = sheet("#PWR1", &[("RAIL", LabelType::Hierarchical, 0.0)]);
    let port = Uuid::from_u128(9);
    root.symbols[0].is_power = true;
    root.symbols[0].value = "VCC".to_string();
    root.symbols[0].uuid = port;

    let found = project(&root, &[]);

    assert_eq!(found.len(), 1, "{found:?}");
    let v = &found[0].1;
    assert!(
        v.message.contains("the netlist uses 'VCC'"),
        "{}",
        v.message
    );
    let primary = v.primary.as_ref().expect("selection");
    assert_eq!((primary.uuid, primary.kind), (port, SelectedKind::Symbol));
    let peer = v.peer.as_ref().expect("same-sheet peer");
    assert_eq!(peer.kind, SelectedKind::Label);
}

#[test]
fn a_peer_on_another_sheet_is_not_selected() {
    let mut root = sheet("R1", &[("I2C_SDA", LabelType::Global, 0.0)]);
    root.child_sheets
        .push(child("charger", "charger.sch", Some("SDA")));
    let charger = sheet("R2", &[("SDA", LabelType::Hierarchical, 0.0)]);

    let found = project(&root, &[("charger.sch", charger)]);

    let v = &found[0].1;
    assert!(v.primary.is_some());
    assert!(v.peer.is_none(), "a uuid would target the wrong canvas");
}

#[test]
fn a_no_erc_directive_on_the_winning_name_suppresses_the_finding() {
    let mut root = sheet(
        "R1",
        &[
            ("SDA", LabelType::Hierarchical, 0.0),
            ("I2C_SDA", LabelType::Hierarchical, 10.0),
        ],
    );
    assert_eq!(project(&root, &[]).len(), 1);
    root.no_erc_directives.push(NoConnect {
        uuid: Uuid::new_v4(),
        position: pt(10.0, 0.0),
    });
    assert!(project(&root, &[]).is_empty());
}

#[test]
fn a_child_placed_twice_gives_one_row_at_one_marker() {
    let mut root = pinless(&[]);
    root.child_sheets.push(child("U1", "c.sch", None));
    root.child_sheets.push(child("U2", "c.sch", None));
    let c = pinless(&[
        ("A", LabelType::Hierarchical, 0.0),
        ("B", LabelType::Hierarchical, 10.0),
    ]);

    let found = project(&root, &[("c.sch", c)]);

    assert_eq!(found.len(), 1, "{found:?}");
}

fn no_erc(at: Point) -> NoConnect {
    NoConnect {
        uuid: Uuid::new_v4(),
        position: at,
    }
}

fn covers(found: &ProjectNetNames, sheet: &str, x: f64) -> bool {
    found
        .covered
        .contains(&(SheetKey::new(sheet), signex_net::pt_key(&pt(x, 0.0))))
}

/// Root wire of sheet pin `SDA` named by `root_labels`; child `charger` has
/// Hierarchical `SDA` at x=0 and `DATA` at x=10 on one wire with a part.
fn root_and_child(root_labels: &[(&str, LabelType, f64)]) -> (SchematicSheet, SchematicSheet) {
    let mut root = sheet("R1", root_labels);
    root.child_sheets
        .push(child("charger", "charger.sch", Some("SDA")));
    let charger = sheet(
        "R2",
        &[
            ("SDA", LabelType::Hierarchical, 0.0),
            ("DATA", LabelType::Hierarchical, 10.0),
        ],
    );
    (root, charger)
}

#[test]
fn a_label_conflict_on_one_sheet_still_reports_the_other_sheets_names() {
    // Root: Net `X` + Global `Y` is a NetLabelConflict there. The child's own
    // `SDA` / `DATA` pair must still be reported, filed under the child.
    let (root, charger) =
        root_and_child(&[("X", LabelType::Net, 0.0), ("Y", LabelType::Global, 10.0)]);

    let found = project_full(&root, &[("charger.sch", charger)]);

    assert_eq!(found.findings.len(), 1, "{:?}", found.findings);
    let (key, v) = &found.findings[0];
    assert_eq!(key.as_str(), "charger.sch");
    assert!(v.message.contains("the netlist uses 'Y'"), "{}", v.message);
    assert!(v.message.contains("'DATA'"), "{}", v.message);
    assert!(
        !v.message.contains("'X'"),
        "root is left to the conflict rule"
    );
    assert!(covers(&found, "charger.sch", 0.0) && covers(&found, "charger.sch", 10.0));
    assert!(
        !covers(&found, "root", 0.0),
        "a sheet the finding leaves out keeps its own findings"
    );
}

#[test]
fn a_no_erc_on_the_roots_winning_name_leaves_the_childs_names_reported() {
    let (mut root, charger) = root_and_child(&[("I2C_SDA", LabelType::Global, 0.0)]);
    root.no_erc_directives.push(no_erc(pt(0.0, 0.0)));

    let found = project_full(&root, &[("charger.sch", charger)]);

    assert_eq!(found.findings.len(), 1, "{:?}", found.findings);
    let (key, v) = &found.findings[0];
    assert_eq!(key.as_str(), "charger.sch");
    assert!(v.message.contains("'SDA'") && v.message.contains("'DATA'"));
    assert!(!v.message.contains("'I2C_SDA' (global)"), "{}", v.message);
    // The marker sits on the child's own winner, `DATA`.
    assert_eq!((v.location.x, v.location.y), (10.0, 0.0));
}

#[test]
fn a_no_erc_on_the_childs_own_winner_still_works_once_the_root_wins() {
    // The user silenced the child's `SDA` / `DATA` pair on `DATA`; the root's
    // Global now wins the net. Only one name is left, so nothing is reported.
    let (root, mut charger) = root_and_child(&[("I2C_SDA", LabelType::Global, 0.0)]);
    charger.no_erc_directives.push(no_erc(pt(10.0, 0.0)));

    let found = project_full(&root, &[("charger.sch", charger)]);

    assert!(found.findings.is_empty(), "{:?}", found.findings);
    assert!(found.covered.is_empty());
}

#[test]
fn the_marker_sits_on_the_winner_when_the_exported_name_is_suffixed() {
    // Two sibling children named `charger`: the second `charger/ZED` exports
    // as `charger/ZED_<n>`. `ZED` (x=10) still carries both markers, not the
    // alphabetically first `ALT` (x=0).
    let mut root = pinless(&[]);
    root.child_sheets.push(child("charger", "a.sch", None));
    root.child_sheets.push(child("charger", "b.sch", None));
    let side = |r| {
        sheet(
            r,
            &[
                ("ALT", LabelType::Net, 0.0),
                ("ZED", LabelType::Hierarchical, 10.0),
            ],
        )
    };

    let found = project(&root, &[("a.sch", side("RA")), ("b.sch", side("RB"))]);

    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found
            .iter()
            .any(|(_, v)| v.message.contains("'charger/ZED_"))
    );
    for (_, v) in &found {
        assert_eq!((v.location.x, v.location.y), (10.0, 0.0), "{}", v.message);
    }
}
