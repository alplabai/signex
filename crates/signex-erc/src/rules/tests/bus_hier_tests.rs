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
