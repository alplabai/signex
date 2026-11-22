//! Replay of a command that carries a whole object or a whole pin list, onto
//! a sheet that has moved on. A replaced object takes only the fields the
//! recorded edit changed; a list changed on both sides that cannot be merged
//! and a Sheet Entries sync whose list no longer fits are refused. Rules:
//! `docs/COMMAND_LOG.md`, "Replay".

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use signex_engine::{Command, SheetPort, SymbolEntryRenames};
use signex_types::schematic::{ChildSheet, SchematicSheet, normalize_port_name};

/// The object `command` replaces, as it stands on `sheet`, in the same form.
fn image_on(sheet: &SchematicSheet, command: &Command) -> Option<Command> {
    match command {
        Command::UpdateSchDrawing { drawing } => (sheet.drawings.iter())
            .find(|d| d.uuid() == drawing.uuid())
            .map(|d| Command::UpdateSchDrawing { drawing: d.clone() }),
        Command::UpdateDirective { directive } => (sheet.directives.iter())
            .find(|d| d.uuid() == directive.uuid())
            .map(|d| Command::UpdateDirective {
                directive: d.clone(),
            }),
        Command::UpdateHarnessConnector { uuid, .. } => (sheet.harness.connectors.iter())
            .find(|c| c.uuid == *uuid)
            .map(|c| Command::UpdateHarnessConnector {
                uuid: *uuid,
                connector: c.clone(),
            }),
        _ => None,
    }
}

fn replaces_whole_object(command: &Command) -> bool {
    matches!(
        command,
        Command::UpdateSchDrawing { .. }
            | Command::UpdateDirective { .. }
            | Command::UpdateHarnessConnector { .. }
    )
}

/// Whether every one of `commands` stores a whole object with a pre-image.
pub(super) fn only_replaces_objects(commands: &[Command]) -> bool {
    !commands.is_empty() && commands.iter().all(replaces_whole_object)
}

fn same_object(a: &Command, b: &Command) -> bool {
    match (a, b) {
        (Command::UpdateSchDrawing { drawing: a }, Command::UpdateSchDrawing { drawing: b }) => {
            a.uuid() == b.uuid()
        }
        (Command::UpdateDirective { directive: a }, Command::UpdateDirective { directive: b }) => {
            a.uuid() == b.uuid()
        }
        (
            Command::UpdateHarnessConnector { uuid: a, .. },
            Command::UpdateHarnessConnector { uuid: b, .. },
        ) => a == b,
        _ => false,
    }
}

/// What `commands` are about to replace, read before they run.
pub(super) fn pre_images(sheet: &SchematicSheet, commands: &[Command]) -> Vec<Command> {
    commands
        .iter()
        .filter_map(|command| image_on(sheet, command))
        .collect()
}

/// `commands` for the sheet as it is now, or `None` when a replaced object
/// can no longer take its recorded change.
pub(super) fn rebased(
    sheet: &SchematicSheet,
    commands: &[Command],
    pre_images: &[Command],
) -> Option<Vec<Command>> {
    commands
        .iter()
        .map(|command| rebase_one(sheet, command, pre_images))
        .collect()
}

fn rebase_one(
    sheet: &SchematicSheet,
    command: &Command,
    pre_images: &[Command],
) -> Option<Command> {
    if !replaces_whole_object(command) {
        return Some(command.clone());
    }
    let before = pre_images.iter().find(|p| same_object(p, command))?;
    let live = image_on(sheet, command)?;
    match (command, before, live) {
        (
            Command::UpdateSchDrawing { drawing },
            Command::UpdateSchDrawing { drawing: was },
            Command::UpdateSchDrawing { drawing: now },
        ) => Some(Command::UpdateSchDrawing {
            drawing: with_changes(was, drawing, &now)?,
        }),
        (
            Command::UpdateDirective { directive },
            Command::UpdateDirective { directive: was },
            Command::UpdateDirective { directive: now },
        ) => Some(Command::UpdateDirective {
            directive: with_changes(was, directive, &now)?,
        }),
        (
            Command::UpdateHarnessConnector { uuid, connector },
            Command::UpdateHarnessConnector { connector: was, .. },
            Command::UpdateHarnessConnector { connector: now, .. },
        ) => Some(Command::UpdateHarnessConnector {
            uuid: *uuid,
            connector: with_changes(was, connector, &now)?,
        }),
        _ => None,
    }
}

