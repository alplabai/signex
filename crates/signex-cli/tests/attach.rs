//! End to end (#1924): `signex-cli` works through a running Signex that has
//! the project open, else runs headless and holds the project. The running
//! app is a stand-in on a real Unix socket (the app side, with the real
//! listener and app, is tested in signex-app); what is checked here is what
//! the real binary does with the registry, the token file and the socket.
#![cfg(unix)]

mod support;

use std::collections::VecDeque;
use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use signex_app::headless::agent_client::entry_path;
use support::{Fixture, Live, fixture, jsonl, run_cli_env, snapshot, temp_root};

/// Boot ids of the right form.
const CC: &str = "cccccccccccccccc";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
/// Above every system's process id limit.
const DEAD_PID: u32 = 2_000_000_000;
/// A sheet of the run's project, and one of another project the same app has
/// open (a folder the run was never given), by id.
const SHEET_A: &str = "11111111-1111-4111-8111-111111111111";
const SHEET_B: &str = "22222222-2222-4222-8222-222222222222";
const OTHER_PROJECT_SHEET: &str = "/somewhere/else/ProjectB/b.snxsch";
/// The run's own project file, as a document.
const PROJECT_DOC: &str = "88888888-8888-4888-8888-888888888888";

/// A runtime folder (`$XDG_RUNTIME_DIR/signex-agent`) and a project.
struct World {
    _root: tempfile::TempDir,
    _runtime_root: tempfile::TempDir,
    runtime: PathBuf,
    dir: PathBuf,
    fx: Fixture,
}

