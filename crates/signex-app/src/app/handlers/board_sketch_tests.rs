//! #2356: board sketch mode through `Signex::update`. The canvas cursor
//! follows the pointer, and Properties shows the board, not footprint fields.

use iced::{Point, Size, Widget as _};
use signex_types::format::SnxPcb;
use signex_types::pcb::Point as BoardPoint;

use crate::app::{BoardSketchMsg, FileMsg, Message, PcbMsg, Signex};
use crate::layout_probe;

const WINDOW: Size = Size::new(1200.0, 800.0);

/// Run `task` to completion, feeding every message back through `update`.
fn drive(app: &mut Signex, task: iced::Task<Message>) {
    use futures::StreamExt as _;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let messages = runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    });
    for message in messages {
        let follow_up = app.update(message);
        drive(app, follow_up);
    }
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    drive(app, task);
}

/// A two-layer board with a 40 x 25 mm outline, open and in sketch mode.
fn sketching() -> (Signex, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("temp dir");
    let path = tmp.path().join("rover.snxpcb");
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.outline = [(0.0, 0.0), (40.0, 0.0), (40.0, 25.0), (0.0, 25.0)]
        .into_iter()
        .map(|(x, y)| BoardPoint::new(x, y))
        .collect();
    let text = SnxPcb::new(board).write_string().expect("encode");
    std::fs::write(&path, text).expect("write board");
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    send(&mut app, Message::File(FileMsg::Opened(Some(path))));
    send(
        &mut app,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::ModeToggled)),
    );
    assert!(
        app.interaction_state.board_sketch.is_some(),
        "sketch mode is on"
    );
    (app, tmp)
}

fn cursor_mm(app: &Signex) -> Option<(f64, f64)> {
    let session = app.interaction_state.board_sketch.as_ref()?;
    session.editor.state.cursor_mm
}

/// The pointer moving over the board sketch canvas moves its cursor: each
/// CursorMoved the canvas sees reaches the sketch editor's state, which
/// the snapped crosshair is drawn from.
#[test]
fn the_canvas_cursor_follows_the_pointer() {
    let (mut app, _tmp) = sketching();
    let moves = [
        Point::new(500.0, 300.0),
        Point::new(510.0, 310.0),
        Point::new(700.0, 450.0),
    ];
    let messages = {
        let session = app.interaction_state.board_sketch.as_ref().expect("on");
        let view = crate::library::editor::standalone::view_board_sketch(
            &session.editor,
            &app.document_state.panel_ctx.tokens,
            &app.ui_state.i18n,
        )
        .map(Message::Library)
        .boxed();
        layout_probe::messages_for_moves(view, WINDOW, &moves)
    };
    let mut seen: Vec<(f64, f64)> = Vec::new();
    for message in messages {
        send(&mut app, message);
        if let Some(at) = cursor_mm(&app)
            && seen.last() != Some(&at)
        {
            seen.push(at);
        }
    }
    let (Some(first), Some(last)) = (seen.first(), seen.last()) else {
        panic!("no cursor position reached the sketch editor");
    };
    assert!(
        seen.len() >= 2,
        "the cursor moved with the pointer: {seen:?}"
    );
    assert!(
        last.0 > first.0 && last.1 > first.1,
        "right and down on screen is right and down on the board: {seen:?}"
    );
}

