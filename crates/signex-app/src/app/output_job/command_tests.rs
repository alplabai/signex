//! Mockup row 114 on the headless path: every action of the Export Job tab
//! is a catalog command with a JSON answer. A job no tab holds is read,
//! edited and written back; a job open in a tab is edited as its draft and
//! left unsaved, as the tab's own edits are.

use std::path::Path;

use signex_types::output_job::{BeforeClose, OnWarnings, OutputKind, read_output_job};

use super::pipeline_tests::{profile_job, write_job};
use super::tests::{headless, project_app, refusal};
use crate::app::Signex;
use crate::app::command::CommandOutcome;

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn run(app: &mut Signex, id: &str, job: &Path, operands: &[&str]) -> CommandOutcome {
    let path = format!("path={}", job.display());
    let mut all = vec![path.as_str()];
    all.extend_from_slice(operands);
    headless(app, id, &all)
}

/// `new_output_job` writes an empty job file, and refuses over one.
#[test]
fn a_new_job_file_is_written_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = dir.path().join("fab.snxout");
    let answer = answered(run(&mut app, "new_output_job", &job, &[]));
    assert_eq!(answer["created"], true);
    assert!(read_output_job(&job).unwrap().outputs.is_empty());
    let again = run(&mut app, "new_output_job", &job, &[]);
    assert!(refusal(&again).contains("exists already"), "{again:?}");
    let outside = tempfile::tempdir().unwrap();
    let stray = run(
        &mut app,
        "new_output_job",
        &outside.path().join("x.snxout"),
        &[],
    );
    assert!(refusal(&stray).contains("open project"), "{stray:?}");
}

/// `add_job_output` and `set_job_output` change the file: a new output of a
/// kind, ticked in a profile; its settings; its removal from every profile.
#[test]
fn outputs_are_added_changed_and_removed_by_command() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &profile_job());
    let added = answered(run(
        &mut app,
        "add_job_output",
        &job,
        &["kind=odb", "profile=Release"],
    ));
    assert_eq!(added["result"]["output"], "ODB++");
    let content = read_output_job(&job).unwrap();
    assert_eq!(
        content.output("ODB++").unwrap().known_kind(),
        Some(OutputKind::Odb)
    );
    assert!(content.profiles[0].ticks("ODB++"));
    let bad = run(&mut app, "add_job_output", &job, &["kind=photo"]);
    assert!(refusal(&bad).contains("`kind` is one of"), "{bad:?}");
    let twice = run(&mut app, "add_job_output", &job, &["kind=bom", "name=BOM"]);
    assert!(refusal(&twice).contains("already"), "{twice:?}");

    answered(run(
        &mut app,
        "set_job_output",
        &job,
        &[
            "name=ODB++",
            "enabled=false",
            "file=board",
            "odb_folder=true",
        ],
    ));
    let odb = read_output_job(&job)
        .unwrap()
        .output("ODB++")
        .unwrap()
        .clone();
    assert!(!odb.enabled);
    assert_eq!(
        (odb.file.as_deref(), odb.odb_folder),
        (Some("board"), Some(true))
    );
    let dpi = run(&mut app, "set_job_output", &job, &["name=PDF", "dpi=5000"]);
    assert!(refusal(&dpi).contains("dpi"), "{dpi:?}");
    answered(run(
        &mut app,
        "set_job_output",
        &job,
        &["name=ODB++", "remove=true"],
    ));
    let content = read_output_job(&job).unwrap();
    assert!(content.output("ODB++").is_none());
    assert!(!content.profiles[0].ticks("ODB++"), "unticked everywhere");
}

