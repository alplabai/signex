//! #2418 — the Location rows as drawn: each placed kind shows editable X / Y
//! in the panel's unit with Y from the page origin, a sheet symbol its Size,
//! and a locked object read-only values under the lock's reason.

use signex_types::coord::Unit;
use signex_types::schematic::{
    ChildSheet, FillType, Junction, Label, LabelType, Point, SchematicSheet, SelectedItem,
    SelectedKind, Symbol,
};
use uuid::Uuid;

use crate::app::{Signex, TabInfo, TabKind};

/// `sheet` open and active in a schematic tab at `path`.
fn add_sheet_with(app: &mut Signex, path: &std::path::Path, sheet: SchematicSheet) {
    app.document_state.tabs.push(TabInfo {
        title: path.display().to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.apply_loaded_schematic(Some(sheet), false, false, false);
}

const PART: u128 = 1;
const LABEL: u128 = 2;
const JUNCTION: u128 = 3;
const SHEET: u128 = 4;
const WIRE: u128 = 5;

/// An A4 sheet (Lower Left origin, 210 mm tall); every object at y = 10, so
/// each shows Y 200.
fn sheet(part_locked: bool) -> SchematicSheet {
    let mut label = Label::new("NET", Point::new(30.0, 10.0), LabelType::Net);
    label.uuid = Uuid::from_u128(LABEL);
    SchematicSheet {
        paper_size: "A4".into(),
        symbols: vec![Symbol {
            uuid: Uuid::from_u128(PART),
            lib_id: "Device:R".into(),
            reference: "R1".into(),
            position: Point::new(10.0, 10.0),
            locked: part_locked,
            ..Default::default()
        }],
        labels: vec![label],
        wires: vec![signex_types::schematic::Wire {
            uuid: Uuid::from_u128(WIRE),
            start: Point::new(80.0, 10.0),
            end: Point::new(100.0, 10.0),
            stroke_width: 0.0,
            color: None,
            locked: false,
            unknown_columns: Default::default(),
        }],
        junctions: vec![Junction {
            uuid: Uuid::from_u128(JUNCTION),
            position: Point::new(40.0, 10.0),
            diameter: 0.0,
            minted: false,
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }],
        child_sheets: vec![ChildSheet {
            uuid: Uuid::from_u128(SHEET),
            name: "Power".into(),
            filename: "power.snxsch".into(),
            position: Point::new(60.0, 10.0),
            size: (25.4, 12.7),
            stroke_width: 0.15,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        }],
        ..Default::default()
    }
}

/// The texts the Properties view of `item` shows, bidi isolation marks
/// dropped, with the panel in `unit`.
fn shown(app: &mut Signex, item: SelectedItem, unit: Unit) -> Vec<String> {
    app.interaction_state.active_canvas_mut().selected = vec![item];
    app.update_selection_info();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.unit = unit;
    ctx.paper_size = "A4".into();
    let view = super::view_selected_element_properties(
        &ctx,
        iced::Color::WHITE,
        iced::Color::WHITE,
        iced::Color::BLACK,
    );
    crate::layout_probe::lay_out_texts(view, iced::Size::new(400.0, 4000.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn app(part_locked: bool) -> (tempfile::TempDir, Signex) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _task) = Signex::new();
    add_sheet_with(
        &mut app,
        &dir.path().join("main.snxsch"),
        sheet(part_locked),
    );
    (dir, app)
}

/// A part, a label, a junction and a sheet symbol each show one Position
/// row: an X and a Y field with the panel's unit inside (#2523). The values
/// sit in edit boxes, so they are not plain text in the drawn tree.
#[test]
fn every_placed_kind_shows_an_editable_position_pair_in_the_panels_unit() {
    let (_dir, mut app) = app(false);
    for (n, kind) in [
        (PART, SelectedKind::Symbol),
        (LABEL, SelectedKind::Label),
        (JUNCTION, SelectedKind::Junction),
        (SHEET, SelectedKind::ChildSheet),
    ] {
        let texts = shown(
            &mut app,
            SelectedItem::new(Uuid::from_u128(n), kind),
            Unit::Mm,
        );
        for label in ["Position", "X", "Y", "mm"] {
            assert!(
                texts.iter().any(|t| t == label),
                "{kind:?} lacks {label}: {texts:?}"
            );
        }
        assert!(
            !texts.iter().any(|t| t == "200.000"),
            "{kind:?}: Y is an edit box, not a read-only value"
        );
    }
    let texts = shown(
        &mut app,
        SelectedItem::new(Uuid::from_u128(SHEET), SelectedKind::ChildSheet),
        Unit::Mil,
    );
    for label in ["Position", "Size", "W", "H", "mil"] {
        assert!(texts.iter().any(|t| t == label), "lacks {label}: {texts:?}");
    }
    let rotation = shown(
        &mut app,
        SelectedItem::new(Uuid::from_u128(PART), SelectedKind::Symbol),
        Unit::Mm,
    );
    // The 4-step Rotation control (#2523), the stored 0 as its first step.
    for label in ["Rotation", "0°", "90°", "180°", "270°"] {
        assert!(rotation.iter().any(|t| t == label), "{label}: {rotation:?}");
    }
}

/// A locked part shows the lock's reason, and its position and rotation as
/// read-only values (Y counted from the Lower Left origin: 210 - 10).
#[test]
fn a_locked_part_shows_the_reason_and_read_only_values() {
    let (_dir, mut app) = app(true);
    let texts = shown(
        &mut app,
        SelectedItem::new(Uuid::from_u128(PART), SelectedKind::Symbol),
        Unit::Mm,
    );
    assert!(
        texts.iter().any(|t| t.starts_with("Locked: R1 is locked")),
        "{texts:?}"
    );
    for value in ["10.000", "200.000", "0°"] {
        assert!(texts.iter().any(|t| t == value), "lacks {value}: {texts:?}");
    }
    assert!(
        !texts.iter().any(|t| t == "90°"),
        "a locked rotation is one read-only value, not the control: {texts:?}"
    );
}

/// #2059: each read-only position value follows its own axis tag, X then Y,
/// and its unit follows it.
#[test]
fn each_position_axis_heads_its_own_value() {
    let (_dir, mut app) = app(true);
    let texts = shown(
        &mut app,
        SelectedItem::new(Uuid::from_u128(PART), SelectedKind::Symbol),
        Unit::Mm,
    );
    let after = |label: &str, n: usize| {
        let at = texts.iter().position(|t| t == label);
        at.and_then(|i| texts.get(i + n)).map(String::as_str)
    };
    assert_eq!(after("Position", 1), Some("X"), "{texts:?}");
    assert_eq!(after("X", 1), Some("10.000"), "{texts:?}");
    assert_eq!(after("X", 2), Some("mm"), "{texts:?}");
    assert_eq!(after("Y", 1), Some("200.000"), "{texts:?}");
}

/// A wire's position is its start: the pair reads Start (#43).
#[test]
fn a_wire_position_reads_start() {
    let (_dir, mut app) = app(false);
    let texts = shown(
        &mut app,
        SelectedItem::new(Uuid::from_u128(WIRE), SelectedKind::Wire),
        Unit::Mm,
    );
    assert!(texts.iter().any(|t| t == "Start"), "{texts:?}");
    assert!(!texts.iter().any(|t| t == "Position"), "{texts:?}");
}

// ── #43 / #45 / #47 (g1): wire End, power-port State, note Text box ──

const WIRE: u128 = 11;
const PORT: u128 = 12;
const NOTE: u128 = 13;

/// A sheet with a 10 mm wire from (10, 10) to (20, 10), a +3V3 power port and
/// a text note.
fn g1_sheet() -> SchematicSheet {
    use signex_types::schematic::{HAlign, TextNote, VAlign, Wire};
    SchematicSheet {
        paper_size: "A4".into(),
        wires: vec![Wire {
            uuid: Uuid::from_u128(WIRE),
            start: Point::new(10.0, 10.0),
            end: Point::new(20.0, 10.0),
            stroke_width: 0.0,
            color: None,
            locked: false,
            unknown_columns: Default::default(),
        }],
        symbols: vec![Symbol {
            uuid: Uuid::from_u128(PORT),
            lib_id: "power:+3V3".into(),
            value: "+3V3".into(),
            is_power: true,
            position: Point::new(60.0, 60.0),
            ..Default::default()
        }],
        text_notes: vec![TextNote {
            uuid: Uuid::from_u128(NOTE),
            text: "note".into(),
            position: Point::new(90.0, 40.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked: false,
        }],
        ..Default::default()
    }
}

fn g1_app() -> (tempfile::TempDir, Signex) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _task) = Signex::new();
    add_sheet_with(&mut app, &dir.path().join("g1.snxsch"), g1_sheet());
    (dir, app)
}

fn g1_doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

fn g1_select(app: &mut Signex, item: SelectedItem) {
    app.interaction_state.active_canvas_mut().selected = vec![item];
    app.update_selection_info();
}

fn g1_panel(app: &mut Signex, msg: crate::panels::PanelMsg) {
    let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
        msg,
    )));
}

