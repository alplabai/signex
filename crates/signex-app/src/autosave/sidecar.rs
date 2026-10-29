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

use super::keys::{self, KeyForm};
use super::pending_move;

/// Extension that marks a file as a recovery sidecar.
pub const SIDECAR_EXTENSION: &str = "recovery";

/// Extension appended to a copy that a newer copy of the same document
/// displaced; it is kept on disk but never offered.
const SUPERSEDED_EXTENSION: &str = "older";

/// Bring a copy written under any other key form of `document` (see
/// [`keys::all_keys`]) to its current key, so a changed case rule, an upgrade,
/// or another spelling does not strand it. Nothing happens when no such copy
/// exists. Where a copy already sits at the current key, only the newest of
/// the two is offered: if the other copy is newer, the current one is set
/// aside and the other adopted; otherwise it stays where it is. No copy is
/// ever deleted or overwritten.
///
/// A copy under a key that merges spellings differing in case is not moved
/// while another existing path spells the document differently up to case: it
/// could belong to that file. It is left where it is and logged.
pub fn adopt_legacy(recovery_dir: &Path, document: &Path) -> Result<(), String> {
    adopt_legacy_with(recovery_dir, document, keys::stable_ignores_case(document))
}

fn adopt_legacy_with(
    recovery_dir: &Path,
    document: &Path,
    ignores_case: bool,
) -> Result<(), String> {
    let current = sidecar_path_with(recovery_dir, document, ignores_case);
    let current_form = KeyForm::Keyed(ignores_case);
    for (form, legacy) in keys::all_keys(recovery_dir, document, ignores_case) {
        if legacy == current || !legacy.is_file() {
            continue;
        }
        if form.merges_spellings()
            && !current_form.merges_spellings()
            && keys::has_case_variant(document)
        {
            crate::diagnostics::log_warning(format!(
                "The recovery copy {} may belong to {} or to an existing file that spells it \
                 differently up to case; it was left where it is.",
                legacy.display(),
                document.display()
            ));
            continue;
        }
        adopt_one(&current, &legacy, document, false)?;
    }
    Ok(())
}

/// Log each recovery file named for `document` up to case that no key form of
/// it matches, so a copy stranded by an unknown spelling can be found by hand.
pub(super) fn log_unmatched_spellings(recovery_dir: &Path, document: &Path) {
    let Some(name) = document
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
    else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(recovery_dir) else {
        return;
    };
    let known = keys::all_keys(recovery_dir, document, keys::stable_ignores_case(document));
    let suffix = format!(".{SIDECAR_EXTENSION}");
    for path in entries.flatten().map(|e| e.path()) {
        let file = path.file_name().map(|n| n.to_string_lossy().to_lowercase());
        let stem = file
            .as_deref()
            .and_then(|f| f.strip_suffix(suffix.as_str()))
            .and_then(|f| f.rsplit_once('-'))
            .map(|(stem, _)| stem);
        if stem == Some(name.as_str()) && known.iter().all(|(_, k)| *k != path) {
            crate::diagnostics::log_warning(format!(
                "The recovery copy {} is named like {} but is not filed under any key of it; \
                 it was not offered.",
                path.display(),
                document.display()
            ));
        }
    }
}

