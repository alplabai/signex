//! #2356: board sketch mode through `Signex::update`. The canvas cursor
//! follows the pointer, and Properties shows the board, not footprint fields.
//! The origin, grid, guides and angles read as on the board view.

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

pub(super) fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    drive(app, task);
}

/// A two-layer board with a 40 x 25 mm outline, open and in sketch mode.
fn sketching() -> (Signex, tempfile::TempDir) {
    sketching_with(|_| {})
}

/// [`sketching`] on the board as `adjust` leaves it.
fn sketching_with(
    adjust: impl FnOnce(&mut signex_types::pcb::PcbBoard),
) -> (Signex, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("temp dir");
    let path = tmp.path().join("rover.snxpcb");
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.outline = [(0.0, 0.0), (40.0, 0.0), (40.0, 25.0), (0.0, 25.0)]
        .into_iter()
        .map(|(x, y)| BoardPoint::new(x, y))
        .collect();
    adjust(&mut board);
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
            None,
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
/// of the editor the sketch is hosted on. The sketch has no closed outline
/// yet, so the size reads from the board's, worded as Board Properties words
/// it (mockup row 179e: the shown unit, three decimals in mm, no "from board
/// shape").
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
    for board_row in ["rover", "Size", "40.000 × 25.000 mm", "Area", "1000.00 mm²"] {
        assert!(shows(board_row), "{board_row:?} missing from {texts:?}");
    }
}

/// Mockup row 187: Messages never repeats the sketch mode hint, however
/// often the mode is entered.
#[test]
fn entering_sketch_mode_again_does_not_repeat_the_hint() {
    let (mut app, _tmp) = sketching();
    assert!(
        app.interaction_state.board_sketch_hint_said,
        "the first entry says the hint"
    );
    // Lines this thread logs: other tests sharing the ring do not move it.
    let said = crate::diagnostics::said_on_this_thread();
    for _ in 0..3 {
        let toggle = || Message::Pcb(PcbMsg::Sketch(BoardSketchMsg::ModeToggled));
        send(&mut app, toggle());
        assert!(app.interaction_state.board_sketch.is_none(), "left");
        send(&mut app, toggle());
        assert!(app.interaction_state.board_sketch.is_some(), "entered");
    }
    assert_eq!(
        crate::diagnostics::said_on_this_thread(),
        said,
        "leaving and entering again with no change says nothing new"
    );
    assert!(app.interaction_state.board_sketch_hint_said);
}

/// The board in sketch mode with its origin at (5, 30).
fn sketching_from_origin() -> (Signex, tempfile::TempDir) {
    sketching_with(|board| {
        board.options.origin =
            signex_types::pcb_origin::BoardOrigin::new(BoardPoint::new(5.0, 30.0));
    })
}

fn marker_mm(app: &Signex) -> Option<(f64, f64)> {
    let session = app.interaction_state.board_sketch.as_ref()?;
    session.editor.state.origin_marker_mm()
}

/// Caner, 2026-11-21: the sketch marks the same board point as the PCB view.
#[test]
fn the_origin_marker_sits_at_the_boards_origin_and_moves_with_it() {
    use crate::app::contracts::BoardOriginMsg;
    let (mut app, _tmp) = sketching_from_origin();
    let board_mark = |app: &Signex| {
        let board = app.active_pcb().expect("the board");
        crate::pcb_canvas::BoardMarks::of(board).origin
    };
    assert_eq!(board_mark(&app), [5.0, 30.0]);
    assert_eq!(marker_mm(&app), Some((5.0, 30.0)));

    // Reset puts it on the outline's bottom-left corner.
    send(
        &mut app,
        Message::Pcb(PcbMsg::Origin(BoardOriginMsg::ResetPicked)),
    );
    assert_eq!(board_mark(&app), [0.0, 25.0]);
    assert_eq!(marker_mm(&app), Some((0.0, 25.0)), "the marker follows");
}