fn world() -> World {
    let (root, canonical) = temp_root();
    // Short: a socket path must fit `SUN_LEN` (104 bytes on macOS).
    let runtime_root = tempfile::Builder::new()
        .prefix("s")
        .tempdir_in("/tmp")
        .unwrap();
    let runtime = runtime_root.path().to_path_buf();
    let dir = runtime.join("signex-agent");
    for folder in [&dir, &dir.join("registry")] {
        std::fs::create_dir(folder).unwrap();
        std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let fx = fixture(&canonical);
    World {
        _root: root,
        _runtime_root: runtime_root,
        runtime,
        dir,
        fx,
    }
}

impl World {
    fn project(&self) -> &str {
        self.fx.project.to_str().unwrap()
    }

    fn entry_file(&self) -> PathBuf {
        entry_path(&self.dir, &self.fx.project).expect("an entry path")
    }

    fn entry(&self) -> Option<Value> {
        let text = std::fs::read_to_string(self.entry_file()).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn plant(&self, kind: &str, pid: u32, boot: &str, socket: &Path) {
        let project = std::fs::canonicalize(&self.fx.project).unwrap();
        let entry = json!({"format": 3, "kind": kind, "project": project,
            "pid": pid, "socket": socket, "boot_id": boot});
        std::fs::write(self.entry_file(), entry.to_string()).unwrap();
    }

    /// What a Signex with the project open and agent connections off leaves:
    /// an entry with no socket, and the app's lock on the project, held by the
    /// returned file.
    fn open_without_listening(&self) -> std::fs::File {
        self.plant("gui", std::process::id(), CC, Path::new(""));
        self.window_lock()
    }

    /// The shared lock a window holds on the project while it has it open.
    fn window_lock(&self) -> std::fs::File {
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.entry_file().with_extension("lock"))
            .unwrap();
        lock.try_lock_shared().expect("the window's lock");
        lock
    }

    fn socket(&self, boot: &str) -> PathBuf {
        self.dir.join(format!("{boot}.sock"))
    }

    fn token(&self, boot: &str, mode: u32) -> PathBuf {
        let path = self.dir.join(format!("{boot}.token"));
        std::fs::write(&path, TOKEN).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    fn cli(&self, args: &[&str], stdin: &str) -> support::Output {
        run_cli_env(args, stdin, &[("XDG_RUNTIME_DIR", self.runtime.as_path())])
    }

    /// A running app of this test process: a socket answering at its boot
    /// id's endpoint, a token file and an entry, replying with `replies` in
    /// turn.
    fn app(&self, replies: &[Value]) -> FakeApp {
        self.app_saving(replies, Vec::new())
    }

    /// [`Self::app`] whose project document says saving it writes `save_writes`
    /// besides its file (the libraries it has pending).
    fn app_saving(&self, replies: &[Value], save_writes: Vec<String>) -> FakeApp {
        let socket = self.socket(CC);
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        self.token(CC, 0o600);
        self.plant("gui", std::process::id(), CC, &socket);
        let sheet = self.fx.sheet.to_str().unwrap().to_string();
        let docs = vec![
            (sheet.clone(), sheet.clone()),
            (SHEET_A.into(), sheet),
            (SHEET_B.into(), OTHER_PROJECT_SHEET.into()),
            (PROJECT_DOC.into(), self.project().to_string()),
        ];
        let mut app = FakeApp::serve(listener, replies, docs, save_writes);
        app.window = Some(self.window_lock());
        app
    }
}

/// What the stand-in app was sent, and whether anything connected beyond a
/// liveness probe (which closes without a byte).
struct FakeApp {
    requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// The app's window lock on the project, held for as long as it runs.
    window: Option<std::fs::File>,
}

impl FakeApp {
    /// `docs` maps a document operand (an id or a path) to the file the app
    /// holds it as; `document_info` is answered from it and does not use up
    /// one of `replies`.
    fn serve(
        listener: UnixListener,
        replies: &[Value],
        docs: Vec<(String, String)>,
        save_writes: Vec<String>,
    ) -> Self {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let mut replies: VecDeque<Value> = replies.iter().cloned().collect();
        let (seen, stopped) = (requests.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let Ok((stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap_or(0) > 0 {
                    let request: Value = serde_json::from_str(&line).unwrap();
                    seen.lock().unwrap().push(request.clone());
                    line.clear();
                    let reply = if request["id"] == "document_info" {
                        let asked = request["args"]["document"].as_str().unwrap_or_default();
                        match docs.iter().find(|(text, _)| text == asked) {
                            Some((_, path)) => {
                                let mut data = json!({"path": path, "kind": "schematic"});
                                if asked == PROJECT_DOC {
                                    data["kind"] = json!("project");
                                    data["save_writes"] = json!(save_writes);
                                }
                                json!({"id": "document_info", "status": "answered",
                                    "data": data, "messages": [], "changed": []})
                            }
                            None => json!({"id": "document_info", "status": "refused",
                                "reason": "no document has the id", "messages": [], "changed": []}),
                        }
                    } else {
                        replies.pop_front().unwrap_or_else(|| json!({}))
                    };
                    writeln!(reader.get_mut(), "{reply}").unwrap();
                }
            }
        });
        Self {
            requests,
            stop,
            thread: Some(thread),
            window: None,
        }
    }

    /// Stop serving and return every command it was sent (not the
    /// `document_info` lookups that check a document before a command).
    fn finish(mut self) -> Vec<Value> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        let sent = self.requests.lock().unwrap().clone();
        sent.into_iter()
            .filter(|request| request["id"] != "document_info")
            .collect()
    }
}

fn answered(data: Value, changed: &[&str]) -> Value {
    // The stand-in answers whatever it is asked, so it names no command.
    json!({"id": null, "status": "answered", "data": data,
        "messages": [{"level": "INFO", "message": "from the app"}], "changed": changed})
}

#[test]
fn a_run_with_the_project_open_in_a_running_app_runs_there_and_prints_the_headless_result() {
    let w = world();
    let doc = w.fx.sheet.to_str().unwrap();
    let edits = json!([{"id": "place_junction", "args": {"at": [20, 20]}}]);
    let app = w.app(&[
        answered(json!({"documents": 1}), &[]),
        json!({"id": "apply_edits", "status": "answered", "data": {"applied": 1},
            "messages": [{"level": "INFO", "message": "from the app"}], "changed": [doc]}),
    ]);
    let before = snapshot(&w.fx.dir);

    let commands = jsonl(&[
        json!({"id": "list_documents"}),
        json!({"id": "apply_edits", "args": {"document": doc, "expect_revision": 3, "edits": edits}}),
    ]);
    let out = w.cli(&["run", w.project()], &commands);
    let sent = app.finish();

    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(out.lines.len(), 2, "{}", out.stdout);
    let (first, second) = (&out.lines[0], &out.lines[1]);
    assert_eq!(first["mode"], "attached");
    assert_eq!(first["status"], "answered");
    assert_eq!(first["id"], "list_documents");
    assert_eq!(first["seq"], 1);
    assert_eq!(first["data"], json!({"documents": 1}));
    // The app's log lines are shown for no command: they may name another
    // open project's files (the app's own Messages panel keeps them all).
    assert_eq!(first["messages"], json!([]));
    assert_eq!(second["messages"], json!([]));
    assert_eq!(second["seq"], 2);
    assert_eq!(second["changed"], json!([doc]));
    assert_eq!(first["run"], second["run"], "one run id");

    // What the app received: the token, the id, the operands (the edit list as
    // JSON), and nothing was written to disk from here.
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent[0],
        json!({"token": TOKEN, "id": "list_documents", "args": {}})
    );
    assert_eq!(sent[1]["id"], "apply_edits");
    assert_eq!(sent[1]["token"], TOKEN);
    assert_eq!(sent[1]["args"]["document"], doc);
    assert_eq!(sent[1]["args"]["expect_revision"], "3");
    assert_eq!(sent[1]["args"]["edits"][0]["id"], "place_junction");
    assert_eq!(snapshot(&w.fx.dir), before, "the CLI wrote a file");
    // The app's entry is not the run's to touch.
    assert_eq!(w.entry().unwrap()["kind"], "gui");
}

