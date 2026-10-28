//! #1039 — rotate (Space / Shift+Space) and mirror (X / Y) work on any
//! schematic selection, not only a lone symbol. Driven through the real
//! keymap path (`UiMsg::KeymapStroke`, Altium profile) against a real
//! engine; geometry is asserted on screen, independent of the engine's own
//! maths.
//!
//! Screen space is y-down. Counter-clockwise turns `(x, y)` into `(y, -x)`,
//! clockwise into `(-y, x)`. The `X` key flips left-right, `Y` flips
//! top-bottom.

use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind, UiMsg};
use signex_app::keymap::{KeyStroke, KeyToken, Modifiers};
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Label, LabelType, Point, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, SymbolTransform, TextNote, VAlign, Wire,
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
enum Op {
    Ccw,
    Cw,
    FlipLeftRight,
    FlipTopBottom,
}

const OPS: [Op; 4] = [Op::Ccw, Op::Cw, Op::FlipLeftRight, Op::FlipTopBottom];

impl Op {
    fn vec(self, (x, y): (f64, f64)) -> (f64, f64) {
        match self {
            Op::Ccw => (y, -x),
            Op::Cw => (-y, x),
            Op::FlipLeftRight => (-x, y),
            Op::FlipTopBottom => (x, -y),
        }
    }

    fn stroke(self) -> KeyStroke {
        let (key, shift) = match self {
            Op::Ccw => (KeyToken::Space, false),
            Op::Cw => (KeyToken::Space, true),
            Op::FlipLeftRight => (KeyToken::Character("x".into()), false),
            Op::FlipTopBottom => (KeyToken::Character("y".into()), false),
        };
        KeyStroke {
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
            key,
        }
    }
}

struct Ids {
    symbol: SelectedItem,
    wire: SelectedItem,
    label: SelectedItem,
    flag: SelectedItem,
    note: SelectedItem,
}

/// Pin tip of the symbol in its own (library) frame.
const PIN: (f64, f64) = (2.0, 0.0);

fn symbol(reference: &str, at: (f64, f64), mirror_x: bool, mirror_y: bool) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        position: Point::new(at.0, at.1),
        mirror_x,
        mirror_y,
        ..Default::default()
    }
}

fn pin_world(symbol: &Symbol) -> (f64, f64) {
    let p = SymbolTransform::from_symbol(symbol).apply(Point::new(PIN.0, PIN.1));
    (p.x, p.y)
}

fn label(text: &str, label_type: LabelType, at: (f64, f64)) -> Label {
    Label {
        uuid: uuid::Uuid::new_v4(),
        text: text.to_string(),
        position: Point::new(at.0, at.1),
        rotation: 0.0,
        label_type,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        font_family: None,
        unknown_columns: Default::default(),
    }
}

/// R1 at (10,10) with a wire from its pin tip to (20,10), a net label at
/// (20,16), a global label at (10,16) and a text note at (14,16). The
/// bounding box is (10,10)-(20,16), centre (15,13): on a 1 mm grid.
fn mixed_sheet(r1_mirror: (bool, bool)) -> (SchematicSheet, Ids) {
    let r1 = symbol("R1", (10.0, 10.0), r1_mirror.0, r1_mirror.1);
    let pin = pin_world(&r1);
    let wire = Wire {
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(pin.0, pin.1),
        end: Point::new(20.0, 10.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
    };
    let net = label("NET", LabelType::Net, (20.0, 16.0));
    let flag = label("FLAG", LabelType::Global, (10.0, 16.0));
    let note = TextNote {
        uuid: uuid::Uuid::new_v4(),
        text: "note".to_string(),
        position: Point::new(14.0, 16.0),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        font_family: None,
    };
    let ids = Ids {
        symbol: SelectedItem::new(r1.uuid, SelectedKind::Symbol),
        wire: SelectedItem::new(wire.uuid, SelectedKind::Wire),
        label: SelectedItem::new(net.uuid, SelectedKind::Label),
        flag: SelectedItem::new(flag.uuid, SelectedKind::Label),
        note: SelectedItem::new(note.uuid, SelectedKind::TextNote),
    };
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![r1],
        wires: vec![wire],
        labels: vec![net, flag],
        text_notes: vec![note],
        ..Default::default()
    };
    (sheet, ids)
}

