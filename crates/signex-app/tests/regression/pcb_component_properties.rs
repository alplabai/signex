//! #2354: the board Component view, driven through `Signex::update` with the
//! messages its controls send. Editing X moves the component, Rotation turns
//! it, the Layer segments flip it, the Designator eye hides its text and the
//! lock stops a drag. Every edit is one board undo step. The illustration is
//! drawn for one component, and its hover link lives in the panel state.

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{EditMsg, FileMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::panels::pcb_component::DESIGNATOR_FIELD;
use signex_app::panels::{ComponentField, PanelMsg, PcbComponentFacts, PcbComponentMsg, Shared};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_app::test_adapter::BoardEngines;
use signex_engine::pcb::component::LocationField;
use signex_engine::pcb::designators::FootprintTextField;
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard, Point};
use signex_types::pcb_layer::BoardSide;
use signex_types::property::PcbProperty;
use signex_widgets::property_illustration::IllustrationMsg;
use uuid::Uuid;

/// R2 at (10, 10) and C7 at (20, 10), both on the top side.
const R2: Uuid = Uuid::from_u128(0x10);
const C7: Uuid = Uuid::from_u128(0x30);

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
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn property(key: &str, value: &str) -> PcbProperty {
    PcbProperty {
        key: key.into(),
        value: value.into(),
        layer: Some("TopSilkscreen".into()),
        ..PcbProperty::default()
    }
}

/// A footprint with pads 2 mm either side of its origin, so a click on the
/// origin picks the footprint, not a pad.
fn footprint(uuid: Uuid, reference: &str, x: f64) -> Footprint {
    let n = uuid.as_u128();
    Footprint {
        uuid,
        reference: reference.into(),
        value: "10k".into(),
        footprint_id: "R_0603".into(),
        position: Point::new(x, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads: vec![pad(n + 1, -2.0), pad(n + 2, 2.0)],
        graphics: Vec::new(),
        properties: vec![
            property("Reference", reference),
            property("Value", "10k"),
            property("MPN", "RC0603"),
        ],
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![footprint(R2, "R2", 10.0), footprint(C7, "C7", 20.0)],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
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
    docs.tab_board_engine(docs.active_tab)
        .map(|engine| engine.board())
        .expect("a board is open")
        .clone()
}

fn snapshot(app: &Signex) -> String {
    serde_json::to_string(&live(app)).unwrap()
}

fn fp(board: &PcbBoard, id: Uuid) -> Footprint {
    board
        .footprints
        .iter()
        .find(|f| f.uuid == id)
        .expect("footprint")
        .clone()
}

fn click(app: &mut Signex, x: f64, mode: ClickMode) {
    send(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(x, 10.0),
            tolerance_mm: 0.05,
            mode,
        }),
    );
}

/// Select R2 alone, by its body.
fn select_r2(app: &mut Signex) {
    click(app, 10.0, ClickMode::Replace);
    let selected: Vec<ItemRef> = app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(selected, [ItemRef::Footprint(R2)]);
}

fn facts(app: &Signex) -> PcbComponentFacts {
    app.document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.component.clone())
        .expect("the Component view")
}

fn panel(msg: PcbComponentMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::PcbComponent(msg)))
}

/// Type `text` into `field` of the selection, then press Enter.
fn type_and_enter(f: &mut Fixture, field: ComponentField, text: &str) {
    let footprints = facts(&f.app).footprints;
    send(
        &mut f.app,
        panel(PcbComponentMsg::FieldTyped {
            board: f.path.clone(),
            footprints: footprints.clone(),
            field: field.clone(),
            text: text.into(),
        }),
    );
    send(
        &mut f.app,
        panel(PcbComponentMsg::FieldSubmitted {
            board: f.path.clone(),
            footprints,
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

/// A notice logged after `since` that contains `wanted`.
fn noticed(since: u64, wanted: &str) -> bool {
    diagnostics::recent_entries().iter().any(|e| {
        e.id > since
            && e.message
                .replace(['\u{2068}', '\u{2069}'], "")
                .contains(wanted)
    })
}

#[test]
fn the_view_reads_the_selected_component() {
    let mut f = open_board();
    select_r2(&mut f.app);
    let shown = facts(&f.app);
    assert_eq!(shown.footprints, [R2]);
    // Y counts up from the board origin (#2384).
    assert_eq!(
        (shown.x, shown.y),
        (Shared::Same(10.0), Shared::Same(-10.0))
    );
    assert_eq!(shown.side, Shared::Same(BoardSide::Top));
    assert_eq!(shown.locked, Shared::Same(false));
    assert_eq!(shown.designator, Shared::Same("R2".to_string()));
    assert_eq!(shown.designator_shown, Some(Shared::Same(true)));
    assert_eq!(shown.parameters.len(), 1);
    assert_eq!(shown.parameters[0].key, "MPN");
}

#[test]
fn an_x_edit_moves_the_component_as_one_step() {
    let mut f = open_board();
    select_r2(&mut f.app);
    let x = ComponentField::Location(LocationField::X);
    send(
        &mut f.app,
        panel(PcbComponentMsg::FieldTyped {
            board: f.path.clone(),
            footprints: vec![R2],
            field: x.clone(),
            text: "12.7".into(),
        }),
    );
    assert_eq!(
        fp(&live(&f.app), R2).position.x,
        10.0,
        "typing alone edits nothing"
    );
    assert_eq!(facts(&f.app).draft(&x).map(String::as_str), Some("12.7"));
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            panel(PcbComponentMsg::FieldSubmitted {
                board: f.path.clone(),
                footprints: vec![R2],
                field: x.clone(),
            }),
        );
    });
    assert_eq!(fp(&edited, R2).position, Point::new(12.7, 10.0));
}

#[test]
fn a_rotation_edit_turns_the_component_as_one_step() {
    let mut f = open_board();
    select_r2(&mut f.app);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, ComponentField::Location(LocationField::Rotation), "90");
    });
    assert_eq!(fp(&edited, R2).rotation, 90.0);
    assert_eq!(fp(&edited, R2).position, Point::new(10.0, 10.0));
}

