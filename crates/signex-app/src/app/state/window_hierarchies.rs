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

/// Hierarchies by the project's root sheet path.
#[derive(Debug, Default)]
pub struct WindowHierarchies(HashMap<PathBuf, Entry>);

impl WindowHierarchies {
    /// Record what the windows need now; a project no window shows any more
    /// is dropped.
    pub fn want(&mut self, requests: Vec<NavigatorRequest>) {
        self.0
            .retain(|root, _| requests.iter().any(|r| &r.root == root));
        for request in requests {
            self.0
                .entry(request.root.clone())
                .or_default()
                .job
                .want(Some(request));
        }
    }

    /// The assemblies to start now, one per project that needs one.
    pub fn begin_all(&mut self) -> Vec<(u64, NavigatorRequest)> {
        self.0
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
        let Some(entry) = self.0.get_mut(root) else {
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
        self.0.get(root)?.applied.as_ref().map(|(_, h)| h)
    }

    /// Whether the hierarchy for `root` is the current one and nothing is
    /// rebuilding it.
    pub fn is_settled(&self, root: &Path) -> bool {
        self.0
            .get(root)
            .is_some_and(|entry| entry.job.is_settled(entry.applied_fingerprint()))
    }

    /// Forget a running assembly, for tests that run the work inline.
    #[doc(hidden)]
    pub fn abandon_running(&mut self) {
        self.0.values_mut().for_each(|e| e.job.abandon_running());
    }
}