/// Move `from` to `to` without ever replacing a file at `to`: a hard link is
/// created first (it fails when `to` exists), then the old name is dropped.
/// Where links are unsupported, an existing `to` is refused and the rename
/// is used.
pub(super) fn move_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::hard_link(from, to) {
        Ok(()) => {
            if let Err(e) = std::fs::remove_file(from) {
                crate::diagnostics::log_warning(format!(
                    "{e}: the recovery copy {} now also exists as {}",
                    from.display(),
                    to.display()
                ));
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(e),
        Err(_) if to.exists() => Err(std::io::ErrorKind::AlreadyExists.into()),
        Err(_) => std::fs::rename(from, to),
    }
}

/// A move found `dst` already holding a copy: keep the newest under `dst` and
/// set the other aside as `.older`, so neither is deleted and the new path is
/// offered the newest edits. For a moved document's copy, not an old key.
pub(super) fn resolve_taken(dst: &Path, src: &Path, document: &Path) -> Result<(), String> {
    adopt_one(dst, src, document, true)
}

/// Rename `from` to a free `.older` name next to `current` (never replacing one).
fn set_aside(from: &Path, current: &Path) -> Result<PathBuf, String> {
    // Never overwrite an earlier set-aside copy: claim the first free
    // name (`.older`, `.older.1`, ...) by creating it with `create_new`,
    // which fails if it exists, so two processes cannot pick one name.
    // With none free the move is refused, so the document stays
    // protected by what is on disk.
    let aside = claim_aside_name(current)?;
    if let Err(e) = std::fs::rename(from, &aside) {
        // Do not leave the empty claim behind.
        if let Err(cleanup) = std::fs::remove_file(&aside) {
            crate::diagnostics::log_warning(format!(
                "{cleanup}: could not remove the unused claim {}",
                aside.display()
            ));
        }
        return Err(format!(
            "{e}: could not set aside recovery copy {} as {}",
            from.display(),
            aside.display()
        ));
    }
    Ok(aside)
}

fn adopt_one(
    current: &Path,
    legacy: &Path,
    document: &Path,
    aside_loser: bool,
) -> Result<(), String> {
    let (current, legacy) = (current.to_path_buf(), legacy.to_path_buf());
    // Both names of one file (a move that stopped between linking and
    // unlinking): drop the stale name, the content is kept under the other.
    if current.exists() && same_file::is_same_file(&legacy, &current).unwrap_or(false) {
        return std::fs::remove_file(&legacy).map_err(|e| {
            format!(
                "{e}: could not drop the second name {} of recovery copy {}",
                legacy.display(),
                current.display()
            )
        });
    }
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
            let aside = set_aside(&current, &current)?;
            crate::diagnostics::log_warning(format!(
                "The recovery copy {} of {} was older than {}; it was kept as {}.",
                current.display(),
                document.display(),
                legacy.display(),
                aside.display()
            ));
        } else if aside_loser {
            let aside = set_aside(&legacy, &current)?;
            crate::diagnostics::log_warning(format!(
                "The recovery copy {} of {} was older than {}; it was kept as {}.",
                legacy.display(),
                document.display(),
                current.display(),
                aside.display()
            ));
            return Ok(());
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
    move_no_replace(&legacy, &current).map_err(|e| {
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
/// Probes the volume (disk IO): call it from a task, never from `update`.
pub fn sidecar_path(recovery_dir: &Path, document: &Path) -> PathBuf {
    sidecar_path_with(recovery_dir, document, keys::stable_ignores_case(document))
}

/// [`sidecar_path`] under an explicit case rule.
fn sidecar_path_with(recovery_dir: &Path, document: &Path, ignores_case: bool) -> PathBuf {
    keys::key_for(recovery_dir, document, KeyForm::Keyed(ignores_case))
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
    // A copy of `from` still under another key form is brought to the current
    // key first. If that fails the move stays owed: journalled, and reported.
    if let Err(why) = adopt_legacy(recovery_dir, from) {
        if let Err(reason) = pending_move::record(recovery_dir, from, to) {
            crate::diagnostics::log_warning(reason);
        }
        return Err(why);
    }
    // Only the current key: a copy `adopt_legacy` left under a case-merging
    // key because it may belong to another file is not this document's to move.
    let src = Some(sidecar_path(recovery_dir, from)).filter(|key| key.is_file());
    let dst = sidecar_path(recovery_dir, to);
    let Some(src) = src.filter(|src| *src != dst) else {
        // Nothing to move: an entry for `from` would only mislead.
        pending_move::clear(recovery_dir, from);
        return Ok(());
    };
    // A journal that cannot be written must not block the move itself.
    if let Err(reason) = pending_move::record(recovery_dir, from, to) {
        crate::diagnostics::log_warning(reason);
    }
    match move_no_replace(&src, &dst) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        // The new path already holds a copy: the newest is offered under it
        // and the other is set aside, never deleted. On failure the journal
        // entry stays and the move is retried.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && dst.is_file() => {
            resolve_taken(&dst, &src, to)?;
        }
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
        let legacy = keys::key_for(&rec, &doc, KeyForm::Raw);
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
        let legacy = keys::key_for(&rec, &doc, KeyForm::Raw);
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
        let legacy = keys::key_for(&rec, &doc, KeyForm::Raw);
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
        let legacy = keys::key_for(&rec, &doc, KeyForm::Raw);
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
        let legacy = keys::key_for(&rec, &doc, KeyForm::Raw);
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

    // The case rule is injected, so both volume kinds run on every host.
    const INSENSITIVE: bool = true;
    const SENSITIVE: bool = false;

    fn put(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn key(rec: &Path, doc: &Path, form: KeyForm) -> PathBuf {
        keys::key_for(rec, doc, form)
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
    fn an_undecided_probe_keeps_case_and_a_missing_folder_gives_the_same_answer() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join("Gone/Sheet.snxsch");
        // The folder does not exist: the nearest existing one is asked, not
        // "undecided means fold".
        let before = keys::stable_ignores_case(&doc);
        std::fs::create_dir_all(doc.parent().unwrap()).unwrap();
        std::fs::write(&doc, b"x").unwrap();
        assert_eq!(keys::stable_ignores_case(&doc), before);
        std::fs::remove_dir_all(doc.parent().unwrap()).unwrap();
        assert_eq!(keys::stable_ignores_case(&doc), before);
    }

    #[test]
    fn a_copy_filed_under_the_other_case_rule_is_found_either_way_round() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("Sheet.snxsch");
        // Filed while the rule read "keep case"; the rule now says "fold".
        put(&key(&rec, &doc, KeyForm::Keyed(SENSITIVE)), b"edits");
        adopt_legacy_with(&rec, &doc, INSENSITIVE).unwrap();
        assert_eq!(
            std::fs::read(sidecar_path_with(&rec, &doc, INSENSITIVE)).unwrap(),
            b"edits"
        );
        // Filed while it read "fold" (folded key); the rule now says "keep case".
        let other = tmp.path().join("Other.snxsch");
        put(&key(&rec, &other, KeyForm::Keyed(INSENSITIVE)), b"folded");
        adopt_legacy_with(&rec, &other, SENSITIVE).unwrap();
        assert_eq!(
            std::fs::read(sidecar_path_with(&rec, &other, SENSITIVE)).unwrap(),
            b"folded"
        );
    }

    #[test]
    fn a_folded_key_copy_is_not_adopted_while_a_case_variant_file_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let (upper, lower) = (
            tmp.path().join("Sheet.snxsch"),
            tmp.path().join("sheet.snxsch"),
        );
        std::fs::write(&upper, b"a").unwrap();
        if !lower.exists() {
            // A case-insensitive folder cannot hold both: nothing to test.
            std::fs::write(&lower, b"b").unwrap();
        }
        if std::fs::read(&upper).unwrap() != b"a" {
            return;
        }
        let folded = key(&rec, &upper, KeyForm::Keyed(INSENSITIVE));
        put(&folded, b"whose?");
        adopt_legacy_with(&rec, &lower, SENSITIVE).unwrap();
        assert!(folded.is_file(), "left on disk, not moved");
        assert!(!sidecar_path_with(&rec, &lower, SENSITIVE).exists());
    }

    #[test]
    fn a_copy_under_the_previous_as_given_key_is_moved_to_the_folded_key() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("Sheet.snxsch");
        let old = key(&rec, &doc, KeyForm::AsGiven);
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
        let old = key(&rec, &doc, KeyForm::FoldedAsGiven);
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
        let old = key(&rec, &doc, KeyForm::AsGiven);
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
    fn a_move_never_replaces_a_file_at_the_target() {
        let tmp = tempfile::tempdir().unwrap();
        let (from, to) = (tmp.path().join("from"), tmp.path().join("to"));
        std::fs::write(&from, b"a").unwrap();
        std::fs::write(&to, b"b").unwrap();
        assert!(move_no_replace(&from, &to).is_err());
        assert_eq!(std::fs::read(&to).unwrap(), b"b");
        assert_eq!(std::fs::read(&from).unwrap(), b"a");
        std::fs::remove_file(&to).unwrap();
        move_no_replace(&from, &to).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), b"a");
        assert!(!from.exists());
    }

    #[test]
    fn a_failed_adopt_keeps_the_move_owed_and_reports_it() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let (old, new) = (
            tmp.path().join("a/main.snxsch"),
            tmp.path().join("b/main.snxsch"),
        );
        // Two raw/as-given copies and every aside name taken: adopt fails.
        let current = sidecar_path(&rec, &old);
        let legacy = key(&rec, &old, KeyForm::Raw);
        let alt = if legacy == current {
            key(&rec, &old, KeyForm::FoldedRaw)
        } else {
            legacy
        };
        if alt == current {
            return;
        }
        put(&current, b"old");
        let base = format!("{}.{SUPERSEDED_EXTENSION}", current.display());
        std::fs::write(&base, b"x").unwrap();
        for n in 1..100u32 {
            std::fs::write(format!("{base}.{n}"), b"x").unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&alt, b"new").unwrap();
        assert!(rename(&rec, &old, &new).is_err());
        assert!(
            pending_move::owed(&rec)
                .iter()
                .any(|(o, n)| *o == old && *n == new)
        );
        assert_eq!(std::fs::read(&alt).unwrap(), b"new");
    }

    #[test]
    fn renaming_one_of_two_case_variant_files_never_takes_the_others_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        // B is the lower-case twin of A's WHOLE path: A's folded key is B's key.
        let a = tmp.path().join("Dir/Sheet.snxsch");
        let b = PathBuf::from(a.to_string_lossy().to_lowercase());
        let main = tmp.path().join("Dir/Main.snxsch");
        std::fs::create_dir_all(a.parent().unwrap()).unwrap();
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        if std::fs::read(&a).unwrap() != b"a" {
            return; // a case-insensitive volume cannot hold both
        }
        write(&rec, &b, b"b's edits").unwrap();
        let copy = sidecar_path(&rec, &b);
        rename(&rec, &a, &main).unwrap();
        assert_eq!(
            std::fs::read(&copy).unwrap(),
            b"b's edits",
            "B keeps its copy"
        );
        assert!(
            !sidecar_path(&rec, &main).exists(),
            "Main is offered nothing"
        );
    }

    #[test]
    fn a_folded_key_copy_is_not_moved_by_a_rename_while_a_case_variant_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let dir = tmp.path().join("w");
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("Sheet.snxsch"), dir.join("sheet.snxsch"));
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        if std::fs::read(&a).unwrap() != b"a" {
            return;
        }
        // B's copy sits under the folded key, which A's lookups also reach.
        let folded = key(&rec, &b, KeyForm::Keyed(INSENSITIVE));
        put(&folded, b"b's edits");
        rename(&rec, &a, &dir.join("Main.snxsch")).unwrap();
        assert!(folded.is_file());
        assert!(!sidecar_path(&rec, &dir.join("Main.snxsch")).exists());
    }

    #[test]
    fn a_second_name_of_the_same_copy_is_dropped_not_adopted() {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let doc = tmp.path().join("Sheet.snxsch");
        let current = sidecar_path_with(&rec, &doc, SENSITIVE);
        let legacy = key(&rec, &doc, KeyForm::Raw);
        let legacy = if legacy == current {
            key(&rec, &doc, KeyForm::FoldedRaw)
        } else {
            legacy
        };
        if legacy == current {
            return;
        }
        put(&current, b"edits");
        std::fs::hard_link(&current, &legacy).unwrap();
        adopt_legacy_with(&rec, &doc, SENSITIVE).unwrap();
        assert!(!legacy.exists());
        assert_eq!(std::fs::read(&current).unwrap(), b"edits");
        assert!(!Path::new(&format!("{}.{SUPERSEDED_EXTENSION}", current.display())).exists());
    }

    /// A moved document's copy at `a`, a stale copy already at `x`'s key; which
    /// is newer decides the winner. Returns (rec, a, x).
    fn taken_target(a_is_newer: bool) -> (PathBuf, PathBuf, PathBuf, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let rec = tmp.path().join("rec");
        let (a, x) = (tmp.path().join("A.snxsch"), tmp.path().join("X.snxsch"));
        let pause = || std::thread::sleep(std::time::Duration::from_millis(50));
        if a_is_newer {
            write(&rec, &x, b"stale").unwrap();
            pause();
            write(&rec, &a, b"newer").unwrap();
        } else {
            write(&rec, &a, b"older").unwrap();
            pause();
            write(&rec, &x, b"stale but newer").unwrap();
        }
        (rec, a, x, tmp)
    }

    fn aside_of(rec: &Path, x: &Path) -> Vec<u8> {
        std::fs::read(format!(
            "{}.{SUPERSEDED_EXTENSION}",
            sidecar_path(rec, x).display()
        ))
        .unwrap()
    }

    #[test]
    fn a_live_move_onto_a_taken_key_offers_the_newest_and_sets_the_other_aside() {
        let (rec, a, x, _tmp) = taken_target(true);
        rename(&rec, &a, &x).unwrap();
        assert_eq!(std::fs::read(sidecar_path(&rec, &x)).unwrap(), b"newer");
        assert_eq!(aside_of(&rec, &x), b"stale");
        assert!(!sidecar_path(&rec, &a).exists());
        assert!(pending_move::owed(&rec).is_empty());

        let (rec, a, x, _tmp) = taken_target(false);
        rename(&rec, &a, &x).unwrap();
        assert_eq!(
            std::fs::read(sidecar_path(&rec, &x)).unwrap(),
            b"stale but newer"
        );
        assert_eq!(aside_of(&rec, &x), b"older", "set aside, not deleted");
        assert!(!sidecar_path(&rec, &a).exists());
    }

    #[test]
    fn a_replayed_move_onto_a_taken_key_offers_the_newest_and_sets_the_other_aside() {
        for a_is_newer in [true, false] {
            let (rec, a, x, _tmp) = taken_target(a_is_newer);
            std::fs::write(&x, b"saved").unwrap();
            pending_move::record(&rec, &a, &x).unwrap();
            pending_move::finish_all(&rec);
            let (won, lost): (&[u8], &[u8]) = if a_is_newer {
                (b"newer", b"stale")
            } else {
                (b"stale but newer", b"older")
            };
            assert_eq!(std::fs::read(sidecar_path(&rec, &x)).unwrap(), won);
            assert_eq!(aside_of(&rec, &x), lost);
            assert!(!sidecar_path(&rec, &a).exists());
            assert!(pending_move::owed(&rec).is_empty());
        }
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
