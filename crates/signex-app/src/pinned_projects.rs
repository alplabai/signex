//! Start screen ▸ Pinned Projects (#2305): the projects the user pinned, in
//! the order they pinned them.
//!
//! Pure list logic, like [`crate::recent_projects`]. Persistence lives next
//! to the other preference readers/writers
//! (`crate::fonts::read_pinned_projects_from` / `write_pinned_projects`).

use std::path::{Path, PathBuf};

/// Pinned project paths in the user's order, and which of them were last
/// found missing on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PinnedProjects {
    entries: Vec<PathBuf>,
    missing: Vec<PathBuf>,
}

impl PinnedProjects {
    /// An empty list.
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            missing: Vec::new(),
        }
    }

    /// Build the list from persisted paths at boot, as saved (the saved
    /// spelling is the recent list's), de-duplicated, order kept. Nothing is
    /// read from disk: which pins are missing comes later, from
    /// [`missing_files`] run off the UI thread.
    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut entries: Vec<PathBuf> = Vec::new();
        for path in paths {
            if !entries.contains(&path) {
                entries.push(path);
            }
        }
        Self {
            entries,
            missing: Vec::new(),
        }
    }

    /// Paths, in the order they were pinned.
    pub fn entries(&self) -> &[PathBuf] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.entries.iter().any(|entry| entry == path)
    }

    /// Whether `path` was last found missing on disk.
    pub fn is_missing(&self, path: &Path) -> bool {
        self.missing.iter().any(|entry| entry == path)
    }

    /// Pin `path` after the others, stored exactly as given: the Start
    /// screen passes a Recent Projects entry, already in the recent list's
    /// spelling, so pinning reads nothing from disk. Returns whether the
    /// list changed.
    pub fn pin(&mut self, path: &Path) -> bool {
        if self.contains(path) {
            return false;
        }
        self.entries.push(path.to_path_buf());
        true
    }

    /// Unpin `path`. Returns whether it was pinned.
    pub fn unpin(&mut self, path: &Path) -> bool {
        self.missing.retain(|entry| entry != path);
        let before = self.entries.len();
        self.entries.retain(|entry| entry != path);
        self.entries.len() != before
    }

    /// Record whether the pinned `path` is missing on disk. An unpinned
    /// path is ignored.
    pub fn set_missing(&mut self, path: &Path, missing: bool) {
        self.missing.retain(|entry| entry != path);
        if missing && self.contains(path) {
            self.missing.push(path.to_path_buf());
        }
    }

    /// Point the pinned `old` at `new`, in place, after a project was
    /// renamed or moved; `new` is spelled by the caller as the recent list
    /// spells it. A duplicate is dropped. Returns whether the list changed.
    pub fn replace(&mut self, old: &Path, new: &Path) -> bool {
        let Some(at) = self.entries.iter().position(|entry| entry == old) else {
            return false;
        };
        if old == new {
            return false;
        }
        self.missing.retain(|entry| entry != old);
        if self.contains(new) {
            self.entries.remove(at);
        } else {
            self.entries[at] = new.to_path_buf();
        }
        true
    }
}

/// The `paths` that are not files on disk. Blocking: run it on a worker,
/// never in `update` or `view`.
pub fn missing_files(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().filter(|path| !path.is_file()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    #[test]
    fn pin_keeps_the_users_order_and_ignores_a_second_pin() {
        let mut pinned = PinnedProjects::default();
        assert!(pinned.pin(&p("b.snxprj")));
        assert!(pinned.pin(&p("a.snxprj")));
        assert!(!pinned.pin(&p("b.snxprj")));
        assert_eq!(pinned.entries(), [p("b.snxprj"), p("a.snxprj")]);
    }

    #[test]
    fn unpin_drops_the_entry_and_its_missing_mark() {
        let mut pinned = PinnedProjects::default();
        pinned.pin(&p("a.snxprj"));
        pinned.pin(&p("b.snxprj"));
        pinned.set_missing(&p("a.snxprj"), true);
        assert!(pinned.unpin(&p("a.snxprj")));
        assert!(!pinned.unpin(&p("a.snxprj")));
        assert_eq!(pinned.entries(), [p("b.snxprj")]);
        assert!(!pinned.is_missing(&p("a.snxprj")));
    }

    #[test]
    fn only_a_pinned_path_can_be_marked_missing() {
        let mut pinned = PinnedProjects::default();
        pinned.set_missing(&p("a.snxprj"), true);
        assert!(!pinned.is_missing(&p("a.snxprj")));
        pinned.pin(&p("a.snxprj"));
        pinned.set_missing(&p("a.snxprj"), true);
        assert!(pinned.is_missing(&p("a.snxprj")));
        pinned.set_missing(&p("a.snxprj"), false);
        assert!(!pinned.is_missing(&p("a.snxprj")));
    }

    #[test]
    fn from_paths_dedupes_keeps_order_and_marks_nothing() {
        let pinned =
            PinnedProjects::from_paths([p("gone.snxprj"), p("here.snxprj"), p("gone.snxprj")]);
        assert_eq!(pinned.entries(), [p("gone.snxprj"), p("here.snxprj")]);
        assert!(!pinned.is_missing(&p("gone.snxprj")));
    }

    #[test]
    fn missing_files_names_only_the_paths_not_on_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let here = dir.path().join("here.snxprj");
        std::fs::write(&here, b"").expect("create project");
        let gone = dir.path().join("gone.snxprj");
        assert_eq!(missing_files(vec![gone.clone(), here]), [gone]);
    }

    #[test]
    fn replace_follows_a_rename_in_place() {
        let mut pinned = PinnedProjects::default();
        for name in ["a.snxprj", "b.snxprj", "c.snxprj"] {
            pinned.pin(&p(name));
        }
        assert!(pinned.replace(&p("b.snxprj"), &p("renamed.snxprj")));
        assert_eq!(
            pinned.entries(),
            [p("a.snxprj"), p("renamed.snxprj"), p("c.snxprj")]
        );
        assert!(pinned.replace(&p("a.snxprj"), &p("c.snxprj")));
        assert_eq!(pinned.entries(), [p("renamed.snxprj"), p("c.snxprj")]);
        assert!(!pinned.replace(&p("nope.snxprj"), &p("x.snxprj")));
    }
}
