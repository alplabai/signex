//! The clock and the documents' file times the computed special strings show
//! (`${CurrentDate}`, `${CurrentTime}`, `${ModifiedDate}`; maintainer
//! decision 2026-11-18). Read in the runtime layer and kept here, so `view`
//! and the resolver read state, never the clock or the disk.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};

/// The format of `${CurrentDate}`, `${ModifiedDate}` and an empty `${Date}`.
const DATE_FORMAT: &str = "%Y-%m-%d";
/// The format of `${CurrentTime}`: to the minute, so the shown value, and
/// what the canvas redraws for, changes at most once a minute.
const TIME_FORMAT: &str = "%H:%M";

/// The clock stamp and the file times of the open documents.
#[derive(Debug, Clone, Default)]
pub struct DocumentTimes {
    /// The clock as read at boot and at each panel refresh; `None` only in a
    /// default-built value.
    pub now: Option<DateTime<Local>>,
    /// Each document file's modified time, from its last read or save.
    pub modified: HashMap<PathBuf, SystemTime>,
}

impl DocumentTimes {
    /// The clock stamp `now`, no file times yet. The caller reads the clock
    /// (bootstrap, runtime); the core never does (ADR-0006). The app starts
    /// with it, so a frame lowered before the first panel refresh already
    /// shows today for an empty Date.
    pub fn at(now: DateTime<Local>) -> Self {
        Self {
            now: Some(now),
            ..Self::default()
        }
    }

    /// Record that the file of `path` was modified at `at`.
    pub fn set_modified(&mut self, path: &Path, at: SystemTime) {
        self.modified.insert(path.to_path_buf(), at);
    }

    /// The values the resolver shows for the computed tokens.
    pub fn facts(&self) -> signex_output::DocumentFacts {
        let format = |at: DateTime<Local>, pattern: &str| at.format(pattern).to_string();
        signex_output::DocumentFacts {
            current_date: self
                .now
                .map(|now| format(now, DATE_FORMAT))
                .unwrap_or_default(),
            current_time: self
                .now
                .map(|now| format(now, TIME_FORMAT))
                .unwrap_or_default(),
            modified_dates: self
                .modified
                .iter()
                .map(|(path, at)| (path.clone(), format(DateTime::from(*at), DATE_FORMAT)))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn the_facts_format_the_clock_and_the_file_times() {
        let at = Local.with_ymd_and_hms(2026, 11, 18, 9, 5, 0).unwrap();
        let mut times = DocumentTimes::at(at);
        assert_eq!(times.now, Some(at));
        assert!(times.modified.is_empty());
        times.set_modified(Path::new("a.snxsch"), SystemTime::from(at));
        let facts = times.facts();
        assert_eq!(facts.current_date, "2026-11-18");
        assert_eq!(facts.current_time, "09:05");
        let modified = facts.modified_dates.get(Path::new("a.snxsch"));
        assert_eq!(modified.map(String::as_str), Some("2026-11-18"));
        assert_eq!(DocumentTimes::default().facts(), Default::default());
    }
}
