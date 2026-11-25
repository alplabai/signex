//! #2907 — nets cross probe both ways. A wire, label or sheet entry selected
//! on the sheet selects its net's pads, tracks, vias and copper zone on the
//! board; a pad, track, via or copper zone selected on the board selects the
//! net's wires, labels and sheet entries on the sheet. Each direction is
//! driven with the board, then the sheet, in an undocked window.
//!
//! A sheet selection is written straight into the canvas, as the net
//! navigation tests do; the cross select pass after the next update reads it
//! as the user's change. A board selection is a canvas click.

use std::collections::{BTreeSet, HashSet};

use pcb_geom::{ClickMode, ItemRef, PickKind};
use signex_app::app::{Message, PcbMsg};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_types::pcb::{
    Footprint, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point, SOLID_FILL, Segment,
    SymbolLink, Via, ViaType, Zone,
};
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, InstancePath, Label, LabelType, SchematicSheet, SelectedItem,
    SelectedKind, SheetPin, Symbol, VAlign, Wire,
};
use uuid::Uuid;

use super::cross_probe::{
    Fixture, fixture_with, main_board_selection, send, show_board, show_sheet, undock,
    window_board_selection,
};

const R1: Uuid = Uuid::from_u128(0x2907_0001);
const WIRE_A: Uuid = Uuid::from_u128(0x2907_0002);
const WIRE_B: Uuid = Uuid::from_u128(0x2907_0003);
const LABEL: Uuid = Uuid::from_u128(0x2907_0004);
const CHILD: Uuid = Uuid::from_u128(0x2907_0005);
const SHEET_PIN: Uuid = Uuid::from_u128(0x2907_0006);
const U_R1: Uuid = Uuid::from_u128(0x2907_0010);
const PAD: Uuid = Uuid::from_u128(0x2907_0011);
const TRACK: Uuid = Uuid::from_u128(0x2907_0012);
const OTHER_TRACK: Uuid = Uuid::from_u128(0x2907_0013);
const VIA: Uuid = Uuid::from_u128(0x2907_0014);
const ZONE: Uuid = Uuid::from_u128(0x2907_0015);

/// The net every test probes, and a second net nothing selects.
const SIG: u32 = 1;
const OTHER: u32 = 2;

fn sch(x: f64, y: f64) -> signex_types::schematic::Point {
    signex_types::schematic::Point::new(x, y)
}

fn wire(uuid: Uuid, from: (f64, f64), to: (f64, f64)) -> Wire {
    Wire {
        uuid,
        start: sch(from.0, from.1),
        end: sch(to.0, to.1),
        ..Default::default()
    }
}

/// R1, and the net `SIG`: a wire from (50, 50) to (60, 50) named by a global
/// label at (50, 50), and a wire on to (70, 50). With `sheet_pin`, the second
/// wire ends on the entry `SIG` of a child sheet symbol for `child.snxsch`.
fn netted_sheet(sheet_pin: bool) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::from_u128(0x2907_0000),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: R1,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            position: sch(20.0, 20.0),
            ..Default::default()
        }],
        wires: vec![
            wire(WIRE_A, (50.0, 50.0), (60.0, 50.0)),
            wire(WIRE_B, (60.0, 50.0), (70.0, 50.0)),
        ],
        labels: vec![Label {
            uuid: LABEL,
            justify: HAlign::default(),
            justify_v: VAlign::default(),
            ..Label::new("SIG", sch(50.0, 50.0), LabelType::Global)
        }],
        ..Default::default()
    };
    if sheet_pin {
        sheet.child_sheets.push(ChildSheet {
            uuid: CHILD,
            name: "child".to_string(),
            filename: "child.snxsch".to_string(),
            position: sch(70.0, 45.0),
            size: (10.0, 10.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![SheetPin {
                uuid: SHEET_PIN,
                name: "SIG".to_string(),
                direction: "input".to_string(),
                position: sch(70.0, 50.0),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            }],
            instances: Vec::new(),
        });
    }
    sheet
}

/// The child sheet behind the entry: a hierarchical label `SIG`.
fn child_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::from_u128(0x2907_0100),
        paper_size: "A4".to_string(),
        labels: vec![Label {
            uuid: Uuid::from_u128(0x2907_0101),
            justify: HAlign::default(),
            justify_v: VAlign::default(),
            ..Label::new("SIG", sch(10.0, 10.0), LabelType::Hierarchical)
        }],
        ..Default::default()
    }
}

