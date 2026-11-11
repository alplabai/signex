//! #1312 PR 1: a board footprint's designator and value edited in the
//! Properties panel, and Tools ▸ Re-Annotate Board, each driven through
//! `Signex::update` with the messages the UI sends. Every edit is one board
//! undo step; a locked footprint and an empty designator refuse with a
//! notice; a designator another footprint has is applied and warned about.

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{
    BoardReannotateMsg, CommandPaletteMsg, ContextAction, ContextMenuMsg, EditMsg, FileMsg,
    Message, PcbMsg, ReannotateProblem, Signex, TabDocument, WindowMsg,
};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::{FootprintEditability, PanelKind, PanelMsg, PcbFootprintTextMsg};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_engine::pcb::designators::{
    BottomSideOrder, FootprintTextField, ReannotateOptions, ReannotateOrder, plan_reannotate,
};
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard, Point, SymbolLink};
use signex_types::property::PcbProperty;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

/// R2 at (10, 10), C7 at (20, 10), R1 at (30, 5) and U9, locked, at (40, 10).
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
fn footprint(uuid: Uuid, reference: &str, (x, y): (f64, f64), locked: bool) -> Footprint {
    let n = uuid.as_u128();
    Footprint {
        uuid,
        reference: reference.into(),
        value: "10k".into(),
        footprint_id: "R_0603".into(),
        position: Point::new(x, y),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked,
        schematic_link: Some(SymbolLink::new(
            InstancePath::root(),
            [Uuid::from_u128(n + 0x1000)],
        )),
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
            footprint(R_LEFT, "R2", (10.0, 10.0), false),
            footprint(R_RIGHT, "R1", (30.0, 5.0), false),
            footprint(C_MID, "C7", (20.0, 10.0), false),
            footprint(LOCKED, "U9", (40.0, 10.0), true),
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
    open_board_as(false)
}

/// The board opened from disk; `newer` writes it in a newer format, which
/// opens read-only.
fn open_board_as(newer: bool) -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    let mut text = SnxPcb::new(board()).write_string().unwrap();
    if newer {
        text = text.replacen("snxpcb/1.3", "snxpcb/1.9", 1);
    }
    std::fs::write(&path, text).unwrap();
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
    select_at(app, x, 10.0);
}

