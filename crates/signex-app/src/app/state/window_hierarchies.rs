//! Sheet hierarchies of the projects that undocked windows show (#2021).
//!
//! The Navigator describes the focused document's project. An undocked
//! window can show a sheet of another project, whose placements the
//! Navigator's hierarchy does not hold. This keeps one hierarchy per such
//! project, assembled off-thread by the same job the Navigator uses.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::navigator_job::{NavigatorAssembly, NavigatorJob, NavigatorRequest};
use crate::app::sheet_hierarchy::SheetHierarchy;

#[derive(Debug, Default)]
struct Entry {
    job: NavigatorJob,
    applied: Option<(String, SheetHierarchy)>,
}

impl Entry {
    fn applied_fingerprint(&self) -> &str {
        self.applied.as_ref().map_or("", |(fp, _)| fp.as_str())
    }
}

/// Each entry's generations start this far from the last entry's, so a late
/// result of a dropped job never matches a job made afterwards.
const GENERATION_STRIDE: u64 = 1 << 32;

/// Hierarchies by the project's root sheet path.
#[derive(Debug, Default)]
pub struct WindowHierarchies {
    entries: HashMap<PathBuf, Entry>,
    created: u64,
}

impl WindowHierarchies {
    /// Record what the windows need now. `held` are projects the Navigator
    /// holds or is about to: their hierarchy is kept but not rebuilt here. A
    /// project in neither list is dropped.
    pub fn want(&mut self, requests: Vec<NavigatorRequest>, held: &[PathBuf]) {
        self.entries
            .retain(|root, _| held.contains(root) || requests.iter().any(|r| &r.root == root));
        for entry in self.entries.values_mut() {
            entry.job.want(None);
        }
        for request in requests {
            let created = &mut self.created;
            self.entries
                .entry(request.root.clone())
                .or_insert_with(|| {
                    *created += 1;
                    Entry {
                        job: NavigatorJob::starting_at(*created * GENERATION_STRIDE),
                        applied: None,
                    }
                })
                .job
                .want(Some(request));
        }
    }

    /// The assemblies to start now, one per project that needs one.
    pub fn begin_all(&mut self) -> Vec<(u64, NavigatorRequest)> {
        self.entries
            .values_mut()
            .filter_map(|entry| {
                let applied = entry.applied_fingerprint().to_owned();
                entry.job.begin(&applied)
            })
            .collect()
    }

    /// An assembly for `root` ended. Whether it was applied.
    pub fn finish(
        &mut self,
        root: &Path,
        generation: u64,
        assembly: Result<Box<NavigatorAssembly>, String>,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(root) else {
            return false;
        };
        let fingerprint = assembly.as_ref().ok().map(|a| a.fingerprint.clone());
        let current = entry
            .job
            .finish(generation, fingerprint.as_deref(), assembly.is_ok());
        match assembly {
            Ok(assembly) if current => {
                entry.applied = Some((assembly.fingerprint, assembly.hierarchy));
                true
            }
            _ => false,
        }
    }

    /// The hierarchy of the project whose root sheet is `root`, if one has
    /// landed.
    pub fn hierarchy_of(&self, root: &Path) -> Option<&SheetHierarchy> {
        self.entries.get(root)?.applied.as_ref().map(|(_, h)| h)
    }

    /// Whether the assembly for `root` has failed for good; `None` when no
    /// window wants that project.
    pub fn is_exhausted(&self, root: &Path) -> Option<bool> {
        let entry = self.entries.get(root)?;
        entry.job.wanted_root()?;
        Some(entry.job.is_exhausted())
    }

    /// Whether the hierarchy for `root` is the current one and nothing is
    /// rebuilding it. An entry no window wants (kept only while the Navigator
    /// takes the project over) may be out of date, so it is never settled.
    pub fn is_settled(&self, root: &Path) -> bool {
        self.entries.get(root).is_some_and(|entry| {
            entry.job.wanted_root().is_some() && entry.job.is_settled(entry.applied_fingerprint())
        })
    }

    /// Forget a running assembly, for tests that run the work inline.
    #[doc(hidden)]
    pub fn abandon_running(&mut self) {
        self.entries
            .values_mut()
            .for_each(|e| e.job.abandon_running());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::scope::CaseFold;

    fn request(root: &Path, fingerprint: &str) -> NavigatorRequest {
        NavigatorRequest {
            fingerprint: fingerprint.into(),
            root: root.to_path_buf(),
            pages: Vec::new(),
            fold: CaseFold::host_default(),
        }
    }

    fn assembly(fingerprint: &str) -> Result<Box<NavigatorAssembly>, String> {
        Ok(Box::new(NavigatorAssembly {
            fingerprint: fingerprint.into(),
            hierarchy: SheetHierarchy::default(),
            unreadable: Vec::new(),
        }))
    }

    #[test]
    fn a_late_answer_of_a_dropped_job_does_not_match_the_job_made_after_it() {
        let root = PathBuf::from("r.snxsch");
        let mut hierarchies = WindowHierarchies::default();
        hierarchies.want(vec![request(&root, "a")], &[]);
        let (first, _) = hierarchies.begin_all().remove(0);
        hierarchies.want(Vec::new(), &[]);
        hierarchies.want(vec![request(&root, "a")], &[]);
        let (second, _) = hierarchies.begin_all().remove(0);
        assert_ne!(first, second);

        assert!(!hierarchies.finish(&root, first, assembly("a")));
        assert!(hierarchies.hierarchy_of(&root).is_none());
        assert!(
            hierarchies.finish(&root, second, assembly("a")),
            "the new job's run survived the late answer"
        );
    }

    #[test]
    fn a_hierarchy_the_navigator_took_over_is_kept_but_not_settled() {
        let root = PathBuf::from("r.snxsch");
        let mut hierarchies = WindowHierarchies::default();
        hierarchies.want(vec![request(&root, "a")], &[]);
        let (generation, _) = hierarchies.begin_all().remove(0);
        assert!(hierarchies.finish(&root, generation, assembly("a")));
        assert!(hierarchies.is_settled(&root));

        hierarchies.want(Vec::new(), std::slice::from_ref(&root));
        assert!(hierarchies.hierarchy_of(&root).is_some(), "kept");
        assert!(!hierarchies.is_settled(&root), "but possibly out of date");
    }
}
