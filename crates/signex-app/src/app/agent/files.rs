//! The per-user runtime directory and the files in it (#1924): the socket, the
//! session token and the registry entries. Everything is private to the user:
//! the directory 0700, each file 0600, and nothing is removed or written
//! through a link or over another user's file. Layout and rules:
//! `docs/AGENT_SOCKET.md`, "Files".

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The folder the registry entries of this user's apps live in.
pub(super) const REGISTRY_DIR: &str = "registry";

/// Where the runtime directory is: `$XDG_RUNTIME_DIR/signex-agent` where the
/// system gives each user one, else a folder named for the user in the
/// temporary directory (macOS: already per user), else the user's local
/// application data (Windows). Only reads the environment.
pub(super) fn runtime_dir() -> Result<PathBuf, String> {
    #[cfg(unix)]
    {
        let xdg = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute());
        Ok(match xdg {
            Some(dir) => dir.join("signex-agent"),
            None => std::env::temp_dir().join(format!("signex-agent-{}", unix::own_uid())),
        })
    }
    #[cfg(not(unix))]
    {
        dirs::data_local_dir()
            .map(|dir| dir.join("Signex").join("agent"))
            .ok_or_else(|| "this user has no local application data folder".to_string())
    }
}

/// Create `dir` (its last component only, private) if it is missing, and
/// refuse to use it unless it is a real folder of this user that nobody else
/// can enter.
pub(super) fn prepare_dir(dir: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        match std::fs::DirBuilder::new().mode(0o700).create(dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("cannot create {}: {error}", dir.display())),
        }
        unix::check_dir(dir)
    }
    #[cfg(not(unix))]
    {
        // The user's profile folder is private to the user by its ACL.
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        match std::fs::symlink_metadata(dir) {
            Ok(meta) if meta.file_type().is_dir() => Ok(()),
            _ => Err(format!("{} is not a plain folder", dir.display())),
        }
    }
}

/// Refuse `dir` unless it is a real folder of this user that nobody else can
/// enter. Creates nothing: for a client that must not make the folder.
pub(super) fn check_dir(dir: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        unix::check_dir(dir)
    }
    #[cfg(not(unix))]
    {
        match std::fs::symlink_metadata(dir) {
            Ok(meta) if meta.file_type().is_dir() => Ok(()),
            _ => Err(format!("{} is not a plain folder", dir.display())),
        }
    }
}

/// Write `bytes` to `path` as a private file (0600), replacing what is there
/// in one step. The temporary file is created new, never through a link.
pub(super) fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("{} has no file name", path.display()))?;
    let tmp = path.with_file_name(format!(".{}.tmp", name.to_string_lossy()));
    check_replaceable(path)?;
    remove_if_ours(&tmp)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let write = options
        .open(&tmp)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
        .and_then(|()| std::fs::rename(&tmp, path));
    write.map_err(|error| {
        // Best effort: the temporary file is ours and holds nothing a person
        // asked to keep.
        std::fs::remove_file(&tmp).ok();
        format!("cannot write {}: {error}", path.display())
    })
}

/// Remove `path` if it is there and it is this user's own plain file or
/// socket. A link, a folder or another user's file is refused and left alone;
/// a path that is not there is fine.
pub(super) fn remove_if_ours(path: &Path) -> Result<(), String> {
    if !check_replaceable(path)? {
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|error| format!("cannot remove {}: {error}", path.display()))
}

/// Whether `path` is there, once it is known that taking its place is safe:
/// it is this user's own plain file or socket. A link, a folder or another
/// user's file is an error.
fn check_replaceable(path: &Path) -> Result<bool, String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("cannot inspect {}: {error}", path.display())),
    };
    #[cfg(unix)]
    let owner = unix::owner_of(&meta);
    #[cfg(not(unix))]
    let owner = None;
    removal_verdict(meta.file_type().is_symlink(), meta.is_dir(), owner)
        .map_err(|why| format!("{} {why}; it was left alone", path.display()))?;
    Ok(true)
}

/// Whether a file may be removed or replaced: not a link, not a folder, and
/// (where the system says who owns it) owned by this user.
fn removal_verdict(
    is_link: bool,
    is_dir: bool,
    owner: Option<(u32, u32)>,
) -> Result<(), &'static str> {
    if is_link {
        return Err("is a link");
    }
    if is_dir {
        return Err("is a folder");
    }
    match owner {
        Some((owner, own_uid)) if owner != own_uid => Err("is owned by another user"),
        _ => Ok(()),
    }
}

#[cfg(unix)]
mod unix {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    use std::path::Path;

    /// The effective user id of this process.
    pub(in crate::app::agent) fn own_uid() -> u32 {
        // SAFETY: `geteuid` takes no arguments, cannot fail and touches no memory.
        unsafe { libc::geteuid() }
    }

