//! A click on a pin of a placed part selects the part and brings that pin
//! forward in Properties (#1963); a click on the body keeps the part view.
//! Every test goes through `Signex::update` with the canvas click event, as
//! the pointer does.

use signex_types::schematic::{
    FillType, Graphic, LibGraphic, LibPin, Point, SchematicSheet, SelectedKind,
};

use super::fixture::*;
use crate::app::Message;
use crate::canvas::CanvasEvent;

/// `R1` at (50, 50): a body from x 47.5 to 52.5, pin 1 from the tip at x 45,
/// pin 2 from the tip at x 55, nothing wired.
fn part_sheet(hide_pin_2: bool) -> (SchematicSheet, uuid::Uuid) {
    part_sheet_wired(hide_pin_2, false)
}

/// [`part_sheet`], with pin 1 wired to a global label `SIG` when `wired`.
fn part_sheet_wired(hide_pin_2: bool, wired: bool) -> (SchematicSheet, uuid::Uuid) {
    let mut sheet = sheet_with_net("R1", "SIG", &[]);
    if wired {
        sheet.wires[0].start = Point::new(45.0, 50.0);
        sheet.wires[0].end = Point::new(40.0, 50.0);
        sheet.labels[0].position = Point::new(40.0, 50.0);
    } else {
        sheet.wires.clear();
        sheet.labels.clear();
    }
    let lib = sheet.lib_symbols.get_mut("Device:R").expect("lib symbol");
    lib.graphics = vec![LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-2.5, -1.0),
            end: Point::new(2.5, 1.0),
            width: 0.254,
            fill: FillType::None,
        },
    }];
    let template = lib.pins[0].clone();
    let pin = |number: &str, x: f64, rotation: f64, visible: bool| {
        let mut lp: LibPin = template.clone();
        lp.pin.number = number.to_string();
        lp.pin.name = format!("P{number}");
        lp.pin.position = Point::new(x, 0.0);
        lp.pin.rotation = rotation;
        lp.pin.length = 2.5;
        lp.pin.visible = visible;
        lp
    };
    lib.pins = vec![pin("1", -5.0, 0.0, true), pin("2", 5.0, 180.0, !hide_pin_2)];
    sheet.symbols[0].position = Point::new(50.0, 50.0);
    let id = sheet.symbols[0].uuid;
    (sheet, id)
}

fn project_with_part(hide_pin_2: bool) -> (Project, uuid::Uuid) {
    let (sheet, id) = part_sheet(hide_pin_2);
    (single_sheet_project(sheet), id)
}

fn click(p: &mut Project, world_x: f64, world_y: f64) {
    let task = p.app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x,
        world_y,
    }));
    drop(task);
}

