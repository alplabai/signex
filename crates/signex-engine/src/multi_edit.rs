//! Editing the fields several selected objects share (Properties of a
//! multi-selection, #1258).
//!
//! A selection is split per kind (symbols, labels, wires). Each group reports
//! its fields as [`Shared`]: one value when every object agrees, `Mixed`
//! otherwise. [`edit_commands`] turns one edit into the commands that apply it
//! to every object of its group; the caller runs them as one batch, so one
//! undo reverts the whole change.

use std::collections::BTreeSet;

use signex_types::schematic::{HAlign, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use uuid::Uuid;

use crate::{Command, SymbolFlag, TextTarget};

/// Tolerance when comparing lengths and angles for "the same value".
const SAME_EPSILON: f64 = 1e-6;

/// What a field holds across a group.
#[derive(Debug, Clone, PartialEq)]
pub enum Shared<T> {
    Same(T),
    Mixed,
}

impl<T> Shared<T> {
    /// The common value, `None` when the group disagrees.
    pub fn same(&self) -> Option<&T> {
        match self {
            Shared::Same(value) => Some(value),
            Shared::Mixed => None,
        }
    }
}

/// Fold `values` into one [`Shared`]; `same` decides equality.
fn fold<T: Clone>(values: impl Iterator<Item = T>, same: impl Fn(&T, &T) -> bool) -> Shared<T> {
    let mut first: Option<T> = None;
    for value in values {
        match &first {
            None => first = Some(value),
            Some(seen) if same(seen, &value) => {}
            Some(_) => return Shared::Mixed,
        }
    }
    first.map_or(Shared::Mixed, Shared::Same)
}

fn fold_eq<T: Clone + PartialEq>(values: impl Iterator<Item = T>) -> Shared<T> {
    fold(values, |a, b| a == b)
}

fn fold_f64(values: impl Iterator<Item = f64>) -> Shared<f64> {
    fold(values, |a, b| (a - b).abs() <= SAME_EPSILON)
}

/// The placed parts of a selection (power ports excluded: their value is a
/// net name, and renaming nets in bulk is not a property edit).
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolGroup {
    pub count: usize,
    pub value: Shared<String>,
    pub footprint: Shared<String>,
    pub dnp: Shared<bool>,
    pub in_bom: Shared<bool>,
    pub on_board: Shared<bool>,
    /// Every custom parameter any of the parts carries, sorted by name; a part
    /// without it counts as a different value (the empty string).
    pub parameters: Vec<(String, Shared<String>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LabelGroup {
    pub count: usize,
    pub text: Shared<String>,
    pub font_size_mm: Shared<f64>,
    pub justify: Shared<HAlign>,
    pub rotation_degrees: Shared<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WireGroup {
    pub count: usize,
    pub width_mm: Shared<f64>,
}

/// The editable groups of a selection; a kind that is not selected is `None`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectionGroups {
    pub symbols: Option<SymbolGroup>,
    pub labels: Option<LabelGroup>,
    pub wires: Option<WireGroup>,
}

impl SelectionGroups {
    pub fn is_empty(&self) -> bool {
        self.symbols.is_none() && self.labels.is_none() && self.wires.is_none()
    }
}

/// One change to a whole group.
#[derive(Debug, Clone, PartialEq)]
pub enum MultiEdit {
    SymbolValue(String),
    SymbolFootprint(String),
    SymbolDnp(bool),
    SymbolInBom(bool),
    SymbolOnBoard(bool),
    /// An empty `value` removes the parameter, as in the Parameter Manager.
    SymbolParameter {
        key: String,
        value: String,
    },
    LabelText(String),
    LabelFontSizeMm(f64),
    LabelJustify(HAlign),
    LabelRotationDegrees(f64),
    WireWidthMm(f64),
}

fn selected_symbols<'a>(sheet: &'a SchematicSheet, items: &[SelectedItem]) -> Vec<&'a Symbol> {
    let ids = ids_of(items, SelectedKind::Symbol);
    sheet
        .symbols
        .iter()
        .filter(|s| ids.contains(&s.uuid) && !s.is_power)
        .collect()
}

fn ids_of(items: &[SelectedItem], kind: SelectedKind) -> BTreeSet<Uuid> {
    items
        .iter()
        .filter(|i| i.kind == kind)
        .map(|i| i.uuid)
        .collect()
}

fn symbol_group(symbols: &[&Symbol]) -> Option<SymbolGroup> {
    if symbols.is_empty() {
        return None;
    }
    let keys: BTreeSet<&String> = symbols.iter().flat_map(|s| s.fields.keys()).collect();
    let parameters = keys
        .into_iter()
        .map(|key| {
            let values = symbols
                .iter()
                .map(|s| s.fields.get(key).cloned().unwrap_or_default());
            (key.clone(), fold_eq(values))
        })
        .collect();
    Some(SymbolGroup {
        count: symbols.len(),
        value: fold_eq(symbols.iter().map(|s| s.value.clone())),
        footprint: fold_eq(symbols.iter().map(|s| s.footprint.clone())),
        dnp: fold_eq(symbols.iter().map(|s| s.dnp)),
        in_bom: fold_eq(symbols.iter().map(|s| s.in_bom)),
        on_board: fold_eq(symbols.iter().map(|s| s.on_board)),
        parameters,
    })
}

/// The groups `items` form on `sheet`. Objects of other kinds (designator
/// fields, junctions, drawings, ...) are not editable here and are ignored.
pub fn selection_groups(sheet: &SchematicSheet, items: &[SelectedItem]) -> SelectionGroups {
    let label_ids = ids_of(items, SelectedKind::Label);
    let labels: Vec<_> = sheet
        .labels
        .iter()
        .filter(|l| label_ids.contains(&l.uuid))
        .collect();
    let wire_ids = ids_of(items, SelectedKind::Wire);
    let wires: Vec<_> = sheet
        .wires
        .iter()
        .filter(|w| wire_ids.contains(&w.uuid))
        .collect();
    SelectionGroups {
        symbols: symbol_group(&selected_symbols(sheet, items)),
        labels: (!labels.is_empty()).then(|| LabelGroup {
            count: labels.len(),
            text: fold_eq(labels.iter().map(|l| l.text.clone())),
            font_size_mm: fold_f64(labels.iter().map(|l| l.font_size)),
            justify: fold_eq(labels.iter().map(|l| l.justify)),
            rotation_degrees: fold_f64(labels.iter().map(|l| l.rotation)),
        }),
        wires: (!wires.is_empty()).then(|| WireGroup {
            count: wires.len(),
            width_mm: fold_f64(wires.iter().map(|w| w.stroke_width)),
        }),
    }
}

/// The commands applying `edit` to every object of its group in `items`.
pub fn edit_commands(
    sheet: &SchematicSheet,
    items: &[SelectedItem],
    edit: &MultiEdit,
) -> Vec<Command> {
    let symbol_ids = || {
        selected_symbols(sheet, items)
            .into_iter()
            .map(|s| s.uuid)
            .collect::<Vec<_>>()
    };
    let label_ids = || {
        let ids = ids_of(items, SelectedKind::Label);
        sheet
            .labels
            .iter()
            .map(|l| l.uuid)
            .filter(move |id| ids.contains(id))
    };
    match edit {
        MultiEdit::SymbolValue(value) => symbol_ids()
            .into_iter()
            .map(|id| Command::UpdateText {
                target: TextTarget::SymbolValue(id),
                value: value.clone(),
            })
            .collect(),
        MultiEdit::SymbolFootprint(footprint) => symbol_ids()
            .into_iter()
            .map(|symbol_id| Command::UpdateSymbolFootprint {
                symbol_id,
                footprint: footprint.clone(),
            })
            .collect(),
        MultiEdit::SymbolDnp(dnp) => symbol_ids()
            .into_iter()
            .map(|symbol_id| Command::SetSymbolDnp {
                symbol_id,
                dnp: *dnp,
            })
            .collect(),
        MultiEdit::SymbolInBom(value) => flag_commands(symbol_ids(), SymbolFlag::InBom, *value),
        MultiEdit::SymbolOnBoard(value) => flag_commands(symbol_ids(), SymbolFlag::OnBoard, *value),
        MultiEdit::SymbolParameter { key, value } => symbol_ids()
            .into_iter()
            .map(|symbol_id| Command::SetSymbolField {
                symbol_id,
                key: key.clone(),
                value: value.clone(),
            })
            .collect(),
        MultiEdit::LabelText(text) => label_ids()
            .map(|id| Command::UpdateText {
                target: TextTarget::Label(id),
                value: text.clone(),
            })
            .collect(),
        MultiEdit::LabelFontSizeMm(mm) => label_props(label_ids(), Some(*mm), None, None),
        MultiEdit::LabelJustify(justify) => label_props(label_ids(), None, Some(*justify), None),
        MultiEdit::LabelRotationDegrees(deg) => label_props(label_ids(), None, None, Some(*deg)),
        MultiEdit::WireWidthMm(width_mm) => ids_of(items, SelectedKind::Wire)
            .into_iter()
            .map(|wire_id| Command::SetWireWidth {
                wire_id,
                width_mm: *width_mm,
            })
            .collect(),
    }
}

fn flag_commands(ids: Vec<Uuid>, flag: SymbolFlag, value: bool) -> Vec<Command> {
    ids.into_iter()
        .map(|symbol_id| Command::SetSymbolFlag {
            symbol_id,
            flag,
            value,
        })
        .collect()
}

fn label_props(
    ids: impl Iterator<Item = Uuid>,
    font_size_mm: Option<f64>,
    justify: Option<HAlign>,
    rotation_degrees: Option<f64>,
) -> Vec<Command> {
    ids.map(|label_id| Command::UpdateLabelProps {
        label_id,
        font_size_mm,
        justify,
        rotation_degrees,
    })
    .collect()
}

#[cfg(test)]
#[path = "multi_edit_tests.rs"]
mod tests;
