//! #2868: a placed dimension's Arrows, Text, Extension Lines, and Units and
//! Format rows, driven through `Signex::update` with the messages the panel
//! sends. Each pick or Enter is one undo step, and the board saves and
//! reads the new look back.

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{EditMsg, FileMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::panels::pcb_primitive_properties::{
    PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget,
};
use signex_app::panels::{PanelMsg, PcbPrimitiveMsg};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_app::test_adapter::BoardEngines;
use signex_engine::pcb::primitive_edit::DimensionStyleEdit;
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_dimension_style::{ArrowStyle, BoardFont, DimensionTextPosition};
use uuid::Uuid;

const DIM: Uuid = Uuid::from_u128(0x504);

/// A 10 mm dimension along y = 10, standing 2 mm above it.
fn board() -> PcbBoard {
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.0, "trace_width": 0.3}
    }))
    .unwrap();
    board.free.dimensions.push(LinearDimension::new(
        DIM,
        Point::new(30.0, 10.0),
        Point::new(40.0, 10.0),
        2.0,
    ));
    board
}

struct Fixture {
    app: Signex,
    path: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

fn open_board() -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    crate::drive::drive(&mut app, task);
    Fixture {
        app,
        path,
        _tmp: tmp,
    }
}

fn send(f: &mut Fixture, message: Message) {
    let task = f.app.update(message);
    crate::drive::drive(&mut f.app, task);
}

fn panel(msg: PcbPrimitiveMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(msg)))
}

fn live(f: &Fixture) -> PcbBoard {
    let docs = &f.app.document_state;
    docs.tab_board_engine(docs.active_tab)
        .map(|engine| engine.board())
        .expect("a board is open")
        .clone()
}

fn snapshot(f: &Fixture) -> String {
    serde_json::to_string(&live(f)).unwrap()
}

/// Selects the dimension with a click on its first extension line.
fn picked(f: &mut Fixture) -> PrimitiveTarget {
    let drawing = board().free.dimensions[0].drawing().expect("a drawing");
    let (a, b) = drawing.strokes[0];
    send(
        f,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
    let facts = f
        .app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.primitive.clone());
    let Some(PcbPrimitiveFacts::Dimension(dimension)) = facts else {
        panic!("the dimension panel shows");
    };
    assert_eq!(dimension.common.target.items, vec![ItemRef::Dimension(DIM)]);
    dimension.common.target
}

/// `run` is one undo step: one undo restores the board, redo replays it.
fn assert_one_step(f: &mut Fixture, run: impl FnOnce(&mut Fixture)) -> LinearDimension {
    let before = snapshot(f);
    run(f);
    let after = snapshot(f);
    assert_ne!(before, after, "the board did not change");
    let edited = live(f).free.dimensions[0].clone();
    send(f, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(f), before, "one undo restores the board");
    send(f, Message::Edit(EditMsg::Redo));
    assert_eq!(snapshot(f), after, "redo replays it");
    edited
}

fn picks(target: &PrimitiveTarget, edit: DimensionStyleEdit) -> Message {
    panel(PcbPrimitiveMsg::DimensionStylePicked {
        target: target.clone(),
        edit,
    })
}

fn typed_and_entered(f: &mut Fixture, target: &PrimitiveTarget, field: PrimitiveField, text: &str) {
    send(
        f,
        panel(PcbPrimitiveMsg::FieldTyped {
            target: target.clone(),
            field,
            text: text.into(),
        }),
    );
    send(
        f,
        panel(PcbPrimitiveMsg::FieldSubmitted {
            target: target.clone(),
            field,
        }),
    );
}

#[test]
fn each_pick_is_one_undo_step() {
    let mut f = open_board();
    let target = picked(&mut f);
    let cases = [
        DimensionStyleEdit::ArrowStyle(ArrowStyle::Filled),
        DimensionStyleEdit::Font(BoardFont::Sans),
        DimensionStyleEdit::Bold(true),
        DimensionStyleEdit::Italic(true),
        DimensionStyleEdit::TextPosition(DimensionTextPosition::Inline),
        DimensionStyleEdit::Unit(signex_types::pcb_dimension_style::DimensionUnits::In),
        DimensionStyleEdit::ShowUnit(false),
        DimensionStyleEdit::TrailingZeros(false),
    ];
    for edit in cases {
        let message = picks(&target, edit.clone());
        let edited = assert_one_step(&mut f, |f| send(f, message));
        assert_ne!(
            edited.style,
            board().free.dimensions[0].style,
            "{edit:?} changed the look"
        );
        send(&mut f, Message::Edit(EditMsg::Undo));
    }
}

#[test]
fn each_typed_field_is_one_undo_step() {
    let mut f = open_board();
    let target = picked(&mut f);
    let cases = [
        (PrimitiveField::ArrowLength, "2"),
        (PrimitiveField::ArrowWidth, "0.8"),
        (PrimitiveField::TextWidth, "0.9"),
        (PrimitiveField::TextGap, "1"),
        (PrimitiveField::ExtensionOffset, "0.5"),
        (PrimitiveField::ExtensionOvershoot, "1.5"),
        (PrimitiveField::ExtensionWidth, "0.3"),
        (PrimitiveField::Prefix, "W="),
        (PrimitiveField::Suffix, " max"),
    ];
    for (field, text) in cases {
        let target = target.clone();
        assert_one_step(&mut f, |f| typed_and_entered(f, &target, field, text));
        send(&mut f, Message::Edit(EditMsg::Undo));
    }
}

#[test]
fn the_new_look_is_saved_and_read_back() {
    let mut f = open_board();
    let target = picked(&mut f);
    send(
        &mut f,
        picks(&target, DimensionStyleEdit::ArrowStyle(ArrowStyle::Filled)),
    );
    typed_and_entered(&mut f, &target, PrimitiveField::Suffix, " ref");
    send(&mut f, Message::File(FileMsg::Save));
    let saved = SnxPcb::parse(&std::fs::read_to_string(&f.path).unwrap())
        .unwrap()
        .board;
    let dimension = &saved.free.dimensions[0];
    assert_eq!(dimension.style.arrow_style, ArrowStyle::Filled);
    assert_eq!(dimension.value_text(), "10.00mm ref");
    assert_eq!(dimension, &live(&f).free.dimensions[0]);
}