fn info(p: &Project, key: &str) -> Option<String> {
    p.app
        .document_state
        .panel_ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

fn focused(p: &Project) -> Option<String> {
    info(p, "Focused Pin")
}

fn selected_part(p: &Project) -> Option<uuid::Uuid> {
    match p.app.interaction_state.active_canvas().selected.as_slice() {
        [item] if item.kind == SelectedKind::Symbol => Some(item.uuid),
        _ => None,
    }
}

#[test]
fn a_click_at_a_pin_end_selects_the_part_and_focuses_that_pin() {
    let (mut p, part) = project_with_part(false);
    click(&mut p, 45.1, 50.0);
    assert_eq!(selected_part(&p), Some(part));
    assert_eq!(focused(&p).as_deref(), Some("1"));
    assert_eq!(info(&p, "Pin: 1").as_deref(), Some("P1\tPassive\tNo"));
}

#[test]
fn a_pin_click_opens_a_collapsed_pins_section() {
    let (mut p, _) = project_with_part(false);
    p.app
        .document_state
        .panel_ctx
        .collapsed_sections
        .insert(crate::panels::PINS_SECTION_KEY.to_string());
    click(&mut p, 50.0, 50.0);
    assert!(
        p.app
            .document_state
            .panel_ctx
            .collapsed_sections
            .contains(crate::panels::PINS_SECTION_KEY),
        "a body click leaves the user's collapse alone"
    );
    click(&mut p, 45.1, 50.0);
    assert!(
        !p.app
            .document_state
            .panel_ctx
            .collapsed_sections
            .contains(crate::panels::PINS_SECTION_KEY)
    );
}

#[test]
fn the_pin_rows_take_their_nets_from_the_project_net_index() {
    use std::time::{Duration, Instant};

    use signex_types::schematic::SelectedItem;

    use crate::app::NetNavMsg;
    use crate::app::state::net_nav::hover::DWELL;

    let mut p = project();
    p.select(ROOT_R1, SelectedKind::Symbol);
    p.app.update_selection_info();
    assert!(info(&p, "Pin: 1").is_some());
    assert_eq!(info(&p, "Pin Net: 1"), None, "no index yet, no net column");

    // A hover dwell builds the index, as it does in the app.
    p.app.observe_net_hover(
        Some(SelectedItem::new(id(ROOT_VCC_WIRE), SelectedKind::Wire)),
        (5.0, 0.0),
    );
    if let Some(c) = p.app.document_state.net_nav.hover.candidate.as_mut() {
        c.since = Instant::now()
            .checked_sub(DWELL + Duration::from_millis(50))
            .expect("clock");
    }
    p.send_settled(NetNavMsg::HoverDwellTicked);

    p.app.update_selection_info();
    assert_eq!(info(&p, "Pin Net: 1").as_deref(), Some("VCC"));
}

#[test]
fn a_cold_cache_pin_click_shows_the_net_once_the_index_lands() {
    let (sheet, part) = part_sheet_wired(false, true);
    let mut p = single_sheet_project(sheet);
    // On the pin line, clear of the wire that ends at its tip.
    click(&mut p, 46.5, 50.0);
    assert_eq!(selected_part(&p), Some(part));
    assert_eq!(focused(&p).as_deref(), Some("1"));
    assert_eq!(info(&p, "Pin Net: 1"), None, "the index is not built yet");

    // The build the click started lands; no hover is involved.
    p.app.settle_net_index_blocking();
    assert_eq!(
        focused(&p).as_deref(),
        Some("1"),
        "the focus survives the refill"
    );
    assert_eq!(info(&p, "Pin Net: 1").as_deref(), Some("SIG"));
    assert_eq!(selected_part(&p), Some(part));
}

#[test]
fn another_selection_drops_the_pin_focus_for_good() {
    let (sheet, part) = part_sheet_wired(false, true);
    let mut p = single_sheet_project(sheet);
    click(&mut p, 46.5, 50.0);
    assert_eq!(focused(&p).as_deref(), Some("1"));
    // Select nothing, then the same part again by box: no stale focus.
    click(&mut p, 10.0, 10.0);
    assert_eq!(focused(&p), None);
    p.select_uuid(part, SelectedKind::Symbol);
    p.app.update_selection_info();
    assert_eq!(focused(&p), None);
}

#[test]
fn a_pin_focus_recorded_for_another_sheet_or_placement_is_not_kept() {
    use signex_types::schematic::InstancePath;

    let (mut p, part) = project_with_part(false);
    click(&mut p, 45.1, 50.0);
    let clicked = p
        .app
        .interaction_state
        .clicked_pin
        .clone()
        .expect("a focus");
    assert_eq!(clicked.symbol, part);

    // The same focus, refreshed in the same place, is kept.
    p.app.update_selection_info();
    assert_eq!(focused(&p).as_deref(), Some("1"));

    // The same symbol uuid under another known placement of the sheet is not
    // it; an unknown placement on either side agrees with any.
    let sheet = clicked.sheet.clone();
    let placement_a = InstancePath::root().child(uuid::Uuid::from_u128(5));
    let placement_b = InstancePath::root().child(uuid::Uuid::from_u128(6));
    p.app
        .document_state
        .tab_placements
        .record(&sheet, Some(placement_a.clone()));
    let mut recorded_a = clicked.clone();
    recorded_a.placement = Some(placement_a);
    p.app.interaction_state.clicked_pin = Some(recorded_a.clone());
    p.app.update_selection_info();
    assert_eq!(focused(&p).as_deref(), Some("1"), "same known placement");

    p.app
        .document_state
        .tab_placements
        .record(&sheet, Some(placement_b));
    p.app.update_selection_info();
    assert_eq!(focused(&p), None, "another known placement");
    assert!(p.app.interaction_state.clicked_pin.is_none());

    // Recorded before the hierarchy was known, shown after: still the same.
    let mut recorded_unknown = clicked.clone();
    recorded_unknown.placement = None;
    p.app.interaction_state.clicked_pin = Some(recorded_unknown);
    p.app.update_selection_info();
    assert_eq!(focused(&p).as_deref(), Some("1"), "unknown agrees");

    // Nor is one recorded for another sheet.
    let mut other_sheet = clicked;
    other_sheet.sheet = std::path::PathBuf::from("other.snxsch");
    p.app.interaction_state.clicked_pin = Some(other_sheet);
    p.app.update_selection_info();
    assert_eq!(focused(&p), None);
}

#[test]
fn a_click_on_the_body_centre_focuses_no_pin() {
    let (mut p, part) = project_with_part(false);
    click(&mut p, 50.0, 50.0);
    assert_eq!(selected_part(&p), Some(part));
    assert_eq!(focused(&p), None);
    assert!(info(&p, "Pin: 1").is_some() && info(&p, "Pin: 2").is_some());
}

#[test]
fn a_re_click_on_another_pin_of_the_selected_part_moves_the_focus() {
    let (mut p, part) = project_with_part(false);
    click(&mut p, 50.0, 50.0);
    assert_eq!(selected_part(&p), Some(part));
    click(&mut p, 55.0, 50.0);
    assert_eq!(selected_part(&p), Some(part));
    assert_eq!(focused(&p).as_deref(), Some("2"));
    click(&mut p, 45.0, 50.0);
    assert_eq!(focused(&p).as_deref(), Some("1"));
    click(&mut p, 50.0, 50.0);
    assert_eq!(focused(&p), None, "a body click drops the pin focus");
}

#[test]
fn the_press_then_release_pair_of_clicks_keeps_the_pin_focused() {
    let (mut p, part) = project_with_part(false);
    // A press on an unselected part publishes a click, and the release
    // without a drag publishes it again.
    click(&mut p, 55.0, 50.0);
    click(&mut p, 55.0, 50.0);
    assert_eq!(selected_part(&p), Some(part));
    assert_eq!(focused(&p).as_deref(), Some("2"));
}

#[test]
fn a_pin_the_symbol_hides_is_listed_but_cannot_be_picked() {
    // 0.1 mm from where pin 2 meets the body: its line, when it is drawn.
    let (mut shown, part) = project_with_part(false);
    click(&mut shown, 52.6, 50.0);
    assert_eq!(selected_part(&shown), Some(part));
    assert_eq!(focused(&shown).as_deref(), Some("2"));

    let (mut p, part) = project_with_part(true);
    click(&mut p, 52.6, 50.0);
    assert_eq!(selected_part(&p), Some(part));
    assert_eq!(focused(&p), None);
    assert_eq!(info(&p, "Pin: 2").as_deref(), Some("P2\tPassive\tYes"));
}

#[test]
fn the_body_centre_picks_no_pin_even_at_a_wide_tolerance() {
    use signex_types::schematic::{pin_pick_tolerance_mm, symbol_pin_at};
    let (sheet, part) = part_sheet(false);
    let centre = Point::new(50.0, 50.0);
    // The centre is 2.5 mm from either inner pin end; the body rule, not the
    // tolerance, keeps it from picking a pin.
    for tolerance in [0.64, 1.27, 2.6, pin_pick_tolerance_mm(0.3)] {
        assert_eq!(symbol_pin_at(&sheet, part, centre, tolerance), None);
    }
    // Near the top edge, between the pin lines: still inside the body.
    for tolerance in [0.64, 1.27, 2.6, pin_pick_tolerance_mm(0.3)] {
        for at in [(52.2, 50.8), (52.2, 49.2), (47.8, 50.8), (47.8, 49.2)] {
            assert_eq!(
                symbol_pin_at(&sheet, part, Point::new(at.0, at.1), tolerance),
                None,
                "{at:?} at {tolerance}"
            );
        }
    }
    let near_tip = symbol_pin_at(&sheet, part, Point::new(45.1, 50.0), 0.64);
    assert_eq!(near_tip.map(|p| p.key).as_deref(), Some("1"));
    // Beside the pin line, past the tolerance, is no pin; along it is.
    assert!(symbol_pin_at(&sheet, part, Point::new(46.0, 51.0), 0.64).is_none());
    assert!(symbol_pin_at(&sheet, part, Point::new(46.0, 50.5), 0.64).is_some());
}

#[test]
fn zoomed_far_out_a_pin_stays_pickable_within_the_px_floor() {
    use signex_types::schematic::{pin_pick_tolerance_mm, symbol_pin_at};
    let (sheet, part) = part_sheet(false);
    // At 0.75 px/mm the 4 px radius is 5.3 mm: 5 mm off the tip is still its pin.
    let tolerance = pin_pick_tolerance_mm(0.75);
    let off = Point::new(40.0, 50.0);
    assert!(symbol_pin_at(&sheet, part, off, 0.64).is_none());
    assert_eq!(
        symbol_pin_at(&sheet, part, off, tolerance)
            .map(|p| p.key)
            .as_deref(),
        Some("1")
    );
}

/// The stock resistor (IEC rectangle) at (50, 50): body x +-1.016, y +-1.27,
/// stroke 0.254, pins 1 and 2 at y -+3.81 with length 2.54.
fn stock_r_sheet() -> (SchematicSheet, uuid::Uuid) {
    let (mut sheet, id) = part_sheet(false);
    let lib = sheet.lib_symbols.get_mut("Device:R").expect("lib symbol");
    lib.graphics = vec![LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-1.016, 1.27),
            end: Point::new(1.016, -1.27),
            width: 0.254,
            fill: FillType::None,
        },
    }];
    for (lp, (y, rotation)) in lib.pins.iter_mut().zip([(3.81, 270.0), (-3.81, 90.0)]) {
        lp.pin.position = Point::new(0.0, y);
        lp.pin.rotation = rotation;
        lp.pin.length = 2.54;
    }
    (sheet, id)
}

