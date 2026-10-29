//! The object lock, enforced at the command level: `Symbol::locked`, and
//! since #1573 the `locked` flag of wires, labels, text notes and drawings.
//!
//! A locked object is frozen: no command may move, turn, mirror, delete or
//! edit it until it is unlocked. Checking here, ahead of every handler,
//! covers each caller (canvas drag, keyboard, Properties panel, align,
//! batches) with one rule. `execute` refuses a command naming a locked
//! symbol whole, before anything is touched. A caller that would rather
//! carry on without the locked symbols (the app) asks `without_locked`
//! first and runs what is left; that also drops a wire ending on a locked
//! symbol's pin from a move or turn, since the symbol stays and the wire
//! would drag off its pin.
//!
//! Left open on purpose (full list: `docs/architecture/snxsch-object-lock.md`):
//! `SetSymbolLocked` and `SetItemLocked` (the way out), `SetSymbolDnp`,
//! `SetSymbolFlag`,
//! font family, library-symbol replacement and annotation (which has its
//! own designator lock, `scope.locked`).

use std::collections::{HashMap, HashSet};

use signex_net::pt_key;
use signex_net::uf::Key;
use signex_types::schematic::{SelectedItem, SelectedKind};
use uuid::Uuid;

use crate::command::{Command, SelectionTransform, TextTarget};
use crate::{Engine, EngineError};

impl Engine {
    /// The refusal for `cmd`, or `None` when it touches no locked object.
    pub(crate) fn locked_item_refusal(&self, cmd: &Command) -> Option<EngineError> {
        let (ids, action) = locked_targets(cmd)?;
        // A component change rewrites every placed unit of the part, so a
        // locked sibling unit refuses it too.
        let ids = match cmd {
            Command::ChangeComponent { symbol_id, .. } => {
                crate::exec::change_component::placed_unit_ids(&self.document, *symbol_id)
            }
            _ => ids,
        };
        let touched: HashSet<Uuid> = ids.into_iter().collect();
        let mut references: Vec<String> = self
            .locked_objects()
            .into_iter()
            .filter(|(id, _)| touched.contains(id))
            .map(|(_, name)| name)
            .collect();
        references.extend(self.symbols_held_by_locked_wires(cmd));
        if references.is_empty() {
            return None;
        }
        references.sort();
        references.dedup();
        Some(EngineError::ItemLocked { references, action })
    }
}

/// The ids of `items`, the fields of a symbol included (they carry the
/// symbol's id). Only a locked object's id can match, so kinds that cannot
/// be locked fall out when the ids are checked.
fn item_ids(items: &[SelectedItem]) -> Vec<Uuid> {
    items.iter().map(|i| i.uuid).collect()
}

impl Engine {
    /// Every locked object of the sheet by id, with the name a refusal or a
    /// skip notice calls it: a symbol's reference, a label's text, a
    /// wire / note / drawing by kind and place.
    fn locked_objects(&self) -> HashMap<Uuid, String> {
        let doc = &self.document;
        let at = |p: &signex_types::schematic::Point| format!("({:.2}, {:.2})", p.x, p.y);
        let mut out = HashMap::new();
        for s in doc.symbols.iter().filter(|s| s.locked) {
            out.insert(s.uuid, s.reference.clone());
        }
        for w in doc.wires.iter().filter(|w| w.locked) {
            out.insert(w.uuid, format!("wire {}", at(&w.start)));
        }
        for l in doc.labels.iter().filter(|l| l.locked) {
            out.insert(l.uuid, format!("label {}", l.text));
        }
        for n in doc.text_notes.iter().filter(|n| n.locked) {
            out.insert(n.uuid, format!("text {}", at(&n.position)));
        }
        for d in doc.drawings.iter().filter(|d| d.is_locked()) {
            out.insert(d.uuid(), drawing_name(d));
        }
        out
    }
}

