//! #2470: the board writers on the agent channel. `export_board_report`,
//! `export_board_pdf` and `fabrication_outputs` write only inside the named
//! board's project folder, never over a Signex document or an existing file
//! without `overwrite=true`, and are answered once the file is written, so a
//! write that is refused (a link out of the project) is refused on the
//! socket too.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{app_in, call, call_settled, fixture};
use crate::app::Signex;

/// The corpus regulator board: it has an outline, so its PDF and its
/// fabrication files can be made.
fn regulator() -> signex_types::pcb::PcbBoard {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb/real/linear-regulator.snxpcb");
    let text = std::fs::read_to_string(file).expect("the corpus board");
    signex_types::format::SnxPcb::parse(&text)
        .expect("it parses")
        .board
}

/// The fixture's project in `dir` with the regulator board saved as
/// `P.snxpcb` and open behind `b`; the board's path.
fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_in(dir);
    let board = regulator();
    let path = dir.join("P.snxpcb");
    let text = signex_types::format::SnxPcb::new(board.clone())
        .write_string()
        .expect("encode");
    std::fs::write(&path, text).expect("write the board");
    app.open_pcb_tab(path.clone(), "P.snxpcb".to_string(), board);
    app.document_state.projects[0].data.pcb_file = Some("P.snxpcb".to_string());
    let project = app.document_state.projects[0].id;
    for tab in &mut app.document_state.tabs {
        tab.project_id = Some(project);
    }
    fixture::activate(&mut app, &dir.join("b.snxsch"));
    (app, path)
}

fn reason(reply: &Value) -> &str {
    reply["reason"].as_str().unwrap_or_default()
}

fn text(path: &Path) -> String {
    path.display().to_string()
}

fn report(app: &mut Signex, board: &Path, target: &Path, extra: Value) -> Value {
    let mut args = json!({"document": text(board), "kind": "bom", "path": text(target)});
    if let (Some(args), Some(extra)) = (args.as_object_mut(), extra.as_object()) {
        args.extend(extra.clone());
    }
    call_settled(app, "export_board_report", args)
}

#[test]
fn a_report_is_answered_once_written_and_replaced_only_with_overwrite() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (mut app, board) = app_with_board(dir.path());
    let csv = dir.path().join("bom.csv");
    let written = report(&mut app, &board, &csv, json!({}));
    assert_eq!(written["status"], "answered", "{written}");
    assert!(csv.is_file(), "the CSV is written in the project");

    let again = report(&mut app, &board, &csv, json!({}));
    assert_eq!(again["status"], "refused", "{again}");
    assert!(reason(&again).contains("overwrite=true"), "{again}");
    let replaced = report(&mut app, &board, &csv, json!({"overwrite": true}));
    assert_eq!(replaced["status"], "answered", "{replaced}");
}

/// The data-loss case: a writer aimed at the project's own board is refused
/// whatever `overwrite` says, and the board's bytes do not change.
#[test]
fn a_signex_document_is_never_a_target_and_its_bytes_stay() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (mut app, board) = app_with_board(dir.path());
    let before = std::fs::read(&board).expect("the board");
    for overwrite in [false, true] {
        let reply = report(&mut app, &board, &board, json!({"overwrite": overwrite}));
        assert_eq!(reply["status"], "refused", "{reply}");
        assert!(reason(&reply).contains("Signex document"), "{reply}");
        let pdf = call_settled(
            &mut app,
            "export_board_pdf",
            json!({"document": text(&board), "path": text(&board), "overwrite": overwrite}),
        );
        assert_eq!(pdf["status"], "refused", "{pdf}");
    }
    assert_eq!(std::fs::read(&board).expect("the board"), before);
}

#[test]
fn a_git_folder_and_the_wrong_extension_are_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (mut app, board) = app_with_board(dir.path());
    let git = dir.path().join(".git").join("bom.csv");
    let reply = report(&mut app, &board, &git, json!({}));
    assert_eq!(reply["status"], "refused", "{reply}");
    assert!(reason(&reply).contains(".git"), "{reply}");
    let fab = call_settled(
        &mut app,
        "fabrication_outputs",
        json!({"document": text(&board), "path": text(&dir.path().join(".GIT"))}),
    );
    assert_eq!(fab["status"], "refused", "{fab}");

    let pdf = call_settled(
        &mut app,
        "export_board_pdf",
        json!({"document": text(&board), "path": text(&dir.path().join("board.csv"))}),
    );
    assert_eq!(pdf["status"], "refused", "{pdf}");
    assert!(reason(&pdf).contains(".pdf"), "{pdf}");
}

