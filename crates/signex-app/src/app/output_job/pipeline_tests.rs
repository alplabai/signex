//! Mockup row 114: a profile's run goes Validate, Generate, Package and
//! Summary; its policy stops, asks or goes on; a stopped run writes
//! nothing; Package lays the files out in group folders, a release folder
//! and a zip; board outputs come from the open board; a run from the window
//! is kept in the history and a headless one is not. Mockup row 193b: the
//! summary's View in Gerber Viewer opens exactly the run's files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_types::output_job::{
    JobOutput, JobProfile, OnErrors, OutputJob, OutputKind, RunResult, RunTrigger, read_output_job,
    write_output_job,
};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::project::SetupProfile;

use super::pipeline::{NodeState, RunMsg, RunStage, StartRun, ValidateOutcome};
use super::run::RunAsk;
use super::tests::{headless, project_app, refusal};
use crate::app::command::CommandOutcome;
use crate::app::handlers::menu::export::settle;
use crate::app::{Message, Signex};

/// Every file under `folder`, by its path below it with `/`.
pub(super) fn tree(folder: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut dirs = vec![(folder.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("the folder") {
            let entry = entry.expect("an entry");
            let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
            if entry.path().is_dir() {
                dirs.push((entry.path(), format!("{name}/")));
            } else {
                out.insert(name, std::fs::read(entry.path()).expect("read"));
            }
        }
    }
    out
}

/// A PDF and a BOM, both ticked in profile `Release`, the job's default:
/// no Validate, files into `out/rel` by group, with a zip.
pub(super) fn profile_job() -> OutputJob {
    let mut release = JobProfile::new("Release", vec!["PDF".into(), "BOM".into()]);
    release.validate = false;
    release.folder = Some("out/rel".into());
    release.group_folders = true;
    release.zip = true;
    OutputJob {
        default_profile: Some("Release".into()),
        outputs: vec![
            JobOutput::new("PDF", OutputKind::SchematicPdf),
            JobOutput::new("BOM", OutputKind::Bom),
            JobOutput::new("Netlist", OutputKind::Netlist),
        ],
        profiles: vec![release],
        ..OutputJob::default()
    }
}

pub(super) fn write_job(dir: &Path, job: &OutputJob) -> PathBuf {
    let path = dir.join("fab.snxout");
    write_output_job(&path, job).expect("the job file");
    path
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

/// Start a run as the window does, without running its tasks: the test
/// feeds the steps' answers itself.
fn start(app: &mut Signex, job: &Path, content: OutputJob, trigger: RunTrigger) -> u64 {
    let _first_step = app.start_job_run(StartRun {
        job_path: job.to_path_buf(),
        job: content,
        ask: RunAsk {
            trigger: Some(trigger),
            ..RunAsk::default()
        },
        trigger,
        waiting: None,
    });
    app.output_jobs().runs.running(job).expect("a run").id
}

fn validated(app: &mut Signex, run: u64, errors: usize) -> iced::Task<Message> {
    let outcome = ValidateOutcome {
        errors,
        lines: vec!["ERC: a pin is not connected".into()],
        failing: vec!["erc"],
        ..ValidateOutcome::default()
    };
    app.update(Message::from(RunMsg::Validated {
        run,
        outcome: Arc::new(outcome),
    }))
}

/// The headless path: the default profile runs its ticked outputs into its
/// folder, one subfolder a group, with a zip of the same files beside it.
#[test]
fn a_profile_writes_its_ticked_outputs_by_group_with_a_zip() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &profile_job());
    let operand = format!("path={}", job.display());
    let answer = answered(headless(&mut app, "run_output_job", &[&operand]));
    assert_eq!(answer["profile"], "Release");
    let folder = dir.path().join("out").join("rel");
    assert_eq!(answer["folder"], folder.display().to_string());
    let files: Vec<String> = tree(&folder).into_keys().collect();
    assert_eq!(
        files,
        ["Documentation/ProjectA.pdf", "Reports/ProjectA-bom.csv"],
        "the netlist is not ticked"
    );
    assert_eq!(answer["zip"], "rel.zip");
    let zip = std::fs::File::open(dir.path().join("out").join("rel.zip")).expect("the zip");
    let archive = zip::ZipArchive::new(zip).expect("a zip");
    let mut names: Vec<String> = archive.file_names().map(str::to_string).collect();
    names.sort();
    assert_eq!(names, files);
    let saved = read_output_job(&job).unwrap();
    assert!(
        saved.history.is_empty(),
        "a headless run leaves the file as it is"
    );
}

