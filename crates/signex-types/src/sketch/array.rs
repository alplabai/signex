use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::sketch::id::SketchEntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArrayId(pub Uuid);

impl ArrayId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
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
        /// Per-instance suppression, as a polar array keeps it: entries are
        /// `(i, 0)` (#2688, DECISIONS 2026-11-27). `None` keeps every
        /// instance and writes nothing; while set the footprint file and the
        /// board need their tokens for it (`SNXFPT_FORMAT_V1_5`,
        /// `SNXPCB_FORMAT_V1_13`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        depopulation: Option<GridDepopulation>,
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

impl ArrayKind {
    /// The entity every instance is replicated from.
    pub fn source(&self) -> SketchEntityId {
        match self {
            Self::Linear { source, .. }
            | Self::Grid { source, .. }
            | Self::Polar { source, .. } => *source,
        }
    }

    /// Every entity this array names: its source, plus the centre of a
    /// polar array.
    pub fn entity_refs(&self) -> Vec<SketchEntityId> {
        match self {
            Self::Linear { source, .. } | Self::Grid { source, .. } => vec![*source],
            Self::Polar { source, center, .. } => vec![*source, *center],
        }
    }

    /// Whether instance `(i, j)` is on the suppression list (`j` is 0 for
    /// a linear or a polar array). The mask expression is not evaluated
    /// here.
    pub fn is_instance_suppressed(&self, i: u32, j: u32) -> bool {
        self.depopulation()
            .is_some_and(|d| d.suppressed_instances.contains(&(i, j)))
    }

    /// Put instance `(i, j)` on the suppression list or take it off.
    /// The list gains a depopulation with an empty mask when it first
    /// needs one, and drops it again when the list empties and the mask is
    /// blank. Returns whether the array changed.
    pub fn set_instance_suppressed(&mut self, i: u32, j: u32, suppressed: bool) -> bool {
        let slot = self.depopulation_mut();
        if suppressed {
            let d = slot.get_or_insert_with(|| GridDepopulation {
                mask_expr: String::new(),
                suppressed_instances: Vec::new(),
            });
            if d.suppressed_instances.contains(&(i, j)) {
                return false;
            }
            d.suppressed_instances.push((i, j));
            return true;
        }
        let Some(d) = slot.as_mut() else {
            return false;
        };
        let before = d.suppressed_instances.len();
        d.suppressed_instances.retain(|&entry| entry != (i, j));
        let changed = d.suppressed_instances.len() != before;
        if d.mask_expr.trim().is_empty() && d.suppressed_instances.is_empty() {
            *slot = None;
        }
        changed
    }

    /// Replace the suppression list with `cells` (the Numbering Mask, mockup
    /// row 126), keeping the mask expression. The depopulation goes when
    /// both end up empty. Returns whether the array changed.
    pub fn set_suppressed_instances(&mut self, cells: Vec<(u32, u32)>) -> bool {
        let slot = self.depopulation_mut();
        let before = slot.clone();
        let mask_expr = slot.take().map(|d| d.mask_expr).unwrap_or_default();
        if !(mask_expr.trim().is_empty() && cells.is_empty()) {
            *slot = Some(GridDepopulation {
                mask_expr,
                suppressed_instances: cells,
            });
        }
        *slot != before
    }

    /// Flip the suppression of instance `(i, j)`. Returns whether the
    /// array changed.
    pub fn toggle_instance_suppressed(&mut self, i: u32, j: u32) -> bool {
        let suppressed = self.is_instance_suppressed(i, j);
        self.set_instance_suppressed(i, j, !suppressed)
    }

    /// The array's depopulation: its mask and suppression list.
    pub fn depopulation(&self) -> Option<&GridDepopulation> {
        match self {
            Self::Linear { depopulation, .. }
            | Self::Grid { depopulation, .. }
            | Self::Polar { depopulation, .. } => depopulation.as_ref(),
        }
    }

    fn depopulation_mut(&mut self) -> &mut Option<GridDepopulation> {
        match self {
            Self::Linear { depopulation, .. }
            | Self::Grid { depopulation, .. }
            | Self::Polar { depopulation, .. } => depopulation,
        }
    }
}

/// Predicate evaluated per `(i, j)` index in a Grid array.
/// `true` keeps the instance; `false` skips it. `i`, `j`, `nx`, `ny`
/// are bound in scope.
///
/// v0.23 — `suppressed_instances` is an explicit list of `(i, j)`
/// indices to skip *in addition to* whatever `mask_expr` evaluates to.
/// The bake skips an instance when EITHER predicate fires (mask returns
/// false OR the index is in the list). This lets the Properties
/// panel's per-instance checkbox grid (Phase B5 UI) toggle individual
/// pads without needing to round-trip through an expression parser.
/// Linear and polar arrays use `j = 0` for every entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridDepopulation {
    pub mask_expr: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppressed_instances: Vec<(u32, u32)>,
}

/// Pad-numbering scheme for an array's expanded primitives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum NumberingScheme {
    /// Sequential integers. Default.
    LinearIncrement {
        start_expr: String,
        step_expr: String,
        /// The order a grid's cells take their numbers (mockup row 126).
        /// A one-dimensional pattern has no second axis and ignores it.
        #[serde(default, skip_serializing_if = "GridSweep::is_row_by_row")]
        sweep: GridSweep,
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
            sweep: GridSweep::default(),
        }
    }
}

