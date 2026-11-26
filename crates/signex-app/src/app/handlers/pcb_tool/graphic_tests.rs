//! #2424 — the Graphics tools through `Signex::update`: armed by their
//! command, drawn on the active layer (not a keep-out), each drawing one undo
//! step, previewed on the cursor, their values set in Properties with Tab.

use std::path::Path;

use signex_types::pcb::{PCB_DEFAULT_GRAPHIC_WIDTH_MM, Point};
use signex_types::pcb_graphic::BoardGraphicKind;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::form_tests::{enter, form, panel, tab, typed};
use super::tests::{app_with_board, board_of, click, escape, hover, live, run, undo};
use crate::app::{EditMsg, Message, PcbMsg, PcbViewMsg, Signex};
use crate::panels::PcbPrimitiveMsg;
use crate::panels::pcb_primitive_properties::{
    GraphicFacts, PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget,
};

const OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);

fn pick_layer(app: &mut Signex, layer: PcbLayer) {
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        layer,
    ))));
}

fn kinds(app: &Signex, path: &Path) -> Vec<String> {
    board_of(app, path)
        .graphics
        .iter()
        .map(|g| g.graphic_type.clone())
        .collect()
}

fn graphic_form(app: &Signex) -> GraphicFacts {
    match form(app) {
        PcbPrimitiveFacts::Graphic(f) => f,
        other => panic!("the drawing form, not {other:?}"),
    }
}

/// Every Graphics tool draws on the active overlay, one undo step each; a
/// line run goes on from its end until a click on that end.
#[test]
fn each_graphics_tool_draws_one_undo_step_per_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    // #2879 turned object and axis snapping on by default: (1, 2) lies on
    // the axis of the net-3 track's end 2 mm away and would land on y = 0.
    // This test is about the drawings, not the snaps, so the grid alone
    // holds the clicks, as the drag rigs do.
    app.ui_state.pcb_snap.objects.clear();
    app.ui_state.pcb_snap.to_axes = false;
    let before = live(&app, &path);
    pick_layer(&mut app, OVERLAY);

    run(&mut app, "place_board_line");
    for (x, y) in [(1.0, 2.0), (5.0, 2.0), (5.0, 4.0), (5.0, 4.0)] {
        click(&mut app, x, y);
    }
    assert_eq!(
        kinds(&app, &path),
        ["line", "line"],
        "two legs, then the end"
    );
    let lines = &board_of(&app, &path).graphics;
    assert_eq!(lines[0].start, Some(Point::new(1.0, 2.0)));
    assert_eq!(lines[1].end, Some(Point::new(5.0, 4.0)));
    assert_eq!(lines[0].layer, "TopSilkscreen");
    assert_eq!(lines[0].width, PCB_DEFAULT_GRAPHIC_WIDTH_MM);
    assert_ne!(lines[0].uuid, lines[1].uuid);

    run(&mut app, "place_board_graphic_arc");
    for (x, y) in [(10.0, 10.0), (15.0, 10.0), (10.0, 20.0)] {
        click(&mut app, x, y);
    }
    run(&mut app, "place_board_circle");
    click(&mut app, 20.0, 20.0);
    click(&mut app, 23.0, 20.0);
    run(&mut app, "place_board_rectangle");
    click(&mut app, 30.0, 30.0);
    click(&mut app, 34.0, 33.0);
    run(&mut app, "place_board_text_frame");
    click(&mut app, 40.0, 40.0);
    click(&mut app, 50.0, 45.0);
    assert_eq!(
        kinds(&app, &path),
        ["line", "line", "arc", "circle", "rect", "text_frame"]
    );
    let graphics = &board_of(&app, &path).graphics;
    assert_eq!(graphics[2].radius, 5.0);
    assert_eq!(
        graphics[2].end,
        Some(Point::new(10.0, 15.0)),
        "on the circle"
    );
    assert_eq!(graphics[3].radius, 3.0);
    assert_eq!(graphics[5].text, "Text");

    for _ in 0..6 {
        undo(&mut app);
    }
    assert_eq!(live(&app, &path), before, "one undo per drawing");
}

