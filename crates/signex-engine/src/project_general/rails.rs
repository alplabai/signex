//! The project's power rails edit (#2943): set, change or remove one rail's
//! voltage and tolerance. One step on the project's General history; the
//! checks that refuse one live in `signex-types` (`ProjectData::set_rail`).

use signex_types::project::{ProjectData, RailEdit, RailProblem};

/// One edit of the project's rails: the rail `net` as `edit` asks.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRailCommand {
    pub net: String,
    pub edit: RailEdit,
}

impl ProjectRailCommand {
    /// Apply to `data`. A refused edit changes nothing.
    pub fn apply(&self, data: &mut ProjectData) -> Result<(), RailProblem> {
        data.set_rail(&self.net, &self.edit)
    }
}
