//! The security rules of #1923, each shown end to end: what is refused, with
//! which reason, and that nothing was written.

mod support;

use serde_json::{Value, json};
use support::{Live, fixture, jsonl, list_libraries, run_cli, snapshot, temp_root};

fn refusal(out: &support::Output, index: usize) -> &str {
    assert_eq!(out.lines[index]["status"], "refused", "{}", out.stdout);
    out.lines[index]["reason"].as_str().unwrap()
}

fn one(command: Value) -> String {
    jsonl(&[command])
}

#[test]
fn a_command_outside_the_default_capabilities_names_the_one_it_needs() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let before = snapshot(&root);

    let save_as = one(
        json!({"id": "save_document_as", "args": {"document": "main.snxsch", "path": root.join("proj").join("copy.snxsch")}}),
    );
    let out = run_cli(&["run", project], &save_as);
    assert_eq!(
        refusal(&out, 0),
        "`save_document_as` needs the `save` capability; pass --save"
    );

    // The export commands only work in a window (#2170): refused for that
    // before their capability is asked.
    let bom = run_cli(&["run", project], one(json!({"id": "report_manager_bom"})));
    assert_eq!(
        refusal(&bom, 0),
        "`report_manager_bom` only works in a window"
    );
    assert_eq!(snapshot(&root), before);
}

#[test]
fn a_path_outside_the_project_is_refused_and_allow_path_opens_that_folder_only() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    let outside = other.join("outside.snxsch");
    std::fs::copy(&fx.sheet, &outside).unwrap();
    let project = fx.project.to_str().unwrap();
    let open = one(json!({"id": "open_document", "args": {"path": outside}}));

    let denied = run_cli(&["run", project], &open);
    assert!(
        refusal(&denied, 0).contains("outside the project"),
        "{}",
        denied.stdout
    );
    let climbing = run_cli(
        &["run", project],
        one(json!({"id": "open_document", "args": {"path": "../../etc/passwd"}})),
    );
    assert_eq!(
        climbing.lines[0]["status"], "refused",
        "{}",
        climbing.stdout
    );

    let allowed = run_cli(
        &["run", project, "--allow-path", other.to_str().unwrap()],
        &open,
    );
    assert_eq!(
        allowed.lines[0]["status"], "answered",
        "{}\n{}",
        allowed.stdout, allowed.stderr
    );

    let (_third_guard, third) = temp_root();
    std::fs::copy(&fx.sheet, third.join("third.snxsch")).unwrap();
    let third_open =
        one(json!({"id": "open_document", "args": {"path": third.join("third.snxsch")}}));
    let still = run_cli(
        &["run", project, "--allow-path", other.to_str().unwrap()],
        &third_open,
    );
    assert_eq!(still.lines[0]["status"], "refused");
}

#[cfg(unix)]
#[test]
fn a_symlink_in_the_project_that_leaves_it_is_refused() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    std::fs::copy(&fx.sheet, other.join("secret.snxsch")).unwrap();
    std::os::unix::fs::symlink(&other, fx.dir.join("link")).unwrap();
    let out = run_cli(
        &["run", fx.project.to_str().unwrap()],
        one(json!({"id": "open_document", "args": {"path": "link/secret.snxsch"}})),
    );
    assert!(
        refusal(&out, 0).contains("outside the project"),
        "{}",
        out.stdout
    );
}

#[test]
fn a_gui_only_command_is_refused_with_the_catalogs_reason() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let out = run_cli(
        &["run", fx.project.to_str().unwrap()],
        one(json!({"id": "toggle_properties_panel"})),
    );
    assert_eq!(
        refusal(&out, 0),
        "`toggle_properties_panel` only works in a window"
    );
}

