//! Whether a sheet or library path is, or passes through, a link to somewhere
//! the walk must not go (#1020).
//!
//! [`super::reference_policy`] judges a path by its spelling, so a symlink or
//! junction spelled as a plain local file could still lead to a device
//! (`\.\NUL`, a raw volume) or an untrusted network share. Opening or
//! canonicalising such a path follows the link, and for a share that sends the
//! user's NTLM credentials to the named host (#771) before any check on the
//! resolved name could run. So the links are read, never followed: each
//! component is `symlink_metadata`d (which does not follow the last component),
//! a link's target is read with `read_link` and classified by spelling, and
//! only then is resolution continued from that target.
//!
//! This is a check, not a lock: a link swapped in between this check and the
//! later open is not caught here.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use super::reference_policy::{Share, is_path_refused};

/// The most links followed while resolving one path, as in the OS's own limit.
const MAX_LINK_HOPS: usize = 63;

/// True when `path`, or any directory on the way to it, is a link (symlink or
/// junction) whose target is a device, an untrusted network share, or a chain
/// of more than [`MAX_LINK_HOPS`] links. A plain path is never refused here
/// even if it canonicalises onto a share. Only links are read from disk; no
/// path is opened. `false` for a path that does not exist.
pub(crate) fn link_leads_to_refused(path: &Path, trusted: &[Share]) -> bool {
    let mut pending: VecDeque<OsString> = path
        .components()
        .map(|c| c.as_os_str().to_owned())
        .collect();
    let mut resolved = PathBuf::new();
    let mut hops = 0;
    while let Some(piece) = pending.pop_front() {
        match Path::new(&piece).components().next() {
            Some(Component::CurDir) => continue,
            Some(Component::ParentDir) => {
                resolved.pop();
                continue;
            }
            _ => resolved.push(&piece),
        }
        // A bare prefix or root (`C:\`, `\srv\share`) is not a file to stat.
        if resolved.file_name().is_none() {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&resolved) else {
            return false;
        };
        if !meta.is_symlink() {
            continue;
        }
        hops += 1;
        if hops > MAX_LINK_HOPS {
            return true;
        }
        let Ok(target) = std::fs::read_link(&resolved) else {
            return true;
        };
        // A rooted target without a drive or share (`\proj\x`, `\Device\..`)
        // resolves against the process's current drive or the NT namespace,
        // not the link's volume, so it cannot be classified by spelling.
        if cfg!(windows) && target.has_root() && !target.components().next().is_some_and(is_prefix)
        {
            return true;
        }
        let target = if target.has_root() || target.components().next().is_some_and(is_prefix) {
            target
        } else {
            resolved.parent().unwrap_or(Path::new("")).join(target)
        };
        if is_path_refused(&target, trusted) {
            return true;
        }
        // Resolve the rest through the target, from its own root.
        for piece in target.components().rev() {
            pending.push_front(piece.as_os_str().to_owned());
        }
        resolved = PathBuf::new();
    }
    false
}

fn is_prefix(c: Component<'_>) -> bool {
    matches!(c, Component::Prefix(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_path_or_a_plain_directory_is_not_a_refused_link() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("a.snxsch"), "x").expect("write");
        assert!(!link_leads_to_refused(&tmp.path().join("none.snxsch"), &[]));
        assert!(!link_leads_to_refused(&tmp.path().join("a.snxsch"), &[]));
        assert!(!link_leads_to_refused(tmp.path(), &[]));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_loop_is_refused_after_the_hop_cap() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        std::os::unix::fs::symlink(&b, &a).expect("link");
        std::os::unix::fs::symlink(&a, &b).expect("link");
        assert!(link_leads_to_refused(&a, &[]));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_local_file_or_directory_is_followed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("real");
        std::fs::create_dir(&dir).expect("dir");
        std::fs::write(dir.join("x.snxsch"), "x").expect("write");
        std::os::unix::fs::symlink(&dir, tmp.path().join("sub")).expect("link");
        std::os::unix::fs::symlink("real/x.snxsch", tmp.path().join("rel")).expect("link");
        assert!(!link_leads_to_refused(
            &tmp.path().join("sub/x.snxsch"),
            &[]
        ));
        assert!(!link_leads_to_refused(&tmp.path().join("rel"), &[]));
    }

    /// An unresolvable host: nothing can answer for it, whatever the LAN does.
    #[cfg(windows)]
    const DEAD_SHARE: &str = r"\\nonexistent.invalid\s";

    /// `false` (skip) when the privilege to create a symlink is missing.
    #[cfg(windows)]
    fn symlink(target: &Path, at: &Path, dir: bool) -> bool {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let made = if dir {
            symlink_dir(target, at)
        } else {
            symlink_file(target, at)
        };
        match made {
            Ok(()) => true,
            Err(e) if e.raw_os_error() == Some(1314) => {
                eprintln!("skipped: cannot create symlinks ({e})");
                false
            }
            Err(e) => panic!("symlink {at:?}: {e}"),
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_symlink_to_a_device_or_untrusted_share_is_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let real = tmp.path().join("real.snxsch");
        std::fs::write(&real, "x").expect("write");
        let ok = tmp.path().join("ok.snxsch");
        if !symlink(&real, &ok, false) {
            return;
        }
        let device = tmp.path().join("dev.snxsch");
        let share = tmp.path().join("share.snxsch");
        assert!(symlink(Path::new(r"\\.\NUL"), &device, false));
        let target = format!(r"{DEAD_SHARE}\x.snxsch");
        assert!(symlink(Path::new(&target), &share, false));

        assert!(!link_leads_to_refused(&ok, &[]));
        assert!(link_leads_to_refused(&device, &[]));
        assert!(link_leads_to_refused(&share, &[]));
    }

    #[cfg(windows)]
    #[test]
    fn a_symlinked_directory_partway_along_the_path_is_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sub = tmp.path().join("sub");
        if !symlink(Path::new(DEAD_SHARE), &sub, true) {
            return;
        }
        assert!(link_leads_to_refused(&sub.join("x.snxsch"), &[]));
        assert!(link_leads_to_refused(
            &sub.join("deeper").join("x.snxsch"),
            &[]
        ));
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_to_a_local_directory_is_followed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("real");
        std::fs::create_dir(&dir).expect("dir");
        std::fs::write(dir.join("x.snxsch"), "x").expect("write");
        let junction = tmp.path().join("j");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&dir)
            .output();
        if !made.is_ok_and(|o| o.status.success()) {
            eprintln!("skipped: cannot create a junction");
            return;
        }
        assert!(!link_leads_to_refused(&junction.join("x.snxsch"), &[]));
    }

    #[cfg(windows)]
    #[test]
    fn a_link_to_a_drive_relative_or_nt_namespace_target_is_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rooted = tmp.path().join("rooted.snxsch");
        if !symlink(Path::new(r"\proj\evil.snxsch"), &rooted, false) {
            return;
        }
        let nt = tmp.path().join("nt.snxsch");
        assert!(symlink(Path::new(r"\Device\HarddiskVolume1\x"), &nt, false));
        assert!(link_leads_to_refused(&rooted, &[]));
        assert!(link_leads_to_refused(&nt, &[]));
    }
}
