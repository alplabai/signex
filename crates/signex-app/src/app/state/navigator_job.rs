//! Bookkeeping for the off-thread Navigator hierarchy assembly (#1470).
//!
//! `refresh_panel_ctx` must not touch the disk, and assembling the project's
//! sheets reads the closed ones. So the refresh only records what it wants
//! ([`NavigatorJob::want`]); `Signex::update` starts the assembly
//! ([`NavigatorJob::begin`]) and the answer is applied only while it is still
//! the latest ([`NavigatorJob::finish`]). Until it lands, the previous
//! hierarchy stays on screen.

use std::path::PathBuf;

use crate::app::sheet_hierarchy::SheetHierarchy;
use crate::app::state::scope::CaseFold;

/// What the assembly is built from; `fingerprint` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigatorRequest {
    pub fingerprint: String,
    pub root: PathBuf,
    pub pages: Vec<PathBuf>,
    pub fold: CaseFold,
}

/// A finished assembly, ready to swap into the Navigator model.
#[derive(Debug, Clone)]
pub struct NavigatorAssembly {
    pub fingerprint: String,
    pub hierarchy: SheetHierarchy,
    /// Sheets that exist but could not be read, with why.
    pub unreadable: Vec<(PathBuf, String)>,
}

/// How many times a failing assembly is tried for one fingerprint. `update`
/// asks again after every message, so an unbounded retry of a build that
/// always fails would spin.
const MAX_ATTEMPTS: u8 = 3;

#[derive(Debug, Default)]
pub struct NavigatorJob {
    wanted: Option<NavigatorRequest>,
    /// Generation and fingerprint of the running assembly.
    in_flight: Option<(u64, String)>,
    /// Fingerprint whose worker died, and how many times in a row; retried
    /// up to [`MAX_ATTEMPTS`], then left until the inputs change.
    failed: Option<(String, u8)>,
    generation: u64,
}

impl NavigatorJob {
    /// Record what the Navigator needs now; `None` when it needs nothing.
    pub fn want(&mut self, request: Option<NavigatorRequest>) {
        self.wanted = request;
    }

    /// Claim an assembly to start. Nothing starts when the wanted one is
    /// already on screen (`applied`), has failed, or another is still
    /// running (the latest wanted starts when that one lands). A wanted state
    /// that is already on screen, or gone, abandons a running assembly: its
    /// answer would install a hierarchy the Navigator no longer wants.
    pub fn begin(&mut self, applied: &str) -> Option<(u64, NavigatorRequest)> {
        let Some(wanted) = self.wanted.as_ref() else {
            self.in_flight = None;
            return None;
        };
        if wanted.fingerprint == applied {
            self.in_flight = None;
            return None;
        }
        let exhausted = self
            .failed
            .as_ref()
            .is_some_and(|(fp, attempts)| *fp == wanted.fingerprint && *attempts >= MAX_ATTEMPTS);
        if exhausted || self.in_flight.is_some() {
            return None;
        }
        self.generation += 1;
        self.in_flight = Some((self.generation, wanted.fingerprint.clone()));
        Some((self.generation, wanted.clone()))
    }

    /// Forget the running assembly, so the next [`begin`](Self::begin) starts
    /// afresh. For tests that run the work inline after `update` already
    /// started a Task they never deliver; its late answer would be dropped.
    #[doc(hidden)]
    pub fn abandon_running(&mut self) {
        self.in_flight = None;
    }

    /// The fingerprint the Navigator currently wants, if any.
    pub fn wanted_fingerprint(&self) -> Option<&str> {
        self.wanted.as_ref().map(|w| w.fingerprint.as_str())
    }

    /// True when the hierarchy on screen (`applied`) is the wanted one and
    /// nothing is being rebuilt. Until then it may describe an earlier state
    /// of the project, so callers that WRITE from it must not trust it.
    pub fn is_settled(&self, applied: &str) -> bool {
        self.in_flight.is_none() && self.wanted_fingerprint().is_none_or(|fp| fp == applied)
    }

    /// The generation of the latest assembly started.
    #[cfg(test)]
    pub fn latest_generation(&self) -> u64 {
        self.generation
    }

    /// An assembly ended. `true` when it is the running one and still wanted
    /// (`fingerprint` is what it was built for), so it should be applied; an
    /// abandoned or superseded one returns `false`. `ok == false` remembers
    /// the failure.
    pub fn finish(&mut self, generation: u64, fingerprint: Option<&str>, ok: bool) -> bool {
        match self.in_flight.take() {
            Some((g, fp)) if g == generation => {
                self.failed = (!ok).then(|| match self.failed.take() {
                    Some((failed, attempts)) if failed == fp => (fp, attempts.saturating_add(1)),
                    _ => (fp, 1),
                });
                ok && fingerprint.is_none_or(|f| Some(f) == self.wanted_fingerprint())
            }
            other => {
                self.in_flight = other;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(fp: &str) -> NavigatorRequest {
        NavigatorRequest {
            fingerprint: fp.into(),
            root: PathBuf::from("r.snxsch"),
            pages: Vec::new(),
            fold: CaseFold::host_default(),
        }
    }

    #[test]
    fn a_result_is_applied_once_and_not_restarted() {
        let mut job = NavigatorJob::default();
        job.want(Some(req("a")));
        let (g, _) = job.begin("").unwrap();
        assert!(job.begin("").is_none(), "already running");
        assert!(!job.is_settled(""));
        assert!(job.finish(g, Some("a"), true));
        assert!(job.begin("a").is_none(), "already on screen");
        assert!(job.is_settled("a"));
    }

    #[test]
    fn a_result_for_a_superseded_request_is_dropped_and_the_latest_starts_next() {
        let mut job = NavigatorJob::default();
        job.want(Some(req("a")));
        let (old, _) = job.begin("").unwrap();
        job.want(Some(req("b")));
        assert!(job.begin("").is_none(), "one at a time");
        assert!(!job.finish(old, Some("a"), true), "stale answer dropped");
        let (new, _) = job.begin("").unwrap();
        assert!(job.finish(new, Some("b"), true));
    }

    #[test]
    fn going_back_to_the_applied_state_abandons_the_running_assembly() {
        let mut job = NavigatorJob::default();
        job.want(Some(req("a")));
        job.begin("").unwrap();
        // The user undid back to what is already on screen ("") ...
        job.want(Some(req("")));
        assert!(job.begin("").is_none());
        // ... so the answer for "a" must not land.
        assert!(job.is_settled(""));
        assert!(!job.finish(1, Some("a"), true));
    }

    #[test]
    fn a_project_closed_during_assembly_drops_the_answer() {
        let mut job = NavigatorJob::default();
        job.want(Some(req("a")));
        let (g, _) = job.begin("").unwrap();
        job.want(None);
        assert!(!job.finish(g, Some("a"), true));
        assert!(job.begin("").is_none());
    }

    #[test]
    fn a_failed_fingerprint_is_retried_a_few_times_then_left_until_inputs_change() {
        let mut job = NavigatorJob::default();
        job.want(Some(req("a")));
        for attempt in 1..=3 {
            let (g, _) = job.begin("").expect("retried while attempts remain");
            assert!(!job.finish(g, None, false), "attempt {attempt} failed");
        }
        assert!(job.begin("").is_none(), "gives up rather than spinning");
        job.want(Some(req("b")));
        assert!(job.begin("").is_some());
    }

    #[test]
    fn nothing_wanted_starts_nothing() {
        let mut job = NavigatorJob::default();
        assert!(job.begin("").is_none());
        assert!(job.is_settled("anything"));
    }
}
