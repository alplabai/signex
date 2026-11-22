//! Mockup row 153: the Net Parameters and Net Blanket directives and their
//! Properties.

use signex_app::active_bar::ActiveBarAction;
use signex_app::panels::{
    BlanketEdit, DirectivePanelMsg, NetParametersEdit, NetRule, OutlineField, PanelMsg,
};
use signex_net::BlanketCatch;
use signex_types::schematic::{
    BlanketPour, NetRules, Point, SchDirective, SelectedItem, SelectedKind,
};
use uuid::Uuid;

use super::support::{Bench, bench, empty_sheet, net_label, wire, wired_part};

fn flag_bench() -> (Bench, Uuid) {
    let mut sheet = empty_sheet();
    sheet.wires.push(wire((0.0, 0.0), (20.0, 0.0)));
    let id = Uuid::now_v7();
    sheet.directives.push(SchDirective::NetParameters {
        uuid: id,
        position: Point::new(10.0, 0.0),
        net_class: None,
        diff_pair_class: None,
        rules: NetRules::default(),
        show_on_flag: false,
    });
    (bench(sheet), id)
}

fn edit_flag(b: &mut Bench, id: Uuid, edit: NetParametersEdit) {
    b.panel(PanelMsg::Directive(DirectivePanelMsg::NetParametersEdited(
        id, edit,
    )));
}

/// The flag's class, diff-pair class, rules and Show switch.
fn flag(b: &Bench) -> (Option<String>, Option<String>, NetRules, bool) {
    match &b.sheet().directives[0] {
        SchDirective::NetParameters {
            net_class,
            diff_pair_class,
            rules,
            show_on_flag,
            ..
        } => (
            net_class.clone(),
            diff_pair_class.clone(),
            rules.clone(),
            *show_on_flag,
        ),
        other => panic!("not a Net Parameters directive: {other:?}"),
    }
}

/// Select directive `id` so its Properties are built.
fn show_panel(b: &mut Bench, id: Uuid, blanket: bool) {
    b.select(vec![SelectedItem::new(id, SelectedKind::Directive)]);
    // An edit that changes nothing: its handler rebuilds the Properties.
    if blanket {
        b.panel(PanelMsg::EditBlanket(
            id,
            BlanketEdit::IncludeEdgeNets(false),
        ));
    } else {
        edit_flag(b, id, NetParametersEdit::ShowOnFlag(false));
    }
    assert!(
        b.app.document_state.panel_ctx.directive_panel.is_some(),
        "the directive's Properties show"
    );
}

fn draft_text(b: &Bench) -> Option<String> {
    b.app
        .document_state
        .panel_ctx
        .directive_panel
        .as_ref()
        .and_then(|panel| panel.draft.as_ref())
        .map(|(_, text)| text.clone())
}

