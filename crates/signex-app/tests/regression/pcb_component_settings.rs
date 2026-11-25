//! #2365 and #2369: the board Component view's stored settings and its
//! parameter and Comment edits, driven through `Signex::update` with the
//! messages the panel sends (the 3D Opacity slider and the Lock row's
//! checkboxes of Graphite mockup row 94 send the same edits). Every edit is one board
//! undo step; Lock Primitives stops a pad edit.

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{EditMsg, FileMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::panels::pcb_primitive_properties::{PrimitiveField, PrimitiveTarget};
use signex_app::panels::{
    ComponentField, PanelMsg, PcbComponentFacts, PcbComponentMsg, PcbPrimitiveMsg, Shared,
};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_app::test_adapter::BoardEngines;
use signex_engine::pcb::component_edit::{ComponentSetting, TypedSetting};
use signex_types::format::SnxPcb;
use signex_types::pcb::{ComponentType, Footprint, Pad, PadShape, PadType, PcbBoard, Point};
use signex_types::property::PcbProperty;
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

fn footprint(uuid: Uuid, reference: &str, value: &str, x: f64) -> Footprint {
    let n = uuid.as_u128();
    Footprint {
        uuid,
        reference: reference.into(),
        value: value.into(),
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
            property("Value", value),
            property("MPN", "RC0603"),
        ],
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
            footprint(R2, "R2", "10k", 10.0),
            footprint(C7, "C7", "100n", 20.0),
        ],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        stackup: None,
        rooms: Vec::new(),
        free: Default::default(),
        options: Default::default(),
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

fn select_r2(app: &mut Signex) {
    click(app, 10.0, ClickMode::Replace);
}

fn select_both(app: &mut Signex) {
    select_r2(app);
    click(app, 20.0, ClickMode::Add);
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

/// Type `text` into `field` of the selection without pressing Enter.
fn type_in(f: &mut Fixture, field: ComponentField, text: &str) {
    let footprints = facts(&f.app).footprints;
    send(
        &mut f.app,
        panel(PcbComponentMsg::FieldTyped {
            board: f.path.clone(),
            footprints,
            field,
            text: text.into(),
        }),
    );
}

/// Type `text` into `field` of the selection, then press Enter.
fn type_and_enter(f: &mut Fixture, field: ComponentField, text: &str) {
    type_in(f, field.clone(), text);
    let footprints = facts(&f.app).footprints;
    send(
        &mut f.app,
        panel(PcbComponentMsg::FieldSubmitted {
            board: f.path.clone(),
            footprints,
            field,
        }),
    );
}

fn pick(f: &mut Fixture, setting: ComponentSetting) {
    let footprints = facts(&f.app).footprints;
    send(
        &mut f.app,
        panel(PcbComponentMsg::SettingPicked {
            board: f.path.clone(),
            footprints,
            setting,
        }),
    );
}

/// The edit `run` makes is one undo step: undo restores the board, redo
/// replays it, and a second undo has nothing of it left.
fn assert_one_step(f: &mut Fixture, run: impl FnOnce(&mut Fixture)) -> PcbBoard {
    let before = snapshot(&f.app);
    run(f);
    let after = snapshot(&f.app);
    assert_ne!(before, after, "the board did not change");
    let edited = live(&f.app);
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), before, "one undo restores the board");
    send(&mut f.app, Message::Edit(EditMsg::Redo));
    assert_eq!(snapshot(&f.app), after, "redo replays it");
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), before, "it was a single step");
    edited
}

fn noticed(since: u64, wanted: &str) -> bool {
    diagnostics::recent_entries().iter().any(|e| {
        e.id > since
            && e.message
                .replace(['\u{2068}', '\u{2069}'], "")
                .contains(wanted)
    })
}

