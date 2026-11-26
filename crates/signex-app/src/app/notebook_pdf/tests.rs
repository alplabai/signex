//! Export Design Notebook PDF in a window (mockup row 202): the dialog opens
//! on the project's notebook and reads it, takes the default file name, the
//! settings change, the stale rule refuses, and Export writes the PDF and
//! shows the done card with its checksum.

use std::path::{Path, PathBuf};

use signex_notebook::AnnotationKind;
use signex_notebook::export::{
    DesignNames, ExportOptions, PageSelection, Paper, StaleCheck, StaleRule, inspect, sha256_hex,
};
use signex_notebook::{NotebookSources, PACKAGE_IMPORT, Reference};

use super::{IncludeFlag, NotebookPdfMsg, PageChoice, Stage};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::task_driver::TaskDriver;

fn notebook_text() -> String {
    format!(
        "{PACKAGE_IMPORT}\n#show: report.with(doc: \"DN-7\", rev: \"A\", title: [Buck])\n\
         = Power stage\nThe divider #component(\"R7\") sets #net(\"+3V3\").\n"
    )
}

/// An app with project `ProjectA` in `dir` and its notebook written.
fn app_with_notebook(dir: &tempfile::TempDir) -> Signex {
    let folder = dir.path().join("notebook");
    std::fs::create_dir_all(&folder).expect("notebook folder");
    std::fs::write(folder.join("design.snxnbk"), notebook_text()).expect("notebook");
    app_workspace(&dir.path().to_string_lossy(), &[])
}

fn send(app: &mut Signex, msg: NotebookPdfMsg) {
    TaskDriver::new()
        .send(app, Message::from(msg))
        .expect("settled");
}

fn opened(dir: &tempfile::TempDir) -> Signex {
    let mut app = app_with_notebook(dir);
    send(&mut app, NotebookPdfMsg::Requested { current_page: None });
    app
}

fn stage(app: &Signex) -> &Stage {
    &app.ui_state
        .notebook_pdf
        .dialog
        .as_ref()
        .expect("the dialog is open")
        .stage
}

fn default_pdf(dir: &tempfile::TempDir) -> PathBuf {
    dir.path()
        .join("Project Outputs")
        .join("notebook")
        .join("ProjectA-DN-7-revA.pdf")
}

#[test]
fn the_dialog_reads_the_notebook_and_takes_the_default_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let app = opened(&dir);
    let dialog = app.ui_state.notebook_pdf.dialog.as_ref().expect("open");
    assert_eq!(dialog.notebook, "design.snxnbk");
    assert_eq!(dialog.project_name, "ProjectA");
    assert_eq!(dialog.variants, ["Base"]);
    assert_eq!(dialog.options, ExportOptions::default());
    let Stage::Ready(inspection) = &dialog.stage else {
        panic!("ready, got {:?}", dialog.stage);
    };
    assert!(inspection.page_count >= 1);
    assert_eq!(inspection.title.doc.as_deref(), Some("DN-7"));
    assert_eq!(
        PathBuf::from(&dialog.path_text),
        Path::new("Project Outputs")
            .join("notebook")
            .join("ProjectA-DN-7-revA.pdf")
    );
}

#[test]
fn with_no_project_nothing_opens() {
    let (mut app, _) = Signex::new();
    send(&mut app, NotebookPdfMsg::Requested { current_page: None });
    assert!(app.ui_state.notebook_pdf.dialog.is_none());
}

#[test]
fn a_missing_notebook_says_it_cannot_be_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    send(&mut app, NotebookPdfMsg::Requested { current_page: None });
    let Stage::Unreadable(why) = stage(&app) else {
        panic!("unreadable, got {:?}", stage(&app));
    };
    assert!(why.contains("notebook/design.snxnbk"), "{why}");
}

#[test]
fn every_setting_follows_its_control() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    send(&mut app, NotebookPdfMsg::RangeEdited("2-3".into()));
    send(&mut app, NotebookPdfMsg::PaperPicked(Paper::Letter));
    send(&mut app, NotebookPdfMsg::VariantPicked("Lite".into()));
    send(
        &mut app,
        NotebookPdfMsg::IncludeToggled(IncludeFlag::PythonSource),
    );
    send(
        &mut app,
        NotebookPdfMsg::IncludeToggled(IncludeFlag::Contents),
    );
    send(&mut app, NotebookPdfMsg::KindToggled(AnnotationKind::Issue));
    send(&mut app, NotebookPdfMsg::StaleRulePicked(StaleRule::Fail));
    send(&mut app, NotebookPdfMsg::PathEdited("out/nb.pdf".into()));
    let dialog = app.ui_state.notebook_pdf.dialog.as_ref().expect("open");
    assert_eq!(
        dialog.choice,
        PageChoice::Range,
        "typing a range picks Range"
    );
    assert_eq!(
        dialog.page_selection(),
        Some(PageSelection::Range { first: 2, last: 3 })
    );
    let options = &dialog.options;
    assert_eq!(options.paper, Paper::Letter);
    assert_eq!(options.variant, "Lite");
    assert!(options.python_source && !options.contents);
    assert!(!options.annotations.contains(&AnnotationKind::Issue));
    assert_eq!(options.stale, StaleRule::Fail);
    assert_eq!(dialog.path_text, "out/nb.pdf");
    assert!(dialog.path_edited);
    send(&mut app, NotebookPdfMsg::KindToggled(AnnotationKind::Issue));
    send(&mut app, NotebookPdfMsg::PagesPicked(PageChoice::Current));
    let dialog = app.ui_state.notebook_pdf.dialog.as_ref().expect("open");
    assert!(dialog.options.annotations.contains(&AnnotationKind::Issue));
    assert_eq!(
        dialog.page_selection(),
        Some(PageSelection::Current { page: 1 })
    );
}