#[test]
fn the_runs_policy_applies_before_anything_is_sent_and_the_apps_refusals_come_back() {
    let w = world();
    let refusal = json!({"id": "save_document", "status": "refused",
        "reason": "saving the person's file is not available to agents yet",
        "messages": [], "changed": []});
    let app = w.app(&[refusal]);

    // No --save: refused here, the app is never asked.
    let out = w.cli(&["run", w.project(), "save_document"], "");
    assert_eq!(out.code, Some(1));
    assert_eq!(out.lines[0]["status"], "refused");
    assert_eq!(out.lines[0]["mode"], "attached");
    assert!(
        out.lines[0]["reason"]
            .as_str()
            .unwrap()
            .contains("pass --save")
    );
    // A path outside the project is confined before it is sent.
    let outside = w.cli(
        &[
            "run",
            w.project(),
            "open_document",
            "--arg",
            "path=/etc/hosts",
        ],
        "",
    );
    assert_eq!(outside.lines[0]["status"], "refused");
    assert!(
        outside.lines[0]["reason"]
            .as_str()
            .unwrap()
            .contains("--allow-path")
    );
    // An edit inside apply_edits is held to the same policy.
    let edits = json!([{"id": "save_document"}]);
    let doc = w.fx.sheet.to_str().unwrap();
    let nested = jsonl(&[json!({"id": "apply_edits", "args": {"document": doc, "edits": edits}})]);
    let nested = w.cli(&["run", w.project()], &nested);
    assert_eq!(nested.lines[0]["status"], "refused");
    assert!(
        nested.lines[0]["reason"]
            .as_str()
            .unwrap()
            .contains("edit 1")
    );
    assert!(
        app.finish().is_empty(),
        "something was sent past the policy"
    );

    // With --save the command goes out, and the app's reason comes back.
    let w = world();
    let refusal = json!({"id": "save_document", "status": "refused",
        "reason": "saving the person's file is not available to agents yet",
        "messages": [], "changed": []});
    let app = w.app(&[refusal]);
    let document = format!("document={}", w.fx.sheet.display());
    let out = w.cli(
        &[
            "run",
            w.project(),
            "save_document",
            "--arg",
            &document,
            "--save",
        ],
        "",
    );
    let sent = app.finish();
    assert_eq!(out.code, Some(1));
    assert_eq!(out.lines[0]["status"], "refused");
    assert_eq!(
        out.lines[0]["reason"],
        "saving the person's file is not available to agents yet"
    );
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["id"], "save_document");
}

/// The app only knows which projects it has open: a document of another open
/// project, named by id, is resolved to its file and confined like a path.
#[test]
fn a_document_of_another_open_project_is_refused_by_id_and_this_projects_is_not() {
    let w = world();
    let app = w.app(&[]);
    let edit = |args: Value| json!({"id": "place_junction", "args": args});
    let on_b = json!({"document": SHEET_B, "edits": [edit(json!({"at": [1, 2]}))]});
    let nested = json!({"document": SHEET_A, "edits": [
        {"id": "color_net", "args": {"document": SHEET_B, "at": [0, 0], "color": "#336699"}}]});
    let unknown = json!({"document": "33333333-3333-4333-8333-333333333333", "edits": []});
    let commands = jsonl(&[
        json!({"id": "read_document", "args": {"document": SHEET_B}}),
        json!({"id": "apply_edits", "args": on_b}),
        json!({"id": "apply_edits", "args": nested}),
        json!({"id": "document_info", "args": {"document": unknown["document"]}}),
        json!({"id": "apply_edits", "args": unknown}),
    ]);
    let out = w.cli(&["run", w.project(), "--keep-going"], &commands);
    let sent = app.finish();

    assert_eq!(out.lines.len(), 5, "{}", out.stdout);
    for (index, line) in out.lines.iter().enumerate() {
        assert_eq!(line["status"], "refused", "line {index}: {line}");
    }
    let reason = |index: usize| out.lines[index]["reason"].as_str().unwrap().to_string();
    assert!(reason(0).contains("--allow-path"), "{}", reason(0));
    assert!(reason(1).contains("--allow-path"), "{}", reason(1));
    assert!(
        reason(2).starts_with("edit 1 (`color_net`)"),
        "{}",
        reason(2)
    );
    assert!(
        reason(3).contains("no document has the id"),
        "{}",
        reason(3)
    );
    assert!(
        reason(4).contains("no document has the id"),
        "{}",
        reason(4)
    );
    assert!(sent.is_empty(), "a command reached the app: {sent:?}");
    // A refusal does not tell where the other project's file is.
    assert!(
        !out.stdout.contains("somewhere/else") && !out.stdout.contains("ProjectB"),
        "a refusal named the other project's file: {}",
        out.stdout
    );

    // A document of this project, by id, goes through.
    let w = world();
    let done = json!({"id": "apply_edits", "status": "answered", "data": {},
        "messages": [], "changed": []});
    let app = w.app(&[answered(json!({}), &[]), done]);
    let commands = jsonl(&[
        json!({"id": "read_document", "args": {"document": SHEET_A}}),
        json!({"id": "apply_edits", "args": {"document": SHEET_A,
            "edits": [edit(json!({"at": [1, 2]}))]}}),
    ]);
    let out = w.cli(&["run", w.project()], &commands);
    let sent = app.finish();
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1]["args"]["document"], SHEET_A);
}

