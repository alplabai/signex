//! End to end (#2253): a headless run journals what it did only when it may
//! save; without `--save` the project folder is left exactly as it was found.
//! `signex-cli agent-log` prints the journal and writes nothing. The attached
//! side (the running app writes, the CLI never does) is pinned in `attach.rs`
//! and, for the app's writing, in `signex-app`.

mod support;

use serde_json::{Value, json};
use support::{Live, fixture, run_cli, snapshot, temp_root};

/// Open the sheet, read its revision and apply one edit list against it, in
/// one headless run with `extra` options; the result of the apply, and the
/// revision it was applied at.
fn edit_once(project: &str, extra: &[&str]) -> (Value, u64) {
    let mut args = vec!["run", project, "--headless", "--keep-going"];
    args.extend_from_slice(extra);
    let mut live = Live::start(&args);
    let open = live.send(&json!({"id": "open_document", "args": {"path": "main.snxsch"}}));
    assert_eq!(open["status"], "answered", "{open}");
    let read = live.send(&json!({"id": "read_document",
        "args": {"document": "main.snxsch", "kinds": "junction"}}));
    let revision = read["data"]["revision"].as_u64().expect("a revision");
    let applied = live.send(&json!({"id": "apply_edits", "args": {
        "document": "main.snxsch", "expect_revision": revision,
        "edits": [{"id": "place_junction", "args": {"at": [5.08, 0]}}],
        "summary": "journal me"}}));
    live.finish();
    (applied, revision)
}

#[test]
fn a_headless_run_without_save_leaves_the_project_folder_as_it_found_it() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let before = snapshot(&root);

    let (applied, _) = edit_once(fx.project.to_str().unwrap(), &[]);

    assert_eq!(applied["status"], "answered", "{applied}");
    assert_eq!(snapshot(&root), before, "a file was written");
    assert!(
        !fx.dir.join(".signex").exists(),
        "no .signex without --save"
    );
}

#[test]
fn a_headless_run_with_save_writes_both_journals_and_agent_log_prints_them() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();

    let (applied, revision) = edit_once(project, &["--save"]);

    assert_eq!(applied["status"], "answered", "{applied}");
    let edits = std::fs::read_to_string(fx.dir.join(".signex/agent-edits.jsonl")).unwrap();
    let lines: Vec<Value> = edits
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    // The open and the read are not journaled: only a document change is.
    assert_eq!(lines.len(), 1, "{edits}");
    let entry = &lines[0];
    assert_eq!(entry["v"], 1);
    assert_eq!(entry["command"], "apply_edits");
    assert_eq!(entry["client"], "signex-cli");
    assert_eq!(entry["revision_before"], revision);
    assert_eq!(entry["outcome"], "ran");
    assert_eq!(entry["replayable"], true);
    assert_eq!(entry["operands"]["summary"], "journal me");
    assert!(
        entry["operands"]["edits"]
            .as_str()
            .unwrap()
            .contains("place_junction")
    );

    let access = std::fs::read_to_string(fx.dir.join(".signex/agent-access.jsonl")).unwrap();
    let events: Vec<String> = access
        .lines()
        .map(|line| {
            let v: Value = serde_json::from_str(line).unwrap();
            v["event"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(events, ["started", "ended"], "{access}");
    assert!(access.contains(r#""mode":"headless""#));
    assert!(
        access.contains(r#""capabilities":["read","edit","save"]"#),
        "{access}"
    );

    // The viewer prints both, and changes nothing.
    let journals = snapshot(&fx.dir.join(".signex"));
    let table = run_cli(&["agent-log", project], "");
    assert_eq!(table.code, Some(0), "{}", table.stderr);
    assert!(table.stdout.starts_with("TIME"), "{}", table.stdout);
    assert!(table.stdout.contains("apply_edits"), "{}", table.stdout);
    assert!(table.stdout.contains("main.snxsch"), "{}", table.stdout);
    let who = run_cli(&["agent-log", project, "--access"], "");
    assert!(who.stdout.contains("started") && who.stdout.contains("ended"));
    assert_eq!(
        snapshot(&fx.dir.join(".signex")),
        journals,
        "agent-log wrote"
    );
}

#[test]
fn agent_log_names_a_partial_last_line_and_a_newer_version_and_prints_the_rest() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    std::fs::create_dir(fx.dir.join(".signex")).unwrap();
    let good = r#"{"v":1,"ts":"2026-11-10T12:00:00.000Z","pid":7,"client":"a","document":null,"path":"/p/x.snxsch","command":"color_net","operands":{},"revision_before":1,"revision_after":2,"group":3,"outcome":"ran","reason":null,"truncated":false,"replayable":true}"#;
    let newer = r#"{"v":2,"ts":"2027-01-01T00:00:00.000Z"}"#;
    let cut = &good[..good.len() / 2];
    std::fs::write(
        fx.dir.join(".signex/agent-edits.jsonl"),
        format!("{good}\n{newer}\n{cut}"),
    )
    .unwrap();
    let before = snapshot(&fx.dir.join(".signex"));

    let out = run_cli(&["agent-log", fx.project.to_str().unwrap()], "");

    assert_eq!(out.code, Some(0));
    assert!(out.stdout.contains("color_net"), "{}", out.stdout);
    assert!(out.stderr.contains("partial line"), "{}", out.stderr);
    assert!(out.stderr.contains("newer Signex"), "{}", out.stderr);
    assert_eq!(
        snapshot(&fx.dir.join(".signex")),
        before,
        "agent-log rewrote it"
    );
}

#[test]
fn a_journal_that_cannot_be_written_never_stops_the_command() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    // `.signex` is a file: no journal can be made.
    std::fs::write(fx.dir.join(".signex"), "in the way").unwrap();

    let (applied, _) = edit_once(fx.project.to_str().unwrap(), &["--save"]);

    assert_eq!(applied["status"], "answered", "{applied}");
    let said = applied["messages"].to_string();
    assert!(said.contains("Agent journal:"), "{said}");
    assert_eq!(
        std::fs::read_to_string(fx.dir.join(".signex")).unwrap(),
        "in the way"
    );
}