#[test]
fn the_origin_marker_follows_the_board_views_visibility() {
    use crate::app::contracts::PcbViewMsg;
    let (mut app, _tmp) = sketching_from_origin();
    let toggle = Message::Pcb(PcbMsg::View(PcbViewMsg::OriginMarkerVisibilityToggled));
    send(&mut app, toggle.clone());
    assert_eq!(marker_mm(&app), None, "hidden, and not at model (0, 0)");
    send(&mut app, toggle);
    assert_eq!(marker_mm(&app), Some((5.0, 30.0)));
}

#[test]
fn a_footprint_sketch_keeps_its_origin_and_readouts() {
    use crate::library::editor::footprint::{EditorMode, FootprintEditorState};
    let footprint = signex_library::Footprint::empty("f");
    let mut state = FootprintEditorState::from_footprint(&footprint);
    state.mode = EditorMode::Sketch;
    assert_eq!(state.origin_marker_mm(), Some((0.0, 0.0)));
    assert_eq!(state.shown_mm([3.0, 4.0]), [3.0, 4.0]);
}

/// The status bar reads the sketch cursor as the PCB view reads a point:
/// from the board origin, Y up.
#[test]
fn the_status_cursor_reads_from_the_board_origin_with_y_up() {
    use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    let (mut app, _tmp) = sketching_from_origin();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    let path = app.active_tab_path().expect("the board tab");
    send(
        &mut app,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path,
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::CursorAt {
                x_mm: 15.0,
                y_mm: 10.0,
                px_per_mm: 30.0,
            }),
        }),
    );
    assert_eq!(cursor_mm(&app), Some((15.0, 10.0)), "stored as model mm");
    assert_eq!(app.status_cursor(window), (10.0, 20.0));
}

/// Properties reads a sketch point from the board origin; the sketch keeps
/// the model coordinates.
#[test]
fn properties_read_a_sketch_point_from_the_board_origin() {
    use crate::library::editor::footprint::SketchTool;
    use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
    use crate::panels::PanelMsg;
    use signex_sketch::entity::EntityKind;
    let (mut app, _tmp) = sketching_from_origin();
    let path = app.active_tab_path().expect("the board tab");
    let edit = |app: &mut Signex, msg| {
        send(
            app,
            Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEdit::Footprint(msg),
            }),
        );
    };
    edit(
        &mut app,
        FootprintEditorMsg::SketchSetTool(SketchTool::Line),
    );
    for x_mm in [15.0, 25.0] {
        edit(
            &mut app,
            FootprintEditorMsg::SketchToolClick {
                x_mm,
                y_mm: 10.0,
                snap_id: None,
            },
        );
    }
    edit(&mut app, FootprintEditorMsg::ToolEscape);
    let (id, x, y) = {
        let session = app.interaction_state.board_sketch.as_ref().expect("on");
        let sketch = session.working_sketch();
        sketch
            .entities
            .iter()
            .find_map(|e| match e.kind {
                EntityKind::Point { x, y } => Some((e.id, x, y)),
                _ => None,
            })
            .expect("a point")
    };
    send(
        &mut app,
        Message::Dock(crate::dock::DockMessage::Panel(
            PanelMsg::FpEditorSelectSketchEntity { id },
        )),
    );
    let shown = app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .and_then(|fp| fp.selected_sketch_entity.as_ref())
        .and_then(|entity| entity.position_mm)
        .expect("the point's position");
    assert_eq!(shown, [x - 5.0, 30.0 - y]);
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let stored = session.working_sketch();
    let kept = stored.entities.iter().find(|e| e.id == id).expect("kept");
    assert!(matches!(kept.kind, EntityKind::Point { x: a, y: b } if a == x && b == y));
}

/// Caner, 2026-11-21: on a board with an off-grid origin at (5.3, 30.7) the
/// sketch's grid, guides and angles follow the board origin.
pub(super) fn sketching_off_grid() -> (Signex, tempfile::TempDir) {
    sketching_with(|board| {
        board.options.origin =
            signex_types::pcb_origin::BoardOrigin::new(BoardPoint::new(5.3, 30.7));
    })
}

pub(super) fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

pub(super) fn sketch_edit(
    app: &mut Signex,
    path: &std::path::Path,
    msg: crate::library::messages::FootprintEditorMsg,
) {
    use crate::library::messages::{LibraryMessage, PrimitiveEdit};
    send(
        app,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Footprint(msg),
        }),
    );
}