/// The app answers a query across every project it has open. Each of the five
/// queries shows this run only what lies inside its own folders: nothing of
/// another open project's paths, text or log lines reaches standard output.
#[test]
fn every_query_answer_is_filtered_to_the_runs_folders() {
    let w = world();
    let sheet = w.fx.sheet.to_str().unwrap().to_string();
    let project = w.project().to_string();
    let b = OTHER_PROJECT_SHEET;
    let secret_log = json!([{"level": "INFO", "message": format!("read {b} SECRET-PART")}]);
    let reply = |id: &str, data: Value, changed: Value| {
        json!({"id": id, "status": "answered", "data": data,
            "messages": secret_log, "changed": changed})
    };
    let element = |text: &str, path: &str| {
        json!({"id": "e", "kind": "symbol", "text": text,
            "document": {"id": "d", "path": path, "name": "n"}})
    };
    let app = w.app(&[
        reply(
            "list_documents",
            json!({"count": 2, "documents": [
                {"id": SHEET_A, "path": sheet, "project": {"path": project}},
                {"id": SHEET_B, "path": b, "project": {"path": "/somewhere/else/B.snxprj"}}]}),
            json!([sheet, b]),
        ),
        reply(
            "find_items",
            json!({"count": 2, "elements": [element("mine", &sheet), element("SECRET-PART", b)],
                "not_loaded": [b, sheet]}),
            json!([]),
        ),
        reply(
            "resolve",
            json!({"found": true, "what": "element", "element": element("SECRET-PART", b)}),
            json!([]),
        ),
        reply(
            "resolve",
            json!({"found": true, "what": "element", "element": element("mine", &sheet)}),
            json!([]),
        ),
        // (`document_info` is answered by the stand-in from its document list.)
        reply(
            "read_document",
            json!({"revision": 3, "text": "mine"}),
            json!([]),
        ),
    ]);
    let commands = jsonl(&[
        json!({"id": "list_documents"}),
        json!({"id": "find_items", "args": {"kind": "symbol", "project": "B"}}),
        json!({"id": "resolve", "args": {"id": "44444444-4444-4444-8444-444444444444"}}),
        json!({"id": "resolve", "args": {"id": "55555555-5555-4555-8555-555555555555"}}),
        json!({"id": "document_info", "args": {"document": SHEET_A}}),
        json!({"id": "read_document", "args": {"document": SHEET_A}}),
    ]);
    let out = w.cli(&["run", w.project()], &commands);
    app.finish();

    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(out.lines.len(), 6, "{}", out.stdout);
    for leaked in ["SECRET-PART", "somewhere/else"] {
        assert!(
            !out.stdout.contains(leaked),
            "{leaked} leaked: {}",
            out.stdout
        );
    }
    let data = |index: usize| &out.lines[index]["data"];
    // list_documents: B is gone and the count follows.
    assert_eq!(data(0)["count"], 1);
    assert_eq!(data(0)["documents"][0]["id"], SHEET_A);
    assert_eq!(out.lines[0]["changed"], json!([sheet]));
    // find_items: B's element is gone, so is B in not_loaded; the count follows.
    assert_eq!(data(1)["count"], 1);
    assert_eq!(data(1)["elements"][0]["text"], "mine");
    assert_eq!(data(1)["not_loaded"], json!([sheet]));
    // resolve: an element of B is answered as an id that does not exist.
    assert_eq!(
        data(2),
        &json!({"found": false, "what": "unknown",
            "id": "44444444-4444-4444-8444-444444444444"})
    );
    assert_eq!(data(3)["found"], true);
    assert_eq!(data(3)["element"]["text"], "mine");
    // document_info and read_document of this project's document are whole.
    assert_eq!(data(4)["path"], sheet);
    assert_eq!(data(5)["revision"], 3);
    // The app's log lines of a query are not shown.
    for line in &out.lines {
        assert_eq!(line["messages"], json!([]), "{line}");
    }
}

/// A sheet never saved has no path to check. It is shown only when the project
/// that owns it is this run's; its elements carry no project, so they are
/// hidden (documented in `docs/CLI.md`). Nothing of another project's unsaved
/// sheet shows, whatever the query.
#[test]
fn an_unsaved_sheet_of_another_project_and_its_elements_never_show() {
    let w = world();
    let sheet = w.fx.sheet.to_str().unwrap().to_string();
    let project = w.project().to_string();
    let doc = |id: &str, name: &str, owner: &str| json!({"id": id, "path": null, "name": name, "project": {"path": owner}});
    let element = json!({"id": "e", "text": "SECRET-UNSAVED",
        "document": {"id": "u", "path": null, "name": "SECRET-UNSAVED"}});
    let reply = |id: &str, data: Value| json!({"id": id, "status": "answered", "data": data, "messages": [], "changed": []});
    let app = w.app(&[
        reply(
            "list_documents",
            json!({"count": 3, "documents": [
                {"id": SHEET_A, "path": sheet, "name": "a", "project": {"path": project}},
                doc("66666666-6666-4666-8666-666666666666", "mine-unsaved", &project),
                doc(SHEET_B, "SECRET-UNSAVED", "/somewhere/else/B.snxprj")]}),
        ),
        reply(
            "find_items",
            json!({"count": 1, "elements": [element], "not_loaded": []}),
        ),
        reply(
            "resolve",
            json!({"found": true, "what": "element", "element": element}),
        ),
    ]);
    let commands = jsonl(&[
        json!({"id": "list_documents"}),
        json!({"id": "find_items", "args": {"kind": "symbol"}}),
        json!({"id": "resolve", "args": {"id": "77777777-7777-4777-8777-777777777777"}}),
    ]);
    let out = w.cli(&["run", w.project()], &commands);
    app.finish();

    assert_eq!(out.lines.len(), 3, "{}", out.stdout);
    assert!(!out.stdout.contains("SECRET-UNSAVED"), "{}", out.stdout);
    assert!(!out.stdout.contains("somewhere/else"), "{}", out.stdout);
    let listed = &out.lines[0]["data"];
    assert_eq!(listed["count"], 2);
    assert_eq!(listed["documents"][1]["name"], "mine-unsaved");
    assert_eq!(out.lines[1]["data"]["count"], 0);
    assert_eq!(out.lines[2]["data"]["what"], "unknown");
}

