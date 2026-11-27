//! #2940: each DRC fix clears its findings with no new one, as one undo
//! step, and leaves a locked item, a waived finding and a rule with no fix
//! alone. The fixtures of the resizing fixes are here, those of the moves in
//! `move_tests`.

use serde_json::{Value, json};
use signex_drc::{DrcRules, Limit, NetScope, UserRule, ViolationKind};
use signex_types::object_id::SequentialIds;
use signex_types::pcb::{MaskExpansion, PcbBoard};
use signex_types::pcb_options::DrcWaiver;
use signex_types::violation::Severity;
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

/// A 0.25 mm top track.
pub(super) fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> Value {
    json!({
        "uuid": Uuid::from_u128(n), "start": {"x": a.0, "y": a.1}, "end": {"x": b.0, "y": b.1},
        "width": 0.25, "layer": "TopCopper", "net": net
    })
}

/// A through via, 0.6 mm across with a 0.3 mm hole.
pub(super) fn via(n: u128, at: (f64, f64), net: u32) -> Value {
    json!({
        "uuid": Uuid::from_u128(n), "position": {"x": at.0, "y": at.1},
        "diameter": 0.6, "drill": 0.3, "layers": ["TopCopper", "BottomCopper"], "net": net
    })
}

/// A footprint at (`x`, `y`) holding `pad`.
pub(super) fn part(n: u128, at: (f64, f64), pad: Value) -> Value {
    json!({
        "uuid": Uuid::from_u128(n), "reference": format!("U{n}"), "value": "",
        "footprint_id": "Part", "layer": "TopCopper", "rotation": 0.0,
        "position": {"x": at.0, "y": at.1}, "pads": [pad]
    })
}

/// A 1 mm square top pad of net A at the footprint's origin.
pub(super) fn smd_pad(n: u128) -> Value {
    json!({
        "uuid": Uuid::from_u128(n), "number": "1", "pad_type": "smd", "shape": "rect",
        "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0}, "drill": null,
        "layers": ["TopCopper"], "net": {"number": 1, "name": "A"}
    })
}

/// Nets A (1), B (2) and C (3), and `items` (`segments`, `vias`, ...).
pub(super) fn board(items: Value) -> PcbBoard {
    let mut body = json!({
        "uuid": Uuid::from_u128(1),
        "nets": [
            {"number": 1, "name": "A"}, {"number": 2, "name": "B"}, {"number": 3, "name": "C"}
        ]
    });
    for (key, value) in items.as_object().expect("an object of item lists") {
        body[key] = value.clone();
    }
    serde_json::from_value(body).expect("a board")
}

/// A 0.2 mm clearance and a board-wide written rule of `limits`.
pub(super) fn rules(board: &PcbBoard, limits: Vec<Limit>) -> DrcRules {
    let rule = (!limits.is_empty()).then(|| UserRule {
        name: "fab".into(),
        severity: Severity::Error,
        message: None,
        nets: NetScope::Board,
        layers: None,
        pair: None,
        limits,
    });
    DrcRules::new(200_000).with_user_rules(board, &[], rule)
}

/// Fix what `scope` names on `board`.
pub(super) fn fix(board: &PcbBoard, rules: &DrcRules, scope: FixScope) -> FixRun {
    let models = BodyModels::new();
    fix_findings(board, rules, &models, &scope, &mut SequentialIds::default()).expect("a scope")
}

/// Fix every finding of `rule` on `board`.
pub(super) fn fix_every(board: &PcbBoard, rules: &DrcRules, rule: FixRule) -> FixRun {
    fix(board, rules, FixScope::Every(Some(rule)))
}

/// `board` as text, to compare two boards.
pub(super) fn text(board: &PcbBoard) -> String {
    serde_json::to_string(board).expect("a board writes")
}

