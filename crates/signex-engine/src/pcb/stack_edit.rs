//! One edit of the layer stack manager's table (#2401), made on a copy of
//! the stack. The result goes to the board as one
//! [`BoardCommand::SetLayerStack`](super::BoardCommand::SetLayerStack), which
//! checks what the board can hold (copper count, layers in use). Pure.
//! The Materials and Finish sub-tabs (mockup rows 176 and 177) edit through
//! here too, so a material pick or a finish change is one undo step.

use signex_types::material_library::Material;
use signex_types::pcb_stack::{BoardFinish, LayerStack, StackLayer, StackLayerKind};

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
    /// The row's loss tangent (Df); 0 clears it.
    SetLossTangent {
        row: usize,
        value: f64,
    },
    /// Copy a library material's values into a row it fits; the board keeps
    /// the copy, not a link to the library.
    UseMaterial {
        row: usize,
        material: Material,
    },
    /// Replace the board's mask, silkscreen and surface finish.
    SetFinish {
        finish: BoardFinish,
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
    #[error("this material does not fit the selected layer")]
    MaterialDoesNotFit,
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
        StackEdit::Move { row, up } => move_row(rows, row_of(*row)?, *up)?,
        StackEdit::SetKind { row, kind } => set_kind(&mut rows[row_of(*row)?], *kind)?,
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
        StackEdit::SetLossTangent { row, value } => {
            rows[row_of(*row)?].loss_tangent = number(*value)?;
        }
        StackEdit::UseMaterial { row, material } => {
            use_material(&mut rows[row_of(*row)?], material)?
        }
        StackEdit::SetFinish { finish } => out.finish = Some(checked_finish(finish)?),
    }
    Ok(out)
}

/// Swap `row` with the row above (`up`) or below.
fn move_row(rows: &mut [StackLayer], row: usize, up: bool) -> Result<(), StackEditRefusal> {
    let other = if up {
        row.checked_sub(1)
    } else {
        Some(row + 1)
    };
    match other.filter(|o| *o < rows.len()) {
        Some(other) => {
            rows.swap(row, other);
            Ok(())
        }
        None => Err(StackEditRefusal::AtEdge),
    }
}

/// A copper row switches between signal and plane; no other kind changes.
fn set_kind(row: &mut StackLayer, kind: StackLayerKind) -> Result<(), StackEditRefusal> {
    if !(row.kind.is_copper() && kind.is_copper()) {
        return Err(StackEditRefusal::KindFixed);
    }
    row.kind = kind;
    Ok(())
}

fn checked_finish(finish: &BoardFinish) -> Result<BoardFinish, StackEditRefusal> {
    if finish.numbers_valid() {
        Ok(finish.clone())
    } else {
        Err(StackEditRefusal::BadNumber)
    }
}

fn use_material(row: &mut StackLayer, material: &Material) -> Result<(), StackEditRefusal> {
    if !material.kind.fits(row.kind) {
        return Err(StackEditRefusal::MaterialDoesNotFit);
    }
    if material.fault().is_some() {
        return Err(StackEditRefusal::BadNumber);
    }
    material.copy_into(row);
    Ok(())
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

    fn library(name: &str) -> Material {
        signex_types::material_library::builtin_materials()
            .into_iter()
            .find(|m| m.name == name)
            .expect("a built-in material")
    }

    /// Mockup row 177: a picked material's values are copied into the row;
    /// the rest of the stack is untouched.
    #[test]
    fn a_library_material_is_copied_into_the_row_it_fits() {
        let stack = LayerStack::derived(&[], 1.6);
        let core = stack
            .layers
            .iter()
            .position(|l| l.kind == StackLayerKind::Dielectric)
            .unwrap();
        let material = library("Prepreg 1080");
        let used = edit_stack(
            &stack,
            &StackEdit::UseMaterial {
                row: core,
                material: material.clone(),
            },
        )
        .unwrap();
        let row = &used.layers[core];
        assert_eq!(row.material, "Prepreg 1080");
        assert_eq!(row.thickness_mm, material.thickness_mm);
        assert_eq!(row.dielectric_constant, material.dielectric_constant);
        assert_eq!(row.loss_tangent, material.loss_tangent);
        for (i, (a, b)) in used.layers.iter().zip(&stack.layers).enumerate() {
            assert!(i == core || a == b, "row {i} changed");
        }
    }

    #[test]
    fn a_material_that_does_not_fit_the_row_is_refused() {
        let stack = LayerStack::derived(&[], 1.6);
        let copper = stack
            .layers
            .iter()
            .position(|l| l.kind.is_copper())
            .unwrap();
        let prepreg = StackEdit::UseMaterial {
            row: copper,
            material: library("Prepreg 1080"),
        };
        assert_eq!(
            edit_stack(&stack, &prepreg),
            Err(StackEditRefusal::MaterialDoesNotFit)
        );
        let foil = StackEdit::UseMaterial {
            row: copper,
            material: library("Copper foil 2 oz"),
        };
        let heavier = edit_stack(&stack, &foil).unwrap();
        assert!((heavier.layers[copper].copper_weight_oz - 2.0).abs() < 1e-9);
        let mut bad = library("Prepreg 1080");
        bad.thickness_mm = -0.1;
        let negative = StackEdit::UseMaterial {
            row: 4,
            material: bad,
        };
        assert_eq!(
            edit_stack(&stack, &negative),
            Err(StackEditRefusal::BadNumber)
        );
    }

    /// Mockup row 176: the finish is set on the stack; a bad number is
    /// refused.
    #[test]
    fn the_finish_is_set_and_a_bad_number_refused() {
        use signex_types::pcb_stack::SurfaceFinish;
        let stack = LayerStack::derived(&[], 1.6);
        let finish = BoardFinish {
            surface: SurfaceFinish::Osp,
            ..BoardFinish::default()
        };
        let set = edit_stack(
            &stack,
            &StackEdit::SetFinish {
                finish: finish.clone(),
            },
        )
        .unwrap();
        assert_eq!(set.finish, Some(finish.clone()));
        assert_eq!(set.layers, stack.layers);
        let mut bad = finish;
        bad.mask.over_laminate_mm = f64::NAN;
        assert_eq!(
            edit_stack(&stack, &StackEdit::SetFinish { finish: bad }),
            Err(StackEditRefusal::BadNumber)
        );
    }

    /// Mockup row 177: the Df a material left on a row is typed over in the
    /// Stack table, 0 clears it, and a bad number is refused.
    #[test]
    fn a_row_loss_tangent_is_set_cleared_and_checked() {
        let stack = LayerStack::derived(&[], 1.6);
        let core = stack
            .layers
            .iter()
            .position(|l| l.kind == StackLayerKind::Dielectric)
            .unwrap();
        let set = |value| StackEdit::SetLossTangent { row: core, value };
        let with_df = edit_stack(&stack, &set(0.018)).unwrap();
        assert_eq!(with_df.layers[core].loss_tangent, 0.018);
        assert!(with_df.has_materials_data());
        let cleared = edit_stack(&with_df, &set(0.0)).unwrap();
        assert_eq!(cleared, stack, "a cleared Df leaves the stack as it was");
        assert_eq!(
            edit_stack(&stack, &set(-0.01)),
            Err(StackEditRefusal::BadNumber)
        );
        assert_eq!(
            edit_stack(
                &stack,
                &StackEdit::SetLossTangent {
                    row: 99,
                    value: 0.01
                }
            ),
            Err(StackEditRefusal::NoRow(99))
        );
    }
}
