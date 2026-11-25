//! Recent packages (mockup row 193f, accepted 2026-11-24): app-wide, ten
//! kept, the empty state shows three, as File ▸ Recent keeps projects. A
//! package whose files are gone stays listed, greyed, with "Files not
//! found". Pure list logic; the existence check runs in a task.

use std::path::PathBuf;

/// How many packages are kept.
pub const MAX_RECENT_PACKAGES: usize = 10;
/// How many the empty state shows.
pub const SHOWN_RECENT_PACKAGES: usize = 3;

/// One package opened before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentPackage {
    /// What was opened: one folder, one zip, or the files.
    pub paths: Vec<PathBuf>,
    pub layers: usize,
    /// When it was opened, in Unix seconds.
    pub opened_at: i64,
    /// Its files were not found at the last check.
    pub missing: bool,
}

/// Most recent first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentPackages {
    entries: Vec<RecentPackage>,
}

impl RecentPackages {
    pub fn entries(&self) -> &[RecentPackage] {
        &self.entries
    }

    /// The rows the empty state shows.
    pub fn shown(&self) -> &[RecentPackage] {
        &self.entries[..self.entries.len().min(SHOWN_RECENT_PACKAGES)]
    }

    /// Remember `paths` as the package opened last: a package listed
    /// before moves to the front, the oldest falls off past the cap.
    pub fn record(&mut self, paths: Vec<PathBuf>, layers: usize, opened_at: i64) {
        self.entries.retain(|entry| entry.paths != paths);
        self.entries.insert(
            0,
            RecentPackage {
                paths,
                layers,
                opened_at,
                missing: false,
            },
        );
        self.entries.truncate(MAX_RECENT_PACKAGES);
    }

    /// Mark the package of `paths` as found or not.
    pub fn set_missing(&mut self, paths: &[PathBuf], missing: bool) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.paths == paths) {
            entry.missing = missing;
        }
    }

    /// Every listed package's paths, for the existence check.
    pub fn all_paths(&self) -> Vec<Vec<PathBuf>> {
        self.entries.iter().map(|e| e.paths.clone()).collect()
    }
}

/// Whether every path of a package is still there. Blocking: run it in a
/// task.
pub fn package_exists(paths: &[PathBuf]) -> bool {
    !paths.is_empty() && paths.iter().all(|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str) -> Vec<PathBuf> {
        vec![PathBuf::from(name)]
    }

    /// mockup row 193f, accepted answer: ten kept, three shown.
    #[test]
    fn ten_packages_are_kept_and_three_are_shown() {
        let mut recent = RecentPackages::default();
        for i in 0..12 {
            recent.record(p(&format!("pkg{i}.zip")), i, i as i64);
        }
        assert_eq!(recent.entries().len(), MAX_RECENT_PACKAGES);
        assert_eq!(MAX_RECENT_PACKAGES, 10);
        assert_eq!(recent.shown().len(), 3);
        assert_eq!(recent.shown()[0].paths, p("pkg11.zip"));
        assert!(recent.entries().iter().all(|e| e.paths != p("pkg0.zip")));
    }

    #[test]
    fn a_package_opened_again_moves_to_the_front_once() {
        let mut recent = RecentPackages::default();
        recent.record(p("a"), 4, 1);
        recent.record(p("b"), 2, 2);
        recent.record(p("a"), 5, 3);
        let listed: Vec<_> = recent.entries().iter().map(|e| e.paths.clone()).collect();
        assert_eq!(listed, vec![p("a"), p("b")]);
        assert_eq!(recent.entries()[0].layers, 5);
    }

    /// A moved package stays listed, marked, until it is found again.
    #[test]
    fn a_moved_package_stays_listed_as_not_found() {
        let mut recent = RecentPackages::default();
        recent.record(p("gone"), 3, 1);
        recent.set_missing(&p("gone"), true);
        assert!(recent.entries()[0].missing);
        assert_eq!(recent.entries().len(), 1);
        let dir = tempfile::tempdir().unwrap();
        assert!(package_exists(&[dir.path().to_path_buf()]));
        assert!(!package_exists(&[dir.path().join("nope.zip")]));
        assert!(!package_exists(&[]));
    }
}
