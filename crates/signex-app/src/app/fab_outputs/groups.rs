//! The dialog's four groups of kinds (#115): what a board house, a
//! single-file job, an assembler and a mechanical designer each take.

use super::{FabOutput, FabOutputKinds};

/// A group of kinds as the dialog lists them, two groups to a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FabGroup {
    /// Gerber, drill and the test netlist: the board house's files.
    Fabrication,
    /// IPC-2581 and ODB++: the whole job in one file.
    SingleFile,
    /// Pick and place and the assembly drawings.
    Assembly,
    /// The STEP model.
    Model,
}

impl FabGroup {
    /// Every group: the first two in the left column, the others in the
    /// right.
    pub const ALL: [FabGroup; 4] = [
        FabGroup::Fabrication,
        FabGroup::SingleFile,
        FabGroup::Assembly,
        FabGroup::Model,
    ];

    /// The groups of the left and the right column.
    pub const COLUMNS: [[FabGroup; 2]; 2] = [
        [FabGroup::Fabrication, FabGroup::SingleFile],
        [FabGroup::Assembly, FabGroup::Model],
    ];

    /// The group's kinds, in the order it lists them.
    pub const fn kinds(self) -> &'static [FabOutput] {
        match self {
            FabGroup::Fabrication => &[FabOutput::Gerber, FabOutput::Drill, FabOutput::TestNetlist],
            FabGroup::SingleFile => &[FabOutput::Ipc2581, FabOutput::Odb],
            FabGroup::Assembly => &[
                FabOutput::Centroid,
                FabOutput::AssemblyPdf,
                FabOutput::AssemblySvg,
            ],
            FabGroup::Model => &[FabOutput::Step],
        }
    }

    /// How many of the group's kinds `kinds` ticks, and how many it has.
    pub fn count(self, kinds: &FabOutputKinds) -> (usize, usize) {
        let all = self.kinds();
        let ticked = all.iter().filter(|kind| kind.is_on(kinds)).count();
        (ticked, all.len())
    }
}

/// How many kinds of file `kinds` ticks; the ODB++ folder box is a form of
/// ODB++, not a kind.
pub fn ticked_count(kinds: &FabOutputKinds) -> usize {
    FabOutput::ALL
        .iter()
        .filter(|kind| kind.is_on(kinds))
        .count()
}