/// The order a grid pattern numbers its cells (mockup row 126).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridSweep {
    /// Along each row, then the next row.
    #[default]
    RowByRow,
    /// Along each column, then the next column.
    ColumnByColumn,
    /// Down the first column, up the next: the DIP order.
    DownThenUp,
}

impl GridSweep {
    pub const ALL: [Self; 3] = [Self::RowByRow, Self::ColumnByColumn, Self::DownThenUp];

    pub fn is_row_by_row(&self) -> bool {
        *self == Self::RowByRow
    }

    /// The 0-based place of cell `(i, j)` in the numbering of an `nx` × `ny`
    /// grid. A `j` past the last row (a mirrored instance) keeps its
    /// row-major place, so it continues after the grid's own numbers.
    pub fn sequence(self, i: usize, j: usize, nx: usize, ny: usize) -> usize {
        if j >= ny {
            return j * nx + i;
        }
        match self {
            Self::RowByRow => j * nx + i,
            Self::ColumnByColumn => i * ny + j,
            Self::DownThenUp if i % 2 == 1 => i * ny + (ny - 1 - j),
            Self::DownThenUp => i * ny + j,
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

/// The BGA row alphabet: IPC-7351 (I/O/Q/S/X/Z skipped) or the full one.
fn bga_row_alphabet(skip_letters: bool) -> &'static str {
    if skip_letters {
        "ABCDEFGHJKLMNPRTUVWY"
    } else {
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    }
}

/// `true` when `start_row` is in the BGA row alphabet for `skip_letters`.
pub fn is_valid_bga_start_row(skip_letters: bool, start_row: char) -> bool {
    bga_row_alphabet(skip_letters).contains(start_row)
}

/// IPC-7351 BGA letter sequence. With `skip_letters` true, the alphabet
/// is `ABCDEFGHJKLMNPRTUVWY` (I/O/Q/S/X/Z skipped). Excel-style
/// extension produces `AA, AB, …` after the single-letter range.
///
/// A `start_row` that is not in the alphabet (for example `'I'` with
/// `skip_letters` true, or a lowercase letter) falls back to `'A'`.
pub fn bga_row_letter(row_index: u32, skip_letters: bool, start_row: char) -> String {
    let alphabet: Vec<char> = bga_row_alphabet(skip_letters).chars().collect();
    let start_idx = alphabet.iter().position(|&c| c == start_row).unwrap_or(0);
    let n = alphabet.len() as u64;
    // u64 so `start_idx + row_index` cannot overflow for any `row_index`.
    let mut idx = start_idx as u64 + u64::from(row_index);

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

#[cfg(test)]
mod suppression_tests {
    use super::*;

    fn grid(depopulation: Option<GridDepopulation>) -> ArrayKind {
        ArrayKind::Grid {
            source: SketchEntityId::new(),
            nx_expr: "2".into(),
            ny_expr: "4".into(),
            dx_expr: "1".into(),
            dy_expr: "1".into(),
            depopulation,
        }
    }

    /// Mockup row 126: the Numbering Mask replaces the list, keeps the mask
    /// expression, and an empty list with a blank mask drops the depopulation.
    #[test]
    fn the_suppressed_list_is_replaced_and_keeps_the_mask_expression() {
        let mut kind = grid(Some(GridDepopulation {
            mask_expr: "i != 0".into(),
            suppressed_instances: vec![(0, 0)],
        }));
        assert!(kind.set_suppressed_instances(vec![(1, 2), (0, 3)]));
        assert!(kind.is_instance_suppressed(1, 2));
        assert!(kind.is_instance_suppressed(0, 3));
        assert!(!kind.is_instance_suppressed(0, 0));
        assert!(!kind.set_suppressed_instances(vec![(1, 2), (0, 3)]));
        assert!(kind.set_suppressed_instances(Vec::new()));
        let ArrayKind::Grid { depopulation, .. } = &kind else {
            unreachable!()
        };
        assert_eq!(
            depopulation.as_ref().map(|d| d.mask_expr.as_str()),
            Some("i != 0")
        );

        let mut bare = grid(None);
        assert!(bare.set_suppressed_instances(vec![(0, 1)]));
        assert!(bare.set_suppressed_instances(Vec::new()));
        assert!(matches!(
            bare,
            ArrayKind::Grid {
                depopulation: None,
                ..
            }
        ));
    }

    /// #2688: a linear pattern keeps `(i, 0)` entries as a polar one does,
    /// and an emptied list leaves no depopulation behind.
    #[test]
    fn a_linear_pattern_keeps_a_suppressed_list() {
        let start = ArrayKind::Linear {
            source: SketchEntityId::new(),
            count_expr: "4".into(),
            dx_expr: "1".into(),
            dy_expr: "0".into(),
            depopulation: None,
        };
        let mut kind = start.clone();
        assert!(kind.set_suppressed_instances(vec![(2, 0)]));
        assert!(kind.is_instance_suppressed(2, 0));
        assert!(kind.toggle_instance_suppressed(3, 0));
        assert_eq!(
            kind.depopulation().map(|d| d.suppressed_instances.clone()),
            Some(vec![(2, 0), (3, 0)])
        );
        assert!(kind.toggle_instance_suppressed(3, 0));
        assert!(kind.set_instance_suppressed(2, 0, false));
        assert_eq!(kind, start, "an empty list with a blank mask is gone");
    }
}
