//! Copying a selection into a [`PlacementPayload`], and what a payload needs
//! to know about the sheet it is about to land on.
//!
//! The payload carries the library bodies its symbols use, captured at copy
//! time, so a paste into another sheet brings the bodies with it instead of
//! leaving parts that point at a body that sheet does not have.

use signex_types::schematic::{Aabb, Point, SchematicSheet, SelectedItem};

use crate::{Command, Engine, EngineError, PlacementPayload};

/// A copied selection: the payload, anchored at `origin`.
#[derive(Debug, Clone)]
pub struct CopiedSelection {
    /// Items positioned relative to `origin`.
    pub payload: PlacementPayload,
    /// Where the payload origin sat on the source sheet (the grid-snapped
    /// centre of the copied items). Placing the payload at `origin` puts the
    /// copy exactly over the original.
    pub origin: Point,
}

impl Engine {
    /// Copy the carriable part of `items`, bodies included. `None` when there
    /// is nothing to carry. `grid_mm` snaps the anchor so an on-grid copy
    /// stays on the grid wherever it is placed.
    pub fn copy_selection(
        &self,
        items: &[SelectedItem],
        grid_mm: f64,
    ) -> Result<Option<CopiedSelection>, EngineError> {
        let clip = self.collect_selection_clipboard(items);
        let mut fragment = SchematicSheet {
            uuid: uuid::Uuid::new_v4(),
            symbols: clip.symbols,
            wires: clip.wires,
            buses: clip.buses,
            labels: clip.labels,
            junctions: clip.junctions,
            no_connects: clip.no_connects,
            text_notes: clip.text_notes,
            ..Default::default()
        };
        for symbol in &fragment.symbols {
            if let Some(body) = self.document().lib_symbols.get(&symbol.lib_id) {
                fragment
                    .lib_symbols
                    .insert(symbol.lib_id.clone(), body.clone());
            }
        }
        let absolute = PlacementPayload::new(fragment);
        let Some(bounds) = absolute.bounds() else {
            return Ok(None);
        };
        let centre = Point::new(
            (bounds.min_x + bounds.max_x) / 2.0,
            (bounds.min_y + bounds.max_y) / 2.0,
        );
        let origin = if grid_mm.is_finite() && grid_mm > 0.0 {
            Point::new(
                (centre.x / grid_mm).round() * grid_mm,
                (centre.y / grid_mm).round() * grid_mm,
            )
        } else {
            centre
        };
        let mut scratch = Engine::new(absolute.fragment.clone())?;
        scratch.execute(Command::MoveSelection {
            items: absolute.items(),
            dx: -origin.x,
            dy: -origin.y,
            keep_wires: false,
        })?;
        Ok(Some(CopiedSelection {
            payload: PlacementPayload::new(scratch.document().clone()),
            origin,
        }))
    }
}

impl PlacementPayload {
    /// How many placeable items the fragment holds.
    pub fn item_count(&self) -> usize {
        self.items().len()
    }

    /// The extent of the fragment (symbols padded to their body, text and
    /// markers to their glyphs), in fragment coordinates.
    pub fn bounds(&self) -> Option<Aabb> {
        let f = &self.fragment;
        let mut bounds: Option<Aabb> = None;
        let mut add = |next: Aabb| {
            bounds = Some(bounds.map_or(next, |b| b.union(&next)));
        };
        let dot = |p: &Point, pad: f64| Aabb::new(p.x, p.y, p.x, p.y).expand(pad);
        for w in &f.wires {
            add(Aabb::new(w.start.x, w.start.y, w.end.x, w.end.y));
        }
        for b in &f.buses {
            add(Aabb::new(b.start.x, b.start.y, b.end.x, b.end.y));
        }
        for l in &f.labels {
            add(dot(&l.position, 1.27));
        }
        for s in &f.symbols {
            add(Aabb::new(
                s.position.x - 5.08,
                s.position.y - 5.08,
                s.position.x + 5.08,
                s.position.y + 5.08,
            ));
            for text in [&s.ref_text, &s.val_text].into_iter().flatten() {
                add(dot(&text.position, 1.27));
            }
        }
        for j in &f.junctions {
            add(dot(&j.position, 0.8));
        }
        for nc in &f.no_connects {
            add(dot(&nc.position, 1.27));
        }
        for tn in &f.text_notes {
            add(dot(&tn.position, 1.27));
        }
        bounds
    }

