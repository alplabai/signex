//! The design-rule check of the board in view (#2222): the run in flight
//! and the last report, kept until a newer run replaces it.
//!
//! The check itself is `pcb_geom`'s; the run is a background task (see
//! `handlers/pcb_drc.rs`) and this holds only what its result needs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_drc::DrcReport;

/// A finished run: the report and what it was run on.
#[derive(Debug)]
pub struct DrcDone {
    /// The board document and its revision when the run began.
    pub path: PathBuf,
    pub revision: u64,
    pub report: DrcReport,
    /// Net names of that board, by number, for the rows.
    pub net_names: BTreeMap<u32, String>,
}

#[derive(Debug, Default)]
pub struct PcbDrc {
    generation: u64,
    /// The board and revision the run in flight began on.
    target: Option<(PathBuf, u64)>,
    running: bool,
    done: Option<DrcDone>,
    /// What the DRC results panel shows of the report (#2399): view state,
    /// never board data.
    pub filter: crate::panels::DrcFilter,
    /// The waiver note being written in the panel.
    pub waive_draft: Option<crate::panels::DrcWaiveDraft>,
}

impl PcbDrc {
    /// Start a run; the number to hand back to [`Self::finish`]. A run still
    /// in flight is superseded: its result will be refused.
    pub fn begin(&mut self, path: PathBuf, revision: u64) -> u64 {
        self.generation += 1;
        self.target = Some((path, revision));
        self.running = true;
        self.generation
    }

    /// Keep the result of run `generation`. False when a newer run was
    /// begun since, in which case nothing changes.
    pub fn finish(
        &mut self,
        generation: u64,
        report: DrcReport,
        net_names: BTreeMap<u32, String>,
    ) -> bool {
        let Some((path, revision)) = self
            .target
            .clone()
            .filter(|_| generation == self.generation)
        else {
            return false;
        };
        self.running = false;
        self.done = Some(DrcDone {
            path,
            revision,
            report,
            net_names,
        });
        true
    }

    /// The run `generation` could not produce a report; the last report
    /// stays. False when a newer run was begun since.
    pub fn abandon(&mut self, generation: u64) -> bool {
        let current = generation == self.generation;
        if current {
            self.running = false;
        }
        current
    }

    /// Whether a run on the board at `path` is in flight.
    pub fn is_running_for(&self, path: &Path) -> bool {
        self.running && self.target.as_ref().is_some_and(|(p, _)| p == path)
    }

    /// The last report, if it is for the board at `path`.
    pub fn done_for(&self, path: &Path) -> Option<&DrcDone> {
        self.done.as_ref().filter(|d| d.path == path)
    }

    /// The board at `path` went from revision `from` to `to` by an edit DRC
    /// does not read (a waiver): a report of revision `from` stays current.
    pub fn carry_revision(&mut self, path: &Path, from: u64, to: u64) {
        if let Some(done) = self
            .done
            .as_mut()
            .filter(|d| d.path == path && d.revision == from)
        {
            done.revision = to;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> DrcReport {
        DrcReport {
            rules: signex_drc::DrcRules::new(1),
            violations: Vec::new(),
            skipped: Vec::new(),
            skipped_keepouts: Vec::new(),
        }
    }

    fn run(drc: &mut PcbDrc, path: &str) -> bool {
        let generation = drc.begin(PathBuf::from(path), 1);
        drc.finish(generation, report(), BTreeMap::new())
    }

    #[test]
    fn a_superseded_run_is_refused() {
        let mut drc = PcbDrc::default();
        let first = drc.begin(PathBuf::from("a.snxpcb"), 1);
        let second = drc.begin(PathBuf::from("a.snxpcb"), 2);
        assert!(!drc.finish(first, report(), BTreeMap::new()));
        let a = Path::new("a.snxpcb");
        assert!(drc.is_running_for(a) && drc.done_for(a).is_none());
        assert!(!drc.is_running_for(Path::new("b.snxpcb")));
        assert!(drc.finish(second, report(), BTreeMap::new()));
        assert!(!drc.is_running_for(a));
        assert_eq!(
            drc.done_for(Path::new("a.snxpcb")).map(|d| d.revision),
            Some(2)
        );
    }

    #[test]
    fn the_report_belongs_to_its_board() {
        let mut drc = PcbDrc::default();
        assert!(run(&mut drc, "a.snxpcb"));
        assert!(drc.done_for(Path::new("a.snxpcb")).is_some());
        assert!(drc.done_for(Path::new("b.snxpcb")).is_none());
    }

    /// #2399: a waiver moves the board's revision; the report of the
    /// revision before it stays current, any other is left stale.
    #[test]
    fn a_waiver_keeps_the_report_current() {
        let mut drc = PcbDrc::default();
        assert!(run(&mut drc, "a.snxpcb"));
        let a = Path::new("a.snxpcb");
        drc.carry_revision(a, 1, 2);
        assert_eq!(drc.done_for(a).map(|d| d.revision), Some(2));
        drc.carry_revision(a, 5, 6);
        assert_eq!(drc.done_for(a).map(|d| d.revision), Some(2));
        drc.carry_revision(Path::new("b.snxpcb"), 2, 3);
        assert_eq!(drc.done_for(a).map(|d| d.revision), Some(2));
    }

    #[test]
    fn an_abandoned_run_keeps_the_last_report() {
        let mut drc = PcbDrc::default();
        assert!(run(&mut drc, "a.snxpcb"));
        let next = drc.begin(PathBuf::from("a.snxpcb"), 2);
        assert!(drc.abandon(next));
        assert!(!drc.is_running_for(Path::new("a.snxpcb")));
        assert!(drc.done_for(Path::new("a.snxpcb")).is_some());
        assert!(!drc.abandon(next - 1));
    }
}
