//! #1299: an Export Job run through `Signex::update` (the palette row and its
//! picker) and through the headless path a CLI and an agent take: the same
//! files, nothing written on a refusal, and two runs give the same bytes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;
use signex_types::output_job::{JobOutput, OutputJob, OutputKind};

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
use crate::app::output_job::file::write_output_job;
use crate::app::{Message, Signex};
use crate::io_ports::{HeadlessAnswers, IoPorts};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

/// A project `ProjectA` in `dir` with its one sheet open: R1 on the global
/// net NET_A.
pub(super) fn project_app(dir: &Path) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &["main.snxsch"]);
    app.ui_state.first_run_tour_open = false;
    let sheet = dir.join("main.snxsch");
    open_with(
        &mut app.document_state,
        &sheet,
        sheet_with_net("R1", "NET_A", &[]),
    );
    app.document_state.active_path = Some(sheet);
    app
}

/// A PDF, PNG pages at 40 dpi, a BOM and a netlist into `out`.
fn job() -> OutputJob {
    let mut pages = JobOutput::new("Pages", OutputKind::SchematicPng);
    pages.dpi = Some(40);
    OutputJob {
        outputs: vec![
            JobOutput::new("PDF", OutputKind::SchematicPdf),
            pages,
            JobOutput::new("BOM", OutputKind::Bom),
            JobOutput::new("Netlist", OutputKind::Netlist),
        ],
        ..OutputJob::default()
    }
}

fn write_job(path: &Path, job: &OutputJob) -> PathBuf {
    write_output_job(path, job).expect("the job file");
    path.to_path_buf()
}

pub(super) fn headless(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    let (outcome, task) = app.run_headless_command_in_test(&command, args);
    settle(app, task);
    outcome
}

pub(super) fn refusal(outcome: &CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

/// Answer the next file or folder picker with `path`.
pub(super) fn answer_picker(app: &mut Signex, path: &Path) {
    let answers = HeadlessAnswers::default();
    answers.set("path", path.to_string_lossy().into_owned());
    app.ui_state.io = IoPorts::headless(answers);
}

pub(super) fn files_in(folder: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(folder)
        .expect("the folder was made")
        .map(|entry| {
            let entry = entry.expect("an entry");
            let name = entry.file_name().to_string_lossy().into_owned();
            (name, std::fs::read(entry.path()).expect("read"))
        })
        .collect()
}

fn assert_full_set(files: &BTreeMap<String, Vec<u8>>) {
    let names: Vec<&str> = files.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "ProjectA-1.png",
            "ProjectA-bom.csv",
            "ProjectA.NET",
            "ProjectA.pdf"
        ]
    );
    assert!(files["ProjectA.pdf"].starts_with(b"%PDF"));
    assert!(files["ProjectA-1.png"].starts_with(b"\x89PNG\r\n\x1a\n"));
    let bom = String::from_utf8_lossy(&files["ProjectA-bom.csv"]);
    assert!(bom.contains("R1"), "{bom}");
    let netlist = String::from_utf8_lossy(&files["ProjectA.NET"]);
    assert!(netlist.starts_with("[\r\nR1\r\n"), "{netlist:?}");
}

/// The palette row (no operands) opens the picker; the picked job writes
/// every enabled output into its folder.
#[test]
fn the_palette_row_runs_the_picked_job() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(&dir.path().join("fab.snxout"), &job());
    answer_picker(&mut app, &job);
    let task = app.update(Message::Menu(MenuMessage::command("run_output_job")));
    settle(&mut app, task);
    assert_full_set(&files_in(&dir.path().join("out")));
    assert!(app.document_view.error_notice.is_none());
}

/// The CLI and an agent name the job and get the files written as JSON, the
/// same files the palette row writes; a second run writes the same bytes.
#[test]
fn a_headless_run_answers_with_what_it_wrote_and_repeats_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(&dir.path().join("fab.snxout"), &job());
    let operand = format!("path={}", job.display());
    let CommandOutcome::Answered(answer) = headless(&mut app, "run_output_job", &[&operand]) else {
        panic!("not answered");
    };
    assert_eq!(answer["variant"], "Base");
    assert_eq!(answer["job"], job.display().to_string());
    assert_eq!(
        answer["folder"],
        dir.path().join("out").display().to_string()
    );
    let outputs: Vec<(String, usize)> = answer["outputs"]
        .as_array()
        .expect("outputs")
        .iter()
        .map(|o| {
            let files = o["files"].as_array().map_or(0, Vec::len);
            (o["name"].as_str().unwrap_or_default().to_string(), files)
        })
        .collect();
    assert_eq!(
        outputs,
        [
            ("PDF".to_string(), 1),
            ("Pages".to_string(), 1),
            ("BOM".to_string(), 1),
            ("Netlist".to_string(), 1)
        ]
    );
    let first = files_in(&dir.path().join("out"));
    assert_full_set(&first);

    let again = headless(&mut app, "run_output_job", &[&operand]);
    assert!(matches!(again, CommandOutcome::Answered(_)), "{again:?}");
    assert_eq!(files_in(&dir.path().join("out")), first, "the same bytes");
}