fn select_at(app: &mut Signex, x: f64, y: f64) {
    send(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(x, y),
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
    diagnostics::recent_entries().iter().any(|e| {
        e.id > since
            && e.message
                .replace(['\u{2068}', '\u{2069}'], "")
                .contains(wanted)
    })
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
        .and_then(|fp| fp.designator_draft.clone());
    assert_eq!(draft.as_deref(), Some("R5"));
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
        reannotate(BoardReannotateMsg::OrderPicked(ReannotateOrder::YThenX)),
    );
    let state = f.app.ui_state.board_reannotate.as_ref().unwrap();
    assert_eq!(state.options.order, ReannotateOrder::YThenX);
    // R1 sits higher (y 5), so by Y it comes first and keeps R1; by X it
    // would be third and become R2.
    assert_eq!(renames(&f), pairs(&[("C7", "C1")]));
    let before = snapshot(&f.app);
    send(&mut f.app, reannotate(BoardReannotateMsg::CancelRequested));
    assert!(f.app.ui_state.board_reannotate.is_none());
    assert_eq!(snapshot(&f.app), before, "Cancel changes nothing");
    assert_eq!(
        f.app.ui_state.board_reannotate_options.order,
        ReannotateOrder::YThenX,
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
    // The board moves on while the dialog is open: a Properties edit.
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
    select_at(&mut f.app, 30.0, 5.0);
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

fn facts(app: &Signex) -> signex_app::panels::PcbFootprintFacts {
    app.document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.footprint.clone())
        .expect("the footprint section")
}

fn typed(f: &mut Fixture, footprint: Uuid, field: FootprintTextField, text: &str) {
    let board = f.path.clone();
    send(
        &mut f.app,
        panel(PcbFootprintTextMsg::Typed {
            board,
            footprint,
            field,
            text: text.into(),
        }),
    );
}

fn enter(f: &mut Fixture, footprint: Uuid, field: FootprintTextField) {
    let board = f.path.clone();
    send(
        &mut f.app,
        panel(PcbFootprintTextMsg::Submitted {
            board,
            footprint,
            field,
        }),
    );
}

/// #1312 review: each field keeps its own draft; Enter in one field applies
/// that field only and keeps the other's text.
#[test]
fn each_field_keeps_its_own_draft_and_enter_applies_only_its_own() {
    let mut f = open_board();
    select(&mut f.app, 10.0);
    typed(&mut f, R_LEFT, FootprintTextField::Designator, "R5");
    enter(&mut f, R_LEFT, FootprintTextField::Value);
    assert_eq!(
        reference(&f.app, R_LEFT),
        "R2",
        "Enter in Value applies nothing"
    );
    assert_eq!(facts(&f.app).designator_draft.as_deref(), Some("R5"));
    typed(&mut f, R_LEFT, FootprintTextField::Value, "1k");
    let shown = facts(&f.app);
    assert_eq!(
        (
            shown.designator_draft.as_deref(),
            shown.value_draft.as_deref()
        ),
        (Some("R5"), Some("1k")),
        "typing Value keeps the Designator draft"
    );
    enter(&mut f, R_LEFT, FootprintTextField::Value);
    let board = live(&f.app);
    let fp = board
        .footprints
        .iter()
        .find(|fp| fp.uuid == R_LEFT)
        .unwrap();
    assert_eq!((fp.reference.as_str(), fp.value.as_str()), ("R2", "1k"));
    assert_eq!(facts(&f.app).designator_draft.as_deref(), Some("R5"));
    enter(&mut f, R_LEFT, FootprintTextField::Designator);
    assert_eq!(reference(&f.app, R_LEFT), "R5");
    assert!(f.app.interaction_state.pcb_footprint_draft.is_none());
}

/// #1312 review: a board opened read-only shows its fields read-only,
/// refuses an edit, and Re-Annotate shows why Execute is off and refuses it.
#[test]
fn a_read_only_board_refuses_properties_edits_and_re_annotate() {
    let mut f = open_board_as(true);
    select(&mut f.app, 10.0);
    assert_eq!(facts(&f.app).editability, FootprintEditability::ReadOnly);
    let before = snapshot(&f.app);
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    type_and_enter(&mut f, R_LEFT, FootprintTextField::Value, "1k");
    assert_eq!(snapshot(&f.app), before);
    assert!(
        noticed(since, "opened read-only"),
        "the refusal is in Messages"
    );

    open_dialog(
        &mut f,
        Message::Menu(MenuMessage::Command(command("reannotate_board"))),
    );
    let state = f.app.ui_state.board_reannotate.as_ref().unwrap();
    assert_eq!(state.problem, Some(ReannotateProblem::ReadOnly));
    assert!(!state.can_execute());
    send(&mut f.app, reannotate(BoardReannotateMsg::ExecuteRequested));
    assert_eq!(snapshot(&f.app), before, "Execute is refused");
    assert_eq!(
        f.app.ui_state.board_reannotate.as_ref().unwrap().problem,
        Some(ReannotateProblem::ReadOnly)
    );
}

/// #1312 review: the bottom-side option remakes the list and is kept for
/// the next opening.
#[test]
fn the_bottom_side_option_remakes_the_list_and_is_remembered() {
    let mut f = open_board();
    open_dialog(
        &mut f,
        Message::Menu(MenuMessage::Command(command("reannotate_board"))),
    );
    send(
        &mut f.app,
        reannotate(BoardReannotateMsg::BottomPicked(BottomSideOrder::WithTop)),
    );
    let state = f.app.ui_state.board_reannotate.as_ref().unwrap();
    let options = ReannotateOptions {
        order: ReannotateOrder::XThenY,
        bottom: BottomSideOrder::WithTop,
    };
    assert_eq!(state.options, options);
    assert_eq!(state.plan, plan_reannotate(&live(&f.app), options));
    assert_eq!(f.app.ui_state.board_reannotate_options, options);
}

/// Boards A (undocked) and B (main window), both the fixture board.
struct Windows {
    f: Fixture,
    b: std::path::PathBuf,
    window: iced::window::Id,
}

fn board_of(app: &Signex, path: &std::path::Path) -> PcbBoard {
    app.document_state
        .tabs
        .iter()
        .find(|t| t.path == path)
        .and_then(|t| t.cached_document.as_ref())
        .and_then(TabDocument::as_pcb)
        .expect("board")
        .clone()
}

fn tab_index(app: &Signex, path: &std::path::Path) -> usize {
    app.document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab")
}

fn windows() -> Windows {
    let mut f = open_board();
    let main = iced::window::Id::unique();
    f.app.ui_state.main_window_id = Some(main);
    let b = f._tmp.path().join("b.snxpcb");
    std::fs::write(&b, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    send(&mut f.app, Message::File(FileMsg::Opened(Some(b.clone()))));
    let index = tab_index(&f.app, &f.path);
    let before: std::collections::HashSet<_> = f.app.ui_state.windows.keys().copied().collect();
    let _ = f.app.update(Message::Window(WindowMsg::UndockTab(index)));
    let window = *f
        .app
        .ui_state
        .windows
        .keys()
        .find(|id| !before.contains(id))
        .expect("an undocked window");
    let _ = f.app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: f.path.clone(),
        id: window,
    }));
    let index = tab_index(&f.app, &b);
    send(
        &mut f.app,
        Message::Tab {
            window_id: main,
            msg: signex_app::tab_bar::TabMessage::Select(index),
        },
    );
    Windows { f, b, window }
}

