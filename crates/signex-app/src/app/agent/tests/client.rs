//! #1924 PR B: the client side. Who holds a project is read from the registry
//! with the checks a client owes; a planted entry never makes it connect
//! anywhere but the endpoint its boot id names, and a token file that is a
//! link, wide or not a token is not read. (Unix only: it plants sockets.)

use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use super::super::client::{self, Lookup};
use super::super::files::{self, REGISTRY_DIR};
use super::super::registry::{Entry, FORMAT, Kind};
use super::super::transport::endpoint_name;

/// Boot ids of the right form.
const CC: &str = "cccccccccccccccc";
const EE: &str = "eeeeeeeeeeeeeeee";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
/// Above every system's process id limit.
const DEAD_PID: u32 = 2_000_000_000;

struct Fixture {
    _root: tempfile::TempDir,
    dir: PathBuf,
    project: PathBuf,
}

fn fixture() -> Fixture {
    let root = tempfile::Builder::new()
        .prefix("snx-1924c")
        .tempdir()
        .expect("temp dir");
    let dir = root.path().join("run");
    files::prepare_dir(&dir).expect("runtime dir");
    files::prepare_dir(&dir.join(REGISTRY_DIR)).expect("registry dir");
    let project = root.path().join("P.snxprj");
    std::fs::write(&project, "{}").unwrap();
    Fixture {
        _root: root,
        dir,
        project,
    }
}

impl Fixture {
    fn entry_file(&self) -> PathBuf {
        client::entry_path(&self.dir, &self.project).expect("an entry path")
    }

