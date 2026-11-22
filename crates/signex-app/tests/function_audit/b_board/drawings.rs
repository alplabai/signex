//! Rows m98 to m102: the Arc, Zone, String, Dimension and Graphic forms.

use signex_app::panels::PcbPrimitiveMsg;
use signex_app::panels::pcb_primitive_properties::{
    ArcFacts, DimensionFacts, PcbPrimitiveFacts, PourMsg, PrimitiveField as F, PrimitiveTarget,
    ZoneFacts,
};
use signex_engine::pcb::draw::ArcMode;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::support::{Bench, all_kinds, assert_at, open};

const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
const BOTTOM_SILK: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenBottom);

fn area_layer(b: &mut Bench, target: &PrimitiveTarget, layer: PcbLayer) {
    b.primitive(PcbPrimitiveMsg::AreaLayerPicked {
        target: target.clone(),
        layer,
    });
}

// ---- m98 Arc ----

fn arc_facts(b: &Bench) -> ArcFacts {
    match b.facts() {
        PcbPrimitiveFacts::Arc(arc) => arc,
        other => panic!("the arc form, not {other:?}"),
    }
}

#[test]
fn an_arc_width_edit_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(25.0, 20.0);
    let edited = b.one_step(|b| b.enter(&t, F::Width, "0.5"));
    assert_eq!(edited.free.arcs[0].width, 0.5);
}

#[test]
fn an_arc_radius_edit_is_one_undo_step_and_the_form_follows() {
    let mut b = open(all_kinds());
    let t = b.pick(25.0, 20.0);
    b.one_step(|b| {
        b.enter(&t, F::Radius, "8");
        let radius = arc_facts(b).radius.expect("one radius");
        assert!((radius - 8.0).abs() < 1e-6, "{radius}");
    });
}

#[test]
fn an_arc_radius_of_zero_width_of_zero_or_text_is_refused() {
    let mut b = open(all_kinds());
    let t = b.pick(25.0, 20.0);
    b.refused(|b| b.enter(&t, F::Radius, "0"));
    b.refused(|b| b.enter(&t, F::Width, "0"));
    b.refused(|b| b.enter(&t, F::Radius, "abc"));
}

#[test]
fn the_armed_arc_tools_placement_pick_sets_how_it_draws_and_edits_no_board() {
    let mut b = open(all_kinds());
    b.command("place_board_arc");
    b.view_only(|b| b.primitive(PcbPrimitiveMsg::ArcModePicked(ArcMode::Edge)));
    assert_eq!(arc_facts(&b).mode, Some(ArcMode::Edge));
}

// ---- m99 Zone / pour ----

fn zone_facts(b: &Bench) -> ZoneFacts {
    match b.facts() {
        PcbPrimitiveFacts::Zone(zone) => zone,
        other => panic!("the area form, not {other:?}"),
    }
}

#[test]
fn a_pour_clearance_edit_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(55.0, 55.0);
    let edited = b.one_step(|b| b.enter(&t, F::Clearance, "0.3"));
    assert_eq!(edited.zones[0].clearance, 0.3);
}

#[test]
fn a_pour_priority_edit_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(55.0, 55.0);
    let edited = b.one_step(|b| b.enter(&t, F::Priority, "3"));
    assert_eq!(edited.zones[0].priority, 3);
}

#[test]
fn a_negative_clearance_or_a_fractional_priority_is_refused() {
    let mut b = open(all_kinds());
    let t = b.pick(55.0, 55.0);
    b.refused(|b| b.enter(&t, F::Clearance, "-0.1"));
    b.refused(|b| b.enter(&t, F::Priority, "1.5"));
    b.refused(|b| b.enter(&t, F::MinThickness, "abc"));
}

#[test]
fn remove_dead_copper_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(55.0, 55.0);
    let on = !zone_facts(&b).remove_dead_islands.expect("one value");
    b.one_step(|b| {
        b.primitive(PcbPrimitiveMsg::DeadIslandsToggled { target: t, on });
        assert_eq!(zone_facts(b).remove_dead_islands, Some(on));
    });
}

