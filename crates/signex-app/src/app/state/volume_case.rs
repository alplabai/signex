//! Whether a directory's volume treats `A` and `a` as one name (#771).
//!
//! An existing path is canonicalised, so a case variant of it collapses onto
//! the real file. A path that does not exist yet (a sheet created but never
//! saved) has no real spelling: only its nearest existing ancestor does. On a
//! case-insensitive volume the missing tail must then be compared without
//! case, or `N.snxsch` and a sibling project's `../A/n.snxsch` read as two
//! files and one project's unsaved sheet is discarded with the other.
//!
//! The answer is per directory (#543: the host family is only a proxy), so it
//! is probed, which is disk IO: it runs inside the off-thread resolve and
//! never in `update`. The probe is injected ([`IgnoresCase`]) so both answers
//! run on every host, including the Linux-only CI job.
//!
//! A directory's own name is looked up in its PARENT, so flipping it measures
//! the parent's volume: wrong for a case-sensitive mount inside a
//! case-insensitive folder, or a FAT/exFAT stick under `/media`. The probe
//! therefore flips an entry INSIDE the directory.
//!
//! When no answer is possible (an empty directory with no ancestor on the
//! same volume holding a lettered entry, or an IO error) the probe answers
//! "ignores case". That is the safe side for data loss: variants are treated
//! as one file, so a sheet is kept rather than discarded.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Decides whether an EXISTING, canonical directory sits on a volume that
/// ignores case. [`directory_ignores_case`] in production; a constant in
/// tests.
pub(crate) type IgnoresCase = fn(&Path) -> bool;

/// The filesystem questions the probe asks, so a test can answer them for a
/// volume layout the host cannot build (a case-sensitive mount in a
/// case-insensitive folder).
trait CaseFs {
    /// A bounded sample of the entry names directly inside `dir`.
    fn entry_names(&self, dir: &Path) -> Vec<OsString>;
    /// Whether `b` names the same filesystem object as the real entry `a`.
    /// `Err` carries why `a` itself could not be stat'ed (deleted since the
    /// listing, permission denied): that says nothing about case, so it must
    /// not read as "different entry". A missing `b` is a plain `Ok(false)`.
    fn is_same_entry(&self, a: &Path, b: &Path) -> Result<bool, String>;
    /// `dir`'s parent when it exists and is on the same volume as `dir`.
    fn parent_on_same_volume(&self, dir: &Path) -> Option<PathBuf>;
}

/// How many entry names the probe looks at before giving up on a directory.
const MAX_ENTRIES_SAMPLED: usize = 256;

/// `Ok` when `entry` still exists. A flipped spelling that fails to resolve
/// only means "case-sensitive" if the original is still there: an entry that
/// vanished between the two lookups (another process cleaning its temp dir)
/// would otherwise read as a case-sensitive volume.
fn entry_still_there(entry: &Path) -> Result<(), String> {
    std::fs::symlink_metadata(entry)
        .map(|_| ())
        .map_err(|e| format!("entry vanished during the probe: {e}"))
}

struct RealFs;

impl CaseFs for RealFs {
    fn entry_names(&self, dir: &Path) -> Vec<OsString> {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    // A link is never probed: canonicalising it would follow
                    // it to a device or a network share.
                    .filter(|entry| entry.file_type().is_ok_and(|t| !t.is_symlink()))
                    .take(MAX_ENTRIES_SAMPLED)
                    .map(|entry| entry.file_name())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn is_same_entry(&self, a: &Path, b: &Path) -> Result<bool, String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let a_meta = std::fs::symlink_metadata(a).map_err(|e| format!("stat failed: {e}"))?;
            match std::fs::symlink_metadata(b) {
                Ok(b) => Ok(a_meta.dev() == b.dev() && a_meta.ino() == b.ino()),
                Err(_) => entry_still_there(a).map(|()| false),
            }
        }
        #[cfg(not(unix))]
        {
            let is_link = |p: &Path| std::fs::symlink_metadata(p).is_ok_and(|m| m.is_symlink());
            if is_link(a) || is_link(b) {
                return Err("is a link, not followed".to_string());
            }
            let a_real = std::fs::canonicalize(a).map_err(|e| format!("stat failed: {e}"))?;
            match std::fs::canonicalize(b) {
                Ok(b) => Ok(a_real == b),
                Err(_) => entry_still_there(a).map(|()| false),
            }
        }
    }

    fn parent_on_same_volume(&self, dir: &Path) -> Option<PathBuf> {
        let parent = dir.parent().filter(|p| !p.as_os_str().is_empty())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let same = std::fs::metadata(dir).ok()?.dev() == std::fs::metadata(parent).ok()?.dev();
            same.then(|| parent.to_path_buf())
        }
        #[cfg(not(unix))]
        {
            Some(parent.to_path_buf())
        }
    }
}

