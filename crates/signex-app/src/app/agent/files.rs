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
/// system gives each user one; on Linux, with that variable unset (a shell over
/// ssh, cron), the same place by the user's id (`/run/user/<uid>`) if it is
/// there as the user's own folder, so a desktop app and such a shell agree;
/// else a folder named for the user in the user's temporary directory (macOS:
/// the system's per-user one, `confstr`, whatever `TMPDIR` says, so a Finder
/// launch and a shell without it agree; elsewhere `temp_dir()`); on Windows the
/// user's local application data. Only reads the environment.
pub(super) fn runtime_dir() -> Result<PathBuf, String> {
    // The app's own unit tests list their open projects here (a project open is
    // listed whether or not agents may connect): never in the user's folder.
    if cfg!(test) {
        return Ok(fallback_test_runtime_dir());
    }
    #[cfg(unix)]
    {
        let xdg = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let run_user = cfg!(target_os = "linux").then_some(Path::new("/run/user"));
        Ok(unix::resolve(
            xdg,
            run_user,
            &unix::user_temp_dir(),
            unix::own_uid(),
        ))
    }
    #[cfg(not(unix))]
    {
        dirs::data_local_dir()
            .map(|dir| dir.join("Signex").join("agent"))
            .ok_or_else(|| "this user has no local application data folder".to_string())
    }
}

/// A runtime directory of a test run of its own, never the user's, and the
/// temporary folder that holds it: removed when the app that owns it is
/// dropped, so a test run leaves nothing behind. Only if no temporary folder
/// can be made, a fixed name for the process (which is then left behind).
pub(super) fn test_runtime_dir() -> (PathBuf, Option<tempfile::TempDir>) {
    match tempfile::Builder::new()
        .prefix("signex-agent-test-")
        .tempdir()
    {
        Ok(root) => (root.path().join("run"), Some(root)),
        Err(_) => (fallback_test_runtime_dir(), None),
    }
}

fn fallback_test_runtime_dir() -> PathBuf {
    std::env::temp_dir().join(format!("signex-agent-test-{}", std::process::id()))
}