fn app_with(sheet: SchematicSheet, selected: Vec<SelectedItem>) -> Signex {
    let path = PathBuf::from("rotate-mirror-selection.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _t) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.grid_size_mm = 1.0;
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "rotate-mirror-selection".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.active_canvas_mut().selected = selected;
    app
}

fn mixed_app(r1_mirror: (bool, bool)) -> (Signex, Ids) {
    let (sheet, ids) = mixed_sheet(r1_mirror);
    let selected = vec![ids.symbol, ids.wire, ids.label, ids.flag, ids.note];
    (app_with(sheet, selected), ids)
}

fn press(app: &mut Signex, op: Op) {
    let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: op.stroke(),
    }));
}

fn doc(app: &Signex) -> SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .clone()
}

fn close(a: (f64, f64), b: (f64, f64), what: &str) {
    assert!(
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9,
        "{what}: got {a:?}, expected {b:?}"
    );
}

/// Centre of the mixed selection's bounding box: x runs from the leftmost
/// of R1 and its pin tip to the wire end at 20, y from 10 to the labels at
/// 16. Integral for every mirror combination, so the grid snap is a no-op.
fn centre_of(mirror: (bool, bool)) -> (f64, f64) {
    let (sheet, _) = mixed_sheet(mirror);
    let pin = pin_world(&sheet.symbols[0]);
    ((10.0_f64.min(pin.0) + 20.0) / 2.0, 13.0)
}

fn about_centre(centre: (f64, f64), op: Op, p: (f64, f64)) -> (f64, f64) {
    let v = op.vec((p.0 - centre.0, p.1 - centre.1));
    (centre.0 + v.0, centre.1 + v.1)
}

/// The two directions that define how a net label / text reads: the way the
/// text runs from its anchor, and the side of the baseline its body is on.
/// Both on screen (y-down), from rotation (clockwise degrees) + justify.
fn text_frame(rotation: f64, h: HAlign, v: VAlign) -> ((f64, f64), (f64, f64)) {
    let (s, c) = rotation.to_radians().sin_cos();
    let run = match h {
        HAlign::Left => 1.0,
        HAlign::Right => -1.0,
        HAlign::Center => 0.0,
    };
    let side = match v {
        VAlign::Top => 1.0,
        VAlign::Bottom => -1.0,
        VAlign::Center => 0.0,
    };
    ((run * c, run * s), (side * -s, side * c))
}

fn assert_text_followed(op: Op, before: (f64, HAlign, VAlign), after: (f64, HAlign, VAlign)) {
    let (run0, side0) = text_frame(before.0, before.1, before.2);
    let (run1, side1) = text_frame(after.0, after.1, after.2);
    close(run1, op.vec(run0), &format!("{op:?}: text run direction"));
    close(side1, op.vec(side0), &format!("{op:?}: text body side"));
    // Readable: running left-to-right, or vertical reading bottom-to-top
    // (rotation 270 in this y-down, clockwise-positive frame).
    let (s, c) = after.0.to_radians().sin_cos();
    assert!(
        c > 1e-9 || (c.abs() <= 1e-9 && s < 0.0),
        "{op:?}: text left reading backwards or upside down (rotation {})",
        after.0
    );
}