/// The run's own sheet comes back whole: the `path` of an instance, or of a
/// library, is not a file, and only the keys that name documents are judged.
#[test]
fn a_read_of_the_runs_own_sheet_comes_back_byte_equal() {
    let w = world();
    let sheet = w.fx.sheet.to_str().unwrap().to_string();
    let data = json!({
        "document": {"id": SHEET_A, "path": sheet, "name": "a", "project": null},
        "header": {"instances": [{"path": "/aaaa-bbbb/cccc-dddd", "reference": "R1"}],
            "libraries": [{"path": "libs/Device.snxlib", "kind": "shared"}]},
        "elements": {"symbol": [{"id": "s", "path": "/aaaa-bbbb", "ref": "R1"}]},
        "total": 1, "offset": 0, "limit": 100, "next_offset": null});
    let app = w.app(&[
        json!({"id": "read_document", "status": "answered", "data": data,
        "messages": [], "changed": []}),
    ]);
    let out = w.cli(
        &[
            "run",
            w.project(),
            "read_document",
            "--arg",
            &format!("document={SHEET_A}"),
        ],
        "",
    );
    app.finish();
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(out.lines[0]["data"], data);
}

/// No command's log lines are shown: an edit's can name another project's
/// files too (a duplicate document id, another thread's line).
#[test]
fn an_edits_log_lines_are_not_shown() {
    let w = world();
    let line = json!([{"level": "WARN",
        "message": "has the same document id as /somewhere/else/ProjectB/b.snxsch"}]);
    let app = w.app(&[
        json!({"id": "apply_edits", "status": "answered", "data": {},
        "messages": line, "changed": []}),
    ]);
    let edits = json!([{"id": "place_junction", "args": {"at": [1, 2]}}]);
    let commands = jsonl(&[json!({"id": "apply_edits",
        "args": {"document": SHEET_A, "edits": edits}})]);
    let out = w.cli(&["run", w.project()], &commands);
    app.finish();
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(out.lines[0]["messages"], json!([]));
    assert!(!out.stdout.contains("somewhere/else"), "{}", out.stdout);
}

/// `save_document` is on the agent channel (#2231): the CLI's `--save` gate and
/// its confinement still apply first (a project file's save is the next tests).
#[test]
fn an_attached_save_needs_save_and_stays_inside_the_runs_folders() {
    let w = world();
    let done = json!({"id": "save_document", "status": "ran", "messages": [], "changed": []});
    let app = w.app(&[done]);
    let save = |document: &str, flags: &[&str]| {
        let arg = format!("document={document}");
        let mut args = vec!["run", w.project(), "save_document", "--arg", &arg];
        args.extend_from_slice(flags);
        w.cli(&args, "")
    };
    // No --save: refused here, and the document is not even asked about.
    let plain = save(SHEET_A, &[]);
    assert_eq!(plain.lines[0]["status"], "refused", "{}", plain.stdout);
    assert!(
        plain.lines[0]["reason"]
            .as_str()
            .unwrap()
            .contains("pass --save")
    );
    // A document of another open project: refused, its file is not named.
    let other = save(SHEET_B, &["--save"]);
    assert_eq!(other.lines[0]["status"], "refused", "{}", other.stdout);
    assert!(
        other.lines[0]["reason"]
            .as_str()
            .unwrap()
            .contains("--allow-path")
    );
    assert!(!other.stdout.contains("somewhere/else"), "{}", other.stdout);
    // A sheet of this project, with --save, is sent.
    let sheet = save(SHEET_A, &["--save"]);
    assert_eq!(sheet.code, Some(0), "{}", sheet.stderr);
    assert_eq!(sheet.lines[0]["status"], "ran");
    let sent = app.finish();
    assert_eq!(sent.len(), 1, "only the sheet's save was sent: {sent:?}");
    assert_eq!(sent[0]["id"], "save_document");
    assert_eq!(sent[0]["args"]["document"], SHEET_A);
}

/// Saving the project file is allowed from an attached run (#1924) when the
/// file and every file the app says the save also writes (`save_writes`, the
/// libraries it has pending) are inside the run's folders.
#[test]
fn an_attached_project_save_goes_through_when_every_file_it_writes_is_inside() {
    let w = world();
    let done = json!({"id": "save_document", "status": "ran", "messages": [], "changed": []});
    let save = |w: &World| {
        w.cli(
            &[
                "run",
                w.project(),
                "save_document",
                "--arg",
                &format!("document={PROJECT_DOC}"),
                "--save",
            ],
            "",
        )
    };

    // No pending library: the project file alone.
    let app = w.app(&[done.clone()]);
    let out = save(&w);
    assert_eq!(out.lines[0]["status"], "ran", "{}", out.stdout);
    assert_eq!(app.finish().len(), 1);
    drop(w);

    // A pending library inside the project folder.
    let w = world();
    let inside = w.fx.dir.join("libs").join("mine.snxlib");
    let app = w.app_saving(&[done], vec![inside.to_str().unwrap().into()]);
    let out = save(&w);
    assert_eq!(out.lines[0]["status"], "ran", "{}", out.stdout);
    let sent = app.finish();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0]["args"]["document"], PROJECT_DOC);
}

