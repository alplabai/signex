//! The recovery sidecar of one document: where it lives and how it is
//! written and removed.
//!
//! **Location (decision for #104).** Sidecars live in one recovery
//! directory under the per-user config root, not next to the document.
//! Next to the document they would show up in the project tree, in
//! `git status` and in a synced folder, would fail for a read-only
//! folder, and would sit one rename away from the real file. The recovery
//! directory keeps every project clean. The cost is that the name must
//! encode the document's full path: `<file name>-<hash of the path>.recovery`.
//!
//! The sidecar holds exactly the bytes a normal save would write, so a
//! user can also rescue it by hand (copy it, rename it to `.snxsch`). It
//! is always a different file from the document, and it is written with
//! [`signex_types::atomic_io::atomic_write`]: a crash mid-autosave leaves
//! the previous sidecar or the new one, never half of one.

use std::path::{Path, PathBuf};

use super::pending_move;
use crate::app::{CaseFold, directory_ignores_case, path_key_with};

/// Extension that marks a file as a recovery sidecar.
pub const SIDECAR_EXTENSION: &str = "recovery";

/// Extension appended to a copy that a newer copy of the same document
/// displaced; it is kept on disk but never offered.
const SUPERSEDED_EXTENSION: &str = "older";

/// The case rule of the volume `document` sits on, probed (disk IO, so only
/// from the off-thread recovery tasks, never `update`). When the volume
/// cannot be asked the probe answers "ignores case", the safe side: two
/// spellings then share one copy rather than losing one.
fn volume_fold(document: &Path) -> CaseFold {
    let dir = document
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    CaseFold::from_probe(directory_ignores_case(dir))
}

/// The key of #1530 and the builds between: the hash of the dot-segment
/// normalised path text exactly as spelled (no case fold).
fn as_given_path_hash(path: &Path) -> u64 {
    let path = crate::app::project_sheets::normalize_clamped(path);
    bytes_hash(path.to_string_lossy().as_bytes())
}

/// The key a build before #1510 gave `path`: the hash of its text exactly as
/// spelled, with no normalising.
fn legacy_path_hash(path: &Path) -> u64 {
    bytes_hash(path.to_string_lossy().as_bytes())
}