/// `live` with the fields that differ between `before` and `after`. `None`
/// when the three are not the same kind of object, or a changed list has
/// moved on in a way the change cannot be merged into.
fn with_changes<T: Serialize + DeserializeOwned>(before: &T, after: &T, live: &T) -> Option<T> {
    let before = serde_json::to_value(before).ok()?;
    let after = serde_json::to_value(after).ok()?;
    let mut live = serde_json::to_value(live).ok()?;
    if before.get("type") != after.get("type") || live.get("type") != after.get("type") {
        return None;
    }
    carry_changes(&before, &after, &mut live)?;
    serde_json::from_value(live).ok()
}

/// Objects are compared key by key, lists by [`carry_list`]; anything else is
/// one value. A list left out of the file when empty counts as empty. `None`
/// when a field the edit changed holds something else now.
fn carry_changes(before: &Value, after: &Value, live: &mut Value) -> Option<()> {
    if before == after {
        return Some(());
    }
    if before.is_array() || after.is_array() || live.is_array() {
        return carry_list(before, after, live);
    }
    if let (Value::Object(was), Value::Object(now), Value::Object(target)) =
        (before, after, &mut *live)
    {
        for (key, old) in was.iter().filter(|(key, _)| !now.contains_key(*key)) {
            if target.get(key).is_some_and(|value| value != old) {
                return None;
            }
            target.remove(key);
        }
        for (key, value) in now {
            let Some(old) = was.get(key) else {
                if let Some(slot) = target.get_mut(key) {
                    if value.is_array() {
                        carry_list(&Value::Array(Vec::new()), value, slot)?;
                    } else if slot != value {
                        return None;
                    }
                    continue;
                }
                target.insert(key.clone(), value.clone());
                continue;
            };
            if old == value {
                continue;
            }
            match target.get_mut(key) {
                Some(slot) => carry_changes(old, value, slot)?,
                // A list left out of the file is an empty one.
                None if old.is_array() || value.is_array() => {
                    let mut emptied = Value::Array(Vec::new());
                    carry_list(old, value, &mut emptied)?;
                    target.insert(key.clone(), emptied);
                }
                None => return None,
            }
        }
        return Some(());
    }
    if *live != *before && *live != *after {
        return None;
    }
    *live = after.clone();
    Some(())
}

/// Carry a list's change element by element, and an element that is an
/// object field by field. `None` unless all three lists have one length and
/// no single field changed on both sides to different values: a list that
/// grew, shrank or was edited at the same field has no safe merge.
fn carry_list(before: &Value, after: &Value, live: &mut Value) -> Option<()> {
    if before == after || *live == *after {
        return Some(());
    }
    if *live == *before {
        *live = after.clone();
        return Some(());
    }
    if before.is_object() && after.is_object() && live.is_object() {
        return carry_changes(before, after, live);
    }
    let (Value::Array(was), Value::Array(now), Value::Array(target)) = (before, after, live) else {
        return None;
    };
    if was.len() != now.len() || was.len() != target.len() {
        return None;
    }
    for ((old, new), slot) in was.iter().zip(now).zip(target.iter_mut()) {
        carry_list(old, new, slot)?;
    }
    Some(())
}

/// Whether a recorded sync would now drop a pin made since, or add one.
fn sync_no_longer_fits(sheet: &SchematicSheet, commands: &[Command]) -> bool {
    commands.iter().any(|command| match command {
        Command::ReconcileChildSheetPins {
            child_filename,
            ports,
            renames,
        } => (sheet.child_sheets.iter())
            .filter(|child| child.filename == *child_filename)
            .any(|child| pins_differ(child, ports, renames.as_ref())),
        _ => false,
    })
}

fn pins_differ(
    child: &ChildSheet,
    ports: &[SheetPort],
    renames: Option<&SymbolEntryRenames>,
) -> bool {
    let renames = renames
        .filter(|r| r.symbol == child.uuid)
        .map(|r| r.renames.as_slice())
        .unwrap_or_default();
    let names: Vec<&str> = (child.pins.iter())
        .filter_map(|pin| normalize_port_name(&pin.name))
        .collect();
    let drops_a_pin = (child.pins.iter())
        .filter(|pin| pin.auto_generated)
        .filter_map(|pin| normalize_port_name(&pin.name))
        .any(|name| {
            !ports.iter().any(|port| port.name == name) && !renames.iter().any(|r| r.from == name)
        });
    let adds_a_pin = ports.iter().any(|port| {
        !names.contains(&port.name.as_str()) && !renames.iter().any(|r| r.to == port.name)
    });
    drops_a_pin || adds_a_pin
}