fn g1_undo(app: &mut Signex) {
    let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));
}

/// #43: End X is an edit box; Enter moves the wire's end, keeps its start,
/// is one undo step, and the read-only Length follows.
#[test]
fn a_wires_end_is_editable_and_its_length_follows() {
    use crate::panels::{PanelMsg, PlacedEditMsg, PlacedField};
    let (_dir, mut app) = g1_app();
    let wire = SelectedItem::new(Uuid::from_u128(WIRE), SelectedKind::Wire);
    let before = shown(&mut app, wire, Unit::Mm);
    for label in ["Start X (mm)", "End X (mm)", "End Y (mm)", "Length"] {
        assert!(
            before.iter().any(|t| t == label),
            "lacks {label}: {before:?}"
        );
    }
    assert!(before.iter().any(|t| t == "10.000 mm"), "{before:?}");
    g1_panel(
        &mut app,
        PanelMsg::PlacedEdit(PlacedEditMsg::Typed {
            item: wire,
            field: PlacedField::EndX,
            text: "30".into(),
        }),
    );
    g1_panel(
        &mut app,
        PanelMsg::PlacedEdit(PlacedEditMsg::Submitted {
            item: wire,
            field: PlacedField::EndX,
        }),
    );
    let moved = &g1_doc(&app).wires[0];
    assert_eq!(moved.start, Point::new(10.0, 10.0), "the start stays");
    assert_eq!(moved.end, Point::new(30.0, 10.0));
    let after = shown(&mut app, wire, Unit::Mm);
    assert!(after.iter().any(|t| t == "20.000 mm"), "{after:?}");
    g1_undo(&mut app);
    assert_eq!(
        g1_doc(&app).wires[0].end,
        Point::new(20.0, 10.0),
        "one undo step"
    );
}

