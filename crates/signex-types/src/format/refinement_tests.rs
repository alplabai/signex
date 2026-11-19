//! Schematic Refinement content in `.snxsch` (`snxsch/1.3`): harness
//! connectors, harness wires, harness-typed sheet entries and unions in
//! `[extras.sheet]`, plus the new drawing kinds and directive fields, which
//! ride in the existing `drawings` / `directives` arrays. A sheet without any
//! of it saves exactly as before; any one of it writes `snxsch/1.3`, which an
//! older build opens read-only instead of dropping it on its next save.
//! Format: `docs/architecture/snxsch-refinement-objects.md`.

use serde_json::json;

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{
    ChildSheet, HarnessConnector, HarnessEntry, HarnessWire, Point, SchDirective, SchDrawing,
    SchUnion, SchematicSheet, Wire,
};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(0x0192_a8c0_0013_7000_8000_0000_0000_0000 | n)
}

fn token(text: &str) -> &str {
    text.lines()
        .next()
        .and_then(|line| line.strip_prefix("format = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn pt(x: f64, y: f64) -> serde_json::Value {
    json!({ "x": x, "y": y })
}

fn connector(n: u128, harness_type: &str) -> HarnessConnector {
    HarnessConnector {
        uuid: id(n),
        harness_type: harness_type.into(),
        position: Point::new(50.8, 25.4),
        width: 10.16,
        entry_pitch: 2.54,
        rotation: 90.0,
        entries: vec![
            HarnessEntry {
                uuid: id(n + 1),
                name: "SCK".into(),
            },
            HarnessEntry {
                uuid: id(n + 2),
                name: "MOSI".into(),
            },
        ],
        locked: true,
    }
}

fn harness_wire(n: u128) -> HarnessWire {
    HarnessWire {
        uuid: id(n),
        start: Point::new(63.5, 30.48),
        end: Point::new(88.9, 30.48),
        locked: false,
    }
}

fn union(n: u128, members: Vec<Uuid>) -> SchUnion {
    SchUnion {
        uuid: id(n),
        name: "Buck stage".into(),
        tags: vec!["power".into(), "smps".into()],
        members,
        locked: true,
    }
}

/// A sheet symbol with one sheet pin, `id(0x41)`.
fn child_sheet() -> ChildSheet {
    serde_json::from_value(json!({
        "uuid": id(0x40),
        "name": "SPI Flash",
        "filename": "spi-flash.snxsch",
        "position": pt(101.6, 50.8),
        "size": [25.4, 15.24],
        "pins": [{ "uuid": id(0x41), "name": "SPI", "position": pt(101.6, 55.88) }],
    }))
    .expect("sheet symbol")
}

fn bezier() -> SchDrawing {
    serde_json::from_value(json!({
        "type": "bezier", "uuid": id(0x21),
        "start": pt(10.16, 10.16), "c1": pt(15.24, 2.54),
        "c2": pt(25.4, 20.32), "end": pt(30.48, 10.16), "width": 0.25,
    }))
    .expect("a Bezier drawing reads")
}

fn dimension() -> SchDrawing {
    serde_json::from_value(json!({
        "type": "dimension", "uuid": id(0x22),
        "from": pt(10.16, 40.64), "to": pt(35.56, 40.64), "offset": -5.08,
        "direction": "horizontal", "units": "mil", "precision": 1,
        "override_text": "1000 mil", "width": 0.15,
    }))
    .expect("a dimension drawing reads")
}

fn table() -> SchDrawing {
    serde_json::from_value(json!({
        "type": "table", "uuid": id(0x23), "position": pt(127.0, 12.7),
        "title": "U1 pins",
        "source": { "kind": "component_pins", "designator": "U1" },
        "columns": [
            { "key": "number", "title": "Pin" },
            { "key": "name", "title": "Name" },
            { "key": "net", "title": "Net", "shown": false },
        ],
        "rows": [["1", "VIN", "VIN_12V"], ["2", "EN", "EN"]],
        "style": { "text_size": 1.8, "cell_padding": 0.6, "borders": "outer", "header_fill": false },
    }))
    .expect("a table drawing reads")
}

fn net_parameters() -> SchDirective {
    serde_json::from_value(json!({
        "type": "net_parameters", "uuid": id(0x31), "position": pt(60.96, 76.2),
        "net_class": "POWER_IN", "diff_pair_class": "USB",
        "rules": { "width_mm": 0.8, "clearance_mm": 0.3 },
        "show_on_flag": true,
    }))
    .expect("a Net Parameters directive reads")
}

fn blanket(extra: serde_json::Value) -> SchDirective {
    let mut value = json!({
        "type": "blanket", "uuid": id(0x32),
        "start": pt(20.32, 20.32), "end": pt(71.12, 50.8), "net_class": "SW_NODE",
    });
    for (key, field) in extra.as_object().expect("an object") {
        value[key] = field.clone();
    }
    serde_json::from_value(value).expect("a Blanket reads")
}

fn diff_pair(class: Option<&str>) -> SchDirective {
    let mut value = json!({ "type": "diff_pair", "uuid": id(0x33), "position": pt(40.64, 63.5) });
    if let Some(class) = class {
        value["class"] = json!(class);
    }
    serde_json::from_value(value).expect("a Differential Pair directive reads")
}

/// One named edit of a sheet.
type Addition = (&'static str, Box<dyn Fn(&mut SchematicSheet)>);

fn addition(what: &'static str, edit: impl Fn(&mut SchematicSheet) + 'static) -> Addition {
    (what, Box::new(edit))
}

/// Each Schematic Refinement object alone, as an edit of an empty sheet.
fn single_additions() -> Vec<Addition> {
    vec![
        addition("harness connector", |s| {
            s.harness.connectors.push(connector(0x10, "SPI_BUS"));
        }),
        addition("harness wire", |s| s.harness.wires.push(harness_wire(0x18))),
        addition("harness entry type", |s| {
            s.harness.entry_types.insert(id(0x41), "SPI_BUS".into());
        }),
        addition("union", |s| s.unions.push(union(0x50, Vec::new()))),
        addition("bezier", |s| s.drawings.push(bezier())),
        addition("dimension", |s| s.drawings.push(dimension())),
        addition("table", |s| s.drawings.push(table())),
        addition("net parameters", |s| s.directives.push(net_parameters())),
        addition("blanket track width", |s| {
            s.directives.push(blanket(json!({ "track_width": 0.5 })));
        }),
        addition("blanket keep-short pour", |s| {
            s.directives.push(blanket(json!({ "pour": "keep_short" })));
        }),
        addition("blanket edge nets", |s| {
            s.directives
                .push(blanket(json!({ "include_edge_nets": true })));
        }),
        addition("diff pair class", |s| {
            s.directives.push(diff_pair(Some("USB")));
        }),
    ]
}

/// The base every single addition is made on: a sheet symbol with the sheet
/// pin an entry type names, which does not raise the token on its own.
fn base_sheet() -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.child_sheets.push(child_sheet());
    sheet
}

#[test]
fn a_sheet_without_refinement_content_writes_1_1_and_none_of_the_keys() {
    let mut sheet = base_sheet();
    sheet.directives.push(blanket(json!({})));
    sheet.directives.push(diff_pair(None));
    let text = save(&sheet);
    assert_eq!(token(&text), SNXSCH_FORMAT_V1, "{text}");
    for key in [
        "harness_connectors",
        "harness_wires",
        "harness_entry_types",
        "unions",
        "track_width",
        "pour",
        "include_edge_nets",
        "\nclass = ",
    ] {
        assert!(!text.contains(key), "{key} written:\n{text}");
    }
    assert_eq!(save(&load(&text)), text, "byte-identical");

    // A No-ERC rule list still writes 1.2.
    let mut marker = crate::schematic::NoErcDirective::new(id(0x60), Point::new(5.08, 7.62));
    marker.rules = vec!["builtin::unused_pin".to_string()];
    sheet.no_erc_directives.push(marker);
    assert_eq!(token(&save(&sheet)), SNXSCH_FORMAT_NO_ERC_RULES);
}

#[test]
fn each_refinement_object_alone_writes_1_3() {
    assert_eq!(SNXSCH_FORMAT_REFINEMENT, "snxsch/1.3");
    assert_eq!(token(&save(&base_sheet())), SNXSCH_FORMAT_V1);
    for (what, add) in single_additions() {
        let mut sheet = base_sheet();
        add(&mut sheet);
        let text = save(&sheet);
        assert_eq!(token(&text), SNXSCH_FORMAT_REFINEMENT, "{what}:\n{text}");
        let file = SnxSchematic::parse(&text).unwrap();
        assert!(!file.is_from_newer_minor(), "{what}: this build edits 1.3");
        assert_eq!(save(&file.sheet), text, "{what}: byte-identical re-save");
    }
}

#[test]
fn a_refinement_object_wins_over_a_no_erc_rule_list() {
    let mut sheet = base_sheet();
    let mut marker = crate::schematic::NoErcDirective::new(id(0x60), Point::new(5.08, 7.62));
    marker.rules = vec!["builtin::unused_pin".to_string()];
    sheet.no_erc_directives.push(marker);
    sheet.unions.push(union(0x50, Vec::new()));
    assert_eq!(token(&save(&sheet)), SNXSCH_FORMAT_REFINEMENT);
    // Removing the last refinement object goes back to the lower token.
    sheet.unions.clear();
    assert_eq!(token(&save(&sheet)), SNXSCH_FORMAT_NO_ERC_RULES);
}

/// A sheet carrying every new object, each with non-default values.
fn full_sheet() -> SchematicSheet {
    let mut sheet = base_sheet();
    sheet.wires.push(Wire {
        uuid: id(0x70),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.16, 0.0),
        ..Default::default()
    });
    sheet.harness.connectors.push(connector(0x10, "SPI_BUS"));
    let mut second = connector(0x14, "UART");
    second.rotation = 0.0;
    second.entry_pitch = 5.08;
    second.locked = false;
    second.position = Point::new(12.7, 88.9);
    sheet.harness.connectors.push(second);
    sheet.harness.wires.push(harness_wire(0x18));
    sheet.harness.entry_types.insert(id(0x41), "SPI_BUS".into());
    sheet
        .unions
        .push(union(0x50, vec![id(0x70), id(0x10), id(0x21)]));
    sheet.unions.push(SchUnion {
        uuid: id(0x51),
        name: "Flags".into(),
        tags: Vec::new(),
        members: vec![id(0x31)],
        locked: false,
    });
    let mut locked_bezier = bezier();
    locked_bezier.set_locked(true);
    sheet.drawings.push(locked_bezier);
    sheet.drawings.push(dimension());
    sheet.drawings.push(table());
    sheet.directives.push(net_parameters());
    sheet.directives.push(blanket(json!({
        "track_width": 0.5, "pour": "keep_short", "include_edge_nets": true,
    })));
    sheet.directives.push(diff_pair(Some("USB")));
    sheet
}

#[test]
fn every_refinement_object_round_trips_byte_identical() {
    let sheet = full_sheet();
    let text = save(&sheet);
    assert_eq!(token(&text), SNXSCH_FORMAT_REFINEMENT, "{text}");
    for key in [
        "[[extras.sheet.harness_connectors]]",
        "[[extras.sheet.harness_connectors.entries]]",
        "[[extras.sheet.harness_wires]]",
        "[extras.sheet.harness_entry_types]",
        "[[extras.sheet.unions]]",
    ] {
        assert!(text.contains(key), "{key} missing:\n{text}");
    }

    let back = load(&text);
    assert!(back.unknown.load_repairs.is_empty(), "{back:?}");
    assert_eq!(back.harness, sheet.harness);
    assert_eq!(back.unions, sheet.unions);
    assert_eq!(back.drawings, sheet.drawings);
    assert!(back.drawings[0].is_locked(), "the Bezier's lock survives");
    assert_eq!(back.directives, sheet.directives);
    assert_eq!(back.child_sheets, sheet.child_sheets);
    assert_eq!(back.wires, sheet.wires);
    assert_eq!(save(&back), text, "byte-identical re-save");
}

/// A Bezier's lock is kept beside the drawings, like every drawing's.
#[test]
fn a_locked_new_drawing_is_listed_in_locked_objects() {
    let mut sheet = empty_sheet();
    let mut drawing = bezier();
    drawing.set_locked(true);
    sheet.drawings.push(drawing);
    let text = save(&sheet);
    assert!(
        text.contains(&format!("locked_objects = [\"{}\"]", id(0x21))),
        "{text}"
    );
    assert!(load(&text).drawings[0].is_locked());
}

#[test]
fn a_harness_object_lock_is_written_only_when_set() {
    let mut sheet = empty_sheet();
    sheet.harness.connectors.push(connector(0x10, "SPI_BUS"));
    sheet.harness.wires.push(harness_wire(0x18));
    let text = save(&sheet);
    assert_eq!(text.matches("locked = true").count(), 1, "{text}");
    assert!(!text.contains("locked = false"), "{text}");
    let back = load(&text);
    assert!(back.harness.connectors[0].locked);
    assert!(!back.harness.wires[0].locked);
}

#[test]
fn entry_types_for_missing_sheet_pins_are_dropped_on_save() {
    let ghost = id(0x4f);
    let mut sheet = base_sheet();
    sheet.harness.entry_types.insert(id(0x41), "SPI_BUS".into());
    sheet.harness.entry_types.insert(ghost, "UART".into());
    let text = save(&sheet);
    assert!(
        text.contains(&format!("{} = \"SPI_BUS\"", id(0x41))),
        "{text}"
    );
    assert!(!text.contains(&ghost.to_string()), "{text}");
    let back = load(&text);
    assert_eq!(back.harness.entry_types.len(), 1);
    assert_eq!(back.harness.entry_types[&id(0x41)], "SPI_BUS");

    // Only a stale entry: nothing is written and the token stays 1.1.
    let mut stale = base_sheet();
    stale.harness.entry_types.insert(ghost, "UART".into());
    let text = save(&stale);
    assert_eq!(token(&text), SNXSCH_FORMAT_V1, "{text}");
    assert!(!text.contains("harness_entry_types"), "{text}");
}

#[test]
fn an_entry_type_for_a_missing_sheet_pin_is_dropped_on_load() {
    let ghost = id(0x4f);
    let mut sheet = base_sheet();
    sheet.harness.entry_types.insert(id(0x41), "SPI_BUS".into());
    let text = save(&sheet).replace(
        "[extras.sheet.harness_entry_types]\n",
        &format!("[extras.sheet.harness_entry_types]\n{ghost} = \"UART\"\n"),
    );
    assert!(text.contains(&ghost.to_string()), "fixture edit applied");
    let back = load(&text);
    assert_eq!(back.harness.entry_types.len(), 1, "{back:?}");
    assert!(!back.harness.entry_types.contains_key(&ghost));
}

#[test]
fn an_entry_type_key_that_is_not_a_uuid_refuses_the_file_naming_it() {
    let mut sheet = base_sheet();
    sheet.harness.entry_types.insert(id(0x41), "SPI_BUS".into());
    let text = save(&sheet).replace(
        "[extras.sheet.harness_entry_types]\n",
        "[extras.sheet.harness_entry_types]\nnot-a-uuid = \"UART\"\n",
    );
    let err = SnxSchematic::parse(&text).unwrap_err();
    assert!(
        matches!(&err, FormatError::KeyNotAUuid { key, .. } if key == "not-a-uuid"),
        "{err:?}"
    );
    let message = err.to_string();
    assert!(message.contains("harness_entry_types"), "{message}");
    assert!(message.contains("not-a-uuid"), "{message}");
}

#[test]
fn union_members_naming_missing_objects_are_dropped_on_load() {
    let ghost = id(0x7f);
    let mut sheet = full_sheet();
    sheet.unions = vec![
        union(0x50, vec![id(0x70), ghost, id(0x11), id(0x41)]),
        SchUnion {
            uuid: id(0x52),
            name: "Gone".into(),
            tags: Vec::new(),
            members: vec![ghost],
            locked: false,
        },
    ];
    let text = save(&sheet);
    assert!(
        text.contains(&ghost.to_string()),
        "written as held:\n{text}"
    );
    let back = load(&text);
    // A wire, a harness entry and a sheet pin are all objects a union can list.
    assert_eq!(back.unions[0].members, vec![id(0x70), id(0x11), id(0x41)]);
    // A union left without members is kept.
    assert_eq!(back.unions.len(), 2);
    assert!(back.unions[1].members.is_empty());
    assert!(!save(&back).contains(&ghost.to_string()));
}

#[test]
fn repeated_harness_and_union_uuids_are_repaired_on_load() {
    let mut sheet = empty_sheet();
    let first = connector(0x10, "SPI_BUS");
    let mut collided = connector(0x10, "UART");
    // The same entry ids too, so both the connector and its entries collide.
    collided.position = Point::new(0.0, 0.0);
    sheet.harness.connectors = vec![first, collided];
    sheet.unions = vec![union(0x50, Vec::new()), union(0x50, Vec::new())];
    let back = load(&save(&sheet));

    let kinds: Vec<&str> = back
        .unknown
        .load_repairs
        .iter()
        .map(|repair| match repair {
            SheetLoadRepair::UuidReminted { object, .. }
            | SheetLoadRepair::CopyDropped { object, .. } => object.kind,
            other => panic!("unexpected repair {other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "harness connector",
            "harness entry",
            "harness entry",
            "union"
        ],
        "{:?}",
        back.unknown.load_repairs
    );
    assert_eq!(back.harness.connectors.len(), 2);
    assert_ne!(
        back.harness.connectors[0].uuid,
        back.harness.connectors[1].uuid
    );
    assert_ne!(
        back.harness.connectors[0].entries[0].uuid,
        back.harness.connectors[1].entries[0].uuid
    );
    // The second union was an exact copy: dropped.
    assert_eq!(back.unions.len(), 1);
    // Every parse of the file agrees on the new ids.
    let again = load(&save(&sheet));
    assert_eq!(again.harness, back.harness);
}

#[test]
fn an_entry_type_follows_a_re_minted_sheet_pin() {
    let mut sheet = base_sheet();
    let mut twin = child_sheet();
    twin.uuid = id(0x42);
    twin.name = "SPI Flash B".into();
    twin.position = Point::new(152.4, 50.8);
    sheet.child_sheets.push(twin);
    sheet.harness.entry_types.insert(id(0x41), "SPI_BUS".into());
    let back = load(&save(&sheet));
    let pins: Vec<Uuid> = back
        .child_sheets
        .iter()
        .map(|child| child.pins[0].uuid)
        .collect();
    assert_ne!(pins[0], pins[1], "the second pin was re-minted");
    for pin in pins {
        assert_eq!(back.harness.entry_types[&pin], "SPI_BUS");
    }
}