/// A pending library outside the run's folders refuses the whole save: nothing
/// is sent to the app, and the refusal names that file.
#[test]
fn an_attached_project_save_is_refused_when_a_pending_library_is_outside() {
    let w = world();
    let outside = w.dir.join("elsewhere").join("theirs.snxlib");
    let outside = outside.to_str().unwrap();
    let done = json!({"id": "save_document", "status": "ran", "messages": [], "changed": []});
    let app = w.app_saving(&[done], vec![outside.into()]);
    let out = w.cli(
        &[
            "run",
            w.project(),
            "save_document",
            "--arg",
            &format!("document={PROJECT_DOC}"),
            "--save",
        ],
        "",
    );
    assert_eq!(out.lines[0]["status"], "refused", "{}", out.stdout);
    let reason = out.lines[0]["reason"].as_str().unwrap();
    assert!(reason.contains(outside), "{reason}");
    assert!(reason.contains("--allow-path"), "{reason}");
    assert!(app.finish().is_empty(), "the app was sent the save");
    assert!(!Path::new(outside).exists());
}

/// `query_pins` reads one named sheet: another open project's part cannot be
/// asked about, and this project's answer comes through whole.
#[test]
fn a_pin_query_cannot_reach_another_open_projects_part() {
    let w = world();
    let pins = json!({"designator": "R1", "count": 1, "pins": [
        {"number": "1", "name": "A", "electrical_type": "passive", "hidden": false,
         "unit": 1, "shared": false, "tip": [1.0, 2.0]}]});
    let app = w.app(&[
        json!({"id": "query_pins", "status": "answered", "data": pins,
        "messages": [], "changed": []}),
    ]);
    let commands = jsonl(&[
        json!({"id": "query_pins", "args": {"document": SHEET_B, "designator": "R1"}}),
        json!({"id": "query_pins", "args": {"document": SHEET_A, "designator": "R1"}}),
    ]);
    let out = w.cli(&["run", w.project(), "--keep-going"], &commands);
    let sent = app.finish();

    assert_eq!(out.lines.len(), 2, "{}", out.stdout);
    assert_eq!(out.lines[0]["status"], "refused", "{}", out.lines[0]);
    assert!(
        out.lines[0]["reason"]
            .as_str()
            .unwrap()
            .contains("--allow-path")
    );
    assert_eq!(out.lines[1]["status"], "answered", "{}", out.lines[1]);
    assert_eq!(out.lines[1]["data"], pins);
    assert_eq!(
        sent.len(),
        1,
        "the other project's query was sent: {sent:?}"
    );
    assert_eq!(sent[0]["args"]["document"], SHEET_A);
}

/// Sent with no `document`, the app would act on the person's active
/// document, which the run's folders were never checked against.
#[test]
fn a_command_that_acts_on_a_document_must_name_it_and_is_never_sent_without_one() {
    let w = world();
    let app = w.app(&[answered(json!({"documents": 0}), &[])]);
    let edits = json!([{"id": "place_junction", "args": {"at": [1, 2]}}]);
    let commands = jsonl(&[
        json!({"id": "undo"}),
        json!({"id": "color_net", "args": {"at": [0, 0], "color": "#336699"}}),
        json!({"id": "apply_edits", "args": {"edits": edits}}),
        json!({"id": "list_documents"}),
    ]);
    let out = w.cli(&["run", w.project(), "--keep-going"], &commands);
    let sent = app.finish();

    assert_eq!(out.lines.len(), 4, "{}", out.stdout);
    for line in &out.lines[..3] {
        assert_eq!(line["status"], "refused", "{line}");
        let reason = line["reason"].as_str().unwrap();
        assert!(reason.contains("name the document"), "{line}");
    }
    // A query that names no document is fine: it acts on none.
    assert_eq!(out.lines[3]["status"], "answered");
    assert_eq!(sent.len(), 1, "only the query was sent: {sent:?}");
    assert_eq!(sent[0]["id"], "list_documents");
}

/// With agent connections off the app lists its open project with no socket:
/// nothing to attach to, and the same files are not edited behind it, whatever
/// the route.
#[test]
fn a_project_open_in_a_signex_that_does_not_listen_stops_every_route() {
    let w = world();
    let _app_lock = w.open_without_listening();
    let theirs = w.entry().unwrap();
    let want = format!(
        "{} is open in Signex (pid {}). To let an agent work on it, turn on Preferences > \
         Allow agent connections.",
        std::fs::canonicalize(&w.fx.project).unwrap().display(),
        std::process::id()
    );
    for flags in [&[][..], &["--headless"][..], &["--attach"][..]] {
        let mut args = vec!["run", w.project(), "list_documents"];
        args.extend_from_slice(flags);
        let out = w.cli(&args, "");
        assert_eq!(out.code, Some(2), "{flags:?}: {}", out.stderr);
        assert!(out.stderr.contains(&want), "{flags:?}: {}", out.stderr);
        assert!(out.lines.is_empty(), "{flags:?}: it ran anyway");
        assert_eq!(
            w.entry().unwrap(),
            theirs,
            "{flags:?}: the app's entry was touched"
        );
    }
}

