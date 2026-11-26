//! Every selectable sheet object moves by press, drag and release (#2870):
//! the real canvas program turns the pointer events into messages and
//! `Signex::update` handles them. The drop is the Move the engine makes for
//! the same selection and snapped delta (wires kept as a Move keeps them),
//! and one Undo puts the sheet back.

use iced::widget::canvas::Program as _;
use iced::{Event, Rectangle, mouse};
use signex_types::schematic::{
    BlanketPour, ChildSheet, DimensionDirection, FillType, Graphic, HAlign, HarnessConnector,
    HarnessEntry, Junction, Label, LabelType, LibGraphic, LibPin, LibSymbol, NoConnect,
    NoErcDirective, Pin, PinDirection, PinShapeStyle, Point, SchBezier, SchDimension, SchDirective,
    SchDrawing, SchTable, SchematicSheet, SelectedItem, SelectedKind, SheetPin, Symbol,
    SymbolTransform, TextNote, VAlign, Wire,
};

use super::super::super::*;
use super::placement_tests::{doc, grid, shape_of, sheet_shape};
use crate::canvas::{CanvasEvent, CanvasState, SchematicCanvas};

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 1200.0,
    height: 900.0,
};

/// The drag, in grid steps: 10 right and 5 down.
const STEPS: (f64, f64) = (10.0, 5.0);

fn id(n: u128) -> uuid::Uuid {
    uuid::Uuid::from_u128(n)
}

/// A snapping app showing `sheet`, nothing selected, the Select tool on.
fn app_with_sheet(sheet: SchematicSheet) -> Signex {
    let _ = crate::diagnostics::init_logging();
    let path = std::path::PathBuf::from("drag-every-kind.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    app.ui_state.snap_hotspots = false;
    app.ui_state.move_keeps_wires = true;
    let cache = crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet);
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "drag-every-kind".to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.current_tool = Tool::Select;
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(cache));
    app
}

fn screen(app: &Signex, at: Point) -> iced::Point {
    let (ox, oy, scale) = app.interaction_state.active_canvas().live_camera();
    iced::Point::new(at.x as f32 * scale + ox, at.y as f32 * scale + oy)
}

/// Deliver one pointer event to the canvas program at `at` and hand the
/// message it publishes to `Signex::update`, as the runtime does.
fn deliver(app: &mut Signex, state: &mut CanvasState, event: Event, at: Point) -> Option<Message> {
    let window = iced::window::Id::unique();
    let cursor = mouse::Cursor::Available(screen(app, at));
    let message = {
        let view = SchematicCanvas::new(
            app.interaction_state.active_canvas(),
            app.canvas_view_prefs(window),
        );
        view.update(state, &event, BOUNDS, cursor)
            .and_then(|action| action.into_inner().0)
    };
    if let Some(message) = &message {
        let _ = app.update(message.clone());
    }
    message
}

fn moved_to(app: &Signex, at: Point) -> Event {
    Event::Mouse(mouse::Event::CursorMoved {
        position: screen(app, at),
    })
}

/// A point the topmost hit of which is `target`, with a margin, searched
/// within 20 mm of `near`.
fn grab_point(app: &Signex, target: SelectedItem, near: Point) -> Point {
    let snapshot = app.active_render_snapshot().expect("snapshot");
    let designators = &app.interaction_state.active_canvas().designators;
    let hits = |x: f64, y: f64| {
        crate::schematic_runtime::hit_test::hit_test(snapshot, designators, x, y) == Some(target)
    };
    for ring in 0..=80 {
        let r = ring as f64 * 0.25;
        for (sx, sy) in [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0), (1.0, 1.0)] {
            let (x, y) = (near.x + sx * r, near.y + sy * r);
            let margin = 0.1;
            if hits(x, y)
                && hits(x + margin, y)
                && hits(x - margin, y)
                && hits(x, y + margin)
                && hits(x, y - margin)
            {
                return Point::new(x, y);
            }
        }
    }
    panic!("no point near {near:?} lands on {target:?}");
}

