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
        .map(Message::Library);
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
