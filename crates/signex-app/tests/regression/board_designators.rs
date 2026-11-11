//! #1312 PR 1: a board footprint's designator and value edited in the
//! Properties panel, and Tools ▸ Re-Annotate Board, each driven through
//! `Signex::update` with the messages the UI sends. Every edit is one board
//! undo step; a locked footprint and an empty designator refuse with a
//! notice; a designator another footprint has is applied and warned about.

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{
    BoardReannotateMsg, CommandPaletteMsg, ContextAction, ContextMenuMsg, EditMsg, FileMsg,
    Message, ReannotateProblem, Signex, TabDocument,
};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::{FootprintEditability, PanelKind, PanelMsg, PcbFootprintTextMsg};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_engine::pcb::designators::{FootprintTextField, ReannotateOrder};
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard, Point};
use signex_types::property::PcbProperty;
use uuid::Uuid;

/// R2 at x 10, R1 at x 30, C7 at x 20 (all y 10), and U9, locked, at x 40.
const R_LEFT: Uuid = Uuid::from_u128(0x10);
const R_RIGHT: Uuid = Uuid::from_u128(0x20);
const C_MID: Uuid = Uuid::from_u128(0x30);
const LOCKED: Uuid = Uuid::from_u128(0x40);

fn pad(n: u128, x: f64) -> Pad {
    Pad {
        uuid: Uuid::from_u128(n),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(x, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.25,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn property(key: &str, value: &str) -> PcbProperty {
    PcbProperty {
        key: key.into(),
        value: value.into(),
        ..PcbProperty::default()
    }
}

/// A footprint with pads 2 mm either side of its origin, so a click on the
/// origin picks the footprint, not a pad.
fn footprint(uuid: Uuid, reference: &str, x: f64, locked: bool) -> Footprint {
    let n = uuid.as_u128();
    Footprint {
        uuid,
        reference: reference.into(),
        value: "10k".into(),
        footprint_id: "R_0603".into(),
        position: Point::new(x, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked,
        schematic_link: None,
        library_ref: None,
        pads: vec![pad(n + 1, -2.0), pad(n + 2, 2.0)],
        graphics: Vec::new(),
        properties: vec![property("Reference", reference), property("Value", "10k")],
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![
            footprint(R_LEFT, "R2", 10.0, false),
            footprint(R_RIGHT, "R1", 30.0, false),
            footprint(C_MID, "C7", 20.0, false),
            footprint(LOCKED, "U9", 40.0, true),
        ],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
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
    docs.tabs[docs.active_tab]
        .cached_document
        .as_ref()
        .and_then(TabDocument::as_pcb)
        .expect("a board is open")
        .clone()
}

fn snapshot(app: &Signex) -> String {
    serde_json::to_string(&live(app)).unwrap()
}

fn reference(app: &Signex, id: Uuid) -> String {
    live(app)
        .footprints
        .iter()
        .find(|f| f.uuid == id)
        .expect("footprint")
        .reference
        .clone()
}

fn select(app: &mut Signex, x: f64) {
    send(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(x, 10.0),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
    let selected: Vec<ItemRef> = app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert!(
        matches!(selected.as_slice(), [ItemRef::Footprint(_)]),
        "one footprint is selected: {selected:?}"
    );
}

fn panel(msg: PcbFootprintTextMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::PcbFootprintText(msg)))
}

/// Type `text` into `field` of `footprint`, then press Enter.
fn type_and_enter(f: &mut Fixture, footprint: Uuid, field: FootprintTextField, text: &str) {
    let board = f.path.clone();
    send(
        &mut f.app,
        panel(PcbFootprintTextMsg::Typed {
            board: board.clone(),
            footprint,
            field,
            text: text.into(),
        }),
    );
    send(
        &mut f.app,
        panel(PcbFootprintTextMsg::Submitted {
            board,
            footprint,
            field,
        }),
    );
}

fn undo(app: &mut Signex) {
    send(app, Message::Edit(EditMsg::Undo));
}

fn redo(app: &mut Signex) {
    send(app, Message::Edit(EditMsg::Redo));
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

/// A notice logged after `since` that contains `wanted`.
fn noticed(since: u64, wanted: &str) -> bool {
    diagnostics::recent_entries()
        .iter()
        .any(|e| e.id > since && e.message.contains(wanted))
}

/// The edit `run` makes is one undo step: undo restores the board, redo
/// replays it, and a second undo has nothing of it left.
fn assert_one_step(f: &mut Fixture, run: impl FnOnce(&mut Fixture)) -> PcbBoard {
    let before = snapshot(&f.app);
    run(f);
    let after = snapshot(&f.app);
    assert_ne!(before, after, "the board did not change");
    let edited = live(&f.app);
    undo(&mut f.app);
    assert_eq!(snapshot(&f.app), before, "one undo restores the board");
    redo(&mut f.app);
    assert_eq!(snapshot(&f.app), after, "redo replays it");
    undo(&mut f.app);
    undo(&mut f.app);
    assert_eq!(snapshot(&f.app), before, "it was a single step");
    edited
}

#[test]
fn one_selected_footprint_shows_its_designator_and_value_in_properties() {
    let mut f = open_board();
    select(&mut f.app, 10.0);
    let facts = f
        .app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.footprint.clone())
        .expect("the footprint section");
    assert_eq!(facts.uuid, R_LEFT);
    assert_eq!(
        (facts.designator.as_str(), facts.value.as_str()),
        ("R2", "10k")
    );
    assert_eq!(facts.footprint_name, "R_0603");
    assert_eq!(facts.editability, FootprintEditability::Editable);
    assert_eq!(facts.also_on, 0);
}

#[test]
fn a_typed_designator_is_a_draft_until_enter_then_one_undo_step() {
    let mut f = open_board();
    select(&mut f.app, 10.0);
    let board = f.path.clone();
    send(
        &mut f.app,
        panel(PcbFootprintTextMsg::Typed {
            board,
            footprint: R_LEFT,
            field: FootprintTextField::Designator,
            text: "R5".into(),
        }),
    );
    assert_eq!(
        reference(&f.app, R_LEFT),
        "R2",
        "typing alone edits nothing"
    );
    let draft = f
        .app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.footprint.as_ref())
        .and_then(|fp| fp.draft.clone());
    assert_eq!(
        draft,
        Some((FootprintTextField::Designator, "R5".to_string()))
    );
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            panel(PcbFootprintTextMsg::Submitted {
                board: f.path.clone(),
                footprint: R_LEFT,
                field: FootprintTextField::Designator,
            }),
        );
    });
    let fp = edited
        .footprints
        .iter()
        .find(|fp| fp.uuid == R_LEFT)
        .unwrap();
    assert_eq!(fp.reference, "R5");
    assert_eq!(fp.value, "10k");
    assert_eq!(fp.properties[0].value, "R5", "the Reference text follows");
}

