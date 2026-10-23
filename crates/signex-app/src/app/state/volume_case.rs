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

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Decides whether an EXISTING, canonical directory sits on a volume that
/// ignores case. [`directory_ignores_case`] in production; a constant in
/// tests.
pub(crate) type IgnoresCase = fn(&Path) -> bool;

/// Probe `dir` by asking the filesystem for a case-flipped spelling of it:
/// the volume ignores case when that spelling is the same directory.
///
/// Any failure (no such directory, a flipped spelling that names nothing or a
/// different directory) answers `false`, the case-sensitive reading that
/// never merges two files.
pub(crate) fn directory_ignores_case(dir: &Path) -> bool {
    let Ok(real) = std::fs::canonicalize(dir) else {
        return false;
    };
    let Some(flipped) = with_last_cased_component_flipped(&real) else {
        return false;
    };
    std::fs::canonicalize(&flipped).is_ok_and(|same| same == real)
}

/// `path` with its last component that has a letter case-flipped, or `None`
/// when no component has one (so nothing can be probed).
fn with_last_cased_component_flipped(path: &Path) -> Option<PathBuf> {
    let mut parts: Vec<std::ffi::OsString> = path
        .components()
        .map(|c| c.as_os_str().to_os_string())
        .collect();
    let index = parts
        .iter()
        .rposition(|p| flip_case(p).is_some_and(|f| f.as_os_str() != p.as_os_str()))?;
    parts[index] = flip_case(&parts[index])?;
    Some(parts.iter().collect())
}

fn flip_case(name: &OsStr) -> Option<std::ffi::OsString> {
    let text = name.to_str()?;
    Some(
        text.chars()
            .map(|c| {
                if c.is_lowercase() {
                    c.to_uppercase().collect::<String>()
                } else {
                    c.to_lowercase().collect::<String>()
                }
            })
            .collect::<String>()
            .into(),
    )
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

    #[test]
    fn the_probe_agrees_with_what_the_volume_does_with_a_case_variant_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("Probe");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("Lower.tmp"), b"x").expect("write");
        // Independent oracle: does the case-flipped file name find the file?
        let volume_ignores_case = dir.join("lOWER.tmp").exists();

        assert_eq!(directory_ignores_case(&dir), volume_ignores_case);
    }

    #[test]
    fn a_missing_directory_is_never_reported_as_ignoring_case() {
        let tmp = tempfile::tempdir().expect("tempdir");

        assert!(!directory_ignores_case(&tmp.path().join("Nope")));
    }

    #[test]
    fn a_path_with_no_letters_cannot_be_probed() {
        assert!(with_last_cased_component_flipped(Path::new("/")).is_none());
        assert_eq!(
            with_last_cased_component_flipped(Path::new("/a/1")),
            Some(PathBuf::from("/A/1")),
        );
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
