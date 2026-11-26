//! The last few runs of the on-demand checks (#2199, decision
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6798772157),
//! kept in memory so a later run can be compared with one of them (its
//! `baseline`). Only the finding keys are kept. See `docs/CLI.md`, "Checks
//! on demand".

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// How many runs are kept; an older one is forgotten.
pub(crate) const KEPT_CHECK_RUNS: usize = 8;

/// One kept run.
#[derive(Debug)]
struct KeptRun {
    run: u64,
    check: String,
    /// What it checked: the project file, or a loose sheet.
    scope: PathBuf,
    keys: Vec<String>,
}

#[derive(Debug, Default)]
pub struct CheckRuns {
    /// The id of the newest run; run ids start at 1.
    last: u64,
    runs: VecDeque<KeptRun>,
}

impl CheckRuns {
    /// Keep a new run of `check` over `scope` and return its id.
    pub(crate) fn record(&mut self, check: &str, scope: &Path, keys: Vec<String>) -> u64 {
        self.last += 1;
        if self.runs.len() == KEPT_CHECK_RUNS {
            self.runs.pop_front();
        }
        self.runs.push_back(KeptRun {
            run: self.last,
            check: check.to_string(),
            scope: scope.to_path_buf(),
            keys,
        });
        self.last
    }

    /// The finding keys of run `run`, when it is kept and was a run of the
    /// same check over the same scope; the refusal otherwise.
    pub(crate) fn keys_of(&self, run: u64, check: &str, scope: &Path) -> Result<&[String], String> {
        let Some(kept) = self.runs.iter().find(|kept| kept.run == run) else {
            let why = if run == 0 || run > self.last {
                "there is no such run"
            } else {
                "it is no longer kept"
            };
            return Err(format!(
                "baseline run {run} cannot be compared with: {why} (the last {KEPT_CHECK_RUNS} runs are kept)"
            ));
        };
        if kept.check != check || kept.scope != scope {
            return Err(format!(
                "baseline run {run} was a `{}` check of {}, not this one",
                kept.check,
                kept.scope.display()
            ));
        }
        Ok(&kept.keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn runs_are_numbered_from_one_and_keep_their_keys() {
        let mut runs = CheckRuns::default();
        let scope = Path::new("/p/demo.snxprj");
        assert_eq!(runs.record("erc", scope, keys(&["a"])), 1);
        assert_eq!(runs.record("erc", scope, keys(&["b"])), 2);
        assert_eq!(runs.keys_of(1, "erc", scope).unwrap(), ["a"]);
        assert_eq!(runs.keys_of(2, "erc", scope).unwrap(), ["b"]);
    }

    #[test]
    fn an_old_run_is_forgotten_and_refused_as_no_longer_kept() {
        let mut runs = CheckRuns::default();
        let scope = Path::new("/p/demo.snxprj");
        for _ in 0..=KEPT_CHECK_RUNS {
            runs.record("erc", scope, Vec::new());
        }
        let why = runs.keys_of(1, "erc", scope).unwrap_err();
        assert!(why.contains("no longer kept"), "{why}");
        assert!(runs.keys_of(2, "erc", scope).is_ok());
    }

    #[test]
    fn an_unknown_run_is_refused() {
        let mut runs = CheckRuns::default();
        let scope = Path::new("/p/demo.snxprj");
        runs.record("erc", scope, Vec::new());
        for run in [0, 2, 99] {
            let why = runs.keys_of(run, "erc", scope).unwrap_err();
            assert!(why.contains("no such run"), "{run}: {why}");
        }
    }

    #[test]
    fn a_run_of_another_project_or_check_is_refused() {
        let mut runs = CheckRuns::default();
        runs.record("erc", Path::new("/p/a.snxprj"), Vec::new());
        let why = runs
            .keys_of(1, "erc", Path::new("/p/b.snxprj"))
            .unwrap_err();
        assert!(why.contains("/p/a.snxprj"), "{why}");
        assert!(runs.keys_of(1, "drc", Path::new("/p/a.snxprj")).is_err());
    }
}
