//! A run never touches git, results that must not say `ran` when nothing
//! happened, and the project folder that may not be home (#1923).

mod support;

use std::path::Path;

use serde_json::json;
use support::{Fixture, enable_git, fixture, jsonl, run_cli, run_cli_env, snapshot, temp_root};

fn refusal(out: &support::Output, index: usize) -> &str {
    assert_eq!(out.lines[index]["status"], "refused", "{}", out.stdout);
    out.lines[index]["reason"].as_str().unwrap()
}

fn open_edit_save() -> String {
    jsonl(&[
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "place_junction", "args": {"at": [5.08, 0]}}),
        json!({"id": "save_document"}),
    ])
}

/// A git-enabled project whose `.git` is made by `dot_git`, with an outside
/// repository folder `victim` the `.git` may point at. Runs `--save` and
/// checks: the sheet is saved inside the project, the outside repository is
/// byte-equal, nothing was written outside the project, and the Messages log
/// says the commit was skipped. `after` checks `.git` itself.
fn save_with_dot_git(dot_git: impl FnOnce(&Fixture, &Path), after: impl FnOnce(&Fixture)) {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    enable_git(&fx);
    let (_victim_guard, victim) = temp_root();
    let victim_git = victim.join(".git");
    std::fs::create_dir_all(&victim_git).unwrap();
    std::fs::write(victim_git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    dot_git(&fx, &victim_git);
    let before = snapshot(&victim);

    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--save"],
        open_edit_save(),
    );

    assert_eq!(
        out.lines[2]["status"], "ran",
        "{}\n{}",
        out.stdout, out.stderr
    );
    let saved = signex_types::format::SnxSchematic::parse(
        &std::fs::read_to_string(&fx.sheet).expect("the saved sheet"),
    )
    .expect("reads back")
    .sheet;
    assert_eq!(saved.junctions.len(), 1, "the document was saved");
    assert_eq!(snapshot(&victim), before, "the outside repository changed");
    assert!(
        snapshot(&root)
            .keys()
            .all(|path| path.starts_with(&fx.dir) && !path.starts_with(fx.dir.join(".git/objects"))),
        "something was written outside the project, or git objects were created"
    );
    let skipped = out.lines[2]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| {
            m["message"]
                .as_str()
                .is_some_and(|text| text.contains("commit skipped in a headless run"))
        });
    assert!(skipped, "the Messages line is missing: {}", out.stdout);
    after(&fx);
}

#[test]
fn a_gitdir_file_pointing_outside_is_never_followed() {
    save_with_dot_git(
        |fx, victim_git| {
            std::fs::write(
                fx.dir.join(".git"),
                format!("gitdir: {}\n", victim_git.display()),
            )
            .unwrap();
        },
        |fx| {
            assert!(
                fx.dir.join(".git").is_file(),
                "the gitdir file is as it was"
            )
        },
    );
}

#[cfg(unix)]
#[test]
fn a_git_symlink_pointing_outside_is_never_followed() {
    save_with_dot_git(
        |fx, victim_git| std::os::unix::fs::symlink(victim_git, fx.dir.join(".git")).unwrap(),
        |fx| {
            let meta = std::fs::symlink_metadata(fx.dir.join(".git")).unwrap();
            assert!(meta.file_type().is_symlink(), "the link is as it was");
        },
    );
}

#[test]
fn a_git_project_with_no_git_folder_gets_none_created() {
    save_with_dot_git(
        |_fx, _victim| {},
        |fx| assert!(!fx.dir.join(".git").exists(), "no repository was created"),
    );
}

#[test]
fn a_command_whose_only_effect_was_a_dropped_window_is_refused() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let commands = jsonl(&[
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "report_manager_bom"}),
        json!({"id": "annotate_schematic"}),
    ]);
    let out = run_cli(
        &[
            "run",
            fx.project.to_str().unwrap(),
            "--keep-going",
            "--allow",
            "export",
        ],
        commands,
    );
    for index in [1, 2] {
        let reason = refusal(&out, index);
        assert!(
            reason.contains("needs a window"),
            "{reason}\n{}",
            out.stdout
        );
    }
}

#[test]
fn closing_a_document_with_unsaved_changes_parks_it_and_the_run_still_reports_it_unsaved() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let dirty = jsonl(&[
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "color_net", "args": {"at": [5, 0], "color": "#336699"}}),
        json!({"id": "close_active_document"}),
    ]);
    let before = snapshot(&root);
    let out = run_cli(&["run", project, "--keep-going"], dirty);
    // The app closes the tab and parks the edited sheet (nothing is lost); the
    // result is `ran`, and the end of the run says the sheet is still unsaved.
    assert_eq!(out.lines[2]["status"], "ran", "{}", out.stdout);
    assert!(
        out.stderr.contains("changes NOT written") && out.stderr.contains("main.snxsch"),
        "{}",
        out.stderr
    );
    assert_eq!(snapshot(&root), before, "nothing was written");

    let clean = jsonl(&[
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "close_active_document"}),
    ]);
    let out = run_cli(&["run", project, "--keep-going"], clean);
    assert_eq!(
        out.lines[1]["status"], "ran",
        "{}\n{}",
        out.stdout, out.stderr
    );
}

/// `dirs::home_dir` reads `HOME` on unix.
#[cfg(unix)]
#[test]
fn a_project_folder_that_is_home_or_above_it_is_refused() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let (_other_guard, elsewhere) = temp_root();
    let project = fx.project.to_str().unwrap();
    let command = jsonl(&[json!({"id": "select_all"})]);

    // Home is the project's folder, or inside it: the folder is too broad.
    let inside = fx.dir.join("sub");
    std::fs::create_dir_all(&inside).unwrap();
    for home in [fx.dir.as_path(), inside.as_path()] {
        let out = run_cli_env(&["run", project], &command, &[("HOME", home)]);
        assert_eq!(out.code, Some(2), "{}", out.stderr);
        assert!(out.stderr.contains("too broad"), "{}", out.stderr);
        assert!(out.lines.is_empty(), "no command ran");
    }
    // Home above the project, or elsewhere, leaves the project folder alone.
    for home in [root.as_path(), elsewhere.as_path()] {
        let ok = run_cli_env(&["run", project], &command, &[("HOME", home)]);
        assert_eq!(ok.lines.len(), 1, "{}", ok.stderr);
    }
}