#[test]
fn shelve_is_one_undo_step_and_holds_no_copper() {
    let mut b = open(all_kinds());
    let t = b.pick(55.0, 55.0);
    assert!(zone_facts(&b).copper.is_some(), "the pour is filled");
    let target = t.clone();
    let edited = b.one_step(|b| {
        b.primitive(PcbPrimitiveMsg::Pour(PourMsg::ShelveToggled {
            target,
            shelved: true,
        }));
        assert_eq!(zone_facts(b).shelved, Some(true));
        assert_eq!(zone_facts(b).copper, None, "a shelved pour holds no copper");
    });
    assert!(edited.zones[0].shelved);
    assert!(
        zone_facts(&b).copper.is_some(),
        "undo brings the copper back"
    );
}

#[test]
fn a_shelved_pours_fields_stay_editable() {
    let mut b = open(all_kinds());
    let t = b.pick(55.0, 55.0);
    b.primitive(PcbPrimitiveMsg::Pour(PourMsg::ShelveToggled {
        target: t.clone(),
        shelved: true,
    }));
    b.enter(&t, F::Clearance, "0.4");
    assert_eq!(b.board().zones[0].clearance, 0.4);
}

#[test]
fn repour_changes_nothing_in_the_document() {
    let mut b = open(all_kinds());
    let t = b.pick(55.0, 55.0);
    b.view_only(|b| b.primitive(PcbPrimitiveMsg::Pour(PourMsg::RepourPressed { target: t })));
}

#[test]
fn shelve_all_and_unshelve_all_are_one_undo_step_each() {
    let mut b = open(all_kinds());
    b.pick(55.0, 55.0);
    let board = b.path.clone();
    let shelve = PourMsg::ShelveAllPressed {
        board: board.clone(),
    };
    let edited = b.one_step(|b| b.primitive(PcbPrimitiveMsg::Pour(shelve.clone())));
    assert!(edited.zones.iter().all(|z| z.shelved));
    b.primitive(PcbPrimitiveMsg::Pour(shelve));
    b.primitive(PcbPrimitiveMsg::Pour(PourMsg::UnshelveAllPressed { board }));
    assert!(b.board().zones.iter().all(|z| !z.shelved));
}

// ---- m100 String ----

#[test]
fn a_string_text_edit_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 30.0);
    let edited = b.one_step(|b| b.enter(&t, F::Text, "SENSOR-NODE rev B"));
    assert_eq!(edited.texts[0].text, "SENSOR-NODE rev B");
}

#[test]
fn a_string_height_position_and_rotation_edit_are_one_undo_step_each() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 30.0);
    let edited = b.one_step(|b| b.enter(&t, F::TextHeight, "1.5"));
    assert_eq!(edited.texts[0].font_size, 1.5);
    let edited = b.one_step(|b| b.enter(&t, F::X, "7"));
    assert_at(edited.texts[0].position, 7.0, 30.0);
    let edited = b.one_step(|b| {
        b.primitive(PcbPrimitiveMsg::ValuePicked {
            target: t.clone(),
            field: F::Rotation,
            value: 90.0,
        });
    });
    assert_eq!(edited.texts[0].rotation, 90.0);
}

#[test]
fn a_string_height_of_zero_or_text_is_refused() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 30.0);
    b.refused(|b| b.enter(&t, F::TextHeight, "0"));
    b.refused(|b| b.enter(&t, F::TextHeight, "abc"));
}

#[test]
fn a_string_moves_to_another_silkscreen_and_copper_is_refused() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 30.0);
    let edited = b.one_step(|b| area_layer(b, &t, BOTTOM_SILK));
    assert_eq!(edited.texts[0].layer, "BottomSilkscreen");
    b.refused(|b| area_layer(b, &t, TOP));
}

// ---- m101 Dimension ----