#[test]
fn malformed_input_is_a_refused_line_never_a_panic() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let mut input =
        b"not json\n[1,2]\n{\"id\":\"undo\",\"args\":{\"at\":null}}\n\0\n\xff\xfe\n".to_vec();
    input.extend_from_slice(OPEN_MAIN.as_bytes());

    let out = run_cli(&["run", project, "--keep-going"], &input);

    assert_eq!(out.lines.len(), 6, "{}\n{}", out.stdout, out.stderr);
    for (index, wanted) in [
        "malformed command line",
        "malformed command line",
        "operand `at` must be a string",
        "malformed command line",
        "not valid UTF-8",
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            refusal(&out, index).contains(wanted),
            "{index}: {}",
            out.stdout
        );
        assert!(out.lines[index]["id"].is_null());
    }
    assert_eq!(
        out.lines[5]["status"], "answered",
        "a good line after bad ones still runs"
    );
    assert!(!out.stderr.contains("panicked"), "{}", out.stderr);

    // Without --keep-going the first bad line ends the run.
    let stopped = run_cli(&["run", project], &input);
    assert_eq!(stopped.lines.len(), 1);
    assert_eq!(stopped.code, Some(1));
}

const OPEN_MAIN: &str = "{\"id\":\"open_document\",\"args\":{\"path\":\"main.snxsch\"}}\n";

#[test]
fn a_line_over_the_limit_and_a_run_over_the_command_limit_are_refused() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();

    let long = format!(
        "{{\"id\":\"undo\",\"args\":{{\"text\":\"{}\"}}}}\n{OPEN_MAIN}",
        "x".repeat(200)
    );
    let out = run_cli(
        &["run", project, "--max-line-bytes", "100", "--keep-going"],
        &long,
    );
    assert!(
        refusal(&out, 0).contains("longer than 100 bytes"),
        "{}",
        out.stdout
    );
    assert_eq!(
        out.lines[1]["status"], "answered",
        "the next line is read after a long one"
    );

    let three = jsonl(&[
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "select_all"}),
        json!({"id": "select_all"}),
    ]);
    let capped = run_cli(
        &["run", project, "--max-commands", "2", "--keep-going"],
        &three,
    );
    assert_eq!(capped.lines.len(), 3);
    assert_eq!(capped.lines[1]["status"], "ran");
    assert!(
        refusal(&capped, 2).contains("more than 2 commands"),
        "{}",
        capped.stdout
    );

    let bytes = std::fs::read(&fx.sheet).unwrap();
    let limit = bytes.len() - 1;
    assert!(std::fs::metadata(&fx.project).unwrap().len() < limit as u64);
    let big = run_cli(
        &["run", project, "--max-file-bytes", &limit.to_string()],
        one(json!({"id": "open_document", "args": {"path": "main.snxsch"}})),
    );
    assert!(
        refusal(&big, 0).contains("over the limit"),
        "{}",
        big.stdout
    );
}

#[test]
fn allow_and_save_grant_exactly_the_capability_they_name() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let save_as = jsonl(&[
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "save_document_as", "args": {"document": "main.snxsch", "path": "copy.snxsch"}}),
        json!({"id": "save_document_as", "args": {"document": "copy.snxsch", "path": "copy.snxsch"}}),
        json!({"id": "save_document_as", "args": {"document": "copy.snxsch", "path": "copy.snxsch", "overwrite": true}}),
    ]);
    let before = snapshot(&root);
    let out = run_cli(&["run", project, "--save", "--keep-going"], &save_as);
    assert_eq!(
        out.lines[1]["status"], "ran",
        "{}\n{}",
        out.stdout, out.stderr
    );
    assert!(fx.dir.join("copy.snxsch").is_file(), "the copy was written");
    assert!(
        refusal(&out, 2).contains("exists; pass overwrite=true"),
        "an existing file is not replaced without overwrite=true: {}",
        out.stdout
    );
    assert_eq!(
        out.lines[3]["status"], "ran",
        "overwrite=true replaces it: {}",
        out.stdout
    );
    assert_eq!(
        before[&fx.sheet],
        snapshot(&root)[&fx.sheet],
        "the original is untouched"
    );
}

fn open(path: &std::path::Path) -> Value {
    json!({"id": "open_document", "args": {"path": path}})
}

