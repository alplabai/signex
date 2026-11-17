//! `Command::ResizeChildSheet` (#2418): a sheet symbol's width and height,
//! as the Properties Size row or the `set_sheet_symbol_size` command sets it.
//!
//! The top-left corner stays. Entries on the right edge travel with it, and
//! entries on the bottom edge travel with the bottom; an entry never slides
//! along its edge. The resize is refused, with nothing changed, when an edge
//! falls under [`MIN_CHILD_SHEET_EDGE_MM`], when an entry would end up past
//! the end of its shortened edge, or when an entry that moves is connected
//! or would land on a connection (its wire would be left behind).

use signex_net::{SheetPinContacts, pt_key};
use signex_types::schematic::{ChildSheet, Point};

use crate::*;

/// Two coordinates count as one when they fall in the same 1 µm bucket.
fn same(a: f64, b: f64) -> bool {
    pt_key(&Point::new(a, 0.0)).0 == pt_key(&Point::new(b, 0.0)).0
}

/// `child` at `size`, its entries moved with the right and bottom edges, or
/// the refusal. `contacts` are what an entry on the sheet connects to.
pub(crate) fn resized_child_sheet(
    child: &ChildSheet,
    size: (f64, f64),
    contacts: &SheetPinContacts,
) -> Result<ChildSheet, EngineError> {
    let (width, height) = size;
    let edge_ok = |e: f64| e.is_finite() && e >= MIN_CHILD_SHEET_EDGE_MM;
    if !edge_ok(width) || !edge_ok(height) {
        return Err(EngineError::InvalidChildSheetSize {
            width,
            height,
            min_mm: MIN_CHILD_SHEET_EDGE_MM,
        });
    }
    let (x0, y0) = (child.position.x, child.position.y);
    let (right, bottom) = (x0 + child.size.0, y0 + child.size.1);
    let (new_right, new_bottom) = (x0 + width, y0 + height);
    let mut resized = child.clone();
    resized.size = size;
    let mut off_border = Vec::new();
    let mut connected = Vec::new();
    for pin in &mut resized.pins {
        let old = pin.position;
        let on_vertical_edge = same(old.x, x0) || same(old.x, right);
        if same(old.x, right) {
            pin.position.x = new_right;
        } else if same(old.y, bottom) {
            pin.position.y = new_bottom;
        }
        let past_end = if on_vertical_edge {
            pin.position.y > new_bottom && !same(pin.position.y, new_bottom)
        } else {
            pin.position.x > new_right && !same(pin.position.x, new_right)
        };
        if past_end {
            off_border.push(pin.name.clone());
            continue;
        }
        let moved = !same(old.x, pin.position.x) || !same(old.y, pin.position.y);
        if moved
            && (contacts.joins_pin_at(old.x, old.y)
                || contacts.hits(pin.position.x, pin.position.y))
        {
            connected.push(pin.name.clone());
        }
    }
    let sheet = child.name.clone();
    if !off_border.is_empty() {
        return Err(EngineError::ChildSheetEntriesOffBorder {
            sheet,
            entries: off_border,
        });
    }
    if !connected.is_empty() {
        return Err(EngineError::ChildSheetEntriesConnected {
            sheet,
            entries: connected,
        });
    }
    Ok(resized)
}