/// #45: a power port ends in State ▸ Locked; its checkbox locks the port.
#[test]
fn a_power_ports_locked_row_locks_it() {
    use crate::panels::PanelMsg;
    let (_dir, mut app) = g1_app();
    let port = SelectedItem::new(Uuid::from_u128(PORT), SelectedKind::Symbol);
    g1_select(&mut app, port);
    let ctx = app.document_state.panel_ctx.clone();
    let view = super::view_selected_element_properties(
        &ctx,
        iced::Color::WHITE,
        iced::Color::WHITE,
        iced::Color::BLACK,
    );
    let window = iced::Size::new(400.0, 4000.0);
    let laid = crate::layout_probe::lay_out_texts(view, window);
    let locked = laid
        .iter()
        .find(|t| t.text == "Locked")
        .expect("Locked row");
    let checkbox = iced::Point::new(
        locked.bounds.x + crate::panels::PROPERTY_LABEL_WIDTH + 16.0,
        locked.bounds.center_y(),
    );
    let view = super::view_selected_element_properties(
        &ctx,
        iced::Color::WHITE,
        iced::Color::WHITE,
        iced::Color::BLACK,
    );
    let sent = crate::layout_probe::messages_on_click_at(view, window, None, checkbox);
    let toggle = sent
        .into_iter()
        .find(|m| matches!(m, PanelMsg::ToggleSymbolLocked(id) if *id == port.uuid))
        .expect("the Locked checkbox toggles the port");
    g1_panel(&mut app, toggle);
    assert!(g1_doc(&app).symbols[0].locked, "the port is locked");
}

/// #47: typing into the note's Text box keeps a draft (the sheet unchanged);
/// Ctrl+Enter applies the whole multi-line text as one undo step.
#[test]
fn a_notes_multi_line_edit_commits_once() {
    use crate::panels::{PanelMsg, PlacedEditMsg};
    use iced::widget::text_editor::{Action, Edit, Motion};
    let (_dir, mut app) = g1_app();
    let note = SelectedItem::new(Uuid::from_u128(NOTE), SelectedKind::TextNote);
    g1_select(&mut app, note);
    let act = |app: &mut Signex, action: Action| {
        g1_panel(
            app,
            PanelMsg::PlacedEdit(PlacedEditMsg::NoteActed { item: note, action }),
        );
    };
    act(&mut app, Action::Move(Motion::DocumentEnd));
    act(&mut app, Action::Edit(Edit::Enter));
    for c in "two".chars() {
        act(&mut app, Action::Edit(Edit::Insert(c)));
    }
    assert_eq!(
        g1_doc(&app).text_notes[0].text,
        "note",
        "typing keeps a draft"
    );
    g1_panel(
        &mut app,
        PanelMsg::PlacedEdit(PlacedEditMsg::NoteSubmitted { item: note }),
    );
    assert_eq!(g1_doc(&app).text_notes[0].text, "note\ntwo");
    g1_undo(&mut app);
    assert_eq!(g1_doc(&app).text_notes[0].text, "note", "one undo step");
}

/// #47: text typed into a note and left by selecting another object is
/// applied, not dropped.
#[test]
fn leaving_a_typed_note_applies_its_text() {
    use crate::panels::{PanelMsg, PlacedEditMsg};
    use iced::widget::text_editor::{Action, Edit, Motion};
    let (_dir, mut app) = g1_app();
    let note = SelectedItem::new(Uuid::from_u128(NOTE), SelectedKind::TextNote);
    g1_select(&mut app, note);
    for action in [
        Action::Move(Motion::DocumentEnd),
        Action::Edit(Edit::Insert('!')),
    ] {
        g1_panel(
            &mut app,
            PanelMsg::PlacedEdit(PlacedEditMsg::NoteActed { item: note, action }),
        );
    }
    let wire = SelectedItem::new(Uuid::from_u128(WIRE), SelectedKind::Wire);
    g1_select(&mut app, wire);
    assert_eq!(g1_doc(&app).text_notes[0].text, "note!");
}
