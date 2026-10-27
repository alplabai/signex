//! Content attached to the cursor before it is placed: a part picked from
//! the library today, pasted content next.
//!
//! A [`PlacementPayload`] is a small fragment sheet: the items plus the
//! `lib_symbols` they use, anchored so that its origin `(0, 0)` is the
//! point the cursor holds. Turning it runs the engine's own selection
//! commands on a scratch engine, so the rotate and mirror rules (pin
//! geometry, field autoplacement, mirror pairing) are the ones a placed
//! selection obeys, not a copy of them. Committing it mints fresh uuids and
//! returns ordinary `Place*` commands for the caller to apply as ONE batch,
//! so a click is one undo step.

use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

use crate::{Command, Engine, EngineError, MirrorAxis, SelectionTransform};

/// A fragment waiting to be placed. Turning returns a new payload.
#[derive(Debug, Clone)]
pub struct PlacementPayload {
    fragment: SchematicSheet,
}

impl PlacementPayload {
    /// Wrap `fragment`, whose items are positioned relative to the cursor.
    pub fn new(fragment: SchematicSheet) -> Self {
        Self { fragment }
    }

    /// The fragment, for drawing the ghost.
    pub fn fragment(&self) -> &SchematicSheet {
        &self.fragment
    }

    /// The lone symbol of a part payload (`None` for a mixed fragment).
    pub fn lone_symbol(&self) -> Option<&signex_types::schematic::Symbol> {
        match self.fragment.symbols.as_slice() {
            [symbol] if self.items().len() == 1 => Some(symbol),
            _ => None,
        }
    }

    /// Whether the fragment holds nothing placeable.
    pub fn is_empty(&self) -> bool {
        self.items().is_empty()
    }

    /// The payload turned 90 degrees on screen.
    pub fn rotate(&self, clockwise: bool) -> Result<Self, EngineError> {
        if self.lone_symbol().is_some() {
            // A lone part turns about its own origin, which stays the cursor.
            let angle_degrees = if clockwise { -90.0 } else { 90.0 };
            return self.run(|_, items| Command::RotateSelection {
                items,
                angle_degrees,
                keep_wires: false,
            });
        }
        let transform = if clockwise {
            SelectionTransform::RotateClockwise
        } else {
            SelectionTransform::RotateCounterclockwise
        };
        self.transformed(transform)
    }

    /// The payload mirrored across `axis` (same pairing as the selection
    /// mirror: `Vertical` flips up/down).
    pub fn mirror(&self, axis: MirrorAxis) -> Result<Self, EngineError> {
        if self.lone_symbol().is_some() {
            return self.run(|_, items| Command::MirrorSelection {
                items,
                axis,
                keep_wires: false,
            });
        }
        self.transformed(SelectionTransform::Mirror(axis))
    }

    /// The commands that place the payload with its origin at `at`:
    /// `RegisterLibSymbol` for each body, then one `Place*` per item, every
    /// item under a fresh uuid. Apply them as one batch.
    pub fn place_commands(&self, at: Point) -> Result<Vec<Command>, EngineError> {
        let moved = self.run(|_, items| Command::MoveSelection {
            items,
            dx: at.x,
            dy: at.y,
            keep_wires: false,
        })?;
        Ok(moved.fresh_place_commands())
    }

    fn items(&self) -> Vec<SelectedItem> {
        let f = &self.fragment;
        let uuids = |kind, ids: Vec<Uuid>| {
            ids.into_iter()
                .map(move |id| SelectedItem::new(id, kind))
                .collect::<Vec<_>>()
        };
        [
            uuids(
                SelectedKind::Symbol,
                f.symbols.iter().map(|s| s.uuid).collect(),
            ),
            uuids(SelectedKind::Wire, f.wires.iter().map(|s| s.uuid).collect()),
            uuids(SelectedKind::Bus, f.buses.iter().map(|s| s.uuid).collect()),
            uuids(
                SelectedKind::BusEntry,
                f.bus_entries.iter().map(|s| s.uuid).collect(),
            ),
            uuids(
                SelectedKind::Junction,
                f.junctions.iter().map(|s| s.uuid).collect(),
            ),
            uuids(
                SelectedKind::NoConnect,
                f.no_connects.iter().map(|s| s.uuid).collect(),
            ),
            uuids(
                SelectedKind::Label,
                f.labels.iter().map(|s| s.uuid).collect(),
            ),
            uuids(
                SelectedKind::TextNote,
                f.text_notes.iter().map(|s| s.uuid).collect(),
            ),
        ]
        .concat()
    }

    /// Turn the whole fragment about its centre.
    fn transformed(&self, transform: SelectionTransform) -> Result<Self, EngineError> {
        let items = self.items();
        let centre = Engine::new(self.fragment.clone())?
            .selection_centre(&items)
            .unwrap_or_else(|| Point::new(0.0, 0.0));
        self.run(|_, items| Command::TransformSelection {
            items,
            centre,
            transform,
            keep_wires: false,
        })
    }