/// Was `a_graphics_tool_draws_nothing_on_copper`. Since Caner's 2026-11-24
/// decision (#2883) a rectangle on copper is a drawing of copper with no
/// net; only a keep-out layer draws nothing, the tool staying armed, and
/// it draws once an overlay is active.
#[test]
fn a_graphics_tool_draws_on_copper_and_nothing_on_a_keep_out() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    pick_layer(&mut app, PcbLayer::Copper(CopperLayer::Top));
    run(&mut app, "place_board_rectangle");
    click(&mut app, 3.0, 3.0);
    click(&mut app, 6.0, 6.0);
    assert_eq!(kinds(&app, &path), ["rect"]);
    assert_eq!(board_of(&app, &path).graphics[0].layer, "TopCopper");
    let before = live(&app, &path);
    pick_layer(&mut app, PcbLayer::Technical(TechnicalLayer::KeepoutTop));
    click(&mut app, 3.0, 3.0);
    click(&mut app, 6.0, 6.0);
    assert_eq!(live(&app, &path), before, "nothing on a keep-out layer");
    let session = app.interaction_state.pcb_tool.as_ref().expect("armed");
    assert!(session.points.is_empty(), "no corner was taken");
    pick_layer(&mut app, OVERLAY);
    click(&mut app, 3.0, 3.0);
    click(&mut app, 6.0, 6.0);
    assert_eq!(kinds(&app, &path), ["rect", "rect"]);
}

/// What the next click adds follows the pointer; Esc drops it, then ends
/// the tool.
#[test]
fn the_next_drawing_follows_the_pointer_and_esc_drops_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    pick_layer(&mut app, OVERLAY);
    run(&mut app, "place_board_circle");
    click(&mut app, 5.0, 5.0);
    hover(&mut app, 8.0, 5.0);
    let preview = app.board_tool_preview().expect("a preview");
    assert_eq!(preview.graphics.len(), 1);
    assert_eq!(preview.graphics[0].radius, 3.0);
    let ghost = app.board_tool_ghost().expect("drawn on the cursor");
    assert!(!ghost.graphics.is_empty(), "the renderer draws the circle");

    escape(&mut app);
    let session = app.interaction_state.pcb_tool.as_ref().expect("armed");
    assert!(session.points.is_empty(), "Esc drops the circle");
    escape(&mut app);
    assert!(
        app.interaction_state.pcb_tool.is_none(),
        "then ends the tool"
    );
    assert!(board_of(&app, &path).graphics.is_empty());
}

/// #2393 form: Properties shows the text frame placed next; Tab pauses,
/// the text, its height and the line width typed there are what the next
/// frames are placed with.
#[test]
fn a_text_frame_takes_the_values_set_in_its_form() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    pick_layer(&mut app, OVERLAY);
    run(&mut app, "place_board_text_frame");
    let f = graphic_form(&app);
    assert!(f.common.target.is_placement());
    assert_eq!(f.layer.as_ref().map(|l| l.layer), Some(OVERLAY));
    assert_eq!(f.text.as_deref(), Some("Text"));

    tab(&mut app);
    assert!(app.is_board_tool_paused());
    typed(&mut app, &path, PrimitiveField::Text, "REV A\\nBOARD 2");
    enter(&mut app, &path, PrimitiveField::Text);
    assert!(!app.is_board_tool_paused(), "Enter goes on placing");
    typed(&mut app, &path, PrimitiveField::TextHeight, "1.5");
    enter(&mut app, &path, PrimitiveField::TextHeight);
    typed(&mut app, &path, PrimitiveField::Width, "0.2");
    enter(&mut app, &path, PrimitiveField::Width);
    assert_eq!(graphic_form(&app).text.as_deref(), Some("REV A\\nBOARD 2"));

    click(&mut app, 10.0, 10.0);
    click(&mut app, 20.0, 16.0);
    let frame = &board_of(&app, &path).graphics[0];
    assert_eq!(frame.text, "REV A\nBOARD 2", "\\n is a line break");
    assert_eq!((frame.font_size, frame.width), (1.5, 0.2));
}