/// The app crashed: its entry is left, its lock is gone, and a living process
/// may even have its pid. The run is not stopped by a ghost.
#[test]
fn an_entry_left_by_a_crashed_signex_is_stale_and_the_run_goes_on() {
    let w = world();
    w.plant("gui", std::process::id(), CC, Path::new(""));
    let out = w.cli(&["run", w.project(), "list_documents"], "");
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(out.lines[0]["mode"], "headless");
}

#[test]
fn a_stale_entry_is_removed_and_the_run_is_headless() {
    for (why, kind, pid, boot, socket) in [
        ("a dead pid", "gui", DEAD_PID, CC, "endpoint"),
        (
            "a malformed boot id",
            "gui",
            std::process::id(),
            "cc",
            "endpoint",
        ),
        ("a dead headless run", "headless", DEAD_PID, CC, ""),
    ] {
        let w = world();
        let socket = if socket.is_empty() {
            PathBuf::new()
        } else {
            w.socket(boot)
        };
        w.plant(kind, pid, boot, &socket);

        let out = w.cli(&["run", w.project(), "list_documents"], "");
        assert_eq!(out.code, Some(0), "{why}: {}", out.stderr);
        assert_eq!(out.lines[0]["mode"], "headless", "{why}");
        assert_eq!(out.lines[0]["status"], "answered", "{why}");
        assert!(w.entry().is_none(), "{why}: the stale entry was left");
    }
}

#[test]
fn a_stale_entry_that_is_a_link_is_not_followed_or_removed() {
    let w = world();
    let precious = w.dir.join("precious");
    std::fs::write(&precious, "keep me").unwrap();
    symlink(&precious, w.entry_file()).unwrap();

    // The link is not an entry to read, and the hold is not written through
    // it: the run does not start, and nothing the link names is touched.
    let out = w.cli(&["run", w.project(), "list_documents"], "");
    assert_eq!(out.code, Some(2), "{}", out.stderr);
    assert!(out.stderr.contains("is a link"), "{}", out.stderr);
    assert!(out.lines.is_empty());
    assert_eq!(std::fs::read_to_string(&precious).unwrap(), "keep me");
    assert!(
        w.entry_file()
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

/// The entry is data another process wrote: the CLI connects to the endpoint
/// the boot id names and to nothing else.
#[test]
fn a_planted_entry_naming_another_socket_is_stale_and_that_socket_is_never_connected() {
    let w = world();
    let planted = w.dir.join("planted.sock");
    let app = FakeApp::serve(
        {
            let listener = UnixListener::bind(&planted).unwrap();
            listener.set_nonblocking(true).unwrap();
            listener
        },
        &[],
        Vec::new(),
        Vec::new(),
    );
    w.token(CC, 0o600);
    // A live process, and a socket that answers: not the one CC names.
    w.plant("gui", std::process::id(), CC, &planted);

    let out = w.cli(&["run", w.project(), "list_documents"], "");
    let sent = app.finish();

    assert_eq!(out.lines[0]["mode"], "headless", "{}", out.stderr);
    assert!(sent.is_empty(), "a request reached the planted socket");
    assert!(w.entry().is_none(), "the planted entry was left");
}

#[test]
fn a_token_file_that_is_a_link_wide_or_foreign_means_no_attach_and_nothing_is_sent() {
    for how in ["link", "wide", "garbage"] {
        let w = world();
        let app = w.app(&[]);
        let token = w.dir.join(format!("{CC}.token"));
        match how {
            "link" => {
                let real = w.dir.join("real.token");
                std::fs::rename(&token, &real).unwrap();
                symlink(&real, &token).unwrap();
            }
            "wide" => {
                std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o644)).unwrap()
            }
            _ => std::fs::write(&token, "not a token").unwrap(),
        }
        let out = w.cli(&["run", w.project(), "list_documents"], "");
        let sent = app.finish();
        assert_eq!(out.code, Some(2), "{how}: {}", out.stderr);
        assert!(
            out.stderr.contains("session token"),
            "{how}: {}",
            out.stderr
        );
        assert!(out.lines.is_empty(), "{how}: it fell back to headless");
        assert!(sent.is_empty(), "{how}: something was sent");
    }
}

