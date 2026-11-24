//! Bottom-up hierarchy (#1276): the sync preview in both directions, the
//! commands that apply it and their undo, and the sheet symbol built from a
//! sheet.

use signex_types::object_id::SequentialIds;
use signex_types::schematic::{
    ChildSheet, Label, LabelType, Point, SchematicSheet, SheetPin, normalize_port_name,
};

use crate::{
    Command, Engine, SheetEntryRename, SheetPort, SheetSyncPreview, SymbolEntryRenames,
    entry_changes_from_sheet, exposed_sheet_ports, port_changes_from_symbol, port_sync_commands,
    ports_for_pin_sync, sheet_symbol_from_sheet, sheet_symbol_spot, sync_from_sheet_command,
    unused_sheet_symbol_name,
};

const FILE: &str = "child.snxsch";

fn port(name: &str, direction: &str) -> SheetPort {
    SheetPort {
        name: name.to_string(),
        direction: direction.to_string(),
    }
}

fn pin(name: &str, direction: &str, y: f64, auto_generated: bool) -> SheetPin {
    SheetPin {
        uuid: uuid::Uuid::now_v7(),
        name: name.to_string(),
        direction: direction.to_string(),
        position: Point::new(if direction == "output" { 130.0 } else { 100.0 }, y),
        rotation: if direction == "output" { 180.0 } else { 0.0 },
        auto_generated,
        user_moved: false,
    }
}

fn symbol(pins: Vec<SheetPin>) -> ChildSheet {
    ChildSheet {
        pins,
        filename: FILE.to_string(),
        ..crate::child_sheet_from_corners(
            Point::new(100.0, 50.0),
            Point::new(130.0, 100.0),
            "child",
            &mut SequentialIds::default(),
        )
        .expect("big enough")
    }
}

fn label(text: &str, label_type: LabelType, shape: &str, y: f64) -> Label {
    Label {
        shape: shape.to_string(),
        ..Label::new(text, Point::new(10.0, y), label_type)
    }
}

fn names(preview_names: &[String]) -> Vec<&str> {
    preview_names.iter().map(String::as_str).collect()
}

fn rename(from: &str, to: &str) -> SheetEntryRename {
    SheetEntryRename {
        from: from.to_string(),
        to: to.to_string(),
    }
}

#[test]
fn sheet_to_symbol_lists_added_removed_and_a_lone_rename() {
    let entries = symbol(vec![
        pin("A", "input", 52.54, true),
        pin("OLD", "output", 52.54, true),
        pin("GONE", "passive", 55.08, true),
        pin("HAND", "input", 55.08, false),
    ]);
    let ports = [
        port("A", "input"),
        port("NEW", "output"),
        port("X", "input"),
        port("Y", "input"),
    ];
    let preview = entry_changes_from_sheet(&entries, &ports);
    assert_eq!(preview.renamed, vec![rename("OLD", "NEW")]);
    assert_eq!(names(&preview.added), ["X", "Y"]);
    assert_eq!(
        names(&preview.removed),
        ["GONE"],
        "a hand-placed entry stays"
    );
}

#[test]
fn two_names_of_one_direction_on_each_side_are_no_rename() {
    let entries = symbol(vec![
        pin("P1", "input", 52.54, true),
        pin("P2", "input", 55.08, true),
    ]);
    let preview = entry_changes_from_sheet(&entries, &[port("Q1", "input"), port("Q2", "input")]);
    assert!(preview.renamed.is_empty());
    assert_eq!(names(&preview.added), ["Q1", "Q2"]);
    assert_eq!(names(&preview.removed), ["P1", "P2"]);
}

#[test]
fn agreeing_sides_preview_nothing() {
    let entries = symbol(vec![pin("A", "input", 52.54, true)]);
    assert!(entry_changes_from_sheet(&entries, &[port("A", "input")]).is_empty());
}

/// The parent with one sheet symbol, and the child it stands for.
fn parent_and_child() -> (SchematicSheet, SchematicSheet) {
    let parent = SchematicSheet {
        child_sheets: vec![symbol(vec![
            pin("A", "input", 52.54, true),
            pin("OLD", "output", 52.54, true),
            pin("GONE", "passive", 55.08, true),
        ])],
        ..SchematicSheet::default()
    };
    let child = SchematicSheet {
        labels: vec![
            label("A", LabelType::Hierarchical, "input", 10.0),
            label("NEW", LabelType::Hierarchical, "output", 20.0),
            label("X", LabelType::Global, "input", 30.0),
        ],
        ..SchematicSheet::default()
    };
    (parent, child)
}

