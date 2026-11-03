//! Comparing footprints by content, ignoring the silk bake markers.

use super::Footprint;

impl Footprint {
    /// Equal apart from the silk `sketch_entity` markers. The editor marks
    /// the baked silk of a footprint an older build saved when it opens it
    /// (#1718); that is not an edit, so a save's merge compares with this.
    pub fn same_ignoring_silk_markers(&self, other: &Footprint) -> bool {
        if self == other {
            return true;
        }
        let unmarked = |footprint: &Footprint| {
            let mut copy = footprint.clone();
            for graphic in copy.silk_f.iter_mut().chain(copy.silk_b.iter_mut()) {
                graphic.sketch_entity = None;
            }
            copy
        };
        unmarked(self) == unmarked(other)
    }
}