#[test]
fn mixed_selection_turns_about_the_centre_and_the_wire_stays_on_the_pin() {
    for mirror in [(false, false), (true, false), (false, true), (true, true)] {
        for op in OPS {
            let (mut app, ids) = mixed_app(mirror);
            let before = doc(&app);
            press(&mut app, op);
            let after = doc(&app);
            let what = format!("{op:?}, R1 mirror {mirror:?}");
            let centre = centre_of(mirror);

            let r1 = &after.symbols[0];
            close(
                (r1.position.x, r1.position.y),
                about_centre(centre, op, (10.0, 10.0)),
                &format!("{what}: symbol position"),
            );
            let wire = &after.wires[0];
            close(
                (wire.start.x, wire.start.y),
                pin_world(r1),
                &format!("{what}: wire start stays on the pin"),
            );
            close(
                (wire.end.x, wire.end.y),
                about_centre(centre, op, (20.0, 10.0)),
                &format!("{what}: wire end"),
            );

            // The pin direction follows the same on-screen turn.
            let dir = |s: &Symbol| {
                let p = pin_world(s);
                (p.0 - s.position.x, p.1 - s.position.y)
            };
            close(
                dir(r1),
                op.vec(dir(&before.symbols[0])),
                &format!("{what}: pin direction"),
            );

            let net_before = &before.labels[0];
            let net = &after.labels[0];
            close(
                (net.position.x, net.position.y),
                about_centre(centre, op, (20.0, 16.0)),
                &format!("{what}: net label position"),
            );
            assert_text_followed(
                op,
                (
                    net_before.rotation,
                    net_before.justify,
                    net_before.justify_v,
                ),
                (net.rotation, net.justify, net.justify_v),
            );

            let flag = &after.labels[1];
            close(
                (flag.position.x, flag.position.y),
                about_centre(centre, op, (10.0, 16.0)),
                &format!("{what}: flag position"),
            );
            let (s, c) = flag.rotation.to_radians().sin_cos();
            close((c, s), op.vec((1.0, 0.0)), &format!("{what}: flag points"));

            let note_before = &before.text_notes[0];
            let note = &after.text_notes[0];
            close(
                (note.position.x, note.position.y),
                about_centre(centre, op, (14.0, 16.0)),
                &format!("{what}: note position"),
            );
            assert_text_followed(
                op,
                (
                    note_before.rotation,
                    note_before.justify_h,
                    note_before.justify_v,
                ),
                (note.rotation, note.justify_h, note.justify_v),
            );
            let _ = &ids;
        }
    }
}

#[test]
fn two_symbols_turn_about_their_centre() {
    let a = symbol("R1", (10.0, 10.0), false, false);
    let b = symbol("R2", (20.0, 10.0), false, true);
    let selected = vec![
        SelectedItem::new(a.uuid, SelectedKind::Symbol),
        SelectedItem::new(b.uuid, SelectedKind::Symbol),
    ];
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![a, b],
        ..Default::default()
    };
    let mut app = app_with(sheet, selected);
    let before = doc(&app);
    press(&mut app, Op::Cw);
    let after = doc(&app);
    close(
        (after.symbols[0].position.x, after.symbols[0].position.y),
        (15.0, 5.0),
        "R1 about (15,10)",
    );
    close(
        (after.symbols[1].position.x, after.symbols[1].position.y),
        (15.0, 15.0),
        "R2 about (15,10)",
    );
    for (b, a) in before.symbols.iter().zip(&after.symbols) {
        let dir = |s: &Symbol| {
            let p = pin_world(s);
            (p.0 - s.position.x, p.1 - s.position.y)
        };
        close(
            dir(a),
            Op::Cw.vec(dir(b)),
            &format!("{} pin direction", a.reference),
        );
    }
}