/// How a refusal names a drawing: its shape and where it starts.
fn drawing_name(drawing: &signex_types::schematic::SchDrawing) -> String {
    use signex_types::schematic::SchDrawing;
    let (shape, p) = match drawing {
        SchDrawing::Line { start, .. } => ("line", start),
        SchDrawing::Rect { start, .. } => ("rectangle", start),
        SchDrawing::Circle { center, .. } => ("circle", center),
        SchDrawing::Arc { start, .. } => ("arc", start),
        SchDrawing::Polyline { points, .. } => match points.first() {
            Some(first) => ("polyline", first),
            None => return "polyline".to_string(),
        },
    };
    format!("{shape} ({:.2}, {:.2})", p.x, p.y)
}

/// The symbols `cmd` would change and a verb phrase for the refusal text;
/// `None` for a command that is not subject to the lock.
fn locked_targets(cmd: &Command) -> Option<(Vec<Uuid>, &'static str)> {
    let target = match cmd {
        Command::MoveSelection { items, .. } => (item_ids(items), "moved"),
        Command::MoveEach { moves, .. } => {
            let items: Vec<_> = moves.iter().map(|(item, _, _)| *item).collect();
            (item_ids(&items), "moved")
        }
        Command::RotateSelection { items, .. } => (item_ids(items), "rotated"),
        Command::MirrorSelection { items, .. } => (item_ids(items), "mirrored"),
        Command::TransformSelection {
            items, transform, ..
        } => {
            let action = match transform {
                SelectionTransform::Mirror(_) => "mirrored",
                SelectionTransform::RotateCounterclockwise
                | SelectionTransform::RotateClockwise => "rotated",
            };
            (item_ids(items), action)
        }
        Command::DeleteSelection { items } => (item_ids(items), "deleted"),
        Command::SetSymbolRotation { symbol_id, .. } => (vec![*symbol_id], "rotated"),
        Command::MoveSymbolAbsolute { symbol_id, .. } => (vec![*symbol_id], "moved"),
        Command::UpdateText { target, .. } => match target {
            TextTarget::SymbolReference(id)
            | TextTarget::SymbolValue(id)
            | TextTarget::Label(id)
            | TextTarget::TextNote(id) => (vec![*id], "edited"),
        },
        Command::UpdateLabelProps { label_id, .. } => (vec![*label_id], "edited"),
        Command::BreakWire { wire, .. } => (vec![*wire], "broken"),
        Command::UpdateSchDrawing { drawing } => (vec![drawing.uuid()], "edited"),
        Command::UpdateSymbolTextSize { symbol_id, .. }
        | Command::UpdateSymbolLibId { symbol_id, .. }
        | Command::UpdateSymbolFootprint { symbol_id, .. }
        | Command::ChangeComponent { symbol_id, .. }
        | Command::UpdateSymbolFields { symbol_id, .. }
        | Command::SetSymbolBodyStyle { symbol_id, .. }
        | Command::SetSymbolField { symbol_id, .. } => (vec![*symbol_id], "edited"),
        // A locked symbol's designator is part of what the lock freezes, on
        // a sheet placed once or many times alike.
        Command::SetSymbolDesignatorIn { symbol, .. } => (vec![*symbol], "edited"),
        // Exempt, by name (see the module docs): the way out of the lock,
        // properties the lock leaves open, and commands that name no
        // existing symbol.
        Command::SetSymbolLocked { .. }
        | Command::SetItemLocked { .. }
        | Command::SetSymbolDnp { .. }
        | Command::SetSymbolFlag { .. }
        | Command::SetWireWidth { .. }
        | Command::SetTextFontFamily { .. }
        | Command::ReplaceLibSymbol { .. }
        | Command::AnnotateAll { .. }
        | Command::ReplaceDocument { .. }
        | Command::PlaceWireSegment { .. }
        | Command::PlaceWireSegmentAsIs { .. }
        | Command::PlaceBus { .. }
        | Command::PlaceLabel { .. }
        | Command::PlaceSymbol { .. }
        | Command::RegisterLibSymbol { .. }
        | Command::PlaceChildSheet { .. }
        | Command::PlaceSheetPin { .. }
        | Command::PlaceJunction { .. }
        | Command::PlaceNoConnect { .. }
        | Command::PlaceNoErcDirective { .. }
        | Command::PlaceBusEntry { .. }
        | Command::PlaceTextNote { .. }
        | Command::PlaceSchDrawing { .. }
        | Command::UpdateChildSheetStyle { .. }
        | Command::ReorderObjects { .. }
        | Command::ReconcileChildSheetPins { .. }
        | Command::RenameChildSheetFile { .. }
        | Command::ModifyChildSheet { .. }
        | Command::SetPaperSize { .. }
        | Command::SetSheetFrame { .. }
        | Command::SetTitleBlockField { .. }
        | Command::ArrangeDrawings { .. }
        | Command::SetDrawingPlacement { .. }
        | Command::SetDocumentFontFamily { .. } => return None,
    };
    Some(target)
}