/// Selects the dimension by the middle of its first drawn stroke.
fn pick_dimension(b: &mut Bench) -> PrimitiveTarget {
    let board = b.board();
    let drawing = board.free.dimensions[0].drawing().expect("a drawing");
    let (p, q) = drawing.strokes[0];
    b.pick((p.x + q.x) / 2.0, (p.y + q.y) / 2.0)
}

fn dimension_facts(b: &Bench) -> DimensionFacts {
    match b.facts() {
        PcbPrimitiveFacts::Dimension(d) => d,
        other => panic!("the dimension form, not {other:?}"),
    }
}

#[test]
fn moving_a_dimension_end_changes_its_value_in_one_undo_step() {
    let mut b = open(all_kinds());
    let t = pick_dimension(&mut b);
    b.one_step(|b| {
        b.enter(&t, F::EndX, "45");
        assert_eq!(dimension_facts(b).value.as_deref(), Some("15.00mm"));
    });
    assert_eq!(dimension_facts(&b).value.as_deref(), Some("10.00mm"));
}

#[test]
fn a_dimension_style_edit_is_one_undo_step() {
    let mut b = open(all_kinds());
    let t = pick_dimension(&mut b);
    let edited = b.one_step(|b| b.enter(&t, F::LineWidth, "0.2"));
    assert_eq!(edited.free.dimensions[0].line_width, 0.2);
    let edited = b.one_step(|b| {
        b.primitive(PcbPrimitiveMsg::ValuePicked {
            target: t.clone(),
            field: F::Precision,
            value: 3.0,
        });
    });
    assert_eq!(edited.free.dimensions[0].value_text(), "10.000mm");
}

#[test]
fn a_dimension_precision_past_six_or_ends_on_one_point_are_refused() {
    let mut b = open(all_kinds());
    let t = pick_dimension(&mut b);
    b.refused(|b| {
        b.primitive(PcbPrimitiveMsg::ValuePicked {
            target: t.clone(),
            field: F::Precision,
            value: 7.0,
        });
    });
    b.refused(|b| b.enter(&t, F::EndX, "30"));
    b.refused(|b| b.enter(&t, F::LineWidth, "0"));
}

#[test]
fn a_dimension_goes_on_copper_as_a_drawing_only() {
    let mut b = open(all_kinds());
    let t = pick_dimension(&mut b);
    let tracks = b.board().segments.len();
    let edited = b.one_step(|b| area_layer(b, &t, TOP));
    assert_eq!(edited.free.dimensions[0].layer, "TopCopper");
    assert_eq!(edited.segments.len(), tracks, "it adds no copper");
    let offered = dimension_facts(&b).layers;
    assert!(
        offered.iter().any(|choice| choice.layer == TOP),
        "the Layer row offers copper"
    );
}

// ---- m102 Graphic ----

#[test]
fn a_line_width_and_end_edit_are_one_undo_step_each() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 40.0);
    assert!(matches!(b.facts(), PcbPrimitiveFacts::Graphic(_)));
    let edited = b.one_step(|b| b.enter(&t, F::Width, "0.3"));
    assert_eq!(edited.graphics[0].width, 0.3);
    let edited = b.one_step(|b| b.enter(&t, F::EndX, "12"));
    let end = edited.graphics[0].end.expect("a line has an end");
    assert_at(end, 12.0, 40.0);
}

#[test]
fn a_line_moves_to_another_layer_in_one_undo_step() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 40.0);
    let edited = b.one_step(|b| area_layer(b, &t, BOTTOM_SILK));
    assert_eq!(edited.graphics[0].layer, "BottomSilkscreen");
}

#[test]
fn a_negative_line_width_or_text_is_refused() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 40.0);
    b.refused(|b| b.enter(&t, F::Width, "-0.1"));
    b.refused(|b| b.enter(&t, F::Width, "abc"));
}

#[test]
fn a_drawing_refuses_a_copper_layer() {
    let mut b = open(all_kinds());
    let t = b.pick(5.0, 40.0);
    b.refused(|b| area_layer(b, &t, TOP));
}
