//! #2234 - board sketch mode: every entry point, a commit that is one undo
//! step and lands the outline, hole and keepout on the board, a refused
//! bake that leaves the board alone, and Esc.

use signex_app::app::{
    BoardSketchMsg, CommandPaletteMsg, ContextAction, ContextMenuMsg, EditMsg, FileMsg, InputMsg,
    Message, PcbMsg, Signex, TabContextAction, TabDocument, UiMsg,
};
use signex_app::dock::DockMessage;
use signex_app::keymap::{
    AppCommandId, ShortcutBinding, ShortcutBindingAction, ShortcutContext, ShortcutTrigger,
};
use signex_app::library::editor::footprint::state::SketchTool;
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit, RoleTag};
use signex_app::panels::{BoardSketchAttrMsg, PanelMsg};
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_types::format::SnxPcb;
use signex_types::pcb::{LayerDef, NetDef, PcbBoard, Point, Segment};
use uuid::Uuid;

use crate::drive::drive;
use crate::menu_support::menu_row;

fn layer(id: u8, name: &str) -> LayerDef {
    LayerDef {
        id,
        name: name.to_owned(),
        layer_type: "signal".to_owned(),
        display_name: String::new(),
    }
}

/// An empty two-layer board with one top track along y = 15.
fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: vec![NetDef {
            number: 1,
            name: "GND".into(),
        }],
        footprints: Vec::new(),
        segments: vec![Segment {
            uuid: Uuid::from_u128(10),
            start: Point::new(5.0, 15.0),
            end: Point::new(25.0, 15.0),
            width: 0.25,
            layer: "TopCopper".into(),
            net: 0,
            unknown_columns: Default::default(),
        }],
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
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    drive(&mut app, task);
    Fixture {
        app,
        path,
        _tmp: tmp,
    }
}

fn send(f: &mut Fixture, message: Message) {
    let task = f.app.update(message);
    drive(&mut f.app, task);
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

fn in_sketch(f: &Fixture) -> bool {
    f.app.interaction_state.board_sketch.is_some()
}

fn live(f: &Fixture) -> PcbBoard {
    let docs = &f.app.document_state;
    docs.tabs[docs.active_tab]
        .cached_document
        .as_ref()
        .and_then(TabDocument::as_pcb)
        .expect("a board is open")
        .clone()
}

fn snapshot(f: &Fixture) -> String {
    serde_json::to_string(&live(f)).unwrap()
}

fn edit(f: &mut Fixture, msg: FootprintEditorMsg) {
    let path = f.path.clone();
    send(
        f,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path,
            msg: PrimitiveEdit::Footprint(msg),
        }),
    );
}

fn click(f: &mut Fixture, x_mm: f64, y_mm: f64) {
    edit(
        f,
        FootprintEditorMsg::SketchToolClick {
            x_mm,
            y_mm,
            snap_id: None,
        },
    );
}

fn panel(f: &mut Fixture, msg: PanelMsg) {
    send(f, Message::Dock(DockMessage::Panel(msg)));
}

/// The ids of the working sketch's entities of one kind.
fn ids_of(f: &Fixture, kind: fn(&EntityKind) -> bool) -> Vec<SketchEntityId> {
    let session = f
        .app
        .interaction_state
        .board_sketch
        .as_ref()
        .expect("sketch mode");
    session
        .working_sketch()
        .entities
        .iter()
        .filter(|e| kind(&e.kind))
        .map(|e| e.id)
        .collect()
}

fn is_line(kind: &EntityKind) -> bool {
    matches!(kind, EntityKind::Line { .. })
}

fn is_circle(kind: &EntityKind) -> bool {
    matches!(kind, EntityKind::Circle { .. })
}

/// Draw a rectangle with the sketch tool; its four lines.
fn rectangle(f: &mut Fixture, from: (f64, f64), to: (f64, f64)) -> Vec<SketchEntityId> {
    let before = ids_of(f, is_line);
    edit(f, FootprintEditorMsg::SketchSetTool(SketchTool::Rectangle));
    click(f, from.0, from.1);
    click(f, to.0, to.1);
    edit(f, FootprintEditorMsg::ToolEscape);
    ids_of(f, is_line)
        .into_iter()
        .filter(|id| !before.contains(id))
        .collect()
}

