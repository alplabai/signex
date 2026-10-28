//! What a recovery sidecar left by an earlier run means for a document
//! that is being opened now (#1164). Blocking disk work: call it from
//! `spawn_blocking`, never from `update`.
//!
//! A sidecar is only worth offering when it holds edits the file lacks.
//! [`reconcile`] therefore removes one that is byte-identical to the file
//! (the copy is of a clean document) and reports the rest. Timestamps never
//! decide whether to delete: a checkout or a cloud sync makes the file look
//! newer without making a differing copy worthless, and offering it is safe.
//! They only word the prompt: a copy older than the file is offered with a
//! warning that Restore replaces newer saved work.
//!
//! Error texts lead with the reason and end with the path: the Messages
//! panel cuts a long line, and a long recovery path must not hide the OS error.
//!
//! The sidecar name encodes a hash of the document path and the sidecar
//! holds only the document's bytes, so a sidecar cannot be traced back to
//! its document without opening that document. Reconciling at open is
//! therefore the sweep for stale copies; [`sweep_stranded_temp_files`] is
//! the startup sweep for what cannot be reached that way.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::Serialize;

use super::{RestoredDocument, sidecar};

/// Extension [`signex_types::atomic_io::atomic_write`] gives its temp file.
const TEMP_EXTENSION: &str = "tmp";

/// Timestamps closer than this count as the same moment (coarse filesystem
/// clocks, a save right after the autosave). A copy that may be older than
/// the file is warned about: a false warning costs a glance, a missing one
/// can cost saved work.
const SAME_TIME_MARGIN: Duration = Duration::from_secs(2);

/// A temp file this young may belong to a write that is still running
/// (here or in another instance), so the startup sweep leaves it.
pub const STRANDED_TEMP_MIN_AGE: Duration = Duration::from_secs(60 * 60);

/// A recovery copy with edits the document's file lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recoverable {
    /// How long ago the copy was written, in seconds.
    pub age_secs: u64,
    /// The copy was last written before the saved file was: restoring it
    /// replaces newer saved work.
    pub older_than_file: bool,
}

/// Decide what the sidecar of `document` means, and drop it when it means
/// nothing. `Ok(None)`: no sidecar, or one that was just removed because
/// the file is identical. `Err`: the copy could not be judged (an IO error
/// on the copy or the document), so it must stay protected.
pub fn reconcile(
    recovery_dir: &Path,
    document: &Path,
    now: SystemTime,
) -> Result<Option<Recoverable>, String> {
    // A copy a rename left under its old key, whose move failed last run,
    // is moved to this document's key before it is looked for.
    super::pending_move::finish_all(recovery_dir);
    let sidecar_file = sidecar::sidecar_path(recovery_dir, document);
    let sidecar_modified = match std::fs::metadata(&sidecar_file) {
        Ok(meta) => meta
            .modified()
            .map_err(|e| format!("{e}: could not read the age of {}", sidecar_file.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(format!(
                "{e}: could not read recovery copy {}",
                sidecar_file.display()
            ));
        }
    };
    // A document that cannot be read now is not judged: the copy stays
    // rather than being deleted on a transient error. It is an `Err`, not
    // `Ok(None)`, so the caller keeps autosave off for the document: the
    // copy was never seen and a later write would replace it.
    let file_meta = std::fs::metadata(document).map_err(|e| {
        format!(
            "{e}: could not read {} to compare its recovery copy",
            document.display()
        )
    })?;
    // The bytes decide, never the timestamps.
    let copy = std::fs::read(&sidecar_file).map_err(|e| {
        format!(
            "{e}: could not read recovery copy {}",
            sidecar_file.display()
        )
    })?;
    if std::fs::read(document).is_ok_and(|file| file == copy) {
        sidecar::remove(recovery_dir, document)?;
        return Ok(None);
    }
    Ok(Some(Recoverable {
        age_secs: now
            .duration_since(sidecar_modified)
            .map_or(0, |age| age.as_secs()),
        older_than_file: file_meta.modified().is_ok_and(|file_modified| {
            file_modified
                .checked_add(SAME_TIME_MARGIN)
                .is_none_or(|limit| sidecar_modified <= limit)
        }),
    }))
}

/// Why a Restore could not load the copy. The kinds need different
/// handling: a copy that cannot be parsed protects nothing, one that could
/// not even be read may be intact and must not be replaced, and one that is
/// gone leaves nothing to protect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum LoadError {
    /// The file could not be read (permissions, a lock, a sharing violation).
    Unreadable(String),
    /// There is no copy any more (removed meanwhile).
    Missing(String),
    /// The bytes were read but are not valid text or not a valid document.
    Invalid(String),
}

impl LoadError {
    pub fn reason(&self) -> &str {
        match self {
            Self::Unreadable(reason) | Self::Missing(reason) | Self::Invalid(reason) => reason,
        }
    }
}

