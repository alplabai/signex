//! #1061: `NetMultipleNames` over the stitched project netlist, so a name that
//! a net gets on another sheet is reported with every name's sheet and
//! position. Rule: `docs/NET_NAMING.md`.

use std::collections::HashMap;

use signex_erc::{RuleKind, Violation, project_net_multiple_names};
use signex_net::{ProjectGraph, ProjectRoot, SheetKey};
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Label, LabelType, LibPin, LibSymbol, Pin, PinDirection,
    PinShapeStyle, Point, SchematicSheet, SheetPin, Symbol, VAlign, Wire,
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

/// Run the project pass over `root` (key `root`) and its `children` (keyed by
/// filename).
fn project(
    root: &SchematicSheet,
    children: &[(&str, SchematicSheet)],
) -> Vec<(SheetKey, Violation)> {
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
