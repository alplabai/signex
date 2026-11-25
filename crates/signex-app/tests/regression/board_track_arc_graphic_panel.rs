//! #2523 (mockup rows 95, 98, 102): the track, arc and drawing panels, driven through
//! `Signex::update` with the messages the panels send. A hovered row lights
//! its illustration callout and a hovered callout its row; the Ends row
//! keeps Round with Square and Flat greyed; the Arc form's Placement pick
//! sets how the armed tool draws; a picked value is one undo step; a
//! selection partly locked counts its locks for the header icon; a picked
//! location moves several tracks at once.

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{EditMsg, FileMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::pcb_primitive_properties::{
    Editability, PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget, end_choices, ends_tip,
};
use signex_app::panels::{PanelMsg, PcbPrimitiveMsg};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_engine::pcb::draw::ArcMode;
use signex_i18n::ids;
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point};
use signex_widgets::property_illustration::{Cap, FieldId, IllustrationMsg};
use uuid::Uuid;

const ARC: Uuid = Uuid::from_u128(0x501);
const LOCKED: Uuid = Uuid::from_u128(0x502);
const FREE: Uuid = Uuid::from_u128(0x503);
const FREE_LOW: Uuid = Uuid::from_u128(0x504);
/// The key the Width row and its callout share.
const WIDTH: FieldId = FieldId::new("w");

/// A half circle of radius 5 round (20, 20) on top copper; on net 3 a
/// locked track along y = 0 and free ones along y = 10 and y = -10.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.0, "trace_width": 0.3},
        "nets": [{"number": 3, "name": "GND"}],
        "segments": [
            {"uuid": LOCKED, "start": {"x": 0.0, "y": 0.0}, "end": {"x": 10.0, "y": 0.0},
             "width": 0.25, "layer": "TopCopper", "net": 3, "locked": true},
            {"uuid": FREE, "start": {"x": 0.0, "y": 10.0}, "end": {"x": 10.0, "y": 10.0},
             "width": 0.25, "layer": "TopCopper", "net": 3},
            {"uuid": FREE_LOW, "start": {"x": 0.0, "y": -10.0}, "end": {"x": 10.0, "y": -10.0},
             "width": 0.25, "layer": "TopCopper", "net": 3}
        ],
        "free": {
            "arcs": [{"uuid": ARC, "start": {"x": 25.0, "y": 20.0},
                      "mid": {"x": 20.0, "y": 25.0}, "end": {"x": 15.0, "y": 20.0},
                      "width": 0.3, "layer": "TopCopper", "net": 0}]
        }
    }))
    .unwrap()
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

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

fn live(app: &Signex) -> PcbBoard {
    let docs = &app.document_state;
    docs.tab_board_engine(docs.active_tab)
        .map(|engine| engine.board())
        .expect("a board is open")
        .clone()
}

fn snapshot(app: &Signex) -> String {
    serde_json::to_string(&live(app)).unwrap()
}

fn click(app: &mut Signex, at: Point, mode: ClickMode) {
    send(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at,
            tolerance_mm: 0.05,
            mode,
        }),
    );
}

fn facts(app: &Signex) -> PcbPrimitiveFacts {
    app.document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.primitive.clone())
        .expect("the primitive panel")
}

fn panel(msg: PcbPrimitiveMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(msg)))
}

fn hover(app: &mut Signex, msg: IllustrationMsg) {
    send(app, panel(PcbPrimitiveMsg::IllustrationHovered(msg)));
}

fn lit(app: &Signex, field: &FieldId) -> bool {
    facts(app).common().hover.is_field_highlighted(field)
}

#[test]
fn hovering_a_row_lights_its_callout() {
    let mut f = open_board();
    click(&mut f.app, Point::new(5.0, 10.0), ClickMode::Replace);
    assert!(matches!(facts(&f.app), PcbPrimitiveFacts::Track(_)));
    assert!(!lit(&f.app, &WIDTH), "nothing is hovered yet");

    hover(&mut f.app, IllustrationMsg::FieldHovered(WIDTH));
    assert!(lit(&f.app, &WIDTH), "the Width row lights its callout");
    hover(&mut f.app, IllustrationMsg::FieldLeft(WIDTH));
    assert!(!lit(&f.app, &WIDTH), "leaving the row puts it out");
}

#[test]
fn the_ends_row_keeps_round_and_greys_square_and_flat() {
    let choices = end_choices();
    assert_eq!(choices.map(|c| c.cap), [Cap::Round, Cap::Square, Cap::Flat]);
    assert!(choices[0].is_on && choices[0].is_enabled, "Round is chosen");
    for greyed in &choices[1..] {
        assert!(!greyed.is_on && !greyed.is_enabled, "{:?}", greyed.cap);
    }
    assert_eq!(ends_tip(true), ids::PCB_PRIM_ENDS_COPPER_TIP);
    assert_eq!(ends_tip(false), ids::PCB_PRIM_ENDS_UNSTORED_TIP);
}