#[test]
fn picking_the_bottom_layer_flips_the_component_as_one_step() {
    let mut f = open_board();
    select_r2(&mut f.app);
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            panel(PcbComponentMsg::SidePicked {
                board: f.path.clone(),
                footprints: vec![R2],
                side: BoardSide::Bottom,
            }),
        );
    });
    assert_eq!(fp(&edited, R2).layer, "BottomCopper");
    assert_eq!(fp(&edited, R2).pads[0].layers, ["BottomCopper"]);
}

#[test]
fn the_designator_eye_hides_the_designator_as_one_step() {
    let mut f = open_board();
    select_r2(&mut f.app);
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            panel(PcbComponentMsg::TextShownToggled {
                board: f.path.clone(),
                footprints: vec![R2],
                field: FootprintTextField::Designator,
                shown: false,
            }),
        );
        assert_eq!(
            facts(&f.app).designator_shown,
            Some(Shared::Same(false)),
            "the eye closes"
        );
    });
    let r2 = fp(&edited, R2);
    assert!(r2.properties[0].hidden, "the Reference text is hidden");
    assert_eq!(r2.properties[0].value, "R2");
    assert!(!r2.properties[1].hidden, "the Value text stays");
}

/// The texts the board canvas draws (#2368).
fn drawn_texts(app: &Signex) -> Vec<String> {
    app.interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot")
        .texts
        .iter()
        .map(|t| t.content.clone())
        .collect()
}

#[test]
fn the_canvas_draws_the_designator_until_the_eye_hides_it() {
    let mut f = open_board();
    assert!(drawn_texts(&f.app).contains(&"R2".to_string()));
    select_r2(&mut f.app);
    send(
        &mut f.app,
        panel(PcbComponentMsg::TextShownToggled {
            board: f.path.clone(),
            footprints: vec![R2],
            field: FootprintTextField::Designator,
            shown: false,
        }),
    );
    let drawn = drawn_texts(&f.app);
    assert!(!drawn.contains(&"R2".to_string()), "{drawn:?}");
    assert!(drawn.contains(&"C7".to_string()), "C7's designator stays");
}

#[test]
fn a_flip_puts_the_designator_mirrored_on_the_bottom_overlay() {
    use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
    let mut f = open_board();
    select_r2(&mut f.app);
    send(
        &mut f.app,
        panel(PcbComponentMsg::SidePicked {
            board: f.path.clone(),
            footprints: vec![R2],
            side: BoardSide::Bottom,
        }),
    );
    let snapshot = f
        .app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot");
    let r2 = snapshot
        .texts
        .iter()
        .find(|t| t.content == "R2")
        .expect("R2 is drawn");
    assert!(r2.mirrored);
    assert_eq!(
        r2.layer,
        PcbLayer::Technical(TechnicalLayer::SilkscreenBottom)
    );
}

#[test]
fn the_lock_is_one_step_and_blocks_a_drag_and_an_edit() {
    let mut f = open_board();
    select_r2(&mut f.app);
    let lock = |f: &mut Fixture| {
        send(
            &mut f.app,
            panel(PcbComponentMsg::LockToggled {
                board: f.path.clone(),
                footprints: vec![R2],
                locked: true,
            }),
        );
    };
    let edited = assert_one_step(&mut f, lock);
    assert!(fp(&edited, R2).locked);
    lock(&mut f);
    assert_eq!(facts(&f.app).locked, Shared::Same(true));
    let before = snapshot(&f.app);

    let since = diagnostics::newest_entry_id().unwrap_or(0);
    send(
        &mut f.app,
        Message::PcbCanvas(PcbCanvasMsg::FootprintDropped {
            from: Point::new(10.0, 10.0),
            to: Point::new(15.0, 10.0),
        }),
    );
    assert_eq!(snapshot(&f.app), before, "a locked component stays put");
    assert!(noticed(since, "Move: R2 is locked"), "the drag says why");

    let since = diagnostics::newest_entry_id().unwrap_or(0);
    type_and_enter(&mut f, ComponentField::Location(LocationField::X), "30");
    assert_eq!(snapshot(&f.app), before, "an X edit is refused too");
    assert!(noticed(since, "R2 is locked"), "the X edit says why");
}