/// The other places a runtime directory of this user may be (an app started
/// with another environment than this process): a lock held there still counts
/// (see `client::lookup` and `client::hold`). Only those that exist now.
#[cfg_attr(not(all(unix, not(test))), allow(unused_variables))]
pub(super) fn alternate_dirs(primary: &Path) -> Vec<PathBuf> {
    // The unit tests never look outside their own folders.
    #[cfg(all(unix, not(test)))]
    {
        let xdg = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let run_user = cfg!(target_os = "linux").then_some(Path::new("/run/user"));
        let temp = std::env::temp_dir();
        unix::alternates(
            primary,
            xdg,
            run_user,
            &[temp.as_path(), Path::new("/tmp")],
            unix::own_uid(),
        )
    }
    #[cfg(not(all(unix, not(test))))]
    {
        Vec::new()
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
    use std::path::{Path, PathBuf};

    /// The effective user id of this process.
    pub(in crate::app::agent) fn own_uid() -> u32 {
        // SAFETY: `geteuid` takes no arguments, cannot fail and touches no memory.
        unsafe { libc::geteuid() }
    }

    /// The runtime directory for the user `uid` (see `super::runtime_dir`),
    /// from the environment and roots it is given.
    pub(super) fn resolve(
        xdg: Option<PathBuf>,
        run_user: Option<&Path>,
        temp: &Path,
        uid: u32,
    ) -> PathBuf {
        if let Some(dir) = xdg.filter(|dir| dir.is_absolute()) {
            return dir.join("signex-agent");
        }
        let own = run_user.map(|root| root.join(uid.to_string()));
        match own.filter(|dir| is_own_folder(dir, uid)) {
            Some(dir) => dir.join("signex-agent"),
            None => temp.join(format!("signex-agent-{uid}")),
        }
    }

    /// The user's temporary directory: on macOS the system's per-user one
    /// (`confstr(_CS_DARWIN_USER_TEMP_DIR)`), which does not depend on
    /// `TMPDIR`; `temp_dir()` where `confstr` fails and on other systems.
    pub(super) fn user_temp_dir() -> PathBuf {
        pick_temp(darwin_user_temp_dir(), std::env::temp_dir())
    }

    /// `system` (the platform's own answer) when there is one, else `env`.
    pub(super) fn pick_temp(system: Option<PathBuf>, env: PathBuf) -> PathBuf {
        system.unwrap_or(env)
    }

    #[cfg(target_os = "macos")]
    pub(super) fn darwin_user_temp_dir() -> Option<PathBuf> {
        use std::os::unix::ffi::OsStringExt as _;
        let mut buf = vec![0_u8; 1024];
        // SAFETY: `confstr` writes at most `buf.len()` bytes, NUL included, into
        // the buffer it is given and returns the length it needs (0 on error).
        let needed = unsafe {
            libc::confstr(
                libc::_CS_DARWIN_USER_TEMP_DIR,
                buf.as_mut_ptr().cast(),
                buf.len(),
            )
        };
        if needed == 0 || needed > buf.len() {
            return None;
        }
        buf.truncate(needed - 1);
        let dir = PathBuf::from(std::ffi::OsString::from_vec(buf));
        dir.is_absolute().then_some(dir)
    }

    #[cfg(not(target_os = "macos"))]
    pub(super) fn darwin_user_temp_dir() -> Option<PathBuf> {
        None
    }

    /// Where else an app or a run of the same user may have put its runtime
    /// directory (another environment), that exist now and are not `primary`.
    pub(super) fn alternates(
        primary: &Path,
        xdg: Option<PathBuf>,
        run_user: Option<&Path>,
        temps: &[&Path],
        uid: u32,
    ) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = Vec::new();
        let mut candidates: Vec<PathBuf> = Vec::new();
        candidates.extend(
            xdg.filter(|dir| dir.is_absolute())
                .map(|d| d.join("signex-agent")),
        );
        candidates.extend(run_user.map(|root| root.join(uid.to_string()).join("signex-agent")));
        candidates.extend(
            temps
                .iter()
                .map(|temp| temp.join(format!("signex-agent-{uid}"))),
        );
        for dir in candidates {
            if dir != primary && !found.contains(&dir) && std::fs::symlink_metadata(&dir).is_ok() {
                found.push(dir);
            }
        }
        found
    }

    /// Whether `dir` is a real folder (not a link) owned by `uid`.
    fn is_own_folder(dir: &Path, uid: u32) -> bool {
        std::fs::symlink_metadata(dir)
            .is_ok_and(|meta| meta.file_type().is_dir() && meta.uid() == uid)
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

    /// A desktop app (with `XDG_RUNTIME_DIR`) and a shell without it (ssh,
    /// cron) must name one folder, or the lock between them protects nothing.
    #[test]
    fn the_runtime_dir_is_the_same_with_and_without_the_environment_variable() {
        let root = scratch();
        let uid = unix::own_uid();
        let run_user = root.path().join("run").join("user");
        let own = run_user.join(uid.to_string());
        std::fs::create_dir_all(&own).unwrap();
        let temp = root.path().join("tmp");
        let want = own.join("signex-agent");

        let with_env = unix::resolve(Some(own.clone()), Some(&run_user), &temp, uid);
        let without = unix::resolve(None, Some(&run_user), &temp, uid);
        assert_eq!(with_env, want);
        assert_eq!(without, want);
        // A relative variable is no folder.
        assert_eq!(
            unix::resolve(Some("rel".into()), Some(&run_user), &temp, uid),
            want
        );

        // No `/run/user/<uid>`, or one that is a link or is another user's:
        // the temporary directory.
        let fallback = temp.join(format!("signex-agent-{uid}"));
        assert_eq!(unix::resolve(None, None, &temp, uid), fallback);
        // A folder named for another user id, though it is ours: not theirs.
        let other = uid + 1;
        std::fs::create_dir_all(run_user.join(other.to_string())).unwrap();
        assert_eq!(
            unix::resolve(None, Some(&run_user), &temp, other),
            temp.join(format!("signex-agent-{other}"))
        );
        std::fs::remove_dir(&own).unwrap();
        symlink(root.path(), &own).unwrap();
        assert_eq!(unix::resolve(None, Some(&run_user), &temp, uid), fallback);
    }

    #[test]
    fn the_other_places_a_runtime_dir_may_be_are_those_that_exist() {
        let root = scratch();
        let uid = unix::own_uid();
        let run_user = root.path().join("run").join("user");
        let primary = run_user.join(uid.to_string()).join("signex-agent");
        let temp = root.path().join("tmp");
        let in_temp = temp.join(format!("signex-agent-{uid}"));
        let none = unix::alternates(&primary, None, Some(&run_user), &[&temp], uid);
        assert!(none.is_empty(), "{none:?}");
        std::fs::create_dir_all(&in_temp).unwrap();
        std::fs::create_dir_all(&primary).unwrap();
        let found = unix::alternates(&primary, None, Some(&run_user), &[&temp, &temp], uid);
        assert_eq!(
            found,
            vec![in_temp],
            "the primary and a repeat are left out"
        );
    }

    /// A Finder launch has `TMPDIR`, a shell over ssh or cron may not: the
    /// system's per-user folder is what names the runtime folder, so both agree.
    #[test]
    fn the_user_temp_dir_is_the_systems_own_whatever_tmpdir_says() {
        let system = PathBuf::from("/var/folders/ab/cd/T");
        for env in ["/tmp", "/var/folders/xx/yy/T", ""] {
            assert_eq!(
                unix::pick_temp(Some(system.clone()), PathBuf::from(env)),
                system,
                "TMPDIR={env:?} changed the folder"
            );
        }
        // Only when the system gives none does the environment's answer count.
        assert_eq!(
            unix::pick_temp(None, PathBuf::from("/tmp")),
            PathBuf::from("/tmp")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_gives_a_per_user_temp_dir_by_confstr() {
        let dir = unix::darwin_user_temp_dir().expect("confstr answers");
        assert!(dir.is_absolute(), "{}", dir.display());
        assert!(std::fs::metadata(&dir).is_ok_and(|meta| meta.is_dir()));
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