#[test]
fn a_projects_library_list_never_widens_the_roots() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    std::fs::copy(&fx.sheet, root.join("sibling.snxsch")).unwrap();
    std::fs::copy(&fx.sheet, other.join("outside.snxsch")).unwrap();
    // A folder that looks like a library, next to the project.
    let library = root.join("parts.snxlib");
    std::fs::create_dir_all(&library).unwrap();
    std::fs::write(library.join("library.toml"), "x").unwrap();
    std::fs::copy(&fx.sheet, library.join("in-library.snxsch")).unwrap();
    // A link to the outside folder, where a test can make one (unix).
    let link = link_to(&other, &fx.dir.join("lib-link"));
    let listed: Vec<std::path::PathBuf> = [
        fx.dir.join(".."),
        std::path::PathBuf::from(std::path::MAIN_SEPARATOR_STR),
        library.clone(),
    ]
    .into_iter()
    .chain(link.clone())
    .collect();
    let refs: Vec<&std::path::Path> = listed.iter().map(|p| p.as_path()).collect();
    list_libraries(&fx, &refs);
    let project = fx.project.to_str().unwrap();

    let attempts: Vec<std::path::PathBuf> = [
        root.join("sibling.snxsch"),
        library.join("in-library.snxsch"),
        other.join("outside.snxsch"),
    ]
    .into_iter()
    .chain(link.map(|link| link.join("outside.snxsch")))
    .collect();
    for target in attempts {
        let out = run_cli(&["run", project], jsonl(&[open(&target)]));
        assert!(
            refusal(&out, 0).contains("outside the project")
                || refusal(&out, 0).contains("symbolic"),
            "{}: {}",
            target.display(),
            out.stdout
        );
    }
}

/// A symbolic link `link` to `target`, on a platform where a test can make one.
#[cfg(unix)]
fn link_to(target: &std::path::Path, link: &std::path::Path) -> Option<std::path::PathBuf> {
    std::os::unix::fs::symlink(target, link).unwrap();
    Some(link.to_path_buf())
}

#[cfg(not(unix))]
fn link_to(_target: &std::path::Path, _link: &std::path::Path) -> Option<std::path::PathBuf> {
    None
}

/// Replace `path` by a link to a copy of it that lives in `elsewhere`.
#[cfg(unix)]
fn replace_with_link_to_outside(
    path: &std::path::Path,
    elsewhere: &std::path::Path,
) -> std::path::PathBuf {
    let copy = elsewhere.join(path.file_name().unwrap());
    std::fs::copy(path, &copy).unwrap();
    std::fs::remove_file(path).unwrap();
    std::os::unix::fs::symlink(&copy, path).unwrap();
    copy
}

#[cfg(unix)]
#[test]
fn a_save_refuses_a_document_that_became_a_link_to_outside_the_project() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    let mut live = Live::start(&[
        "run",
        fx.project.to_str().unwrap(),
        "--save",
        "--keep-going",
    ]);
    assert_eq!(live.send(&open(&fx.sheet))["status"], "answered");
    let paint = json!({"id": "color_net", "args": {"at": [5, 0], "color": "#336699", "document": "main.snxsch"}});
    assert_eq!(live.send(&paint)["status"], "ran");

    let outside = replace_with_link_to_outside(&fx.sheet, &other);
    let before = std::fs::read(&outside).unwrap();
    let saved = live.send(&json!({"id": "save_document", "args": {"document": "main.snxsch"}}));

    assert_eq!(saved["status"], "refused", "{saved}");
    let reason = saved["reason"].as_str().unwrap();
    assert!(reason.contains("symbolic link"), "{reason}");
    assert_eq!(
        std::fs::read(&outside).unwrap(),
        before,
        "nothing was written through the link"
    );
    live.finish();
}

#[cfg(unix)]
#[test]
fn a_save_refuses_a_project_file_that_became_a_link() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    let mut live = Live::start(&[
        "run",
        fx.project.to_str().unwrap(),
        "--save",
        "--keep-going",
    ]);
    assert_eq!(live.send(&open(&fx.sheet))["status"], "answered");

    let outside = replace_with_link_to_outside(&fx.project, &other);
    let before = std::fs::read(&outside).unwrap();
    let saved = live.send(&json!({"id": "save_document", "args": {"document": "demo.snxprj"}}));

    assert_eq!(saved["status"], "refused", "{saved}");
    assert!(
        saved["reason"].as_str().unwrap().contains("symbolic link"),
        "{saved}"
    );
    assert_eq!(std::fs::read(&outside).unwrap(), before);
    live.finish();
}