/// Whether running `commands` now would undo what was done since they ran.
pub(super) fn is_stale(
    sheet: &SchematicSheet,
    commands: &[Command],
    pre_images: &[Command],
) -> bool {
    sync_no_longer_fits(sheet, commands) || rebased(sheet, commands, pre_images).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{FillType, Point, SchDrawing, SchTable, SheetPin};
    use uuid::Uuid;

    const ID: Uuid = Uuid::from_u128(0x50);

    fn rect(right: f64, fill: FillType) -> SchDrawing {
        SchDrawing::Rect {
            uuid: ID,
            start: Point::new(0.0, 0.0),
            end: Point::new(right, 10.0),
            width: 0.2,
            fill,
            stroke_color: None,
            locked: false,
        }
    }

    fn sheet_with(drawing: SchDrawing) -> SchematicSheet {
        SchematicSheet {
            drawings: vec![drawing],
            ..Default::default()
        }
    }

    /// A fill edit replayed after a move keeps the move.
    #[test]
    fn a_replaced_drawing_takes_only_the_field_the_edit_changed() {
        let edit = Command::UpdateSchDrawing {
            drawing: rect(20.0, FillType::Background),
        };
        let was = pre_images(
            &sheet_with(rect(20.0, FillType::None)),
            std::slice::from_ref(&edit),
        );
        let moved = sheet_with(rect(50.0, FillType::None));

        let commands = rebased(&moved, &[edit], &was).expect("it still fits");

        assert!(matches!(
            &commands[0],
            Command::UpdateSchDrawing { drawing } if *drawing == rect(50.0, FillType::Background)
        ));
    }

    fn table(rows: &[&[&str]]) -> SchDrawing {
        let mut table = SchTable::new_typed(ID, Point::new(0.0, 0.0));
        table.rows = rows
            .iter()
            .map(|row| row.iter().map(|cell| (*cell).to_string()).collect())
            .collect();
        SchDrawing::Table(table)
    }

    fn table_linking(links: &[&str]) -> SchDrawing {
        let mut table = SchTable::new_typed(ID, Point::new(0.0, 0.0));
        table.links = links.iter().map(|link| (*link).to_string()).collect();
        SchDrawing::Table(table)
    }

    /// A table whose first column reads `title` and is `shown` or not.
    fn table_column(title: &str, shown: bool) -> SchDrawing {
        let mut table = SchTable::new_typed(ID, Point::new(0.0, 0.0));
        table.columns[0].title = title.to_string();
        table.columns[0].shown = shown;
        SchDrawing::Table(table)
    }

    /// DECISIONS 2026-11-22: per field, inside a list's element too.
    #[test]
    fn a_nested_list_element_takes_the_edit_and_keeps_its_other_field() {
        let now = replayed(
            table_column("a", true),
            table_column("x", true),
            table_column("a", false),
        );
        assert_eq!(now, Some(table_column("x", false)));
    }

    #[test]
    fn the_same_field_of_a_nested_list_element_changed_since_refuses() {
        let now = replayed(
            table_column("a", true),
            table_column("x", true),
            table_column("z", true),
        );
        assert!(now.is_none());
    }

    /// DECISIONS 2026-11-22: another field changed since stays.
    #[test]
    fn a_properties_replay_keeps_another_field_changed_since() {
        let now = replayed(
            rect(20.0, FillType::None),
            rect(20.0, FillType::Background),
            rect(50.0, FillType::None),
        );
        assert_eq!(now, Some(rect(50.0, FillType::Background)));
    }

    /// The edit `was` to `edit`, replayed onto `live`; `None` when refused.
    fn replayed(was: SchDrawing, edit: SchDrawing, live: SchDrawing) -> Option<SchDrawing> {
        let edit = Command::UpdateSchDrawing { drawing: edit };
        let images = pre_images(&sheet_with(was), std::slice::from_ref(&edit));
        let live = sheet_with(live);
        let stale = is_stale(&live, std::slice::from_ref(&edit), &images);
        let commands = rebased(&live, &[edit], &images);
        assert_eq!(stale, commands.is_none(), "the refusal and the run agree");
        match commands?.remove(0) {
            Command::UpdateSchDrawing { drawing } => Some(drawing),
            _ => None,
        }
    }

    /// The review's case: y, typed after the recorded edit, stays.
    #[test]
    fn a_replayed_cell_edit_keeps_a_later_edit_of_another_cell() {
        let now = replayed(
            table(&[&["a", "b"]]),
            table(&[&["x", "b"]]),
            table(&[&["x", "y"]]),
        );
        assert_eq!(now, Some(table(&[&["x", "y"]])));
    }

    /// The fill the recorded edit set was changed again since: the replay
    /// would write over it.
    #[test]
    fn a_field_the_edit_changed_and_that_changed_since_refuses_the_replay() {
        let now = replayed(
            rect(20.0, FillType::None),
            rect(20.0, FillType::Background),
            rect(20.0, FillType::Outline),
        );
        assert!(now.is_none());
    }

    /// `links` is left out of the JSON while empty.
    #[test]
    fn a_list_cleared_since_the_edit_is_not_filled_again() {
        let now = replayed(
            table_linking(&["a.snxsch"]),
            table_linking(&["b.snxsch"]),
            table_linking(&[]),
        );
        assert!(now.is_none());
    }

    #[test]
    fn a_replayed_cell_edit_merges_with_a_change_to_another_cell() {
        let now = replayed(
            table(&[&["a", "b"]]),
            table(&[&["x", "b"]]),
            table(&[&["a", "y"]]),
        );
        assert_eq!(now, Some(table(&[&["x", "y"]])));
    }

    #[test]
    fn a_cell_changed_since_to_another_value_refuses_the_replay() {
        let now = replayed(
            table(&[&["a", "b"]]),
            table(&[&["x", "b"]]),
            table(&[&["z", "b"]]),
        );
        assert!(now.is_none());
    }

    #[test]
    fn a_row_added_since_refuses_the_replay() {
        let now = replayed(
            table(&[&["a", "b"]]),
            table(&[&["x", "b"]]),
            table(&[&["a", "b"], &["c", "d"]]),
        );
        assert!(now.is_none());
    }

    /// `links` is left out of the JSON while empty.
    #[test]
    fn a_list_first_filled_by_the_edit_does_not_replace_a_later_one() {
        let now = replayed(
            table_linking(&[]),
            table_linking(&["a.snxsch"]),
            table_linking(&["b.snxsch"]),
        );
        assert!(now.is_none());
    }

    #[test]
    fn a_list_emptied_by_the_edit_does_not_drop_a_later_one() {
        let now = replayed(
            table_linking(&["a.snxsch"]),
            table_linking(&[]),
            table_linking(&["b.snxsch"]),
        );
        assert!(now.is_none());
    }

    fn pin(name: &str, slot: f64) -> SheetPin {
        serde_json::from_value(serde_json::json!({
            "uuid": Uuid::from_u128(0x60 + slot as u128),
            "name": name,
            "direction": "input",
            "position": { "x": 0.0, "y": slot },
            "auto_generated": true,
        }))
        .expect("a pin")
    }

    fn sheet_with_pins(pins: Vec<SheetPin>) -> SchematicSheet {
        let child: ChildSheet = serde_json::from_value(serde_json::json!({
            "uuid": Uuid::from_u128(0x51),
            "name": "Child",
            "filename": "child.snxsch",
            "position": { "x": 0.0, "y": 0.0 },
            "size": [30.0, 30.0],
        }))
        .expect("a sheet symbol");
        SchematicSheet {
            child_sheets: vec![ChildSheet { pins, ..child }],
            ..Default::default()
        }
    }

    fn sync_of(names: &[&str]) -> Command {
        Command::ReconcileChildSheetPins {
            child_filename: "child.snxsch".into(),
            ports: names
                .iter()
                .map(|name| SheetPort {
                    name: (*name).into(),
                    direction: "input".into(),
                })
                .collect(),
            renames: None,
        }
    }

    /// DATA came after the recorded sync: running it would drop DATA's pin.
    #[test]
    fn a_recorded_sync_that_would_drop_a_newer_pin_is_stale() {
        let sheet = sheet_with_pins(vec![pin("A", 0.0), pin("DATA", 1.0)]);
        assert!(is_stale(&sheet, &[sync_of(&["A"])], &[]));
    }

    #[test]
    fn a_recorded_sync_that_would_add_a_pin_is_stale() {
        let sheet = sheet_with_pins(vec![pin("A", 0.0)]);
        assert!(is_stale(&sheet, &[sync_of(&["A", "B"])], &[]));
    }

    #[test]
    fn a_recorded_sync_that_still_fits_is_not_stale() {
        let sheet = sheet_with_pins(vec![pin("A", 0.0)]);
        assert!(!is_stale(&sheet, &[sync_of(&["A"])], &[]));
    }
}