    /// The owner of `meta`'s file and the user running this process.
    pub(super) fn owner_of(meta: &std::fs::Metadata) -> Option<(u32, u32)> {
        Some((meta.uid(), own_uid()))
    }

    pub(super) fn check_dir(dir: &Path) -> Result<(), String> {
        let meta = std::fs::symlink_metadata(dir)
            .map_err(|error| format!("cannot inspect {}: {error}", dir.display()))?;
        if !meta.file_type().is_dir() {
            return Err(format!("{} is not a plain folder (a link?)", dir.display()));
        }
        dir_verdict(meta.uid(), meta.permissions().mode(), own_uid())
            .map_err(|why| format!("{} {why}", dir.display()))
    }

    /// Whether a folder of owner `owner` and `mode` is fit to hold the
    /// socket and the token for the user `own_uid`.
    pub(in crate::app::agent) fn dir_verdict(
        owner: u32,
        mode: u32,
        own_uid: u32,
    ) -> Result<(), String> {
        if owner != own_uid {
            return Err(format!(
                "is owned by user {owner}, not by this user: refusing to use it"
            ));
        }
        if mode & 0o077 != 0 {
            return Err(format!(
                "has mode {:o}, wider than 700: refusing to use it",
                mode & 0o777
            ));
        }
        Ok(())
    }
}

#[cfg(unix)]
pub(in crate::app::agent) use unix::own_uid;

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _, symlink};

    use super::*;

    fn scratch() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("snx-1924-files")
            .tempdir()
            .expect("temp dir")
    }

    #[test]
    fn a_foreign_or_wide_folder_is_refused_and_a_private_own_one_passes() {
        assert!(unix::dir_verdict(501, 0o40700, 501).is_ok());
        assert!(unix::dir_verdict(501, 0o40500, 501).is_ok());
        let foreign = unix::dir_verdict(0, 0o40700, 501).unwrap_err();
        assert!(foreign.contains("owned by user 0"), "{foreign}");
        for wide in [0o40750, 0o40705, 0o40755, 0o41777] {
            let why = unix::dir_verdict(501, wide, 501).unwrap_err();
            assert!(why.contains("wider than 700"), "{wide:o}: {why}");
        }
    }

    #[test]
    fn a_folder_is_created_private_and_a_wider_existing_one_is_refused() {
        let root = scratch();
        let dir = root.path().join("run");
        prepare_dir(&dir).expect("created");
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        prepare_dir(&dir).expect("an own private folder is reused");

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let why = prepare_dir(&dir).unwrap_err();
        assert!(why.contains("wider than 700"), "{why}");

        let loose = root.path().join("loose");
        std::fs::DirBuilder::new()
            .mode(0o770)
            .create(&loose)
            .unwrap();
        assert!(prepare_dir(&loose).is_err());
    }

    #[test]
    fn a_link_in_place_of_the_folder_is_refused_not_followed() {
        let root = scratch();
        let real = root.path().join("real");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&real)
            .unwrap();
        let link = root.path().join("link");
        symlink(&real, &link).unwrap();
        let why = prepare_dir(&link).unwrap_err();
        assert!(why.contains("not a plain folder"), "{why}");
    }

    #[test]
    fn a_private_file_is_written_0600_and_replaced_whole() {
        let root = scratch();
        let path = root.path().join("token");
        write_private(&path, b"one").expect("written");
        write_private(&path, b"two").expect("replaced");
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(!root.path().join(".token.tmp").exists());
    }

    #[test]
    fn a_stale_link_is_neither_removed_nor_followed() {
        let root = scratch();
        let target = root.path().join("precious");
        std::fs::write(&target, "keep me").unwrap();
        let stale = root.path().join("agent.sock");
        symlink(&target, &stale).unwrap();

        let why = remove_if_ours(&stale).unwrap_err();
        assert!(why.contains("is a link"), "{why}");
        assert!(stale.symlink_metadata().is_ok(), "the link is still there");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me");
        // Nor is a private write over the same name made through it.
        let why = write_private(&stale, b"x").unwrap_err();
        assert!(why.contains("is a link"), "{why}");
        assert!(stale.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me");
    }

    #[test]
    fn removal_needs_an_own_plain_file() {
        assert!(removal_verdict(false, false, Some((7, 7))).is_ok());
        assert!(removal_verdict(false, false, None).is_ok());
        assert_eq!(removal_verdict(true, false, Some((7, 7))), Err("is a link"));
        assert_eq!(
            removal_verdict(false, true, Some((7, 7))),
            Err("is a folder")
        );
        assert_eq!(
            removal_verdict(false, false, Some((0, 7))),
            Err("is owned by another user")
        );
        let root = scratch();
        assert!(remove_if_ours(&root.path().join("absent")).is_ok());
        let folder = root.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        assert!(remove_if_ours(&folder).unwrap_err().contains("is a folder"));
    }
}
