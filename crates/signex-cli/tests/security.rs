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
        json!({"id": "save_document_as", "args": {"path": root.join("proj").join("copy.snxsch")}}),
    );
    let out = run_cli(&["run", project], &save_as);
    assert_eq!(
        refusal(&out, 0),
        "`save_document_as` needs the `save` capability; pass --save"
    );

    let bom = run_cli(&["run", project], &one(json!({"id": "report_manager_bom"})));
    assert_eq!(
        refusal(&bom, 0),
        "`report_manager_bom` needs the `export` capability; pass --allow export"
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
        &one(json!({"id": "open_document", "args": {"path": "../../etc/passwd"}})),
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
        allowed.lines[0]["status"], "ran",
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
        &one(json!({"id": "open_document", "args": {"path": "link/secret.snxsch"}})),
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
        &one(json!({"id": "toggle_properties_panel"})),
    );
    assert_eq!(
        refusal(&out, 0),
        format!(
            "`toggle_properties_panel` is {}",
            signex_cli::catalog::GUI_ONLY_REASON
        )
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
        out.lines[5]["status"], "ran",
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
        out.lines[1]["status"], "ran",
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
        &one(json!({"id": "open_document", "args": {"path": "main.snxsch"}})),
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
    let bom = one(json!({"id": "report_manager_bom"}));

    let granted = run_cli(&["run", project, "--allow", "export"], &bom);
    let reason = granted.lines[0]["reason"].as_str().unwrap_or_default();
    assert!(!reason.contains("capability"), "{}", granted.stdout);
    let wrong = run_cli(&["run", project, "--allow", "delete,library-write"], &bom);
    assert!(refusal(&wrong, 0).contains("`export` capability"));

    let save_as = jsonl(&[
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "save_document_as", "args": {"path": "copy.snxsch"}}),
        json!({"id": "save_document_as", "args": {"path": "copy.snxsch"}}),
        json!({"id": "save_document_as", "args": {"path": "copy.snxsch", "overwrite": true}}),
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
        refusal(&out, 2).contains("refused in a headless run")
            && refusal(&out, 2).contains("exists; give"),
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
fn a_project_that_lists_broad_libraries_does_not_widen_the_confinement() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, other) = temp_root();
    std::fs::copy(&fx.sheet, root.join("sibling.snxsch")).unwrap();
    std::fs::copy(&fx.sheet, other.join("outside.snxsch")).unwrap();
    let mut listed = vec![
        fx.dir.join(".."),
        std::path::PathBuf::from(std::path::MAIN_SEPARATOR_STR),
    ];
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&other, fx.dir.join("lib-link")).unwrap();
        listed.push(fx.dir.join("lib-link"));
    }
    let refs: Vec<&std::path::Path> = listed.iter().map(|p| p.as_path()).collect();
    list_libraries(&fx, &refs);
    let project = fx.project.to_str().unwrap();

    let mut attempts = vec![root.join("sibling.snxsch")];
    #[cfg(unix)]
    {
        attempts.push(fx.dir.join("lib-link").join("outside.snxsch"));
        attempts.push(other.join("outside.snxsch"));
    }
    for target in attempts {
        let out = run_cli(&["run", project], &jsonl(&[open(&target)]));
        assert!(
            refusal(&out, 0).contains("outside the project")
                || refusal(&out, 0).contains("symbolic"),
            "{}: {}",
            target.display(),
            out.stdout
        );
    }
}

#[test]
fn editing_the_project_library_list_during_the_run_does_not_widen_it() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    std::fs::copy(&fx.sheet, root.join("sibling.snxsch")).unwrap();
    let sibling = root.join("sibling.snxsch");
    let mut live = Live::start(&["run", fx.project.to_str().unwrap(), "--keep-going"]);

    let before = live.send(&open(&sibling));
    assert_eq!(before["status"], "refused", "{before}");
    list_libraries(&fx, &[fx.dir.join("..").as_path()]);
    let after = live.send(&open(&sibling));
    assert_eq!(after["status"], "refused", "{after}");
    assert!(
        after["reason"]
            .as_str()
            .unwrap()
            .contains("outside the project")
    );
    assert_eq!(live.send(&open(&fx.sheet))["status"], "ran");
    live.finish();
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
    assert_eq!(live.send(&open(&fx.sheet))["status"], "ran");
    let paint = json!({"id": "color_net", "args": {"at": [5, 0], "color": "#336699"}});
    assert_eq!(live.send(&paint)["status"], "ran");

    let outside = replace_with_link_to_outside(&fx.sheet, &other);
    let before = std::fs::read(&outside).unwrap();
    let saved = live.send(&json!({"id": "save_document"}));

    assert_eq!(saved["status"], "refused", "{saved}");
    let reason = saved["reason"].as_str().unwrap();
    assert!(
        reason.contains("it would write") && reason.contains("symbolic link"),
        "{reason}"
    );
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
    assert_eq!(live.send(&open(&fx.sheet))["status"], "ran");

    let outside = replace_with_link_to_outside(&fx.project, &other);
    let before = std::fs::read(&outside).unwrap();
    let saved = live.send(&json!({"id": "save_document"}));

    assert_eq!(saved["status"], "refused", "{saved}");
    assert!(
        saved["reason"].as_str().unwrap().contains("symbolic link"),
        "{saved}"
    );
    assert_eq!(std::fs::read(&outside).unwrap(), before);
    live.finish();
}