impl Engine {
    pub(crate) fn exec_resize_child_sheet(
        &mut self,
        before: SchematicSheet,
        cmd: Command,
    ) -> Result<CommandResult, EngineError> {
        let (sheet_id, size) = match cmd {
            Command::ResizeChildSheet { sheet_id, size } => (sheet_id, size),
            other => return Err(EngineError::UnsupportedCommand(other.kind())),
        };
        let Some(index) = (self.document.child_sheets.iter()).position(|cs| cs.uuid == sheet_id)
        else {
            return Ok(CommandResult::unchanged());
        };
        let child = &self.document.child_sheets[index];
        if same(child.size.0, size.0) && same(child.size.1, size.1) {
            return Ok(CommandResult::unchanged());
        }
        let contacts = SheetPinContacts::of_sheet(&self.document);
        let resized = resized_child_sheet(child, size, &contacts)?;
        self.document.child_sheets[index] = resized;
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SelectionMoved,
            document: DocumentPatch::CHILD_SHEETS,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{FillType, SheetPin, Wire};

    fn pin(name: &str, x: f64, y: f64) -> SheetPin {
        SheetPin {
            uuid: uuid::Uuid::now_v7(),
            name: name.into(),
            direction: "input".into(),
            position: Point::new(x, y),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }
    }

    /// A 20 x 20 symbol at (10, 10): `IN` on the left, `OUT` on the right,
    /// `B` on the bottom.
    fn child() -> ChildSheet {
        ChildSheet {
            uuid: uuid::Uuid::now_v7(),
            name: "Power".into(),
            filename: "power.snxsch".into(),
            position: Point::new(10.0, 10.0),
            size: (20.0, 20.0),
            stroke_width: 0.15,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![
                pin("IN", 10.0, 15.0),
                pin("OUT", 30.0, 15.0),
                pin("B", 20.0, 30.0),
            ],
            instances: Vec::new(),
        }
    }

    fn engine_with(child: ChildSheet, wires: Vec<Wire>) -> Engine {
        Engine::new(SchematicSheet {
            child_sheets: vec![child],
            wires,
            ..Default::default()
        })
        .expect("engine")
    }

    #[test]
    fn the_right_and_bottom_entries_follow_their_edges_in_one_undo_step() {
        let sheet = child();
        let id = sheet.uuid;
        let mut engine = engine_with(sheet, Vec::new());
        engine
            .execute(Command::ResizeChildSheet {
                sheet_id: id,
                size: (30.0, 25.0),
            })
            .expect("resized");
        let resized = &engine.document().child_sheets[0];
        assert_eq!(resized.size, (30.0, 25.0));
        assert_eq!(resized.position, Point::new(10.0, 10.0), "top-left stays");
        let at = |name: &str| {
            resized
                .pins
                .iter()
                .find(|p| p.name == name)
                .map(|p| p.position)
        };
        assert_eq!(at("IN"), Some(Point::new(10.0, 15.0)));
        assert_eq!(at("OUT"), Some(Point::new(40.0, 15.0)));
        assert_eq!(at("B"), Some(Point::new(20.0, 35.0)));
        engine.undo().expect("undo");
        assert_eq!(engine.document().child_sheets[0].size, (20.0, 20.0));
    }

    #[test]
    fn a_size_under_the_minimum_or_an_entry_past_its_edge_is_refused() {
        let sheet = child();
        let id = sheet.uuid;
        let mut engine = engine_with(sheet.clone(), Vec::new());
        for size in [(1.0, 20.0), (f64::NAN, 20.0), (20.0, f64::INFINITY)] {
            let refused = engine.execute(Command::ResizeChildSheet { sheet_id: id, size });
            assert!(
                matches!(refused, Err(EngineError::InvalidChildSheetSize { .. })),
                "{size:?}"
            );
        }
        // An 8 mm wide symbol ends at x = 18, so B (bottom, x = 20) falls off.
        let refused = engine.execute(Command::ResizeChildSheet {
            sheet_id: id,
            size: (8.0, 20.0),
        });
        assert!(
            matches!(refused, Err(EngineError::ChildSheetEntriesOffBorder { ref entries, .. }) if entries == &["B".to_string()]),
            "{refused:?}"
        );
        assert_eq!(engine.document().child_sheets[0], sheet, "nothing moved");
    }

    #[test]
    fn a_connected_entry_that_would_move_refuses_the_resize() {
        let sheet = child();
        let id = sheet.uuid;
        let wire = Wire {
            color: None,
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(30.0, 15.0),
            end: Point::new(40.0, 15.0),
            stroke_width: 0.0,
            unknown_columns: Default::default(),
            locked: false,
        };
        let mut engine = engine_with(sheet.clone(), vec![wire]);
        let refused = engine.execute(Command::ResizeChildSheet {
            sheet_id: id,
            size: (25.0, 20.0),
        });
        assert!(
            matches!(refused, Err(EngineError::ChildSheetEntriesConnected { ref entries, .. }) if entries == &["OUT".to_string()]),
            "{refused:?}"
        );
        assert_eq!(engine.document().child_sheets[0], sheet);
        // Only the height changes: OUT stays, the wire keeps its entry.
        engine
            .execute(Command::ResizeChildSheet {
                sheet_id: id,
                size: (20.0, 25.0),
            })
            .expect("only the bottom entry moves");
    }
}