#[test]
fn the_type_is_picked_as_one_step_and_shown() {
    let mut f = open_board();
    select_r2(&mut f.app);
    assert_eq!(
        facts(&f.app).settings.component_type,
        Shared::Same(ComponentType::Standard)
    );
    let edited = assert_one_step(&mut f, |f| {
        pick(f, ComponentSetting::Type(ComponentType::Mechanical));
        assert_eq!(
            facts(&f.app).settings.component_type,
            Shared::Same(ComponentType::Mechanical)
        );
    });
    assert_eq!(
        fp(&edited, R2).component.component_type,
        ComponentType::Mechanical
    );
    assert!(fp(&edited, C7).component.is_default(), "only R2");
}

/// A checkbox setting and how to read it back off a footprint.
type CheckboxCase = (fn(bool) -> ComponentSetting, fn(&Footprint) -> bool);

#[test]
fn every_checkbox_setting_is_one_step() {
    let cases: [CheckboxCase; 4] = [
        (ComponentSetting::LockPrimitives, |fp| {
            fp.component.lock_primitives
        }),
        (ComponentSetting::LockStrings, |fp| {
            fp.component.lock_strings
        }),
        (ComponentSetting::PinSwapping, |fp| {
            fp.component.pin_swapping
        }),
        (ComponentSetting::PartSwapping, |fp| {
            fp.component.part_swapping
        }),
    ];
    for (make, read) in cases {
        let mut f = open_board();
        select_r2(&mut f.app);
        let edited = assert_one_step(&mut f, |f| pick(f, make(true)));
        assert!(read(&fp(&edited, R2)), "{:?}", make(true));
    }
}

#[test]
fn every_typed_setting_is_one_step() {
    let mut f = open_board();
    select_both(&mut f.app);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, ComponentField::Setting(TypedSetting::Height), "2.5mm");
    });
    assert_eq!(fp(&edited, R2).component.height_mm, 2.5);
    assert_eq!(fp(&edited, C7).component.height_mm, 2.5);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, ComponentField::Setting(TypedSetting::BodyOpacity), "40%");
    });
    assert_eq!(fp(&edited, R2).component.body_opacity, 0.4);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, ComponentField::Setting(TypedSetting::ChannelOffset), "3");
    });
    assert_eq!(fp(&edited, C7).component.channel_offset, 3);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(
            f,
            ComponentField::Setting(TypedSetting::Description),
            "Thick film",
        );
    });
    assert_eq!(fp(&edited, R2).component.description, "Thick film");
}

/// The 3D Opacity slider (Graphite mockup row 94): each drag step is the field's draft
/// in two decimals and changes nothing; letting go applies the last one as
/// one step, the edit the typed field made.
#[test]
fn a_dragged_opacity_applies_once_on_release() {
    let mut f = open_board();
    select_both(&mut f.app);
    let field = ComponentField::Setting(TypedSetting::BodyOpacity);
    let edited = assert_one_step(&mut f, |f| {
        let before = snapshot(&f.app);
        for step in ["0.90", "0.62", "0.25"] {
            type_in(f, field.clone(), step);
        }
        assert_eq!(snapshot(&f.app), before, "a drag edits nothing yet");
        let footprints = facts(&f.app).footprints;
        send(
            &mut f.app,
            panel(PcbComponentMsg::FieldSubmitted {
                board: f.path.clone(),
                footprints,
                field: field.clone(),
            }),
        );
    });
    assert_eq!(fp(&edited, R2).component.body_opacity, 0.25);
    assert_eq!(fp(&edited, C7).component.body_opacity, 0.25);
}

#[test]
fn an_opacity_past_one_is_refused_and_says_why() {
    let mut f = open_board();
    select_r2(&mut f.app);
    let before = snapshot(&f.app);
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    type_and_enter(
        &mut f,
        ComponentField::Setting(TypedSetting::BodyOpacity),
        "1.5",
    );
    assert_eq!(snapshot(&f.app), before);
    assert!(noticed(since, "out of range"), "the refusal says why");
}