/// Read and parse the sidecar of `document` for a Restore. The document's
/// extension says what the copy holds.
pub fn load_document(recovery_dir: &Path, document: &Path) -> Result<RestoredDocument, LoadError> {
    let sidecar_file = sidecar::sidecar_path(recovery_dir, document);
    let bytes = std::fs::read(&sidecar_file).map_err(|e| {
        let reason = format!(
            "{e}: could not read recovery copy {}",
            sidecar_file.display()
        );
        if e.kind() == std::io::ErrorKind::NotFound {
            LoadError::Missing(reason)
        } else {
            LoadError::Unreadable(reason)
        }
    })?;
    let unreadable = |kind: &str, e: &dyn std::fmt::Display| {
        LoadError::Invalid(format!(
            "{e}: recovery copy is not a readable {kind}, so autosave resumes: {}",
            sidecar_file.display()
        ))
    };
    let extension = document
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("snxsym") => signex_library::SymbolFile::from_bytes(&bytes)
            .map(|file| RestoredDocument::Symbol(Box::new(file)))
            .map_err(|e| unreadable("symbol", &e)),
        Some("snxfpt") => {
            let text = String::from_utf8(bytes).map_err(|e| unreadable("footprint", &e))?;
            signex_library::FootprintFile::from_toml_str(&text)
                .map(|file| RestoredDocument::Footprint(Box::new(file)))
                .map_err(|e| unreadable("footprint", &e))
        }
        Some("snxpcb" | "standard_pcb") => {
            let text = String::from_utf8(bytes).map_err(|e| unreadable("PCB", &e))?;
            signex_types::format::SnxPcb::parse(&text)
                .map(|pcb| RestoredDocument::Pcb(Box::new(pcb.board)))
                .map_err(|e| unreadable("PCB", &format!("{e:#}")))
        }
        _ => {
            let text = String::from_utf8(bytes).map_err(|e| unreadable("schematic", &e))?;
            signex_types::format::SnxSchematic::parse(&text)
                .map(|schematic| RestoredDocument::Schematic(Box::new(schematic.sheet)))
                .map_err(|e| unreadable("schematic", &format!("{e:#}")))
        }
    }
}

/// What the startup sweep did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SweepReport {
    /// Temp files removed.
    pub removed: usize,
    /// Folder entries that could not be read and temp files that could not
    /// be removed.
    pub failed: usize,
    /// The first failure, reason first, for the Messages panel.
    pub first_failure: Option<String>,
}

impl SweepReport {
    fn record_failure(&mut self, reason: String) {
        self.failed += 1;
        self.first_failure.get_or_insert(reason);
    }
}