#[test]
fn four_rotations_are_the_identity_and_two_flips_cancel() {
    for (op, times) in [
        (Op::Ccw, 4),
        (Op::Cw, 4),
        (Op::FlipLeftRight, 2),
        (Op::FlipTopBottom, 2),
    ] {
        let (mut app, _) = mixed_app((false, true));
        let before = doc(&app);
        press(&mut app, op);
        let once = doc(&app);
        assert!(
            (once.wires[0].end.x, once.wires[0].end.y)
                != (before.wires[0].end.x, before.wires[0].end.y),
            "{op:?} must move the selection before the identity check means anything"
        );
        for _ in 1..times {
            press(&mut app, op);
        }
        let after = doc(&app);
        let what = format!("{op:?} x{times}");
        let r = (&before.symbols[0], &after.symbols[0]);
        close(
            (r.1.position.x, r.1.position.y),
            (r.0.position.x, r.0.position.y),
            &what,
        );
        assert_eq!(
            r.1.rotation.rem_euclid(360.0),
            r.0.rotation.rem_euclid(360.0),
            "{what}"
        );
        assert_eq!(
            (r.1.mirror_x, r.1.mirror_y),
            (r.0.mirror_x, r.0.mirror_y),
            "{what}"
        );
        let w = (&before.wires[0], &after.wires[0]);
        close(
            (w.1.start.x, w.1.start.y),
            (w.0.start.x, w.0.start.y),
            &what,
        );
        close((w.1.end.x, w.1.end.y), (w.0.end.x, w.0.end.y), &what);
        for (b, a) in before.labels.iter().zip(&after.labels) {
            close(
                (a.position.x, a.position.y),
                (b.position.x, b.position.y),
                &what,
            );
            assert_eq!(
                a.rotation.rem_euclid(360.0),
                b.rotation.rem_euclid(360.0),
                "{what}"
            );
            assert_eq!((a.justify, a.justify_v), (b.justify, b.justify_v), "{what}");
        }
        let n = (&before.text_notes[0], &after.text_notes[0]);
        assert_eq!(
            n.1.rotation.rem_euclid(360.0),
            n.0.rotation.rem_euclid(360.0),
            "{what}"
        );
        assert_eq!(
            (n.1.justify_h, n.1.justify_v),
            (n.0.justify_h, n.0.justify_v),
            "{what}"
        );
    }
}

#[test]
fn one_undo_and_one_redo_per_action() {
    let (mut app, _) = mixed_app((false, false));
    let original = doc(&app);
    press(&mut app, Op::Ccw);
    let turned = doc(&app);
    assert_ne!(turned.wires[0].end.x, original.wires[0].end.x);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    let undone = doc(&app);
    close(
        (undone.wires[0].end.x, undone.wires[0].end.y),
        (original.wires[0].end.x, original.wires[0].end.y),
        "one undo restores the wire",
    );
    close(
        (undone.symbols[0].position.x, undone.symbols[0].position.y),
        (
            original.symbols[0].position.x,
            original.symbols[0].position.y,
        ),
        "one undo restores the symbol",
    );
    assert_eq!(undone.labels[0].rotation, original.labels[0].rotation);
    assert_eq!(undone.labels[0].justify, original.labels[0].justify);
    assert_eq!(undone.symbols[0].rotation, original.symbols[0].rotation);

    let _ = app.update(Message::Edit(EditMsg::Redo));
    let redone = doc(&app);
    close(
        (redone.wires[0].end.x, redone.wires[0].end.y),
        (turned.wires[0].end.x, turned.wires[0].end.y),
        "one redo re-applies the whole action",
    );
    assert_eq!(redone.labels[0].rotation, turned.labels[0].rotation);
    assert_eq!(redone.symbols[0].rotation, turned.symbols[0].rotation);
}

#[test]
fn a_lone_symbol_still_turns_in_place() {
    let r1 = symbol("R1", (10.0, 10.0), false, false);
    let item = SelectedItem::new(r1.uuid, SelectedKind::Symbol);
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![r1],
        ..Default::default()
    };
    let mut app = app_with(sheet, vec![item]);
    press(&mut app, Op::Ccw);
    let s = &doc(&app).symbols[0];
    close(
        (s.position.x, s.position.y),
        (10.0, 10.0),
        "position unchanged",
    );
    close(
        pin_world(s),
        (10.0, 8.0),
        "pin turned counter-clockwise (east -> north)",
    );
}