#[test]
fn the_design_menu_row_enters_and_leaves() {
    let mut f = open_board();
    send(&mut f, menu_row("board_sketch"));
    assert!(in_sketch(&f), "Design > Board Sketch enters");
    send(&mut f, menu_row("board_sketch"));
    assert!(!in_sketch(&f), "the same row leaves");
}

#[test]
fn the_board_canvas_menu_row_enters() {
    let mut f = open_board();
    send(
        &mut f,
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
            id: command("board_sketch"),
            project: None,
        })),
    );
    assert!(in_sketch(&f));
}

#[test]
fn the_palette_enters() {
    let mut f = open_board();
    send(&mut f, Message::CommandPalette(CommandPaletteMsg::Open));
    send(
        &mut f,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged("Board Sketch".into())),
    );
    send(
        &mut f,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );
    assert!(in_sketch(&f));
}

/// No profile binds `board_sketch`; a key the user binds to it runs it.
#[test]
fn a_key_bound_by_the_user_enters() {
    let mut f = open_board();
    assert!(
        f.app
            .ui_state
            .active_keymap
            .shortcut_label(&command("board_sketch"))
            .is_none(),
        "no default key"
    );
    let profiles = &mut f.app.ui_state.keymap_profiles;
    let mut mine = profiles
        .active_profile()
        .copy_as_custom("mine", "Mine")
        .unwrap();
    mine.bindings.push(ShortcutBinding {
        action: ShortcutBindingAction::Command(command("board_sketch")),
        context: ShortcutContext::Pcb,
        triggers: vec![ShortcutTrigger::parse("Ctrl+Alt+K").unwrap()],
    });
    profiles.insert_custom_profile(mine).unwrap();
    profiles.set_active_profile("mine").unwrap();
    f.app.ui_state.active_keymap = f.app.ui_state.keymap_profiles.compile_active().into();
    let ShortcutTrigger::KeySequence(mut strokes) = ShortcutTrigger::parse("Ctrl+Alt+K").unwrap()
    else {
        panic!("a key");
    };
    send(
        &mut f,
        Message::Ui(UiMsg::KeymapStroke {
            window: None,
            stroke: strokes.remove(0),
        }),
    );
    assert!(in_sketch(&f));
}

/// The PCB Active Bar button sends the `board_sketch` command, the same
/// message as the menu row (the button itself: `view::board_sketch` tests).
#[test]
fn the_active_bar_message_enters() {
    let mut f = open_board();
    send(
        &mut f,
        Message::Menu(signex_app::menu_bar::MenuMessage::Command(command(
            "board_sketch",
        ))),
    );
    assert!(in_sketch(&f));
}

#[test]
fn esc_cancels_the_tool_then_leaves() {
    let mut f = open_board();
    send(&mut f, menu_row("board_sketch"));
    edit(&mut f, FootprintEditorMsg::SketchSetTool(SketchTool::Line));
    let esc = || Message::Input(InputMsg::EscapePressed { window: None });
    send(&mut f, esc());
    assert!(in_sketch(&f), "the first Esc drops the Line tool");
    send(&mut f, esc());
    assert!(!in_sketch(&f), "the next Esc leaves");
}

