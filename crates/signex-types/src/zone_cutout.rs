//! A pour cutout (#2871): a zone whose `fill_type` is [`POUR_CUTOUT_FILL`],
//! placed by the Pour Cutout tool on a copper layer. It holds no copper and
//! has no net; every polygon pour on its layer leaves its area empty when it
//! fills, and no pour's outline is changed. Stored in the `.snxpcb`
//! `[[pour_cutouts]]` array (`format::pour_cutouts`), apart from
//! `[[zones]]`, so a build without it ignores the cutout and keeps it.

use crate::pcb::Zone;

/// The `fill_type` of a pour cutout.
pub const POUR_CUTOUT_FILL: &str = "pour_cutout";

impl Zone {
    /// Whether this zone is a pour cutout: an area the pours on its layer
    /// leave empty.
    pub fn is_pour_cutout(&self) -> bool {
        self.fill_type == POUR_CUTOUT_FILL
    }
}