#[cfg(unix)]
#[test]
fn a_new_project_into_a_folder_that_is_a_link_to_outside_is_refused() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_allowed_guard, allowed) = temp_root();
    let (_outside_guard, outside) = temp_root();
    let outside_dir = outside.join("empty");
    std::fs::create_dir(&outside_dir).unwrap();
    std::os::unix::fs::symlink(&outside_dir, allowed.join("x")).unwrap();

    let out = run_cli(
        &[
            "run",
            fx.project.to_str().unwrap(),
            "--save",
            "--allow-path",
            allowed.to_str().unwrap(),
        ],
        jsonl(&[json!({"id": "new_document", "args": {"path": allowed.join("x.snxprj")}})]),
    );

    assert!(
        refusal(&out, 0).contains("the folder it creates beside it"),
        "{}",
        out.stdout
    );
    assert_eq!(
        std::fs::read_dir(&outside_dir).unwrap().count(),
        0,
        "nothing was created through the link"
    );
}

fn sheet_id(path: &std::path::Path) -> String {
    signex_types::format::document_id_in_header("snxsch", &std::fs::read(path).expect("sheet"))
        .expect("the sheet carries an id")
        .to_string()
}

/// #2172: a document named by id is confined as the file it names, not as a
/// string: a sheet that became a link to outside is refused for a read and a
/// write alike, and nothing is written through the link.
#[cfg(unix)]
#[test]
fn a_document_id_whose_file_leads_outside_is_refused_for_read_and_write() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    let id = sheet_id(&fx.sheet);
    let mut live = Live::start(&[
        "run",
        fx.project.to_str().unwrap(),
        "--save",
        "--keep-going",
    ]);
    assert_eq!(live.send(&open(&fx.sheet))["status"], "answered");
    let outside = replace_with_link_to_outside(&fx.sheet, &other);
    let before = std::fs::read(&outside).unwrap();

    let read = live.send(&json!({"id": "read_document", "args": {"document": id}}));
    assert_eq!(read["status"], "refused", "{read}");
    assert!(
        read["reason"]
            .as_str()
            .unwrap()
            .starts_with("operand `document`:"),
        "{read}"
    );
    let write = live.send(&json!({"id": "save_document", "args": {"document": id}}));
    assert_eq!(write["status"], "refused", "{write}");
    assert!(
        write["reason"].as_str().unwrap().contains("symbolic link"),
        "{write}"
    );
    let paint = live.send(&json!({"id": "color_net",
        "args": {"at": [5, 0], "color": "#336699", "document": id}}));
    assert_eq!(paint["status"], "refused", "{paint}");
    assert_eq!(
        std::fs::read(&outside).unwrap(),
        before,
        "nothing was written through the link"
    );
    live.finish();
}

/// The same id of a document inside the roots still works, read and write.
#[test]
fn a_document_id_inside_the_project_reads_and_saves() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let id = sheet_id(&fx.sheet);
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--save"],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            json!({"id": "color_net", "args": {"at": [5, 0], "color": "#336699", "document": id}}),
            json!({"id": "read_document", "args": {"document": id}}),
            json!({"id": "save_document", "args": {"document": id}}),
        ]),
    );
    assert_eq!(
        out.lines
            .iter()
            .map(|l| l["status"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["answered", "ran", "answered", "ran"],
        "{}\n{}",
        out.stdout,
        out.stderr
    );
}

/// A project that names a sheet outside its folder: a headless run does not
/// read it for a query, and no id of it can be used to reach it.
#[test]
fn a_project_sheet_reference_outside_the_folder_is_not_read_by_a_query() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    let outside = other.join("outside.snxsch");
    // Its own id, as a copy of a real sheet carries one.
    let mut engine = signex_engine::Engine::open(&fx.sheet).expect("sheet");
    engine.reassign_document_id(uuid::Uuid::now_v7());
    std::fs::write(&outside, engine.serialize().expect("encode")).unwrap();
    let outside_id = sheet_id(&outside);
    let mut data = signex_types::project::parse_project(&fx.project).expect("project");
    data.sheets.push(signex_types::project::SheetEntry {
        name: "Outside".into(),
        filename: outside.display().to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    });
    signex_types::project::write_project(&fx.project, &data).expect("project");

    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--keep-going"],
        jsonl(&[
            json!({"id": "list_documents"}),
            json!({"id": "find_items", "args": {"designator": "R1"}}),
            json!({"id": "read_document", "args": {"document": outside_id}}),
            json!({"id": "read_document", "args": {"document": outside}}),
        ]),
    );

    let listed = out.lines[0]["data"]["documents"].as_array().unwrap();
    let entry = listed
        .iter()
        .find(|d| d["path"] == outside.display().to_string())
        .expect("the reference is listed");
    assert!(entry["id"].is_null(), "its head was not read: {entry}");
    let found = out.lines[1]["data"]["elements"].as_array().unwrap();
    assert!(
        found
            .iter()
            .all(|e| e["document"]["path"] != outside.display().to_string()),
        "the outside sheet was not searched: {}",
        out.stdout
    );
    for index in [2, 3] {
        assert_eq!(out.lines[index]["status"], "refused", "{}", out.stdout);
    }
    assert!(
        out.lines[1]["messages"]
            .to_string()
            .contains("outside the project folder"),
        "the refusal is in Messages: {}",
        out.stdout
    );
}

