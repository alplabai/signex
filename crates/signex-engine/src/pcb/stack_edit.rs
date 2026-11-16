//! One edit of the layer stack manager's table (#2401), made on a copy of
//! the stack. The result goes to the board as one
//! [`BoardCommand::SetLayerStack`](super::BoardCommand::SetLayerStack), which
//! checks what the board can hold (copper count, layers in use). Pure.

use signex_types::pcb_stack::{LayerStack, StackLayer, StackLayerKind};

/// One change to the table, by row index (top row 0).
#[derive(Debug, Clone, PartialEq)]
pub enum StackEdit {
    /// A new signal, plane or dielectric layer below row `below` (above the
    /// bottom copper when `None`), kept between the outer copper rows. A new
    /// copper layer brings a dielectric with it.
    Add {
        below: Option<usize>,
        kind: StackLayerKind,
    },
    /// Remove a row; a copper row takes a dielectric next to it along.
    Remove {
        row: usize,
    },
    /// Swap a row with the one above (`up`) or below.
    Move {
        row: usize,
        up: bool,
    },
    /// Switch a copper row between signal and plane.
    SetKind {
        row: usize,
        kind: StackLayerKind,
    },
    SetName {
        row: usize,
        text: String,
    },
    SetMaterial {
        row: usize,
        text: String,
    },
    SetThickness {
        row: usize,
        mm: f64,
    },
    SetDielectricConstant {
        row: usize,
        value: f64,
    },
    SetCopperWeight {
        row: usize,
        oz: f64,
    },
}

/// Why an edit was not made. The stack is unchanged.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum StackEditRefusal {
    #[error("the layer stack has no row {}", .0 + 1)]
    NoRow(usize),
    #[error("only signal, plane and dielectric layers can be added")]
    NotAddable,
    #[error("a copper layer switches only between signal and plane")]
    KindFixed,
    #[error("the value must be a number that is not negative")]
    BadNumber,
    #[error("the row is already at the end of the stack")]
    AtEdge,
}

/// `stack` with `edit` made.
pub fn edit_stack(stack: &LayerStack, edit: &StackEdit) -> Result<LayerStack, StackEditRefusal> {
    let mut out = stack.clone();
    let rows = &mut out.layers;
    let len = rows.len();
    let row_of = |row: usize| {
        (row < len)
            .then_some(row)
            .ok_or(StackEditRefusal::NoRow(row))
    };
    match edit {
        StackEdit::Add { below, kind } => add(rows, *below, *kind)?,
        StackEdit::Remove { row } => {
            let row = row_of(*row)?;
            remove(rows, row);
        }
        StackEdit::Move { row, up } => {
            let row = row_of(*row)?;
            let other = if *up {
                row.checked_sub(1)
            } else {
                Some(row + 1)
            };
            match other.filter(|o| *o < rows.len()) {
                Some(other) => rows.swap(row, other),
                None => return Err(StackEditRefusal::AtEdge),
            }
        }
        StackEdit::SetKind { row, kind } => {
            let row = row_of(*row)?;
            if !(rows[row].kind.is_copper() && kind.is_copper()) {
                return Err(StackEditRefusal::KindFixed);
            }
            rows[row].kind = *kind;
        }
        StackEdit::SetName { row, text } => rows[row_of(*row)?].name = text.trim().to_string(),
        StackEdit::SetMaterial { row, text } => {
            rows[row_of(*row)?].material = text.trim().to_string();
        }
        StackEdit::SetThickness { row, mm } => rows[row_of(*row)?].thickness_mm = number(*mm)?,
        StackEdit::SetDielectricConstant { row, value } => {
            rows[row_of(*row)?].dielectric_constant = number(*value)?;
        }
        StackEdit::SetCopperWeight { row, oz } => {
            rows[row_of(*row)?].copper_weight_oz = number(*oz)?;
        }
    }
    Ok(out)
}

fn number(value: f64) -> Result<f64, StackEditRefusal> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(StackEditRefusal::BadNumber)
    }
}

/// The first and last copper rows, or `None` for a stack without copper.
fn copper_bounds(rows: &[StackLayer]) -> Option<(usize, usize)> {
    let first = rows.iter().position(|l| l.kind.is_copper())?;
    let last = rows.iter().rposition(|l| l.kind.is_copper())?;
    Some((first, last))
}

