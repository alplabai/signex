use super::*;
use signex_types::pcb::{Footprint, NetDef, Pad, PadNet, PadShape, PadType, Point, Segment};
use signex_types::pcb_layout_plan::LayoutSetting;
use signex_types::pcb_options::DrcWaiver;
use signex_types::zone_pour::POUR_FILL;
use uuid::Uuid;

use crate::{DrcRules, Violation, waiver_key};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn pad(n: u128, net: u32) -> Pad {
    Pad {
        uuid: id(n),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: net,
            name: format!("N{net}"),
        }),
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        net_locked: false,
        layer_sizes: None,
        thermal: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// `reference` with one pad on net 1 at `(x, y)`.
fn part(n: u128, reference: &str, x: f64, y: f64) -> Footprint {
    Footprint {
        uuid: id(1000 + n),
        reference: reference.into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(x, y),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads: vec![pad(n, 1)],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn track(a: (f64, f64), b: (f64, f64)) -> Segment {
    Segment {
        uuid: id(2000),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 1,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        locked: false,
    }
}

fn pour(n: u128, shelved: bool, unfilled: bool) -> Zone {
    Zone {
        uuid: id(n),
        net: 1,
        net_name: "N1".into(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 3.0),
        ],
        holes: Vec::new(),
        priority: 1,
        fill_type: POUR_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.3,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved,
        unfilled,
        corners: Default::default(),
        corner_size: 0.0,
        fill_mode: Default::default(),
        hatch_width: 0.0,
        hatch_gap: 0.0,
        unknown: Default::default(),
    }
}

/// A 50 x 40 mm board with R1 and R2 on net `N1`.
fn board() -> PcbBoard {
    PcbBoard {
        stackup: None,
        rooms: Vec::new(),
        free: Default::default(),
        sketch: None,
        options: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: vec![
            Point::new(0.0, 0.0),
            Point::new(50.0, 0.0),
            Point::new(50.0, 40.0),
            Point::new(0.0, 40.0),
        ],
        layers: Vec::new(),
        setup: None,
        nets: vec![
            NetDef {
                number: 0,
                name: String::new(),
            },
            NetDef {
                number: 1,
                name: "N1".into(),
            },
        ],
        footprints: vec![part(1, "R1", 10.0, 10.0), part(2, "R2", 20.0, 10.0)],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn status(plan: &LayoutPlan, board: &PcbBoard, drc: Option<&DrcReport>) -> Vec<StageStatus> {
    let requirements = vec!["size".to_string()];
    let input = PlanInput {
        board,
        net_classes: &[],
        requirements: &requirements,
        drc,
    };
    plan_status(plan, &input)
}

fn of(all: &[StageStatus], stage: LayoutStage) -> &StageStatus {
    all.iter().find(|s| s.stage == stage).expect("every stage")
}

fn plan(settings: &[(LayoutSetting, &str)]) -> LayoutPlan {
    let mut plan = LayoutPlan::default();
    for (setting, text) in settings {
        plan.set(*setting, text).unwrap();
    }
    plan
}

fn report(violations: Vec<Violation>) -> DrcReport {
    DrcReport {
        rules: DrcRules::new(1),
        violations,
        skipped: Vec::new(),
        skipped_keepouts: Vec::new(),
    }
}

fn clearance() -> Violation {
    Violation {
        kind: ViolationKind::Clearance,
        items: Vec::new(),
        nets: (1, 2),
        layers: 1,
        at: PointNm::new(0, 0),
        measured_nm: 100_000,
        required_nm: 200_000,
        keepout: None,
        rule: None,
    }
}

#[test]
fn the_stages_answer_in_order() {
    let all = status(&LayoutPlan::default(), &board(), None);
    let stages: Vec<LayoutStage> = all.iter().map(|s| s.stage).collect();
    assert_eq!(stages, LayoutStage::ALL);
}

#[test]
fn placement_is_done_when_every_part_in_scope_is_on_the_board() {
    let mut board = board();
    let all = status(&LayoutPlan::default(), &board, None);
    let placement = of(&all, LayoutStage::Placement);
    assert_eq!(placement.state, StageState::Done, "{placement:?}");
    assert_eq!(placement.counts["placed"], 2);

    board.footprints[1].position = Point::new(80.0, 10.0);
    let all = status(&LayoutPlan::default(), &board, None);
    let placement = of(&all, LayoutStage::Placement);
    assert_eq!(placement.state, StageState::Failing);
    assert_eq!(placement.reasons, ["R2 is not inside the board outline"]);

    // Kept where it is, R2 is out of scope.
    let keep = plan(&[(LayoutSetting::Keep, "R2")]);
    let all = status(&keep, &board, None);
    assert_eq!(of(&all, LayoutStage::Placement).state, StageState::Done);

    // Nothing on the board yet: not started.
    board.footprints[0].position = Point::new(90.0, 10.0);
    let all = status(&LayoutPlan::default(), &board, None);
    assert_eq!(
        of(&all, LayoutStage::Placement).state,
        StageState::NotStarted
    );
}

#[test]
fn routing_counts_the_nets_in_scope_and_skips_the_skipped() {
    let mut board = board();
    let all = status(&LayoutPlan::default(), &board, None);
    let routing = of(&all, LayoutStage::Routing);
    assert_eq!(routing.state, StageState::NotStarted, "{routing:?}");
    assert_eq!(routing.counts["nets"], 1);

    board.segments = vec![track((10.0, 10.0), (20.0, 10.0))];
    let all = status(&LayoutPlan::default(), &board, None);
    assert_eq!(of(&all, LayoutStage::Routing).state, StageState::Done);

    board.segments.clear();
    let skip = plan(&[(LayoutSetting::Skip, "N1")]);
    let all = status(&skip, &board, None);
    let routing = of(&all, LayoutStage::Routing);
    assert_eq!(routing.state, StageState::NotStarted);
    assert_eq!(routing.counts["nets"], 0);
    assert_eq!(routing.reasons, ["no net is in scope"]);
}

#[test]
fn what_the_plan_names_and_the_board_lacks_fails_its_stage() {
    let wrong = plan(&[
        (LayoutSetting::Nets, "VCC,class:Power"),
        (LayoutSetting::Rooms, "Analog"),
        (LayoutSetting::Requirements, "cost"),
    ]);
    let board = board();
    let all = status(&wrong, &board, None);
    let routing = of(&all, LayoutStage::Routing);
    assert_eq!(routing.state, StageState::Failing);
    assert_eq!(
        routing.reasons[..2],
        [
            "net `VCC` is not on the board".to_string(),
            "net class `Power` is not in the project".to_string()
        ]
    );
    assert_eq!(of(&all, LayoutStage::Placement).state, StageState::Failing);
    assert_eq!(of(&all, LayoutStage::Checks).state, StageState::Failing);
    let input = PlanInput {
        board: &board,
        net_classes: &[],
        requirements: &[],
        drc: None,
    };
    assert_eq!(missing_references(&wrong, &input).len(), 4);
}

#[test]
fn pours_are_done_when_every_pour_in_scope_fills() {
    let mut board = board();
    board.zones = vec![pour(7, false, false), pour(8, true, false)];
    let all = status(&LayoutPlan::default(), &board, None);
    let pours = of(&all, LayoutStage::Pours);
    assert_eq!(
        pours.state,
        StageState::Done,
        "the shelved pour is out of scope"
    );
    assert_eq!(pours.counts["pours"], 1);

    let named = plan(&[(LayoutSetting::Pours, &format!("{},{}", id(7), id(8)))]);
    let all = status(&named, &board, None);
    let pours = of(&all, LayoutStage::Pours);
    assert_eq!(pours.state, StageState::Failing);
    assert!(pours.reasons[0].ends_with("is shelved"), "{pours:?}");

    board.zones = vec![pour(7, false, true)];
    let all = status(&LayoutPlan::default(), &board, None);
    assert_eq!(of(&all, LayoutStage::Pours).state, StageState::NotStarted);
}

#[test]
fn checks_read_the_current_drc_run_and_its_waivers() {
    let mut board = board();
    let all = status(&LayoutPlan::default(), &board, None);
    assert_eq!(of(&all, LayoutStage::Checks).state, StageState::NotStarted);

    let clean = report(Vec::new());
    let all = status(&LayoutPlan::default(), &board, Some(&clean));
    assert_eq!(of(&all, LayoutStage::Checks).state, StageState::Done);

    let found = report(vec![clearance()]);
    let all = status(&LayoutPlan::default(), &board, Some(&found));
    let checks = of(&all, LayoutStage::Checks);
    assert_eq!(checks.state, StageState::Failing);
    assert_eq!(checks.reasons, ["builtin::clearance: 1 open findings"]);

    // Out of scope when the plan names another rule.
    let short = plan(&[(LayoutSetting::Rules, "builtin::short")]);
    let all = status(&short, &board, Some(&found));
    assert_eq!(of(&all, LayoutStage::Checks).state, StageState::Done);

    // A waived finding does not hold the stage back.
    let v = clearance();
    board.options.drc_waivers = vec![DrcWaiver {
        key: waiver_key(&v),
        rule: "builtin::clearance".into(),
        note: String::new(),
        fingerprint: crate::fingerprint(&v, &board),
        author: String::new(),
    }];
    let all = status(&LayoutPlan::default(), &board, Some(&found));
    let checks = of(&all, LayoutStage::Checks);
    assert_eq!(checks.state, StageState::Done, "{checks:?}");
    assert_eq!(checks.counts["waived"], 1);
}

/// A requirement the plan names is never taken as passed: the status does
/// not run `check_requirements`.
#[test]
fn a_named_requirement_keeps_the_checks_from_reading_done() {
    let named = plan(&[(LayoutSetting::Requirements, "size")]);
    let clean = report(Vec::new());
    let all = status(&named, &board(), Some(&clean));
    let checks = of(&all, LayoutStage::Checks);
    assert_eq!(checks.state, StageState::NotStarted);
    assert!(
        checks.reasons[0].contains("check_requirements"),
        "{checks:?}"
    );
}

#[test]
fn the_known_rules_are_the_built_in_kinds_and_any_written_rule() {
    assert!(is_known_rule("builtin::clearance"));
    assert!(is_known_rule("user::hv_gap"));
    assert!(!is_known_rule("builtin::nothing"));
    assert!(!is_known_rule("user::"));
}

#[test]
fn a_long_list_of_reasons_is_cut_and_counted() {
    let lines: Vec<String> = (0..LISTED + 3).map(|n| n.to_string()).collect();
    let cut = capped(lines);
    assert_eq!(cut.len(), LISTED + 1);
    assert_eq!(cut[LISTED], "and 3 more");
}

/// #2986: a finding whose rule is switched off is not shown by Run DRC and
/// does not hold the Checks stage back.
#[test]
fn checks_do_not_count_a_finding_whose_rule_is_off() {
    let board = board();
    let unconnected = Violation {
        kind: ViolationKind::Unconnected,
        ..clearance()
    };
    let mut off = report(vec![unconnected]);
    let mut project = signex_types::project::DrcRuleSet::default();
    project
        .severity
        .insert("builtin::unconnected".into(), "off".into());
    off.rules.severities = crate::resolve_severities(None, Some(&project));
    let all = status(&LayoutPlan::default(), &board, Some(&off));
    let checks = of(&all, LayoutStage::Checks);
    assert_eq!(checks.state, StageState::Done, "{checks:?}");
    assert_eq!(checks.counts.get("findings").copied().unwrap_or(0), 0);
    assert!(checks.reasons.is_empty(), "{:?}", checks.reasons);
}
