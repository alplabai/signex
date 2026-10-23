//! The pin connection matrix: which pairs of pin electrical types may share a
//! net, and how loudly to complain when they do.
//!
//! The matrix lives here, once. The ERC dialog renders it, the dialog's cell
//! handler cycles it, and [`crate::RuleKind::PinTypeConflict`] reads it; none
//! of them keep a private copy.
//!
//! # Axes
//!
//! The matrix has six axes, but a pin carries one of 14
//! [`PinDirection`]s. [`PinAxis::of`] is the single place that folds the 14
//! onto the six, and its table is the decision record:
//!
//! | `PinDirection`                                    | axis          | why |
//! |---------------------------------------------------|---------------|-----|
//! | `Input`                                           | `Input`       | a pure sink |
//! | `Output`                                          | `Output`      | a push-pull driver; two on one net fight |
//! | `Bidirectional`                                   | `Bidir`       | |
//! | `ThreeStatable`, `OpenDrainLow`, `OpenDrainHigh`  | `Bidir`       | these may legally share a net (bus, wired-OR); filing them under `Output` would flag every shared bus as a short |
//! | `Differential`, `Clock`                           | `Bidir`       | the pin model does not say which way they drive, so they are not treated as a hard driver |
//! | `PowerInput`, `GroundReference`                   | `PowerIn`     | a ground pin is a supply sink whose rail is 0 V |
//! | `PowerOutput`                                     | `PowerOut`    | a regulator / battery output |
//! | `DoNotConnect`                                    | `NC`          | manufacturer-marked no-connect |
//! | `Passive`, `Unclassified`                         | *(none)*      | a resistor terminal can sit on any net, and an unclassified pin carries no claim to check; neither can conflict |

use std::collections::HashMap;

use signex_types::schematic::PinDirection;

use crate::Severity;

/// Number of axes on each side of the matrix.
pub const AXES: usize = 6;

/// One axis of the pin connection matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinAxis {
    Input,
    Output,
    Bidir,
    PowerIn,
    PowerOut,
    NotConnected,
}

impl PinAxis {
    /// Every axis, in row/column order of the matrix.
    pub const ALL: [PinAxis; AXES] = [
        PinAxis::Input,
        PinAxis::Output,
        PinAxis::Bidir,
        PinAxis::PowerIn,
        PinAxis::PowerOut,
        PinAxis::NotConnected,
    ];

    /// Row / column of this axis in the matrix.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Short header text shown on the ERC dialog's matrix.
    pub const fn label(self) -> &'static str {
        match self {
            PinAxis::Input => "Input",
            PinAxis::Output => "Output",
            PinAxis::Bidir => "Bidir",
            PinAxis::PowerIn => "PowerIn",
            PinAxis::PowerOut => "PowerOut",
            PinAxis::NotConnected => "NC",
        }
    }

    /// The axis a pin's electrical type is checked under, or `None` when the
    /// type never takes part in a conflict. See the module docs for why each
    /// of the 14 variants lands where it does.
    pub const fn of(direction: PinDirection) -> Option<PinAxis> {
        Some(match direction {
            PinDirection::Input => PinAxis::Input,
            PinDirection::Output => PinAxis::Output,
            PinDirection::Bidirectional
            | PinDirection::ThreeStatable
            | PinDirection::OpenDrainLow
            | PinDirection::OpenDrainHigh
            | PinDirection::Differential
            | PinDirection::Clock => PinAxis::Bidir,
            PinDirection::PowerInput | PinDirection::GroundReference => PinAxis::PowerIn,
            PinDirection::PowerOutput => PinAxis::PowerOut,
            PinDirection::DoNotConnect => PinAxis::NotConnected,
            PinDirection::Passive | PinDirection::Unclassified => return None,
        })
    }
}

const O: Severity = Severity::Off;
const E: Severity = Severity::Error;
const W: Severity = Severity::Warning;

/// Factory severity for every ordered axis pair, indexed `[row][col]` in
/// [`PinAxis::ALL`] order. Symmetric.
#[rustfmt::skip]
pub const BASELINE: [[Severity; AXES]; AXES] = [
    //         Input Output Bidir PowerIn PowerOut NC
    /* Input    */ [O, O, O, O, O, O],
    /* Output   */ [O, E, O, O, E, E],
    /* Bidir    */ [O, O, O, O, O, W],
    /* PowerIn  */ [O, O, O, O, O, E],
    /* PowerOut */ [O, E, O, O, E, E],
    /* NC       */ [O, E, W, E, E, O],
];

