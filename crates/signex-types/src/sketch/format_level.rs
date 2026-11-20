//! Which format a sketch needs on disk. The links added in `snxfpt/1.2`
//! (reference dimensions, pattern sources and mirrors, per-instance pad
//! overrides) are lost on a re-save by a build that does not know them, so
//! a sketch holding any of them asks for the newer token.

use crate::sketch::data::SketchData;

impl SketchData {
    /// Whether the sketch holds anything only `snxfpt/1.2` (and the
    /// matching board token) carries: a reference dimension, a pattern
    /// source link, a pattern mirror or a per-instance pad override.
    pub fn needs_format_1_2(&self) -> bool {
        !self.driven_dimensions.is_empty()
            || !self.pattern_sources.is_empty()
            || !self.pattern_mirrors.is_empty()
            || !self.pad_overrides.is_empty()
    }
}