#[test]
fn locked_primitives_stop_a_pad_edit() {
    let mut f = open_board();
    select_r2(&mut f.app);
    pick(&mut f, ComponentSetting::LockPrimitives(true));
    assert!(fp(&live(&f.app), R2).component.lock_primitives);
    let before = snapshot(&f.app);
    // Select R2's left pad on its own, as the pad panel edits it.
    click(&mut f.app, 8.0, ClickMode::Replace);
    let pad = ItemRef::Pad {
        footprint: R2,
        pad: Uuid::from_u128(0x11),
    };
    let selected: Vec<ItemRef> = f
        .app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(selected, [pad]);
    let target = PrimitiveTarget {
        board: f.path.clone(),
        items: vec![pad],
    };
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    for message in [
        PcbPrimitiveMsg::FieldTyped {
            target: target.clone(),
            field: PrimitiveField::SizeX,
            text: "3".into(),
        },
        PcbPrimitiveMsg::FieldSubmitted {
            target: target.clone(),
            field: PrimitiveField::SizeX,
        },
    ] {
        send(
            &mut f.app,
            Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(message))),
        );
    }
    assert_eq!(snapshot(&f.app), before, "the pad keeps its size");
    assert!(noticed(since, "locked"), "the refusal says why");
}

#[test]
fn adding_then_removing_a_parameter_is_one_step_each() {
    let mut f = open_board();
    select_both(&mut f.app);
    f.app.document_state.panel_ctx.properties_tab = 1;
    let edited = assert_one_step(&mut f, |f| {
        type_in(f, ComponentField::NewParameterName, "Tolerance");
        type_in(f, ComponentField::NewParameterValue, "1%");
        let footprints = facts(&f.app).footprints;
        send(
            &mut f.app,
            panel(PcbComponentMsg::ParameterAdded {
                board: f.path.clone(),
                footprints,
            }),
        );
        assert!(
            facts(&f.app)
                .draft(&ComponentField::NewParameterName)
                .is_none(),
            "the typed name goes once added"
        );
    });
    for id in [R2, C7] {
        let added = fp(&edited, id).properties.last().cloned().unwrap();
        assert_eq!(
            (added.key.as_str(), added.value.as_str()),
            ("Tolerance", "1%")
        );
    }

    let edited = assert_one_step(&mut f, |f| {
        let footprints = facts(&f.app).footprints;
        send(
            &mut f.app,
            panel(PcbComponentMsg::ParameterRemoved {
                board: f.path.clone(),
                footprints,
                key: "MPN".into(),
            }),
        );
        assert!(facts(&f.app).parameter("MPN").is_none());
    });
    assert!(fp(&edited, R2).properties.iter().all(|p| p.key != "MPN"));
    assert!(fp(&edited, C7).properties.iter().all(|p| p.key != "MPN"));
}

#[test]
fn a_parameter_already_there_is_not_added_twice_and_the_name_stays() {
    let mut f = open_board();
    select_r2(&mut f.app);
    let before = snapshot(&f.app);
    type_in(&mut f, ComponentField::NewParameterName, "MPN");
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    send(
        &mut f.app,
        panel(PcbComponentMsg::ParameterAdded {
            board: f.path.clone(),
            footprints: vec![R2],
        }),
    );
    assert_eq!(snapshot(&f.app), before);
    assert!(noticed(since, "already has a parameter MPN"));
    assert_eq!(
        facts(&f.app)
            .draft(&ComponentField::NewParameterName)
            .map(String::as_str),
        Some("MPN"),
        "the name can be corrected"
    );
}

#[test]
fn one_comment_sets_every_selected_component_in_one_step() {
    let mut f = open_board();
    select_both(&mut f.app);
    assert_eq!(facts(&f.app).comment, Shared::Mixed);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, ComponentField::Comment, "22k");
        assert_eq!(facts(&f.app).comment, Shared::Same("22k".to_string()));
    });
    for id in [R2, C7] {
        let fp = fp(&edited, id);
        assert_eq!(fp.value, "22k");
        let shown = fp.properties.iter().find(|p| p.key == "Value").unwrap();
        assert_eq!(shown.value, "22k", "the Comment text follows");
    }
    assert_eq!(fp(&edited, R2).reference, "R2");
}
