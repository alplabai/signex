//! Mockup row 110 (#3002) through `Signex::update`: a bake refused for an
//! open outline is an error toast with the gap's place, not a modal card
//! naming an edge id; the place frames the gap on the canvas, the close
//! button dismisses the toast, and the headless close names the place too.

use iced::Size;

use super::tests::{send, sketch_edit, sketching};
use crate::app::state::RefusedGap;
use crate::app::{BoardSketchMsg, Message, PcbMsg, Signex};
use crate::layout_probe;
use crate::library::editor::footprint::SketchTool;
use crate::library::messages::FootprintEditorMsg;
use crate::panels::{BoardSketchAttrMsg, PanelMsg};
use signex_i18n::RenderTranslatable as _;
use signex_sketch::id::SketchEntityId;
use signex_types::pcb::Point;

const TOAST: &str = "The sketch was not applied to the board: the outline is not closed.";

fn sketch(event: BoardSketchMsg) -> Message {
    Message::Pcb(PcbMsg::Sketch(event))
}

/// A 10 mm line from (60, 60) tagged as the board outline: an outline that
/// does not close. Returns the line.
fn open_outline(app: &mut Signex) -> SketchEntityId {
    let path = app.active_tab_path().expect("the board tab");
    sketch_edit(
        app,
        &path,
        FootprintEditorMsg::SketchSetTool(SketchTool::Line),
    );
    for x_mm in [60.0, 70.0] {
        sketch_edit(
            app,
            &path,
            FootprintEditorMsg::SketchToolClick {
                x_mm,
                y_mm: 60.0,
                snap_id: None,
            },
        );
    }
    sketch_edit(app, &path, FootprintEditorMsg::ToolEscape);
    let line = {
        let session = app.interaction_state.board_sketch.as_ref().expect("on");
        session
            .working_sketch()
            .entities
            .last()
            .expect("the line")
            .id
    };
    send(
        app,
        Message::Dock(crate::dock::DockMessage::Panel(PanelMsg::BoardSketch(
            BoardSketchAttrMsg::OutlineToggled { id: line, on: true },
        ))),
    );
    line
}