/// Probe `dir` by asking the filesystem for a case-flipped spelling of an
/// entry inside it: the volume ignores case when that spelling is the same
/// entry. See the module docs for the fallback.
pub(crate) fn directory_ignores_case(dir: &Path) -> bool {
    probe_with(&RealFs, dir)
}

/// [`directory_ignores_case`] without the safe-side fallback: `None` when the
/// volume could not be asked (a missing, unreadable or lettered-entry-free
/// directory with no usable ancestor), so a caller can tell the user instead
/// of silently guessing (#694). `Err` is an undecided answer with a reason
/// (the real entry could not be stat'ed), so the warning can say why.
pub(crate) fn probe_directory_case(dir: &Path) -> Result<Option<bool>, String> {
    probe_tri_with(&RealFs, dir)
}

fn probe_with(fs: &impl CaseFs, dir: &Path) -> bool {
    probe_tri_with(fs, dir).ok().flatten().unwrap_or(true)
}

fn probe_tri_with(fs: &impl CaseFs, dir: &Path) -> Result<Option<bool>, String> {
    let mut current = dir.to_path_buf();
    loop {
        // An entry that cannot be stat'ed (deleted since the listing: a shared
        // temp folder churns) says nothing about case, so the next entry is
        // asked; the answer must not flip with another process's cleanup.
        let mut undecided = None;
        for name in fs.entry_names(&current) {
            if let Some(flipped) = flip_ascii_case(&name) {
                match fs.is_same_entry(&current.join(&name), &current.join(flipped)) {
                    Ok(same) => return Ok(Some(same)),
                    Err(reason) => undecided = undecided.or(Some(reason)),
                }
            }
        }
        if let Some(reason) = undecided {
            return Err(reason);
        }
        match fs.parent_on_same_volume(&current) {
            Some(parent) => current = parent,
            None => return Ok(None),
        }
    }
}