/// Press on `target`, drag it [`STEPS`] grid steps, release; check the drop
/// is the Move of the selection by the snapped delta, and that one Undo
/// restores the sheet. Returns the sheet after the drag.
fn assert_drags(mut app: Signex, target: SelectedItem, near: Point) -> SchematicSheet {
    let before = doc(&app).clone();
    let grab = grab_point(&app, target, near);
    let g = crate::canvas::grid::pitch_mm_exact(1.27);
    let to = Point::new(grab.x + STEPS.0 * g, grab.y + STEPS.1 * g);

    let mut state = CanvasState::default();
    let event = moved_to(&app, grab);
    let _ = deliver(&mut app, &mut state, event, grab);
    let left = mouse::Button::Left;
    let _ = deliver(
        &mut app,
        &mut state,
        Event::Mouse(mouse::Event::ButtonPressed(left)),
        grab,
    );
    let selection = app.interaction_state.active_canvas().selected.clone();
    assert_eq!(
        selection,
        vec![target],
        "the press selects what it lands on"
    );
    for step in 1..=4 {
        let f = step as f64 / 4.0;
        let at = Point::new(grab.x + (to.x - grab.x) * f, grab.y + (to.y - grab.y) * f);
        let event = moved_to(&app, at);
        let _ = deliver(&mut app, &mut state, event, at);
    }
    let dropped = deliver(
        &mut app,
        &mut state,
        Event::Mouse(mouse::Event::ButtonReleased(left)),
        to,
    );
    let (dx, dy, connections) = match dropped {
        Some(Message::CanvasEvent(CanvasEvent::MoveSelected {
            dx,
            dy,
            connections,
        })) => (dx, dy, connections),
        other => panic!("{target:?}: the release did not drop a move: {other:?}"),
    };

    // The Move the app makes of this selection: the same snap, wire rule
    // and engine command.
    let (sdx, sdy) = crate::canvas::drag_preview::snapped_move_delta(
        Some(&before),
        &selection,
        dx,
        dy,
        true,
        g,
        None,
    );
    assert!(
        (sdx.abs() - STEPS.0 * g).abs() < 1.27 && (sdy.abs() - STEPS.1 * g).abs() < 1.27,
        "{target:?}: snapped to about the drag: ({sdx}, {sdy})"
    );
    let mut expected = signex_engine::Engine::new(before.clone()).expect("engine");
    expected
        .execute(signex_engine::Command::MoveSelection {
            items: selection,
            dx: sdx,
            dy: sdy,
            keep_wires: connections.keeps_wires(app.ui_state.move_keeps_wires),
            wiring: app.interaction_state.drag_wiring,
        })
        .expect("the move runs");

    let after = doc(&app).clone();
    assert_ne!(
        shape_of(&after),
        shape_of(&before),
        "{target:?} did not move"
    );
    assert_eq!(
        shape_of(&after),
        shape_of(expected.document()),
        "{target:?}: the drag is not the Move"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        sheet_shape(&app),
        shape_of(&before),
        "{target:?}: one Undo puts the sheet back"
    );
    after
}

fn sheet() -> SchematicSheet {
    SchematicSheet::default()
}

