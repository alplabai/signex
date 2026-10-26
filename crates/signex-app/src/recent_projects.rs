//! File ▸ Open Recent — the ordered, capped, de-duplicated list of
//! projects the user opened last.
//!
//! Pure list logic only. Persistence lives next to the other preference
//! readers/writers (`crate::fonts::read_recent_projects` /
//! `write_recent_projects`), and the menu reads [`RecentProjects::entries`].

use std::path::{Path, PathBuf};

/// How many projects File ▸ Open Recent remembers.
pub const MAX_RECENT_PROJECTS: usize = 10;

/// Most-recent-first list of absolute project paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecentProjects {
    entries: Vec<PathBuf>,
}

/// Canonical spelling of `path`, used both to store and to compare.
///
/// A path that cannot be canonicalised (the project vanished, or the
/// volume is offline) keeps its given spelling so it can still be listed
/// and later dropped by [`RecentProjects::remove`].
fn canonical(path: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    strip_verbatim_prefix(resolved)
}

/// `std::fs::canonicalize` returns `\\?\C:\…` on Windows; strip the
/// verbatim prefix so the label and the native dialogs show the path the
/// user knows.
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    match path.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => path,
    }
}

impl RecentProjects {
    /// Build a list from persisted paths, applying the same canonical
    /// de-duplication and cap as [`Self::record`] so a hand-edited
    /// `prefs.json` cannot exceed either.
    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut entries: Vec<PathBuf> = Vec::new();
        for path in paths {
            let path = canonical(&path);
            if !entries.contains(&path) {
                entries.push(path);
            }
        }
        entries.truncate(MAX_RECENT_PROJECTS);
        Self { entries }
    }

    /// Paths, most recent first.
    pub fn entries(&self) -> &[PathBuf] {
        &self.entries
    }

    /// Remember `path` as the most recent project. A path already listed
    /// (under any spelling that canonicalises the same) moves to the front
    /// instead of appearing twice; the oldest entry falls off past the cap.
    pub fn record(&mut self, path: &Path) {
        let path = canonical(path);
        self.entries.retain(|entry| *entry != path);
        self.entries.insert(0, path);
        self.entries.truncate(MAX_RECENT_PROJECTS);
    }

    /// Drop `path` (as spelled in the list). Returns whether it was there.
    pub fn remove(&mut self, path: &Path) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry != path);
        self.entries.len() != before
    }

    /// `from` was renamed to `to` (which now exists): replace its entry in
    /// place, keeping its position. Returns whether `from` was listed.
    /// Called after the move, so `from` is matched by its folder's
    /// canonical spelling plus its file name.
    pub fn retarget(&mut self, from: &Path, to: &Path) -> bool {
        let key = match (from.parent(), from.file_name()) {
            (Some(dir), Some(name)) => canonical(dir).join(name),
            _ => from.to_path_buf(),
        };
        let Some(slot) = self
            .entries
            .iter()
            .position(|entry| *entry == key || entry == from)
        else {
            return false;
        };
        let to = canonical(to);
        self.entries[slot] = to.clone();
        let mut seen = false;
        self.entries.retain(|entry| {
            if *entry != to {
                return true;
            }
            !std::mem::replace(&mut seen, true)
        });
        true
    }

    /// Forget every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_in(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"").expect("create project file");
        path
    }

    #[test]
    fn retarget_follows_a_rename_in_place() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = project_in(dir.path(), "a.snxprj");
        let b = project_in(dir.path(), "b.snxprj");
        let mut recent = RecentProjects::default();
        recent.record(&a);
        recent.record(&b);
        let renamed = dir.path().join("c.snxprj");
        std::fs::rename(&a, &renamed).unwrap();
        assert!(recent.retarget(&a, &renamed));
        assert_eq!(recent.entries()[1], canonical(&renamed));
        assert_eq!(recent.entries().len(), 2);
        assert!(!recent.retarget(&a, &renamed), "a is no longer listed");
    }

    #[test]
    fn record_puts_the_newest_first() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = project_in(dir.path(), "a.snxprj");
        let b = project_in(dir.path(), "b.snxprj");
        let mut recent = RecentProjects::default();
        recent.record(&a);
        recent.record(&b);
        assert_eq!(recent.entries(), [canonical(&b), canonical(&a)]);
    }

    #[test]
    fn record_caps_the_list_and_drops_the_oldest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut recent = RecentProjects::default();
        let mut first = None;
        for i in 0..MAX_RECENT_PROJECTS + 3 {
            let path = project_in(dir.path(), &format!("p{i}.snxprj"));
            first.get_or_insert_with(|| canonical(&path));
            recent.record(&path);
        }
        assert_eq!(recent.entries().len(), MAX_RECENT_PROJECTS);
        assert!(!recent.entries().contains(&first.expect("first path")));
    }

    #[test]
    fn the_same_project_under_two_spellings_is_listed_once() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).expect("mkdir");
        let path = project_in(dir.path(), "a.snxprj");
        let roundabout = sub.join("..").join("a.snxprj");
        let mut recent = RecentProjects::default();
        recent.record(&path);
        recent.record(&roundabout);
        assert_eq!(recent.entries().len(), 1);
    }

    #[test]
    fn re_recording_moves_an_entry_to_the_front() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = project_in(dir.path(), "a.snxprj");
        let b = project_in(dir.path(), "b.snxprj");
        let mut recent = RecentProjects::default();
        recent.record(&a);
        recent.record(&b);
        recent.record(&a);
        assert_eq!(recent.entries(), [canonical(&a), canonical(&b)]);
    }

    #[test]
    fn from_paths_dedupes_and_caps_a_hand_edited_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = project_in(dir.path(), "a.snxprj");
        let many = std::iter::repeat_n(a, 3).chain(
            (0..MAX_RECENT_PROJECTS + 5).map(|i| dir.path().join(format!("gone{i}.snxprj"))),
        );
        let recent = RecentProjects::from_paths(many);
        assert_eq!(recent.entries().len(), MAX_RECENT_PROJECTS);
        assert_eq!(
            recent
                .entries()
                .iter()
                .filter(|p| p.ends_with("a.snxprj"))
                .count(),
            1
        );
    }

    #[test]
    fn remove_and_clear_forget_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = project_in(dir.path(), "a.snxprj");
        let mut recent = RecentProjects::default();
        recent.record(&a);
        assert!(recent.remove(&canonical(&a)));
        assert!(!recent.remove(&canonical(&a)));
        recent.record(&a);
        recent.clear();
        assert!(recent.entries().is_empty());
    }
}