/// `run` fixed what it was asked and left nothing, its board checks with no
/// finding at all, and its commands are one undo step on `before`.
pub(super) fn assert_clean_fix(before: &PcbBoard, rules: &DrcRules, run: &FixRun) {
    assert!(run.left.is_empty(), "left: {:?}", run.left);
    assert!(!run.fixed.is_empty(), "nothing fixed");
    assert!(
        run.report.violations.is_empty(),
        "{:?}",
        run.report.violations
    );
    assert!(signex_drc::run(&run.board, rules).violations.is_empty());
    let mut engine = BoardEngine::new(before.clone(), 1);
    engine
        .execute_batch(run.commands.clone())
        .expect("the fixes apply");
    assert_eq!(text(engine.board()), text(&run.board), "the board planned");
    assert_eq!(engine.undo_len(), 1, "one undo step");
    engine.undo().expect("undo").expect("a step");
    assert_eq!(text(engine.board()), text(before), "undo gives it back");
}

/// `run` left one finding of `kind` for `reason` and changed nothing.
pub(super) fn assert_left(before: &PcbBoard, run: &FixRun, kind: ViolationKind, reason: &str) {
    assert!(run.fixed.is_empty(), "fixed: {:?}", run.fixed);
    assert_eq!(run.left.len(), 1, "{:?}", run.left);
    assert_eq!(run.left[0].violation.kind, kind);
    assert_eq!(
        run.left[0].reason.code(),
        reason,
        "{:?}",
        run.left[0].reason
    );
    assert!(run.commands.is_empty());
    assert_eq!(text(&run.board), text(before));
}

#[test]
fn a_rule_token_reads_with_or_without_its_prefix() {
    for rule in FixRule::ALL {
        assert_eq!(FixRule::parse(rule.token()), Some(rule));
        assert_eq!(FixRule::parse(rule.kind().rule_id()), Some(rule));
        assert_eq!(FixRule::of(rule.kind()), Some(rule));
    }
    assert_eq!(FixRule::parse("Track_Width"), Some(FixRule::TrackWidth));
    assert_eq!(FixRule::parse("short"), None);
    assert_eq!(FixRule::of(ViolationKind::Short), None);
}

#[test]
fn a_narrow_track_is_set_to_the_minimum_width() {
    let before = board(json!({"segments": [track(11, (0.0, 0.0), (5.0, 0.0), 1)]}));
    let rules = rules(&before, vec![Limit::MinTrackWidth(300_000)]);
    let run = fix_every(&before, &rules, FixRule::TrackWidth);
    assert_clean_fix(&before, &rules, &run);
    assert_eq!(run.board.segments[0].width, 0.3);
    assert_eq!(run.fixed[0].changed, [Uuid::from_u128(11).to_string()]);
}

#[test]
fn a_wide_track_is_set_to_the_maximum_width() {
    let before = board(json!({"segments": [track(11, (0.0, 0.0), (5.0, 0.0), 1)]}));
    let rules = rules(&before, vec![Limit::MaxTrackWidth(200_000)]);
    let run = fix_every(&before, &rules, FixRule::TrackWidth);
    assert_clean_fix(&before, &rules, &run);
    assert_eq!(run.board.segments[0].width, 0.2);
}

#[test]
fn a_locked_track_keeps_its_width() {
    let mut locked = track(11, (0.0, 0.0), (5.0, 0.0), 1);
    locked["locked"] = json!(true);
    let before = board(json!({"segments": [locked]}));
    let rules = rules(&before, vec![Limit::MinTrackWidth(300_000)]);
    let run = fix_every(&before, &rules, FixRule::TrackWidth);
    assert_left(&before, &run, ViolationKind::TrackWidth, "locked");
    let LeftReason::Locked(names) = &run.left[0].reason else {
        unreachable!()
    };
    assert_eq!(names, &[format!("track {}", Uuid::from_u128(11))]);
}

#[test]
fn a_via_grows_to_the_minimum_ring() {
    let before = board(json!({"vias": [via(21, (0.0, 0.0), 1)]}));
    let rules = rules(&before, vec![Limit::MinAnnularRing(200_000)]);
    let run = fix_every(&before, &rules, FixRule::AnnularRing);
    assert_clean_fix(&before, &rules, &run);
    assert_eq!(run.board.vias[0].diameter, 0.7, "0.3 mm hole + 2 x 0.2 mm");
    assert_eq!(run.board.vias[0].drill, 0.3, "the hole stays");
}