/// A save names its document and writes that document only: another file in
/// the project that happens to be a link out is not a write target of it.
#[cfg(unix)]
#[test]
fn a_save_by_id_checks_the_named_document_not_the_active_or_unrelated_files() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    let id = sheet_id(&fx.sheet);
    let mut live = Live::start(&[
        "run",
        fx.project.to_str().unwrap(),
        "--save",
        "--keep-going",
    ]);
    assert_eq!(live.send(&open(&fx.sheet))["status"], "answered");
    let outside = replace_with_link_to_outside(&fx.project, &other);
    let before = std::fs::read(&outside).unwrap();

    let saved = live.send(&json!({"id": "save_document", "args": {"document": id}}));

    assert_eq!(saved["status"], "ran", "{saved}");
    assert_eq!(
        std::fs::read(&outside).unwrap(),
        before,
        "the project was not written"
    );
    live.finish();
}

/// A Save As by document also re-points the project file that lists the
/// sheet: that file is checked with the named one, so a project file that is
/// a link out refuses the run before anything is written.
#[cfg(unix)]
#[test]
fn a_save_as_by_id_also_checks_the_project_file_it_re_points() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    let id = sheet_id(&fx.sheet);
    let mut live = Live::start(&[
        "run",
        fx.project.to_str().unwrap(),
        "--save",
        "--keep-going",
    ]);
    assert_eq!(live.send(&open(&fx.sheet))["status"], "answered");
    let outside = replace_with_link_to_outside(&fx.project, &other);
    let before = std::fs::read(&outside).unwrap();
    let copy = fx.sheet.with_file_name("copy.snxsch");

    let saved = live.send(&json!({
        "id": "save_document_as",
        "args": {"document": id, "path": copy}
    }));

    assert_eq!(saved["status"], "refused", "{saved}");
    assert!(
        saved["reason"]
            .as_str()
            .is_some_and(|r| r.contains("also writes the project file")
                && r.contains("demo.snxprj is a symbolic link")),
        "{saved}"
    );
    assert!(!copy.exists(), "nothing was written");
    assert_eq!(std::fs::read(&outside).unwrap(), before);
    live.finish();
}

/// Lists a project that also names `extra` (a file in its folder), and says
/// what a query made of it.
fn list_with_reference(
    root: &std::path::Path,
    extra: &str,
    prepare: impl Fn(&std::path::Path),
) -> support::Output {
    let fx = fixture(root);
    prepare(&fx.sheet.with_file_name(extra));
    let mut data = signex_types::project::parse_project(&fx.project).expect("project");
    data.sheets.push(signex_types::project::SheetEntry {
        name: "Extra".into(),
        filename: extra.into(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    });
    signex_types::project::write_project(&fx.project, &data).expect("project");
    run_cli(
        &["run", fx.project.to_str().unwrap(), "--keep-going"],
        jsonl(&[
            json!({"id": "list_documents"}),
            json!({"id": "find_items", "args": {"designator": "R1"}}),
        ]),
    )
}

/// A project-referenced sheet over the size cap is not read by a query.
#[test]
fn a_referenced_sheet_over_the_size_cap_is_not_read_by_a_query() {
    let (_guard, root) = temp_root();
    let out = list_with_reference(&root, "big.snxsch", |path| {
        let file = std::fs::File::create(path).unwrap();
        file.set_len(signex_app::headless::MAX_FILE_BYTES + 1)
            .unwrap();
    });
    for line in &out.lines {
        assert_eq!(line["status"], "answered", "{}", out.stdout);
    }
    assert!(
        out.lines[1]["messages"]
            .to_string()
            .contains("is larger than"),
        "the refusal is in Messages: {}",
        out.stdout
    );
}

/// A FIFO named like a sheet does not hang a query.
#[cfg(unix)]
#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "`mkfifo` makes the pipe; the standard library has no call for it"
)]
fn a_referenced_fifo_does_not_hang_a_query() {
    let (_guard, root) = temp_root();
    let out = list_with_reference(&root, "pipe.snxsch", |path| {
        let made = std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap();
        assert!(made.success());
    });
    for line in &out.lines {
        assert_eq!(line["status"], "answered", "{}", out.stdout);
    }
    assert!(
        out.lines[1]["messages"]
            .to_string()
            .contains("not a regular file"),
        "{}",
        out.stdout
    );
}