#[test]
fn the_arc_placement_pick_sets_how_the_armed_tool_draws() {
    let mut f = open_board();
    let id = AppCommandId::new("place_board_arc").expect("a catalog id");
    send(&mut f.app, Message::Menu(MenuMessage::Command(id)));
    let PcbPrimitiveFacts::Arc(form) = facts(&f.app) else {
        panic!("arming Arc shows the arc form");
    };
    assert_eq!(form.mode, Some(ArcMode::Centre), "by centre by default");

    send(
        &mut f.app,
        panel(PcbPrimitiveMsg::ArcModePicked(ArcMode::Edge)),
    );
    assert_eq!(
        f.app.interaction_state.pcb_tool_defaults.arc_mode,
        ArcMode::Edge
    );
    let PcbPrimitiveFacts::Arc(form) = facts(&f.app) else {
        panic!("the arc form stays");
    };
    assert_eq!(form.mode, Some(ArcMode::Edge), "the form shows the pick");
    assert!(form.common.target.is_placement(), "the tool stays armed");

    let before = live(&f.app).free.arcs.len();
    click(&mut f.app, Point::new(40.0, 40.0), ClickMode::Replace);
    click(&mut f.app, Point::new(44.0, 40.0), ClickMode::Replace);
    assert_eq!(
        live(&f.app).free.arcs.len(),
        before + 1,
        "two clicks make an arc by edge"
    );
}

#[test]
fn a_picked_value_is_one_undo_step() {
    let mut f = open_board();
    click(&mut f.app, Point::new(25.0, 20.0), ClickMode::Replace);
    let target = PrimitiveTarget {
        board: f.path.clone(),
        items: vec![ItemRef::Arc(ARC)],
    };
    assert_eq!(facts(&f.app).common().target, target);
    let before = snapshot(&f.app);
    send(
        &mut f.app,
        panel(PcbPrimitiveMsg::ValuePicked {
            target,
            field: PrimitiveField::Width,
            value: 0.5,
        }),
    );
    let after = snapshot(&f.app);
    assert_ne!(before, after, "the pick changed the arc");
    assert_eq!(live(&f.app).free.arcs[0].width, 0.5);
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), before, "one undo restores the arc");
    send(&mut f.app, Message::Edit(EditMsg::Redo));
    assert_eq!(snapshot(&f.app), after, "redo replays it");
}

#[test]
fn a_partly_locked_selection_counts_its_locks_for_the_header() {
    let mut f = open_board();
    click(&mut f.app, Point::new(5.0, 0.0), ClickMode::Replace);
    let PcbPrimitiveFacts::Track(one) = facts(&f.app) else {
        panic!("the track panel shows");
    };
    assert_eq!(one.locked, Some(true));
    assert_eq!(one.common.locked_count, None, "all of one is locked");

    click(&mut f.app, Point::new(5.0, 10.0), ClickMode::Toggle);
    let PcbPrimitiveFacts::Track(both) = facts(&f.app) else {
        panic!("the track panel shows");
    };
    assert_eq!(both.common.target.items.len(), 2);
    assert_eq!(both.locked, None, "the lock is mixed");
    assert_eq!(both.common.locked_count, Some((1, 2)));
    assert_eq!(both.common.editability, Editability::ItemLocked);
}

#[test]
fn the_position_row_moves_several_tracks_at_once() {
    let mut f = open_board();
    click(&mut f.app, Point::new(5.0, 10.0), ClickMode::Replace);
    click(&mut f.app, Point::new(5.0, -10.0), ClickMode::Toggle);
    let PcbPrimitiveFacts::Track(both) = facts(&f.app) else {
        panic!("the track panel shows");
    };
    assert_eq!(both.common.target.items.len(), 2);
    assert_eq!(both.start, (Some(0.0), None), "Start reads Mixed in Y");
    let (x, _) = both.location.expect("the selection has a corner");
    let start_x = |app: &Signex, id: Uuid| {
        live(app)
            .segments
            .iter()
            .find(|s| s.uuid == id)
            .map(|s| s.start.x)
            .expect("the track is on the board")
    };
    let before = [LOCKED, FREE, FREE_LOW].map(|id| start_x(&f.app, id));
    send(
        &mut f.app,
        panel(PcbPrimitiveMsg::ValuePicked {
            target: both.common.target.clone(),
            field: PrimitiveField::X,
            value: x + 3.0,
        }),
    );
    for (id, was, moved) in [
        (LOCKED, before[0], 0.0),
        (FREE, before[1], 3.0),
        (FREE_LOW, before[2], 3.0),
    ] {
        let now = start_x(&f.app, id);
        assert!((now - (was + moved)).abs() < 1e-9, "{id} moved by {moved}");
    }
}