#[test]
fn a_through_pad_grows_to_the_minimum_ring_and_a_locked_part_does_not() {
    let pad = json!({
        "uuid": Uuid::from_u128(32), "number": "1", "pad_type": "thru", "shape": "circle",
        "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
        "drill": {"diameter": 0.8}, "layers": ["TopCopper", "BottomCopper"],
        "net": {"number": 1, "name": "A"}
    });
    let before = board(json!({"footprints": [part(31, (2.0, 2.0), pad.clone())]}));
    let rules = rules(&before, vec![Limit::MinAnnularRing(200_000)]);
    let run = fix_every(&before, &rules, FixRule::AnnularRing);
    assert_clean_fix(&before, &rules, &run);
    let grown = &run.board.footprints[0].pads[0];
    assert_eq!((grown.size.x, grown.size.y), (1.2, 1.2));
    let id = format!("{}/{}", Uuid::from_u128(31), Uuid::from_u128(32));
    assert_eq!(run.fixed[0].changed, [id]);

    let mut fixed_part = part(31, (2.0, 2.0), pad);
    fixed_part["locked"] = json!(true);
    let before = board(json!({"footprints": [fixed_part]}));
    let run = fix_every(&before, &rules, FixRule::AnnularRing);
    assert_left(&before, &run, ViolationKind::AnnularRing, "locked");
}

#[test]
fn a_locked_via_keeps_its_ring() {
    let mut locked = via(21, (0.0, 0.0), 1);
    locked["locked"] = json!(true);
    let before = board(json!({"vias": [locked]}));
    let rules = rules(&before, vec![Limit::MinAnnularRing(200_000)]);
    let run = fix_every(&before, &rules, FixRule::AnnularRing);
    assert_left(&before, &run, ViolationKind::AnnularRing, "locked");
}

/// Two open vias of two nets, 0.3 mm of mask between them under a 0.4 mm
/// sliver rule: the first via's opening shrinks past it.
#[test]
fn a_mask_sliver_is_widened_by_shrinking_a_via_opening() {
    let before = board(json!({"vias": [via(21, (0.0, 0.0), 1), via(22, (0.9, 0.0), 2)]}));
    let rules = rules(&before, vec![Limit::MinMaskSliver(400_000)]);
    let run = fix_every(&before, &rules, FixRule::MaskSliver);
    assert_clean_fix(&before, &rules, &run);
    let shrunk = run.board.vias[0].attributes;
    assert_eq!(shrunk.top.expansion, MaskExpansion::Manual { mm: -0.101 });
    assert_eq!(
        shrunk.bottom.expansion,
        MaskExpansion::Manual { mm: -0.101 }
    );
    assert!(
        run.board.vias[1].attributes.is_default(),
        "one opening is enough"
    );
}

#[test]
fn a_mask_sliver_between_locked_vias_is_left() {
    let (mut a, mut b) = (via(21, (0.0, 0.0), 1), via(22, (0.9, 0.0), 2));
    a["locked"] = json!(true);
    b["locked"] = json!(true);
    let before = board(json!({"vias": [a, b]}));
    let rules = rules(&before, vec![Limit::MinMaskSliver(400_000)]);
    let run = fix_every(&before, &rules, FixRule::MaskSliver);
    assert_left(&before, &run, ViolationKind::MaskSliver, "locked");
}

#[test]
fn a_waived_finding_is_never_fixed() {
    let mut before = board(json!({"segments": [track(11, (0.0, 0.0), (5.0, 0.0), 1)]}));
    let rules = rules(&before, vec![Limit::MinTrackWidth(300_000)]);
    let finding = signex_drc::run(&before, &rules).violations[0].clone();
    let key = signex_drc::waiver_key(&finding);
    before.options.drc_waivers.push(DrcWaiver {
        key: key.clone(),
        rule: signex_drc::rule_id(&finding).to_string(),
        note: "thin on purpose".into(),
        fingerprint: signex_drc::fingerprint(&finding, &before),
        author: String::new(),
    });
    let run = fix(&before, &rules, FixScope::One(key));
    assert_left(&before, &run, ViolationKind::TrackWidth, "waived");
    assert_eq!(
        before.options.drc_waivers.len(),
        run.board.options.drc_waivers.len()
    );
}