#[test]
fn m153_net_parameters_go_on_a_wire_and_are_refused_off_one() {
    let mut sheet = empty_sheet();
    sheet.wires.push(wire((0.0, 0.0), (20.0, 0.0)));
    let mut b = bench(sheet);
    b.arm(ActiveBarAction::PlaceParameterSet);
    b.click(60.0, 60.0);
    assert!(b.sheet().directives.is_empty(), "off a wire: nothing");
    assert!(!b.dirty());
    b.click(10.0, 0.0);
    assert_eq!(b.sheet().directives.len(), 1);
    assert!(b.dirty());
    b.undo();
    assert!(b.sheet().directives.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m153_every_rule_value_is_stored_and_undo_returns_to_the_save_point() {
    let (mut b, id) = flag_bench();
    for (rule, text) in [
        (NetRule::Width, "0.8"),
        (NetRule::Clearance, "0.3"),
        (NetRule::ViaDiameter, "0.6"),
        (NetRule::ViaHole, "0.3"),
        (NetRule::Current, "3"),
    ] {
        edit_flag(&mut b, id, NetParametersEdit::Rule(rule, text.to_string()));
    }
    let rules = flag(&b).2;
    assert_eq!(rules.width_mm, Some(0.8));
    assert_eq!(rules.clearance_mm, Some(0.3));
    assert_eq!(rules.via_diameter_mm, Some(0.6));
    assert_eq!(rules.via_hole_mm, Some(0.3));
    assert_eq!(rules.current_a, Some(3.0));
    assert!(b.dirty());
    for _ in 0..5 {
        if b.dirty() {
            b.undo();
        }
    }
    assert_eq!(flag(&b).2, NetRules::default());
    assert!(!b.dirty());
}

#[test]
fn m153_the_classes_and_the_show_switch_are_each_one_undo_step() {
    let (mut b, id) = flag_bench();
    edit_flag(
        &mut b,
        id,
        NetParametersEdit::NetClass(Some("POWER_IN".to_string())),
    );
    edit_flag(
        &mut b,
        id,
        NetParametersEdit::DiffPairClass(Some("USB".to_string())),
    );
    edit_flag(&mut b, id, NetParametersEdit::ShowOnFlag(true));
    let (class, pair, _, show) = flag(&b);
    assert_eq!(class.as_deref(), Some("POWER_IN"));
    assert_eq!(pair.as_deref(), Some("USB"));
    assert!(show);
    b.undo();
    assert!(!flag(&b).3);
    assert_eq!(flag(&b).1.as_deref(), Some("USB"));
    b.undo();
    assert_eq!(flag(&b).1, None);
    assert_eq!(flag(&b).0.as_deref(), Some("POWER_IN"));
    b.undo();
    assert_eq!(flag(&b).0, None);
    assert!(!b.dirty());
}

#[test]
fn m153_a_bad_rule_value_leaves_the_rule_and_the_dirty_mark() {
    let (mut b, id) = flag_bench();
    for bad in ["wide", "-1", "nan", "inf"] {
        edit_flag(
            &mut b,
            id,
            NetParametersEdit::Rule(NetRule::Width, bad.to_string()),
        );
        assert_eq!(flag(&b).2.width_mm, None, "{bad:?}");
    }
    assert!(!b.dirty());
}

/// DECISIONS 2026-11-22: a refused number stays in the box as typed.
#[test]
fn m153_a_refused_rule_value_stays_in_the_box_as_typed() {
    let (mut b, id) = flag_bench();
    show_panel(&mut b, id, false);
    for bad in ["wide", "-1"] {
        edit_flag(
            &mut b,
            id,
            NetParametersEdit::Rule(NetRule::Width, bad.to_string()),
        );
        assert_eq!(draft_text(&b).as_deref(), Some(bad), "the box keeps it");
    }
}

fn blanket_bench() -> (Bench, Uuid) {
    let mut sheet = empty_sheet();
    // SW lies inside the outline, VOUT only crosses its right edge.
    sheet.wires.push(wire((20.0, 20.0), (30.0, 20.0)));
    sheet.labels.push(net_label("SW", (20.0, 20.0)));
    sheet.wires.push(wire((50.0, 30.0), (70.0, 30.0)));
    sheet.labels.push(net_label("VOUT", (70.0, 30.0)));
    wired_part(&mut sheet, "R1", (30.0, 20.0));
    wired_part(&mut sheet, "R2", (50.0, 30.0));
    let id = Uuid::now_v7();
    sheet.directives.push(SchDirective::Blanket {
        uuid: id,
        start: Point::new(10.0, 10.0),
        end: Point::new(60.0, 50.0),
        net_class: None,
        erc_exclusions: Vec::new(),
        track_width: None,
        pour: BlanketPour::Normal,
        include_edge_nets: false,
    });
    (bench(sheet), id)
}

fn edit_blanket(b: &mut Bench, id: Uuid, edit: BlanketEdit) {
    b.panel(PanelMsg::EditBlanket(id, edit));
}

fn pour(b: &Bench) -> BlanketPour {
    match &b.sheet().directives[0] {
        SchDirective::Blanket { pour, .. } => *pour,
        other => panic!("not a blanket: {other:?}"),
    }
}

fn track_width(b: &Bench) -> Option<f64> {
    match &b.sheet().directives[0] {
        SchDirective::Blanket { track_width, .. } => *track_width,
        other => panic!("not a blanket: {other:?}"),
    }
}

fn catch_of(b: &Bench, net: &str) -> Option<BlanketCatch> {
    b.app
        .document_state
        .panel_ctx
        .directive_panel
        .as_ref()?
        .nets
        .iter()
        .find(|row| row.name == net)
        .map(|row| row.catch)
}

#[test]
fn m153_two_clicks_place_a_blanket_and_undo_takes_it_away() {
    let mut b = bench(empty_sheet());
    b.arm(ActiveBarAction::PlaceBlanket);
    b.click(10.0, 10.0);
    assert!(b.sheet().directives.is_empty(), "the first corner only");
    b.click(50.0, 40.0);
    assert_eq!(b.sheet().directives.len(), 1);
    assert!(b.dirty());
    b.undo();
    assert!(b.sheet().directives.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m153_pour_on_nets_switches_to_keep_short_and_undo_restores_normal() {
    let (mut b, id) = blanket_bench();
    edit_blanket(&mut b, id, BlanketEdit::Pour(BlanketPour::KeepShort));
    assert_eq!(pour(&b), BlanketPour::KeepShort);
    assert!(b.dirty());
    b.undo();
    assert_eq!(pour(&b), BlanketPour::Normal);
    assert!(!b.dirty());
}

#[test]
fn m153_a_blanket_track_width_is_stored_and_a_bad_one_refused() {
    let (mut b, id) = blanket_bench();
    edit_blanket(&mut b, id, BlanketEdit::TrackWidth("wide".to_string()));
    assert_eq!(track_width(&b), None);
    assert!(!b.dirty());
    edit_blanket(&mut b, id, BlanketEdit::TrackWidth("1.0".to_string()));
    assert_eq!(track_width(&b), Some(1.0));
    b.undo();
    assert_eq!(track_width(&b), None);
}

/// DECISIONS 2026-11-22: a refused number stays in the box as typed.
#[test]
fn m153_a_refused_blanket_size_stays_in_the_box_as_typed() {
    let (mut b, id) = blanket_bench();
    show_panel(&mut b, id, true);
    for bad in ["wide", "0", "-4"] {
        let before = b.sheet().directives[0].clone();
        edit_blanket(
            &mut b,
            id,
            BlanketEdit::Outline(OutlineField::Width, bad.to_string()),
        );
        assert_eq!(b.sheet().directives[0], before, "{bad:?} changes nothing");
        assert_eq!(draft_text(&b).as_deref(), Some(bad), "the box keeps it");
    }
    assert!(!b.dirty());
}

#[test]
fn m153_the_blanket_lists_a_net_inside_and_flags_one_that_only_crosses_its_edge() {
    let (mut b, id) = blanket_bench();
    show_panel(&mut b, id, true);
    assert_eq!(catch_of(&b, "SW"), Some(BlanketCatch::Inside));
    assert_eq!(catch_of(&b, "VOUT"), Some(BlanketCatch::EdgeOnly));
}

#[test]
fn m153_the_edge_checkbox_catches_the_crossing_net_and_undo_lets_it_go() {
    let (mut b, id) = blanket_bench();
    show_panel(&mut b, id, true);
    edit_blanket(&mut b, id, BlanketEdit::IncludeEdgeNets(true));
    assert_eq!(catch_of(&b, "VOUT"), Some(BlanketCatch::EdgeIncluded));
    assert!(b.dirty());
    b.undo();
    // Undo clears the selection, and the Properties with it.
    show_panel(&mut b, id, true);
    assert_eq!(catch_of(&b, "VOUT"), Some(BlanketCatch::EdgeOnly));
    assert!(!b.dirty());
}
