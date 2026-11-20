//! Recent- and pinned-projects preference IO (File ▸ Open Recent, the Start
//! screen's Pinned Projects). Split from `fonts.rs`.

use super::*;

const RECENT_PROJECTS_KEY: &str = "recent_projects";
const PINNED_PROJECTS_KEY: &str = "pinned_projects";
/// When each recent project was last opened: an object of path -> Unix
/// seconds beside `recent_projects`. Prefs written before it existed have
/// no key and read as no times.
const RECENT_OPENED_KEY: &str = "recent_projects_opened";

/// The persisted last-opened times of the recent projects (Unix seconds).
/// A missing or malformed value, or a non-integer entry, reads as fewer
/// (or no) times, never as an error.
pub fn read_recent_opened_from(json: Option<&serde_json::Value>) -> Vec<(PathBuf, i64)> {
    json.and_then(|json| json.get(RECENT_OPENED_KEY))
        .and_then(|value| value.as_object())
        .map(|times| {
            times
                .iter()
                .filter(|(path, _)| !path.is_empty())
                .filter_map(|(path, at)| Some((PathBuf::from(path), at.as_i64()?)))
                .collect()
        })
        .unwrap_or_default()
}

/// Persist the recent-projects paths and their last-opened times into
/// `path` in one write, without clobbering the other keys.
pub(super) fn write_recent_projects_and_times_at(
    path: &Path,
    projects: &[PathBuf],
    opened: &std::collections::BTreeMap<PathBuf, i64>,
) {
    update_prefs_json(path, RECENT_PROJECTS_KEY, |prefs| {
        let entries = projects
            .iter()
            .map(|project| serde_json::Value::String(project.display().to_string()))
            .collect();
        prefs.insert(
            RECENT_PROJECTS_KEY.to_string(),
            serde_json::Value::Array(entries),
        );
        let times = opened
            .iter()
            .map(|(project, at)| (project.display().to_string(), serde_json::Value::from(*at)))
            .collect();
        prefs.insert(
            RECENT_OPENED_KEY.to_string(),
            serde_json::Value::Object(times),
        );
    })
}

/// Persist the recent projects and their last-opened times to the user's
/// `prefs.json`.
pub fn write_recent_projects_and_times(
    projects: &[PathBuf],
    opened: &std::collections::BTreeMap<PathBuf, i64>,
) {
    write_recent_projects_and_times_at(&prefs_path(), projects, opened)
}

/// Read the persisted recent-projects paths from `path`, most recent
/// first. A missing, unreadable or malformed file — or a non-array /
/// non-string entry — degrades to fewer (or no) entries rather than an
/// error, like every other reader in this module.
pub(super) fn read_recent_projects_at(path: &Path) -> Vec<PathBuf> {
    read_recent_projects_from(read_prefs_json(path).as_ref())
}

pub fn read_recent_projects_from(json: Option<&serde_json::Value>) -> Vec<PathBuf> {
    read_paths_from(json, RECENT_PROJECTS_KEY)
}

/// The pinned projects in the user's order; prefs written before pinning
/// existed have no key and read as none.
pub fn read_pinned_projects_from(json: Option<&serde_json::Value>) -> Vec<PathBuf> {
    read_paths_from(json, PINNED_PROJECTS_KEY)
}

pub(super) fn read_pinned_projects_at(path: &Path) -> Vec<PathBuf> {
    read_pinned_projects_from(read_prefs_json(path).as_ref())
}

