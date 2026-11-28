//! Mockup rows 114 and 167 through `Signex::update`: View ▸ Panels ▸
//! Export Jobs opens the project's job (or a new one) as a document tab
//! titled `fab.snxout — Export Job`; an edit marks it unsaved and Save
//! writes it; the profile popover's actions; Run from a tab with unsaved
//! edits saves first; a closed dirty tab keeps its draft for Save All.

use std::path::Path;

use signex_types::output_job::{BeforeClose, OnErrors, OutputJob, OutputKind, RunTrigger};

use super::pipeline_tests::{profile_job, tree, write_job};
use super::tab::{JobTabMsg, NamingMode, PackagingSwitch, TabPage, TriggerEdit};
use super::tests::project_app;
use crate::app::handlers::menu::export::settle;
use crate::app::output_job::file::read_output_job;
use crate::app::{Message, Signex};
use crate::menu_bar::MenuMessage;
use crate::panels::PanelKind;

fn send(app: &mut Signex, msg: JobTabMsg) {
    let task = app.update(Message::from(msg));
    settle(app, task);
}

fn open(app: &mut Signex, job: &Path) {
    let task = app.open_output_job_tab(job.to_path_buf());
    settle(app, task);
}

fn draft<'a>(app: &'a Signex, job: &Path) -> &'a OutputJob {
    app.document_state.output_jobs.get(job).expect("the draft")
}

fn active_is(app: &Signex, job: &Path) -> bool {
    let tab = &app.document_state.tabs[app.document_state.active_tab];
    tab.kind.is_output_job() && tab.path == job
}

/// The panel row with no job in the project opens a new, unsaved one named
/// after the project; with a job it opens that job, titled by its file.
#[test]
fn the_panel_row_opens_the_projects_job_or_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let task = app.update(Message::Menu(MenuMessage::command("open_output_job")));
    settle(&mut app, task);
    let fresh = dir.path().join("ProjectA.snxout");
    assert!(active_is(&app, &fresh));
    assert!(
        app.document_state.is_document_dirty(&fresh),
        "unsaved until Save"
    );
    assert!(!fresh.exists());

    let other = tempfile::tempdir().unwrap();
    let mut app = project_app(other.path());
    let job = write_job(other.path(), &profile_job());
    let task = app.show_panel(PanelKind::OutputJobs);
    settle(&mut app, task);
    assert!(active_is(&app, &job));
    let tab = &app.document_state.tabs[app.document_state.active_tab];
    assert_eq!(tab.title, "fab.snxout — Export Job");
    assert_eq!(draft(&app, &job), &profile_job());
    assert!(!app.document_state.is_document_dirty(&job));
    assert!(
        super::opens_by_name("FAB.SNXOUT"),
        "a Projects row opens it"
    );
}

/// Add Output picks the new output and marks the job unsaved; Save writes
/// it and the job reads clean.
#[test]
fn an_edit_marks_the_job_unsaved_and_save_writes_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &profile_job());
    open(&mut app, &job);
    send(
        &mut app,
        JobTabMsg::OutputAdded {
            job: job.clone(),
            kind: OutputKind::Gerber,
        },
    );
    assert!(app.document_state.is_document_dirty(&job));
    assert_eq!(
        app.output_jobs().tabs[&job].selected.as_deref(),
        Some("Gerber")
    );
    assert!(
        draft(&app, &job).profiles[0].ticks("Gerber"),
        "ticked in the shown profile"
    );
    let task = app.save_active_document().expect("the save starts");
    settle(&mut app, task);
    assert!(!app.document_state.is_document_dirty(&job));
    let saved = read_output_job(&job).unwrap();
    assert!(saved.output("Gerber").is_some());
}