fn write_sheet_with_id(template: &std::path::Path, path: &std::path::Path, id: uuid::Uuid) {
    let mut engine = signex_engine::Engine::open(template).expect("sheet");
    engine.reassign_document_id(id);
    std::fs::write(path, engine.serialize().expect("encode")).unwrap();
}

/// The head of a sheet the app has not opened is read again when the file
/// changes on disk (a `git checkout` during a long run), not served stale.
#[test]
fn a_changed_head_is_read_again_by_the_next_listing() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let extra = fx.sheet.with_file_name("Extra.snxsch");
    let (first, second) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
    write_sheet_with_id(&fx.sheet, &extra, first);
    let mut data = signex_types::project::parse_project(&fx.project).expect("project");
    data.sheets.push(signex_types::project::SheetEntry {
        name: "Extra".into(),
        filename: "Extra.snxsch".into(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    });
    signex_types::project::write_project(&fx.project, &data).expect("project");
    let mut live = Live::start(&["run", fx.project.to_str().unwrap(), "--keep-going"]);
    let id_of = |listing: &Value| {
        listing["data"]["documents"]
            .as_array()
            .unwrap()
            .iter()
            // By file name: a listed path may carry a verbatim prefix on Windows.
            .find(|d| {
                d["path"]
                    .as_str()
                    .is_some_and(|p| p.ends_with("Extra.snxsch"))
            })
            .map(|d| d["id"].clone())
            .expect("listed")
    };

    let before = live.send(&json!({"id": "list_documents"}));
    assert_eq!(id_of(&before), json!(first.to_string()));
    // A file system with a coarse clock must see a different time.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    write_sheet_with_id(&fx.sheet, &extra, second);
    let after = live.send(&json!({"id": "list_documents"}));

    assert_eq!(id_of(&after), json!(second.to_string()), "{after}");
    live.finish();
}

/// A Save As may rewrite the project's other sheets that reference the one
/// saved, so a project sheet that is a link out refuses it, and is named.
#[cfg(unix)]
#[test]
fn a_save_as_by_id_names_a_project_sheet_that_is_a_link_out() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    let extra = fx.sheet.with_file_name("Extra.snxsch");
    std::fs::copy(&fx.sheet, &extra).unwrap();
    let outside = replace_with_link_to_outside(&extra, &other);
    let before = std::fs::read(&outside).unwrap();
    let mut data = signex_types::project::parse_project(&fx.project).expect("project");
    data.sheets.push(signex_types::project::SheetEntry {
        name: "Extra".into(),
        filename: "Extra.snxsch".into(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    });
    signex_types::project::write_project(&fx.project, &data).expect("project");
    let id = sheet_id(&fx.sheet);
    let mut live = Live::start(&[
        "run",
        fx.project.to_str().unwrap(),
        "--save",
        "--keep-going",
    ]);
    assert_eq!(live.send(&open(&fx.sheet))["status"], "answered");
    let copy = fx.sheet.with_file_name("copy.snxsch");

    let saved = live.send(&json!({
        "id": "save_document_as",
        "args": {"document": id, "path": copy}
    }));

    assert_eq!(saved["status"], "refused", "{saved}");
    assert!(
        saved["reason"]
            .as_str()
            .is_some_and(|r| r.contains("also writes the project sheet")),
        "{saved}"
    );
    assert!(!copy.exists());
    assert_eq!(std::fs::read(&outside).unwrap(), before);
    live.finish();
}
