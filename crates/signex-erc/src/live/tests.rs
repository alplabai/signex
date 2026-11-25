use super::*;
use crate::{NetFinding, RuleId, RuleKind, Severity};
use signex_types::schematic::{NoConnect, SelectedItem, SelectedKind, Wire};

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: a,
        end: b,
        ..Default::default()
    }
}

/// Two runs of wire far apart: (0,0)-(10,0) and (0,50)-(10,50).
fn two_runs() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(wire(pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire(pt(0.0, 50.0), pt(10.0, 50.0)));
    sheet
}

fn dangling(at: Point, on: Option<Uuid>) -> Violation {
    let mut v = Violation::new(
        RuleId::builtin(RuleKind::DanglingWire),
        RuleKind::DanglingWire,
        Severity::Warning,
        "dangling wire end",
        at,
    );
    v.primary = on.map(|uuid| SelectedItem::new(uuid, SelectedKind::Wire));
    v
}

/// What tells the findings of these tests apart: where each is and what it
/// is on, in list order.
fn ids(list: &[Violation]) -> Vec<(Key, Option<Uuid>, Severity)> {
    list.iter()
        .map(|v| (pt_key(&v.location), v.primary.map(|p| p.uuid), v.severity))
        .collect()
}

#[test]
fn a_sheet_with_no_earlier_copy_is_checked_whole() {
    let sheet = two_runs();
    let scope = edit_scope(None, &sheet);
    assert!(scope.whole_sheet);
    assert!(scope.covers(&dangling(pt(99.0, 99.0), None)));
}

#[test]
fn an_unchanged_sheet_re_checks_nothing() {
    let sheet = two_runs();
    let scope = edit_scope(Some(&sheet), &sheet.clone());
    assert!(scope.is_empty(), "{scope:?}");
    let cached = vec![dangling(pt(0.0, 0.0), Some(sheet.wires[0].uuid))];
    let fresh = vec![dangling(pt(0.0, 50.0), Some(sheet.wires[1].uuid))];
    assert_eq!(ids(&merge_live(&cached, &fresh, &scope)), ids(&cached));
}

#[test]
fn a_new_wire_brings_its_findings_and_leaves_the_rest_alone() {
    let before = two_runs();
    let mut after = before.clone();
    let new = wire(pt(100.0, 0.0), pt(110.0, 0.0));
    after.wires.push(new.clone());
    let scope = edit_scope(Some(&before), &after);
    assert!(scope.objects.contains(&new.uuid));
    assert!(scope.points.contains(&pt_key(&new.end)));

    let untouched = dangling(pt(0.0, 50.0), Some(before.wires[1].uuid));
    let on_new = dangling(new.end, Some(new.uuid));
    // A finding nobody touched stays out of a live check (quiet): it is not
    // added, and one already shown is kept.
    let merged = merge_live(&[], &[untouched.clone(), on_new.clone()], &scope);
    assert_eq!(ids(&merged), ids(std::slice::from_ref(&on_new)));
    let merged = merge_live(std::slice::from_ref(&untouched), &[on_new.clone()], &scope);
    let mut got = ids(&merged);
    got.sort_by_key(|(at, ..)| *at);
    assert_eq!(got, ids(&[untouched, on_new]));
}

#[test]
fn a_wire_joined_to_a_run_re_checks_the_whole_net() {
    let before = two_runs();
    let mut after = before.clone();
    after.wires.push(wire(pt(10.0, 0.0), pt(20.0, 0.0)));
    let scope = edit_scope(Some(&before), &after);
    // The far end of the old run is on the changed net.
    assert!(scope.objects.contains(&before.wires[0].uuid));
    assert!(scope.points.contains(&pt_key(&pt(0.0, 0.0))));
    // The other run's net did not change.
    assert!(!scope.objects.contains(&before.wires[1].uuid));
    let stale = dangling(pt(10.0, 0.0), Some(before.wires[0].uuid));
    assert!(merge_live(&[stale], &[], &scope).is_empty());
}

#[test]
fn a_deleted_wire_takes_its_findings_with_it() {
    let before = two_runs();
    let mut after = before.clone();
    let gone = after.wires.remove(1);
    let scope = edit_scope(Some(&before), &after);
    let cached = vec![
        dangling(pt(0.0, 50.0), Some(gone.uuid)),
        dangling(pt(0.0, 0.0), Some(before.wires[0].uuid)),
    ];
    assert_eq!(ids(&merge_live(&cached, &[], &scope)), ids(&cached[1..]));
}

#[test]
fn a_placed_no_connect_re_checks_its_point() {
    let before = two_runs();
    let mut after = before.clone();
    after.no_connects.push(NoConnect {
        uuid: Uuid::now_v7(),
        position: pt(0.0, 50.0),
    });
    let scope = edit_scope(Some(&before), &after);
    let cleared = dangling(pt(0.0, 50.0), None);
    assert!(scope.covers(&cleared));
    assert!(merge_live(&[cleared], &[], &scope).is_empty());
}

#[test]
fn a_changed_sheet_variable_re_checks_the_whole_sheet() {
    let before = two_runs();
    let mut after = before.clone();
    after.variables.insert("RAIL".into(), "3V3".into());
    assert!(edit_scope(Some(&before), &after).whole_sheet);
}

#[test]
fn a_named_scope_covers_a_net_finding_by_its_name_only() {
    let scope = EditScope::named(BTreeSet::from(["VCC".to_string()]));
    let mut on_vcc = dangling(pt(5.0, 5.0), None);
    on_vcc.location_space = LocationSpace::Net;
    on_vcc.net = Some(NetFinding {
        name: "VCC".into(),
        pins: Vec::new(),
    });
    let mut unnamed = on_vcc.clone();
    unnamed.net = Some(NetFinding {
        name: String::new(),
        pins: Vec::new(),
    });
    assert!(scope.covers(&on_vcc));
    assert!(!scope.covers(&unnamed));
    assert!(!scope.covers(&dangling(pt(5.0, 5.0), None)));
}

#[test]
fn a_merged_list_is_in_report_order() {
    let scope = EditScope::whole_sheet();
    let warning = dangling(pt(1.0, 0.0), None);
    let mut error = dangling(pt(2.0, 0.0), None);
    error.severity = Severity::Error;
    let merged = merge_live(&[], &[warning.clone(), error.clone()], &scope);
    assert_eq!(ids(&merged), ids(&[error, warning]));
}