#[test]
fn a_commit_is_one_undo_step_with_the_outline_hole_and_keepout() {
    let mut f = open_board();
    let before = snapshot(&f);
    send(&mut f, menu_row("board_sketch"));

    let outline = rectangle(&mut f, (0.0, 0.0), (40.0, 30.0));
    assert_eq!(outline.len(), 4, "a rectangle is four lines");
    panel(
        &mut f,
        PanelMsg::BoardSketch(BoardSketchAttrMsg::OutlineToggled {
            id: outline[0],
            on: true,
        }),
    );

    edit(
        &mut f,
        FootprintEditorMsg::SketchSetTool(SketchTool::Circle),
    );
    // Whole-mm clicks, so no snap grid moves them: a 4 mm circle at (5, 5).
    click(&mut f, 5.0, 5.0);
    click(&mut f, 7.0, 5.0);
    edit(&mut f, FootprintEditorMsg::ToolEscape);
    let hole = ids_of(&f, is_circle)[0];
    panel(
        &mut f,
        PanelMsg::BoardSketch(BoardSketchAttrMsg::MountingHoleToggled { id: hole, on: true }),
    );

    let keepout = rectangle(&mut f, (10.0, 10.0), (20.0, 20.0));
    panel(
        &mut f,
        PanelMsg::FpEditorSetRole {
            id: keepout[0],
            role: RoleTag::Keepout,
        },
    );
    assert_eq!(
        snapshot(&f),
        before,
        "nothing reaches the board while drawing"
    );

    send(&mut f, menu_row("board_sketch"));
    assert!(!in_sketch(&f), "leaving commits and ends the mode");
    assert!(f.app.document_state.error_notice.is_none());
    let after = live(&f);
    assert_eq!(after.outline.len(), 4, "the rectangle is the outline");
    let stored = after.sketch.as_ref().expect("the board keeps its sketch");
    assert_eq!(stored.holes.len(), 1);
    assert!(
        (stored.holes[0].drill - 4.0).abs() < 1e-6,
        "the circle's diameter"
    );
    assert!((stored.holes[0].position.x - 5.0).abs() < 1e-6);
    assert_eq!(stored.keepouts.len(), 1);
    let drc = pcb_geom::run_drc(&after, &pcb_geom::DrcRules::new(200_000));
    assert!(
        drc.violations
            .iter()
            .any(|v| v.kind == pcb_geom::ViolationKind::Keepout),
        "DRC sees the track through the keepout"
    );

    send(&mut f, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f), before, "one undo restores the old board");
}

/// Closing the board's tab ends its sketch mode and drops the uncommitted
/// copy (Messages names the board); the reopened board starts fresh and
/// nothing old is committed.
#[test]
fn closing_the_board_drops_the_sketch_and_a_reopen_starts_fresh() {
    let mut f = open_board();
    let before = snapshot(&f);
    send(&mut f, menu_row("board_sketch"));
    let drawn = rectangle(&mut f, (0.0, 0.0), (40.0, 30.0));
    assert_eq!(drawn.len(), 4);

    let idx = f.app.document_state.active_tab;
    send(
        &mut f,
        Message::ContextMenu(ContextMenuMsg::TabAction(TabContextAction::Close(idx))),
    );
    assert!(!in_sketch(&f), "the mode ends with the tab");
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("board.snxpcb") && e.message.contains("discarded")),
        "Messages names the board whose changes were dropped"
    );

    let path = f.path.clone();
    send(&mut f, Message::File(FileMsg::Opened(Some(path))));
    assert!(!in_sketch(&f), "reopening does not bring the mode back");
    assert_eq!(snapshot(&f), before, "nothing old reached the board");

    send(&mut f, menu_row("board_sketch"));
    assert!(ids_of(&f, is_line).is_empty(), "a fresh, empty copy");
    send(&mut f, menu_row("board_sketch"));
    assert!(!in_sketch(&f));
    assert_eq!(
        snapshot(&f),
        before,
        "leaving the fresh copy commits nothing"
    );
}

#[test]
fn a_refused_bake_keeps_the_board_and_the_mode_and_says_why() {
    let mut f = open_board();
    let before = snapshot(&f);
    send(&mut f, menu_row("board_sketch"));
    edit(&mut f, FootprintEditorMsg::SketchSetTool(SketchTool::Line));
    click(&mut f, 0.0, 0.0);
    click(&mut f, 10.0, 0.0);
    edit(&mut f, FootprintEditorMsg::ToolEscape);
    let line = ids_of(&f, is_line)[0];
    panel(
        &mut f,
        PanelMsg::BoardSketch(BoardSketchAttrMsg::OutlineToggled { id: line, on: true }),
    );

    send(
        &mut f,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::LeaveRequested)),
    );
    assert!(in_sketch(&f), "a refused commit keeps the mode on");
    assert_eq!(snapshot(&f), before, "the board is unchanged");
    let notice = f
        .app
        .document_state
        .error_notice
        .as_ref()
        .expect("the notice card says why");
    assert!(
        notice.detail.contains("closed profile"),
        "{}",
        notice.detail
    );
    let session = f.app.interaction_state.board_sketch.as_ref().unwrap();
    assert_eq!(session.refusal.as_deref(), Some(notice.detail.as_str()));
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains(&notice.detail)),
        "Messages says why too"
    );

    // Discard drops the copy and leaves the board as it was.
    send(
        &mut f,
        Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::DiscardRequested)),
    );
    assert!(!in_sketch(&f));
    assert_eq!(snapshot(&f), before);
}
