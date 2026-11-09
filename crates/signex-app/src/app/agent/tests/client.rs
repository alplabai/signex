//! #1924 PR B: the client side. Who holds a project is read from the registry
//! with the checks a client owes; a planted entry never makes it connect
//! anywhere but the endpoint its boot id names, and a token file that is a
//! link, wide or not a token is not read. (Unix only: it plants sockets.)

use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use super::super::client::{self, Lookup};
use super::super::files::{self, REGISTRY_DIR};
use super::super::registry::{self, Entry, FORMAT, Kind};
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
    /// The windows' shared locks on the project, which a running app holds.
    windows: std::cell::RefCell<Vec<std::fs::File>>,
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
        windows: std::cell::RefCell::new(Vec::new()),
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
    /// token file and its entry, and its window's lock on the project.
    fn live_app(&self, boot_id: &str) -> UnixListener {
        let listener = UnixListener::bind(endpoint_name(&self.dir, boot_id)).unwrap();
        listener.set_nonblocking(true).unwrap();
        self.write_token(boot_id, 0o600);
        self.plant(&self.entry(Kind::Gui, std::process::id(), boot_id));
        self.open_in_a_window();
        listener
    }

    /// A window that has the project open: its shared lock.
    fn open_in_a_window(&self) {
        let canonical = std::fs::canonicalize(&self.project).unwrap();
        match registry::claim_shared(&self.dir, &canonical) {
            Ok(lock) => self.windows.borrow_mut().push(lock),
            Err(_) => panic!("a window could not claim the project"),
        }
    }

    /// Every window closes (or the apps crash): no lock is held any more.
    fn close_every_window(&self) {
        self.windows.borrow_mut().clear();
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

    // The window closes. A headless run holds its lock: live, as long as it does.
    std::fs::remove_file(fx.entry_file()).unwrap();
    fx.close_every_window();
    let run = client::hold(&fx.dir, &fx.project).expect("a hold");
    let pid = std::process::id();
    assert!(matches!(fx.lookup(), Lookup::Headless { pid: held } if held == pid));
    // A second run cannot hold what the first does.
    let why = client::hold(&fx.dir, &fx.project).unwrap_err();
    assert!(why.contains("held by another signex-cli run"), "{why}");
    drop(run);

    // Process 1 is always running, but holds no lock: its entry is a dead
    // run's whose process id was reused.
    fx.plant(&fx.entry(Kind::Headless, 1, CC));
    assert!(matches!(fx.lookup(), Lookup::Stale(_)));
}

#[test]
fn an_entry_in_a_newer_format_is_not_touched_by_a_lookup_or_a_hold() {
    let fx = fixture();
    let newer = Entry {
        format: FORMAT + 1,
        ..fx.entry(Kind::Gui, DEAD_PID, CC)
    };
    fx.plant(&newer);
    let before = std::fs::read(fx.entry_file()).unwrap();
    let why = client::lookup(&fx.dir, &fx.project).unwrap_err();
    assert!(why.contains("newer than this signex-cli reads"), "{why}");
    let why = client::hold(&fx.dir, &fx.project).unwrap_err();
    assert!(why.contains("newer format"), "{why}");
    assert_eq!(std::fs::read(fx.entry_file()).unwrap(), before);
}

/// A server that accepts and never answers must not hang a run.
#[test]
fn a_reply_that_never_comes_is_given_up_on() {
    let fx = fixture();
    let app = fx.live_app(CC);
    let Lookup::Gui(peer) = fx.lookup() else {
        panic!("a live app was not found");
    };
    let mut connection = peer
        .connect()
        .expect("connected")
        .with_reply_timeout(std::time::Duration::from_millis(100));
    // Keep what the listener accepted open and silent.
    let _silent = loop {
        if let Ok((stream, _)) = app.accept() {
            break stream;
        }
    };
    let started = std::time::Instant::now();
    let why = connection.ask("list_documents", &[]).unwrap_err();
    assert!(why.contains("did not answer"), "{why}");
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

/// Replies are matched to requests by order. A reply that comes after its
/// request timed out must not be taken for the answer to the next one: it
/// would let a check of one document pass for another's.
#[test]
fn a_late_reply_is_never_taken_for_the_answer_to_the_next_request() {
    use std::io::{BufRead as _, BufReader, Write as _};
    let fx = fixture();
    let app = fx.live_app(CC);
    let Lookup::Gui(peer) = fx.lookup() else {
        panic!("a live app was not found");
    };
    // The app answers the first request after the client has given up on it.
    let server = std::thread::spawn(move || {
        let late = std::time::Duration::from_millis(300);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            let Ok((stream, _)) = app.accept() else {
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) > 0 {
                std::thread::sleep(late);
                let reply = r#"{"id":"read_document","status":"answered","data":{"revision":1},"messages":[],"changed":[]}"#;
                writeln!(reader.get_mut(), "{reply}").ok();
                std::thread::sleep(late);
                return;
            }
        }
    });
    let mut connection = peer
        .connect()
        .expect("connected")
        .with_reply_timeout(std::time::Duration::from_millis(100));
    let first = connection.ask("read_document", &[]).unwrap_err();
    assert!(first.contains("did not answer"), "{first}");
    // The first reply is on its way; the second request must not receive it.
    let second = connection.ask("read_document", &[]);
    let why = second.expect_err("a late reply was taken for the next answer");
    assert!(why.contains("failed earlier"), "{why}");
    server.join().unwrap();
}

