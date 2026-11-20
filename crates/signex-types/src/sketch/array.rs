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

    /// Whether single instances of this array can be suppressed. A grid
    /// and a polar array keep a suppression list; a linear array has none.
    pub fn has_suppression_model(&self) -> bool {
        !matches!(self, Self::Linear { .. })
    }

    /// Whether instance `(i, j)` is on the suppression list (`j` is 0 for
    /// a polar array). The mask expression is not evaluated here.
    pub fn is_instance_suppressed(&self, i: u32, j: u32) -> bool {
        self.depopulation()
            .is_some_and(|d| d.suppressed_instances.contains(&(i, j)))
    }

    /// Put instance `(i, j)` on the suppression list or take it off.
    /// The list gains a depopulation with an empty mask when it first
    /// needs one, and drops it again when the list empties and the mask is
    /// blank. Returns whether the array changed; a linear array never does.
    pub fn set_instance_suppressed(&mut self, i: u32, j: u32, suppressed: bool) -> bool {
        let slot = match self {
            Self::Grid { depopulation, .. } | Self::Polar { depopulation, .. } => depopulation,
            Self::Linear { .. } => return false,
        };
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

    /// Flip the suppression of instance `(i, j)`. Returns whether the
    /// array changed.
    pub fn toggle_instance_suppressed(&mut self, i: u32, j: u32) -> bool {
        let suppressed = self.is_instance_suppressed(i, j);
        self.set_instance_suppressed(i, j, !suppressed)
    }

    fn depopulation(&self) -> Option<&GridDepopulation> {
        match self {
            Self::Grid { depopulation, .. } | Self::Polar { depopulation, .. } => {
                depopulation.as_ref()
            }
            Self::Linear { .. } => None,
        }
    }
}

/// An array's shape once its expressions are evaluated: the counts and
/// steps as numbers (mm, rad). Places each instance slot relative to the
/// source, the way the bake expands the array.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArrayLayout {
    /// `count` instances, each `step` further from the source.
    Linear { count: u32, step: (f64, f64) },
    /// `nx` by `ny` instances; `i` steps along x, `j` along y.
    Grid { nx: u32, ny: u32, step: (f64, f64) },
    /// `count` instances turned about `center`, together sweeping
    /// `sweep_rad`.
    Polar {
        count: u32,
        sweep_rad: f64,
        center: (f64, f64),
    },
}

impl ArrayLayout {
    /// How many instance slots the array has.
    pub fn slot_count(&self) -> u64 {
        match *self {
            Self::Linear { count, .. } | Self::Polar { count, .. } => u64::from(count),
            Self::Grid { nx, ny, .. } => u64::from(nx) * u64::from(ny),
        }
    }

    /// Every instance slot `(i, j)`, the source's `(0, 0)` first.
    pub fn slots(&self) -> Vec<(u32, u32)> {
        match *self {
            Self::Linear { count, .. } | Self::Polar { count, .. } => {
                (0..count).map(|i| (i, 0)).collect()
            }
            Self::Grid { nx, ny, .. } => {
                (0..ny).flat_map(|j| (0..nx).map(move |i| (i, j))).collect()
            }
        }
    }

    /// Where slot `(i, j)` sits when the source sits at `source`.
    pub fn position(&self, source: (f64, f64), i: u32, j: u32) -> (f64, f64) {
        match *self {
            Self::Linear { step, .. } => (
                source.0 + f64::from(i) * step.0,
                source.1 + f64::from(i) * step.1,
            ),
            Self::Grid { step, .. } => (
                source.0 + f64::from(i) * step.0,
                source.1 + f64::from(j) * step.1,
            ),
            Self::Polar {
                count,
                sweep_rad,
                center,
            } => {
                let theta = f64::from(i) * sweep_rad / f64::from(count.max(1));
                let (dx, dy) = (source.0 - center.0, source.1 - center.1);
                let (sin, cos) = theta.sin_cos();
                (
                    center.0 + dx * cos - dy * sin,
                    center.1 + dx * sin + dy * cos,
                )
            }
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
/// Polar arrays use `j = 0` for every entry.
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