/// A Properties message from the undocked window, as its panel sends it.
fn in_window(app: &Signex, window: iced::window::Id, msg: PcbFootprintTextMsg) -> Message {
    let sent = Message::Dock(DockMessage::Panel(PanelMsg::PcbFootprintText(msg)));
    let scoped = app.scope_dock_message(window, sent);
    assert!(
        matches!(&scoped, Message::Dock(DockMessage::ScopedToWindow(w, _)) if *w == window),
        "the window's fields act on its board"
    );
    scoped
}

/// #1312 review: in an undocked board window the fields edit that window's
/// board, and a selection in the main window (another board) keeps the
/// window's draft.
#[test]
fn an_undocked_board_windows_fields_edit_its_own_board() {
    let mut w = windows();
    send(
        &mut w.f.app,
        Message::Pcb(PcbMsg::CanvasInWindow {
            window: w.window,
            msg: PcbCanvasMsg::Clicked {
                at: Point::new(10.0, 10.0),
                tolerance_mm: 0.05,
                mode: ClickMode::Replace,
            },
        }),
    );
    let a = w.f.path.clone();
    let msg = in_window(
        &w.f.app,
        w.window,
        PcbFootprintTextMsg::Typed {
            board: a.clone(),
            footprint: R_LEFT,
            field: FootprintTextField::Designator,
            text: "R77".into(),
        },
    );
    send(&mut w.f.app, msg);
    assert!(
        w.f.app.interaction_state.pcb_footprint_draft.is_some(),
        "the window keeps its draft"
    );
    // A click in the main window on board B selects there; A's draft stays.
    send(
        &mut w.f.app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(20.0, 10.0),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
    assert!(
        w.f.app.interaction_state.pcb_footprint_draft.is_some(),
        "a selection on another board keeps the draft"
    );
    let b_before = serde_json::to_string(&board_of(&w.f.app, &w.b)).unwrap();
    let msg = in_window(
        &w.f.app,
        w.window,
        PcbFootprintTextMsg::Submitted {
            board: a.clone(),
            footprint: R_LEFT,
            field: FootprintTextField::Designator,
        },
    );
    send(&mut w.f.app, msg);
    let on_a = board_of(&w.f.app, &a);
    let r_left = on_a.footprints.iter().find(|fp| fp.uuid == R_LEFT).unwrap();
    assert_eq!(
        r_left.reference, "R77",
        "the edit lands on the window's board"
    );
    assert_eq!(
        serde_json::to_string(&board_of(&w.f.app, &w.b)).unwrap(),
        b_before,
        "the main window's board is untouched"
    );
}

/// #1312 review: an unapplied draft goes with an undo or redo of its board
/// and with a tab switch away from it.
#[test]
fn a_draft_goes_with_an_undo_and_a_tab_switch() {
    let mut f = open_board();
    select(&mut f.app, 10.0);
    type_and_enter(&mut f, R_LEFT, FootprintTextField::Designator, "R5");
    typed(&mut f, R_LEFT, FootprintTextField::Value, "1k");
    assert!(f.app.interaction_state.pcb_footprint_draft.is_some());
    undo(&mut f.app);
    assert_eq!(reference(&f.app, R_LEFT), "R2");
    assert!(
        f.app.interaction_state.pcb_footprint_draft.is_none(),
        "undo drops the draft"
    );

    typed(&mut f, R_LEFT, FootprintTextField::Value, "1k");
    assert!(f.app.interaction_state.pcb_footprint_draft.is_some());
    let other = f._tmp.path().join("other.snxpcb");
    std::fs::write(&other, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    send(&mut f.app, Message::File(FileMsg::Opened(Some(other))));
    assert!(
        f.app.interaction_state.pcb_footprint_draft.is_none(),
        "a tab switch away from the board drops the draft"
    );
}