/// The profile popover: Save as profile…, Set as default, Rename…, the
/// variant, the container, the policy and the triggers of the shown
/// profile; Delete… drops it and the default follows.
#[test]
fn the_profile_popover_and_the_shown_profiles_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    app.document_state.projects[0]
        .data
        .variant_definitions
        .push("Lite".into());
    let job = write_job(dir.path(), &profile_job());
    open(&mut app, &job);
    let j = || job.clone();
    send(
        &mut app,
        JobTabMsg::ProfileNamingStarted {
            job: j(),
            mode: NamingMode::SaveAs,
        },
    );
    send(
        &mut app,
        JobTabMsg::ProfileNameEdited {
            job: j(),
            text: "Docs".into(),
        },
    );
    send(&mut app, JobTabMsg::ProfileNamingConfirmed { job: j() });
    assert_eq!(
        app.output_jobs().tabs[&job].profile.as_deref(),
        Some("Docs")
    );
    send(&mut app, JobTabMsg::ProfileMadeDefault { job: j() });
    send(
        &mut app,
        JobTabMsg::VariantPicked {
            job: j(),
            variant: "Lite".into(),
        },
    );
    send(
        &mut app,
        JobTabMsg::PackagingToggled {
            job: j(),
            switch: PackagingSwitch::ReleaseFolder,
        },
    );
    send(
        &mut app,
        JobTabMsg::OnErrorsPicked {
            job: j(),
            policy: OnErrors::Ask,
        },
    );
    send(
        &mut app,
        JobTabMsg::TriggerEdited {
            job: j(),
            edit: TriggerEdit::OnSaveToggled,
        },
    );
    send(
        &mut app,
        JobTabMsg::TriggerEdited {
            job: j(),
            edit: TriggerEdit::WaitEdited("5".into()),
        },
    );
    send(
        &mut app,
        JobTabMsg::TriggerEdited {
            job: j(),
            edit: TriggerEdit::BeforeClosePicked(BeforeClose::Always),
        },
    );
    let docs = draft(&app, &job).profile("Docs").expect("the copy").clone();
    assert_eq!(draft(&app, &job).default_profile.as_deref(), Some("Docs"));
    assert_eq!(docs.variant.as_deref(), Some("Lite"));
    assert!(docs.release_folder && docs.zip, "the copy keeps the zip");
    assert_eq!(docs.on_errors, OnErrors::Ask);
    assert!(docs.triggers.on_save);
    assert_eq!(docs.triggers.wait_seconds, 5);
    assert_eq!(docs.triggers.before_close, BeforeClose::Always);
    send(
        &mut app,
        JobTabMsg::ProfileNamingStarted {
            job: j(),
            mode: NamingMode::Rename,
        },
    );
    send(
        &mut app,
        JobTabMsg::ProfileNameEdited {
            job: j(),
            text: "Docs only".into(),
        },
    );
    send(&mut app, JobTabMsg::ProfileNamingConfirmed { job: j() });
    assert_eq!(
        draft(&app, &job).default_profile.as_deref(),
        Some("Docs only")
    );
    send(&mut app, JobTabMsg::ProfileDeleted { job: j() });
    let left = draft(&app, &job);
    assert_eq!(left.profiles.len(), 1);
    assert_eq!(left.default_profile.as_deref(), Some("Release"));
    assert!(app.document_state.is_document_dirty(&job));
}

/// Run profile on a job with unsaved edits saves it first; the run writes
/// the profile's files, ends on the summary and is kept in the history.
#[test]
fn run_from_the_tab_saves_first_and_ends_on_the_summary() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &profile_job());
    open(&mut app, &job);
    send(
        &mut app,
        JobTabMsg::OutputTicked {
            job: job.clone(),
            name: "Netlist".into(),
        },
    );
    assert!(app.document_state.is_document_dirty(&job));
    send(
        &mut app,
        JobTabMsg::RunPressed {
            job: job.clone(),
            selected: false,
        },
    );
    assert!(!app.document_state.is_document_dirty(&job));
    let files: Vec<String> = tree(&dir.path().join("out").join("rel"))
        .into_keys()
        .collect();
    assert_eq!(
        files,
        [
            "Documentation/ProjectA.pdf",
            "Reports/ProjectA-bom.csv",
            "Reports/ProjectA.NET"
        ]
    );
    assert_eq!(app.output_jobs().tabs[&job].page, TabPage::Summary);
    let saved = read_output_job(&job).unwrap();
    assert!(saved.profiles[0].ticks("Netlist"), "saved before the run");
    assert_eq!(saved.history[0].trigger, RunTrigger::Manual);
    assert_eq!(saved.history[0].outputs, 3);
    assert_eq!(saved.history[0].zip.as_deref(), Some("rel.zip"));
}