/// `set_job_profile` makes a copy, renames it, sets what it runs and how,
/// makes it the default and removes it; `set_job_triggers` sets what runs
/// it without a click.
#[test]
fn profiles_and_their_triggers_are_set_by_command() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &profile_job());
    answered(run(
        &mut app,
        "set_job_profile",
        &job,
        &[
            "profile=Docs",
            "from=Release",
            "outputs=PDF",
            "on_warnings=stop",
            "folder=docs",
            "naming={project}-{rev}",
            "default=true",
            "zip=false",
        ],
    ));
    let content = read_output_job(&job).unwrap();
    let docs = content.profile("Docs").unwrap();
    assert_eq!(docs.outputs, ["PDF"]);
    assert_eq!(docs.on_warnings, OnWarnings::Stop);
    assert_eq!(docs.folder.as_deref(), Some("docs"));
    assert_eq!(docs.naming.as_deref(), Some("{project}-{rev}"));
    assert!(!docs.zip && docs.group_folders, "the copy keeps the rest");
    assert_eq!(content.default_profile.as_deref(), Some("Docs"));

    answered(run(
        &mut app,
        "set_job_profile",
        &job,
        &["profile=Docs", "to=Docs only"],
    ));
    let content = read_output_job(&job).unwrap();
    assert_eq!(content.default_profile.as_deref(), Some("Docs only"));
    let missing = run(
        &mut app,
        "set_job_profile",
        &job,
        &["profile=Docs only", "outputs=Gerber"],
    );
    assert!(refusal(&missing).contains("no output named"), "{missing:?}");
    let policy = run(
        &mut app,
        "set_job_profile",
        &job,
        &["profile=Release", "on_errors=maybe"],
    );
    assert!(
        refusal(&policy).contains("`on_errors` is one of"),
        "{policy:?}"
    );

    answered(run(
        &mut app,
        "set_job_triggers",
        &job,
        &[
            "profile=Docs only",
            "on_save=true",
            "wait=5",
            "sources=board.snxpcb, *.snxsch",
            "before_close=always",
            "tag=v*",
        ],
    ));
    let triggers = read_output_job(&job)
        .unwrap()
        .profile("Docs only")
        .unwrap()
        .triggers
        .clone();
    assert!(triggers.on_save);
    assert_eq!(triggers.wait_seconds, 5);
    assert_eq!(triggers.sources, ["board.snxpcb", "*.snxsch"]);
    assert_eq!(triggers.before_close, BeforeClose::Always);
    assert_eq!(triggers.on_tag.as_deref(), Some("v*"));
    let wait = run(
        &mut app,
        "set_job_triggers",
        &job,
        &["profile=Docs only", "wait=99999"],
    );
    assert!(refusal(&wait).contains("`wait`"), "{wait:?}");

    answered(run(
        &mut app,
        "set_job_profile",
        &job,
        &["profile=Docs only", "remove=true"],
    ));
    let content = read_output_job(&job).unwrap();
    assert!(content.profile("Docs only").is_none());
    assert_eq!(content.default_profile.as_deref(), Some("Release"));
}

/// A job open in a tab is edited as its draft: the answer says unsaved,
/// the file is untouched, and `read_output_job` reads the draft.
#[test]
fn a_job_open_in_a_tab_is_edited_as_its_draft() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &profile_job());
    let opened = answered(run(&mut app, "open_output_job", &job, &[]));
    assert_eq!(opened["opened"], true);
    let answer = answered(run(&mut app, "add_job_output", &job, &["kind=gerber"]));
    assert_eq!(answer["saved"], false);
    assert!(read_output_job(&job).unwrap().output("Gerber").is_none());
    let read = answered(run(&mut app, "read_output_job", &job, &[]));
    assert_eq!(read["unsaved"], true);
    assert!(
        read["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["name"] == "Gerber")
    );
}

/// `read_output_job` of a closed job reads its file; `view_output_job_run`
/// with no run to view is refused.
#[test]
fn a_closed_job_is_read_from_its_file_and_a_job_never_run_has_nothing_to_view() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &profile_job());
    let read = answered(run(&mut app, "read_output_job", &job, &[]));
    assert_eq!(read["default_profile"], "Release");
    assert_eq!(read["unsaved"], false);
    let view = run(&mut app, "view_output_job_run", &job, &[]);
    assert!(refusal(&view).contains("no run"), "{view:?}");
    let not_a_job = run(
        &mut app,
        "read_output_job",
        &dir.path().join("main.snxsch"),
        &[],
    );
    assert!(
        refusal(&not_a_job).contains("not an Export Job"),
        "{not_a_job:?}"
    );
}
