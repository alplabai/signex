//! What a recovery sidecar left by an earlier run means for a document
//! that is being opened now (#1164). Blocking disk work: call it from
//! `spawn_blocking`, never from `update`.
//!
//! A sidecar is only worth offering when it holds edits the file lacks.
//! [`reconcile`] therefore removes one that is byte-identical to the file
//! (the copy is of a clean document) and reports the rest. Timestamps are
//! not consulted: a checkout or a cloud sync makes the file look newer
//! without making a differing copy worthless, and offering it is safe.
//!
//! The sidecar name encodes a hash of the document path and the sidecar
//! holds only the document's bytes, so a sidecar cannot be traced back to
//! its document without opening that document. Reconciling at open is
//! therefore the sweep for stale copies; [`sweep_stranded_temp_files`] is
//! the startup sweep for what cannot be reached that way.

use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::Serialize;

use super::sidecar;

/// Extension [`signex_types::atomic_io::atomic_write`] gives its temp file.
const TEMP_EXTENSION: &str = "tmp";

/// A temp file this young may belong to a write that is still running
/// (here or in another instance), so the startup sweep leaves it.
pub const STRANDED_TEMP_MIN_AGE: Duration = Duration::from_secs(60 * 60);

/// A recovery copy with edits the document's file lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recoverable {
    /// How long ago the copy was written, in seconds.
    pub age_secs: u64,
}

/// Decide what the sidecar of `document` means, and drop it when it means
/// nothing. `Ok(None)`: no sidecar, or one that was just removed because
/// the file is identical.
pub fn reconcile(
    recovery_dir: &Path,
    document: &Path,
    now: SystemTime,
) -> Result<Option<Recoverable>, String> {
    let sidecar_file = sidecar::sidecar_path(recovery_dir, document);
    let sidecar_modified = match std::fs::metadata(&sidecar_file) {
        Ok(meta) => meta
            .modified()
            .map_err(|e| format!("could not read the age of {}: {e}", sidecar_file.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(format!(
                "could not read recovery copy {}: {e}",
                sidecar_file.display()
            ));
        }
    };
    // A document that cannot be read now is not judged: the copy stays
    // for a later open rather than being deleted on a transient error.
    if std::fs::metadata(document).is_err() {
        return Ok(None);
    }
    // The bytes decide, never the timestamps.
    let copy = std::fs::read(&sidecar_file).map_err(|e| {
        format!(
            "could not read recovery copy {}: {e}",
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
    }))
}

/// Read and parse the sidecar of `document` for a Restore.
pub fn load_sheet(
    recovery_dir: &Path,
    document: &Path,
) -> Result<signex_types::schematic::SchematicSheet, String> {
    let sidecar_file = sidecar::sidecar_path(recovery_dir, document);
    let text = std::fs::read_to_string(&sidecar_file).map_err(|e| {
        format!(
            "could not read recovery copy {}: {e}",
            sidecar_file.display()
        )
    })?;
    signex_types::format::SnxSchematic::parse(&text)
        .map(|schematic| schematic.sheet)
        .map_err(|e| {
            format!(
                "recovery copy {} is not a readable schematic: {e:#}",
                sidecar_file.display()
            )
        })
}

/// Remove temp files a crashed write stranded in the recovery directory.
/// They are clutter, never recovery data. Returns how many were removed.
pub fn sweep_stranded_temp_files(
    recovery_dir: &Path,
    now: SystemTime,
    min_age: Duration,
) -> Result<usize, String> {
    let entries = match std::fs::read_dir(recovery_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => {
            return Err(format!(
                "could not list recovery folder {}: {e}",
                recovery_dir.display()
            ));
        }
    };
    let mut removed = 0;
    for entry in entries.flatten() {
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
        if old_enough && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
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
        assert!(offer.is_some_and(|o| (59..=70).contains(&o.age_secs)));
        assert!(sidecar::sidecar_path(&f.rec, &f.doc).exists());
    }

    #[test]
    fn a_differing_copy_is_offered_even_when_the_file_looks_newer() {
        let f = fixture(b"saved later", b"older edits");
        set_age(&f.doc, 60);
        set_age(&sidecar::sidecar_path(&f.rec, &f.doc), 600);
        let offer = reconcile(&f.rec, &f.doc, SystemTime::now()).unwrap();
        assert!(offer.is_some(), "a touched mtime must not delete a copy");
        assert!(sidecar::sidecar_path(&f.rec, &f.doc).exists());
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
    fn an_unreadable_document_keeps_its_copy() {
        let f = fixture(b"saved", b"unsaved edits");
        std::fs::remove_file(&f.doc).unwrap();
        assert_eq!(reconcile(&f.rec, &f.doc, SystemTime::now()).unwrap(), None);
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
        let removed =
            sweep_stranded_temp_files(rec, SystemTime::now(), STRANDED_TEMP_MIN_AGE).unwrap();
        assert_eq!(removed, 1);
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
        let removed = sweep_stranded_temp_files(
            &tmp.path().join("nope"),
            SystemTime::now(),
            STRANDED_TEMP_MIN_AGE,
        )
        .unwrap();
        assert_eq!(removed, 0);
    }
}