#[test]
fn several_components_share_values_and_take_one_x_as_one_step() {
    let mut f = open_board();
    select_r2(&mut f.app);
    click(&mut f.app, 20.0, ClickMode::Add);
    let shown = facts(&f.app);
    assert_eq!(shown.footprints, [R2, C7]);
    assert_eq!(shown.x, Shared::Mixed, "a differing X is blank");
    assert_eq!(shown.y, Shared::Same(-10.0), "Y up from the origin (#2384)");
    assert_eq!(shown.designator, Shared::Mixed);
    assert_eq!(shown.comment, Shared::Same("10k".to_string()));
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, ComponentField::Location(LocationField::X), "5");
    });
    assert_eq!(fp(&edited, R2).position, Point::new(5.0, 10.0));
    assert_eq!(fp(&edited, C7).position, Point::new(5.0, 10.0));
}

#[test]
fn a_parameter_edit_is_one_step() {
    let mut f = open_board();
    select_r2(&mut f.app);
    f.app.document_state.panel_ctx.properties_tab = 1;
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, ComponentField::Parameter("MPN".into()), "RC0805");
    });
    assert_eq!(fp(&edited, R2).properties[2].value, "RC0805");
    assert_eq!(fp(&edited, C7).properties[2].value, "RC0603");
}

#[test]
fn a_draft_for_another_selection_is_dropped() {
    let mut f = open_board();
    select_r2(&mut f.app);
    let x = ComponentField::Location(LocationField::X);
    send(
        &mut f.app,
        panel(PcbComponentMsg::FieldTyped {
            board: f.path.clone(),
            footprints: vec![R2],
            field: x.clone(),
            text: "99".into(),
        }),
    );
    click(&mut f.app, 20.0, ClickMode::Replace);
    assert!(f.app.interaction_state.pcb_component.draft.is_none());
    let before = snapshot(&f.app);
    send(
        &mut f.app,
        panel(PcbComponentMsg::FieldSubmitted {
            board: f.path.clone(),
            footprints: vec![R2],
            field: x,
        }),
    );
    assert_eq!(snapshot(&f.app), before, "Enter has nothing to apply");
}

#[test]
fn one_component_is_drawn_unturned_and_several_are_not() {
    let mut f = open_board();
    select_r2(&mut f.app);
    let drawing = facts(&f.app).drawing.expect("one component is drawn");
    assert_eq!(drawing.side, BoardSide::Top);
    assert_eq!(drawing.pads.len(), 2);
    assert_eq!(
        drawing.pin_one,
        Some(Point::new(-2.0, 0.0)),
        "pin 1 is the first pad numbered 1, at its place in the footprint"
    );
    assert!(drawing.courtyard.is_empty(), "R2 draws no courtyard");
    send(
        &mut f.app,
        panel(PcbComponentMsg::FieldTyped {
            board: f.path.clone(),
            footprints: vec![R2],
            field: ComponentField::Location(LocationField::Rotation),
            text: "90".into(),
        }),
    );
    send(
        &mut f.app,
        panel(PcbComponentMsg::FieldSubmitted {
            board: f.path.clone(),
            footprints: vec![R2],
            field: ComponentField::Location(LocationField::Rotation),
        }),
    );
    assert_eq!(fp(&live(&f.app), R2).rotation, 90.0);
    assert_eq!(
        facts(&f.app).drawing.and_then(|d| d.pin_one),
        Some(Point::new(-2.0, 0.0)),
        "the drawing shows no rotation"
    );
    click(&mut f.app, 20.0, ClickMode::Add);
    let shown = facts(&f.app);
    assert_eq!(shown.footprints, [R2, C7]);
    assert!(shown.drawing.is_none(), "no merged drawing of several");
}

#[test]
fn a_hovered_designator_row_is_kept_for_the_illustration_until_left() {
    let mut f = open_board();
    select_r2(&mut f.app);
    send(
        &mut f.app,
        panel(PcbComponentMsg::IllustrationHovered(
            IllustrationMsg::FieldHovered(DESIGNATOR_FIELD),
        )),
    );
    let shown = facts(&f.app);
    assert!(
        shown
            .illustration_hover
            .is_field_highlighted(&DESIGNATOR_FIELD),
        "the facts carry the hover to the view"
    );
    let before = snapshot(&f.app);
    send(
        &mut f.app,
        panel(PcbComponentMsg::IllustrationHovered(
            IllustrationMsg::FieldLeft(DESIGNATOR_FIELD),
        )),
    );
    assert_eq!(facts(&f.app).illustration_hover.hovered(), None);
    assert_eq!(snapshot(&f.app), before, "a hover edits nothing");
}