fn resistor_lib() -> LibSymbol {
    let pin = |number: &str, x: f64| LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    };
    LibSymbol {
        id: "R".to_string(),
        reference: String::new(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: vec![LibGraphic {
            unit: 0,
            body_style: 1,
            graphic: Graphic::Rectangle {
                start: Point::new(-3.81, -2.54),
                end: Point::new(3.81, 2.54),
                width: 0.25,
                fill: FillType::Background,
            },
        }],
        pins: vec![pin("1", -5.08), pin("2", 5.08)],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn at() -> Point {
    grid(40.0, 40.0)
}

fn drag_one(sheet: SchematicSheet, item: SelectedItem) {
    assert_drags(app_with_sheet(sheet), item, at());
}

/// A part with a wire on each pin: the wires stay on the pins.
#[test]
fn a_part_drags_and_keeps_its_wires() {
    let mut s = sheet();
    s.lib_symbols.insert("R".to_string(), resistor_lib());
    let part = Symbol {
        uuid: id(1),
        lib_id: "R".to_string(),
        reference: "R1".to_string(),
        position: at(),
        unit: 1,
        ..Default::default()
    };
    let tips: Vec<Point> = [-5.08, 5.08]
        .into_iter()
        .map(|x| SymbolTransform::from_symbol(&part).apply(Point::new(x, 0.0)))
        .collect();
    for (n, (tip, out)) in tips.iter().zip([-10.16, 10.16]).enumerate() {
        s.wires.push(Wire {
            uuid: id(10 + n as u128),
            start: *tip,
            end: Point::new(tip.x + out, tip.y),
            ..Default::default()
        });
    }
    s.symbols.push(part);
    let after = assert_drags(
        app_with_sheet(s),
        SelectedItem::new(id(1), SelectedKind::Symbol),
        at(),
    );
    let moved = after.symbols[0].clone();
    for x in [-5.08, 5.08] {
        let tip = SymbolTransform::from_symbol(&moved).apply(Point::new(x, 0.0));
        let near = |p: Point| (p.x - tip.x).abs() < 1e-6 && (p.y - tip.y).abs() < 1e-6;
        assert!(
            after.wires.iter().any(|w| near(w.start) || near(w.end)),
            "a wire still ends on the pin at {tip:?}: {:?}",
            after.wires
        );
    }
}

#[test]
fn a_power_port_drags() {
    let mut app = app_with_sheet(sheet());
    assert!(app.place_power_at(at(), "VCC", "power:VCC"));
    let port = doc(&app).symbols[0].uuid;
    assert_drags(app, SelectedItem::new(port, SelectedKind::Symbol), at());
}

#[test]
fn a_wire_drags() {
    let mut s = sheet();
    s.wires.push(Wire {
        uuid: id(1),
        start: grid(30.0, 40.0),
        end: grid(50.0, 40.0),
        ..Default::default()
    });
    drag_one(s, SelectedItem::new(id(1), SelectedKind::Wire));
}

#[test]
fn a_bus_drags() {
    let mut s = sheet();
    s.buses.push(signex_types::schematic::Bus {
        uuid: id(1),
        start: grid(30.0, 40.0),
        end: grid(50.0, 40.0),
        ..Default::default()
    });
    drag_one(s, SelectedItem::new(id(1), SelectedKind::Bus));
}

#[test]
fn a_bus_entry_drags() {
    let mut s = sheet();
    s.bus_entries.push(signex_types::schematic::BusEntry {
        uuid: id(1),
        position: at(),
        size: (2.54, 2.54),
    });
    drag_one(s, SelectedItem::new(id(1), SelectedKind::BusEntry));
}

#[test]
fn net_global_and_hierarchical_labels_drag() {
    for kind in [LabelType::Net, LabelType::Global, LabelType::Hierarchical] {
        let mut s = sheet();
        s.labels.push(Label {
            uuid: id(1),
            ..Label::new("SDA", at(), kind)
        });
        drag_one(s, SelectedItem::new(id(1), SelectedKind::Label));
    }
}

fn sheet_symbol() -> ChildSheet {
    let corner = grid(30.0, 30.0);
    ChildSheet {
        uuid: id(1),
        name: "Child".into(),
        filename: "child.snxsch".into(),
        position: corner,
        size: (25.4, 25.4),
        stroke_width: 0.12,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: id(2),
            name: "IN".into(),
            direction: "input".into(),
            position: Point::new(corner.x, corner.y + 12.7),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    }
}

#[test]
fn a_sheet_symbol_drags_with_its_entries() {
    let mut s = sheet();
    s.child_sheets.push(sheet_symbol());
    let centre = Point::new(grid(30.0, 30.0).x + 15.0, grid(30.0, 30.0).y + 15.0);
    let after = assert_drags(
        app_with_sheet(s),
        SelectedItem::new(id(1), SelectedKind::ChildSheet),
        centre,
    );
    let child = &after.child_sheets[0];
    assert_eq!(
        child.pins[0].position.x, child.position.x,
        "the entry stays on the sheet symbol's edge"
    );
}

/// An entry slides along its sheet symbol's edge, as a Move of it does.
#[test]
fn a_sheet_entry_drags() {
    let mut s = sheet();
    s.child_sheets.push(sheet_symbol());
    let pin = s.child_sheets[0].pins[0].position;
    drag_one_near(s, SelectedItem::new(id(2), SelectedKind::SheetPin), pin);
}

fn drag_one_near(sheet: SchematicSheet, item: SelectedItem, near: Point) {
    assert_drags(app_with_sheet(sheet), item, near);
}

#[test]
fn a_junction_drags() {
    let mut s = sheet();
    s.junctions.push(Junction {
        uuid: id(1),
        position: at(),
        ..Default::default()
    });
    drag_one(s, SelectedItem::new(id(1), SelectedKind::Junction));
}

#[test]
fn a_no_connect_drags() {
    let mut s = sheet();
    s.no_connects.push(NoConnect {
        uuid: id(1),
        position: at(),
    });
    drag_one(s, SelectedItem::new(id(1), SelectedKind::NoConnect));
}

#[test]
fn a_no_erc_directive_drags() {
    let mut s = sheet();
    s.no_erc_directives.push(NoErcDirective {
        uuid: id(1),
        position: at(),
        rules: Vec::new(),
    });
    drag_one(s, SelectedItem::new(id(1), SelectedKind::NoErcDirective));
}

#[test]
fn a_harness_connector_drags() {
    let mut s = sheet();
    s.harness.connectors.push(HarnessConnector {
        uuid: id(1),
        harness_type: "SPI_BUS".into(),
        position: at(),
        width: 10.16,
        entry_pitch: 2.54,
        rotation: 0.0,
        entries: vec![HarnessEntry {
            uuid: id(2),
            name: "SCK".into(),
        }],
        locked: false,
    });
    drag_one(s, SelectedItem::new(id(1), SelectedKind::HarnessConnector));
}

/// A drag on an entry slides it along its connector's side, its wire kept
/// on it (docs/DECISIONS.md, 2026-11-24).
#[test]
fn a_harness_entry_slides_along_its_connector() {
    let mut s = sheet();
    let connector = HarnessConnector {
        uuid: id(1),
        harness_type: "SPI_BUS".into(),
        position: at(),
        width: 10.16,
        entry_pitch: 2.54,
        rotation: 0.0,
        entries: ["SCK", "MOSI", "MISO"]
            .iter()
            .enumerate()
            .map(|(i, name)| HarnessEntry {
                uuid: id(10 + i as u128),
                name: (*name).into(),
            })
            .collect(),
        locked: false,
    };
    let first = connector.entry_point(0);
    s.wires.push(Wire {
        uuid: id(20),
        start: Point::new(first.x - 10.16, first.y),
        end: first,
        ..Default::default()
    });
    s.harness.connectors.push(connector);
    let after = assert_drags(
        app_with_sheet(s),
        SelectedItem::new(id(10), SelectedKind::HarnessEntry),
        first,
    );
    let moved = &after.harness.connectors[0];
    let names: Vec<&str> = moved.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["MOSI", "MISO", "SCK"], "SCK slid to the last slot");
    let wire = after.wires.iter().find(|w| w.uuid == id(20)).expect("wire");
    let sck = moved.entry_point(2);
    assert!(
        (wire.end.x - sck.x).abs() < 1e-9 && (wire.end.y - sck.y).abs() < 1e-9,
        "the wire stays on SCK: {wire:?}"
    );
}

#[test]
fn a_text_note_drags() {
    let mut s = sheet();
    s.text_notes.push(TextNote {
        uuid: id(1),
        text: "Note".into(),
        position: at(),
        rotation: 0.0,
        font_size: 2.54,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
    });
    drag_one(s, SelectedItem::new(id(1), SelectedKind::TextNote));
}

fn drag_drawing(drawing: SchDrawing) {
    let mut s = sheet();
    let item = SelectedItem::new(drawing.uuid(), SelectedKind::Drawing);
    s.drawings.push(drawing);
    drag_one(s, item);
}

#[test]
fn every_graphic_drags() {
    let (a, b) = (grid(30.0, 30.0), grid(50.0, 46.0));
    drag_drawing(SchDrawing::Line {
        uuid: id(1),
        start: a,
        end: b,
        width: 0.25,
        stroke_color: None,
        locked: false,
    });
    drag_drawing(SchDrawing::Rect {
        uuid: id(1),
        start: a,
        end: b,
        width: 0.25,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    });
    drag_drawing(SchDrawing::Circle {
        uuid: id(1),
        center: at(),
        radius: 7.62,
        width: 0.25,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    });
    drag_drawing(SchDrawing::Arc {
        uuid: id(1),
        start: grid(34.0, 40.0),
        mid: grid(40.0, 34.0),
        end: grid(46.0, 40.0),
        width: 0.25,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    });
    drag_drawing(SchDrawing::Polyline {
        uuid: id(1),
        points: vec![a, grid(40.0, 46.0), b],
        width: 0.25,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    });
    drag_drawing(SchDrawing::Bezier(SchBezier::new(
        id(1),
        a,
        grid(36.0, 30.0),
        grid(44.0, 46.0),
        b,
    )));
}

#[test]
fn a_dimension_drags() {
    drag_drawing(SchDrawing::Dimension(SchDimension::new(
        id(1),
        grid(30.0, 40.0),
        grid(50.0, 40.0),
        5.08,
        DimensionDirection::Horizontal,
    )));
}

#[test]
fn a_table_drags() {
    drag_drawing(SchDrawing::Table(SchTable::new_typed(id(1), at())));
}

#[test]
fn every_directive_drags() {
    let (a, b) = (grid(30.0, 30.0), grid(50.0, 46.0));
    // A region is searched for a grab point from the middle of its left edge.
    let edge = Point::new(a.x, (a.y + b.y) / 2.0);
    for (directive, near) in [
        (
            SchDirective::Blanket {
                uuid: id(1),
                start: a,
                end: b,
                net_class: Some("Power".into()),
                erc_exclusions: Vec::new(),
                track_width: None,
                pour: BlanketPour::Normal,
                include_edge_nets: false,
            },
            edge,
        ),
        (
            SchDirective::CompileMask {
                uuid: id(1),
                start: a,
                end: b,
            },
            edge,
        ),
        (
            SchDirective::DiffPair {
                uuid: id(1),
                position: at(),
                class: None,
            },
            at(),
        ),
        (
            SchDirective::NetParameters {
                uuid: id(1),
                position: at(),
                net_class: Some("Power".into()),
                diff_pair_class: None,
                rules: Default::default(),
                show_on_flag: false,
            },
            at(),
        ),
    ] {
        let mut s = sheet();
        s.directives.push(directive);
        let item = SelectedItem::new(id(1), SelectedKind::Directive);
        drag_one_near(s, item, near);
    }
}
