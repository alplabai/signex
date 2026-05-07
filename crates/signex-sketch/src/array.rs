use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::id::SketchEntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArrayId(pub Uuid);

impl ArrayId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ArrayId {
    fn default() -> Self {
        Self::new()
    }
}

/// A sketch array — expands to multiple baked primitives at bake time.
/// Each replica inherits attributes (PadAttr etc.) from `source`, with
/// per-instance overrides applied by the bake pipeline (number from
/// `numbering`, position from the array geometry).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Array {
    pub id: ArrayId,
    #[serde(flatten)]
    pub kind: ArrayKind,
    #[serde(default)]
    pub numbering: NumberingScheme,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum ArrayKind {
    /// One-dimensional array. v0.13 bakes this kind.
    Linear {
        source: SketchEntityId,
        count_expr: String,
        dx_expr: String,
        dy_expr: String,
    },

    /// Two-dimensional grid with optional depopulation. v0.14 bakes.
    Grid {
        source: SketchEntityId,
        nx_expr: String,
        ny_expr: String,
        dx_expr: String,
        dy_expr: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        depopulation: Option<GridDepopulation>,
    },

    /// Polar (circular) array. v0.14 bakes.
    Polar {
        source: SketchEntityId,
        center: SketchEntityId,
        count_expr: String,
        sweep_angle_expr: String,
        /// v0.22 Phase B5 — per-instance suppression. Mirrors
        /// [`GridDepopulation`] from `ArrayKind::Grid`. The
        /// expression is evaluated per index `i in 0..count`
        /// (the `j` bound from Grid is unused; consider it 0).
        /// `false` skips the instance without breaking the
        /// parametric chain. `None` (the default) keeps every
        /// instance.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        depopulation: Option<GridDepopulation>,
    },
}

/// Predicate evaluated per `(i, j)` index in a Grid array.
/// `true` keeps the instance; `false` skips it. `i`, `j`, `nx`, `ny`
/// are bound in scope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridDepopulation {
    pub mask_expr: String,
}

/// Pad-numbering scheme for an array's expanded primitives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum NumberingScheme {
    /// Sequential integers. Default.
    LinearIncrement {
        start_expr: String,
        step_expr: String,
    },

    /// BGA convention — row letter + col number (`A1`, `H17`). IPC-7351
    /// skips I/O/Q/S/X/Z when `skip_letters = true`.
    BgaRowCol {
        #[serde(default = "default_skip_letters")]
        skip_letters: bool,
        #[serde(default = "default_start_row")]
        start_row: char,
        #[serde(default = "default_start_col")]
        start_col: u32,
    },

    /// Explicit list — `names[i]` per index. For named-pin connectors.
    Explicit { names: Vec<String> },
}

impl Default for NumberingScheme {
    fn default() -> Self {
        Self::LinearIncrement {
            start_expr: "1".into(),
            step_expr: "1".into(),
        }
    }
}

fn default_skip_letters() -> bool {
    true
}
fn default_start_row() -> char {
    'A'
}
fn default_start_col() -> u32 {
    1
}

/// IPC-7351 BGA letter sequence. With `skip_letters` true, the alphabet
/// is `ABCDEFGHJKLMNPRTUVWY` (I/O/Q/S/X/Z skipped). Excel-style
/// extension produces `AA, AB, …` after the single-letter range.
pub fn bga_row_letter(row_index: u32, skip_letters: bool, start_row: char) -> String {
    let alphabet: Vec<char> = if skip_letters {
        "ABCDEFGHJKLMNPRTUVWY".chars().collect()
    } else {
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars().collect()
    };
    let start_idx = alphabet.iter().position(|&c| c == start_row).unwrap_or(0);
    let n = alphabet.len() as u32;
    let mut idx = start_idx as u32 + row_index;

    let mut digits = Vec::new();
    loop {
        digits.push(alphabet[(idx % n) as usize]);
        idx /= n;
        if idx == 0 {
            break;
        }
        idx -= 1;
    }
    let mut out = String::new();
    for c in digits.iter().rev() {
        out.push(*c);
    }
    out
}
