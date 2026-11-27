//! `import-report.json` (contract §4): what each source item became.

use std::path::Path;

use serde::Deserialize;

/// The report file's size limit; a report is per-item text, not data.
pub const MAX_REPORT_BYTES: u64 = 64 << 20;

/// The report a converter writes into the library folder (contract §4).
pub const REPORT_JSON: &str = "import-report.json";
/// Its human-readable copy, beside it.
pub const REPORT_MD: &str = "import-report.md";

/// How much of an item was lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Approximated,
    Dropped,
    Error,
    /// A severity a newer contract minor added.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct KindCounts {
    pub symbols: u64,
    pub footprints: u64,
    pub parts: u64,
    pub models: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ReportCounts {
    pub source: KindCounts,
    pub written: KindCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReportSource {
    pub path: String,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ReportItem {
    pub kind: String,
    pub source: String,
    #[serde(default)]
    pub id: Option<String>,
    pub code: String,
    pub severity: Severity,
    pub message: String,
    #[serde(default)]
    pub detail: serde_json::Value,
}

/// A parsed loss report.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ImportReport {
    pub contract: String,
    pub converter: String,
    pub converter_version: String,
    #[serde(default)]
    pub started: Option<String>,
    #[serde(default)]
    pub sources: Vec<ReportSource>,
    #[serde(default)]
    pub library_id: Option<String>,
    #[serde(default)]
    pub counts: ReportCounts,
    #[serde(default)]
    pub items: Vec<ReportItem>,
}

impl ImportReport {
    /// Items at `severity`.
    pub fn count(&self, severity: Severity) -> usize {
        self.items.iter().filter(|i| i.severity == severity).count()
    }
}

pub fn parse_report(text: &str) -> Result<ImportReport, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

/// Read and parse the report at `path`, refusing one over
/// [`MAX_REPORT_BYTES`].
pub fn read_report(path: &Path) -> Result<ImportReport, String> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut text = String::new();
    file.take(MAX_REPORT_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() as u64 > MAX_REPORT_BYTES {
        return Err(format!("larger than {MAX_REPORT_BYTES} bytes"));
    }
    parse_report(&text)
}

/// The report stored in an imported library's folder `library_dir`.
/// Blocking.
pub fn read_stored_report(library_dir: &Path) -> Result<ImportReport, String> {
    read_report(&library_dir.join(REPORT_JSON))
}

/// Copy the stored readable report of `library_dir` to `target` with an
/// atomic write, so a failed copy leaves `target` as it was. Blocking.
pub fn save_stored_report(library_dir: &Path, target: &Path) -> Result<(), String> {
    use std::io::Read as _;
    let file = std::fs::File::open(library_dir.join(REPORT_MD)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_REPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_REPORT_BYTES {
        return Err(format!("larger than {MAX_REPORT_BYTES} bytes"));
    }
    signex_types::atomic_io::atomic_write(target, &bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str =
        include_str!("../../../../docs/fixtures/library-converter/import-report.json");

    #[test]
    fn the_fixture_report_parses() {
        let r = parse_report(FIXTURE).unwrap();
        assert_eq!(r.converter, "example-import");
        assert_eq!(r.counts.source.parts, 4);
        assert_eq!(r.counts.written.parts, 3);
        assert_eq!(r.items.len(), 3);
        assert_eq!(r.count(Severity::Error), 1);
        assert_eq!(r.items[2].id, None);
        assert_eq!(r.items[0].detail["pads"], 2);
    }

    #[test]
    fn the_stored_report_is_read_and_its_readable_copy_saved_whole() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_stored_report(dir.path()).is_err(), "none yet");
        std::fs::write(dir.path().join(REPORT_JSON), FIXTURE).unwrap();
        assert_eq!(read_stored_report(dir.path()).unwrap().items.len(), 3);
        let target = dir.path().join("saved.md");
        assert!(save_stored_report(dir.path(), &target).is_err(), "no .md");
        assert!(!target.exists(), "a failed save writes nothing");
        std::fs::write(dir.path().join(REPORT_MD), "# Report\n").unwrap();
        save_stored_report(dir.path(), &target).unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "# Report\n");
    }

    #[test]
    fn a_new_severity_is_kept_as_other_and_garbage_refused() {
        let text = FIXTURE.replace("\"approximated\"", "\"rounded\"");
        assert_eq!(parse_report(&text).unwrap().count(Severity::Other), 1);
        assert!(parse_report("{").is_err());
        assert!(parse_report("{}").is_err());
    }
}