/// A hold removes its entry and leaves its (empty) lock file: deleting a file
/// that is locked lets a second run lock the unlinked one.
#[test]
fn a_hold_leaves_its_lock_file_in_place() {
    let fx = fixture();
    let run = client::hold(&fx.dir, &fx.project).expect("held");
    let lock = fx
        .dir
        .join(REGISTRY_DIR)
        .join(fx.entry_file().file_name().unwrap())
        .with_extension("lock");
    assert!(lock.exists());
    drop(run);
    assert!(!fx.entry_file().exists(), "the entry outlived the run");
    assert!(lock.exists(), "the lock file was deleted");
    // And the next run holds again.
    drop(client::hold(&fx.dir, &fx.project).expect("held again"));
}

#[test]
fn an_entry_is_stale_only_when_no_window_holds_the_project_and_attachable_only_when_it_checks_out()
{
    let fx = fixture();
    let _app = fx.live_app(CC);
    let live = fx.entry(Kind::Gui, std::process::id(), CC);
    let bad = [
        (
            "a dead pid",
            Entry {
                pid: DEAD_PID,
                ..live.clone()
            },
        ),
        (
            "a malformed boot id",
            Entry {
                boot_id: "cc".into(),
                ..live.clone()
            },
        ),
        (
            "another project",
            Entry {
                project: "/another/Project.snxprj".into(),
                ..live.clone()
            },
        ),
        (
            "a socket the boot id does not name",
            Entry {
                socket: "/tmp/elsewhere.sock".into(),
                ..live.clone()
            },
        ),
        (
            "a dead headless run",
            fx.entry(Kind::Headless, DEAD_PID, EE),
        ),
    ];
    // A window holds the project: whatever the entry says, it is not stale
    // (the project is open), and nothing is attached to that does not check out.
    for (why, entry) in &bad {
        fx.plant(entry);
        assert!(
            !matches!(fx.lookup(), Lookup::Stale(_) | Lookup::Gui(_)),
            "{why}: {:?}",
            fx.lookup()
        );
    }
    fx.plant(&live);
    assert!(matches!(fx.lookup(), Lookup::Gui(_)), "the live app");
    // No window holds it any more: every one of those entries is stale.
    fx.close_every_window();
    for (why, entry) in &bad {
        fx.plant(entry);
        assert!(matches!(fx.lookup(), Lookup::Stale(_)), "{why}");
    }
    fx.plant(&live);
    assert!(matches!(fx.lookup(), Lookup::Stale(_)), "the app is gone");
}

/// Someone has the project open but no entry says who: nothing to attach to,
/// and not free.
#[test]
fn a_held_project_with_no_entry_is_not_free() {
    let fx = fixture();
    fx.open_in_a_window();
    assert!(
        matches!(fx.lookup(), Lookup::GuiNotListening { pid: None }),
        "{:?}",
        fx.lookup()
    );
    fx.close_every_window();
    assert!(matches!(fx.lookup(), Lookup::Free));
}

/// An entry is data another process wrote: whatever socket it names is never
/// connected to, not even to see if it answers.
#[test]
fn a_planted_entry_naming_another_socket_is_not_attached_to_and_that_socket_is_never_touched() {
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
    assert!(matches!(fx.lookup(), Lookup::GuiNotListening { .. }));

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
    fx.close_every_window();
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
