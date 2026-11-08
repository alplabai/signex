//! Git on save, results that must not say `ran` when nothing happened, and the
//! project folder that may not be home (#1923).

mod support;

use serde_json::json;
use support::{enable_git, fixture, jsonl, run_cli, run_cli_env, snapshot, temp_root};

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

#[test]
fn a_gitdir_file_pointing_outside_is_refused_and_the_outside_repo_is_untouched() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    enable_git(&fx);
    let (_victim_guard, victim) = temp_root();
    std::fs::create_dir_all(victim.join(".git")).unwrap();
    std::fs::write(victim.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::write(
        fx.dir.join(".git"),
        format!("gitdir: {}\n", victim.join(".git").display()),
    )
    .unwrap();
    let before = snapshot(&victim);
    let project = fx.project.to_str().unwrap();

    for allow in [&["--save"][..], &["--save", "--allow", "vcs"][..]] {
        let mut args = vec!["run", project, "--keep-going"];
        args.extend_from_slice(allow);
        let out = run_cli(&args, open_edit_save());
        let reason = refusal(&out, 2);
        assert!(
            reason.contains("pass --allow vcs") || reason.contains("`gitdir:` file"),
            "{allow:?}: {reason}"
        );
        assert_eq!(
            snapshot(&victim),
            before,
            "{allow:?}: the outside repo changed"
        );
    }
    assert!(
        fx.dir.join(".git").is_file(),
        "the gitdir file was left alone"
    );
}

#[test]
fn a_save_on_a_git_project_needs_vcs_and_never_runs_git_init() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    enable_git(&fx);
    let project = fx.project.to_str().unwrap();

    let without = run_cli(
        &["run", project, "--keep-going", "--save"],
        open_edit_save(),
    );
    assert!(
        refusal(&without, 2).contains("pass --allow vcs"),
        "{}",
        without.stdout
    );
    assert!(!fx.dir.join(".git").exists(), "no .git was created");

    let allowed = run_cli(
        &["run", project, "--keep-going", "--save", "--allow", "vcs"],
        open_edit_save(),
    );
    assert!(
        refusal(&allowed, 2).contains("has no .git folder"),
        "{}",
        allowed.stdout
    );
    assert!(!fx.dir.join(".git").exists(), "no .git was created");
}

#[test]
fn with_vcs_and_a_real_git_folder_the_commit_happens_inside_the_project_only() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    enable_git(&fx);
    git2::Repository::init(&fx.dir).expect("init the project's repository");
    let project = fx.project.to_str().unwrap();

    let out = run_cli(
        &["run", project, "--save", "--allow", "vcs"],
        open_edit_save(),
    );

    assert_eq!(
        out.lines[2]["status"], "ran",
        "{}\n{}",
        out.stdout, out.stderr
    );
    let repo = git2::Repository::open(&fx.dir).expect("repository");
    assert!(
        repo.head().is_ok(),
        "the save committed inside the project's repository"
    );
    assert!(fx.dir.join(".git").is_dir());
    assert!(
        snapshot(&root).keys().all(|path| path.starts_with(&fx.dir)),
        "nothing was written outside the project"
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
    let reason = refusal(&out, 1);
    assert!(
        reason.contains("needs a window"),
        "{reason}\n{}",
        out.stdout
    );
    let reason = refusal(&out, 2);
    assert!(
        reason.contains("needs a window"),
        "{reason}\n{}",
        out.stdout
    );
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
