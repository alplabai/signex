use super::*;

// ---------------------------------------------------------------------------
// Issue #1049 — a bus through a sheet pin must be as wide as the child's port.
// ---------------------------------------------------------------------------

/// A parent with one sheet symbol `Sub` (`child.snxsch`) whose only pin is
/// `pin`, over a child sheet holding `child_labels`.
fn hierarchy(pin: &str, child_labels: Vec<ErcLabel>) -> ErcContext {
    let mut parent = ctx(Vec::new(), Vec::new(), Vec::new(), Vec::new());
    parent.child_sheets.push(crate::context::ErcChildSheet {
        uuid: Uuid::new_v4(),
        name: "Sub".into(),
        filename: "child.snxsch".into(),
        position: pt(0.0, 0.0),
        pins: vec![crate::context::ErcSheetPin {
            uuid: Uuid::new_v4(),
            name: pin.into(),
            position: pt(0.0, 40.0),
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
        uuid: Uuid::new_v4(),
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
    assert!(out[0].peer.is_some() || out[0].primary.is_some());
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
