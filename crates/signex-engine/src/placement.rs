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
    pub(crate) fragment: SchematicSheet,
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

    /// The lone symbol when it sits exactly on the origin (a library part on
    /// the cursor). A pasted lone symbol need not: it turns about the origin
    /// like any group, so it stays where the cursor puts the copy.
    fn lone_symbol_at_origin(&self) -> Option<&signex_types::schematic::Symbol> {
        self.lone_symbol()
            .filter(|s| s.position.x == 0.0 && s.position.y == 0.0)
    }

    /// Whether the fragment holds nothing placeable.
    pub fn is_empty(&self) -> bool {
        self.items().is_empty()
    }

    /// The payload turned 90 degrees on screen.
    pub fn rotate(&self, clockwise: bool) -> Result<Self, EngineError> {
        if self.lone_symbol_at_origin().is_some() {
            // A part on the cursor sits at the origin: it turns in place and
            // its fields are re-placed around it.
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
        if self.lone_symbol_at_origin().is_some() {
            return self.run(|_, items| Command::MirrorSelection {
                items,
                axis,
                keep_wires: false,
            });
        }
        self.transformed(SelectionTransform::Mirror(axis))
    }

    /// The payload shifted by `(dx, dy)`: a plain translate of every point,
    /// with no engine command behind it, so no junction is added, dropped or
    /// retired. What was copied is what lands.
    #[must_use]
    pub fn translated(&self, dx: f64, dy: f64) -> Self {
        let mut f = self.fragment.clone();
        let shift = |p: &mut Point| {
            p.x += dx;
            p.y += dy;
        };
        for s in &mut f.symbols {
            shift(&mut s.position);
            for text in [&mut s.ref_text, &mut s.val_text].into_iter().flatten() {
                shift(&mut text.position);
            }
        }
        for w in &mut f.wires {
            shift(&mut w.start);
            shift(&mut w.end);
        }
        for b in &mut f.buses {
            shift(&mut b.start);
            shift(&mut b.end);
        }
        f.bus_entries
            .iter_mut()
            .for_each(|e| shift(&mut e.position));
        f.junctions.iter_mut().for_each(|j| shift(&mut j.position));
        f.no_connects
            .iter_mut()
            .for_each(|n| shift(&mut n.position));
        f.labels.iter_mut().for_each(|l| shift(&mut l.position));
        f.text_notes.iter_mut().for_each(|t| shift(&mut t.position));
        Self::new(f)
    }

    /// The payload with its stored symbol rotation set to `degrees`
    /// (the Properties "Rotation" value). Reference and value fields follow
    /// like any other turn. A payload that is not a lone part is returned
    /// unchanged.
    pub fn with_rotation(&self, degrees: f64) -> Result<Self, EngineError> {
        let Some(symbol) = self.lone_symbol() else {
            return Ok(self.clone());
        };
        let delta = degrees - symbol.rotation;
        // The engine's delta is counter-clockwise on screen, so with one
        // mirror axis set it moves the stored angle the other way.
        let angle_degrees = if symbol.mirror_x != symbol.mirror_y {
            -delta
        } else {
            delta
        };
        if delta.abs() < f64::EPSILON {
            return Ok(self.clone());
        }
        self.run(|_, items| Command::RotateSelection {
            items,
            angle_degrees,
            keep_wires: false,
        })
    }

    /// A copy with `edit` applied to every symbol (designator or value
    /// changes between placements).
    pub fn map_symbols(&self, edit: impl Fn(&mut signex_types::schematic::Symbol)) -> Self {
        let mut fragment = self.fragment.clone();
        fragment.symbols.iter_mut().for_each(edit);
        Self::new(fragment)
    }

    /// The commands that place the payload with its origin at `at`:
    /// `RegisterLibSymbol` for each body, then one `Place*` per item, every
    /// item under a fresh uuid. Apply them as one batch.
    pub fn place_commands(&self, at: Point) -> Result<Vec<Command>, EngineError> {
        Ok(self.translated(at.x, at.y).fresh_place_commands())
    }

    /// Every coordinate of every item, for the finiteness check.
    pub(crate) fn coordinates(&self) -> Vec<f64> {
        let f = &self.fragment;
        let mut out = Vec::new();
        let mut add = |p: &Point| out.extend([p.x, p.y]);
        for s in &f.symbols {
            add(&s.position);
            for text in [&s.ref_text, &s.val_text].into_iter().flatten() {
                add(&text.position);
            }
        }
        for w in &f.wires {
            add(&w.start);
            add(&w.end);
        }
        for b in &f.buses {
            add(&b.start);
            add(&b.end);
        }
        f.bus_entries.iter().for_each(|e| add(&e.position));
        f.junctions.iter().for_each(|j| add(&j.position));
        f.no_connects.iter().for_each(|n| add(&n.position));
        f.labels.iter().for_each(|l| add(&l.position));
        f.text_notes.iter().for_each(|t| add(&t.position));
        out
    }

    pub(crate) fn items(&self) -> Vec<SelectedItem> {
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

    /// Turn the whole fragment about the cursor point, the fragment origin.
    /// The origin sits on the grid and the fragment was anchored to it, so an
    /// on-grid selection stays on the grid through any number of turns.
    fn transformed(&self, transform: SelectionTransform) -> Result<Self, EngineError> {
        self.run(|_, items| Command::TransformSelection {
            items,
            centre: Point::new(0.0, 0.0),
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
        // The engine tidies junction dots after a turn (mints a missing one,
        // retires a minted one that no longer sits on a meeting). A payload
        // is what was copied, so its dots are shielded: all of them count as
        // user-placed while the command runs, and the result keeps exactly
        // the original dots, turned, with their original flags.
        let minted: std::collections::HashMap<Uuid, bool> = self
            .fragment
            .junctions
            .iter()
            .map(|j| (j.uuid, j.minted))
            .collect();
        let mut shielded = self.fragment.clone();
        shielded.junctions.iter_mut().for_each(|j| j.minted = false);
        // The scratch engine is only a turning tool: a locked item of the
        // copy must turn with the rest, and keep its lock on the result.
        let locks = take_locks(&mut shielded);
        let mut scratch = Engine::new(shielded)?;
        let command = build(&self.fragment, self.items());
        scratch.execute(command)?;
        let mut turned = scratch.document().clone();
        restore_locks(&mut turned, &locks);
        turned.junctions.retain(|j| minted.contains_key(&j.uuid));
        for j in &mut turned.junctions {
            j.minted = minted[&j.uuid];
        }
        Ok(Self::new(turned))
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
            symbol.uuid = Uuid::now_v7();
            // A placed copy is a new part: its designators come from its own
            // `reference`, never the original's per-placement entries (#1138).
            symbol.instances.clear();
            Command::PlaceSymbol { symbol }
        }));
        commands.extend(f.wires.iter().cloned().map(|mut wire| {
            wire.uuid = Uuid::now_v7();
            Command::PlaceWireSegmentAsIs { wire }
        }));
        commands.extend(f.buses.iter().cloned().map(|mut bus| {
            bus.uuid = Uuid::now_v7();
            Command::PlaceBus { bus }
        }));
        commands.extend(f.bus_entries.iter().cloned().map(|mut bus_entry| {
            bus_entry.uuid = Uuid::now_v7();
            Command::PlaceBusEntry { bus_entry }
        }));
        commands.extend(f.junctions.iter().cloned().map(|mut junction| {
            junction.uuid = Uuid::now_v7();
            Command::PlaceJunction { junction }
        }));
        commands.extend(f.no_connects.iter().cloned().map(|mut no_connect| {
            no_connect.uuid = Uuid::now_v7();
            Command::PlaceNoConnect { no_connect }
        }));
        commands.extend(f.labels.iter().cloned().map(|mut label| {
            label.uuid = Uuid::now_v7();
            Command::PlaceLabel { label }
        }));
        commands.extend(f.text_notes.iter().cloned().map(|mut text_note| {
            text_note.uuid = Uuid::now_v7();
            Command::PlaceTextNote { text_note }
        }));
        commands
    }
}

