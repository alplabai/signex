//! The class-to-class matrix: one clearance per pair of classes. The pair
//! is symmetric, so only the lower triangle (column <= row) is live; the
//! mirror half is inert. An empty cell falls back to the larger of the two
//! classes' own clearances.

use std::collections::BTreeMap;

use super::update::{format_mm, parse_mm};

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
    /// The class's own clearance in mm, used where a pair has no cell.
    pub clearance_mm: f64,
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

/// A value change the caller stores in its rules.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatrixChange {
    ClearanceSet { pair: ClassPair, mm: f64 },
    ClearanceCleared { pair: ClassPair },
}

#[derive(Debug, Clone, Default)]
pub struct ClassMatrix {
    classes: Vec<MatrixClass>,
    cells: BTreeMap<ClassPair, f64>,
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

    /// Write a pair's clearance, as loaded from the caller's rules.
    /// Returns false for an unknown class or a negative or non-finite value.
    pub fn set(&mut self, pair: ClassPair, mm: f64) -> bool {
        if !self.has(pair) || !(mm.is_finite() && mm >= 0.0) {
            return false;
        }
        self.cells.insert(pair, mm);
        true
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

    /// The pair's clearance: its written value, else the fallback.
    pub fn clearance(&self, pair: ClassPair) -> Option<PairClearance> {
        if !self.has(pair) {
            return None;
        }
        if let Some(mm) = self.cells.get(&pair) {
            return Some(PairClearance::Written(*mm));
        }
        let a = self.classes[pair.row].clearance_mm;
        let b = self.classes[pair.col].clearance_mm;
        Some(PairClearance::Fallback(a.max(b)))
    }

    pub fn update(&mut self, message: MatrixMessage) -> Option<MatrixChange> {
        match message {
            MatrixMessage::CellPressed(pair) => {
                if !self.has(pair) {
                    return None;
                }
                self.pinned = Some(pair);
                self.edit = Some(MatrixEdit {
                    pair,
                    text: self
                        .cells
                        .get(&pair)
                        .map(|mm| format_mm(*mm))
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
            MatrixMessage::EditSubmitted => self.submit(),
            MatrixMessage::EditCancelled => {
                self.edit = None;
                None
            }
            MatrixMessage::CellCleared(pair) => {
                if self.edit.as_ref().is_some_and(|e| e.pair == pair) {
                    self.edit = None;
                }
                self.cells
                    .remove(&pair)
                    .map(|_| MatrixChange::ClearanceCleared { pair })
            }
        }
    }

    fn submit(&mut self) -> Option<MatrixChange> {
        let edit = self.edit.as_ref()?;
        let pair = edit.pair;
        let text = edit.text.trim();
        if text.is_empty() {
            self.edit = None;
            return self
                .cells
                .remove(&pair)
                .map(|_| MatrixChange::ClearanceCleared { pair });
        }
        let Some(mm) = parse_mm(text) else {
            if let Some(edit) = &mut self.edit {
                edit.is_invalid = true;
            }
            return None;
        };
        self.edit = None;
        if self.cells.get(&pair) == Some(&mm) {
            return None;
        }
        self.cells.insert(pair, mm);
        Some(MatrixChange::ClearanceSet { pair, mm })
    }
}