fn add(
    rows: &mut Vec<StackLayer>,
    below: Option<usize>,
    kind: StackLayerKind,
) -> Result<(), StackEditRefusal> {
    if !(kind.is_copper() || kind == StackLayerKind::Dielectric) {
        return Err(StackEditRefusal::NotAddable);
    }
    if let Some(row) = below.filter(|r| *r >= rows.len()) {
        return Err(StackEditRefusal::NoRow(row));
    }
    let at = match copper_bounds(rows) {
        Some((first, last)) => below
            .map_or(last, |r| r + 1)
            .clamp(first + 1, last.max(first + 1)),
        None => below.map_or(rows.len(), |r| r + 1),
    };
    if kind == StackLayerKind::Dielectric {
        rows.insert(at, StackLayer::new(kind));
        return Ok(());
    }
    let new = StackLayer::new(kind);
    let dielectric = StackLayer::new(StackLayerKind::Dielectric);
    let after_copper = at > 0 && rows[at - 1].kind.is_copper();
    let pair = if after_copper {
        [dielectric, new]
    } else {
        [new, dielectric]
    };
    for (offset, layer) in pair.into_iter().enumerate() {
        rows.insert(at + offset, layer);
    }
    Ok(())
}

fn remove(rows: &mut Vec<StackLayer>, row: usize) {
    let copper = rows[row].kind.is_copper();
    rows.remove(row);
    if !copper {
        return;
    }
    let dielectric = |rows: &[StackLayer], i: usize| {
        rows.get(i)
            .is_some_and(|l| l.kind == StackLayerKind::Dielectric)
    };
    // The dielectric below it while copper is still below, else the one above.
    let copper_below = rows[row..].iter().any(|l| l.kind.is_copper());
    if copper_below && dielectric(rows, row) {
        rows.remove(row);
    } else if row > 0 && dielectric(rows, row - 1) {
        rows.remove(row - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(stack: &LayerStack) -> Vec<StackLayerKind> {
        stack.layers.iter().map(|l| l.kind).collect()
    }

    #[test]
    fn a_new_copper_layer_comes_with_a_dielectric_inside_the_outer_copper() {
        use StackLayerKind::*;
        let stack = LayerStack::derived(&[], 1.6);
        let added = edit_stack(
            &stack,
            &StackEdit::Add {
                below: None,
                kind: Signal,
            },
        )
        .unwrap();
        assert_eq!(added.copper_count(), 3);
        assert_eq!(
            kinds(&added),
            [
                Overlay, Paste, SolderMask, Signal, Dielectric, Signal, Dielectric, Signal,
                SolderMask, Paste, Overlay
            ]
        );
        // Below the top overlay still lands below the top copper.
        let top = edit_stack(
            &stack,
            &StackEdit::Add {
                below: Some(0),
                kind: Plane,
            },
        )
        .unwrap();
        assert_eq!(top.layers[3].kind, Signal);
        assert_eq!(top.layers[4].kind, Dielectric);
        assert_eq!(top.layers[5].kind, Plane);
    }

    #[test]
    fn removing_a_copper_layer_takes_its_dielectric_and_undoes_an_add() {
        let stack = LayerStack::derived(&[], 1.6);
        let added = edit_stack(
            &stack,
            &StackEdit::Add {
                below: None,
                kind: StackLayerKind::Signal,
            },
        )
        .unwrap();
        let removed = edit_stack(&added, &StackEdit::Remove { row: 5 }).unwrap();
        assert_eq!(removed, stack);
    }

    #[test]
    fn bad_edits_are_refused() {
        let stack = LayerStack::derived(&[], 1.6);
        let mask = StackEdit::Add {
            below: None,
            kind: StackLayerKind::SolderMask,
        };
        assert_eq!(edit_stack(&stack, &mask), Err(StackEditRefusal::NotAddable));
        let negative = StackEdit::SetThickness { row: 0, mm: -1.0 };
        assert_eq!(
            edit_stack(&stack, &negative),
            Err(StackEditRefusal::BadNumber)
        );
        let mask_kind = StackEdit::SetKind {
            row: 2,
            kind: StackLayerKind::Signal,
        };
        assert_eq!(
            edit_stack(&stack, &mask_kind),
            Err(StackEditRefusal::KindFixed)
        );
        let top = StackEdit::Move { row: 0, up: true };
        assert_eq!(edit_stack(&stack, &top), Err(StackEditRefusal::AtEdge));
        let gone = StackEdit::Remove { row: 99 };
        assert_eq!(edit_stack(&stack, &gone), Err(StackEditRefusal::NoRow(99)));
    }
}
