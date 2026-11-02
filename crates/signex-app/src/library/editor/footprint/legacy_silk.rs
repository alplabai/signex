//! Telling apart the silk an older build baked from hand-drawn silk, once,
//! when a footprint file opens into an editor (#1718). It has to run before
//! the first sketch edit: the classifier solves the sketch as loaded.
//! See `docs/FOOTPRINT_SKETCH.md`, "Bake ownership".

use std::path::Path;

use signex_bake::LegacySilk;
use signex_library::FootprintFile;
use signex_sketch::solver::Solver;

/// Mark the legacy baked silk of every footprint in `file`. The markers are
/// copied into `baseline` too: they are not an edit, so they neither dirty
/// the document nor make a save treat the footprint as changed. A footprint
/// whose sketch does not solve keeps all its silk unmarked; `log` says so
/// in the Messages panel.
pub(crate) fn recover_legacy_silk(
    path: &Path,
    file: &mut FootprintFile,
    baseline: &mut FootprintFile,
    log: bool,
) {
    let solver = Solver::default();
    for footprint in &mut file.footprints {
        match signex_bake::recover_legacy_silk(footprint, &solver) {
            LegacySilk::NotNeeded | LegacySilk::Marked(0) => {}
            LegacySilk::Marked(_) => {
                if let Some(loaded) = baseline.get_footprint_mut(footprint.uuid) {
                    loaded.silk_f.clone_from(&footprint.silk_f);
                    loaded.silk_b.clone_from(&footprint.silk_b);
                }
            }
            LegacySilk::Unsolved(error) => {
                if log {
                    crate::diagnostics::log_warning(format!(
                        "{}: footprint {:?}: the sketch does not solve ({error}), so its silk \
                         from an older build cannot be told from hand-drawn silk; the next \
                         sketch edit may duplicate that silk",
                        path.display(),
                        footprint.name,
                    ));
                }
            }
        }
    }
}
