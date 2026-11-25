//! Sheet entry sync on a repeated sheet symbol (#2659): a `Repeat(<net>)`
//! entry meets the child's port `<net>`, so neither direction offers to add
//! `<net>` again, drop the repeat entry or add a `Repeat(<net>)` port.

use signex_types::object_id::SequentialIds;
use signex_types::schematic::{ChildSheet, Label, LabelType, Point, SchematicSheet, SheetPin};

use crate::{
    Engine, SheetEntryChange, SheetEntryRename, SheetPort, entry_changes_from_sheet,
    exposed_sheet_ports, port_changes_from_symbol, port_sync_commands, ports_for_pin_sync,
    sheet_ports_after_sync, symbol_entries_after_sync, sync_from_sheet_command,
    unplaced_sheet_ports,
};

const FILE: &str = "buck_channel.snxsch";

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

fn repeated_symbol(pins: Vec<SheetPin>) -> ChildSheet {
    ChildSheet {
        pins,
        filename: FILE.to_string(),
        ..crate::child_sheet_from_corners(
            Point::new(100.0, 50.0),
            Point::new(130.0, 100.0),
            "Repeat(CH, 1, 4)",
            &mut SequentialIds::default(),
        )
        .expect("big enough")
    }
}

fn child(ports: &[(&str, &str)]) -> SchematicSheet {
    SchematicSheet {
        labels: ports
            .iter()
            .enumerate()
            .map(|(row, (name, shape))| Label {
                shape: (*shape).to_string(),
                ..Label::new(
                    *name,
                    Point::new(10.0, 10.0 * (row + 1) as f64),
                    LabelType::Hierarchical,
                )
            })
            .collect(),
        ..SchematicSheet::default()
    }
}

/// Mockup row 151's symbol: a hand-placed `Repeat(PGOOD)`, shared VIN_12V
/// and a generated `Repeat(VOUT_3V3)`, in the order a sync lists the ports.
fn buck() -> ChildSheet {
    repeated_symbol(vec![
        pin(" Repeat( PGOOD ) ", "output", 55.08, false),
        pin("VIN_12V", "input", 52.54, true),
        pin("Repeat(VOUT_3V3)", "output", 52.54, true),
    ])
}

/// The child. Its VOUT_3V3 label is passive, so a sync that reads the
/// direction off the label and not the entry shows it.
fn buck_child() -> SchematicSheet {
    child(&[
        ("VIN_12V", "input"),
        ("VOUT_3V3", "passive"),
        ("PGOOD", "output"),
    ])
}

#[test]
fn a_repeat_entry_and_its_port_agree_in_both_directions() {
    let (symbol, child) = (buck(), buck_child());
    assert!(entry_changes_from_sheet(&symbol, &exposed_sheet_ports(&child)).is_empty());
    assert!(port_changes_from_symbol(&symbol, &child).is_empty());
}

#[test]
fn sheet_to_symbol_keeps_the_repeat_entries_and_adds_no_duplicate() {
    let (symbol, child) = (buck(), buck_child());
    let parent = SchematicSheet {
        child_sheets: vec![symbol.clone()],
        ..SchematicSheet::default()
    };
    let ports = ports_for_pin_sync(&child, &parent, FILE);
    let vout = ports
        .iter()
        .find(|p| p.name == "VOUT_3V3")
        .expect("the port");
    assert_eq!(vout.direction, "output", "the repeat entry's direction");
    let preview = entry_changes_from_sheet(&symbol, &exposed_sheet_ports(&child));
    let mut engine = Engine::new(parent).expect("engine");
    let before = engine.document().child_sheets.clone();

    let result = engine
        .execute(sync_from_sheet_command(&symbol, ports.clone(), &preview))
        .expect("sync");

    assert!(!result.changed, "nothing to add, drop or rename");
    assert_eq!(engine.document().child_sheets, before);
    assert!(unplaced_sheet_ports(engine.document(), FILE, &ports).is_empty());
}

#[test]
fn a_renamed_port_renames_the_repeat_entry_and_keeps_it_a_repeat() {
    let symbol = buck();
    let child = child(&[
        ("VIN_12V", "input"),
        ("VOUT_5V", "output"),
        ("PGOOD", "output"),
    ]);
    let preview = entry_changes_from_sheet(&symbol, &exposed_sheet_ports(&child));
    assert_eq!(
        preview.renamed,
        vec![SheetEntryRename {
            from: "Repeat(VOUT_3V3)".to_string(),
            to: "VOUT_5V".to_string(),
        }]
    );
    assert!(preview.added.is_empty() && preview.removed.is_empty());
    let listed = symbol_entries_after_sync(&symbol, &child, &preview);
    let renamed = listed
        .iter()
        .find(|e| matches!(e.change, SheetEntryChange::Renamed { .. }))
        .expect("listed");
    assert_eq!(
        renamed.name, "Repeat(VOUT_5V)",
        "the dialog shows the result"
    );

    let id = symbol.pins[2].uuid;
    let parent = SchematicSheet {
        child_sheets: vec![symbol.clone()],
        ..SchematicSheet::default()
    };
    let ports = ports_for_pin_sync(&child, &parent, FILE);
    let mut engine = Engine::new(parent).expect("engine");
    engine
        .execute(sync_from_sheet_command(&symbol, ports, &preview))
        .expect("sync");

    let pins = &engine.document().child_sheets[0].pins;
    let entry = pins.iter().find(|p| p.uuid == id).expect("kept");
    assert_eq!(entry.name, "Repeat(VOUT_5V)");
    assert_eq!(pins.len(), 3, "no plain VOUT_5V entry is added");
}

#[test]
fn a_repeat_entry_whose_port_is_gone_is_removed_as_written() {
    let symbol = buck();
    let child = child(&[("VIN_12V", "input"), ("PGOOD", "output")]);
    let preview = entry_changes_from_sheet(&symbol, &exposed_sheet_ports(&child));
    assert_eq!(preview.removed, ["Repeat(VOUT_3V3)"]);
    assert!(preview.added.is_empty() && preview.renamed.is_empty());
}

#[test]
fn symbol_to_sheet_adds_the_plain_port_of_a_new_repeat_entry() {
    let mut symbol = buck();
    symbol
        .pins
        .push(pin("Repeat(ISENSE)", "output", 57.62, false));
    let child = buck_child();
    let preview = port_changes_from_symbol(&symbol, &child);
    assert_eq!(preview.added, ["ISENSE"]);
    assert!(preview.removed.is_empty() && preview.renamed.is_empty());
    let listed = sheet_ports_after_sync(&symbol, &child, &preview);
    let added = listed.iter().find(|e| e.name == "ISENSE").expect("listed");
    assert_eq!(added.direction, "output", "the entry's direction");

    let commands = port_sync_commands(&child, &symbol, &preview, &mut SequentialIds::default());
    let mut engine = Engine::new(child).expect("engine");
    engine.execute_batch(commands).expect("push");

    let ports: Vec<SheetPort> = exposed_sheet_ports(engine.document());
    let isense = ports.iter().find(|p| p.name == "ISENSE").expect("added");
    assert_eq!(isense.direction, "output");
    assert!(port_changes_from_symbol(&symbol, engine.document()).is_empty());
}