#[test]
fn a_selection_holding_a_sheet_symbol_is_refused_whole() {
    let (sheet, ids) = mixed_sheet((false, false));
    let mut sheet = sheet;
    let child = ChildSheet {
        uuid: uuid::Uuid::new_v4(),
        name: "Child".to_string(),
        filename: "child.snxsch".to_string(),
        position: Point::new(30.0, 30.0),
        size: (10.0, 10.0),
        stroke_width: 0.12,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    };
    let child_item = SelectedItem::new(child.uuid, SelectedKind::ChildSheet);
    sheet.child_sheets.push(child);

    let mut engine = signex_engine::Engine::new(sheet.clone()).expect("engine");
    let err = engine
        .execute(signex_engine::Command::TransformSelection {
            items: vec![ids.symbol, ids.wire, child_item],
            centre: Point::new(15.0, 13.0),
            transform: signex_engine::SelectionTransform::RotateClockwise,
            keep_wires: false,
        })
        .expect_err("a sheet symbol in the selection refuses the action");
    assert!(
        err.to_string().contains("a sheet symbol"),
        "refusal names the blocker: {err}"
    );
    assert_eq!(
        engine.document().wires[0].end.x,
        20.0,
        "nothing moved half-way"
    );

    let mut app = app_with(sheet, vec![ids.symbol, ids.wire, child_item]);
    let before = doc(&app);
    press(&mut app, Op::Cw);
    let after = doc(&app);
    assert_eq!(after.wires[0].end.x, before.wires[0].end.x);
    assert_eq!(after.symbols[0].position.x, before.symbols[0].position.x);
    assert_eq!(after.symbols[0].rotation, before.symbols[0].rotation);
}

/// R1 off the grid at (10.3, 10.3) with visible reference and value fields,
/// as box-select picks them up together.
fn off_grid_symbol_with_fields() -> (SchematicSheet, uuid::Uuid) {
    use signex_types::schematic::TextProp;
    let mut r1 = symbol("R1", (10.3, 10.3), false, false);
    let field = |dy: f64| TextProp {
        position: Point::new(10.3, 10.3 + dy),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        hidden: false,
        font_family: None,
    };
    r1.ref_text = Some(field(-2.0));
    r1.val_text = Some(field(2.0));
    let uuid = r1.uuid;
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![r1],
        ..Default::default()
    };
    (sheet, uuid)
}

#[test]
fn a_box_selected_lone_symbol_turns_in_place() {
    use signex_app::schematic_runtime::hit_test::{SelectionMode, hit_test_rect_mode};
    use signex_types::schematic::Aabb;
    let (sheet, uuid) = off_grid_symbol_with_fields();
    let selected = hit_test_rect_mode(
        &sheet,
        &Default::default(),
        &Aabb::new(0.0, 0.0, 30.0, 30.0),
        SelectionMode::Touching,
    );
    assert!(
        selected.contains(&SelectedItem::new(uuid, SelectedKind::Symbol))
            && selected.contains(&SelectedItem::new(uuid, SelectedKind::SymbolRefField))
            && selected.contains(&SelectedItem::new(uuid, SelectedKind::SymbolValField)),
        "box-select picks up the symbol with its fields: {selected:?}"
    );
    let mut app = app_with(sheet, selected);
    press(&mut app, Op::Ccw);
    let s = &doc(&app).symbols[0];
    close(
        (s.position.x, s.position.y),
        (10.3, 10.3),
        "an off-grid symbol turns in place, not about a grid-snapped centre",
    );
    close(
        pin_world(s),
        (10.3, 8.3),
        "pin turned counter-clockwise (east -> north)",
    );
}