/// Properties shows the board's name and outline, not the footprint fields
/// of the editor the sketch is hosted on.
#[test]
fn properties_show_the_board_not_footprint_fields() {
    let (app, _tmp) = sketching();
    let ctx = &app.document_state.panel_ctx;
    assert!(
        ctx.footprint_editor.is_some(),
        "the sketch drives Properties"
    );
    let view = crate::panels::view_panel(crate::panels::PanelKind::Properties, ctx);
    let texts: Vec<String> = layout_probe::lay_out_texts(view, Size::new(360.0, 6000.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    let shows = |label: &str| texts.iter().any(|t| t == label);
    for footprint_only in [
        "Default Designator",
        "Description",
        "Height (mm)",
        "Version",
        "Pads",
    ] {
        assert!(!shows(footprint_only), "{footprint_only:?} in {texts:?}");
    }
    for board_row in ["rover", "Size", "40.00 × 25.00 mm", "Area", "1000.00 mm²"] {
        assert!(shows(board_row), "{board_row:?} missing from {texts:?}");
    }
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

/// #2059: a save held by a refused bake shows the refusal on the notice
/// card as a message, rendered when the card is drawn, so a language switch
/// re-renders it with the card.
#[test]
fn the_held_save_card_renders_its_refusal_in_the_view() {
    use crate::library::editor::footprint::SketchTool;
    use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use crate::panels::{BoardSketchAttrMsg, PanelMsg};

    let (mut app, _tmp) = sketching();
    let path = app.active_tab_path().expect("the board tab");
    let edit = |app: &mut Signex, msg| {
        let path = path.clone();
        send(
            app,
            Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Footprint(msg),
            }),
        );
    };
    edit(
        &mut app,
        FootprintEditorMsg::SketchSetTool(SketchTool::Line),
    );
    for x_mm in [60.0, 70.0] {
        edit(
            &mut app,
            FootprintEditorMsg::SketchToolClick {
                x_mm,
                y_mm: 60.0,
                snap_id: None,
            },
        );
    }
    edit(&mut app, FootprintEditorMsg::ToolEscape);
    let line = {
        let session = app.interaction_state.board_sketch.as_ref().expect("on");
        let sketch = session.working_sketch();
        sketch.entities.last().expect("the line").id
    };
    send(
        &mut app,
        Message::Dock(crate::dock::DockMessage::Panel(PanelMsg::BoardSketch(
            BoardSketchAttrMsg::OutlineToggled { id: line, on: true },
        ))),
    );
    app.document_state.error_notice = None;
    send(&mut app, Message::File(FileMsg::Save));

    const REFUSED: &str = "The sketch was not applied to the board";
    let en = main_window_texts(&mut app);
    assert!(
        en.iter()
            .any(|t| t.contains("could not be applied") && t.contains(REFUSED)),
        "the card names the hold and the refusal: {en:#?}"
    );
    app.ui_state.i18n = std::sync::Arc::new(signex_i18n::I18n::new(
        &signex_i18n::LanguagePref::Locale("en-XA".into()),
    ));
    let xa = main_window_texts(&mut app);
    assert!(
        !xa.iter().any(|t| t.contains(REFUSED)),
        "the refusal follows the language switch: {xa:#?}"
    );
    let refusal = app
        .interaction_state
        .board_sketch
        .as_ref()
        .and_then(|session| session.refusal.as_ref())
        .expect("the refusal")
        .render(&*app.ui_state.i18n);
    let card = app
        .document_state
        .error_notice
        .as_ref()
        .expect("the card")
        .detail
        .render(&*app.ui_state.i18n);
    assert!(card.contains(&refusal), "{card:?} carries {refusal:?}");
    assert!(
        xa.contains(&card),
        "the card still shows, in en-XA ({card:?}): {xa:#?}"
    );
}

/// #2518: the board's mode bar is the shared exit set, Bake preview ·
/// Discard changes · Bake; Leave and Apply are gone. With edits, Discard
/// changes turns the bar into its question.
#[test]
fn the_mode_bar_is_the_shared_exit_set() {
    let (mut app, _tmp) = sketching();
    let texts = main_window_texts(&mut app);
    for label in ["Board Sketch", "Bake preview", "Discard changes", "Bake"] {
        assert!(texts.iter().any(|t| t == label), "{label:?} in {texts:#?}");
    }
    // The bar's old Apply / Leave / Discard buttons.
    for gone in ["Leave", "Discard"] {
        assert!(!texts.iter().any(|t| t == gone), "{gone:?} in {texts:#?}");
    }

    let session = app.interaction_state.board_sketch.as_mut().expect("on");
    session.editor.state.sketch_exit.confirming_discard = true;
    let texts = main_window_texts(&mut app);
    assert!(
        texts
            .iter()
            .any(|t| t == "Discard the changes made in this sketch?"),
        "{texts:#?}"
    );
    assert!(texts.iter().any(|t| t == "Keep editing"), "{texts:#?}");
}

/// #2518: Esc never applies the sketch. Without edits it leaves; it never
/// commits a board command.
#[test]
fn esc_without_edits_leaves_and_writes_nothing() {
    let (mut app, _tmp) = sketching();
    let message = app.board_sketch_escape().expect("the mode is in view");
    assert!(
        matches!(
            message,
            Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::DiscardRequested))
        ),
        "{message:?}"
    );
    send(&mut app, message);
    assert!(app.interaction_state.board_sketch.is_none());
}

/// #2518: the mode toggle while "Discard changes?" is open answers Keep
/// editing, as Esc does: the mode stays on and nothing is applied.
#[test]
fn the_toggle_answers_an_open_discard_question_with_keep_editing() {
    let (mut app, _tmp) = sketching();
    let session = app.interaction_state.board_sketch.as_mut().expect("on");
    session.editor.state.sketch_exit.confirming_discard = true;
    send(
        &mut app,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::ModeToggled)),
    );
    let session = app
        .interaction_state
        .board_sketch
        .as_ref()
        .expect("the mode stays on");
    assert!(!session.editor.state.sketch_exit.confirming_discard);
}

/// #2518: a board role picked for a shape it cannot take (a V-score on a
/// point) leaves the role as it was and Messages says why.
#[test]
fn a_role_on_the_wrong_shape_is_refused_with_a_message() {
    use crate::library::editor::footprint::apply_footprint_primitive_edit;
    use crate::library::messages::{FootprintEditorMsg, RoleTag};
    let (mut app, _tmp) = sketching();
    let session = app.interaction_state.board_sketch.as_mut().expect("on");
    apply_footprint_primitive_edit(
        &mut session.editor,
        FootprintEditorMsg::SketchPlacePoint {
            x_mm: 5.0,
            y_mm: 5.0,
        },
    );
    let point = session
        .editor
        .primitive()
        .sketch
        .as_ref()
        .and_then(|s| {
            s.entities
                .iter()
                .rev()
                .find(|e| matches!(e.kind, signex_sketch::entity::EntityKind::Point { .. }))
        })
        .expect("the point")
        .clone();
    let mark = crate::diagnostics::newest_entry_id();
    send(
        &mut app,
        Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::FpEditorSetRole {
                id: point.id,
                role: RoleTag::VScore,
            },
        )),
    );
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let after = session
        .editor
        .primitive()
        .sketch
        .as_ref()
        .and_then(|s| s.entities.iter().find(|e| e.id == point.id))
        .expect("the point")
        .clone();
    assert_eq!(after, point, "the point keeps its role");
    let said = crate::diagnostics::entries_since(mark)
        .entries
        .iter()
        .any(|e| e.message.contains("V-score was not set"));
    assert!(said, "Messages says why");
}