#[test]
fn a_value_edit_is_one_undo_step_and_keeps_the_designator() {
    let mut f = open_board();
    select(&mut f.app, 10.0);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, R_LEFT, FootprintTextField::Value, "4k7");
    });
    let fp = edited
        .footprints
        .iter()
        .find(|fp| fp.uuid == R_LEFT)
        .unwrap();
    assert_eq!((fp.reference.as_str(), fp.value.as_str()), ("R2", "4k7"));
    assert_eq!(fp.properties[1].value, "4k7");
}

#[test]
fn a_designator_another_footprint_has_is_applied_and_warned_about() {
    let mut f = open_board();
    select(&mut f.app, 10.0);
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    type_and_enter(&mut f, R_LEFT, FootprintTextField::Designator, "R1");
    assert_eq!(reference(&f.app, R_LEFT), "R1");
    assert!(
        noticed(since, "Designator: R1 is also on 1 other footprint"),
        "the Messages panel says R1 is shared"
    );
    let facts = f
        .app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.footprint.clone())
        .unwrap();
    assert_eq!(facts.also_on, 1, "Properties warns about the shared R1");
}

#[test]
fn a_locked_footprint_and_an_empty_designator_refuse_with_a_notice() {
    let mut f = open_board();
    select(&mut f.app, 40.0);
    let facts = f
        .app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.footprint.clone())
        .unwrap();
    assert_eq!(facts.editability, FootprintEditability::Locked);
    let before = snapshot(&f.app);
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    type_and_enter(&mut f, LOCKED, FootprintTextField::Value, "x");
    assert_eq!(snapshot(&f.app), before);
    assert!(noticed(since, "Value: U9 is locked"));

    select(&mut f.app, 10.0);
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    type_and_enter(&mut f, R_LEFT, FootprintTextField::Designator, "  ");
    assert_eq!(snapshot(&f.app), before);
    assert!(noticed(since, "Designator: a designator cannot be empty"));
}

#[test]
fn the_board_menu_properties_row_runs_on_a_board_selection() {
    let mut f = open_board();
    select(&mut f.app, 10.0);
    f.app.document_state.dock.close_panel(PanelKind::Properties);
    assert!(
        f.app
            .document_state
            .dock
            .locate(PanelKind::Properties)
            .is_none()
    );
    send(
        &mut f.app,
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
            id: command("edit_selected_object_properties"),
            project: None,
        })),
    );
    assert!(
        f.app
            .document_state
            .dock
            .locate(PanelKind::Properties)
            .is_some(),
        "the Properties panel is shown"
    );
}

fn open_dialog(f: &mut Fixture, message: Message) {
    send(&mut f.app, message);
    assert!(
        f.app.ui_state.board_reannotate.is_some(),
        "the Re-Annotate dialog opened"
    );
}