/// A placed drawing is picked on its stroke; Properties edits its radius
/// and layer, one undo step each, and Delete removes it.
#[test]
fn a_placed_drawing_is_edited_in_properties_and_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    pick_layer(&mut app, OVERLAY);
    run(&mut app, "place_board_circle");
    click(&mut app, 5.0, 5.0);
    click(&mut app, 8.0, 5.0);
    escape(&mut app);
    assert!(app.interaction_state.pcb_tool.is_none());
    click(&mut app, 8.0, 5.0);
    let f = graphic_form(&app);
    assert_eq!(f.kind, Some(BoardGraphicKind::Circle));
    assert_eq!(f.radius, Some(3.0));
    let target = f.common.target.clone();
    assert_eq!(target.items.len(), 1, "the circle is selected");

    panel(
        &mut app,
        PcbPrimitiveMsg::FieldTyped {
            target: target.clone(),
            field: PrimitiveField::Radius,
            text: "4".into(),
        },
    );
    panel(
        &mut app,
        PcbPrimitiveMsg::FieldSubmitted {
            target: target.clone(),
            field: PrimitiveField::Radius,
        },
    );
    let circle = &board_of(&app, &path).graphics[0];
    assert_eq!(circle.radius, 4.0);
    assert_eq!(circle.end, Some(Point::new(9.0, 5.0)), "the rim follows");
    undo(&mut app);
    assert_eq!(board_of(&app, &path).graphics[0].radius, 3.0, "one step");

    let fab = PcbLayer::Technical(TechnicalLayer::FabBottom);
    panel(
        &mut app,
        PcbPrimitiveMsg::AreaLayerPicked {
            target: target.clone(),
            layer: fab,
        },
    );
    assert_eq!(board_of(&app, &path).graphics[0].layer, "BottomFab");
    // A drawing never went on copper here; since #2883 it does, and only a
    // keep-out is refused.
    panel(
        &mut app,
        PcbPrimitiveMsg::AreaLayerPicked {
            target,
            layer: PcbLayer::Technical(TechnicalLayer::KeepoutTop),
        },
    );
    assert_eq!(
        board_of(&app, &path).graphics[0].layer,
        "BottomFab",
        "a drawing never goes on a keep-out"
    );

    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert!(
        board_of(&app, &path).graphics.is_empty(),
        "Delete removes it"
    );
    undo(&mut app);
    assert_eq!(board_of(&app, &path).graphics.len(), 1);
}

/// A layer picked in the form is the active layer; a keep-out is refused.
/// Was `the_form_layer_is_the_active_layer_and_copper_is_refused`: copper
/// takes a drawing since Caner's 2026-11-24 decision (#2883).
#[test]
fn the_form_layer_is_the_active_layer_and_a_keep_out_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_line");
    let fab = PcbLayer::Technical(TechnicalLayer::FabTop);
    let pick = |layer| PcbPrimitiveMsg::AreaLayerPicked {
        target: PrimitiveTarget::placement(path.clone()),
        layer,
    };
    panel(&mut app, pick(fab));
    assert_eq!(app.pcb_layer_view(&path).active(), fab);
    panel(
        &mut app,
        pick(PcbLayer::Technical(TechnicalLayer::KeepoutTop)),
    );
    assert_eq!(
        app.pcb_layer_view(&path).active(),
        fab,
        "a keep-out is refused"
    );
    click(&mut app, 1.0, 3.0);
    click(&mut app, 4.0, 3.0);
    assert_eq!(board_of(&app, &path).graphics[0].layer, "TopFab");
}

/// #3000, mockup row 102: in Properties, the line's illustration names its
/// layer and ends; a circle's and a text frame's, which have no Ends row,
/// name the layer alone.
#[test]
fn only_the_lines_illustration_names_its_ends() {
    use crate::panels::pcb_primitive_properties::corner_notes;
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    pick_layer(&mut app, OVERLAY);
    let i18n = app.ui_state.i18n.clone();
    let mut notes_of = |command: &str| {
        run(&mut app, command);
        let f = graphic_form(&app);
        escape(&mut app);
        let layer = f
            .layer
            .as_ref()
            .map(|l| l.label.clone())
            .unwrap_or_default();
        (layer, corner_notes(&i18n, &f))
    };
    let (layer, line) = notes_of("place_board_line");
    assert_eq!(line, [format!("{layer} · round ends")]);
    for command in ["place_board_circle", "place_board_text_frame"] {
        let (layer, notes) = notes_of(command);
        assert_eq!(notes, [layer], "{command}");
    }
}