#[test]
fn the_pdf_and_the_fabrication_files_are_written_inside_the_project() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (mut app, board) = app_with_board(dir.path());
    let pdf = dir.path().join("board.pdf");
    let reply = call_settled(
        &mut app,
        "export_board_pdf",
        json!({"document": text(&board), "path": text(&pdf)}),
    );
    assert_eq!(reply["status"], "answered", "{reply}");
    assert!(std::fs::read(&pdf).expect("written").starts_with(b"%PDF"));

    let fab = dir.path().join("fab");
    let reply = call_settled(
        &mut app,
        "fabrication_outputs",
        json!({"document": text(&board), "path": text(&fab), "kinds": "gerber,drill"}),
    );
    assert_eq!(reply["status"], "answered", "{reply}");
    let files = reply["data"]["files"].as_array().expect("files");
    assert!(!files.is_empty(), "{reply}");
    for file in files {
        let path = PathBuf::from(file["path"].as_str().expect("a path"));
        assert!(path.starts_with(&fab) && path.is_file(), "{file}");
    }
}

/// The gate refuses a relative path (the operand is not a path) and one
/// with `..`; a path that only shares a prefix with the project folder is
/// refused by the write, which resolves both.
#[test]
fn a_relative_path_a_dot_dot_and_a_sibling_folder_are_refused() {
    let base = tempfile::tempdir().expect("temp dir");
    let project = base.path().join("p");
    let sibling = base.path().join("pp");
    std::fs::create_dir_all(&project).expect("project folder");
    std::fs::create_dir_all(&sibling).expect("sibling folder");
    let (mut app, board) = app_with_board(&project);

    let relative = call(
        &mut app,
        "export_board_report",
        json!({"document": text(&board), "kind": "bom", "path": "bom.csv"}),
    );
    assert_eq!(relative["status"], "refused", "{relative}");
    assert!(reason(&relative).contains("absolute"), "{relative}");

    let up = project.join("..").join("pp").join("bom.csv");
    let dotdot = report(&mut app, &board, &up, json!({}));
    assert_eq!(dotdot["status"], "refused", "{dotdot}");
    assert!(reason(&dotdot).contains("without `..`"), "{dotdot}");

    for (id, target) in [
        ("export_board_report", sibling.join("bom.csv")),
        ("export_board_pdf", sibling.join("board.pdf")),
        ("fabrication_outputs", sibling.join("fab")),
    ] {
        let mut args = json!({"document": text(&board), "path": text(&target)});
        if id == "export_board_report" {
            args["kind"] = json!("bom");
        }
        let reply = call_settled(&mut app, id, args);
        assert_eq!(reply["status"], "refused", "{id}: {reply}");
    }
    assert_eq!(std::fs::read_dir(&sibling).expect("read").count(), 0);
}

/// A folder in the project that links out of it: the write is refused, the
/// socket says so, and nothing lands where the link points.
#[cfg(unix)]
#[test]
fn a_link_out_of_the_project_is_refused_on_the_socket_and_not_written() {
    let dir = tempfile::tempdir().expect("temp dir");
    let outside = tempfile::tempdir().expect("temp dir");
    let (mut app, board) = app_with_board(dir.path());
    std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).expect("link");

    let csv = report(
        &mut app,
        &board,
        &dir.path().join("out").join("bom.csv"),
        json!({}),
    );
    assert_eq!(csv["status"], "refused", "{csv}");
    assert!(reason(&csv).contains("behind a link"), "{csv}");

    let pdf = call_settled(
        &mut app,
        "export_board_pdf",
        json!({"document": text(&board), "path": text(&dir.path().join("out").join("b.pdf"))}),
    );
    assert_eq!(pdf["status"], "refused", "{pdf}");

    let fab = call_settled(
        &mut app,
        "fabrication_outputs",
        json!({"document": text(&board), "path": text(&dir.path().join("out").join("fab")), "kinds": "gerber"}),
    );
    assert_eq!(fab["status"], "refused", "{fab}");
    assert_eq!(std::fs::read_dir(outside.path()).expect("read").count(), 0);
}

/// The project folder reached through a link (macOS's `/var` is
/// `/private/var`, a linked home): a path spelled through the link is not
/// refused for its spelling, as the write resolves both.
#[cfg(unix)]
#[test]
fn a_project_reached_through_a_link_is_written_through_either_spelling() {
    let base = tempfile::tempdir().expect("temp dir");
    let real = base.path().join("real");
    std::fs::create_dir_all(&real).expect("folder");
    let linked = base.path().join("linked");
    std::os::unix::fs::symlink(&real, &linked).expect("link");
    let (mut app, board) = app_with_board(&linked);
    for (folder, name) in [(&linked, "a.csv"), (&real, "b.csv")] {
        let reply = report(&mut app, &board, &folder.join(name), json!({}));
        assert_eq!(reply["status"], "answered", "{name}: {reply}");
        assert!(real.join(name).is_file(), "{name}");
    }
}
