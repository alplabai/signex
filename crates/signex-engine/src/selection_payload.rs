//! Copying a selection into a [`PlacementPayload`], and what a payload needs
//! to know about the sheet it is about to land on.
//!
//! The payload carries the library bodies its symbols use, captured at copy
//! time, so a paste into another sheet brings the bodies with it instead of
//! leaving parts that point at a body that sheet does not have.

use signex_types::schematic::{Aabb, Point, SchematicSheet, SelectedItem};

use crate::{Engine, EngineError, PlacementPayload};

/// A copied selection: the payload, anchored at `origin`.
#[derive(Debug, Clone)]
pub struct CopiedSelection {
    /// Items positioned relative to `origin`.
    pub payload: PlacementPayload,
    /// The extent the items had on the source sheet, absolute: what Smart
    /// Paste measures against the target sheet's content.
    pub bounds: Option<Aabb>,
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
            uuid: uuid::Uuid::now_v7(),
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
        CopiedSelection::from_fragment(fragment, grid_mm)
    }
}

impl CopiedSelection {
    /// A copy of `fragment`, whose items sit at their absolute positions on
    /// the sheet they came from. `None` when it holds nothing placeable;
    /// `grid_mm` snaps the anchor as in [`Engine::copy_selection`].
    pub fn from_fragment(
        fragment: SchematicSheet,
        grid_mm: f64,
    ) -> Result<Option<Self>, EngineError> {
        let absolute = PlacementPayload::new(fragment);
        if absolute.coordinates().iter().any(|v| !v.is_finite()) {
            return Err(EngineError::NonFiniteCoordinate);
        }
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
        Ok(Some(Self {
            payload: absolute.translated(-origin.x, -origin.y),
            bounds: Some(bounds),
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
            if !existing.same_body(body) {
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
    use crate::Command;
    use crate::test_support::test_sheet;
    use signex_types::schematic::{Junction, SelectedKind, Symbol, Wire};
    use uuid::Uuid;

    fn sheet_with_part_and_wire() -> (Engine, Vec<SelectedItem>) {
        let mut sheet = test_sheet();
        let lib: signex_types::schematic::LibSymbol =
            serde_json::from_str(r#"{"id":"lib:r"}"#).unwrap();
        sheet.lib_symbols.insert(lib.id.clone(), lib);
        let part = Symbol {
            uuid: Uuid::now_v7(),
            lib_id: "lib:r".to_owned(),
            reference: "R7".to_owned(),
            position: Point::new(25.4, 25.4),
            unit: 1,
            ..Default::default()
        };
        let wire = Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(25.4, 25.4),
            end: Point::new(38.1, 25.4),
            stroke_width: 0.0,
            unknown_columns: Default::default(),
            locked: false,
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

    #[test]
    fn a_copied_partial_t_keeps_its_junction_verbatim_through_turns_and_commit() {
        let mut sheet = test_sheet();
        // A stem ending on a trunk that is NOT copied: the engine would call
        // a minted dot there unjustified and retire it.
        let stem = Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(10.16, 0.0),
            end: Point::new(10.16, 10.16),
            stroke_width: 0.0,
            unknown_columns: Default::default(),
            locked: false,
        };
        let dot = Junction {
            uuid: Uuid::now_v7(),
            position: Point::new(10.16, 0.0),
            minted: true,
            ..Default::default()
        };
        let items = vec![
            SelectedItem::new(stem.uuid, SelectedKind::Wire),
            SelectedItem::new(dot.uuid, SelectedKind::Junction),
        ];
        sheet.wires.push(stem);
        sheet.junctions.push(dot);
        let engine = Engine::new(sheet).unwrap();

        let copied = engine.copy_selection(&items, 1.27).unwrap().unwrap();
        assert_eq!(copied.payload.fragment().junctions.len(), 1);
        for turned in [
            copied.payload.rotate(false).unwrap(),
            copied.payload.mirror(crate::MirrorAxis::Vertical).unwrap(),
        ] {
            let j = &turned.fragment().junctions;
            assert_eq!(j.len(), 1, "no dot added or dropped by a turn");
            assert!(j[0].minted, "its flag is untouched");
        }
        let commands = copied.payload.place_commands(copied.origin).unwrap();
        let placed: Vec<_> = commands
            .iter()
            .filter_map(|c| match c {
                Command::PlaceJunction { junction } => Some(junction),
                _ => None,
            })
            .collect();
        assert_eq!(placed.len(), 1);
        assert!((placed[0].position.x - 10.16).abs() < 1e-9 && placed[0].position.y.abs() < 1e-9);
    }

    #[test]
    fn a_selection_with_a_non_finite_coordinate_is_not_copied() {
        let (mut engine, items) = sheet_with_part_and_wire();
        let mut sheet = engine.document().clone();
        sheet.wires[0].end.x = f64::NAN;
        engine.set_document(sheet);
        assert!(matches!(
            engine.copy_selection(&items, 1.27),
            Err(EngineError::NonFiniteCoordinate)
        ));
    }

    #[test]
    fn a_pasted_lone_symbol_turns_about_the_cursor_not_its_own_position() {
        let mut sheet = test_sheet();
        // The ref text sits far above the part, so the copy's centre (the
        // origin) is not the part's position.
        let part = Symbol {
            uuid: Uuid::now_v7(),
            lib_id: "lib:r".to_owned(),
            reference: "R1".to_owned(),
            position: Point::new(25.4, 25.4),
            ref_text: Some(signex_types::schematic::TextProp {
                position: Point::new(25.4, -12.7),
                rotation: 0.0,
                font_size: 1.27,
                justify_h: Default::default(),
                justify_v: Default::default(),
                hidden: false,
                font_family: None,
            }),
            unit: 1,
            ..Default::default()
        };
        let items = vec![SelectedItem::new(part.uuid, SelectedKind::Symbol)];
        sheet.symbols.push(part);
        let engine = Engine::new(sheet).unwrap();
        let copied = engine.copy_selection(&items, 1.27).unwrap().unwrap();
        let before = copied.payload.lone_symbol().unwrap().position;
        assert!(before.x != 0.0 || before.y != 0.0, "off the origin");

        let turned = copied.payload.rotate(false).unwrap();
        let after = turned.lone_symbol().unwrap().position;
        assert!((after.x, after.y) != (before.x, before.y), "it moved");
        let r = |p: Point| (p.x * p.x + p.y * p.y).sqrt();
        assert!((r(after) - r(before)).abs() < 1e-9, "about the origin");
    }
}