/// `name` with the case of every ASCII letter swapped, or `None` when it has
/// none (so nothing can be probed). Only ASCII: full Unicode case folding
/// differs per volume, and a mismatch there would read as case-sensitive.
fn flip_ascii_case(name: &OsStr) -> Option<OsString> {
    let text = name.to_str()?;
    if !text.chars().any(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let flipped: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    Some(flipped.into())
}

/// Answers [`IgnoresCase`] once per directory for the length of one resolve.
pub(crate) struct VolumeCase {
    probe: IgnoresCase,
    memo: HashMap<PathBuf, bool>,
}

impl VolumeCase {
    pub(crate) fn new(probe: IgnoresCase) -> Self {
        Self {
            probe,
            memo: HashMap::new(),
        }
    }

    /// True when the existing directory `dir` ignores case.
    pub(crate) fn ignores_case(&mut self, dir: &Path) -> bool {
        let probe = self.probe;
        *self
            .memo
            .entry(dir.to_path_buf())
            .or_insert_with(|| probe(dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A volume tree: each directory is case-sensitive or not, holds some
    /// entries, and says whether its parent shares its volume.
    #[derive(Default)]
    struct FakeFs {
        dirs: HashMap<PathBuf, FakeDir>,
    }

    struct FakeDir {
        sensitive: bool,
        entries: Vec<&'static str>,
        parent_on_same_volume: bool,
        /// Every stat of an entry here fails, as on an unreadable share.
        stat_fails: bool,
        /// This one entry has vanished since the listing; its stat fails.
        vanished: Option<&'static str>,
    }

    impl FakeFs {
        fn dir(
            mut self,
            path: &str,
            sensitive: bool,
            entries: &[&'static str],
            parent_on_same_volume: bool,
        ) -> Self {
            self.dirs.insert(
                PathBuf::from(path),
                FakeDir {
                    sensitive,
                    entries: entries.to_vec(),
                    parent_on_same_volume,
                    stat_fails: false,
                    vanished: None,
                },
            );
            self
        }

        fn with_vanished_entry(mut self, path: &str, entry: &'static str) -> Self {
            self.dirs.get_mut(Path::new(path)).unwrap().vanished = Some(entry);
            self
        }

        fn with_failing_stat(mut self, path: &str) -> Self {
            self.dirs.get_mut(Path::new(path)).unwrap().stat_fails = true;
            self
        }
    }

    impl CaseFs for FakeFs {
        fn entry_names(&self, dir: &Path) -> Vec<OsString> {
            self.dirs
                .get(dir)
                .map_or_else(Vec::new, |d| d.entries.iter().map(OsString::from).collect())
        }

        /// Looked up in the directory that holds `a`: a case-sensitive one
        /// needs the exact name, an insensitive one folds ASCII case.
        fn is_same_entry(&self, a: &Path, b: &Path) -> Result<bool, String> {
            let Some(dir) = a.parent().and_then(|p| self.dirs.get(p)) else {
                return Err("stat failed: no such directory".to_string());
            };
            let (a, b) = (a.file_name().unwrap(), b.file_name().unwrap());
            if dir.stat_fails {
                return Err("stat failed: Permission denied (os error 13)".to_string());
            }
            if dir.vanished.is_some_and(|gone| OsStr::new(gone) == a) {
                return Err("stat failed: entry vanished".to_string());
            }
            let a_exists = dir.entries.iter().any(|e| OsStr::new(e) == a);
            Ok(a_exists && (a == b || (!dir.sensitive && a.eq_ignore_ascii_case(b))))
        }

        fn parent_on_same_volume(&self, dir: &Path) -> Option<PathBuf> {
            let same = self.dirs.get(dir)?.parent_on_same_volume;
            dir.parent().filter(|_| same).map(Path::to_path_buf)
        }
    }

    #[test]
    fn a_case_sensitive_mount_inside_a_case_insensitive_folder_is_case_sensitive() {
        // `/Media` ignores case, and so does the NAME `usb` there; the
        // mount's own entries do not.
        let fs = FakeFs::default().dir("/Media", false, &["usb"], true).dir(
            "/Media/usb",
            true,
            &["Data.bin"],
            false,
        );

        assert!(!probe_with(&fs, Path::new("/Media/usb")));
    }

    #[test]
    fn a_case_insensitive_stick_inside_a_case_sensitive_folder_ignores_case() {
        let fs = FakeFs::default().dir("/media", true, &["STICK"], true).dir(
            "/media/STICK",
            false,
            &["Report.TXT"],
            false,
        );

        assert!(probe_with(&fs, Path::new("/media/STICK")));
    }

    #[test]
    fn an_empty_directory_asks_the_nearest_ancestor_on_the_same_volume() {
        let fs = FakeFs::default()
            .dir("/vol", true, &["Notes"], false)
            .dir("/vol/a", true, &[], true)
            .dir("/vol/a/empty", true, &[], true);

        // Nothing lettered in `empty` or `a`, so the answer is `/vol`'s.
        assert!(!probe_with(&fs, Path::new("/vol/a/empty")));
    }

    #[test]
    fn an_ancestor_across_a_mount_boundary_is_not_asked() {
        // `/mnt/empty` is its own volume, so `/mnt` says nothing about it:
        // no answer, the safe side.
        let fs = FakeFs::default().dir("/mnt", true, &["Data"], false).dir(
            "/mnt/empty",
            true,
            &[],
            false,
        );

        assert!(probe_with(&fs, Path::new("/mnt/empty")));
    }

    #[test]
    fn an_entry_without_letters_is_skipped_for_one_with_letters() {
        let fs = FakeFs::default().dir("/d", true, &["123", ".#", "Abc"], false);

        assert!(!probe_with(&fs, Path::new("/d")));
    }

    #[test]
    fn with_no_answer_possible_the_probe_ignores_case_so_a_sheet_is_kept() {
        let fs = FakeFs::default().dir("/d", true, &["123"], false);

        assert!(probe_with(&fs, Path::new("/d")));
        assert!(probe_with(&FakeFs::default(), Path::new("/absent")));
    }

    #[test]
    fn the_tri_state_probe_tells_insensitive_sensitive_and_undecidable_apart() {
        let insensitive = FakeFs::default().dir("/ci", false, &["Top.snxsch"], false);
        let sensitive = FakeFs::default().dir("/cs", true, &["Top.snxsch"], false);
        let undecidable = FakeFs::default().dir("/none", true, &["123"], false);

        assert_eq!(
            probe_tri_with(&insensitive, Path::new("/ci")),
            Ok(Some(true))
        );
        assert_eq!(
            probe_tri_with(&sensitive, Path::new("/cs")),
            Ok(Some(false))
        );
        assert_eq!(probe_tri_with(&undecidable, Path::new("/none")), Ok(None));
        assert_eq!(
            probe_tri_with(&FakeFs::default(), Path::new("/absent")),
            Ok(None)
        );
        // The bool wrapper keeps folding "undecidable" into "ignores case".
        assert!(probe_with(&undecidable, Path::new("/none")));
    }

    #[test]
    fn a_failing_stat_of_the_real_entry_is_undecided_not_case_sensitive() {
        // `/share` ignores case, but its entry cannot be stat'ed (deleted
        // since the listing, permission denied): the old `false` read as a
        // confident "case-sensitive" and folded wrongly (#970).
        let fs = FakeFs::default()
            .dir("/share", false, &["Top.snxsch"], false)
            .with_failing_stat("/share");

        let reason = probe_tri_with(&fs, Path::new("/share")).expect_err("undecided");

        assert!(reason.contains("Permission denied"), "{reason}");
        // The bool wrapper stays on the data-loss-safe side.
        assert!(probe_with(&fs, Path::new("/share")));
    }

    #[test]
    fn an_entry_that_vanished_since_the_listing_does_not_change_the_answer() {
        // Another process deleted `Gone.tmp` between the listing and the
        // stat (#1851): the next entry answers, exactly as without the churn.
        let churned = FakeFs::default()
            .dir("/tmp", false, &["Gone.tmp", "Stays.tmp"], false)
            .with_vanished_entry("/tmp", "Gone.tmp");
        let calm = FakeFs::default().dir("/tmp", false, &["Stays.tmp"], false);

        assert_eq!(
            probe_tri_with(&churned, Path::new("/tmp")),
            probe_tri_with(&calm, Path::new("/tmp"))
        );
        assert_eq!(probe_tri_with(&churned, Path::new("/tmp")), Ok(Some(true)));
    }

    #[test]
    fn flipping_swaps_ascii_letters_only() {
        assert_eq!(
            flip_ascii_case(OsStr::new("aB1.txt")),
            Some(OsString::from("Ab1.TXT"))
        );
        assert_eq!(flip_ascii_case(OsStr::new("123.")), None);
        assert_eq!(flip_ascii_case(OsStr::new("\u{e9}\u{e8}")), None);
    }

    #[test]
    fn the_real_probe_agrees_with_what_the_volume_does_with_a_case_variant_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("Probe");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("Lower.tmp"), b"x").expect("write");
        // Independent oracle: does the case-flipped file name find the file?
        let volume_ignores_case = dir.join("lOWER.tmp").exists();

        assert_eq!(directory_ignores_case(&dir), volume_ignores_case);
    }

    #[cfg(windows)]
    #[test]
    fn a_sibling_link_is_never_sampled_or_canonicalised_by_the_probe() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("Zed.tmp"), b"x").expect("write");
        // An unresolvable host: nothing can answer for it, whatever the LAN does.
        let link = tmp.path().join("aaa");
        if let Err(e) = std::os::windows::fs::symlink_file(r"\\nonexistent.invalid\s\x", &link) {
            assert_eq!(e.raw_os_error(), Some(1314), "{e}");
            eprintln!("skipped: cannot create symlinks ({e})");
            return;
        }

        assert_eq!(
            RealFs.entry_names(tmp.path()),
            vec![OsString::from("Zed.tmp")]
        );
        assert!(RealFs.is_same_entry(&link, &link).is_err());
        assert_eq!(
            directory_ignores_case(tmp.path()),
            tmp.path().join("zED.tmp").exists()
        );
    }

    #[test]
    fn the_real_probe_of_an_empty_directory_asks_its_ancestor_on_the_same_volume() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("Marker.tmp"), b"x").expect("write");
        let empty = tmp.path().join("Empty");
        std::fs::create_dir_all(&empty).expect("mkdir");
        let volume_ignores_case = tmp.path().join("mARKER.tmp").exists();

        assert_eq!(directory_ignores_case(&empty), volume_ignores_case);
    }

    #[test]
    fn a_missing_directory_has_no_answer_so_it_ignores_case() {
        let tmp = tempfile::tempdir().expect("tempdir");

        assert!(directory_ignores_case(
            &tmp.path().join("Nope").join("Deeper")
        ));
    }

    #[test]
    fn the_answer_is_probed_once_per_directory() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn counting(_: &Path) -> bool {
            CALLS.fetch_add(1, Ordering::SeqCst);
            true
        }
        let mut volume = VolumeCase::new(counting);

        assert!(volume.ignores_case(Path::new("/x")));
        assert!(volume.ignores_case(Path::new("/x")));

        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }
}