/// Run Selected writes the ticked outputs without the zip; a profile the
/// job does not have writes nothing.
#[test]
fn run_selected_skips_the_zip_and_an_unknown_profile_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &profile_job());
    let operand = format!("path={}", job.display());
    let answer = answered(headless(
        &mut app,
        "run_output_job",
        &[&operand, "selected=true"],
    ));
    assert_eq!(answer.get("zip"), None);
    assert!(!dir.path().join("out").join("rel.zip").exists());
    let other = tempfile::tempdir().unwrap();
    let mut app = project_app(other.path());
    let job = write_job(other.path(), &profile_job());
    let operand = format!("path={}", job.display());
    let refused = headless(&mut app, "run_output_job", &[&operand, "profile=Docs"]);
    assert!(
        refusal(&refused).contains("no profile named"),
        "{refused:?}"
    );
    assert!(!other.path().join("out").exists());
}

/// A release folder is named with the date; a second run the same day
/// writes a folder of its own.
#[test]
fn a_second_release_the_same_day_gets_its_own_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let mut content = profile_job();
    content.profiles[0].release_folder = true;
    content.profiles[0].zip = false;
    let job = write_job(dir.path(), &content);
    let operand = format!("path={}", job.display());
    let first = answered(headless(&mut app, "run_output_job", &[&operand]));
    let second = answered(headless(&mut app, "run_output_job", &[&operand]));
    let first = first["folder"].as_str().unwrap().to_string();
    let second = second["folder"].as_str().unwrap().to_string();
    assert!(first.contains("ProjectA-"), "{first}");
    assert_eq!(second, format!("{first}-2"));
    assert!(
        Path::new(&second)
            .join("Documentation")
            .join("ProjectA.pdf")
            .is_file()
    );
}

/// Validate found errors and the profile stops on errors: nothing is
/// generated, the flow says where it stopped, and the run is kept in the
/// history as stopped.
#[test]
fn errors_stop_the_run_with_nothing_written_and_the_history_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let mut content = profile_job();
    content.profiles[0].validate = true;
    let job = write_job(dir.path(), &content);
    let run = start(&mut app, &job, content, RunTrigger::Manual);
    assert_eq!(
        app.output_jobs().runs.get(run).unwrap().stage,
        RunStage::Validating
    );
    let task = validated(&mut app, run, 1);
    settle(&mut app, task);
    let ended = app.output_jobs().runs.get(run).unwrap();
    assert_eq!(ended.stage, RunStage::Done);
    assert_eq!(ended.stopped.as_ref().unwrap().at, "validate");
    assert_eq!(ended.validate.as_ref().unwrap().state, NodeState::Failed);
    assert!(ended.nodes.iter().all(|n| n.state == NodeState::Pending));
    assert!(!dir.path().join("out").exists(), "nothing written");
    let history = read_output_job(&job).unwrap().history;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].result, RunResult::Stopped);
    assert_eq!(history[0].stopped_at.as_deref(), Some("validate"));
    assert_eq!(history[0].trigger, RunTrigger::Manual);
}

/// A profile that asks on errors waits for the person; Continue anyway
/// writes the files. A run nobody started (an auto run) never asks: it
/// stops.
#[test]
fn ask_waits_for_the_person_and_an_auto_run_stops_instead() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let mut content = profile_job();
    content.profiles[0].validate = true;
    content.profiles[0].on_errors = OnErrors::Ask;
    let job = write_job(dir.path(), &content);
    let run = start(&mut app, &job, content.clone(), RunTrigger::Manual);
    let task = validated(&mut app, run, 2);
    settle(&mut app, task);
    assert!(app.output_jobs().runs.get(run).unwrap().is_asking());
    let task = app.answer_job_run(run, true);
    settle(&mut app, task);
    let done = app.output_jobs().runs.get(run).unwrap();
    assert!(done.stopped.is_none(), "{:?}", done.stopped);
    assert!(
        dir.path()
            .join("out")
            .join("rel")
            .join("Reports")
            .join("ProjectA-bom.csv")
            .is_file()
    );

    let other = tempfile::tempdir().unwrap();
    let mut app = project_app(other.path());
    let job = write_job(other.path(), &content);
    let run = start(&mut app, &job, content, RunTrigger::OnSave);
    let task = validated(&mut app, run, 2);
    settle(&mut app, task);
    let ended = app.output_jobs().runs.get(run).unwrap();
    assert_eq!(ended.stopped.as_ref().unwrap().at, "validate");
    assert!(!other.path().join("out").exists());
}