#[test]
fn a_run_of_one_output_writes_only_that_output() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(&dir.path().join("fab.snxout"), &job());
    let operand = format!("path={}", job.display());
    let outcome = headless(&mut app, "run_output_job", &[&operand, "name=Netlist"]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    let names: Vec<String> = files_in(&dir.path().join("out")).into_keys().collect();
    assert_eq!(names, ["ProjectA.NET"]);

    let unknown = headless(&mut app, "run_output_job", &[&operand, "name=Gerber"]);
    assert!(refusal(&unknown).contains("no output named"), "{unknown:?}");
}

/// The job names its variant; the answer spells it as the project does, and
/// a variant the project does not have writes nothing.
#[test]
fn the_job_names_its_variant() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    app.document_state.projects[0]
        .data
        .variant_definitions
        .push("Lite".into());
    let mut named = job();
    named.variant = Some("lite".into());
    let job_path = write_job(&dir.path().join("fab.snxout"), &named);
    let operand = format!("path={}", job_path.display());
    let CommandOutcome::Answered(answer) = headless(&mut app, "run_output_job", &[&operand]) else {
        panic!("not answered");
    };
    assert_eq!(answer["variant"], "Lite");

    let other = tempfile::tempdir().unwrap();
    let mut app = project_app(other.path());
    named.variant = Some("Pro".into());
    let job_path = write_job(&other.path().join("fab.snxout"), &named);
    let operand = format!("path={}", job_path.display());
    let refused = headless(&mut app, "run_output_job", &[&operand]);
    assert!(
        refusal(&refused).contains("no variant named"),
        "{refused:?}"
    );
    assert!(!other.path().join("out").exists(), "nothing written");
}

/// A job outside an open project, a file that is not a job, a folder that
/// climbs out of the project and a job with nothing enabled write nothing.
#[test]
fn a_run_that_cannot_run_writes_nothing_and_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());

    let elsewhere = tempfile::tempdir().unwrap();
    let outside = write_job(&elsewhere.path().join("fab.snxout"), &job());
    let refused = headless(
        &mut app,
        "run_output_job",
        &[&format!("path={}", outside.display())],
    );
    assert!(
        refusal(&refused).contains("not in the folder of an open project"),
        "{refused:?}"
    );

    let not_a_job = dir.path().join("main.snxsch");
    let refused = headless(
        &mut app,
        "run_output_job",
        &[&format!("path={}", not_a_job.display())],
    );
    assert!(
        refusal(&refused).contains("not an Export Job"),
        "{refused:?}"
    );

    let climbing = dir.path().join("climb.snxout");
    std::fs::write(
        &climbing,
        "format = \"snxout/1\"\nfolder = \"../escape\"\n\n[[outputs]]\nname = \"PDF\"\nkind = \"schematic_pdf\"\n",
    )
    .unwrap();
    let refused = headless(
        &mut app,
        "run_output_job",
        &[&format!("path={}", climbing.display())],
    );
    assert!(refusal(&refused).contains("../escape"), "{refused:?}");

    let empty = write_job(&dir.path().join("empty.snxout"), &OutputJob::default());
    let refused = headless(
        &mut app,
        "run_output_job",
        &[&format!("path={}", empty.display())],
    );
    assert!(
        refusal(&refused).contains("no enabled output"),
        "{refused:?}"
    );

    assert!(!dir.path().join("out").exists(), "nothing written");
    assert!(!dir.path().parent().unwrap().join("escape").exists());
}

/// A netlist with a hole in it is never written by a run, and neither is
/// anything else of that run.
#[test]
fn an_incomplete_netlist_stops_the_whole_run() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["main.snxsch"]);
    let sheet = dir.path().join("main.snxsch");
    open_with(
        &mut app.document_state,
        &sheet,
        sheet_with_net("R1", "NET_A", &["missing.snxsch"]),
    );
    app.document_state.active_path = Some(sheet);
    let job = write_job(&dir.path().join("fab.snxout"), &job());
    let refused = headless(
        &mut app,
        "run_output_job",
        &[&format!("path={}", job.display())],
    );
    assert!(refusal(&refused).contains("incomplete"), "{refused:?}");
    assert!(!dir.path().join("out").exists(), "nothing written");
}

/// A job the person picks outside any open project shows the notice card.
#[test]
fn a_picked_job_that_cannot_run_shows_the_notice() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let elsewhere = tempfile::tempdir().unwrap();
    let outside = write_job(&elsewhere.path().join("fab.snxout"), &job());
    answer_picker(&mut app, &outside);
    let task = app.update(Message::Menu(MenuMessage::command("run_output_job")));
    settle(&mut app, task);
    let notice = app.document_view.error_notice.as_ref().expect("a notice");
    let text = notice.detail.render_en_us();
    assert!(text.contains("wrote nothing"), "{text}");
    assert!(!elsewhere.path().join("out").exists());
}

/// The row runs behind the open-project gate: with no project it is refused
/// and nothing is picked.
#[test]
fn the_row_needs_an_open_project() {
    let command = AppCommandId::new("run_output_job").unwrap();
    let (mut app, _boot) = Signex::new();
    let (outcome, _) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let (outcome, _) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    assert_eq!(outcome, CommandOutcome::Ran);
}