/// Every key form an older build could have filed a copy of `document` under,
/// other than the current one: the raw (pre-#1510) and the as-given
/// (#1510 to #1540) spellings, and on a case-insensitive volume the
/// volume-folded spelling under both. Distinct from `current` and each other.
fn older_keys(recovery_dir: &Path, document: &Path, fold: CaseFold) -> Vec<PathBuf> {
    let mut spellings = vec![document.to_path_buf()];
    if fold.fold_case {
        spellings.push(PathBuf::from(path_key_with(document, fold)));
    }
    let current = sidecar_path_with(recovery_dir, document, fold);
    let mut keys: Vec<PathBuf> = Vec::new();
    for spelling in &spellings {
        for hash in [as_given_path_hash(spelling), legacy_path_hash(spelling)] {
            let key = key_path(recovery_dir, spelling, hash);
            if key != current && !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    keys
}

/// Bring a copy written under an older key of `document` (see [`older_keys`])
/// to its current key, so the upgrade (or another spelling) does not strand it.
/// Nothing happens when no older-key copy exists. Where a copy already sits at
/// the current key, only the newest of the two is offered: if the older-key
/// copy is newer, the current one is set aside and the older-key one adopted;
/// otherwise it stays where it is. No copy is ever deleted or overwritten: the
/// move is one atomic `rename` inside the recovery directory, which a crash
/// leaves either done or not done.
pub fn adopt_legacy(recovery_dir: &Path, document: &Path) -> Result<(), String> {
    adopt_legacy_with(recovery_dir, document, volume_fold(document))
}

fn adopt_legacy_with(recovery_dir: &Path, document: &Path, fold: CaseFold) -> Result<(), String> {
    let current = sidecar_path_with(recovery_dir, document, fold);
    for legacy in older_keys(recovery_dir, document, fold) {
        if legacy.is_file() {
            adopt_one(&current, &legacy, document)?;
        }
    }
    Ok(())
}

fn adopt_one(current: &Path, legacy: &Path, document: &Path) -> Result<(), String> {
    let (current, legacy) = (current.to_path_buf(), legacy.to_path_buf());
    if current.exists() {
        let newer = |a: &Path, b: &Path| match (
            std::fs::metadata(a).and_then(|m| m.modified()),
            std::fs::metadata(b).and_then(|m| m.modified()),
        ) {
            (Ok(a), Ok(b)) => a > b,
            _ => false,
        };
        // Only the newest of the two is offered. The other is never deleted:
        // it stays on disk under a name no scan offers, and is logged.
        if newer(&legacy, &current) {
            // Never overwrite an earlier set-aside copy: claim the first free
            // name (`.older`, `.older.1`, ...) by creating it with `create_new`,
            // which fails if it exists, so two processes cannot pick one name.
            // With none free the move is refused, so the document stays
            // protected by what is on disk.
            let aside = claim_aside_name(&current)?;
            if let Err(e) = std::fs::rename(&current, &aside) {
                // Do not leave the empty claim behind.
                if let Err(cleanup) = std::fs::remove_file(&aside) {
                    crate::diagnostics::log_warning(format!(
                        "{cleanup}: could not remove the unused claim {}",
                        aside.display()
                    ));
                }
                return Err(format!(
                    "{e}: could not set aside recovery copy {} as {}",
                    current.display(),
                    aside.display()
                ));
            }
            crate::diagnostics::log_warning(format!(
                "The recovery copy {} of {} was older than {}; it was kept as {}.",
                current.display(),
                document.display(),
                legacy.display(),
                aside.display()
            ));
        } else {
            crate::diagnostics::log_warning(format!(
                "The recovery copy {} (an older key of {}) was left on disk: a copy already \
                 sits under the current key.",
                legacy.display(),
                document.display()
            ));
            return Ok(());
        }
    }
    std::fs::rename(&legacy, &current).map_err(|e| {
        format!(
            "{e}: could not move recovery copy {} to {}",
            legacy.display(),
            current.display()
        )
    })
}

/// Atomically claim the first free set-aside name for `current` by creating an
/// empty file there. The caller renames the copy over its own claim.
fn claim_aside_name(current: &Path) -> Result<PathBuf, String> {
    let mut base = current.to_path_buf().into_os_string();
    base.push(".");
    base.push(SUPERSEDED_EXTENSION);
    for n in 0..100u32 {
        let mut name = base.clone();
        if n > 0 {
            name.push(format!(".{n}"));
        }
        let candidate = PathBuf::from(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(_) => return Ok(candidate),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => {
                return Err(format!(
                    "{e}: could not claim a name to set aside the recovery copy {}",
                    current.display()
                ));
            }
        }
    }
    Err(format!(
        "no free name to set aside the recovery copy {}",
        current.display()
    ))
}

/// FNV-1a over `bytes`; also the stamp of a draft without a revision counter.
pub fn bytes_hash(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The sidecar path for the document at `document` inside `recovery_dir`.
pub fn sidecar_path(recovery_dir: &Path, document: &Path) -> PathBuf {
    sidecar_path_with(recovery_dir, document, volume_fold(document))
}

/// [`sidecar_path`] under an explicit case rule: the key is the app-wide path
/// comparison key ([`path_key_with`]), so two spellings of one file on a
/// case-insensitive volume share a copy and on a case-sensitive one stay
/// distinct. Dot segments resolve lexically, never by a disk lookup:
/// `w/link/../x` and `w/x` share a key even when `link` is a symlink (the key
/// must be computable without IO, as `KeyMap::lexical_key` is). The file name is the folded
/// key's last segment, so two spellings of one file share the name as well as
/// the hash.
fn sidecar_path_with(recovery_dir: &Path, document: &Path, fold: CaseFold) -> PathBuf {
    let key = path_key_with(document, fold);
    let name = key
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("document");
    recovery_dir.join(format!(
        "{name}-{:016x}.{SIDECAR_EXTENSION}",
        bytes_hash(key.as_bytes())
    ))
}

fn key_path(recovery_dir: &Path, document: &Path, hash: u64) -> PathBuf {
    let name = document
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".to_string());
    recovery_dir.join(format!("{name}-{hash:016x}.{SIDECAR_EXTENSION}"))
}

/// Atomically write `bytes` as the sidecar of `document`.
pub fn write(recovery_dir: &Path, document: &Path, bytes: &[u8]) -> Result<(), String> {
    let sidecar = sidecar_path(recovery_dir, document);
    signex_types::atomic_io::atomic_write(&sidecar, bytes)
        .map_err(|e| format!("{e}: could not write recovery copy {}", sidecar.display()))
}

/// Remove the sidecar of `document`. A sidecar that is already gone is
/// success: the goal is "no sidecar", and two cleanups may race.
pub fn remove(recovery_dir: &Path, document: &Path) -> Result<(), String> {
    let sidecar = sidecar_path(recovery_dir, document);
    match std::fs::remove_file(&sidecar) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(format!(
                "{e}: could not remove recovery copy {}",
                sidecar.display()
            ));
        }
    }
    // No copy left to move.
    pending_move::clear(recovery_dir, document);
    Ok(())
}

/// Move the sidecar of `from` to where `to` looks for it, after the
/// document was renamed or its folder moved. A sidecar that is not there is
/// success: the document had no recovery copy.
///
/// The move is journalled first ([`pending_move`]) and the entry cleared
/// once the copy sits under the new key, so a move that fails until the app
/// quits is finished by the next run instead of stranding the copy.
pub fn rename(recovery_dir: &Path, from: &Path, to: &Path) -> Result<(), String> {
    // A copy of `from` still under an older key is brought to the current one
    // first, so the move below (and a replayed journal entry) finds it.
    if let Err(why) = adopt_legacy(recovery_dir, from) {
        crate::diagnostics::log_warning(why);
    }
    let (src, dst) = (
        sidecar_path(recovery_dir, from),
        sidecar_path(recovery_dir, to),
    );
    if src == dst || !src.is_file() {
        // Nothing to move: an entry for `from` would only mislead.
        pending_move::clear(recovery_dir, from);
        return Ok(());
    }
    // A journal that cannot be written must not block the move itself.
    if let Err(reason) = pending_move::record(recovery_dir, from, to) {
        crate::diagnostics::log_warning(reason);
    }
    match std::fs::rename(&src, &dst) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(format!(
                "could not move recovery copy {} to {}: {e}",
                src.display(),
                dst.display()
            ));
        }
    }
    pending_move::clear_if(recovery_dir, from, std::slice::from_ref(&to.to_path_buf()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_is_never_the_document_and_differs_per_directory() {
        let dir = Path::new("rec");
        let a = Path::new("proj_a/main.snxsch");
        let b = Path::new("proj_b/main.snxsch");
        assert_ne!(sidecar_path(dir, a), a);
        assert_ne!(sidecar_path(dir, a), sidecar_path(dir, b));
        assert_eq!(sidecar_path(dir, a), sidecar_path(dir, a));
        assert_eq!(sidecar_path(dir, a).extension().unwrap(), SIDECAR_EXTENSION);
    }

    #[test]
    fn write_then_remove_round_trips_and_remove_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join("main.snxsch");
        let rec = tmp.path().join("recovery");
        write(&rec, &doc, b"draft").unwrap();
        assert_eq!(std::fs::read(sidecar_path(&rec, &doc)).unwrap(), b"draft");
        assert!(!doc.exists(), "the document itself is never created");
        remove(&rec, &doc).unwrap();
        remove(&rec, &doc).unwrap();
        assert!(!sidecar_path(&rec, &doc).exists());
    }

    #[test]
    fn the_newest_of_two_copies_is_offered_and_the_other_is_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("w/../main.snxsch");
        let current = sidecar_path(&rec, &doc);
        let legacy = key_path(&rec, &doc, legacy_path_hash(&doc));
        assert_ne!(current, legacy);
        std::fs::create_dir_all(&rec).unwrap();
        std::fs::write(&current, b"old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&legacy, b"new").unwrap();
        adopt_legacy(&rec, &doc).unwrap();
        assert_eq!(std::fs::read(&current).unwrap(), b"new");
        let aside = format!("{}.{SUPERSEDED_EXTENSION}", current.display());
        assert_eq!(std::fs::read(aside).unwrap(), b"old");
        assert!(!legacy.exists());
    }

    #[test]
    fn an_existing_older_copy_is_never_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("w/../main.snxsch");
        let current = sidecar_path(&rec, &doc);
        let legacy = key_path(&rec, &doc, legacy_path_hash(&doc));
        std::fs::create_dir_all(&rec).unwrap();
        let older = format!("{}.{SUPERSEDED_EXTENSION}", current.display());
        std::fs::write(&older, b"earliest").unwrap();
        std::fs::write(&current, b"old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&legacy, b"new").unwrap();
        adopt_legacy(&rec, &doc).unwrap();
        assert_eq!(std::fs::read(&current).unwrap(), b"new");
        assert_eq!(std::fs::read(&older).unwrap(), b"earliest");
        assert_eq!(std::fs::read(format!("{older}.1")).unwrap(), b"old");
        assert!(!legacy.exists());
    }

    #[test]
    fn with_every_aside_name_taken_adopt_fails_and_touches_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("w/../main.snxsch");
        let current = sidecar_path(&rec, &doc);
        let legacy = key_path(&rec, &doc, legacy_path_hash(&doc));
        std::fs::create_dir_all(&rec).unwrap();
        let mut files = vec![(current.clone(), b"old".to_vec())];
        let base = format!("{}.{SUPERSEDED_EXTENSION}", current.display());
        for n in 0..100u32 {
            let name = if n == 0 {
                base.clone()
            } else {
                format!("{base}.{n}")
            };
            files.push((PathBuf::from(name), format!("aside{n}").into_bytes()));
        }
        for (p, b) in &files {
            std::fs::write(p, b).unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&legacy, b"new").unwrap();
        files.push((legacy.clone(), b"new".to_vec()));
        assert!(adopt_legacy(&rec, &doc).is_err());
        for (p, b) in &files {
            assert_eq!(&std::fs::read(p).unwrap(), b, "{}", p.display());
        }
        assert!(!Path::new(&format!("{base}.100")).exists());
    }

    #[test]
    fn a_failed_set_aside_rename_is_an_error_and_cleans_its_claim() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("w/../main.snxsch");
        let current = sidecar_path(&rec, &doc);
        let legacy = key_path(&rec, &doc, legacy_path_hash(&doc));
        // Make the rename (and only the rename) fail. Unix refuses a directory
        // renamed over a file; Windows replaces it, so there the copy is held
        // open without delete sharing, which blocks renaming it.
        #[cfg(unix)]
        {
            std::fs::create_dir_all(&current).unwrap();
            std::fs::write(current.join("inner"), b"x").unwrap();
        }
        #[cfg(windows)]
        let _held = {
            use std::os::windows::fs::OpenOptionsExt;
            std::fs::create_dir_all(&rec).unwrap();
            std::fs::write(&current, b"old").unwrap();
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0x1 | 0x2) // FILE_SHARE_READ | FILE_SHARE_WRITE
                .open(&current)
                .unwrap()
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&legacy, b"new").unwrap();
        assert!(adopt_legacy(&rec, &doc).is_err());
        assert!(current.exists());
        assert_eq!(std::fs::read(&legacy).unwrap(), b"new");
        let aside = format!("{}.{SUPERSEDED_EXTENSION}", current.display());
        assert!(!Path::new(&aside).exists());
    }

    #[test]
    fn a_newer_current_copy_leaves_both_files_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("w/../main.snxsch");
        let current = sidecar_path(&rec, &doc);
        let legacy = key_path(&rec, &doc, legacy_path_hash(&doc));
        std::fs::create_dir_all(&rec).unwrap();
        std::fs::write(&legacy, b"old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&current, b"new").unwrap();
        adopt_legacy(&rec, &doc).unwrap();
        assert_eq!(std::fs::read(&current).unwrap(), b"new");
        assert_eq!(std::fs::read(&legacy).unwrap(), b"old");
        let aside = format!("{}.{SUPERSEDED_EXTENSION}", current.display());
        assert!(!Path::new(&aside).exists());
    }

    // Folding rules injected, so both volume kinds run on every host.
    const INSENSITIVE: CaseFold = CaseFold::case_only();
    const SENSITIVE: CaseFold = CaseFold::PRESERVE;

    fn put(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn two_spellings_share_a_copy_on_a_case_insensitive_volume() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let (upper, lower) = (
            tmp.path().join("W/Sheet.snxsch"),
            tmp.path().join("w/sheet.snxsch"),
        );
        assert_eq!(
            sidecar_path_with(&rec, &upper, INSENSITIVE),
            sidecar_path_with(&rec, &lower, INSENSITIVE)
        );
        put(&sidecar_path_with(&rec, &upper, INSENSITIVE), b"draft");
        adopt_legacy_with(&rec, &lower, INSENSITIVE).unwrap();
        assert_eq!(
            std::fs::read(sidecar_path_with(&rec, &lower, INSENSITIVE)).unwrap(),
            b"draft"
        );
    }

    #[test]
    fn two_files_do_not_share_a_copy_on_a_case_sensitive_volume() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let (upper, lower) = (
            tmp.path().join("Sheet.snxsch"),
            tmp.path().join("sheet.snxsch"),
        );
        assert_ne!(
            sidecar_path_with(&rec, &upper, SENSITIVE),
            sidecar_path_with(&rec, &lower, SENSITIVE)
        );
        put(&sidecar_path_with(&rec, &upper, SENSITIVE), b"upper");
        adopt_legacy_with(&rec, &lower, SENSITIVE).unwrap();
        assert!(!sidecar_path_with(&rec, &lower, SENSITIVE).exists());
        assert_eq!(
            std::fs::read(sidecar_path_with(&rec, &upper, SENSITIVE)).unwrap(),
            b"upper"
        );
    }

    #[test]
    fn a_copy_under_the_previous_as_given_key_is_moved_to_the_folded_key() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("Sheet.snxsch");
        let old = key_path(&rec, &doc, as_given_path_hash(&doc));
        put(&old, b"crash edits");
        adopt_legacy_with(&rec, &doc, INSENSITIVE).unwrap();
        let current = sidecar_path_with(&rec, &doc, INSENSITIVE);
        assert_eq!(std::fs::read(&current).unwrap(), b"crash edits");
        assert!(!old.exists(), "moved, not copied");
    }

    #[test]
    fn a_copy_under_the_previous_volume_folded_key_is_found() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("Sheet.snxsch");
        let folded = PathBuf::from(path_key_with(&doc, INSENSITIVE));
        let old = key_path(&rec, &folded, as_given_path_hash(&folded));
        put(&old, b"crash edits");
        adopt_legacy_with(&rec, &doc, INSENSITIVE).unwrap();
        assert_eq!(
            std::fs::read(sidecar_path_with(&rec, &doc, INSENSITIVE)).unwrap(),
            b"crash edits"
        );
    }

    #[test]
    fn with_an_old_and_a_new_copy_only_the_newest_is_offered_and_the_other_stays() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("Sheet.snxsch");
        let old = key_path(&rec, &doc, as_given_path_hash(&doc));
        let current = sidecar_path_with(&rec, &doc, INSENSITIVE);
        put(&old, b"old");
        std::thread::sleep(std::time::Duration::from_millis(50));
        put(&current, b"new");
        adopt_legacy_with(&rec, &doc, INSENSITIVE).unwrap();
        assert_eq!(std::fs::read(&current).unwrap(), b"new");
        assert_eq!(std::fs::read(&old).unwrap(), b"old", "left on disk");
        // And the other way round: the older-key copy is the newer one.
        std::thread::sleep(std::time::Duration::from_millis(50));
        put(&old, b"newest");
        adopt_legacy_with(&rec, &doc, INSENSITIVE).unwrap();
        assert_eq!(std::fs::read(&current).unwrap(), b"newest");
        let aside = format!("{}.{SUPERSEDED_EXTENSION}", current.display());
        assert_eq!(
            std::fs::read(aside).unwrap(),
            b"new",
            "set aside, not deleted"
        );
    }

    #[test]
    fn dot_segment_spellings_of_a_document_share_one_sidecar() {
        let dir = Path::new("rec");
        assert_eq!(
            sidecar_path(dir, Path::new("w/x/../Foo/./a.snxsch")),
            sidecar_path(dir, Path::new("w/Foo/a.snxsch"))
        );
    }

    #[test]
    fn rename_carries_the_copy_to_the_new_document_and_tolerates_none() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("recovery");
        let (old, new) = (
            tmp.path().join("a/main.snxsch"),
            tmp.path().join("b/main.snxsch"),
        );
        rename(&rec, &old, &new).unwrap();
        write(&rec, &old, b"draft").unwrap();
        rename(&rec, &old, &new).unwrap();
        assert!(!sidecar_path(&rec, &old).exists());
        assert_eq!(std::fs::read(sidecar_path(&rec, &new)).unwrap(), b"draft");
    }
}
