//! Messages-panel warning for a part whose units draw with different versions
//! of their library symbol (#2209). Nothing is changed: the user decides when
//! to run Update from library.

use crate::app::Signex;

impl Signex {
    /// Warn once per part on the open sheets. Called when Annotate numbers
    /// units into parts and when a placement lands, the two places a unit can
    /// join a part of another version. Unopened sheets are not read.
    pub(crate) fn warn_mixed_version_parts(&self) {
        for (path, engine) in &self.document_state.engines {
            for reference in engine.document().mixed_version_parts() {
                crate::diagnostics::log_warning(format!(
                    "{reference} on {}: its units use different versions of the library symbol; \
                     use Update from library to bring them together",
                    path.display()
                ));
            }
        }
    }
}