#[test]
fn a_finding_whose_rule_has_no_fix_is_left_and_named() {
    // Two tracks of two nets crossing: a short, which no fix touches.
    let before = board(json!({"segments": [
        track(11, (0.0, 0.0), (5.0, 0.0), 1),
        track(12, (2.5, -1.0), (2.5, 1.0), 2),
    ]}));
    let rules = rules(&before, Vec::new());
    let run = fix(&before, &rules, FixScope::Every(None));
    let short = run
        .left
        .iter()
        .find(|l| l.violation.kind == ViolationKind::Short)
        .expect("the short is left");
    assert_eq!(short.reason, LeftReason::NoFix);
    assert_eq!(short.reason.code(), "no_fix");
    assert!(run.commands.is_empty());
}

#[test]
fn an_unknown_finding_key_is_refused() {
    let before = board(json!({"segments": [track(11, (0.0, 0.0), (5.0, 0.0), 1)]}));
    let rules = rules(&before, Vec::new());
    let scope = FixScope::One("feed".into());
    let models = BodyModels::new();
    let refused = fix_findings(
        &before,
        &rules,
        &models,
        &scope,
        &mut SequentialIds::default(),
    );
    assert_eq!(
        refused.err(),
        Some(FixScopeError::NoSuchFinding("feed".into()))
    );
}

/// #2939, #2940: a run checks the board with the parts' 3D bodies, as Run
/// DRC and `check_drc` do: a body clearance finding only the library model
/// shows is in its check, judged by its new-finding test and left (it has
/// no fix); without the models the run does not see it.
#[test]
fn a_run_checks_the_board_with_the_parts_3d_bodies() {
    use signex_drc::{BodyModel, DrcReport};
    use signex_types::part::FootprintSourceRef;

    let mut before = board(json!({"footprints": [
        part(31, (0.0, 0.0), smd_pad(32)),
        part(41, (5.0, 0.0), smd_pad(42)),
    ]}));
    let source = FootprintSourceRef {
        library: Uuid::from_u128(7),
        footprint: Uuid::from_u128(8),
        version: "1".into(),
    };
    before.footprints[0].library_ref = Some(source.clone());
    before.footprints[1].component.height_mm = 1.0;
    // U31's model reaches x = 4 mm: 0.5 mm from U41's 1 mm tall body.
    let models = BodyModels::from([(
        source,
        BodyModel::Extrusion {
            outline: Some(vec![[-1.0, -1.0], [4.0, -1.0], [4.0, 1.0], [-1.0, 1.0]]),
            offset_z_mm: 0.0,
            height_mm: 1.0,
        },
    )]);
    let rules = rules(&before, vec![Limit::MinBodyClearance(1_000_000)]);
    let bodies = |report: &DrcReport| {
        report
            .violations
            .iter()
            .filter(|v| v.kind == ViolationKind::BodyClearance)
            .count()
    };
    let every = FixScope::Every(None);
    let with = fix_findings(
        &before,
        &rules,
        &models,
        &every,
        &mut SequentialIds::default(),
    )
    .expect("a scope");
    assert_eq!(bodies(&with.report), 1, "the model's body is measured");
    assert_eq!(
        bodies(&with.report),
        bodies(&signex_drc::run_with_bodies(&before, &rules, &models)),
        "the same check as Run DRC's"
    );
    assert!(
        with.left
            .iter()
            .any(|l| l.violation.kind == ViolationKind::BodyClearance
                && l.reason == LeftReason::NoFix),
        "{:?}",
        with.left
    );
    let none = BodyModels::new();
    let without = fix_findings(
        &before,
        &rules,
        &none,
        &every,
        &mut SequentialIds::default(),
    )
    .expect("a scope");
    assert_eq!(bodies(&without.report), 0, "no model, no body for U31");
}
