//! The side a sync writes, entry by entry, as the sync dialog draws and
//! lists it (mockup row 79): each name with its I/O type and what Apply
//! does to it. Read-only: Apply works from `SheetSyncPreview` alone.

use signex_types::schematic::{
    ChildSheet, SchematicSheet, entry_port_name, normalize_port_name, renamed_entry,
};

use crate::sheet::{exposed_sheet_ports, normalize_sheet_pin_direction};
use crate::sheet_sync::SheetSyncPreview;

/// What Apply does to one entry or port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetEntryChange {
    Kept,
    Added,
    Removed,
    /// `name` is the new name; `from` is the name it has now.
    Renamed {
        from: String,
    },
}

/// One entry or port of the written side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetSyncEntry {
    pub name: String,
    /// The normalised direction: `input`, `output`, `bidirectional`,
    /// `tri_state` or `passive`.
    pub direction: &'static str,
    pub change: SheetEntryChange,
}

/// The sheet symbol's entries by name, first of each name.
fn symbol_side(symbol: &ChildSheet) -> Vec<(String, &'static str)> {
    let mut side: Vec<(String, &'static str)> = Vec::new();
    for pin in &symbol.pins {
        if let Some(name) = normalize_port_name(&pin.name)
            && !side.iter().any(|(seen, _)| seen == name)
        {
            side.push((
                name.to_string(),
                normalize_sheet_pin_direction(&pin.direction),
            ));
        }
    }
    side
}

/// The ports the sheet exposes.
fn sheet_side(child: &SchematicSheet) -> Vec<(String, &'static str)> {
    exposed_sheet_ports(child)
        .into_iter()
        .map(|port| {
            let direction = normalize_sheet_pin_direction(&port.direction);
            (port.name, direction)
        })
        .collect()
}

/// `written` with `preview` applied: every current name with its change,
/// then the added names, which take their direction from the `source` name
/// that meets them (`Repeat(VOUT)` meets `VOUT`). A renamed `Repeat(…)`
/// entry is listed as the repeat it becomes.
fn merged(
    written: Vec<(String, &'static str)>,
    source: &[(String, &'static str)],
    preview: &SheetSyncPreview,
) -> Vec<SheetSyncEntry> {
    let mut entries: Vec<SheetSyncEntry> = written
        .into_iter()
        .map(|(name, direction)| {
            let rename = preview.renamed.iter().find(|r| r.from == name);
            match rename {
                Some(rename) => SheetSyncEntry {
                    name: renamed_entry(&name, &rename.to),
                    direction,
                    change: SheetEntryChange::Renamed { from: name },
                },
                None => {
                    let change = if preview.removed.contains(&name) {
                        SheetEntryChange::Removed
                    } else {
                        SheetEntryChange::Kept
                    };
                    SheetSyncEntry {
                        name,
                        direction,
                        change,
                    }
                }
            }
        })
        .collect();
    entries.extend(preview.added.iter().map(|name| {
        SheetSyncEntry {
            name: name.clone(),
            direction: source
                .iter()
                .find(|(seen, _)| entry_port_name(seen) == Some(name.as_str()))
                .map_or("bidirectional", |(_, direction)| *direction),
            change: SheetEntryChange::Added,
        }
    }));
    entries
}

/// The sheet symbol's entries under Sheet to Symbol's `preview`.
pub fn symbol_entries_after_sync(
    symbol: &ChildSheet,
    child: &SchematicSheet,
    preview: &SheetSyncPreview,
) -> Vec<SheetSyncEntry> {
    merged(symbol_side(symbol), &sheet_side(child), preview)
}

/// The sheet's ports under Symbol to Sheet's `preview`.
pub fn sheet_ports_after_sync(
    symbol: &ChildSheet,
    child: &SchematicSheet,
    preview: &SheetSyncPreview,
) -> Vec<SheetSyncEntry> {
    merged(sheet_side(child), &symbol_side(symbol), preview)
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::{Label, LabelType, Point, SheetPin};
    use uuid::Uuid;

    use super::*;
    use crate::{entry_changes_from_sheet, port_changes_from_symbol};

    fn pin(name: &str, direction: &str, auto_generated: bool) -> SheetPin {
        SheetPin {
            uuid: Uuid::now_v7(),
            name: name.to_string(),
            direction: direction.to_string(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            auto_generated,
            user_moved: false,
        }
    }

    fn symbol(pins: Vec<SheetPin>) -> ChildSheet {
        ChildSheet {
            uuid: Uuid::now_v7(),
            name: "U_Power".to_string(),
            filename: "power.snxsch".to_string(),
            position: Point::new(0.0, 0.0),
            size: (30.0, 30.0),
            stroke_width: 0.0,
            fill: signex_types::schematic::FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins,
            instances: Vec::new(),
        }
    }

    fn sheet(ports: &[(&str, &str)]) -> SchematicSheet {
        SchematicSheet {
            labels: ports
                .iter()
                .enumerate()
                .map(|(row, (name, shape))| Label {
                    shape: (*shape).to_string(),
                    ..Label::new(
                        *name,
                        Point::new(10.0, 10.0 * row as f64),
                        LabelType::Hierarchical,
                    )
                })
                .collect(),
            ..SchematicSheet::default()
        }
    }

    fn change_of<'e>(entries: &'e [SheetSyncEntry], name: &str) -> &'e SheetSyncEntry {
        entries
            .iter()
            .find(|entry| entry.name == name)
            .expect("listed")
    }

    /// Mockup row 79: each change carries its I/O type, an added entry the
    /// direction of the port it comes from.
    #[test]
    fn sheet_to_symbol_lists_each_entry_with_its_type() {
        let s = symbol(vec![
            pin("GND", "passive", true),
            pin("VIN", "input", true),
            pin("VSENSE", "output", true),
        ]);
        let child = sheet(&[
            ("GND", "passive"),
            ("VIN_12V", "input"),
            ("PGOOD", "output"),
        ]);
        let ports = exposed_sheet_ports(&child);
        let preview = entry_changes_from_sheet(&s, &ports);
        let entries = symbol_entries_after_sync(&s, &child, &preview);

        assert_eq!(change_of(&entries, "GND").change, SheetEntryChange::Kept);
        let renamed = change_of(&entries, "VIN_12V");
        assert_eq!(renamed.direction, "input");
        assert_eq!(
            renamed.change,
            SheetEntryChange::Renamed {
                from: "VIN".to_string()
            }
        );
        assert_eq!(
            entries.len(),
            3,
            "the lone output pair is a rename: {entries:?}"
        );
        let output = change_of(&entries, "PGOOD");
        assert_eq!(output.direction, "output");
    }

    #[test]
    fn symbol_to_sheet_lists_the_ports_with_an_added_ones_entry_type() {
        let s = symbol(vec![
            pin("KEEP", "input", false),
            pin("NEW", "output", false),
        ]);
        let child = sheet(&[("KEEP", "input"), ("GONE", "passive")]);
        let preview = port_changes_from_symbol(&s, &child);
        let entries = sheet_ports_after_sync(&s, &child, &preview);

        assert_eq!(change_of(&entries, "KEEP").change, SheetEntryChange::Kept);
        let added = change_of(&entries, "NEW");
        assert_eq!(
            (added.direction, &added.change),
            ("output", &SheetEntryChange::Added)
        );
        let removed = change_of(&entries, "GONE");
        assert_eq!(
            (removed.direction, &removed.change),
            ("passive", &SheetEntryChange::Removed)
        );
    }
}