    /// Run the command `build` makes over every item on a scratch engine and
    /// return the resulting fragment.
    fn run(
        &self,
        build: impl FnOnce(&SchematicSheet, Vec<SelectedItem>) -> Command,
    ) -> Result<Self, EngineError> {
        let mut scratch = Engine::new(self.fragment.clone())?;
        let command = build(&self.fragment, self.items());
        scratch.execute(command)?;
        Ok(Self::new(scratch.document().clone()))
    }

    fn fresh_place_commands(&self) -> Vec<Command> {
        let f = &self.fragment;
        let mut commands: Vec<Command> = f
            .lib_symbols
            .values()
            .cloned()
            .map(|lib_symbol| Command::RegisterLibSymbol { lib_symbol })
            .collect();
        commands.extend(f.symbols.iter().cloned().map(|mut symbol| {
            symbol.uuid = Uuid::new_v4();
            Command::PlaceSymbol { symbol }
        }));
        commands.extend(f.wires.iter().cloned().map(|mut wire| {
            wire.uuid = Uuid::new_v4();
            Command::PlaceWireSegment { wire }
        }));
        commands.extend(f.buses.iter().cloned().map(|mut bus| {
            bus.uuid = Uuid::new_v4();
            Command::PlaceBus { bus }
        }));
        commands.extend(f.bus_entries.iter().cloned().map(|mut bus_entry| {
            bus_entry.uuid = Uuid::new_v4();
            Command::PlaceBusEntry { bus_entry }
        }));
        commands.extend(f.junctions.iter().cloned().map(|mut junction| {
            junction.uuid = Uuid::new_v4();
            Command::PlaceJunction { junction }
        }));
        commands.extend(f.no_connects.iter().cloned().map(|mut no_connect| {
            no_connect.uuid = Uuid::new_v4();
            Command::PlaceNoConnect { no_connect }
        }));
        commands.extend(f.labels.iter().cloned().map(|mut label| {
            label.uuid = Uuid::new_v4();
            Command::PlaceLabel { label }
        }));
        commands.extend(f.text_notes.iter().cloned().map(|mut text_note| {
            text_note.uuid = Uuid::new_v4();
            Command::PlaceTextNote { text_note }
        }));
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_sheet;
    use signex_types::schematic::{LibSymbol, Symbol};

    fn part_payload() -> PlacementPayload {
        let mut sheet = test_sheet();
        let lib: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).unwrap();
        sheet.lib_symbols.insert(lib.id.clone(), lib);
        sheet.symbols.push(Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "lib:r".to_owned(),
            reference: "R1".to_owned(),
            position: Point::new(0.0, 0.0),
            unit: 1,
            ..Default::default()
        });
        PlacementPayload::new(sheet)
    }

    #[test]
    fn place_commands_register_the_body_then_place_at_the_cursor() {
        let payload = part_payload();
        let commands = payload.place_commands(Point::new(10.16, 5.08)).unwrap();
        assert!(matches!(commands[0], Command::RegisterLibSymbol { .. }));
        let Command::PlaceSymbol { symbol } = &commands[1] else {
            panic!("expected PlaceSymbol, got {:?}", commands[1]);
        };
        assert_eq!((symbol.position.x, symbol.position.y), (10.16, 5.08));
        assert_ne!(symbol.uuid, payload.fragment().symbols[0].uuid);
    }

    #[test]
    fn every_commit_mints_a_new_uuid() {
        let payload = part_payload();
        let id = |cmds: Vec<Command>| match &cmds[1] {
            Command::PlaceSymbol { symbol } => symbol.uuid,
            other => panic!("expected PlaceSymbol, got {other:?}"),
        };
        let a = id(payload.place_commands(Point::new(0.0, 0.0)).unwrap());
        let b = id(payload.place_commands(Point::new(0.0, 0.0)).unwrap());
        assert_ne!(a, b);
    }

    #[test]
    fn rotate_and_mirror_turn_the_part_in_place() {
        let payload = part_payload();
        let turned = payload.rotate(false).unwrap();
        let symbol = turned.lone_symbol().unwrap();
        assert_eq!(symbol.rotation, 90.0);
        assert_eq!((symbol.position.x, symbol.position.y), (0.0, 0.0));
        let clockwise = payload.rotate(true).unwrap();
        assert_eq!(clockwise.lone_symbol().unwrap().rotation, 270.0);

        let mirrored = payload.mirror(MirrorAxis::Vertical).unwrap();
        let symbol = mirrored.lone_symbol().unwrap();
        assert!(symbol.mirror_x && !symbol.mirror_y);
        let back = mirrored.mirror(MirrorAxis::Vertical).unwrap();
        assert!(!back.lone_symbol().unwrap().mirror_x);
    }

    #[test]
    fn turning_leaves_the_original_payload_unchanged() {
        let payload = part_payload();
        let _ = payload.rotate(false).unwrap();
        assert_eq!(payload.lone_symbol().unwrap().rotation, 0.0);
    }

    #[test]
    fn an_empty_fragment_places_nothing() {
        let payload = PlacementPayload::new(test_sheet());
        assert!(payload.is_empty());
    }
}