/// The main window's texts, isolation marks stripped.
fn main_window_texts(app: &mut Signex) -> Vec<String> {
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    layout_probe::lay_out_texts(app.view(window), Size::new(1400.0, 900.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn toast_gap(app: &Signex) -> Option<RefusedGap> {
    let session = app.interaction_state.board_sketch.as_ref()?;
    session.toast.as_ref()?.gap
}

#[test]
fn an_open_outline_is_refused_with_a_toast_that_places_the_gap() {
    let (mut app, _tmp) = sketching();
    let line = open_outline(&mut app);
    send(&mut app, sketch(BoardSketchMsg::BakeRequested));

    assert!(
        app.interaction_state.board_sketch.is_some(),
        "the mode stays on"
    );
    assert!(
        app.document_state.error_notice.is_none(),
        "no modal card (mockup row 110)"
    );
    let gap = toast_gap(&app).expect("the toast places the gap");
    let width = gap.gap_mm.expect("the line's two ends pair across the gap");
    assert!((width - 10.0).abs() < 1e-6, "gap {width}");

    let texts = main_window_texts(&mut app);
    assert!(texts.iter().any(|t| t == TOAST), "{texts:#?}");
    let place = format!("Gap at ({:.2}, {:.2}) mm", gap.at.x, gap.at.y);
    assert!(texts.iter().any(|t| *t == place), "{place:?} in {texts:#?}");
    let id = line.to_string();
    assert!(
        !texts.iter().any(|t| t.contains(&id)),
        "no edge id on screen: {texts:#?}"
    );

    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let why = session
        .refusal
        .as_ref()
        .expect("the refusal")
        .render_en_us();
    let why = why.replace(['\u{2068}', '\u{2069}'], "");
    let said = format!(
        "The sketch was not applied to the board: the outline is not closed at ({:.2}, {:.2}) mm.",
        gap.at.x, gap.at.y
    );
    assert_eq!(why, said);
}

#[test]
fn the_place_link_frames_the_gap_until_the_canvas_takes_the_fit() {
    let (mut app, _tmp) = sketching();
    open_outline(&mut app);
    send(&mut app, sketch(BoardSketchMsg::BakeRequested));
    let gap = toast_gap(&app).expect("the gap");

    send(&mut app, sketch(BoardSketchMsg::RefusalGapClicked));
    let state = |app: &Signex| {
        let session = app.interaction_state.board_sketch.as_ref().expect("on");
        (
            session.editor.state.fit_pending,
            session.editor.state.fit_target_mm,
        )
    };
    assert_eq!(state(&app), (true, Some(gap.frame())));

    let path = app.active_tab_path().expect("the board tab");
    sketch_edit(
        &mut app,
        &path,
        FootprintEditorMsg::FitConsumed { px_per_mm: 20.0 },
    );
    assert_eq!(state(&app), (false, None), "a later Fit frames the content");
}

#[test]
fn the_close_button_dismisses_the_toast_and_keeps_the_refusal() {
    let (mut app, _tmp) = sketching();
    open_outline(&mut app);
    send(&mut app, sketch(BoardSketchMsg::BakeRequested));
    assert!(main_window_texts(&mut app).iter().any(|t| t == TOAST));

    send(&mut app, sketch(BoardSketchMsg::RefusalToastDismissed));
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    assert!(session.toast.is_none());
    assert!(session.refusal.is_some(), "kept for a held save's card");
    let texts = main_window_texts(&mut app);
    assert!(!texts.iter().any(|t| t == TOAST), "{texts:#?}");

    // With no toast the link has nothing to frame.
    send(&mut app, sketch(BoardSketchMsg::RefusalGapClicked));
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    assert_eq!(session.editor.state.fit_target_mm, None);
}

/// A refusal of a sketch whose board is not in view (leaving it to sketch
/// another board) shows no toast there: the notice card says it.
#[test]
fn a_refusal_off_view_goes_on_the_notice_card() {
    let (mut app, _tmp) = sketching();
    let session = app.interaction_state.board_sketch.as_mut().expect("on");
    session.board_path = std::path::PathBuf::from("elsewhere.snxpcb");
    app.refuse_board_sketch("the outline is gone".into());

    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    assert!(session.toast.is_none());
    let card = app
        .document_state
        .error_notice
        .as_ref()
        .expect("the card")
        .detail
        .render_en_us();
    assert!(card.contains("the outline is gone"), "{card}");
}

/// The headless path: a close by operand held by the refused bake answers
/// with the place, not the edge id.
#[test]
fn a_held_close_by_operand_names_where_the_outline_is_open() {
    use crate::app::command::{CommandArgs, CommandOutcome};
    use crate::keymap::AppCommandId;

    let (mut app, _tmp) = sketching();
    let line = open_outline(&mut app);
    let path = app.active_tab_path().expect("the board tab");
    let command = AppCommandId::new("close_active_document").expect("id");
    let operand = format!("document={}", path.display());
    let args = CommandArgs::parse_operands(&command, &[operand.as_str()]).expect("operands");
    let (outcome, _task) = app.run_operands(&command, &args);

    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("a held close reported {outcome:?}");
    };
    let reason = reason.render_en_us().replace(['\u{2068}', '\u{2069}'], "");
    assert!(
        reason.contains("the outline is not closed at ("),
        "{reason}"
    );
    assert!(!reason.contains(&line.to_string()), "no edge id: {reason}");
}

#[test]
fn the_gap_frame_shows_both_ends_and_is_never_under_20_mm() {
    let at = Point::new(5.0, -3.0);
    let wide = RefusedGap {
        at,
        gap_mm: Some(30.0),
    };
    assert_eq!(wide.frame(), (-25.0, -33.0, 35.0, 27.0));
    let lone = RefusedGap { at, gap_mm: None };
    assert_eq!(lone.frame(), (-5.0, -13.0, 15.0, 7.0));
    let label = lone.place_label().render_en_us();
    let label = label.replace(['\u{2068}', '\u{2069}'], "");
    assert_eq!(label, "Open end at (5.00, -3.00) mm");
}