/// Clear every lock of `sheet` and return the ids that held one.
fn take_locks(sheet: &mut SchematicSheet) -> std::collections::HashSet<Uuid> {
    let mut locked = std::collections::HashSet::new();
    for s in sheet.symbols.iter_mut().filter(|s| s.locked) {
        s.locked = false;
        locked.insert(s.uuid);
    }
    for w in sheet.wires.iter_mut().filter(|w| w.locked) {
        w.locked = false;
        locked.insert(w.uuid);
    }
    for l in sheet.labels.iter_mut().filter(|l| l.locked) {
        l.locked = false;
        locked.insert(l.uuid);
    }
    for n in sheet.text_notes.iter_mut().filter(|n| n.locked) {
        n.locked = false;
        locked.insert(n.uuid);
    }
    for d in sheet.drawings.iter_mut().filter(|d| d.is_locked()) {
        d.set_locked(false);
        locked.insert(d.uuid());
    }
    locked
}

/// Put back the locks [`take_locks`] cleared.
fn restore_locks(sheet: &mut SchematicSheet, locked: &std::collections::HashSet<Uuid>) {
    sheet
        .symbols
        .iter_mut()
        .for_each(|s| s.locked = locked.contains(&s.uuid));
    sheet
        .wires
        .iter_mut()
        .for_each(|w| w.locked = locked.contains(&w.uuid));
    sheet
        .labels
        .iter_mut()
        .for_each(|l| l.locked = locked.contains(&l.uuid));
    sheet
        .text_notes
        .iter_mut()
        .for_each(|n| n.locked = locked.contains(&n.uuid));
    for d in &mut sheet.drawings {
        d.set_locked(locked.contains(&d.uuid()));
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
            uuid: Uuid::now_v7(),
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
    fn with_rotation_sets_the_stored_angle() {
        let payload = part_payload();
        let turned = payload.with_rotation(180.0).unwrap();
        assert_eq!(turned.lone_symbol().unwrap().rotation, 180.0);
        let mirrored = payload.mirror(MirrorAxis::Vertical).unwrap();
        let turned = mirrored.with_rotation(90.0).unwrap();
        assert_eq!(turned.lone_symbol().unwrap().rotation, 90.0);
    }

    #[test]
    fn map_symbols_edits_a_copy() {
        let payload = part_payload();
        let renamed = payload.map_symbols(|s| s.reference = "R7".to_owned());
        assert_eq!(renamed.lone_symbol().unwrap().reference, "R7");
        assert_eq!(payload.lone_symbol().unwrap().reference, "R1");
    }

    #[test]
    fn an_empty_fragment_places_nothing() {
        let payload = PlacementPayload::new(test_sheet());
        assert!(payload.is_empty());
    }

    use signex_types::schematic::{SchDrawing, Wire};

    fn group_with_locked_items() -> PlacementPayload {
        let mut sheet = SchematicSheet::default();
        sheet.wires.push(Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            locked: true,
            ..Default::default()
        });
        sheet.drawings.push(SchDrawing::Line {
            uuid: Uuid::now_v7(),
            start: Point::new(0.0, 5.0),
            end: Point::new(10.0, 5.0),
            width: 0.2,
            stroke_color: None,
            locked: true,
        });
        sheet.symbols.push(Symbol {
            uuid: Uuid::now_v7(),
            position: Point::new(20.0, 0.0),
            unit: 1,
            locked: true,
            ..Default::default()
        });
        sheet.labels.push(signex_types::schematic::Label {
            uuid: Uuid::now_v7(),
            text: "N".into(),
            position: Point::new(5.0, 3.0),
            rotation: 0.0,
            label_type: signex_types::schematic::LabelType::Net,
            shape: String::new(),
            font_size: 1.27,
            font_family: None,
            justify: Default::default(),
            justify_v: signex_types::schematic::VAlign::Bottom,
            locked: true,
            unknown_columns: Default::default(),
        });
        sheet.text_notes.push(signex_types::schematic::TextNote {
            uuid: Uuid::now_v7(),
            text: "t".into(),
            position: Point::new(6.0, 4.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: Default::default(),
            justify_v: Default::default(),
            locked: true,
        });
        PlacementPayload::new(sheet)
    }

    /// Space, X and Y turn the pasted group; a locked item turns with it and
    /// stays locked in the result.
    #[test]
    fn a_pasted_group_with_locked_items_still_turns_and_keeps_its_locks() {
        let payload = group_with_locked_items();
        for turned in [
            payload.rotate(false).unwrap(),
            payload.rotate(true).unwrap(),
            payload.mirror(MirrorAxis::Vertical).unwrap(),
            payload.mirror(MirrorAxis::Horizontal).unwrap(),
        ] {
            let f = turned.fragment();
            assert!(f.wires[0].locked && f.drawings[0].is_locked());
            assert!(f.symbols[0].locked && f.labels[0].locked && f.text_notes[0].locked);
        }
        let turned = payload.rotate(false).unwrap();
        assert_ne!(
            turned.fragment().wires[0].end,
            payload.fragment().wires[0].end,
            "it turned"
        );
    }

    /// A pasted wire is placed as copied: two collinear halves, as a Break Wire
    /// cut leaves them, do not fold back into one.
    #[test]
    fn a_pasted_split_wire_is_not_folded() {
        let mut sheet = SchematicSheet::default();
        for (a, b) in [(0.0, 5.0), (5.0, 10.0)] {
            sheet.wires.push(Wire {
                uuid: Uuid::now_v7(),
                start: Point::new(a, 0.0),
                end: Point::new(b, 0.0),
                ..Default::default()
            });
        }
        let payload = PlacementPayload::new(sheet);
        let mut engine = Engine::new(SchematicSheet::default()).unwrap();
        engine
            .execute_batch(payload.place_commands(Point::new(20.0, 20.0)).unwrap())
            .unwrap();
        assert_eq!(engine.document().wires.len(), 2);
    }
}
