//! The Board size card (mockup row 38, decision 2026-11-24, #2815): Measured
//! from, the read-only preview of the board against the limit and the
//! values the last run measured, in the header and on the drawing.

use iced::Size;
use serde_json::json;
use signex_i18n::I18n;
use signex_types::format::SnxPcb;
use signex_types::pcb::Point;
use signex_types::project::SetupProfile;
use signex_widgets::property_illustration::{Illustration, Ink, Paint, Primitive};

use super::super::RequirementsMsg;
use super::board_size::{Limit, fits, illustration, mm_text};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectSection, ProjectTabMsg, Signex};
use crate::layout_probe::{lay_out_inputs, lay_out_texts};
use crate::styles::board_palette::{BoardPalette, BoardSwatch};

const LIMIT: Limit = Limit {
    width_mm: 50.0,
    height_mm: 40.0,
};

/// Text as shown, without the isolation marks Fluent puts around a value.
fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

/// Every label and callout text of `drawing`, in drawing order.
fn words(drawing: &Illustration) -> Vec<String> {
    drawing
        .visible_parts()
        .filter_map(|part| match &part.primitive {
            Primitive::Label(label) => Some(plain(label.text.as_str())),
            Primitive::Callout(callout) => callout.label.as_ref().map(|l| plain(l.as_str())),
            _ => None,
        })
        .collect()
}

/// The stroke paint of the drawn board, the one rounded rectangle.
fn board_stroke(drawing: &Illustration) -> Option<Paint> {
    drawing
        .visible_parts()
        .find(|part| matches!(part.primitive, Primitive::RoundedRect { .. }))
        .and_then(|part| part.style.stroke)
        .map(|stroke| stroke.paint)
}

#[test]
fn a_board_inside_the_limit_draws_in_the_edge_colour_with_its_margin() {
    let i18n = I18n::en_us();
    let drawing = illustration(LIMIT, Some((48.5, 36.0)), &i18n).expect("a drawing");
    let shown = words(&drawing);
    for expected in [
        "48.5 × 36 mm measured",
        "≤ 50 mm",
        "≤ 40",
        "limit (fits the enclosure)",
        "board outline, last run",
        "margin 1.5 × 4 mm",
        "the board fits",
    ] {
        assert!(
            shown.iter().any(|w| w == expected),
            "{expected:?} in {shown:?}"
        );
    }
    let edge = BoardPalette::new().swatch(BoardSwatch::BoardEdge);
    assert_eq!(board_stroke(&drawing), Some(Paint::Document(edge)));
}

#[test]
fn a_board_over_the_limit_draws_in_the_error_colour_and_says_so() {
    let i18n = I18n::en_us();
    let drawing = illustration(LIMIT, Some((51.25, 36.0)), &i18n).expect("a drawing");
    let shown = words(&drawing);
    assert!(
        shown.iter().any(|w| w == "the board does not fit"),
        "{shown:?}"
    );
    assert!(
        shown.iter().any(|w| w == "margin -1.25 × 4 mm"),
        "{shown:?}"
    );
    assert_eq!(board_stroke(&drawing), Some(Paint::Ink(Ink::Error)));
    assert!(!fits(LIMIT, (51.25, 36.0)));
    assert!(fits(LIMIT, (50.0, 40.0)), "the limit itself fits");
}

#[test]
fn before_a_run_the_preview_draws_the_limit_and_asks_for_one() {
    let i18n = I18n::en_us();
    let drawing = illustration(LIMIT, None, &i18n).expect("a drawing");
    assert_eq!(board_stroke(&drawing), None, "no board is drawn");
    let shown = words(&drawing);
    assert!(shown.iter().any(|w| w == "≤ 50 mm"), "{shown:?}");
    assert!(
        shown.iter().any(|w| w == "Run checks to measure the board"),
        "{shown:?}"
    );
    assert!(!shown.iter().any(|w| w.starts_with("margin")), "{shown:?}");
    let bad = Limit {
        width_mm: 0.0,
        height_mm: 40.0,
    };
    assert!(illustration(bad, None, &i18n).is_none(), "nothing to draw");
}

