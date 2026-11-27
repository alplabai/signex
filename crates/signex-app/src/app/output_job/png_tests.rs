//! #1299: the pages of a schematic as PNG files, through the palette row and
//! its folder picker and through the headless path, at the resolution asked.

use signex_types::output_job::DEFAULT_PNG_DPI;

use super::tests::{answer_picker, files_in, headless, project_app, refusal};
use crate::app::Message;
use crate::app::command::CommandOutcome;
use crate::app::handlers::menu::export::settle;
use crate::menu_bar::MenuMessage;

/// Width and height of a PNG file, from its header.
fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "a PNG");
    let word =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    (word(16), word(20))
}

#[test]
fn the_cli_writes_each_page_at_the_resolution_it_names() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let sheet = format!("document={}", dir.path().join("main.snxsch").display());
    let folder = dir.path().join("png");
    let path = format!("path={}", folder.display());
    let CommandOutcome::Answered(answer) =
        headless(&mut app, "export_sheet_png", &[&sheet, &path, "dpi=50"])
    else {
        panic!("not answered");
    };
    assert_eq!(answer["outputs"][0]["files"][0], "ProjectA-1.png");
    assert_eq!(answer["folder"], folder.display().to_string());
    let files = files_in(&folder);
    let (low_w, low_h) = png_size(&files["ProjectA-1.png"]);

    let finer = dir.path().join("finer");
    let path = format!("path={}", finer.display());
    let outcome = headless(&mut app, "export_sheet_png", &[&sheet, &path, "dpi=100"]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    let (w, h) = png_size(&files_in(&finer)["ProjectA-1.png"]);
    assert!(w > low_w && h > low_h, "twice the dpi, more pixels");
    assert!((f64::from(w) / f64::from(low_w) - 2.0).abs() < 0.05);
}

#[test]
fn a_resolution_out_of_range_or_a_missing_sheet_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    let sheet = format!("document={}", dir.path().join("main.snxsch").display());
    let folder = dir.path().join("png");
    let path = format!("path={}", folder.display());
    for dpi in ["dpi=5", "dpi=5000", "dpi=72.5", "dpi=high"] {
        let refused = headless(&mut app, "export_sheet_png", &[&sheet, &path, dpi]);
        assert!(refusal(&refused).contains("`dpi`"), "{dpi}: {refused:?}");
    }
    let closed = format!("document={}", dir.path().join("other.snxsch").display());
    let refused = headless(&mut app, "export_sheet_png", &[&closed, &path]);
    assert!(
        matches!(refused, CommandOutcome::Disabled { .. }),
        "{refused:?}"
    );
    assert!(!folder.exists(), "nothing written");
}

/// The palette row opens the folder picker and writes every page at the
/// default resolution.
#[test]
fn the_palette_row_writes_the_pages_into_the_picked_folder() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(dir.path());
    // A small paper keeps the 300 dpi page quick to draw.
    let sheet = dir.path().join("main.snxsch");
    if let Some(engine) = app.document_state.engines.get_mut(&sheet) {
        let mut doc = engine.document().clone();
        doc.paper_size = "A5".into();
        *engine = signex_engine::Engine::new(doc).expect("engine");
    }
    let folder = dir.path().join("pages");
    answer_picker(&mut app, &folder);
    let task = app.update(Message::Menu(MenuMessage::command("export_sheet_png")));
    settle(&mut app, task);
    let files = files_in(&folder);
    let (w, h) = png_size(&files["ProjectA-1.png"]);
    // A5 is 210 x 148 mm, either way round.
    let expect = |mm: f64| (mm / 25.4 * f64::from(DEFAULT_PNG_DPI)) as u32;
    assert!(w.max(h).abs_diff(expect(210.0)) <= 2, "{w} x {h}");
    assert!(w.min(h).abs_diff(expect(148.0)) <= 2, "{w} x {h}");
    assert!(app.document_state.error_notice.is_none());
}