#[test]
fn a_range_that_names_no_page_refuses_the_export() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    send(&mut app, NotebookPdfMsg::RangeEdited("7-3".into()));
    send(&mut app, NotebookPdfMsg::ExportRequested);
    assert!(app.document_state.error_notice.is_some(), "it says why");
    assert!(matches!(stage(&app), Stage::Ready(_)), "nothing started");
    assert!(!default_pdf(&dir).exists());
}

/// The dialog's inspection with `R7` stale, as the design check reports it.
fn mark_r7_stale(app: &mut Signex) {
    let dialog = app.ui_state.notebook_pdf.dialog.as_mut().expect("open");
    let Stage::Ready(inspection) = &mut dialog.stage else {
        panic!("ready");
    };
    inspection.stale = StaleCheck {
        references: 2,
        stale: vec![Reference::Component("R7".into())],
    };
}

#[test]
fn a_stale_reference_refuses_the_export_only_under_fail() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    mark_r7_stale(&mut app);
    send(&mut app, NotebookPdfMsg::StaleRulePicked(StaleRule::Fail));
    send(&mut app, NotebookPdfMsg::ExportRequested);
    let notice = app.document_state.error_notice.as_ref().expect("refused");
    assert!(
        signex_i18n::RenderTranslatable::render_en_us(&notice.detail).contains("R7"),
        "the notice names the stale reference"
    );
    assert!(!default_pdf(&dir).exists());
    app.document_state.error_notice = None;
    send(&mut app, NotebookPdfMsg::StaleRulePicked(StaleRule::Warn));
    send(&mut app, NotebookPdfMsg::ExportRequested);
    assert!(default_pdf(&dir).exists(), "Warn exports anyway");
}

#[test]
fn export_writes_the_pdf_and_shows_the_done_card() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    send(&mut app, NotebookPdfMsg::ExportRequested);
    let Stage::Done(done) = stage(&app) else {
        panic!("done, got {:?}", stage(&app));
    };
    assert_eq!(done.path, default_pdf(&dir));
    let bytes = std::fs::read(&done.path).expect("the PDF is written");
    assert!(bytes.starts_with(b"%PDF-"));
    assert_eq!(done.summary.sha256, sha256_hex(&bytes));
    assert_eq!(done.summary.bookmarks.sections, 1);
    assert_eq!(done.same_as_last, None, "the first export");

    send(&mut app, NotebookPdfMsg::Requested { current_page: None });
    send(&mut app, NotebookPdfMsg::ExportRequested);
    let Stage::Done(again) = stage(&app) else {
        panic!("done again");
    };
    assert_eq!(
        again.same_as_last,
        Some(true),
        "the same file, byte for byte"
    );
    assert_eq!(std::fs::read(&again.path).expect("read"), bytes);
}

#[test]
fn the_pdf_is_the_one_the_export_makes_from_the_notebook() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    send(&mut app, NotebookPdfMsg::ExportRequested);
    let written = std::fs::read(default_pdf(&dir)).expect("written");
    let sources = NotebookSources::new("design.snxnbk", notebook_text());
    let made = signex_notebook::export::export_pdf(
        &sources,
        &ExportOptions::default(),
        "ProjectA/notebook/design.snxnbk",
        |_| true,
    )
    .expect("exports");
    assert_eq!(written, made.pdf, "the dialog adds nothing of its own");
}

#[test]
fn an_answer_for_an_earlier_opening_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    let sources = NotebookSources::new("design.snxnbk", "= Other\n");
    let other = inspect(&sources, |r| DesignNames::default().has(r));
    let generation = app
        .ui_state
        .notebook_pdf
        .dialog
        .as_ref()
        .expect("open")
        .generation;
    let dialog = app.ui_state.notebook_pdf.dialog.as_mut().expect("open");
    dialog.stage = Stage::Reading;
    send(
        &mut app,
        NotebookPdfMsg::Inspected {
            generation: generation - 1,
            result: std::sync::Arc::new(Ok(other)),
        },
    );
    assert!(
        matches!(stage(&app), Stage::Reading),
        "a late answer is dropped"
    );
}

#[test]
fn close_drops_the_dialog() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    send(&mut app, NotebookPdfMsg::Closed);
    assert!(app.ui_state.notebook_pdf.dialog.is_none());
}

#[test]
fn the_include_ticks_read_their_settings() {
    let mut options = ExportOptions::default();
    for flag in IncludeFlag::ALL {
        let before = flag.get(&options);
        flag.toggle(&mut options);
        assert_ne!(flag.get(&options), before, "{flag:?}");
    }
}