fn segment(uuid: Uuid, y: f64, net: u32) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid,
        start: Point::new(20.0, y),
        end: Point::new(30.0, y),
        width: 0.2,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// A solid copper square from (50, 20) to (60, 30) on `SIG`.
fn netted_zone() -> Zone {
    Zone {
        uuid: ZONE,
        net: SIG,
        net_name: "SIG".into(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(50.0, 20.0),
            Point::new(60.0, 20.0),
            Point::new(60.0, 30.0),
            Point::new(50.0, 30.0),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: SOLID_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled: false,
        unknown: Default::default(),
    }
}

/// R1's footprint at (10, 10), its pad on `SIG`; on `SIG` too a track from
/// (20, 10) to (30, 10), a via at (40, 10) and the zone; a track from
/// (20, 30) to (30, 30) on `OTHER`.
fn netted_board() -> PcbBoard {
    let pad = Pad {
        uuid: PAD,
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: SIG,
            name: "SIG".into(),
        }),
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let footprint = Footprint {
        uuid: U_R1,
        reference: "R1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(10.0, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: Some(SymbolLink::new(InstancePath::root(), [R1])),
        library_ref: None,
        component: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let net = |number: u32, name: &str| NetDef {
        number,
        name: name.into(),
    };
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
        nets: vec![net(0, ""), net(SIG, "SIG"), net(OTHER, "OTHER")],
        footprints: vec![footprint],
        segments: vec![segment(TRACK, 10.0, SIG), segment(OTHER_TRACK, 30.0, OTHER)],
        vias: vec![Via {
            attributes: Default::default(),
            uuid: VIA,
            position: Point::new(40.0, 10.0),
            diameter: 0.6,
            drill: 0.3,
            layers: vec!["TopCopper".into(), "BottomCopper".into()],
            net: SIG,
            locked: false,
            layer_sizes: None,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
        }],
        zones: vec![netted_zone()],
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

/// The netted sheet and board open, the sheet in front, the project net
/// index built (cross select reads it, never builds it).
fn netted(sheet_pin: bool) -> Fixture {
    let extra = if sheet_pin {
        vec![("child.snxsch", child_sheet())]
    } else {
        Vec::new()
    };
    let mut f = fixture_with(netted_sheet(sheet_pin), netted_board(), &extra, true);
    f.app.settle_net_index_blocking();
    f
}

/// What `SIG` is on the board.
fn sig_on_the_board() -> BTreeSet<ItemRef> {
    let kind = PickKind::of_zone(&netted_zone()).expect("a pickable copper zone");
    BTreeSet::from([
        ItemRef::Pad {
            footprint: U_R1,
            pad: PAD,
        },
        ItemRef::Track(TRACK),
        ItemRef::Via(VIA),
        ItemRef::Zone { zone: ZONE, kind },
    ])
}

/// What `SIG` is on the sheet without the child sheet symbol.
fn sig_on_the_sheet() -> HashSet<SelectedItem> {
    HashSet::from([
        SelectedItem::new(WIRE_A, SelectedKind::Wire),
        SelectedItem::new(WIRE_B, SelectedKind::Wire),
        SelectedItem::new(LABEL, SelectedKind::Label),
    ])
}

/// The sheet items that stand for `SIG`, each with the fixture it needs.
fn sheet_members() -> [(SelectedItem, bool); 3] {
    [
        (SelectedItem::new(WIRE_A, SelectedKind::Wire), false),
        (SelectedItem::new(LABEL, SelectedKind::Label), false),
        (SelectedItem::new(SHEET_PIN, SelectedKind::SheetPin), true),
    ]
}

/// The board items that stand for `SIG`, and the point a click picks each.
fn board_members() -> [(ItemRef, Point); 4] {
    let kind = PickKind::of_zone(&netted_zone()).expect("a pickable copper zone");
    [
        (
            ItemRef::Pad {
                footprint: U_R1,
                pad: PAD,
            },
            Point::new(10.0, 10.0),
        ),
        (ItemRef::Track(TRACK), Point::new(25.0, 10.0)),
        (ItemRef::Via(VIA), Point::new(40.0, 10.0)),
        (ItemRef::Zone { zone: ZONE, kind }, Point::new(55.0, 25.0)),
    ]
}

fn click_at(at: Point) -> PcbCanvasMsg {
    PcbCanvasMsg::Clicked {
        at,
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }
}

/// A sheet selection as a set: the order members come in is not the point.
fn sheet_set(items: &[SelectedItem]) -> HashSet<SelectedItem> {
    items.iter().copied().collect()
}

#[test]
fn a_net_selected_on_the_sheet_is_selected_on_an_undocked_board() {
    for (item, sheet_pin) in sheet_members() {
        let mut f = netted(sheet_pin);
        let board = f.board.clone();
        let window = undock(&mut f, &board);
        show_sheet(&mut f);
        f.app.interaction_state.canvas.selected = vec![item];
        send(&mut f.app, Message::noop());
        let selected: BTreeSet<ItemRef> = window_board_selection(&f, window).into_iter().collect();
        assert_eq!(selected, sig_on_the_board(), "from {item:?}");
    }
}

#[test]
fn a_net_selected_on_an_undocked_sheet_is_selected_on_the_board() {
    for (item, sheet_pin) in sheet_members() {
        let mut f = netted(sheet_pin);
        let sheet = f.sheet.clone();
        let window = undock(&mut f, &sheet);
        show_board(&mut f);
        f.app
            .interaction_state
            .canvases
            .get_mut(&window)
            .expect("the window's sheet canvas")
            .selected = vec![item];
        send(&mut f.app, Message::noop());
        let selected: BTreeSet<ItemRef> = main_board_selection(&f).into_iter().collect();
        assert_eq!(selected, sig_on_the_board(), "from {item:?}");
    }
}

#[test]
fn a_net_selected_on_the_board_is_selected_on_an_undocked_sheet() {
    for (item, at) in board_members() {
        let mut f = netted(false);
        let sheet = f.sheet.clone();
        let window = undock(&mut f, &sheet);
        show_board(&mut f);
        send(&mut f.app, Message::PcbCanvas(click_at(at)));
        assert_eq!(main_board_selection(&f), vec![item], "the click picks it");
        let selected = sheet_set(&f.app.interaction_state.canvases[&window].selected);
        assert_eq!(selected, sig_on_the_sheet(), "from {item:?}");
    }
}

#[test]
fn a_net_selected_on_an_undocked_board_is_selected_on_the_sheet() {
    for (item, at) in board_members() {
        let mut f = netted(false);
        let board = f.board.clone();
        let window = undock(&mut f, &board);
        show_sheet(&mut f);
        let msg = click_at(at);
        send(
            &mut f.app,
            Message::Pcb(PcbMsg::CanvasInWindow { window, msg }),
        );
        assert_eq!(
            window_board_selection(&f, window),
            vec![item],
            "the click picks it"
        );
        let selected = sheet_set(&f.app.interaction_state.canvas.selected);
        assert_eq!(selected, sig_on_the_sheet(), "from {item:?}");
    }
}

/// The child sheet symbol's entry is one of the net's members on the sheet,
/// so a board pick of the net selects it with the wires and the label.
#[test]
fn a_net_selected_on_the_board_selects_the_sheet_entry_on_it() {
    let mut f = netted(true);
    let sheet = f.sheet.clone();
    let window = undock(&mut f, &sheet);
    show_board(&mut f);
    send(
        &mut f.app,
        Message::PcbCanvas(click_at(Point::new(25.0, 10.0))),
    );
    let mut expected = sig_on_the_sheet();
    expected.insert(SelectedItem::new(SHEET_PIN, SelectedKind::SheetPin));
    let selected = sheet_set(&f.app.interaction_state.canvases[&window].selected);
    assert_eq!(selected, expected);
}

/// A net pick never drags in a part: R1 stays unselected on the sheet even
/// though its footprint owns the picked pad.
#[test]
fn a_pad_on_a_net_stands_for_the_net_not_its_part() {
    let mut f = netted(false);
    let sheet = f.sheet.clone();
    let window = undock(&mut f, &sheet);
    show_board(&mut f);
    send(
        &mut f.app,
        Message::PcbCanvas(click_at(Point::new(10.0, 10.0))),
    );
    let selected = &f.app.interaction_state.canvases[&window].selected;
    assert!(
        !selected.contains(&SelectedItem::new(R1, SelectedKind::Symbol)),
        "got {selected:?}"
    );
}
