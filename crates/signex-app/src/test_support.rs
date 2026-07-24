//! Test-only helpers for persistence tests (#416, #469, #482).
//!
//! `atomic_write` picks a unique `<pid>-<counter>` temp sibling name per
//! call, so a test can no longer force a write failure by pre-creating a
//! directory at a fixed `<path>.tmp` name (that name is never used). Deny
//! new-file creation in the destination's parent directory instead —
//! that fails `File::create` regardless of the exact temp name chosen.
//!
//! On Windows, the deny is settled with a real probe write
//! ([`settle_deny`]) before `DenyNewFiles::on` returns, rather than
//! trusting `icacls`'s exit code — see #482.

use std::path::{Path, PathBuf};

/// RAII guard: while alive, `dir` rejects new-file creation. Restores
/// permissions on drop so the tempdir this is used inside still cleans
/// up normally.
pub struct DenyNewFiles {
    dir: PathBuf,
}

impl DenyNewFiles {
    pub fn on(dir: &Path) -> Self {
        deny(dir);
        Self {
            dir: dir.to_path_buf(),
        }
    }
}

impl Drop for DenyNewFiles {
    fn drop(&mut self) {
        allow(&self.dir);
    }
}

#[cfg(unix)]
fn deny(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555))
        .expect("chmod dir read-only for test");
}

#[cfg(unix)]
fn allow(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
}

// Windows directories ignore the FILE_ATTRIBUTE_READONLY bit for new-file
// creation inside them, so `Permissions::set_readonly` can't simulate this —
// deny the current user's "create files" ACE instead.
#[cfg(windows)]
fn deny(dir: &Path) {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "Everyone".into());
    let status = std::process::Command::new("icacls")
        .arg(dir)
        .arg("/deny")
        .arg(format!("{user}:(WD)"))
        .status()
        .expect("run icacls /deny for test");
    assert!(status.success(), "icacls /deny failed");
    settle_deny(dir);
}

/// `icacls`'s exit code is not proof the deny is enforced yet — under
/// full-workspace parallel test load on Windows, the write to the
/// directory's security descriptor can lose the race against the very next
/// `File::create` in the same directory, so a caller that trusts the exit
/// code alone occasionally observes a write that should have failed
/// succeed instead (#482). Poll with a real probe write and only return
/// once the deny is actually observed to take effect.
#[cfg(windows)]
fn settle_deny(dir: &Path) {
    let probe = dir.join(format!(".icacls-settle-probe-{}", std::process::id()));
    for _ in 0..100 {
        match std::fs::File::create(&probe) {
            Ok(_) => {
                let _ = std::fs::remove_file(&probe);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(_) => return,
        }
    }
    panic!("icacls /deny on {dir:?} did not take effect within 1s (#482)");
}

#[cfg(windows)]
fn allow(dir: &Path) {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "Everyone".into());
    let _ = std::process::Command::new("icacls")
        .arg(dir)
        .arg("/remove:d")
        .arg(&user)
        .status();
}

/// True if `dir` contains a leftover atomic-write temp sibling (`*.tmp`).
/// `atomic_write` picks a unique per-writer name, so callers must scan
/// for any match rather than a fixed name.
pub fn has_stray_tmp(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| entry.path().extension().is_some_and(|ext| ext == "tmp"))
}