    /// The payload with every body the `sheet` already holds under the same
    /// id replaced by the sheet's copy, since `RegisterLibSymbol` keeps the
    /// sheet's and the placed parts bind to it. Also returns the ids whose
    /// body differed, for the caller to warn about.
    pub fn adopt_sheet_bodies(&self, sheet: &SchematicSheet) -> (Self, Vec<String>) {
        let mut fragment = self.fragment.clone();
        let mut differing = Vec::new();
        for (id, body) in fragment.lib_symbols.iter_mut() {
            let Some(existing) = sheet.lib_symbols.get(id) else {
                continue;
            };
            let same = format!("{existing:?}") == format!("{body:?}");
            if !same {
                differing.push(id.clone());
                *body = existing.clone();
            }
        }
        differing.sort();
        (Self::new(fragment), differing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_sheet;
    use signex_types::schematic::{SelectedKind, Symbol, Wire};
    use uuid::Uuid;

    fn sheet_with_part_and_wire() -> (Engine, Vec<SelectedItem>) {
        let mut sheet = test_sheet();
        let lib: signex_types::schematic::LibSymbol =
            serde_json::from_str(r#"{"id":"lib:r"}"#).unwrap();
        sheet.lib_symbols.insert(lib.id.clone(), lib);
        let part = Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "lib:r".to_owned(),
            reference: "R7".to_owned(),
            position: Point::new(25.4, 25.4),
            unit: 1,
            ..Default::default()
        };
        let wire = Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(25.4, 25.4),
            end: Point::new(38.1, 25.4),
            stroke_width: 0.0,
            unknown_columns: Default::default(),
        };
        let items = vec![
            SelectedItem::new(part.uuid, SelectedKind::Symbol),
            SelectedItem::new(wire.uuid, SelectedKind::Wire),
        ];
        sheet.symbols.push(part);
        sheet.wires.push(wire);
        (Engine::new(sheet).unwrap(), items)
    }

    #[test]
    fn a_copy_carries_the_body_and_is_anchored_on_the_grid() {
        let (engine, items) = sheet_with_part_and_wire();
        let copied = engine.copy_selection(&items, 1.27).unwrap().unwrap();
        assert!(copied.payload.fragment().lib_symbols.contains_key("lib:r"));
        assert_eq!(copied.payload.item_count(), 2);
        let k = copied.origin.x / 1.27;
        assert!((k - k.round()).abs() < 1e-9, "origin on the grid");
        // Placing at the origin lands the copy over the original.
        let commands = copied.payload.place_commands(copied.origin).unwrap();
        let moved = commands
            .iter()
            .find_map(|c| match c {
                Command::PlaceSymbol { symbol } => Some(symbol.position),
                _ => None,
            })
            .unwrap();
        assert!((moved.x - 25.4).abs() < 1e-9 && (moved.y - 25.4).abs() < 1e-9);
    }

    #[test]
    fn nothing_selected_copies_nothing() {
        let (engine, _) = sheet_with_part_and_wire();
        assert!(engine.copy_selection(&[], 1.27).unwrap().is_none());
    }

    #[test]
    fn a_turned_group_keeps_its_points_on_the_grid() {
        let (engine, items) = sheet_with_part_and_wire();
        let copied = engine.copy_selection(&items, 1.27).unwrap().unwrap();
        let turned = copied.payload.rotate(false).unwrap();
        for w in &turned.fragment().wires {
            for p in [w.start, w.end] {
                for v in [p.x, p.y] {
                    let k = v / 1.27;
                    assert!((k - k.round()).abs() < 1e-6, "{v} left the grid");
                }
            }
        }
    }

    #[test]
    fn adopting_the_sheet_body_reports_a_difference_and_keeps_the_sheets_copy() {
        let (engine, items) = sheet_with_part_and_wire();
        let copied = engine.copy_selection(&items, 1.27).unwrap().unwrap();
        let mut other = test_sheet();
        let mut body = copied.payload.fragment().lib_symbols["lib:r"].clone();
        body.description = "different".to_owned();
        other.lib_symbols.insert(body.id.clone(), body);

        let (adopted, differing) = copied.payload.adopt_sheet_bodies(&other);
        assert_eq!(differing, ["lib:r"]);
        assert_eq!(
            adopted.fragment().lib_symbols["lib:r"].description,
            "different"
        );
        let (_, none) = copied.payload.adopt_sheet_bodies(&test_sheet());
        assert!(none.is_empty());
    }
}