impl Engine {
    /// The lock of a wire, label, text note or drawing, `None` for any other
    /// kind (a symbol keeps its own `SetSymbolLocked` path).
    #[must_use]
    pub fn item_locked(&self, item: &SelectedItem) -> Option<bool> {
        let doc = &self.document;
        match item.kind {
            SelectedKind::Wire => doc
                .wires
                .iter()
                .find(|w| w.uuid == item.uuid)
                .map(|w| w.locked),
            SelectedKind::Label => doc
                .labels
                .iter()
                .find(|l| l.uuid == item.uuid)
                .map(|l| l.locked),
            SelectedKind::TextNote => doc
                .text_notes
                .iter()
                .find(|n| n.uuid == item.uuid)
                .map(|n| n.locked),
            SelectedKind::Drawing => doc
                .drawings
                .iter()
                .find(|d| d.uuid() == item.uuid)
                .map(|d| d.is_locked()),
            _ => None,
        }
    }
}

/// Whether `cmd` moves, turns or mirrors with the wires on the pins following.
fn keeps_wires(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::MoveSelection {
            keep_wires: true,
            ..
        } | Command::MoveEach {
            keep_wires: true,
            ..
        } | Command::RotateSelection {
            keep_wires: true,
            ..
        } | Command::MirrorSelection {
            keep_wires: true,
            ..
        } | Command::TransformSelection {
            keep_wires: true,
            ..
        }
    )
}

impl Engine {
    /// Where both ends of every locked wire sit.
    fn locked_wire_end_keys(&self) -> HashSet<Key> {
        self.document
            .wires
            .iter()
            .filter(|w| w.locked)
            .flat_map(|w| [pt_key(&w.start), pt_key(&w.end)])
            .collect()
    }

    /// Whether `item` is a part, sheet symbol, sheet pin or bus a locked wire
    /// ends on (at a pin, or at the far end of a bus entry it carries), so
    /// the rubber band would drag that wire.
    fn held_by_locked_wire(&self, item: &SelectedItem, ends: &HashSet<Key>) -> bool {
        if ends.is_empty() {
            return false;
        }
        match item.kind {
            // A dragged bus carries the far end of each entry on it, and the
            // wires attached there follow.
            SelectedKind::Bus => self
                .bus_followers(&[(*item, 0.0, 0.0)], true)
                .far_ends
                .iter()
                .any(|(key, ..)| ends.contains(key)),
            SelectedKind::Symbol | SelectedKind::ChildSheet | SelectedKind::SheetPin => {
                crate::moved_pin_tips(&self.document, std::slice::from_ref(item))
                    .iter()
                    .any(|tip| ends.contains(&tip.old_key()))
            }
            _ => false,
        }
    }

    /// The symbols `cmd` would move with the wires on their pins following
    /// while a locked wire ends on one of those pins, named for the notice.
    fn symbols_held_by_locked_wires(&self, cmd: &Command) -> Vec<String> {
        if !keeps_wires(cmd) {
            return Vec::new();
        }
        let items: Vec<SelectedItem> = match cmd {
            Command::MoveEach { moves, .. } => moves.iter().map(|(item, _, _)| *item).collect(),
            Command::MoveSelection { items, .. }
            | Command::RotateSelection { items, .. }
            | Command::MirrorSelection { items, .. }
            | Command::TransformSelection { items, .. } => items.clone(),
            _ => return Vec::new(),
        };
        let ends = self.locked_wire_end_keys();
        items
            .iter()
            .filter(|item| self.held_by_locked_wire(item, &ends))
            .map(|item| self.symbol_name(item.uuid))
            .collect()
    }