#[test]
fn fields_ride_with_their_symbol_in_a_mixed_selection() {
    let (sheet, ids) = mixed_sheet((false, false));
    let fields = [
        SelectedItem::new(ids.symbol.uuid, SelectedKind::SymbolRefField),
        SelectedItem::new(ids.symbol.uuid, SelectedKind::SymbolValField),
    ];
    let mut app = app_with(sheet, vec![ids.symbol, fields[0], fields[1], ids.wire]);
    let before = doc(&app);
    press(&mut app, Op::Ccw);
    let after = doc(&app);
    assert_ne!(
        (after.symbols[0].position.x, after.symbols[0].position.y),
        (before.symbols[0].position.x, before.symbols[0].position.y),
        "a selection holding the symbol's own fields is transformed, not refused"
    );
    close(
        pin_world(&after.symbols[0]),
        (after.wires[0].start.x, after.wires[0].start.y),
        "the wire stays on the pin",
    );
}

#[test]
fn vertical_text_reads_bottom_to_top() {
    let (sheet, ids) = mixed_sheet((false, false));
    for op in [Op::Ccw, Op::Cw] {
        let mut app = app_with(sheet.clone(), vec![ids.wire, ids.label, ids.note]);
        press(&mut app, op);
        let d = doc(&app);
        let net = d.labels.iter().find(|l| l.uuid == ids.label.uuid).unwrap();
        assert!(
            (net.rotation - 270.0).abs() < 1e-9,
            "{op:?}: a vertical net label reads bottom-to-top (270), got {}",
            net.rotation
        );
        assert!(
            (d.text_notes[0].rotation - 270.0).abs() < 1e-9,
            "{op:?}: a vertical note reads bottom-to-top (270), got {}",
            d.text_notes[0].rotation
        );
    }
}

/// A two-pin `Device:R` whose pins sit at local (±2, 0), as `PIN` assumes.
fn add_resistor_lib(sheet: &mut SchematicSheet) {
    use signex_types::schematic::{LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle};
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
    sheet.lib_symbols.insert(
        "Device:R".to_string(),
        LibSymbol {
            id: "Device:R".to_string(),
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
            pins: vec![pin("1", PIN.0), pin("2", -PIN.0)],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        },
    );
}

/// Every net as (name, sorted "REF.pin" terminals), sorted.
fn nets(sheet: &SchematicSheet) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = signex_net::build_netlist(sheet)
        .nets
        .iter()
        .map(|n| {
            let mut t: Vec<String> = n
                .terminals
                .iter()
                .map(|t| format!("{}.{}", t.reference, t.pin))
                .collect();
            t.sort();
            (n.name.clone(), t)
        })
        .filter(|(_, t)| !t.is_empty())
        .collect();
    out.sort();
    out
}

#[test]
fn a_self_contained_selection_keeps_its_netlist() {
    let r1 = symbol("R1", (10.0, 10.0), false, false);
    let r2 = symbol("R2", (30.0, 10.0), false, false);
    let (a, b) = (pin_world(&r1), (28.0, 10.0));
    let wire = Wire {
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
    };
    let net = label("N1", LabelType::Net, (20.0, 10.0));
    let selected = vec![
        SelectedItem::new(r1.uuid, SelectedKind::Symbol),
        SelectedItem::new(r2.uuid, SelectedKind::Symbol),
        SelectedItem::new(wire.uuid, SelectedKind::Wire),
        SelectedItem::new(net.uuid, SelectedKind::Label),
    ];
    let mut sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![r1, r2],
        wires: vec![wire],
        labels: vec![net],
        ..Default::default()
    };
    add_resistor_lib(&mut sheet);
    let before = nets(&sheet);
    assert!(
        before
            .iter()
            .any(|(name, t)| name == "N1" && t == &["R1.1", "R2.2"]),
        "fixture: R1.1 and R2.2 share net N1: {before:?}"
    );
    for op in OPS {
        let mut app = app_with(sheet.clone(), selected.clone());
        press(&mut app, op);
        assert_eq!(nets(&doc(&app)), before, "{op:?}: the netlist is unchanged");
    }
}

