//! The symbol lock (`Symbol::locked`), enforced at the command level.
//!
//! A locked symbol is frozen: no command may move, turn, mirror, delete or
//! edit it until it is unlocked. Checking here, ahead of every handler,
//! covers each caller (canvas drag, keyboard, Properties panel, align,
//! batches) with one rule. A command naming a locked symbol is refused
//! whole, before anything is touched, so a mixed selection never half-moves.
//!
//! Left open on purpose: `SetSymbolLocked` (the way out), `SetSymbolDnp`,
//! font family, per-sheet designator, library-symbol replacement and
//! annotation (which has its own designator lock, `scope.locked`).

use std::collections::HashSet;

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
                _ => "rotated",
            };
            (symbol_ids(items), action)
        }
        Command::DeleteSelection { items } => (symbol_ids(items), "deleted"),
        Command::SetSymbolRotation { symbol_id, .. } => (vec![*symbol_id], "rotated"),
        Command::MoveSymbolAbsolute { symbol_id, .. } => (vec![*symbol_id], "moved"),
        Command::UpdateText {
            target: TextTarget::SymbolReference(id) | TextTarget::SymbolValue(id),
            ..
        } => (vec![*id], "edited"),
        Command::UpdateSymbolTextSize { symbol_id, .. }
        | Command::UpdateSymbolLibId { symbol_id, .. }
        | Command::UpdateSymbolFootprint { symbol_id, .. }
        | Command::UpdateSymbolFields { symbol_id, .. }
        | Command::SetSymbolField { symbol_id, .. } => (vec![*symbol_id], "edited"),
        _ => return None,
    };
    Some(target)
}
