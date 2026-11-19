//! The class-to-class matrix: one clearance per pair of classes. The pair
//! is symmetric, so only the lower triangle (column <= row) is live; the
//! mirror half is inert. An empty cell falls back to the larger of the two
//! classes' own clearances. The clearances are the caller's: the matrix
//! reads them through [`PairValues`] and holds only the view state.

use crate::LengthFormat;

/// A pair of classes by index, stored lower-triangle first: `row >= col`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassPair {
    pub row: usize,
    pub col: usize,
}

impl ClassPair {
    /// The pair of `a` and `b` in either order.
    pub fn new(a: usize, b: usize) -> Self {
        Self {
            row: a.max(b),
            col: a.min(b),
        }
    }

    /// Whether the cell at (`row`, `col`) is the live (lower) half.
    pub fn is_live(row: usize, col: usize) -> bool {
        col <= row
    }
}

/// One class of the matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct MatrixClass {
    /// The class name: user content.
    pub name: String,
}

/// The caller's read-only view of the clearances, in mm.
pub trait PairValues {
    /// The clearance written in the pair's cell, if any.
    fn written(&self, pair: ClassPair) -> Option<f64>;

    /// A class's own clearance, by index; `None` for a class the caller
    /// does not know.
    fn class_clearance(&self, class: usize) -> Option<f64>;
}

/// The value a pair resolves to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PairClearance {
    /// Written in the pair's cell.
    Written(f64),
    /// No cell: the larger of the two classes' own clearances.
    Fallback(f64),
}

impl PairClearance {
    pub fn mm(self) -> f64 {
        match self {
            Self::Written(mm) | Self::Fallback(mm) => mm,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixEdit {
    pub pair: ClassPair,
    pub text: String,
    pub is_invalid: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatrixMessage {
    /// A live cell was pressed: pin it and edit it.
    CellPressed(ClassPair),
    EditTextChanged(String),
    EditSubmitted,
    EditCancelled,
    /// A pinned cell's clear was pressed: drop its written value.
    CellCleared(ClassPair),
}

/// A value change for the caller to store in its rules; the matrix holds
/// no values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatrixChange {
    ClearanceSet { pair: ClassPair, mm: f64 },
    ClearanceCleared { pair: ClassPair },
}

/// The matrix's layout and view state: the classes in order, the pinned
/// cell and the open edit.
#[derive(Debug, Clone, Default)]
pub struct ClassMatrix {
    classes: Vec<MatrixClass>,
    pinned: Option<ClassPair>,
    edit: Option<MatrixEdit>,
}

impl ClassMatrix {
    pub fn new(classes: Vec<MatrixClass>) -> Self {
        Self {
            classes,
            ..Self::default()
        }
    }

    pub fn classes(&self) -> &[MatrixClass] {
        &self.classes
    }

    pub fn pinned(&self) -> Option<ClassPair> {
        self.pinned
    }

    pub fn edit(&self) -> Option<&MatrixEdit> {
        self.edit.as_ref()
    }

    fn has(&self, pair: ClassPair) -> bool {
        pair.row < self.classes.len()
    }

    /// The larger of the pair's two class clearances: what an empty cell
    /// shows.
    pub fn fallback(&self, values: &impl PairValues, pair: ClassPair) -> Option<f64> {
        if !self.has(pair) {
            return None;
        }
        let a = values.class_clearance(pair.row)?;
        let b = values.class_clearance(pair.col)?;
        Some(a.max(b))
    }

    /// The pair's clearance: its written value, else the fallback.
    pub fn clearance(&self, values: &impl PairValues, pair: ClassPair) -> Option<PairClearance> {
        if !self.has(pair) {
            return None;
        }
        match values.written(pair) {
            Some(mm) => Some(PairClearance::Written(mm)),
            None => self.fallback(values, pair).map(PairClearance::Fallback),
        }
    }

    /// Apply `message` to the view state. `values` is the caller's view of
    /// the clearances, `format` its locale and display unit. Returns the
    /// value change, if any.
    pub fn update(
        &mut self,
        message: MatrixMessage,
        values: &impl PairValues,
        format: LengthFormat<'_>,
    ) -> Option<MatrixChange> {
        match message {
            MatrixMessage::CellPressed(pair) => {
                if !self.has(pair) {
                    return None;
                }
                self.pinned = Some(pair);
                self.edit = Some(MatrixEdit {
                    pair,
                    text: values
                        .written(pair)
                        .map(|mm| format.field_text(mm))
                        .unwrap_or_default(),
                    is_invalid: false,
                });
                None
            }
            MatrixMessage::EditTextChanged(text) => {
                if let Some(edit) = &mut self.edit {
                    edit.text = text;
                    edit.is_invalid = false;
                }
                None
            }
            MatrixMessage::EditSubmitted => self.submit(values, format),
            MatrixMessage::EditCancelled => {
                self.edit = None;
                None
            }
            MatrixMessage::CellCleared(pair) => {
                if self.edit.as_ref().is_some_and(|e| e.pair == pair) {
                    self.edit = None;
                }
                cleared(pair, values)
            }
        }
    }

    fn submit(
        &mut self,
        values: &impl PairValues,
        format: LengthFormat<'_>,
    ) -> Option<MatrixChange> {
        let edit = self.edit.as_ref()?;
        let pair = edit.pair;
        let text = edit.text.trim();
        if text.is_empty() {
            self.edit = None;
            return cleared(pair, values);
        }
        // A clearance is never negative.
        let Some(mm) = format.parse(text).filter(|mm| *mm >= 0.0) else {
            if let Some(edit) = &mut self.edit {
                edit.is_invalid = true;
            }
            return None;
        };
        self.edit = None;
        if values.written(pair) == Some(mm) {
            return None;
        }
        Some(MatrixChange::ClearanceSet { pair, mm })
    }
}

/// The change that drops the pair's written value, if it has one.
fn cleared(pair: ClassPair, values: &impl PairValues) -> Option<MatrixChange> {
    values
        .written(pair)
        .map(|_| MatrixChange::ClearanceCleared { pair })
}