/// Stop ends the run going now; an output that lands after is dropped and
/// nothing is written.
#[test]
fn stop_ends_the_run_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let content = profile_job();
    let job = write_job(dir.path(), &content);
    let run = start(&mut app, &job, content, RunTrigger::Manual);
    let operand = format!("path={}", job.display());
    let answer = answered(headless(&mut app, "stop_output_job", &[&operand]));
    assert_eq!(answer["stopped"], true);
    let late = app.update(Message::from(RunMsg::OutputBuilt {
        run,
        node: 0,
        result: Arc::new(Err("late".into())),
    }));
    settle(&mut app, late);
    let ended = app.output_jobs().runs.get(run).unwrap();
    assert_eq!(ended.stopped.as_ref().unwrap().at, "stop");
    assert!(!dir.path().join("out").exists());
    let again = headless(&mut app, "stop_output_job", &[&operand]);
    assert!(refusal(&again).contains("no run"), "{again:?}");
}

fn board() -> PcbBoard {
    let mut board = SetupProfile::Standard.pcb(2).new_board();
    board.options.origin = Default::default();
    board.outline = vec![
        Point::new(10.0, 20.0),
        Point::new(60.0, 20.0),
        Point::new(60.0, 50.0),
        Point::new(10.0, 50.0),
    ];
    board
}

/// A project with its board open: board outputs come from it.
fn board_app(dir: &Path) -> Signex {
    let mut app = project_app(dir);
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    app.open_pcb_tab(dir.join("board.snxpcb"), "board".into(), board());
    app
}

fn gerber_job() -> OutputJob {
    OutputJob {
        outputs: vec![
            JobOutput::new("Gerber", OutputKind::Gerber),
            JobOutput::new("BOM", OutputKind::Bom),
        ],
        ..OutputJob::default()
    }
}

/// Gerber comes from the open board, named from the project; a
/// project whose board is not open writes nothing and says why.
#[test]
fn board_outputs_come_from_the_open_board() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = board_app(dir.path());
    let job = write_job(dir.path(), &gerber_job());
    let operand = format!("path={}", job.display());
    let answer = answered(headless(&mut app, "run_output_job", &[&operand]));
    let files: Vec<String> = tree(&dir.path().join("out")).into_keys().collect();
    assert!(files.iter().any(|f| f.ends_with(".gbr")), "{files:?}");
    assert!(files.iter().all(|f| f.starts_with("ProjectA")), "{files:?}");
    assert!(
        answer["outputs"][0]["files"]
            .as_array()
            .is_some_and(|f| !f.is_empty())
    );

    let other = tempfile::tempdir().unwrap();
    let mut app = project_app(other.path());
    let job = write_job(other.path(), &gerber_job());
    let operand = format!("path={}", job.display());
    let refused = headless(&mut app, "run_output_job", &[&operand]);
    assert!(refusal(&refused).contains("names no board"), "{refused:?}");
    assert!(!other.path().join("out").exists());
}

/// Mockup row 193b: View in Gerber Viewer opens exactly the files the run
/// wrote that the viewer reads, with the run's banner; the same run again
/// focuses the tab; Back to run summary opens the job on its summary.
#[test]
fn view_in_gerber_viewer_opens_the_runs_files_with_its_banner() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = board_app(dir.path());
    let job = write_job(dir.path(), &gerber_job());
    let task = app.open_output_job_tab(job.clone());
    settle(&mut app, task);
    let task = app.start_tab_run(&job, false);
    settle(&mut app, task);
    let task = app.update(Message::from(super::JobTabMsg::ViewInGerberPressed {
        job: job.clone(),
    }));
    settle(&mut app, task);
    let viewer = &app.document_view.gerber_viewer;
    let origin = viewer.from_run.as_ref().expect("the run banner");
    assert_eq!(origin.job, job);
    let open = viewer.open.as_ref().expect("the run's files are open");
    assert!(
        open.paths
            .iter()
            .all(|p| p.starts_with(dir.path().join("out")))
    );
    assert!(
        open.paths
            .iter()
            .all(|p| !p.to_string_lossy().ends_with(".csv")),
        "the BOM is not a viewer file"
    );
    let tabs = app.document_state.tabs.len();
    let task = app.view_job_run_in_gerber(&job, None);
    settle(&mut app, task);
    assert_eq!(
        app.document_state.tabs.len(),
        tabs,
        "the same run focuses its tab"
    );
    let task = app.update(Message::from(crate::app::GerberViewerMsg::BackToRunPressed));
    settle(&mut app, task);
    let active = &app.document_state.tabs[app.document_state.active_tab];
    assert!(active.kind.is_output_job());
    assert_eq!(app.output_jobs().tabs[&job].page, super::TabPage::Summary);
    let saved = read_output_job(&job).unwrap();
    assert!(
        !saved.history[0].viewable.is_empty(),
        "the history keeps the files"
    );
}