fn read_paths_from(json: Option<&serde_json::Value>, key: &str) -> Vec<PathBuf> {
    let Some(json) = json else {
        return Vec::new();
    };
    json.get(key)
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
/// other keys, and without times: the shape prefs written before the
/// last-opened times had. The app writes through
/// [`write_recent_projects_and_times`].
#[cfg(test)]
pub(super) fn write_recent_projects_at(path: &Path, projects: &[PathBuf]) {
    write_paths_at(path, RECENT_PROJECTS_KEY, projects)
}

/// Persist the pinned projects into `path`, in order, without clobbering
/// the other keys.
pub(super) fn write_pinned_projects_at(path: &Path, projects: &[PathBuf]) {
    write_paths_at(path, PINNED_PROJECTS_KEY, projects)
}

fn write_paths_at(path: &Path, key: &'static str, projects: &[PathBuf]) {
    update_prefs_json(path, key, |prefs| {
        let entries = projects
            .iter()
            .map(|project| serde_json::Value::String(project.display().to_string()))
            .collect();
        prefs.insert(key.to_string(), serde_json::Value::Array(entries));
    })
}

/// Pinned projects from the user's `prefs.json`, in the user's order.
pub fn read_pinned_projects() -> Vec<PathBuf> {
    read_pinned_projects_at(&prefs_path())
}

/// Persist the pinned-projects list to the user's `prefs.json`.
pub fn write_pinned_projects(projects: &[PathBuf]) {
    write_pinned_projects_at(&prefs_path(), projects)
}

/// Recent projects from the user's `prefs.json`, most recent first.
pub fn read_recent_projects() -> Vec<PathBuf> {
    read_recent_projects_at(&prefs_path())
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

    /// Old prefs (no times key) read as no times; the times round-trip
    /// beside the list, and a malformed entry is skipped.
    #[test]
    fn recent_opened_times_round_trip_and_old_prefs_read_as_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, br#"{"recent_projects": ["a.snxprj"]}"#).expect("write");
        assert!(read_recent_opened_from(read_prefs_json(&path).as_ref()).is_empty());
        assert!(read_recent_opened_from(None).is_empty());

        let projects = [PathBuf::from("b.snxprj"), PathBuf::from("a.snxprj")];
        let opened = std::collections::BTreeMap::from([(PathBuf::from("b.snxprj"), 1_700_000_000)]);
        write_recent_projects_and_times_at(&path, &projects, &opened);
        assert_eq!(read_recent_projects_at(&path), projects);
        assert_eq!(
            read_recent_opened_from(read_prefs_json(&path).as_ref()),
            [(PathBuf::from("b.snxprj"), 1_700_000_000)]
        );

        std::fs::write(
            &path,
            br#"{"recent_projects_opened": {"a.snxprj": "soon", "b.snxprj": 5, "": 3}}"#,
        )
        .expect("write");
        assert_eq!(
            read_recent_opened_from(read_prefs_json(&path).as_ref()),
            [(PathBuf::from("b.snxprj"), 5)]
        );
    }

    #[test]
    fn prefs_without_pinned_projects_read_as_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, br#"{"recent_projects": ["a.snxprj"]}"#).expect("write");
        assert!(read_pinned_projects_at(&path).is_empty());
        assert!(read_pinned_projects_from(None).is_empty());
    }

    /// As for `recent_projects`: a malformed value reads as its string
    /// entries (or none), and the next write replaces it.
    #[test]
    fn a_malformed_pinned_list_reads_as_its_strings_and_the_next_write_replaces_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        std::fs::write(&path, br#"{"pinned_projects": "oops"}"#).expect("write");
        assert!(read_pinned_projects_at(&path).is_empty());

        std::fs::write(
            &path,
            br#"{"pinned_projects": ["a.snxprj", 7, null, "", {"x": 1}, "b.snxprj"]}"#,
        )
        .expect("write");
        assert_eq!(
            read_pinned_projects_at(&path),
            [PathBuf::from("a.snxprj"), PathBuf::from("b.snxprj")]
        );

        write_pinned_projects_at(&path, &[PathBuf::from("c.snxprj")]);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
        assert_eq!(json["pinned_projects"], serde_json::json!(["c.snxprj"]));
    }

    #[test]
    fn pinned_projects_round_trip_in_order_beside_the_recent_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prefs.json");
        let recent = [PathBuf::from("r.snxprj")];
        let pinned = [
            PathBuf::from("c.snxprj"),
            PathBuf::from("a.snxprj"),
            PathBuf::from("b.snxprj"),
        ];

        write_recent_projects_at(&path, &recent);
        write_pinned_projects_at(&path, &pinned);

        assert_eq!(read_pinned_projects_at(&path), pinned);
        assert_eq!(read_recent_projects_at(&path), recent);
    }
}
