use super::*;

// ---------------------------------------------------------------------------
// Issue #1049 — a bus through a sheet pin must be as wide as the child's port.
// ---------------------------------------------------------------------------

/// A parent with one sheet symbol `Sub` (`child.snxsch`) whose only pin is
/// `pin`, over a child sheet holding `child_labels`.
fn hierarchy(pin: &str, child_labels: Vec<ErcLabel>) -> ErcContext {
    let mut parent = ctx(Vec::new(), Vec::new(), Vec::new(), Vec::new());
    parent.child_sheets.push(crate::context::ErcChildSheet {
        uuid: Uuid::now_v7(),
        name: "Sub".into(),
        filename: "child.snxsch".into(),
        position: pt(0.0, 0.0),
        pins: vec![crate::context::ErcSheetPin {
            uuid: Uuid::now_v7(),
            name: pin.into(),
            position: pt(0.0, 40.0),
            direction: String::new(),
            connected: false,
        }],
    });
    let mut child = ctx(Vec::new(), Vec::new(), Vec::new(), Vec::new());
    child.labels = child_labels;
    parent
        .children
        .insert("child.snxsch".into(), std::sync::Arc::new(child));
    parent
}

fn hier_label(text: &str) -> ErcLabel {
    ErcLabel {
        label_type: LabelType::Hierarchical,
        ..net_label(text, pt(0.0, 40.0))
    }
}

/// The width rule and the pin/label match rule, as the engine runs them.
fn findings(c: &ErcContext) -> Vec<crate::diagnostic::Diagnostic> {
    let mut out = Vec::new();
    bus_port_width_mismatch(c, &mut out);
    bad_hier_sheet_pin(c, &mut out);
    out
}

fn kinds(out: &[crate::diagnostic::Diagnostic]) -> Vec<crate::RuleKind> {
    out.iter().map(|d| d.rule_kind).collect()
}

#[test]
fn a_bus_pin_wider_than_the_child_port_is_a_width_mismatch_naming_both_sides() {
    let c = hierarchy("D[0..7]", vec![hier_label("D[0..3]")]);
    let out = findings(&c);
    assert_eq!(kinds(&out), [crate::RuleKind::BusBitWidthMismatch]);
    let msg = &out[0].message;
    assert!(msg.contains("D[0..7]") && msg.contains("8"), "{msg}");
    assert!(msg.contains("D[0..3]") && msg.contains("4"), "{msg}");
    assert!(msg.contains("child.snxsch"), "{msg}");
}

#[test]
fn a_bus_pin_and_port_of_one_width_are_clean() {
    let c = hierarchy("D[0..7]", vec![hier_label("D[0..7]")]);
    assert!(findings(&c).is_empty());
}

#[test]
fn the_same_width_over_different_members_is_still_a_mismatch() {
    let c = hierarchy("D[0..3]", vec![hier_label("D[4..7]")]);
    assert_eq!(kinds(&findings(&c)), [crate::RuleKind::BusBitWidthMismatch]);
}

#[test]
fn a_plain_pin_is_not_compared_as_a_bus() {
    let c = hierarchy("CLK", vec![hier_label("CLK")]);
    assert!(findings(&c).is_empty());
}

#[test]
fn a_width_mismatch_has_no_peer_on_the_parent_canvas() {
    // The child port lives on another sheet; a peer would target the wrong
    // object on the parent canvas.
    let c = hierarchy("D[0..7]", vec![hier_label("D[0..3]")]);
    let out = findings(&c);
    assert!(out[0].peer.is_none());
    assert!(out[0].primary.is_some());
}

fn global_bus(text: &str) -> ErcLabel {
    ErcLabel {
        uuid: Uuid::now_v7(),
        label_type: LabelType::Global,
        ..net_label(text, pt(0.0, 0.0))
    }
}

fn global_findings(c: &ErcContext) -> Vec<crate::diagnostic::Diagnostic> {
    let mut out = Vec::new();
    global_bus_width_mismatch(c, &mut out);
    out
}

#[test]
fn one_global_bus_at_two_widths_across_sheets_is_a_mismatch() {
    let mut c = hierarchy("CLK", vec![global_bus("D[0..3]")]);
    c.labels.push(global_bus("D[0..7]"));
    let out = global_findings(&c);
    assert_eq!(kinds(&out), [crate::RuleKind::BusBitWidthMismatch]);
    let msg = &out[0].message;
    assert!(msg.contains("D[0..7]") && msg.contains("D[0..3]"), "{msg}");
    assert!(out[0].primary.is_some());
    assert!(out[0].peer.is_none());
    assert_eq!(
        out[0].message,
        "Port bus width mismatch: 'D[0..7]' on this sheet carries 8 signals, 'D[0..3]' on 'child.snxsch' carries 4; only the shared signals join"
    );
}

#[test]
fn global_buses_of_one_width_or_different_names_are_clean() {
    let mut c = hierarchy("CLK", vec![global_bus("D[0..7]"), global_bus("A[0..3]")]);
    c.labels.push(global_bus("D[0..7]"));
    assert!(global_findings(&c).is_empty());
}

#[test]
fn local_net_label_buses_are_never_compared_across_sheets() {
    let mut c = hierarchy("CLK", vec![net_label("D[0..3]", pt(0.0, 0.0))]);
    c.labels.push(net_label("D[0..7]", pt(0.0, 0.0)));
    assert!(global_findings(&c).is_empty());
}

// ---------------------------------------------------------------------------
// Issue #1206 — keyed hierarchies: who reports a Port bus clash, and how.
// ---------------------------------------------------------------------------