fn click(app: &mut Signex, path: &std::path::Path, x_mm: f64, y_mm: f64) {
    use crate::library::messages::FootprintEditorMsg;
    sketch_edit(
        app,
        path,
        FootprintEditorMsg::SketchToolClick {
            x_mm,
            y_mm,
            snap_id: None,
        },
    );
}

fn typed(app: &mut Signex, path: &std::path::Path, digits: &str) {
    use crate::library::messages::FootprintEditorMsg;
    for ch in digits.chars() {
        sketch_edit(app, path, FootprintEditorMsg::SketchPlacementInputChar(ch));
    }
}

/// A Line from (15, 10), 5 mm long at a typed 30 degrees.
fn draw_typed_line(app: &mut Signex, path: &std::path::Path) {
    use crate::library::editor::footprint::SketchTool;
    use crate::library::messages::FootprintEditorMsg;
    sketch_edit(
        app,
        path,
        FootprintEditorMsg::SketchSetTool(SketchTool::Line),
    );
    click(app, path, 15.0, 10.0);
    typed(app, path, "5");
    sketch_edit(app, path, FootprintEditorMsg::SketchPlacementInputTab);
    typed(app, path, "30");
    click(app, path, 1.0, 1.0);
}

/// An Arc about (10, 10) from (15, 10), with a typed sweep of 90 degrees.
fn draw_typed_arc(app: &mut Signex, path: &std::path::Path) {
    use crate::library::editor::footprint::SketchTool;
    use crate::library::messages::FootprintEditorMsg;
    sketch_edit(
        app,
        path,
        FootprintEditorMsg::SketchSetTool(SketchTool::Arc),
    );
    click(app, path, 10.0, 10.0);
    click(app, path, 15.0, 10.0);
    typed(app, path, "90");
    click(app, path, 1.0, 1.0);
}