    fn canonical(&self) -> String {
        std::fs::canonicalize(&self.project)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    fn entry(&self, kind: Kind, pid: u32, boot_id: &str) -> Entry {
        let socket = match kind {
            Kind::Gui => endpoint_name(&self.dir, boot_id),
            Kind::Headless => String::new(),
        };
        Entry {
            format: FORMAT,
            kind,
            project: self.canonical(),
            pid,
            socket,
            boot_id: boot_id.into(),
        }
    }

    fn plant(&self, entry: &Entry) {
        let text = serde_json::to_string(entry).unwrap();
        std::fs::write(self.entry_file(), text).unwrap();
    }

    /// An app of this process listening where its boot id says, with its
    /// token file and its entry.
    fn live_app(&self, boot_id: &str) -> UnixListener {
        let listener = UnixListener::bind(endpoint_name(&self.dir, boot_id)).unwrap();
        listener.set_nonblocking(true).unwrap();
        self.write_token(boot_id, 0o600);
        self.plant(&self.entry(Kind::Gui, std::process::id(), boot_id));
        listener
    }

    fn token_file(&self, boot_id: &str) -> PathBuf {
        self.dir.join(format!("{boot_id}.token"))
    }

    fn write_token(&self, boot_id: &str, mode: u32) {
        let path = self.token_file(boot_id);
        std::fs::write(&path, TOKEN).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    fn lookup(&self) -> Lookup {
        client::lookup(&self.dir, &self.project).expect("a lookup")
    }
}

/// Take every connection waiting on `listener` (a liveness probe connects and
/// closes): what is left is a connection made after this call.
fn drain(listener: &UnixListener) {
    while listener.accept().is_ok() {}
}

fn nobody_connected(listener: &UnixListener) -> bool {
    matches!(
        listener.accept(),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
    )
}

#[test]
fn no_entry_is_free_a_live_app_is_found_and_a_live_headless_run_holds() {
    let fx = fixture();
    assert!(matches!(fx.lookup(), Lookup::Free), "no entry");
    let nothing = tempfile::tempdir().unwrap();
    let absent = client::lookup(&nothing.path().join("never"), &fx.project).unwrap();
    assert!(matches!(absent, Lookup::Free), "no runtime folder");

    let _app = fx.live_app(CC);
    let Lookup::Gui(peer) = fx.lookup() else {
        panic!("a live app was not found");
    };
    assert_eq!(peer.pid, std::process::id());

    // Process 1 is always running and has no endpoint: a headless hold.
    fx.plant(&fx.entry(Kind::Headless, 1, CC));
    assert!(matches!(fx.lookup(), Lookup::Headless { pid: 1 }));
}

#[test]
fn a_dead_pid_a_wrong_socket_a_bad_boot_id_or_another_project_is_stale() {
    let fx = fixture();
    let _app = fx.live_app(CC);
    let live = fx.entry(Kind::Gui, std::process::id(), CC);
    let stale = |entry: &Entry, why: &str| {
        fx.plant(entry);
        assert!(matches!(fx.lookup(), Lookup::Stale(_)), "{why}");
    };
    stale(
        &Entry {
            pid: DEAD_PID,
            ..live.clone()
        },
        "a dead pid",
    );
    stale(
        &Entry {
            boot_id: "cc".into(),
            ..live.clone()
        },
        "a malformed boot id",
    );
    stale(
        &Entry {
            project: "/another/Project.snxprj".into(),
            ..live.clone()
        },
        "another project",
    );
    stale(
        &Entry {
            socket: "/tmp/elsewhere.sock".into(),
            ..live.clone()
        },
        "a socket the boot id does not name",
    );
    stale(
        &fx.entry(Kind::Headless, DEAD_PID, EE),
        "a dead headless run",
    );
    // And the live one is still live.
    fx.plant(&live);
    assert!(matches!(fx.lookup(), Lookup::Gui(_)));
}

/// An entry is data another process wrote: whatever socket it names is never
/// connected to, not even to see if it answers.
#[test]
fn a_planted_entry_naming_another_socket_is_stale_and_that_socket_is_never_touched() {
    let fx = fixture();
    let planted_path = fx.dir.join("planted.sock");
    let planted = UnixListener::bind(&planted_path).unwrap();
    planted.set_nonblocking(true).unwrap();
    // The boot id's own endpoint is a live app, the entry names the other one.
    let _app = fx.live_app(CC);
    let entry = Entry {
        socket: planted_path.to_string_lossy().into_owned(),
        ..fx.entry(Kind::Gui, std::process::id(), CC)
    };
    fx.plant(&entry);
    assert!(matches!(fx.lookup(), Lookup::Stale(_)));

    // A live app's entry never leads the client anywhere but its own endpoint.
    fx.plant(&fx.entry(Kind::Gui, std::process::id(), CC));
    let Lookup::Gui(peer) = fx.lookup() else {
        panic!("a live app was not found");
    };
    peer.connect().expect("the app's own endpoint is used");
    assert!(
        nobody_connected(&planted),
        "the planted socket was connected to"
    );
}

#[test]
fn a_stale_entry_is_removed_only_if_it_is_still_stale_and_a_plain_file_of_ours() {
    let fx = fixture();
    let dead = fx.entry(Kind::Gui, DEAD_PID, CC);
    fx.plant(&dead);
    let Lookup::Stale(stale) = fx.lookup() else {
        panic!("not stale");
    };
    stale.remove().expect("removed");
    assert!(!fx.entry_file().exists());

    // Rewritten by a live app between the look and the removal: kept.
    fx.plant(&dead);
    let Lookup::Stale(stale) = fx.lookup() else {
        panic!("not stale");
    };
    let _app = fx.live_app(EE);
    stale.remove().expect("left alone");
    assert!(
        fx.entry_file().exists(),
        "an entry that changed was removed"
    );

    // A link in the entry's place is not read, so it is never removed.
    std::fs::remove_file(fx.entry_file()).unwrap();
    let target = fx.dir.join("precious");
    std::fs::write(&target, serde_json::to_string(&dead).unwrap()).unwrap();
    symlink(&target, fx.entry_file()).unwrap();
    assert!(matches!(fx.lookup(), Lookup::Free));
    assert!(
        fx.entry_file()
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(target.exists());
}

#[test]
fn a_token_file_that_is_a_link_wide_or_not_a_token_is_not_read_and_nothing_connects() {
    let fx = fixture();
    let app = fx.live_app(CC);
    let Lookup::Gui(peer) = fx.lookup() else {
        panic!("a live app was not found");
    };
    drain(&app);
    let refused = |why: &str, expected: &str| {
        let error = match peer.connect() {
            Ok(_) => panic!("{why}: attached"),
            Err(error) => error,
        };
        assert!(error.contains(expected), "{why}: {error}");
        assert!(nobody_connected(&app), "{why}: it connected anyway");
    };

    for wide in [0o644, 0o640, 0o604] {
        fx.write_token(CC, wide);
        refused("a wide token file", "mode 0600");
    }
    fx.write_token(CC, 0o600);
    std::fs::write(fx.token_file(CC), "not a token").unwrap();
    refused("a file that is not a token", "is not a session token");

    let real = fx.dir.join("real.token");
    std::fs::write(&real, TOKEN).unwrap();
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::remove_file(fx.token_file(CC)).unwrap();
    symlink(&real, fx.token_file(CC)).unwrap();
    refused("a link", "cannot be opened");

    std::fs::remove_file(fx.token_file(CC)).unwrap();
    refused("no token file", "cannot be opened");
    fx.write_token(CC, 0o600);
    peer.connect().expect("a private token file is read");
}

/// A runtime folder other users can enter is not trusted with anything.
#[test]
fn a_runtime_folder_with_wider_permissions_is_refused() {
    let fx = fixture();
    std::fs::set_permissions(&fx.dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let why = client::lookup(&fx.dir, &fx.project).unwrap_err();
    assert!(why.contains("wider than 700"), "{why}");
}

#[test]
fn a_headless_hold_is_an_entry_that_dropping_it_removes_even_by_a_panic() {
    let fx = fixture();
    let hold = client::hold(&fx.dir, &fx.project).expect("held");
    let text = std::fs::read_to_string(fx.entry_file()).unwrap();
    let entry: Entry = serde_json::from_str(&text).unwrap();
    assert_eq!(
        (entry.kind, entry.pid),
        (Kind::Headless, std::process::id())
    );
    assert_eq!(entry.format, FORMAT);
    assert!(entry.socket.is_empty(), "a headless run has no endpoint");
    assert_eq!(entry.project, fx.canonical());
    assert_eq!(
        std::fs::metadata(fx.entry_file())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(matches!(fx.lookup(), Lookup::Headless { .. }));
    drop(hold);
    assert!(!fx.entry_file().exists(), "the hold outlived its run");

    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _hold = client::hold(&fx.dir, &fx.project).expect("held");
        panic!("the run failed");
    }));
    assert!(unwound.is_err());
    assert!(!fx.entry_file().exists(), "an error left the project held");
}

#[test]
fn a_hold_takes_over_a_stale_entry_leaves_a_live_holder_and_never_removes_anothers() {
    let fx = fixture();
    fx.plant(&fx.entry(Kind::Gui, DEAD_PID, CC));
    let hold = client::hold(&fx.dir, &fx.project).expect("a stale entry is taken over");

    // The entry now belongs to another live run: dropping ours leaves it.
    let theirs = fx.entry(Kind::Headless, 1, EE);
    fx.plant(&theirs);
    drop(hold);
    assert_eq!(
        serde_json::from_str::<Entry>(&std::fs::read_to_string(fx.entry_file()).unwrap()).unwrap(),
        theirs
    );

    // And another live app's entry is not held over (process 1 is not us).
    let _app = fx.live_app(CC);
    fx.plant(&fx.entry(Kind::Gui, 1, CC));
    let why = client::hold(&fx.dir, &fx.project).unwrap_err();
    assert!(why.contains("also open in process"), "{why}");
    assert!(matches!(fx.lookup(), Lookup::Gui(_)));
}

#[test]
fn the_runtime_folder_of_a_client_is_made_private() {
    let root = tempfile::Builder::new()
        .prefix("snx-1924c")
        .tempdir()
        .unwrap();
    let dir = root.path().join("fresh");
    let project = root.path().join("P.snxprj");
    std::fs::write(&project, "{}").unwrap();
    let _hold = client::hold(&dir, &project).expect("held");
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join(REGISTRY_DIR)), 0o700);
}
