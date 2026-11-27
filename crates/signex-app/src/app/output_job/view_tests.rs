//! Mockup rows 114 and 193b laid out: each page of the Export Job tab shows
//! its parts, the stopped run its banner, the summary View in Gerber Viewer
//! and the history, and the Gerber Viewer the run's banner.

use std::path::Path;
use std::sync::Arc;

use iced::Size;
use signex_types::output_job::RunTrigger;

use super::pipeline::{RunMsg, StartRun, ValidateOutcome};
use super::pipeline_tests::{profile_job, write_job};
use super::run::RunAsk;
use super::tab::{JobTabMsg, TabPage};
use super::tests::project_app;
use crate::app::handlers::menu::export::settle;
use crate::app::{Message, Signex};
use crate::layout_probe::lay_out_texts;

const WINDOW: Size = Size::new(1600.0, 1000.0);

/// The words of the tab of `job` laid out in the window.
fn words(app: &Signex, job: &Path) -> String {
    let texts = lay_out_texts(app.view_output_job_tab(job.to_path_buf()), WINDOW);
    texts
        .iter()
        .map(|laid| laid.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn page(app: &mut Signex, job: &Path, page: TabPage) {
    let task = app.update(Message::from(JobTabMsg::PagePicked {
        job: job.to_path_buf(),
        page,
    }));
    settle(app, task);
}

fn opened(dir: &Path) -> (Signex, std::path::PathBuf) {
    let mut app = project_app(dir);
    let job = write_job(dir, &profile_job());
    let task = app.open_output_job_tab(job.clone());
    settle(&mut app, task);
    (app, job)
}

/// Settings: the toolbar, the outputs by group with Add Output, the picked
/// output's form, the container and the pipeline; Automation: the triggers
/// and the command line.
#[test]
fn settings_and_automation_lay_out_their_parts() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, job) = opened(dir.path());
    let task = app.update(Message::from(JobTabMsg::OutputPicked {
        job: job.clone(),
        name: "PDF".into(),
    }));
    settle(&mut app, task);
    let shown = words(&app, &job);
    for part in [
        "Run profile",
        "Add Output",
        "Documentation",
        "Reports",
        "Output container",
        "Run pipeline",
        "Remove output",
    ] {
        assert!(shown.contains(part), "{part} in {shown}");
    }
    page(&mut app, &job, TabPage::Automation);
    let shown = words(&app, &job);
    for part in [
        "Run on save",
        "Keep outputs up to date",
        "Command line, for CI",
        "signex-cli run",
    ] {
        assert!(shown.contains(part), "{part} in {shown}");
    }
}

/// A run stopped at Validate shows its banner with Show the findings and
/// Run again; a complete run's summary offers View in Gerber Viewer and
/// lists the history.
#[test]
fn the_flow_of_a_stopped_run_and_the_summary_of_a_complete_one() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, job) = opened(dir.path());
    let mut content = profile_job();
    content.profiles[0].validate = true;
    let _first_step = app.start_job_run(StartRun {
        job_path: job.clone(),
        job: content,
        ask: RunAsk {
            trigger: Some(RunTrigger::Manual),
            ..RunAsk::default()
        },
        trigger: RunTrigger::Manual,
        waiting: None,
    });
    let run = app.output_jobs().runs.running(&job).expect("a run").id;
    assert_eq!(app.output_jobs().tabs[&job].page, TabPage::Flow);
    let task = app.update(Message::from(RunMsg::Validated {
        run,
        outcome: Arc::new(ValidateOutcome {
            errors: 1,
            failing: vec!["erc"],
            ..ValidateOutcome::default()
        }),
    }));
    settle(&mut app, task);
    let shown = words(&app, &job);
    for part in ["Show the findings", "Run again", "Validate"] {
        assert!(shown.contains(part), "{part} in {shown}");
    }

    let task = app.start_tab_run(&job, true);
    settle(&mut app, task);
    assert_eq!(app.output_jobs().tabs[&job].page, TabPage::Summary);
    let shown = words(&app, &job);
    for part in [
        "View in Gerber Viewer",
        "Written to",
        "History kept in this .snxout",
        "Copy path",
    ] {
        assert!(shown.contains(part), "{part} in {shown}");
    }
}

/// Mockup row 193b: the Gerber Viewer opened from a run names it and leads
/// back to its summary.
#[test]
fn the_viewer_names_the_run_it_was_opened_from() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    app.document_state.gerber_viewer.from_run = Some(crate::app::gerber_viewer::RunOrigin {
        job: dir.path().join("fab.snxout"),
        profile: Some("Release".into()),
        variant: "Base".into(),
        started: "2026-11-18 14:41:08".into(),
        folder: dir.path().join("out"),
    });
    app.open_gerber_viewer_tab();
    let texts = lay_out_texts(app.view_gerber_viewer_tab(), WINDOW);
    let shown: Vec<&str> = texts.iter().map(|laid| laid.text.as_str()).collect();
    assert!(
        shown.iter().any(|t| t.contains("Back to run summary")),
        "{shown:?}"
    );
    assert!(
        shown.iter().any(|t| t.contains("From Export Job run")),
        "{shown:?}"
    );
}