    /// How a skip notice names a symbol that stayed put because a locked
    /// wire is attached to it.
    fn symbol_name(&self, id: Uuid) -> String {
        let reference = self
            .document
            .symbols
            .iter()
            .find(|s| s.uuid == id)
            .map_or_else(
                || {
                    if self.document.buses.iter().any(|b| b.uuid == id) {
                        "bus".to_string()
                    } else {
                        id.to_string()
                    }
                },
                |s| s.reference.clone(),
            );
        format!("{reference} (a wire attached to it is locked)")
    }

    /// Where each pin of a locked symbol sits, with the symbol's reference.
    fn locked_pin_keys(&self) -> Vec<(Key, String)> {
        let locked: Vec<SelectedItem> = self
            .document
            .symbols
            .iter()
            .filter(|s| s.locked)
            .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol))
            .collect();
        crate::moved_pin_tips(&self.document, &locked)
            .into_iter()
            .filter_map(|tip| {
                let owner = self.document.symbols.iter().find(
                    |s| matches!(tip.ident(), crate::PinIdent::Terminal(id, _) if *id == s.uuid),
                )?;
                Some((tip.old_key(), owner.reference.clone()))
            })
            .collect()
    }

    /// Whether `cmd` names a locked object, so `execute` would refuse it.
    #[must_use]
    pub fn names_locked_item(&self, cmd: &Command) -> bool {
        self.locked_item_refusal(cmd).is_some()
    }

    /// `cmd` with every locked symbol left out, for a caller that would
    /// rather skip locked parts than have the whole command refused: the
    /// command to run (`None` when nothing is left of it) and the
    /// references that were skipped, sorted.
    ///
    /// A selection command loses its locked items; a command aimed at one
    /// locked symbol is dropped. `execute` still refuses a command that
    /// names a locked symbol, so this is the caller's choice, never a way
    /// around the lock.
    #[must_use]
    pub fn without_locked(&self, mut cmd: Command) -> (Option<Command>, Vec<String>) {
        let mut skipped = Vec::new();
        // A locked symbol stays put, so a wire ending on one of its pins must
        // not be moved either, or it would drag off the pin. Deleting such a
        // wire leaves the symbol alone, so delete skips only symbols.
        let pinned = if matches!(cmd, Command::DeleteSelection { .. }) {
            Vec::new()
        } else {
            self.locked_pin_keys()
        };
        // The reverse: a locked wire ending on a moved symbol's pin would be
        // dragged by the rubber band, so that symbol stays put and is named.
        let locked_wire_ends = if keeps_wires(&cmd) {
            self.locked_wire_end_keys()
        } else {
            HashSet::new()
        };
        let locked = self.locked_objects();
        let mut keep = |item: &SelectedItem| {
            let own = locked.get(&item.uuid);
            if let Some(name) = own {
                skipped.push(name.clone());
            }
            let on_pin = (item.kind == SelectedKind::Wire)
                .then(|| self.document.wires.iter().find(|w| w.uuid == item.uuid))
                .flatten()
                .and_then(|w| {
                    let ends = [pt_key(&w.start), pt_key(&w.end)];
                    pinned.iter().find(|(key, _)| ends.contains(key))
                });
            if let Some((_, reference)) = on_pin {
                skipped.push(reference.clone());
            }
            let on_locked_wire = own.is_none() && self.held_by_locked_wire(item, &locked_wire_ends);
            if on_locked_wire {
                skipped.push(self.symbol_name(item.uuid));
            }
            own.is_none() && on_pin.is_none() && !on_locked_wire
        };
        let emptied = match &mut cmd {
            Command::MoveSelection { items, .. }
            | Command::RotateSelection { items, .. }
            | Command::MirrorSelection { items, .. }
            | Command::TransformSelection { items, .. }
            | Command::DeleteSelection { items } => {
                items.retain(&mut keep);
                items.is_empty()
            }
            Command::MoveEach { moves, .. } => {
                moves.retain(|(item, _, _)| keep(item));
                moves.is_empty()
            }
            other => {
                if let Some(EngineError::ItemLocked { references, .. }) =
                    self.locked_item_refusal(other)
                {
                    skipped = references;
                    return (None, skipped);
                }
                false
            }
        };
        skipped.sort();
        skipped.dedup();
        ((!emptied).then_some(cmd), skipped)
    }
}
