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
    /// True when `a` and `b` both exist and are the same filesystem object.
    fn is_same_entry(&self, a: &Path, b: &Path) -> bool;
    /// `dir`'s parent when it exists and is on the same volume as `dir`.
    fn parent_on_same_volume(&self, dir: &Path) -> Option<PathBuf>;
}

/// How many entry names the probe looks at before giving up on a directory.
const MAX_ENTRIES_SAMPLED: usize = 256;

struct RealFs;

impl CaseFs for RealFs {
    fn entry_names(&self, dir: &Path) -> Vec<OsString> {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .take(MAX_ENTRIES_SAMPLED)
                    .map(|entry| entry.file_name())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn is_same_entry(&self, a: &Path, b: &Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
                (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
                _ => false,
            }
        }
        #[cfg(not(unix))]
        {
            match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
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

fn probe_with(fs: &impl CaseFs, dir: &Path) -> bool {
    let mut current = dir.to_path_buf();
    loop {
        for name in fs.entry_names(&current) {
            if let Some(flipped) = flip_ascii_case(&name) {
                return fs.is_same_entry(&current.join(&name), &current.join(flipped));
            }
        }
        match fs.parent_on_same_volume(&current) {
            Some(parent) => current = parent,
            None => return true,
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
                },
            );
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
        fn is_same_entry(&self, a: &Path, b: &Path) -> bool {
            let Some(dir) = a.parent().and_then(|p| self.dirs.get(p)) else {
                return false;
            };
            let (a, b) = (a.file_name().unwrap(), b.file_name().unwrap());
            let a_exists = dir.entries.iter().any(|e| OsStr::new(e) == a);
            a_exists && (a == b || (!dir.sensitive && a.eq_ignore_ascii_case(b)))
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