/// The baseline matrix plus the user's per-cell overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinMatrix {
    overrides: HashMap<(u8, u8), Severity>,
}

impl PinMatrix {
    /// The factory matrix with no overrides.
    pub fn baseline() -> Self {
        Self::default()
    }

    /// The baseline with `overrides` (keyed `(row, col)`) laid over it. Cells
    /// outside the 6x6 grid are dropped: they can only come from a stale or
    /// hand-edited preferences file and name no pair of axes.
    pub fn with_overrides(overrides: &HashMap<(u8, u8), Severity>) -> Self {
        let in_grid = |&(r, c): &(u8, u8)| usize::from(r) < AXES && usize::from(c) < AXES;
        Self {
            overrides: overrides
                .iter()
                .filter(|(cell, _)| in_grid(cell))
                .map(|(&cell, &sev)| (cell, sev))
                .collect(),
        }
    }

    /// Severity of the `(row, col)` cell: the override if there is one, else
    /// the baseline. `Off` for a cell outside the grid.
    pub fn cell(&self, row: u8, col: u8) -> Severity {
        self.overrides
            .get(&(row, col))
            .copied()
            .unwrap_or_else(|| baseline_cell(row, col))
    }

    /// Severity for two pins of axes `a` and `b` sharing a net. A pair is
    /// unordered, so the `(a, b)` cell wins and the mirrored `(b, a)` cell is
    /// the fallback when only that one was overridden.
    pub fn pair_severity(&self, a: PinAxis, b: PinAxis) -> Severity {
        let (r, c) = (a.index() as u8, b.index() as u8);
        self.overrides
            .get(&(r, c))
            .or_else(|| self.overrides.get(&(c, r)))
            .copied()
            .unwrap_or_else(|| baseline_cell(r, c))
    }
}

/// The factory severity of one cell; `Off` outside the grid.
pub fn baseline_cell(row: u8, col: u8) -> Severity {
    BASELINE
        .get(usize::from(row))
        .and_then(|r| r.get(usize::from(col)))
        .copied()
        .unwrap_or(Severity::Off)
}

/// Run-time configuration for ERC. `Default` is the factory baseline, which is
/// what every entry point without a `_with_config` suffix uses.
#[derive(Debug, Clone, Default)]
pub struct ErcConfig {
    pub pin_matrix: PinMatrix,
}

impl ErcConfig {
    /// A config carrying the user's pin-matrix overrides.
    pub fn with_pin_matrix_overrides(overrides: &HashMap<(u8, u8), Severity>) -> Self {
        Self {
            pin_matrix: PinMatrix::with_overrides(overrides),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_is_symmetric() {
        for (r, row) in BASELINE.iter().enumerate() {
            for (c, &cell) in row.iter().enumerate() {
                assert_eq!(cell, BASELINE[c][r], "cell ({r},{c})");
            }
        }
    }

    #[test]
    fn every_direction_has_a_documented_axis() {
        use PinDirection::*;
        let neutral = [Passive, Unclassified];
        let all = [
            Input,
            Output,
            Bidirectional,
            ThreeStatable,
            Passive,
            PowerInput,
            PowerOutput,
            GroundReference,
            OpenDrainLow,
            OpenDrainHigh,
            Differential,
            Clock,
            DoNotConnect,
            Unclassified,
        ];
        for d in all {
            assert_eq!(PinAxis::of(d).is_none(), neutral.contains(&d), "{d:?}");
        }
        // Shared-bus capable types must never be filed under the hard-driver axis.
        assert_eq!(PinAxis::of(ThreeStatable), Some(PinAxis::Bidir));
        assert_eq!(PinAxis::of(OpenDrainLow), Some(PinAxis::Bidir));
    }

    #[test]
    fn overrides_win_and_mirror_cell_is_the_fallback() {
        let m = PinMatrix::with_overrides(&HashMap::from([((4, 1), Severity::Warning)]));
        assert_eq!(m.cell(4, 1), Severity::Warning);
        assert_eq!(
            m.pair_severity(PinAxis::Output, PinAxis::PowerOut),
            Severity::Warning
        );
        assert_eq!(
            m.pair_severity(PinAxis::PowerOut, PinAxis::Output),
            Severity::Warning
        );
    }

    #[test]
    fn out_of_grid_overrides_are_ignored() {
        let m = PinMatrix::with_overrides(&HashMap::from([((9, 9), Severity::Error)]));
        assert_eq!(m, PinMatrix::baseline());
    }
}
