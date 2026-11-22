//! Mockup row 49: a Net Blanket's and a No-ERC directive's Properties.

use super::support::{Bench, bench, empty_sheet};
use signex_app::panels::{BlanketEdit, NoErcRuleEdit, OutlineField, PanelMsg, SelectionFormMsg};
use signex_types::schematic::{BlanketPour, NoErcDirective, Point, SchDirective};
use uuid::Uuid;

const UNUSED_PIN: &str = "builtin::unused_pin";
const OFF_GRID: &str = "builtin::off_grid_pin_or_wire";

fn no_erc_bench() -> (Bench, Uuid) {
    let mut sheet = empty_sheet();
    let id = Uuid::now_v7();
    sheet
        .no_erc_directives
        .push(NoErcDirective::new(id, Point::new(10.0, 20.0)));
    (bench(sheet), id)
}

fn rules(b: &Bench) -> Vec<String> {
    b.sheet().no_erc_directives[0].rules.clone()
}

fn tick(b: &mut Bench, id: Uuid, rule_id: &str, silenced: bool) {
    b.panel(PanelMsg::EditNoErcRules(
        id,
        NoErcRuleEdit::Rule {
            rule_id: rule_id.to_string(),
            silenced,
        },
    ));
}

#[test]
fn m49_unticking_all_rules_opens_the_list_and_leaves_the_directive() {
    let (mut b, id) = no_erc_bench();
    b.panel(PanelMsg::SelectionForm(SelectionFormMsg::AllRulesUnticked(
        id,
    )));
    let form = &b.app.document_state.panel_ctx.selection_form;
    assert_eq!(form.picking_rules_of, Some(id));
    assert!(rules(&b).is_empty());
    assert!(!b.dirty());
}

#[test]
fn m49_ticking_all_rules_back_with_none_picked_closes_the_list() {
    let (mut b, id) = no_erc_bench();
    b.panel(PanelMsg::SelectionForm(SelectionFormMsg::AllRulesUnticked(
        id,
    )));
    b.panel(PanelMsg::SelectionForm(SelectionFormMsg::AllRulesTicked(
        id,
    )));
    let form = &b.app.document_state.panel_ctx.selection_form;
    assert_eq!(form.picking_rules_of, None);
    assert!(!b.dirty());
}

#[test]
fn m49_each_rule_tick_is_one_undo_step() {
    let (mut b, id) = no_erc_bench();
    tick(&mut b, id, UNUSED_PIN, true);
    assert_eq!(rules(&b), [UNUSED_PIN]);
    assert!(b.dirty());
    tick(&mut b, id, OFF_GRID, true);
    assert_eq!(rules(&b).len(), 2);
    b.undo();
    assert_eq!(rules(&b), [UNUSED_PIN]);
    b.undo();
    assert!(rules(&b).is_empty());
    assert!(!b.dirty());
}

#[test]
fn m49_all_rules_clears_the_picked_list_and_ends_picking() {
    let (mut b, id) = no_erc_bench();
    b.panel(PanelMsg::SelectionForm(SelectionFormMsg::AllRulesUnticked(
        id,
    )));
    tick(&mut b, id, UNUSED_PIN, true);
    b.panel(PanelMsg::EditNoErcRules(id, NoErcRuleEdit::AllRules));
    assert!(rules(&b).is_empty());
    let form = &b.app.document_state.panel_ctx.selection_form;
    assert_eq!(form.picking_rules_of, None);
    b.undo();
    assert_eq!(rules(&b), [UNUSED_PIN]);
}

#[test]
fn m49_the_rule_filter_changes_only_the_list() {
    let (mut b, _) = no_erc_bench();
    b.panel(PanelMsg::SelectionForm(SelectionFormMsg::RuleQueryChanged(
        "pin".into(),
    )));
    assert_eq!(
        b.app.document_state.panel_ctx.selection_form.rule_query,
        "pin"
    );
    assert!(!b.dirty());
}

// ── Net Blanket ──