fn keyed(name: &str, labels: Vec<ErcLabel>) -> ErcContext {
    let mut c = ctx(Vec::new(), Vec::new(), Vec::new(), Vec::new());
    c.labels = labels;
    c.key = Some(signex_net::SheetKey::new(name));
    c
}

fn with_child(mut parent: ErcContext, file: &str, child: ErcContext) -> ErcContext {
    parent
        .children
        .insert(file.into(), std::sync::Arc::new(child));
    parent
}

fn key(name: &str) -> Option<signex_net::SheetKey> {
    Some(signex_net::SheetKey::new(name))
}

#[test]
fn a_clash_inside_one_child_is_silent_at_the_root_and_reported_by_the_child() {
    let grandchild = keyed("g", vec![global_bus("D[0..3]")]);
    let a = with_child(
        keyed("a", vec![global_bus("D[0..7]")]),
        "g.snxsch",
        grandchild,
    );
    let a_run = global_findings(&a);
    let root = with_child(keyed("root", Vec::new()), "a.snxsch", a);
    assert!(global_findings(&root).is_empty());

    assert_eq!(a_run.len(), 1);
    assert!(a_run[0].primary.is_some());
    assert!(a_run[0].peer.is_none());
    assert_eq!(a_run[0].owner_sheet, None);
}

#[test]
fn a_clash_across_two_children_is_reported_by_the_root_owned_by_the_first_child() {
    let a = keyed("a", vec![global_bus("D[0..7]")]);
    let b = keyed("b", vec![global_bus("D[0..3]")]);
    let root = with_child(
        with_child(keyed("root", Vec::new()), "a.snxsch", a),
        "b.snxsch",
        b,
    );
    let out = global_findings(&root);
    assert_eq!(out.len(), 1);
    assert!(out[0].primary.is_none());
    assert!(out[0].peer.is_none());
    assert_eq!(out[0].owner_sheet, key("a"));
    assert_eq!(
        out[0].message,
        "Port bus width mismatch: 'D[0..7]' on 'a.snxsch' carries 8 signals, 'D[0..3]' on 'b.snxsch' carries 4; only the shared signals join"
    );
}

#[test]
fn a_clash_with_the_root_names_the_root_label_as_primary_and_no_peer() {
    let child = keyed("a", vec![global_bus("D[0..3]")]);
    let root_label = global_bus("D[0..7]");
    let uuid = root_label.uuid;
    let root = with_child(keyed("root", vec![root_label]), "a.snxsch", child);
    let out = global_findings(&root);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].primary.as_ref().map(|s| s.uuid), Some(uuid));
    assert!(out[0].peer.is_none());
    assert_eq!(out[0].owner_sheet, None);
}

#[test]
fn every_differing_sheet_counts_for_the_scope_not_just_the_first_pair() {
    // a holds D[0..7] and D[0..3]; b holds D[0..1]. The first differing pair
    // is confined to `a`, but b's D[0..1] against a is a root-level clash.
    let a = keyed("a", vec![global_bus("D[0..7]"), global_bus("D[0..3]")]);
    let b = keyed("b", vec![global_bus("D[0..1]")]);
    let root = with_child(
        with_child(keyed("root", Vec::new()), "a.snxsch", a),
        "b.snxsch",
        b,
    );
    let out = global_findings(&root);
    assert_eq!(out.len(), 1);
    assert!(out[0].primary.is_none());
    assert!(out[0].peer.is_none());
    assert_eq!(out[0].owner_sheet, key("a"));
    assert_eq!(
        out[0].message,
        "Port bus width mismatch: 'D[0..7]' on 'a.snxsch' carries 8 signals, 'D[0..1]' on 'b.snxsch' carries 2; only the shared signals join"
    );
}

#[test]
fn a_root_clash_skips_a_pair_inside_one_child_and_names_the_sheet_outside_it() {
    // Scope order is root, a, g, b. The first differing pair, a/g, is confined
    // to `a` and is `a`'s own finding; the root reports a against b instead.
    let g = keyed("g", vec![global_bus("D[0..3]")]);
    let a = with_child(keyed("a", vec![global_bus("D[0..7]")]), "g.snxsch", g);
    let b = keyed("b", vec![global_bus("D[0..1]")]);
    let root = with_child(
        with_child(keyed("root", Vec::new()), "a.snxsch", a.clone()),
        "b.snxsch",
        b,
    );
    let out = global_findings(&root);
    assert_eq!(out.len(), 1);
    assert!(out[0].primary.is_none());
    assert_eq!(out[0].owner_sheet, key("a"));
    assert_eq!(
        out[0].message,
        "Port bus width mismatch: 'D[0..7]' on 'a.snxsch' carries 8 signals, 'D[0..1]' on 'b.snxsch' carries 2; only the shared signals join"
    );
    // a's own run still reports the a/g pair, so the two never coincide.
    let a_run = global_findings(&a);
    assert_eq!(a_run.len(), 1);
    assert!(
        a_run[0].message.contains("'g.snxsch'"),
        "{}",
        a_run[0].message
    );
}

#[test]
fn two_names_on_one_sheet_are_left_to_the_same_sheet_bus_rule() {
    let c = keyed("root", vec![global_bus("D[0..7]"), global_bus("D[0..3]")]);
    assert!(global_findings(&c).is_empty());
}

#[test]
fn equally_wide_ports_with_different_signals_are_a_name_conflict() {
    let mut c = hierarchy("CLK", vec![global_bus("D[4..7]")]);
    c.labels.push(global_bus("D[0..3]"));
    let out = global_findings(&c);
    assert_eq!(kinds(&out), [crate::RuleKind::BusNameConflict]);
    assert_eq!(
        out[0].message,
        "Port bus names disagree: 'D[0..3]' on this sheet and 'D[4..7]' on 'child.snxsch' each carry 4 signals, but different ones"
    );
}