#[test]
fn lengths_read_without_trailing_zeros() {
    assert_eq!(mm_text(36.0), "36");
    assert_eq!(mm_text(48.25), "48.25");
    assert_eq!(mm_text(0.1234), "0.123");
    assert_eq!(mm_text(-0.0001), "0");
}

/// A project tab on the Requirements section, its project listing a board
/// 48.5 x 36 mm written to its folder, and one Board size requirement of
/// 50 x 40 mm.
fn app_with_board() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    let mut board = SetupProfile::Standard.pcb(2).new_board();
    board.outline = vec![
        Point::new(0.0, 0.0),
        Point::new(48.5, 0.0),
        Point::new(48.5, 36.0),
        Point::new(0.0, 36.0),
    ];
    let project = &mut app.document_state.projects[0];
    let text = SnxPcb::new(board).write_string().expect("encode");
    std::fs::write(project.dir().join("board.snxpcb"), text).expect("write board");
    project.data.pcb_file = Some("board.snxpcb".into());
    project.data.requirements = serde_json::from_value(json!([
        {"id": "fits-enclosure", "kind": "board_size", "max_width_mm": 50, "max_height_mm": 40},
    ]))
    .expect("a list");
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Requirements,
        },
    )));
    (app, id, dir)
}

fn send(app: &mut Signex, id: ProjectId, msg: RequirementsMsg) -> iced::Task<Message> {
    app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::Requirements {
            project_id: id,
            msg,
        },
    )))
}

fn shown_texts(app: &Signex, id: ProjectId) -> Vec<String> {
    lay_out_texts(app.view_project_tab(id), Size::new(1600.0, 2400.0))
        .iter()
        .map(|t| plain(&t.text))
        .collect()
}

#[test]
fn run_checks_measures_the_board_and_the_card_shows_it() {
    let (mut app, id, _dir) = app_with_board();
    let before = shown_texts(&app, id);
    assert!(
        !before.iter().any(|t| t.starts_with("· measured")),
        "nothing is measured before a run: {before:?}"
    );
    // Before a run the card is open: Measured from and the preview show.
    assert!(before.iter().any(|t| t == "Measured from"), "{before:?}");
    assert!(
        before.iter().any(|t| t == "Board against the limit"),
        "{before:?}"
    );
    let inputs = lay_out_inputs(app.view_project_tab(id), Size::new(1600.0, 2400.0));
    assert!(
        inputs.iter().any(|i| plain(&i.text) == "Board outline"),
        "{inputs:?}"
    );

    let task = send(&mut app, id, RequirementsMsg::RunRequested);
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run settles");
    let tab = &app.document_view.project_tabs[&id].requirements;
    let run = tab.last_run.as_deref().expect("a run");
    assert_eq!(run.board_mm, Some((48.5, 36.0)));
    let report = run.report.as_ref().expect("a report");
    assert_eq!(report.passed, ["req::fits-enclosure"]);

    // A passed card is closed to its header, which carries the measurement.
    let after = shown_texts(&app, id);
    assert!(
        after.iter().any(|t| t == "· measured 48.5 × 36 mm"),
        "{after:?}"
    );
    assert!(!after.iter().any(|t| t == "Measured from"), "{after:?}");
    let _ = send(&mut app, id, RequirementsMsg::ExpandToggled { index: 0 });
    let open = shown_texts(&app, id);
    assert!(open.iter().any(|t| t == "Measured from"), "{open:?}");
    assert!(
        open.iter().any(|t| t == "Board against the limit"),
        "{open:?}"
    );
}

#[test]
fn a_run_without_a_board_measures_nothing() {
    let (mut app, id, _dir) = app_with_board();
    app.document_state.projects[0].data.pcb_file = None;
    let task = send(&mut app, id, RequirementsMsg::RunRequested);
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run settles");
    let tab = &app.document_view.project_tabs[&id].requirements;
    assert_eq!(tab.last_run.as_deref().expect("a run").board_mm, None);
    let shown = shown_texts(&app, id);
    assert!(
        !shown.iter().any(|t| t.starts_with("· measured")),
        "{shown:?}"
    );
}
