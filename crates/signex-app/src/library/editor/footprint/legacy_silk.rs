//! Telling apart the silk an older build baked from hand-drawn silk, once,
//! when a footprint file opens into an editor (#1718). It has to run before
//! the first sketch edit: the classifier solves the sketch as loaded.
//! See `docs/FOOTPRINT_SKETCH.md`, "Bake ownership".

use std::path::Path;

use signex_bake::LegacySilk;
use signex_library::FootprintFile;
use signex_sketch::solver::Solver;

/// Mark the legacy baked silk of every footprint in `file`. The markers are
/// copied into `baseline` too (`baseline` is a clone of `file`, footprint
/// for footprint): they are not an edit, so they neither dirty the document
/// nor make a save treat the footprint as changed. When the silk cannot be
/// told apart for certain, `log` says so in the Messages panel.
pub(crate) fn recover_legacy_silk(
    path: &Path,
    file: &mut FootprintFile,
    baseline: &mut FootprintFile,
    log: bool,
) {
    let solver = Solver::default();
    for (idx, footprint) in file.footprints.iter_mut().enumerate() {
        let outcome = signex_bake::recover_legacy_silk(footprint, &solver);
        if let LegacySilk::Marked { marked, .. } = outcome
            && marked > 0
            && let Some(loaded) = baseline.footprints.get_mut(idx)
        {
            debug_assert_eq!(loaded.uuid, footprint.uuid, "baseline is a clone");
            loaded.silk_f.clone_from(&footprint.silk_f);
            loaded.silk_b.clone_from(&footprint.silk_b);
        }
        let why = match outcome {
            LegacySilk::Unsolved(error) => format!("the sketch does not solve ({error})"),
            LegacySilk::Marked { partial: true, .. } => {
                "some of its silk matches nothing the sketch bakes now".to_string()
            }
            LegacySilk::NotNeeded | LegacySilk::Marked { .. } => continue,
        };
        if log {
            crate::diagnostics::log_warning(format!(
                "{}: footprint {:?}: {why}, so silk from an older build cannot all be \
                 told from hand-drawn silk; the next sketch edit may duplicate that silk",
                path.display(),
                footprint.name,
            ));
        }
    }
}
