//! #2552 — `set_object_position` takes `keep_wires` and `wiring` as operands
//! and bends wires exactly as the GUI drag commit of the same delta does;
//! neither is read from `UiState`, and a contact the guard cannot keep is
//! named in the answer.

use serde_json::Value;
use signex_types::corner::DragWiringMode;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, Wire,
};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::canvas::CanvasEvent;
use crate::keymap::AppCommandId;

type Key = (i64, i64);

/// The parts, wire segments and dots of a sheet, by position: two sheets
/// with the same shape compare equal whatever ids new wires got.
#[derive(Debug, PartialEq)]
struct Shape {
    parts: Vec<(String, Key)>,
    wires: Vec<(Key, Key)>,
    junctions: Vec<Key>,
}

fn shape(sheet: &SchematicSheet) -> Shape {
    let key = signex_net::pt_key;
    let mut parts: Vec<_> = (sheet.symbols.iter())
        .map(|s| (s.reference.clone(), key(&s.position)))
        .collect();
    let mut wires: Vec<_> = (sheet.wires.iter())
        .map(|w| {
            let (a, b) = (key(&w.start), key(&w.end));
            (a.min(b), a.max(b))
        })
        .collect();
    let mut junctions: Vec<_> = sheet.junctions.iter().map(|j| key(&j.position)).collect();
    parts.sort();
    wires.sort();
    junctions.sort();
    Shape {
        parts,
        wires,
        junctions,
    }
}

/// Which pins share a net: the netlist's shape, names aside.
fn nets(sheet: &SchematicSheet) -> Vec<Vec<(Uuid, String)>> {
    let mut nets: Vec<Vec<_>> = signex_net::net_signatures(sheet)
        .into_iter()
        .map(|net| net.terminals.into_iter().collect::<Vec<_>>())
        .filter(|terminals| !terminals.is_empty())
        .collect();
    nets.sort();
    nets
}

fn app_with(sheet: SchematicSheet) -> (Signex, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _task) = Signex::new();
    app.ui_state.snap_enabled = false;
    app.ui_state.snap_hotspots = false;
    add_sheet_with(&mut app, &dir.path().join("main.snxsch"), sheet);
    (app, dir)
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

fn part_at(app: &Signex, id: Uuid) -> Point {
    let part = doc(app).symbols.iter().find(|s| s.uuid == id);
    part.expect("the part").position
}

/// The GUI's drag commit of `id` by `(dx, dy)`.
fn drag(app: &mut Signex, id: Uuid, dx: f64, dy: f64) {
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Symbol)];
    let _ = app.update(crate::app::Message::CanvasEvent(
        CanvasEvent::MoveSelected {
            dx,
            dy,
            connections: Default::default(),
        },
    ));
}

