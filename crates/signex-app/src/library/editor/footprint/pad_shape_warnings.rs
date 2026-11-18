//! A trapezoid pad whose delta makes no copper (a hand-edited `.snxfpt`,
//! #2242) is drawn as its plain box; say so once, when the file opens into
//! an editor, naming the footprint and the pad.

use std::path::Path;

use signex_library::FootprintFile;

/// Log one Messages warning per such pad in `file`.
pub(crate) fn warn_trapezoid_faults(path: &Path, file: &FootprintFile) {
    for footprint in &file.footprints {
        for fault in footprint.trapezoid_pad_faults() {
            crate::diagnostics::log_warning(format!(
                "{}: footprint {:?}, {fault}; the pad is drawn as a rectangle and is \
                 refused on a board until its delta is fixed",
                path.display(),
                footprint.name,
            ));
        }
    }
}
