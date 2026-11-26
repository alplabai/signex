//! Mockup rows 196 and 199: the notebook tab's margin lane. A timeline
//! opens beside the preview with its target as a tab, is read with the
//! compile, closes, and exports as CSV; Re-measure… inserts a Measurement
//! block.

use super::tests::{app_with_notebook, body, send};
use super::{NotebookMsg, ViewMode};
use signex_notebook::Reference;

/// A notebook with two ripple readings of `+3V3`.
const MEASURED: &str = "+++\nformat = \"snxnbk/1\"\n+++\n\
    #import \"@signex/notebook:0.1.0\": *\n\
    = Bring-up\n\
    #measurement(id: \"MEAS-001\", target: net(\"+3V3\"), quantity: \"ripple\", \
    value: 27.0, unit: \"mV\", limit: (max: 30), timestamp: \"2026-10-09\")\n\
    #measurement(id: \"MEAS-002\", target: net(\"+3V3\"), quantity: \"ripple\", \
    value: 18.0, unit: \"mV\", limit: (max: 30), timestamp: \"2026-11-24\")\n";

fn rail() -> Reference {
    Reference::Net("+3V3".to_string())
}

#[test]
fn a_timeline_opens_beside_the_preview_with_its_readings() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), MEASURED);
    send(
        &mut app,
        NotebookMsg::ViewPicked {
            project_id: id,
            view: ViewMode::Source,
        },
    );
    send(
        &mut app,
        NotebookMsg::TimelineOpened {
            project_id: id,
            reference: rail(),
        },
    );
    let tab = &app.ui_state.notebook_tabs[&id];
    assert_eq!(tab.view, ViewMode::Split, "the lane needs the preview");
    let open = tab.timeline.as_ref().expect("a timeline is open");
    assert_eq!(open.targets, [rail()]);
    assert_eq!(open.shown, rail());
    let lane = tab.compile.lane().expect("a compile ran");
    let timeline = lane.timeline.as_ref().expect("the compile read it");
    assert_eq!(timeline.reference, rail());
    assert_eq!(timeline.entries.len(), 2, "{timeline:?}");
    assert_eq!(lane.summary.readings, 2);

    send(&mut app, NotebookMsg::TimelineClosed { project_id: id });
    assert!(app.ui_state.notebook_tabs[&id].timeline.is_none());
}

#[test]
fn the_timeline_exports_as_csv_to_the_picked_file() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), MEASURED);
    send(
        &mut app,
        NotebookMsg::TimelineOpened {
            project_id: id,
            reference: rail(),
        },
    );
    let path = dir.path().join("ripple.csv");
    send(
        &mut app,
        NotebookMsg::TimelineExportChosen {
            project_id: id,
            path: Some(path.clone()),
        },
    );
    let csv = std::fs::read_to_string(&path).expect("the CSV was written");
    assert!(csv.contains("MEAS-001"), "{csv}");
    assert!(csv.contains("MEAS-002"), "{csv}");

    let cancelled = dir.path().join("none.csv");
    send(
        &mut app,
        NotebookMsg::TimelineExportChosen {
            project_id: id,
            path: None,
        },
    );
    assert!(!cancelled.exists());
}

#[test]
fn remeasure_inserts_a_measurement_block_at_the_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), MEASURED);
    let before = body(&app, id).matches("#measurement(").count();
    send(&mut app, NotebookMsg::RemeasurePressed { project_id: id });
    let after = body(&app, id).matches("#measurement(").count();
    assert_eq!(after, before + 1);
}

/// A notebook file with a stale attachment, and a chapter with another.
const ATTACHED: &str = "+++\nformat = \"snxnbk/1\"\n+++\n= Bring-up\n\
    #attachment(\"scope/a.csv\", taken-with: (\"L1\": \"3.3 µH\"))\n\
    #include \"chapters/b.snxnbk\"\n";
const CHAPTER: &str = "= Bench\n#attachment(\"scope/b.csv\", taken-with: (\"L1\": \"3.3 µH\"))\n";

fn attached_doc(main: &str) -> signex_notebook::NotebookDocument {
    let mut doc = signex_notebook::NotebookDocument::reading("design.snxnbk".into());
    doc.open_text(main).expect("opens");
    assert!(doc.sources.insert("chapters/b.snxnbk", CHAPTER));
    doc
}

fn l1_now() -> signex_notebook::attachment::DesignValues {
    [("L1", "4.7 µH")].into_iter().collect()
}

/// Mark still valid is found again where the attachment is now, after an
/// edit moved it since the card was drawn.
#[test]
fn mark_still_valid_finds_an_attachment_that_moved() {
    let moved = ATTACHED.replace("= Bring-up\n", "= Bring-up\nA new line.\n\n");
    let doc = attached_doc(&moved);
    let marked =
        super::lane::marked_valid(&doc, "scope/a.csv", "design.snxnbk", &l1_now()).expect("marked");
    assert!(
        marked.contains("#attachment(\"scope/a.csv\", taken-with: (\"L1\": \"4.7 µH\"))"),
        "{marked}"
    );
    assert!(marked.contains("A new line."), "the rest is kept: {marked}");
}

/// The tab holds the notebook file only: an attachment in a chapter, or
/// one no longer there, is refused.
#[test]
fn mark_still_valid_refuses_a_chapter_or_a_gone_attachment() {
    use signex_i18n::RenderTranslatable as _;
    let doc = attached_doc(ATTACHED);
    let chapter = super::lane::marked_valid(&doc, "scope/b.csv", "chapters/b.snxnbk", &l1_now())
        .expect_err("in a chapter");
    assert!(chapter.render_en_us().contains("chapters/b.snxnbk"));
    let moved_there = super::lane::marked_valid(&doc, "scope/b.csv", "design.snxnbk", &l1_now())
        .expect_err("now in a chapter");
    assert!(moved_there.render_en_us().contains("chapters/b.snxnbk"));
    let gone = super::lane::marked_valid(&doc, "scope/c.csv", "design.snxnbk", &l1_now())
        .expect_err("gone");
    assert!(gone.render_en_us().contains("scope/c.csv"));
}