/// Closing a tab with unsaved edits keeps its draft: the dot moves to Save
/// All, which writes it; a clean tab's draft goes with it.
#[test]
fn a_closed_dirty_tab_keeps_its_draft_for_save_all() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &profile_job());
    open(&mut app, &job);
    send(
        &mut app,
        JobTabMsg::OutputEnabledToggled {
            job: job.clone(),
            name: "PDF".into(),
        },
    );
    let idx = app.document_state.active_tab;
    let task = app.close_tab_parking_dirty(idx);
    settle(&mut app, task);
    assert!(!app.has_output_job_tab(&job));
    assert!(app.document_state.output_jobs.contains_key(&job), "parked");
    assert!(app.document_state.is_document_dirty(&job));
    let task = app.begin_document_path_save(&job).expect("the save starts");
    settle(&mut app, task);
    assert!(!read_output_job(&job).unwrap().outputs[0].enabled);
    assert!(!app.document_state.is_document_dirty(&job));
    assert!(
        !app.document_state.output_jobs.contains_key(&job),
        "no tab holds it now"
    );

    open(&mut app, &job);
    let idx = app.document_state.active_tab;
    let task = app.close_tab_parking_dirty(idx);
    settle(&mut app, task);
    assert!(
        !app.document_state.output_jobs.contains_key(&job),
        "a clean draft goes"
    );
}

/// The pages switch on the tab; a node of the flow and a history row are
/// picked; Add Output's menu opens and closes.
#[test]
fn pages_menus_and_picks_are_the_tabs_own() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let job = write_job(dir.path(), &profile_job());
    open(&mut app, &job);
    send(
        &mut app,
        JobTabMsg::PagePicked {
            job: job.clone(),
            page: TabPage::Automation,
        },
    );
    assert_eq!(app.output_jobs().tabs[&job].page, TabPage::Automation);
    send(&mut app, JobTabMsg::AddMenuToggled { job: job.clone() });
    assert!(app.output_jobs().tabs[&job].add_menu);
    send(&mut app, JobTabMsg::ProfileMenuToggled { job: job.clone() });
    let ui = &app.output_jobs().tabs[&job];
    assert!(ui.profile_menu && !ui.add_menu, "one menu at a time");
    send(
        &mut app,
        JobTabMsg::FlowNodePicked {
            job: job.clone(),
            node: Some(1),
        },
    );
    send(
        &mut app,
        JobTabMsg::HistoryPicked {
            job: job.clone(),
            index: 0,
        },
    );
    let ui = &app.output_jobs().tabs[&job];
    assert_eq!(ui.flow_node, Some(1));
    assert_eq!((ui.history_pick, ui.page), (Some(0), TabPage::Summary));
    assert!(
        !app.document_state.is_document_dirty(&job),
        "nothing of the job changed"
    );
}

/// A job with no profile runs every enabled output; the first untick makes
/// the default profile, ticking every output but that one.
#[test]
fn the_first_untick_makes_the_default_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let mut content = profile_job();
    content.profiles.clear();
    content.default_profile = None;
    let job = write_job(dir.path(), &content);
    open(&mut app, &job);
    send(
        &mut app,
        JobTabMsg::OutputTicked {
            job: job.clone(),
            name: "BOM".into(),
        },
    );
    let made = draft(&app, &job);
    assert_eq!(made.default_profile.as_deref(), Some("Default"));
    assert_eq!(made.profiles[0].outputs, ["PDF", "Netlist"]);
}
