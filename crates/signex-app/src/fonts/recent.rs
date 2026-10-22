//! Recent-projects preference IO (File ▸ Open Recent). Split from `fonts.rs`.

use super::*;

const RECENT_PROJECTS_KEY: &str = "recent_projects";

/// Read the persisted recent-projects paths from `path`, most recent
/// first. A missing, unreadable or malformed file — or a non-array /
/// non-string entry — degrades to fewer (or no) entries rather than an
/// error, like every other reader in this module.
pub(super) fn read_recent_projects_at(path: &Path) -> Vec<PathBuf> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Vec::new();
    };
    json.get(RECENT_PROJECTS_KEY)
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str())
                .filter(|entry| !entry.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Persist the recent-projects paths into `path` without clobbering the
/// other keys.
pub(super) fn write_recent_projects_at(path: &Path, projects: &[PathBuf]) {
    update_prefs_json(path, RECENT_PROJECTS_KEY, |prefs| {
        let entries = projects
            .iter()
            .map(|project| serde_json::Value::String(project.display().to_string()))
            .collect();
        prefs.insert(
            RECENT_PROJECTS_KEY.to_string(),
            serde_json::Value::Array(entries),
        );
    })
}

/// Recent projects from the user's `prefs.json`, most recent first.
pub fn read_recent_projects() -> Vec<PathBuf> {
    read_recent_projects_at(&prefs_path())
}

/// Persist the recent-projects list to the user's `prefs.json`.
pub fn write_recent_projects(projects: &[PathBuf]) {
    write_recent_projects_at(&prefs_path(), projects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_prefs_file_yields_an_empty_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(read_recent_projects_at(&dir.path().join("prefs.json")).is_empty());
    }

    #[test]
    fn corrupt_prefs_file_yields_an_empty_list_without_panicking() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, b"{ not json at all").expect("write");
        assert!(read_recent_projects_at(&path).is_empty());
    }

    #[test]
    fn wrong_shaped_entries_are_skipped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, br#"{"recent_projects": ["a.snxprj", 7, null, ""]}"#).expect("write");
        assert_eq!(read_recent_projects_at(&path), [PathBuf::from("a.snxprj")]);
        std::fs::write(&path, br#"{"recent_projects": "oops"}"#).expect("write");
        assert!(read_recent_projects_at(&path).is_empty());
    }

    #[test]
    fn write_round_trips_and_keeps_other_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, br#"{"ui_font": "Roboto"}"#).expect("write");
        let projects = [PathBuf::from("b.snxprj"), PathBuf::from("a.snxprj")];

        write_recent_projects_at(&path, &projects);

        assert_eq!(read_recent_projects_at(&path), projects);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
        assert_eq!(json["ui_font"], "Roboto");
    }
}