fn last_point(sketch: &signex_sketch::SketchData) -> (f64, f64) {
    use signex_sketch::entity::EntityKind;
    sketch
        .entities
        .iter()
        .rev()
        .find_map(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .expect("a point")
}

fn arc_turn(sketch: &signex_sketch::SketchData) -> bool {
    use signex_sketch::entity::EntityKind;
    sketch
        .entities
        .iter()
        .find_map(|e| match e.kind {
            EntityKind::Arc { sweep_ccw, .. } => Some(sweep_ccw),
            _ => None,
        })
        .expect("an arc")
}

pub(super) fn board_sketch_data(app: &Signex) -> signex_sketch::SketchData {
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    session.working_sketch()
}

/// A footprint tab in sketch mode, the same inputs' other host.
pub(super) fn footprint_sketching() -> (Signex, std::path::PathBuf) {
    use crate::app::{FootprintDocument, TabInfo, TabKind};
    use crate::library::editor::footprint::EditorMode;
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = std::path::PathBuf::from("angles.snxfpt");
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    let mut editor = FootprintDocument::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "f".into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    (app, path)
}

pub(super) fn footprint_sketch_data(
    app: &Signex,
    path: &std::path::Path,
) -> signex_sketch::SketchData {
    let editor = app.document_state.footprint_editors.get(path);
    let editor = editor.expect("the editor");
    editor.primitive().sketch.clone().expect("a sketch")
}

#[test]
fn grid_snap_lands_on_the_board_origin_plus_whole_steps() {
    use crate::library::editor::footprint::snap::{SnapKind, snap_cursor};
    let (app, _tmp) = sketching_off_grid();
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let state = &session.editor.state;
    let sketch = session.working_sketch();
    let step = state.snap_options.grid_step_mm;
    let hit = snap_cursor((12.2, 13.1), Some(&sketch), state, None);
    assert!(matches!(hit.kind, SnapKind::Grid), "{:?}", hit.kind);
    for (at, from) in [(hit.pos.0, 5.3), (hit.pos.1, 30.7)] {
        let steps = (at - from) / step;
        assert!(near(steps, steps.round()), "{at} from {from}");
    }
    let [x, y] = state.shown_mm([hit.pos.0, hit.pos.1]);
    assert!(near(x / step, (x / step).round()), "reads round: {x}");
    assert!(near(y / step, (y / step).round()), "reads round: {y}");
}

#[test]
fn a_footprint_sketch_grid_still_steps_from_zero() {
    use crate::library::editor::footprint::FootprintEditorState;
    use crate::library::editor::footprint::snap::snap_cursor;
    let footprint = signex_library::Footprint::empty("f");
    let state = FootprintEditorState::from_footprint(&footprint);
    assert_eq!(state.grid_origin_mm(), None);
    let step = state.snap_options.grid_step_mm;
    let hit = snap_cursor((12.2, 13.1), None, &state, None);
    let expected = ((12.2 / step).round() * step, (13.1 / step).round() * step);
    assert_eq!(hit.pos, expected);
}

#[test]
fn a_guide_shows_and_takes_its_position_from_the_board_origin() {
    use crate::panels::PanelMsg;
    let (mut app, _tmp) = sketching_off_grid();
    let panel = |msg| Message::Dock(crate::dock::DockMessage::Panel(msg));
    let stored = |app: &Signex, i: usize| {
        let session = app.interaction_state.board_sketch.as_ref().expect("on");
        session.editor.state.guides[i].position_mm
    };
    let shown = |app: &Signex, i: usize| {
        let ctx = app.document_state.panel_ctx.footprint_editor.as_ref();
        ctx.expect("the panel").guides[i].position_mm
    };
    send(&mut app, panel(PanelMsg::FpEditorGuideAddVertical));
    assert!(near(stored(&app, 0), 5.3), "a new guide reads 0");
    assert!(near(shown(&app, 0), 0.0));
    send(
        &mut app,
        panel(PanelMsg::FpEditorGuideSetPosition(0, "12.5".into())),
    );
    assert!(near(stored(&app, 0), 17.8), "{}", stored(&app, 0));
    assert!(near(shown(&app, 0), 12.5));

    send(&mut app, panel(PanelMsg::FpEditorGuideAddHorizontal));
    assert!(near(stored(&app, 1), 30.7));
    send(
        &mut app,
        panel(PanelMsg::FpEditorGuideSetPosition(1, "4".into())),
    );
    assert!(near(stored(&app, 1), 26.7), "Y runs up from the origin");
    assert!(near(shown(&app, 1), 4.0));
}

#[test]
fn a_footprint_guide_stores_what_is_typed() {
    use crate::panels::PanelMsg;
    let (mut app, path) = footprint_sketching();
    let panel = |msg| Message::Dock(crate::dock::DockMessage::Panel(msg));
    send(&mut app, panel(PanelMsg::FpEditorGuideAddHorizontal));
    send(
        &mut app,
        panel(PanelMsg::FpEditorGuideSetPosition(0, "4".into())),
    );
    let editor = app.document_state.footprint_editors.get(&path);
    assert_eq!(editor.expect("the editor").state.guides[0].position_mm, 4.0);
}

#[test]
fn a_typed_line_angle_runs_counter_clockwise_on_screen() {
    let (mut app, _tmp) = sketching_off_grid();
    let path = app.active_tab_path().expect("the board tab");
    draw_typed_line(&mut app, &path);
    let end = last_point(&board_sketch_data(&app));
    let run = 5.0 * 30.0_f64.to_radians().cos();
    assert!(near(end.0, 15.0 + run), "{end:?}");
    assert!(near(end.1, 10.0 - 2.5), "up the screen is less y: {end:?}");
}

#[test]
fn a_footprint_typed_line_angle_is_unchanged() {
    let (mut app, path) = footprint_sketching();
    draw_typed_line(&mut app, &path);
    let end = last_point(&footprint_sketch_data(&app, &path));
    let run = 5.0 * 30.0_f64.to_radians().cos();
    assert!(near(end.0, 15.0 + run), "{end:?}");
    assert!(near(end.1, 10.0 + 2.5), "{end:?}");
}

#[test]
fn a_typed_arc_sweep_turns_counter_clockwise_on_screen() {
    let (mut app, _tmp) = sketching_off_grid();
    let path = app.active_tab_path().expect("the board tab");
    draw_typed_arc(&mut app, &path);
    let sketch = board_sketch_data(&app);
    let end = last_point(&sketch);
    assert!(near(end.0, 10.0) && near(end.1, 5.0), "{end:?}");
    assert!(!arc_turn(&sketch), "clockwise in the model's y-down axes");
}

#[test]
fn a_footprint_typed_arc_sweep_is_unchanged() {
    let (mut app, path) = footprint_sketching();
    draw_typed_arc(&mut app, &path);
    let sketch = footprint_sketch_data(&app, &path);
    let end = last_point(&sketch);
    assert!(near(end.0, 10.0) && near(end.1, 15.0), "{end:?}");
    assert!(arc_turn(&sketch));
}

/// A literal angle reads and is entered in the board's sense and is stored
/// as it was; an expression reads through the same mirror.
#[test]
fn a_literal_angle_dimension_converts_only_at_the_boundary() {
    use crate::library::editor::footprint::FootprintEditorState;
    use crate::library::editor::footprint::sketch_mode::value_edit::constraint_buffer;
    use signex_sketch::constraint::{ConstraintKind, DimTarget};
    use signex_sketch::id::SketchEntityId;
    let (app, _tmp) = sketching_off_grid();
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let state = &session.editor.state;
    let (l1, l2) = (SketchEntityId::new(), SketchEntityId::new());
    let angle = |target| ConstraintKind::Angle { l1, l2, target };
    let stored = angle(DimTarget::Literal((-30.0_f64).to_radians()));
    let shown = state.shown_constraint(&stored);
    assert_eq!(constraint_buffer(&shown).as_deref(), Some("30"));
    let back = state.stored_target(&stored, DimTarget::Literal(30.0_f64.to_radians()));
    assert_eq!(back, DimTarget::Literal(0.0 - 30.0_f64.to_radians()));
    let bound = angle(DimTarget::Expr("slope".into()));
    assert_eq!(
        *state.shown_constraint(&bound),
        angle(DimTarget::Expr("-slope".into()))
    );

    let footprint = signex_library::Footprint::empty("f");
    let plain = FootprintEditorState::from_footprint(&footprint);
    assert_eq!(*plain.shown_constraint(&stored), stored);
    assert_eq!(plain.angle_deg_mirrored(30.0), 30.0);
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
    let _ = crate::diagnostics::init_logging();
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
    // Fluent wraps the role name in isolation marks; read the line without.
    let said = crate::diagnostics::entries_since(mark)
        .entries
        .iter()
        .any(|e| {
            e.message
                .replace(['\u{2068}', '\u{2069}'], "")
                .contains("V-score was not set")
        });
    assert!(said, "Messages says why");
}

/// #2546: a close by operand that a refused bake holds answers with the
/// refusal and its reason, never `Ran`, and the tab stays open.
#[test]
fn a_close_by_operand_held_by_a_refused_bake_is_refused_with_the_reason() {
    use crate::app::command::{CommandArgs, CommandOutcome};
    use crate::keymap::AppCommandId;
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
        session
            .working_sketch()
            .entities
            .last()
            .expect("the line")
            .id
    };
    send(
        &mut app,
        Message::Dock(crate::dock::DockMessage::Panel(PanelMsg::BoardSketch(
            BoardSketchAttrMsg::OutlineToggled { id: line, on: true },
        ))),
    );
    let tabs = app.document_state.tabs.len();

    let command = AppCommandId::new("close_active_document").expect("id");
    let operand = format!("document={}", path.display());
    let args = CommandArgs::parse_operands(&command, &[operand.as_str()]).expect("operands");
    let (outcome, _task) = app.run_operands(&command, &args);

    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("a held close reported {outcome:?}");
    };
    let reason = signex_i18n::RenderTranslatable::render_en_us(&reason);
    assert!(reason.contains("could not be applied"), "{reason}");
    assert!(
        reason.contains("The sketch was not applied to the board"),
        "the reason carries the bake's refusal: {reason}"
    );
    assert_eq!(app.document_state.tabs.len(), tabs, "the tab stays open");
}
