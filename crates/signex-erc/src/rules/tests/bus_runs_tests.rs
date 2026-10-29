use super::*;

// ---------------------------------------------------------------------------
// Issue #1049 — two runs on one sheet that name one bus at two widths.
// ---------------------------------------------------------------------------

/// Two disjoint horizontal runs, `a` on the first and `b` on the second.
fn two_runs(a: ErcLabel, b: ErcLabel) -> ErcContext {
    bus_ctx(
        vec![
            bus(pt(0.0, 0.0), pt(10.0, 0.0)),
            bus(pt(0.0, 50.0), pt(10.0, 50.0)),
        ],
        vec![a, b],
    )
}

fn kinds(out: &[crate::diagnostic::Diagnostic]) -> Vec<crate::RuleKind> {
    out.iter().map(|d| d.rule_kind).collect()
}

fn at(mut l: ErcLabel, y: f64) -> ErcLabel {
    l.position = pt(5.0, y);
    l
}

fn run_findings(c: &ErcContext) -> Vec<crate::diagnostic::Diagnostic> {
    let mut out = Vec::new();
    bus_runs_width_mismatch(c, &bus_analysis(c), &mut out);
    out
}

#[test]
fn one_bus_at_two_widths_on_separate_runs_is_a_mismatch() {
    for ty in [LabelType::Net, LabelType::Global, LabelType::Hierarchical] {
        let l = |t: &str, y| {
            at(
                ErcLabel {
                    label_type: ty,
                    uuid: Uuid::now_v7(),
                    ..net_label(t, pt(0.0, 0.0))
                },
                y,
            )
        };
        let out = run_findings(&two_runs(l("D[0..7]", 0.0), l("D[0..3]", 50.0)));
        assert_eq!(
            kinds(&out),
            [crate::RuleKind::BusBitWidthMismatch],
            "{ty:?}"
        );
        let msg = &out[0].message;
        assert!(msg.contains("D[0..7]") && msg.contains("8"), "{msg}");
        assert!(msg.contains("D[0..3]") && msg.contains("4"), "{msg}");
        assert!(out[0].primary.is_some() && out[0].peer.is_some());
    }
}

#[test]
fn overlapping_equal_width_names_on_separate_runs_are_a_name_conflict() {
    let c = two_runs(
        at(net_label("D[0..7]", pt(0.0, 0.0)), 0.0),
        at(net_label("D[4..11]", pt(0.0, 0.0)), 50.0),
    );
    assert_eq!(kinds(&run_findings(&c)), [crate::RuleKind::BusNameConflict]);
}

#[test]
fn separate_runs_that_agree_or_share_no_signal_are_clean() {
    let clean = |a: &str, b: &str| {
        run_findings(&two_runs(
            at(net_label(a, pt(0.0, 0.0)), 0.0),
            at(net_label(b, pt(0.0, 0.0)), 50.0),
        ))
        .is_empty()
    };
    assert!(clean("D[0..7]", "D[7:0]"), "same bus, same members");
    assert!(clean("D[0..7]", "D[8..15]"), "a split shares no signal");
    assert!(clean("D[0..7]", "A[0..3]"), "different buses");
}

#[test]
fn power_labels_and_names_on_one_run_are_not_this_rules_business() {
    let power = |t: &str, y| {
        at(
            ErcLabel {
                label_type: LabelType::Power,
                ..net_label(t, pt(0.0, 0.0))
            },
            y,
        )
    };
    assert!(run_findings(&two_runs(power("D[0..7]", 0.0), power("D[0..3]", 50.0))).is_empty());
    let one_run = bus_ctx(
        vec![bus(pt(0.0, 0.0), pt(10.0, 0.0))],
        vec![
            at(net_label("D[0..7]", pt(0.0, 0.0)), 0.0),
            net_label("D[0..3]", pt(8.0, 0.0)),
        ],
    );
    assert!(
        run_findings(&one_run).is_empty(),
        "same run is bus_bit_width_mismatch's"
    );
}

#[test]
fn the_engine_runs_the_separate_run_rule() {
    let c = two_runs(
        at(net_label("D[0..7]", pt(0.0, 0.0)), 0.0),
        at(net_label("D[0..3]", pt(0.0, 0.0)), 50.0),
    );
    let out = crate::engine::run_all(&c);
    assert!(
        out.iter()
            .any(|d| d.rule_kind == crate::RuleKind::BusBitWidthMismatch),
        "{out:?}"
    );
}