/// Remove temp files a crashed write stranded in the recovery directory.
/// They are clutter, never recovery data. A failure to list the folder is an
/// `Err`; a failure on one entry is counted in the report and the sweep goes on.
pub fn sweep_stranded_temp_files(
    recovery_dir: &Path,
    now: SystemTime,
    min_age: Duration,
) -> Result<SweepReport, String> {
    let entries = match std::fs::read_dir(recovery_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SweepReport::default()),
        Err(e) => {
            return Err(format!(
                "{e}: could not list recovery folder {}",
                recovery_dir.display()
            ));
        }
    };
    let mut report = SweepReport::default();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                report.record_failure(format!(
                    "{e}: could not read an entry of recovery folder {}",
                    recovery_dir.display()
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(TEMP_EXTENSION) {
            continue;
        }
        let old_enough = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= min_age);
        if !old_enough {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => report.removed += 1,
            Err(e) => report.record_failure(format!(
                "{e}: could not remove stranded temp file {}",
                path.display()
            )),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _tmp: tempfile::TempDir,
        doc: std::path::PathBuf,
        rec: std::path::PathBuf,
    }

    fn fixture(file: &[u8], copy: &[u8]) -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join("main.snxsch");
        let rec = tmp.path().join("recovery");
        std::fs::write(&doc, file).unwrap();
        sidecar::write(&rec, &doc, copy).unwrap();
        Fixture {
            _tmp: tmp,
            doc,
            rec,
        }
    }

    fn set_age(path: &Path, secs_ago: u64) {
        let when = SystemTime::now() - Duration::from_secs(secs_ago);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }

    #[test]
    fn a_newer_differing_copy_is_offered_and_kept() {
        let f = fixture(b"saved", b"unsaved edits");
        set_age(&f.doc, 600);
        set_age(&sidecar::sidecar_path(&f.rec, &f.doc), 60);
        let offer = reconcile(&f.rec, &f.doc, SystemTime::now()).unwrap();
        assert!(offer.is_some_and(|o| (59..=70).contains(&o.age_secs) && !o.older_than_file));
        assert!(sidecar::sidecar_path(&f.rec, &f.doc).exists());
    }

    #[test]
    fn a_differing_copy_is_offered_even_when_the_file_looks_newer() {
        let f = fixture(b"saved later", b"older edits");
        set_age(&f.doc, 60);
        set_age(&sidecar::sidecar_path(&f.rec, &f.doc), 600);
        let offer = reconcile(&f.rec, &f.doc, SystemTime::now()).unwrap();
        assert!(
            offer.is_some_and(|o| o.older_than_file),
            "a touched mtime must not delete a copy, and the offer says the file is newer"
        );
        assert!(sidecar::sidecar_path(&f.rec, &f.doc).exists());
    }

    #[test]
    fn a_copy_and_a_file_with_the_same_mtime_are_offered_with_the_warning() {
        let f = fixture(b"saved", b"unsaved edits");
        let when = SystemTime::now() - Duration::from_secs(300);
        for path in [&f.doc, &sidecar::sidecar_path(&f.rec, &f.doc)] {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(when)
                .unwrap();
        }
        let offer = reconcile(&f.rec, &f.doc, SystemTime::now()).unwrap();
        assert!(
            offer.is_some_and(|o| o.older_than_file),
            "equal timestamps cannot rule out newer saved work"
        );
    }

    #[test]
    fn a_copy_that_is_not_a_schematic_or_not_text_is_invalid_and_a_missing_one_missing() {
        let f = fixture(b"saved", b"not a schematic");
        let Err(LoadError::Invalid(reason)) = load_document(&f.rec, &f.doc) else {
            panic!("garbage text is an invalid copy");
        };
        assert!(reason.contains("recovery copy is not a readable schematic"));
        sidecar::write(&f.rec, &f.doc, &[0xff, 0xfe, 0x00]).unwrap();
        assert!(matches!(
            load_document(&f.rec, &f.doc),
            Err(LoadError::Invalid(_))
        ));
        sidecar::remove(&f.rec, &f.doc).unwrap();
        assert!(matches!(
            load_document(&f.rec, &f.doc),
            Err(LoadError::Missing(_))
        ));
    }

    #[test]
    fn a_copy_identical_to_the_file_is_removed_not_offered() {
        let f = fixture(b"same bytes", b"same bytes");
        set_age(&f.doc, 600);
        set_age(&sidecar::sidecar_path(&f.rec, &f.doc), 60);
        assert_eq!(reconcile(&f.rec, &f.doc, SystemTime::now()).unwrap(), None);
        assert!(!sidecar::sidecar_path(&f.rec, &f.doc).exists());
    }

    #[test]
    fn no_sidecar_means_nothing_to_offer() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join("main.snxsch");
        std::fs::write(&doc, b"x").unwrap();
        let rec = tmp.path().join("recovery");
        assert_eq!(reconcile(&rec, &doc, SystemTime::now()).unwrap(), None);
    }

    #[test]
    fn an_unreadable_document_keeps_its_copy_and_is_an_error() {
        let f = fixture(b"saved", b"unsaved edits");
        std::fs::remove_file(&f.doc).unwrap();
        assert!(reconcile(&f.rec, &f.doc, SystemTime::now()).is_err());
        assert!(sidecar::sidecar_path(&f.rec, &f.doc).exists());
    }

    #[test]
    fn the_startup_sweep_removes_old_temp_files_only() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path();
        let stale_tmp = rec.join("main.snxsch-00.recovery.123-0.tmp");
        let fresh_tmp = rec.join("main.snxsch-00.recovery.123-1.tmp");
        let copy = rec.join("main.snxsch-00.recovery");
        for file in [&stale_tmp, &fresh_tmp, &copy] {
            std::fs::write(file, b"x").unwrap();
        }
        set_age(&stale_tmp, 2 * 3600);
        set_age(&copy, 2 * 3600);
        let report =
            sweep_stranded_temp_files(rec, SystemTime::now(), STRANDED_TEMP_MIN_AGE).unwrap();
        assert_eq!(report.removed, 1);
        assert_eq!(report.failed, 0);
        assert!(!stale_tmp.exists());
        assert!(
            fresh_tmp.exists(),
            "a young temp may belong to a live write"
        );
        assert!(copy.exists(), "a recovery copy is never swept by age");
    }

    #[test]
    fn sweeping_a_missing_folder_is_fine() {
        let tmp = tempfile::tempdir().unwrap();
        let report = sweep_stranded_temp_files(
            &tmp.path().join("nope"),
            SystemTime::now(),
            STRANDED_TEMP_MIN_AGE,
        )
        .unwrap();
        assert_eq!(report, SweepReport::default());
    }

    /// A stale temp the sweep cannot remove (here a directory with the temp
    /// extension) is counted and named, not dropped.
    #[cfg(unix)]
    #[test]
    fn the_startup_sweep_counts_a_temp_it_cannot_remove() {
        let tmp = tempfile::tempdir().unwrap();
        let stuck = tmp.path().join("stuck.recovery.1-0.tmp");
        std::fs::create_dir(&stuck).unwrap();
        std::fs::File::open(&stuck)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(2 * 3600))
            .unwrap();
        let report =
            sweep_stranded_temp_files(tmp.path(), SystemTime::now(), STRANDED_TEMP_MIN_AGE)
                .unwrap();
        assert_eq!(report.removed, 0);
        assert_eq!(report.failed, 1);
        let reason = report.first_failure.expect("a reason");
        assert!(
            reason.ends_with(&stuck.display().to_string()),
            "path last: {reason}"
        );
    }
}