/// The agent's `set_object_position` of `id` by `(dx, dy)`, with `extra`
/// operands; its answer.
fn agent_move(app: &mut Signex, id: Uuid, dx: f64, dy: f64, extra: &[&str]) -> Value {
    let from = part_at(app, id);
    let mut operands = vec![
        format!("id={id}"),
        format!("at={},{}", from.x + dx, from.y + dy),
    ];
    operands.extend(extra.iter().map(|s| (*s).to_string()));
    let command = AppCommandId::new("set_object_position").expect("a catalog id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).expect("operands");
    match app.run_operands_settled(&command, &args).0 {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn sample() -> SchematicSheet {
    SnxSchematic::parse(crate::sample_project::SHEET_SOURCE)
        .expect("sample parses")
        .sheet
}

fn u1(sheet: &SchematicSheet) -> Uuid {
    let part = sheet.symbols.iter().find(|s| s.reference == "U1");
    part.expect("U1 on the sample").uuid
}

/// The answer's `moved_wires` are exactly the wires that are new or got other
/// ends between `loaded` and `after`, its `removed_wires` the ones gone.
fn assert_wire_ids(answer: &Value, loaded: &[Wire], after: &[Wire]) {
    let ids = |key: &str| -> Vec<String> {
        let mut ids: Vec<String> = (answer[key].as_array().expect("a list").iter())
            .map(|id| id.as_str().expect("an id").to_string())
            .collect();
        ids.sort();
        ids
    };
    let same = |a: &Wire, b: &Wire| (a.start, a.end) == (b.start, b.end);
    let mut moved: Vec<String> = (after.iter())
        .filter(|w| !loaded.iter().any(|l| l.uuid == w.uuid && same(l, w)))
        .map(|w| w.uuid.to_string())
        .collect();
    let mut removed: Vec<String> = (loaded.iter())
        .filter(|l| !after.iter().any(|w| w.uuid == l.uuid))
        .map(|l| l.uuid.to_string())
        .collect();
    moved.sort();
    removed.sort();
    assert_eq!(ids("moved_wires"), moved, "{answer}");
    assert_eq!(ids("removed_wires"), removed, "{answer}");
}

const MODES: [(DragWiringMode, &str); 4] = [
    (DragWiringMode::Auto, "auto"),
    (DragWiringMode::HorizontalFirst, "horizontal_first"),
    (DragWiringMode::VerticalFirst, "vertical_first"),
    (DragWiringMode::Straight, "straight"),
];

/// A diagonal step of U1 on the linear-regulator sample, so each wire that
/// follows gets a corner and the modes bend it differently.
const DX: f64 = 2.54;
const DY: f64 = 5.08;

/// Each wiring mode: the agent's move of `id` by `(dx, dy)` leaves the same
/// sheet as the drag commit with that mode, wires follow and the nets keep
/// their shape. The agent's sheets, in `MODES` order.
fn every_mode_as_the_drag(sheet: &SchematicSheet, id: Uuid, dx: f64, dy: f64) -> Vec<Shape> {
    let before = nets(sheet);
    let mut shapes = Vec::new();
    for (mode, word) in MODES {
        let (mut gui, _g) = app_with(sheet.clone());
        gui.ui_state.move_keeps_wires = true;
        gui.interaction_state.drag_wiring = mode;
        drag(&mut gui, id, dx, dy);
        let (mut agent, _a) = app_with(sheet.clone());
        let loaded = doc(&agent).wires.clone();
        let wiring = format!("wiring={word}");
        let answer = agent_move(&mut agent, id, dx, dy, &[&wiring]);

        assert_eq!(shape(doc(&agent)), shape(doc(&gui)), "{word}");
        assert_ne!(shape(doc(&agent)).wires, shape(sheet).wires, "{word}");
        assert_eq!(nets(doc(&agent)), before, "{word}: the net shape");
        assert_wire_ids(&answer, &loaded, &doc(&agent).wires);
        assert!(
            !answer["moved_wires"].as_array().expect("list").is_empty(),
            "{word}: {answer}"
        );
        assert_eq!(answer["fallbacks"], serde_json::json!([]), "{word}");
        shapes.push(shape(doc(&agent)));
    }
    shapes
}

#[test]
fn an_agent_move_bends_wires_as_the_drag_commit_does_in_every_mode() {
    let sheet = sample();
    let shapes = every_mode_as_the_drag(&sheet, u1(&sheet), DX, DY);
    assert!(
        shapes.iter().any(|s| *s != shapes[0]),
        "the modes bend the same move differently"
    );
}

/// R1 with a vertical wire into pin 1 and a horizontal one into pin 2:
/// `auto` keeps each wire's own axis, so it bends unlike every fixed mode
/// and each `wiring` word is pinned to its own sheet.
#[test]
fn each_wiring_word_gives_its_own_sheet() {
    let (sheet, r1) = resistor_with(vec![
        wire((45.0, 30.0), (45.0, 50.0)),
        wire((55.0, 50.0), (70.0, 50.0)),
    ]);
    let shapes = every_mode_as_the_drag(&sheet, r1, 10.0, -10.0);
    for (i, (_, a)) in MODES.iter().enumerate() {
        for (j, (_, b)) in MODES.iter().enumerate().skip(i + 1) {
            assert_ne!(shapes[i], shapes[j], "{a} and {b} bend alike");
        }
    }
}

/// `keep_wires=false` is a plain move, as the drag with the preference off.
#[test]
fn keep_wires_false_leaves_the_wires_as_they_were() {
    let sheet = sample();
    let id = u1(&sheet);
    let (mut gui, _g) = app_with(sheet.clone());
    gui.ui_state.move_keeps_wires = false;
    drag(&mut gui, id, DX, DY);
    let (mut agent, _a) = app_with(sheet.clone());
    let loaded = doc(&agent).wires.clone();
    let answer = agent_move(&mut agent, id, DX, DY, &["keep_wires=false"]);

    assert_eq!(doc(&agent).wires, loaded);
    assert_eq!(shape(doc(&agent)), shape(doc(&gui)));
    assert_eq!(answer["moved_wires"], serde_json::json!([]));
    assert_eq!(answer["removed_wires"], serde_json::json!([]));
    let moved = part_at(&agent, id);
    assert_eq!(signex_net::pt_key(&moved), {
        let from = sheet.symbols.iter().find(|s| s.uuid == id).expect("U1");
        signex_net::pt_key(&Point::new(from.position.x + DX, from.position.y + DY))
    });
}

/// The person's "keep wires" preference and drag wiring mode do not reach an
/// agent's move: left out, wires follow with `auto`.
#[test]
fn the_persons_move_settings_do_not_change_an_agent_move() {
    let sheet = sample();
    let id = u1(&sheet);
    let (mut gui, _g) = app_with(sheet.clone());
    gui.ui_state.move_keeps_wires = true;
    gui.interaction_state.drag_wiring = DragWiringMode::Auto;
    drag(&mut gui, id, DX, DY);
    let (mut agent, _a) = app_with(sheet.clone());
    agent.ui_state.move_keeps_wires = false;
    agent.interaction_state.drag_wiring = DragWiringMode::Straight;
    let loaded = doc(&agent).wires.clone();
    agent_move(&mut agent, id, DX, DY, &[]);

    assert_eq!(shape(doc(&agent)), shape(doc(&gui)));
    assert_ne!(doc(&agent).wires, loaded, "wires followed");
}

#[test]
fn an_unknown_wiring_word_is_refused_with_the_choices() {
    let sheet = sample();
    let id = u1(&sheet);
    let (mut app, _d) = app_with(sheet.clone());
    let command = AppCommandId::new("set_object_position").expect("a catalog id");
    let operands = [
        format!("id={id}"),
        "at=1,1".into(),
        "wiring=diagonal".into(),
    ];
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).expect("operands");
    let reason = match app.run_operands_settled(&command, &args).0 {
        CommandOutcome::Disabled { reason } => {
            signex_i18n::RenderTranslatable::render_en_us(&reason)
        }
        other => panic!("not refused: {other:?}"),
    };
    assert!(reason.contains("horizontal_first"), "{reason}");
    assert!(reason.contains("`diagonal`"), "{reason}");
    assert_eq!(doc(&app).symbols, sheet.symbols, "nothing moved");
}

fn lib_pin(number: &str, x: f64) -> LibPin {
    LibPin {
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
    }
}

fn wire(a: (f64, f64), b: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        ..Default::default()
    }
}

/// R1 at (50, 50), pins at (45, 50) and (55, 50), a wire into pin 1 and
/// foreign wire ends on both corners a 10 mm drop could take: the engine's
/// "every route collides" case.
fn blocked_resistor() -> (SchematicSheet, Uuid) {
    resistor_with(vec![
        wire((30.0, 50.0), (45.0, 50.0)),
        wire((45.0, 55.0), (45.0, 58.0)),
        wire((30.0, 55.0), (30.0, 58.0)),
    ])
}

/// R1 at (50, 50), pins at (45, 50) and (55, 50), with `wires`.
fn resistor_with(wires: Vec<Wire>) -> (SchematicSheet, Uuid) {
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    sheet.lib_symbols.insert(
        "R".into(),
        LibSymbol {
            id: "R".into(),
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
            graphics: Vec::new(),
            pins: vec![lib_pin("1", -5.0), lib_pin("2", 5.0)],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
    let r1 = Uuid::now_v7();
    sheet.symbols.push(Symbol {
        uuid: r1,
        lib_id: "R".into(),
        reference: "R1".into(),
        position: Point::new(50.0, 50.0),
        ..Default::default()
    });
    sheet.wires = wires;
    (sheet, r1)
}

/// A contact the guard cannot keep falls back to a plain move, and the
/// answer names it with its Messages line; the wires are as they were.
#[test]
fn a_contact_that_cannot_be_kept_is_named_in_the_answer() {
    let (sheet, r1) = blocked_resistor();
    let (mut plain, _p) = app_with(sheet.clone());
    agent_move(&mut plain, r1, 0.0, 10.0, &["keep_wires=false"]);
    let (mut app, _d) = app_with(sheet);
    let loaded = doc(&app).wires.clone();
    let answer = agent_move(&mut app, r1, 0.0, 10.0, &[]);

    assert_eq!(doc(&app).wires, loaded, "a plain move");
    assert_eq!(
        nets(doc(&app)),
        nets(doc(&plain)),
        "the plain move's netlist"
    );
    assert_eq!(part_at(&app, r1), Point::new(50.0, 60.0));
    let fallbacks = answer["fallbacks"].as_array().expect("list");
    assert_eq!(fallbacks.len(), 1, "{answer}");
    assert_eq!(fallbacks[0]["kind"], "pins_not_kept");
    assert_eq!(fallbacks[0]["names"], serde_json::json!(["R1 pin 1"]));
    let message = fallbacks[0]["message"].as_str().expect("a line");
    assert!(message.contains("R1 pin 1"), "{message}");
}

/// A move that folds a following wire to nothing removes it, as the drag
/// commit does, and the answer names it in `removed_wires`.
#[test]
fn a_wire_the_move_folds_away_is_named_as_removed() {
    let (sheet, r1) = resistor_with(vec![
        wire((45.0, 40.0), (45.0, 50.0)),
        wire((30.0, 40.0), (45.0, 40.0)),
    ]);
    let (mut gui, _g) = app_with(sheet.clone());
    gui.ui_state.move_keeps_wires = true;
    drag(&mut gui, r1, 0.0, -10.0);
    let (mut agent, _a) = app_with(sheet);
    let loaded = doc(&agent).wires.clone();
    let answer = agent_move(&mut agent, r1, 0.0, -10.0, &[]);

    assert_eq!(shape(doc(&agent)), shape(doc(&gui)));
    assert_wire_ids(&answer, &loaded, &doc(&agent).wires);
    let removed = answer["removed_wires"].as_array().expect("a list");
    assert!(!removed.is_empty(), "{answer}");
}