#[test]
fn sheet_to_symbol_renames_in_place_adds_and_drops_then_undoes() {
    let (parent, child) = parent_and_child();
    let preview = entry_changes_from_sheet(&parent.child_sheets[0], &exposed_sheet_ports(&child));
    assert_eq!(preview.renamed, vec![rename("OLD", "NEW")]);
    assert_eq!(names(&preview.added), ["X"]);
    assert_eq!(names(&preview.removed), ["GONE"]);
    let target = parent.child_sheets[0].clone();
    let old = target.pins[1].clone();
    let ports = ports_for_pin_sync(&child, &parent, FILE);
    let mut engine = Engine::new(parent).expect("engine");
    let before = engine.document().child_sheets.clone();
    let result = engine
        .execute(sync_from_sheet_command(&target, ports, &preview))
        .expect("sync");
    assert!(result.changed);

    let pins = &engine.document().child_sheets[0].pins;
    let renamed = pins
        .iter()
        .find(|p| p.name == "NEW")
        .expect("renamed entry");
    assert_eq!(
        (renamed.uuid, renamed.position, renamed.direction.as_str()),
        (old.uuid, old.position, "output"),
        "a renamed entry keeps its place, id and direction"
    );
    let mut now: Vec<&str> = pins.iter().map(|p| p.name.as_str()).collect();
    now.sort_unstable();
    assert_eq!(now, ["A", "NEW", "X"]);

    engine.undo().expect("undo");
    assert_eq!(engine.document().child_sheets, before);
}

#[test]
fn a_rename_onto_a_taken_name_leaves_the_entries() {
    let parent = SchematicSheet {
        child_sheets: vec![symbol(vec![
            pin("OLD", "input", 52.54, true),
            pin("NEW", "input", 55.08, false),
        ])],
        ..SchematicSheet::default()
    };
    let symbol_id = parent.child_sheets[0].uuid;
    let mut engine = Engine::new(parent).expect("engine");
    let before = engine.document().child_sheets.clone();
    let command = Command::ReconcileChildSheetPins {
        child_filename: FILE.to_string(),
        ports: vec![port("OLD", "input"), port("NEW", "input")],
        renames: Some(SymbolEntryRenames {
            symbol: symbol_id,
            renames: vec![rename("OLD", "NEW")],
        }),
    };
    let result = engine.execute(command).expect("sync");
    assert!(!result.changed);
    assert_eq!(engine.document().child_sheets, before);
}

/// The rename is made for one sheet symbol: a hand-placed entry with the old
/// name on another symbol of the same file keeps its name.
#[test]
fn a_rename_stays_on_the_previewed_symbol_and_its_generated_entries() {
    let previewed = symbol(vec![
        pin("A", "input", 52.54, true),
        pin("OLD", "output", 52.54, true),
    ]);
    let hand = pin("OLD", "output", 60.16, false);
    let other = ChildSheet {
        uuid: uuid::Uuid::now_v7(),
        ..symbol(vec![hand.clone()])
    };
    let ports = vec![port("A", "input"), port("NEW", "output")];
    let preview = entry_changes_from_sheet(&previewed, &ports);
    assert_eq!(preview.renamed, vec![rename("OLD", "NEW")]);
    let renamed_id = previewed.pins[1].uuid;
    let parent = SchematicSheet {
        child_sheets: vec![previewed.clone(), other],
        ..SchematicSheet::default()
    };
    let mut engine = Engine::new(parent).expect("engine");
    let result = engine
        .execute(sync_from_sheet_command(&previewed, ports, &preview))
        .expect("sync");
    assert!(result.changed);

    let sheets = &engine.document().child_sheets;
    let on = |idx: usize, id: uuid::Uuid| {
        sheets[idx]
            .pins
            .iter()
            .find(|p| p.uuid == id)
            .map(|p| (p.name.clone(), p.auto_generated))
    };
    assert_eq!(on(0, renamed_id), Some(("NEW".to_string(), true)));
    assert_eq!(
        on(1, hand.uuid),
        Some(("OLD".to_string(), false)),
        "the other symbol's hand-placed entry keeps its name"
    );
}

/// The child for the Symbol to Sheet tests: one port to rename, one carried
/// by two labels to remove, and a Global label that is never touched.
fn child_for_push() -> SchematicSheet {
    SchematicSheet {
        labels: vec![
            label("OLD", LabelType::Hierarchical, "output", 10.0),
            label("STALE", LabelType::Hierarchical, "input", 20.0),
            label("STALE", LabelType::Hierarchical, "input", 30.0),
            label("VCC", LabelType::Global, "input", 40.0),
        ],
        ..SchematicSheet::default()
    }
}

fn symbol_for_push() -> ChildSheet {
    symbol(vec![
        pin("NEW", "output", 52.54, false),
        pin("VCC", "input", 52.54, true),
        pin("IN1", "input", 55.08, false),
        pin("IN2", "input", 57.62, false),
    ])
}

#[test]
fn symbol_to_sheet_lists_ports_to_add_remove_and_rename() {
    let preview = port_changes_from_symbol(&symbol_for_push(), &child_for_push());
    assert_eq!(
        preview,
        SheetSyncPreview {
            added: vec!["IN1".to_string(), "IN2".to_string()],
            removed: vec!["STALE".to_string()],
            renamed: vec![rename("OLD", "NEW")],
        }
    );
}

