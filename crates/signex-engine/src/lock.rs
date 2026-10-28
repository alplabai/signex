//! The symbol lock (`Symbol::locked`), enforced at the command level.
//!
//! A locked symbol is frozen: no command may move, turn, mirror, delete or
//! edit it until it is unlocked. Checking here, ahead of every handler,
//! covers each caller (canvas drag, keyboard, Properties panel, align,
//! batches) with one rule. `execute` refuses a command naming a locked
//! symbol whole, before anything is touched. A caller that would rather
//! carry on without the locked symbols (the app) asks `without_locked`
//! first and runs what is left; that also drops a wire ending on a locked
//! symbol's pin from a move or turn, since the symbol stays and the wire
//! would drag off its pin.
//!
//! Left open on purpose: `SetSymbolLocked` (the way out), `SetSymbolDnp`,
//! font family, library-symbol replacement and annotation (which has its
//! own designator lock, `scope.locked`).

use std::collections::HashSet;

use signex_net::pt_key;
use signex_net::uf::Key;
use signex_types::schematic::{SelectedItem, SelectedKind};
use uuid::Uuid;

use crate::command::{Command, SelectionTransform, TextTarget};
use crate::{Engine, EngineError};

impl Engine {
    /// The refusal for `cmd`, or `None` when it touches no locked symbol.
    pub(crate) fn locked_symbol_refusal(&self, cmd: &Command) -> Option<EngineError> {
        let (ids, action) = locked_targets(cmd)?;
        let touched: HashSet<Uuid> = ids.into_iter().collect();
        let mut references: Vec<String> = self
            .document
            .symbols
            .iter()
            .filter(|s| s.locked && touched.contains(&s.uuid))
            .map(|s| s.reference.clone())
            .collect();
        if references.is_empty() {
            return None;
        }
        references.sort();
        references.dedup();
        Some(EngineError::SymbolLocked { references, action })
    }
}

/// The symbol ids among `items`, fields of a symbol included.
fn symbol_ids(items: &[SelectedItem]) -> Vec<Uuid> {
    items
        .iter()
        .filter(|i| {
            matches!(
                i.kind,
                SelectedKind::Symbol | SelectedKind::SymbolRefField | SelectedKind::SymbolValField
            )
        })
        .map(|i| i.uuid)
        .collect()
}

/// The symbols `cmd` would change and a verb phrase for the refusal text;
/// `None` for a command that is not subject to the lock.
fn locked_targets(cmd: &Command) -> Option<(Vec<Uuid>, &'static str)> {
    let target = match cmd {
        Command::MoveSelection { items, .. } => (symbol_ids(items), "moved"),
        Command::MoveEach { moves, .. } => {
            let items: Vec<_> = moves.iter().map(|(item, _, _)| *item).collect();
            (symbol_ids(&items), "moved")
        }
        Command::RotateSelection { items, .. } => (symbol_ids(items), "rotated"),
        Command::MirrorSelection { items, .. } => (symbol_ids(items), "mirrored"),
        Command::TransformSelection {
            items, transform, ..
        } => {
            let action = match transform {
                SelectionTransform::Mirror(_) => "mirrored",
                SelectionTransform::RotateCounterclockwise
                | SelectionTransform::RotateClockwise => "rotated",
            };
            (symbol_ids(items), action)
        }
        Command::DeleteSelection { items } => (symbol_ids(items), "deleted"),
        Command::SetSymbolRotation { symbol_id, .. } => (vec![*symbol_id], "rotated"),
        Command::MoveSymbolAbsolute { symbol_id, .. } => (vec![*symbol_id], "moved"),
        Command::UpdateText { target, .. } => match target {
            TextTarget::SymbolReference(id) | TextTarget::SymbolValue(id) => (vec![*id], "edited"),
            TextTarget::Label(_) | TextTarget::TextNote(_) => return None,
        },
        Command::UpdateSymbolTextSize { symbol_id, .. }
        | Command::UpdateSymbolLibId { symbol_id, .. }
        | Command::UpdateSymbolFootprint { symbol_id, .. }
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
        | Command::SetSymbolDnp { .. }
        | Command::SetTextFontFamily { .. }
        | Command::ReplaceLibSymbol { .. }
        | Command::AnnotateAll { .. }
        | Command::ReplaceDocument { .. }
        | Command::UpdateLabelProps { .. }
        | Command::PlaceWireSegment { .. }
        | Command::PlaceBus { .. }
        | Command::PlaceLabel { .. }
        | Command::PlaceSymbol { .. }
        | Command::RegisterLibSymbol { .. }
        | Command::PlaceChildSheet { .. }
        | Command::PlaceJunction { .. }
        | Command::PlaceNoConnect { .. }
        | Command::PlaceNoErcDirective { .. }
        | Command::PlaceBusEntry { .. }
        | Command::PlaceTextNote { .. }
        | Command::PlaceSchDrawing { .. }
        | Command::UpdateSchDrawing { .. }
        | Command::UpdateChildSheetStyle { .. }
        | Command::ReorderObjects { .. }
        | Command::ReconcileChildSheetPins { .. }
        | Command::RenameChildSheetFile { .. }
        | Command::ModifyChildSheet { .. }
        | Command::SetPaperSize { .. }
        | Command::SetSheetFrame { .. }
        | Command::SetTitleBlockField { .. }
        | Command::SetDocumentFontFamily { .. } => return None,
    };
    Some(target)
}

impl Engine {
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

    /// Whether `cmd` names a locked symbol, so `execute` would refuse it.
    #[must_use]
    pub fn names_locked_symbol(&self, cmd: &Command) -> bool {
        self.locked_symbol_refusal(cmd).is_some()
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
        let mut keep = |item: &SelectedItem| {
            let locked = symbol_ids(std::slice::from_ref(item))
                .first()
                .and_then(|id| self.document.symbols.iter().find(|s| s.uuid == *id))
                .filter(|s| s.locked);
            if let Some(symbol) = locked {
                skipped.push(symbol.reference.clone());
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
            locked.is_none() && on_pin.is_none()
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
                if let Some(EngineError::SymbolLocked { references, .. }) =
                    self.locked_symbol_refusal(other)
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