fn blanket_bench() -> (Bench, Uuid) {
    let mut sheet = empty_sheet();
    let id = Uuid::now_v7();
    sheet.directives.push(SchDirective::Blanket {
        uuid: id,
        start: Point::new(10.0, 20.0),
        end: Point::new(60.0, 50.0),
        net_class: None,
        erc_exclusions: Vec::new(),
        track_width: None,
        pour: BlanketPour::Normal,
        include_edge_nets: false,
    });
    (bench(sheet), id)
}

/// The blanket's net class, exclusions, track width, edge-net flag and
/// outline ends.
type BlanketFacts = (Option<String>, Vec<String>, Option<f64>, bool, Point, Point);

fn blanket(b: &Bench) -> BlanketFacts {
    match &b.sheet().directives[0] {
        SchDirective::Blanket {
            net_class,
            erc_exclusions,
            track_width,
            include_edge_nets,
            start,
            end,
            ..
        } => (
            net_class.clone(),
            erc_exclusions.clone(),
            *track_width,
            *include_edge_nets,
            *start,
            *end,
        ),
        other => panic!("not a blanket: {other:?}"),
    }
}

fn edit(b: &mut Bench, id: Uuid, change: BlanketEdit) {
    b.panel(PanelMsg::EditBlanket(id, change));
}

#[test]
fn m49_a_blanket_net_class_is_set_blanked_and_undone() {
    let (mut b, id) = blanket_bench();
    edit(&mut b, id, BlanketEdit::NetClass("Power".into()));
    assert_eq!(blanket(&b).0.as_deref(), Some("Power"));
    assert!(b.dirty());
    edit(&mut b, id, BlanketEdit::NetClass("   ".into()));
    assert_eq!(blanket(&b).0, None, "blank sets none");
    b.undo();
    assert_eq!(blanket(&b).0, None, "one typing run, one undo step");
    assert!(!b.dirty());
}

#[test]
fn m49_a_blanket_rule_exclusion_is_one_undo_step() {
    let (mut b, id) = blanket_bench();
    edit(
        &mut b,
        id,
        BlanketEdit::Rule {
            rule_id: UNUSED_PIN.into(),
            excluded: true,
        },
    );
    assert_eq!(blanket(&b).1, [UNUSED_PIN]);
    b.undo();
    assert!(blanket(&b).1.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m49_a_blanket_track_width_that_is_not_a_width_is_refused() {
    let (mut b, id) = blanket_bench();
    for bad in ["wide", "-1"] {
        edit(&mut b, id, BlanketEdit::TrackWidth(bad.into()));
        assert_eq!(blanket(&b).2, None, "{bad:?}");
    }
    assert!(!b.dirty());
    edit(&mut b, id, BlanketEdit::TrackWidth("0.5".into()));
    assert_eq!(blanket(&b).2, Some(0.5));
}

#[test]
fn m49_a_blanket_outline_of_zero_width_is_refused() {
    let (mut b, id) = blanket_bench();
    let before = (blanket(&b).4, blanket(&b).5);
    edit(
        &mut b,
        id,
        BlanketEdit::Outline(OutlineField::Width, "0".into()),
    );
    assert_eq!((blanket(&b).4, blanket(&b).5), before);
    assert!(!b.dirty());
}

#[test]
fn m49_a_blanket_outline_width_resizes_from_its_start() {
    let (mut b, id) = blanket_bench();
    edit(
        &mut b,
        id,
        BlanketEdit::Outline(OutlineField::Width, "70".into()),
    );
    assert_eq!(blanket(&b).4, Point::new(10.0, 20.0));
    assert_eq!(blanket(&b).5, Point::new(80.0, 50.0));
}

#[test]
fn m49_include_edge_nets_toggles_and_undoes() {
    let (mut b, id) = blanket_bench();
    edit(&mut b, id, BlanketEdit::IncludeEdgeNets(true));
    assert!(blanket(&b).3);
    b.undo();
    assert!(!blanket(&b).3);
}