fn renames(f: &Fixture) -> Vec<(String, String)> {
    let state = f.app.ui_state.board_reannotate.as_ref().unwrap();
    state
        .plan
        .renames
        .iter()
        .map(|r| (r.from.clone(), r.to.clone()))
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

fn reannotate(msg: BoardReannotateMsg) -> Message {
    Message::from(msg)
}

#[test]
fn the_tools_menu_row_re_annotates_by_position_as_one_undo_step() {
    let mut f = open_board();
    open_dialog(
        &mut f,
        Message::Menu(MenuMessage::Command(command("reannotate_board"))),
    );
    assert_eq!(
        renames(&f),
        pairs(&[("R2", "R1"), ("C7", "C1"), ("R1", "R2")]),
        "the locked U9 is kept"
    );
    let edited = assert_one_step(&mut f, |f| {
        send(&mut f.app, reannotate(BoardReannotateMsg::ExecuteRequested));
    });
    assert!(
        f.app.ui_state.board_reannotate.is_none(),
        "Execute closes it"
    );
    let refs: Vec<&str> = edited
        .footprints
        .iter()
        .map(|fp| fp.reference.as_str())
        .collect();
    assert_eq!(refs, vec!["R1", "R2", "C1", "U9"]);
    assert!(edited.footprints.iter().all(|fp| fp.value == "10k"));
}

#[test]
fn the_palette_opens_the_dialog_and_the_options_remake_the_list() {
    let mut f = open_board();
    send(&mut f.app, Message::CommandPalette(CommandPaletteMsg::Open));
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged("Re-Annotate Board".into())),
    );
    open_dialog(
        &mut f,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );
    send(
        &mut f.app,
        reannotate(BoardReannotateMsg::OrderPicked(
            ReannotateOrder::RowsThenColumns,
        )),
    );
    let state = f.app.ui_state.board_reannotate.as_ref().unwrap();
    assert_eq!(state.options.order, ReannotateOrder::RowsThenColumns);
    // One row: the same walk as by columns.
    assert_eq!(
        renames(&f),
        pairs(&[("R2", "R1"), ("C7", "C1"), ("R1", "R2")])
    );
    let before = snapshot(&f.app);
    send(&mut f.app, reannotate(BoardReannotateMsg::CancelRequested));
    assert!(f.app.ui_state.board_reannotate.is_none());
    assert_eq!(snapshot(&f.app), before, "Cancel changes nothing");
    assert_eq!(
        f.app.ui_state.board_reannotate_options.order,
        ReannotateOrder::RowsThenColumns,
        "the next opening keeps the order"
    );
}

#[test]
fn a_board_changed_under_the_dialog_gets_a_new_list_before_it_runs() {
    let mut f = open_board();
    select(&mut f.app, 20.0);
    open_dialog(
        &mut f,
        Message::Menu(MenuMessage::Command(command("reannotate_board"))),
    );
    // The board moves on while the dialog is open (an undocked window).
    type_and_enter(&mut f, C_MID, FootprintTextField::Designator, "C3");
    let before = snapshot(&f.app);
    send(&mut f.app, reannotate(BoardReannotateMsg::ExecuteRequested));
    assert_eq!(snapshot(&f.app), before, "the stale list did not run");
    let state = f
        .app
        .ui_state
        .board_reannotate
        .as_ref()
        .expect("still open");
    assert_eq!(state.problem, Some(ReannotateProblem::Stale));
    assert_eq!(
        renames(&f),
        pairs(&[("R2", "R1"), ("C3", "C1"), ("R1", "R2")])
    );
    send(&mut f.app, reannotate(BoardReannotateMsg::ExecuteRequested));
    assert_eq!(reference(&f.app, C_MID), "C1");
}

#[test]
fn selecting_another_footprint_drops_the_unapplied_text() {
    let mut f = open_board();
    select(&mut f.app, 10.0);
    send(
        &mut f.app,
        panel(PcbFootprintTextMsg::Typed {
            board: f.path.clone(),
            footprint: R_LEFT,
            field: FootprintTextField::Value,
            text: "1M".into(),
        }),
    );
    assert!(f.app.interaction_state.pcb_footprint_draft.is_some());
    select(&mut f.app, 30.0);
    assert!(f.app.interaction_state.pcb_footprint_draft.is_none());
    // A late Enter for the first footprint finds no draft and edits nothing.
    let before = snapshot(&f.app);
    send(
        &mut f.app,
        panel(PcbFootprintTextMsg::Submitted {
            board: f.path.clone(),
            footprint: R_LEFT,
            field: FootprintTextField::Value,
        }),
    );
    assert_eq!(snapshot(&f.app), before);
}

#[test]
fn re_annotate_does_nothing_without_a_board() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    send(
        &mut app,
        Message::Menu(MenuMessage::Command(command("reannotate_board"))),
    );
    assert!(app.ui_state.board_reannotate.is_none());
}