#[test]
fn a_headless_run_holds_its_project_while_it_runs_and_releases_it_at_the_end() {
    let w = world();
    let mut live = Live::start_in(&w.runtime, &["run", w.project()]);
    let first = live.send(&json!({"id": "list_documents"}));
    assert_eq!(first["mode"], "headless");

    let held = w.entry().expect("a hold while the run lasts");
    assert_eq!(held["kind"], "headless");
    assert_eq!(held["pid"], live.pid());
    assert_eq!(held["socket"], "");
    assert_eq!(
        held["project"],
        json!(std::fs::canonicalize(&w.fx.project).unwrap())
    );
    assert_eq!(
        std::fs::metadata(w.entry_file())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    live.finish();
    assert!(w.entry().is_none(), "the hold outlived the run");
}

#[test]
fn the_hold_is_released_when_the_run_fails_to_start_or_ends_with_a_refused_command() {
    let w = world();
    // The project file cannot be opened: the run ends in an error.
    std::fs::write(&w.fx.project, "not a project").unwrap();
    let broken = w.cli(&["run", w.project(), "list_documents"], "");
    assert_eq!(broken.code, Some(2), "{}", broken.stderr);
    assert!(w.entry().is_none(), "an error left the project held");

    // A command refused: exit 1, the hold is released.
    let w = world();
    let refused = w.cli(&["run", w.project(), "save_document"], "");
    assert_eq!(refused.code, Some(1));
    assert_eq!(refused.lines[0]["mode"], "headless");
    assert!(
        w.entry().is_none(),
        "a refused command left the project held"
    );
}

/// A second headless run on a held project is refused, with the pid of the
/// holder, and the holder's entry is not touched.
#[test]
fn a_run_that_finds_another_headless_run_holding_the_project_is_refused() {
    let w = world();
    let mut other = Live::start_in(&w.runtime, &["run", w.project()]);
    other.send(&json!({"id": "list_documents"}));
    let theirs = w.entry().unwrap();
    for flags in [&[][..], &["--headless"][..]] {
        let mut args = vec!["run", w.project(), "list_documents"];
        args.extend_from_slice(flags);
        let out = w.cli(&args, "");
        assert_eq!(out.code, Some(2), "{flags:?}: {}", out.stderr);
        let want = format!("held by another signex-cli run (pid {})", other.pid());
        assert!(out.stderr.contains(&want), "{flags:?}: {}", out.stderr);
        assert!(out.lines.is_empty(), "{flags:?}: it ran anyway");
        assert_eq!(w.entry().unwrap(), theirs, "the holder's entry was touched");
    }
    other.finish();
}

/// A runtime folder other users can enter is not trusted, so a run that cannot
/// tell who holds its project does not go on without holding it.
#[test]
fn a_runtime_folder_that_others_can_enter_stops_the_run() {
    let w = world();
    std::fs::set_permissions(&w.dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = w.cli(&["run", w.project(), "list_documents"], "");
    assert_eq!(out.code, Some(2), "{}", out.stderr);
    assert!(out.stderr.contains("wider than 700"), "{}", out.stderr);
    assert!(out.lines.is_empty(), "it ran without a hold");
}

/// A run killed by a signal leaves its entry; the kernel dropped its lock, so
/// the next run finds it stale. The same for a pid that a living process reuses.
#[test]
fn a_killed_run_and_a_reused_pid_leave_a_stale_hold_and_the_next_run_goes_on() {
    let w = world();
    let mut doomed = Live::start_in(&w.runtime, &["run", w.project()]);
    doomed.send(&json!({"id": "list_documents"}));
    assert!(w.entry().is_some());
    doomed.kill();
    assert!(w.entry().is_some(), "a killed run cleaned up after itself");
    let out = w.cli(&["run", w.project(), "list_documents"], "");
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(out.lines[0]["mode"], "headless");
    assert!(w.entry().is_none());

    // Process 1 runs for ever and holds no lock.
    w.plant("headless", 1, CC, Path::new(""));
    let out = w.cli(&["run", w.project(), "list_documents"], "");
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(out.lines[0]["mode"], "headless");
}

#[test]
fn attach_fails_without_a_live_entry_and_headless_fails_while_an_app_holds_the_project() {
    let w = world();
    let none = w.cli(&["run", w.project(), "list_documents", "--attach"], "");
    assert_eq!(none.code, Some(2));
    assert!(
        none.stderr.contains("--attach: no running Signex"),
        "{}",
        none.stderr
    );
    assert!(none.lines.is_empty(), "it ran headless anyway");
    assert!(w.entry().is_none(), "a failed --attach left an entry");

    // A stale entry is no live entry.
    w.plant("gui", DEAD_PID, CC, &w.socket(CC));
    let stale = w.cli(&["run", w.project(), "list_documents", "--attach"], "");
    assert_eq!(stale.code, Some(2));
    assert!(stale.lines.is_empty());
    assert!(
        w.entry().is_none(),
        "the stale entry was left after --attach"
    );

    // A headless run holding it is not a running Signex either.
    let mut run = Live::start_in(&w.runtime, &["run", w.project()]);
    run.send(&json!({"id": "list_documents"}));
    let held = w.cli(&["run", w.project(), "list_documents", "--attach"], "");
    assert_eq!(held.code, Some(2));
    assert!(held.stderr.contains("headless run"), "{}", held.stderr);
    run.finish();

    // With a live app: --headless refuses to edit a file open in a window,
    // and --attach works.
    let w = world();
    let app = w.app(&[answered(json!({}), &[])]);
    let before = snapshot(&w.fx.dir);
    let headless = w.cli(&["run", w.project(), "list_documents", "--headless"], "");
    assert_eq!(headless.code, Some(2), "{}", headless.stderr);
    assert!(
        headless.stderr.contains("open in a running Signex"),
        "{}",
        headless.stderr
    );
    assert!(headless.lines.is_empty(), "it ran headless anyway");
    assert_eq!(snapshot(&w.fx.dir), before);
    let attached = w.cli(&["run", w.project(), "list_documents", "--attach"], "");
    assert_eq!(attached.code, Some(0), "{}", attached.stderr);
    assert_eq!(attached.lines[0]["mode"], "attached");
    assert_eq!(app.finish().len(), 1, "only the attached run sent anything");
}