#[test]
fn symbol_to_sheet_commands_apply_as_one_step_and_undo() {
    let child = child_for_push();
    let entries = symbol_for_push();
    let preview = port_changes_from_symbol(&entries, &child);
    let old = child.labels[0].clone();
    let commands = port_sync_commands(&child, &entries, &preview, &mut SequentialIds::default());
    let mut engine = Engine::new(child).expect("engine");
    let before = engine.document().labels.clone();
    let result = engine.execute_batch(commands).expect("push");
    assert!(result.changed);

    let after = engine.document();
    let renamed = after
        .labels
        .iter()
        .find(|l| l.uuid == old.uuid)
        .expect("kept");
    assert_eq!(renamed.text, "NEW", "the port is renamed in place");
    assert!(!after.labels.iter().any(|l| l.text == "STALE"));
    let added: Vec<&Label> = after
        .labels
        .iter()
        .filter(|l| l.text == "IN1" || l.text == "IN2")
        .collect();
    assert_eq!(added.len(), 2);
    assert!(
        added
            .iter()
            .all(|l| l.label_type == LabelType::Hierarchical && l.shape == "input")
    );
    assert_ne!(added[0].position, added[1].position, "one row each");
    let global = after
        .labels
        .iter()
        .find(|l| l.text == "VCC")
        .expect("global");
    assert_eq!(
        global.label_type,
        LabelType::Global,
        "a Global label is never touched"
    );
    let mut ports: Vec<String> = exposed_sheet_ports(after)
        .into_iter()
        .map(|p| p.name)
        .collect();
    ports.sort_unstable();
    assert_eq!(ports, ["IN1", "IN2", "NEW", "VCC"]);
    assert!(port_changes_from_symbol(&entries, after).is_empty());

    engine.undo().expect("undo");
    assert_eq!(engine.document().labels, before);
}

#[test]
fn a_symbol_from_a_sheet_has_an_entry_for_every_port_and_places_as_one_step() {
    let child = SchematicSheet {
        labels: vec![
            label("A", LabelType::Hierarchical, "input", 10.0),
            label("Q", LabelType::Hierarchical, "output", 20.0),
            label("G", LabelType::Global, "", 30.0),
            label("A", LabelType::Global, "input", 40.0),
        ],
        ..SchematicSheet::default()
    };
    let at = Point::new(50.8, 50.8);
    let made = sheet_symbol_from_sheet(&child, FILE, "child", at, &mut SequentialIds::default());
    assert_eq!(
        (made.name.as_str(), made.filename.as_str()),
        ("child", FILE)
    );
    assert_eq!(made.position, at);
    let mut entry_names: Vec<&str> = made
        .pins
        .iter()
        .filter_map(|p| normalize_port_name(&p.name))
        .collect();
    entry_names.sort_unstable();
    assert_eq!(entry_names, ["A", "G", "Q"]);
    assert!(made.pins.iter().all(|p| p.auto_generated));
    let q = made.pins.iter().find(|p| p.name == "Q").expect("Q");
    assert_eq!((q.position.x, q.rotation), (at.x + made.size.0, 180.0));
    let a = made.pins.iter().find(|p| p.name == "A").expect("A");
    assert_eq!((a.position.x, a.rotation), (at.x, 0.0));
    assert!(
        made.pins
            .iter()
            .all(|p| p.position.y > at.y && p.position.y < at.y + made.size.1),
        "every entry sits on the edge, off the corners"
    );
    assert!(entry_changes_from_sheet(&made, &exposed_sheet_ports(&child)).is_empty());
    assert!(port_changes_from_symbol(&made, &child).is_empty());

    let mut engine = Engine::new(SchematicSheet::default()).expect("engine");
    let placed = engine
        .execute(Command::PlaceChildSheet {
            child_sheet: made.clone(),
        })
        .expect("place");
    assert!(placed.changed);
    assert_eq!(engine.document().child_sheets, vec![made]);
    engine.undo().expect("undo");
    assert!(engine.document().child_sheets.is_empty());
}

#[test]
fn a_taken_symbol_name_gets_a_number_and_an_empty_sheet_a_spot() {
    let (parent, _) = parent_and_child();
    assert_eq!(unused_sheet_symbol_name(&parent, "power"), "power");
    assert_eq!(unused_sheet_symbol_name(&parent, "child"), "child_2");
    let empty = SchematicSheet::default();
    let spot = sheet_symbol_spot(&empty);
    let below = sheet_symbol_spot(&parent);
    assert!(below.y > 100.0, "under the lowest sheet symbol: {below:?}");
    assert!(spot.y < below.y);
}

/// Mockup row 79 (#2831): a removal left out goes; every other change stays.
#[test]
fn leaving_out_a_removal_keeps_every_other_change() {
    let rename = SheetEntryRename {
        from: "OLD".to_string(),
        to: "NEW".to_string(),
    };
    let preview = SheetSyncPreview {
        added: vec!["IN".to_string()],
        removed: vec!["A".to_string(), "B".to_string()],
        renamed: vec![rename.clone()],
    };
    let kept = std::collections::BTreeSet::from(["A".to_string()]);
    let chosen = preview.leaving_out(&kept);
    assert_eq!(chosen.removed, ["B"]);
    assert_eq!(chosen.added, ["IN"]);
    assert_eq!(chosen.renamed, [rename]);
    assert_eq!(preview.removed, ["A", "B"], "the preview is left as it was");
}
