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

#[derive(Debug, Default)]
pub struct NavigatorJob {
    wanted: Option<NavigatorRequest>,
    /// Generation and fingerprint of the running assembly.
    in_flight: Option<(u64, String)>,
    /// Fingerprint whose worker died; not retried until the inputs change.
    failed: Option<String>,
    generation: u64,
}

impl NavigatorJob {
    /// Record what the Navigator needs now; `None` when it needs nothing.
    pub fn want(&mut self, request: Option<NavigatorRequest>) {
        self.wanted = request;
    }

    /// Claim an assembly to start, unless the wanted one is already on screen
    /// (`applied`), running, or failed. A newer request supersedes a running
    /// one: bumping the generation makes the older answer drop on arrival.
    pub fn begin(&mut self, applied: &str) -> Option<(u64, NavigatorRequest)> {
        let wanted = self.wanted.as_ref()?;
        let fp = wanted.fingerprint.as_str();
        if fp == applied
            || self.failed.as_deref() == Some(fp)
            || self.in_flight.as_ref().is_some_and(|(_, f)| f == fp)
        {
            return None;
        }
        self.generation += 1;
        self.in_flight = Some((self.generation, fp.to_owned()));
        Some((self.generation, wanted.clone()))
    }

    /// The generation of the latest assembly started.
    #[cfg(test)]
    pub fn latest_generation(&self) -> u64 {
        self.generation
    }

    /// An assembly ended. `true` when it is the latest and should be applied;
    /// a superseded one returns `false`. `ok == false` remembers the failure.
    pub fn finish(&mut self, generation: u64, ok: bool) -> bool {
        match self.in_flight.take() {
            Some((g, fp)) if g == generation => {
                self.failed = (!ok).then_some(fp);
                true
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
        assert!(job.finish(g, true));
        assert!(job.begin("a").is_none(), "already on screen");
    }

    #[test]
    fn a_superseded_result_is_dropped() {
        let mut job = NavigatorJob::default();
        job.want(Some(req("a")));
        let (old, _) = job.begin("").unwrap();
        job.want(Some(req("b")));
        let (new, _) = job.begin("").unwrap();
        assert!(!job.finish(old, true), "stale generation dropped");
        assert!(job.finish(new, true), "the latest still applies");
    }

    #[test]
    fn a_failed_fingerprint_is_not_retried_until_inputs_change() {
        let mut job = NavigatorJob::default();
        job.want(Some(req("a")));
        let (g, _) = job.begin("").unwrap();
        assert!(job.finish(g, false));
        assert!(job.begin("").is_none());
        job.want(Some(req("b")));
        assert!(job.begin("").is_some());
    }

    #[test]
    fn nothing_wanted_starts_nothing() {
        let mut job = NavigatorJob::default();
        assert!(job.begin("").is_none());
    }
}