#[test]
fn the_whole_stock_r_body_belongs_to_the_part_at_every_zoom() {
    use signex_types::schematic::{pin_pick_tolerance_mm, symbol_pin_at};
    let (sheet, part) = stock_r_sheet();
    let pick = |dx: f64, dy: f64, tolerance: f64| {
        symbol_pin_at(&sheet, part, Point::new(50.0 + dx, 50.0 + dy), tolerance).map(|p| p.key)
    };
    // 100 %, 25 %, 10 % and the 0.3 px/mm of the review.
    for scale in [3.0, 0.75, 0.3, 0.15] {
        let tolerance = pin_pick_tolerance_mm(scale);
        for dx in [0.0, 0.5, 0.9, -0.9] {
            for dy in [0.0, 1.0, -1.0, 1.2, -1.2] {
                assert_eq!(
                    pick(dx, dy, tolerance),
                    None,
                    "({dx}, {dy}) inside the body at {scale} px/mm"
                );
            }
        }
        // The pin lines, and the stub just past the body edge, still pick.
        assert_eq!(pick(0.0, -2.5, tolerance).as_deref(), Some("1"));
        assert_eq!(pick(0.0, 2.5, tolerance).as_deref(), Some("2"));
        assert_eq!(pick(0.0, -1.4, tolerance).as_deref(), Some("1"));
    }
}

#[test]
fn a_real_click_near_the_stock_r_top_edge_selects_the_part_with_no_pin() {
    let (sheet, part) = stock_r_sheet();
    let mut p = single_sheet_project(sheet);
    click(&mut p, 50.9, 49.0);
    assert_eq!(selected_part(&p), Some(part));
    assert_eq!(focused(&p), None);
    click(&mut p, 50.0, 47.5);
    assert_eq!(focused(&p).as_deref(), Some("1"));
}