#[test]
fn two_mirrors_cancel_without_drift() {
    // Anchors at x = 10 and 11: the centre 10.5 is off the grid but on the
    // half grid, the drift case that re-snapping the centre each press had.
    let wire = Wire {
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(10.0, 10.0),
        end: Point::new(11.0, 12.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
    };
    let item = SelectedItem::new(wire.uuid, SelectedKind::Wire);
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        wires: vec![wire],
        ..Default::default()
    };
    for op in [Op::FlipLeftRight, Op::FlipTopBottom] {
        let mut app = app_with(sheet.clone(), vec![item]);
        press(&mut app, op);
        press(&mut app, op);
        let w = &doc(&app).wires[0];
        let ends = [(w.start.x, w.start.y), (w.end.x, w.end.y)];
        assert!(
            ends.contains(&(10.0, 10.0)) && ends.contains(&(11.0, 12.0)),
            "{op:?} twice is the identity: {ends:?}"
        );
    }
}

#[test]
fn a_lone_off_grid_label_turns_in_place() {
    let net = label("N1", LabelType::Net, (10.5, 10.0));
    let item = SelectedItem::new(net.uuid, SelectedKind::Label);
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        labels: vec![net],
        ..Default::default()
    };
    for op in OPS {
        let mut app = app_with(sheet.clone(), vec![item]);
        press(&mut app, op);
        let l = &doc(&app).labels[0];
        close(
            (l.position.x, l.position.y),
            (10.5, 10.0),
            &format!("{op:?}: a lone label stays where it is"),
        );
    }
}

fn wire_from(a: (f64, f64), b: (f64, f64)) -> (SchematicSheet, SelectedItem) {
    let wire = Wire {
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
    };
    let item = SelectedItem::new(wire.uuid, SelectedKind::Wire);
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        wires: vec![wire],
        ..Default::default()
    };
    (sheet, item)
}

fn ends(app: &Signex) -> [(f64, f64); 2] {
    let w = &doc(app).wires[0];
    [(w.start.x, w.start.y), (w.end.x, w.end.y)]
}

#[test]
fn four_quarter_turns_of_a_half_grid_centred_box_are_the_identity() {
    // A 1x1-step box: centre (10.5, 10.5), both coordinates half steps, so
    // the pivot is used as is and four turns come back exactly.
    let (sheet, item) = wire_from((10.0, 10.0), (11.0, 11.0));
    for op in [Op::Ccw, Op::Cw] {
        let mut app = app_with(sheet.clone(), vec![item]);
        let start = ends(&app);
        press(&mut app, op);
        assert_ne!(ends(&app), start, "{op:?}: the first turn moves the wire");
        for _ in 0..3 {
            press(&mut app, op);
        }
        let mut got = ends(&app);
        let mut want = start;
        got.sort_by(|a, b| a.partial_cmp(b).unwrap());
        want.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(got, want, "{op:?} four times is the identity");
    }
}

#[test]
fn a_mixed_parity_box_stays_on_the_grid_when_turned() {
    // A 1x2-step box: centre (10.5, 11) mixes a half and a whole step, which
    // has no drift-free pivot; every point must still land on the grid.
    let (sheet, item) = wire_from((10.0, 10.0), (11.0, 12.0));
    for op in [Op::Ccw, Op::Cw] {
        let mut app = app_with(sheet.clone(), vec![item]);
        for turn in 1..=4 {
            press(&mut app, op);
            for (x, y) in ends(&app) {
                assert!(
                    (x - x.round()).abs() < 1e-9 && (y - y.round()).abs() < 1e-9,
                    "{op:?} turn {turn}: ({x}, {y}) is off the 1 mm grid"
                );
            }
        }
    }
}
