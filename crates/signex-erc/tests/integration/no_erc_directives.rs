//! A No-ERC directive suppresses the diagnostics located at its point, for
//! built-in and DSL rules alike (issue #349): every rule when it names none,
//! else only the rules it names (#1699).

use signex_erc::engine::{self, EvalFn};
use signex_erc::{Diagnostic, ErcContext, LocationSpace, RuleId, RuleKind};
use signex_types::format::SnxSchematic;
use signex_types::schematic::{NoErcDirective, Point, SchematicSheet, Wire};
use std::sync::Arc;
use uuid::Uuid;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn sheet(wires: Vec<Wire>, directives: Vec<NoErcDirective>) -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".to_string(),
        wires,
        no_erc_directives: directives,
        ..Default::default()
    }
}

fn wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: a,
        end: b,
        ..Default::default()
    }
}

fn directive(p: Point) -> NoErcDirective {
    NoErcDirective::new(Uuid::now_v7(), p)
}

/// A directive at `p` that silences only `rules`.
fn directive_for(p: Point, rules: &[RuleKind]) -> NoErcDirective {
    let mut d = directive(p);
    d.rules = rules.iter().map(|k| RuleId::builtin(*k).0).collect();
    d
}

/// One wire (0,0)-(12.7,0), on the connection grid: two dangling endpoints.
fn one_wire(directives: Vec<NoErcDirective>) -> SchematicSheet {
    sheet(vec![wire(pt(0.0, 0.0), pt(12.7, 0.0))], directives)
}

fn dangling_at(diags: &[Diagnostic], p: Point) -> usize {
    diags
        .iter()
        .filter(|d| {
            d.rule_id == RuleId::builtin(RuleKind::DanglingWire)
                && (d.location.x - p.x).abs() < 1e-6
                && (d.location.y - p.y).abs() < 1e-6
        })
        .count()
}

fn run(sheet: &SchematicSheet) -> Vec<Diagnostic> {
    engine::run_all(&ErcContext::from_snapshot(sheet))
}

#[test]
fn directive_at_dangling_endpoint_removes_only_that_diagnostic() {
    let diags = run(&one_wire(vec![directive(pt(0.0, 0.0))]));
    assert_eq!(dangling_at(&diags, pt(0.0, 0.0)), 0);
    assert_eq!(dangling_at(&diags, pt(12.7, 0.0)), 1);
}

#[test]
fn directive_one_millimetre_away_leaves_the_diagnostic() {
    let diags = run(&one_wire(vec![directive(pt(1.0, 0.0))]));
    assert_eq!(dangling_at(&diags, pt(0.0, 0.0)), 1);
    assert_eq!(dangling_at(&diags, pt(12.7, 0.0)), 1);
}

#[test]
fn sheet_without_directives_is_unaffected() {
    let diags = run(&one_wire(Vec::new()));
    assert_eq!(dangling_at(&diags, pt(0.0, 0.0)), 1);
    assert_eq!(dangling_at(&diags, pt(12.7, 0.0)), 1);
    assert_eq!(diags.len(), 2);
}

#[test]
fn directive_suppresses_dsl_diagnostic_at_same_point_only() {
    let rule: EvalFn = Arc::new(|_ctx: &ErcContext| {
        vec![
            Diagnostic::new(RuleKind::UnusedPin, "dsl on directive", pt(5.0, 5.0)),
            Diagnostic::new(RuleKind::UnusedPin, "dsl elsewhere", pt(6.0, 6.0)),
        ]
    });
    let ctx = ErcContext::from_snapshot(&sheet(Vec::new(), vec![directive(pt(5.0, 5.0))]));
    let diags = engine::run_all_with_dsl(&ctx, &[rule]);
    let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(messages, vec!["dsl elsewhere"]);
}

#[test]
fn origin_directive_does_not_suppress_unplaced_diagnostics() {
    let rule: EvalFn = Arc::new(|_ctx: &ErcContext| {
        vec![
            Diagnostic::new(RuleKind::UnusedPin, "net-scoped", pt(0.0, 0.0))
                .in_space(LocationSpace::Unplaced),
        ]
    });
    let ctx = ErcContext::from_snapshot(&sheet(Vec::new(), vec![directive(pt(0.0, 0.0))]));
    let diags = engine::run_all_with_dsl(&ctx, &[rule]);
    assert_eq!(diags.len(), 1);
}

#[test]
fn directive_does_not_suppress_child_sheet_coordinate_diagnostics() {
    let rule: EvalFn = Arc::new(|_ctx: &ErcContext| {
        vec![
            Diagnostic::new(RuleKind::BadHierSheetPin, "child label", pt(5.0, 5.0))
                .in_space(LocationSpace::ChildSheet),
        ]
    });
    let ctx = ErcContext::from_snapshot(&sheet(Vec::new(), vec![directive(pt(5.0, 5.0))]));
    let diags = engine::run_all_with_dsl(&ctx, &[rule]);
    assert_eq!(diags.len(), 1);
}

#[test]
fn snxsch_with_directive_roundtrips_a_no_edit_save() {
    let original = one_wire(vec![directive(pt(0.0, 0.0))]);
    let text = SnxSchematic::new(original).write_string().expect("write");

    let dir = std::env::temp_dir().join(format!("signex-349-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("fixture.snxsch");
    std::fs::write(&path, &text).expect("write fixture");

    let loaded =
        SnxSchematic::parse(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    let resaved = SnxSchematic::new(loaded.sheet.clone())
        .write_string()
        .expect("rewrite");
    std::fs::remove_dir_all(&dir).expect("cleanup");

    assert_eq!(loaded.sheet.no_erc_directives.len(), 1);
    assert_eq!(loaded.sheet.no_erc_directives[0].position, pt(0.0, 0.0));
    assert_eq!(resaved, text, "no-edit save must be byte-identical");
    let diags = run(&loaded.sheet);
    assert_eq!(dangling_at(&diags, pt(0.0, 0.0)), 0);
    assert_eq!(dangling_at(&diags, pt(12.7, 0.0)), 1);
}

#[test]
fn a_directive_for_the_rule_suppresses_it() {
    let marker = directive_for(pt(0.0, 0.0), &[RuleKind::DanglingWire]);
    let diags = run(&one_wire(vec![marker]));
    assert_eq!(dangling_at(&diags, pt(0.0, 0.0)), 0);
    assert_eq!(dangling_at(&diags, pt(12.7, 0.0)), 1);
}

#[test]
fn a_directive_for_another_rule_leaves_the_diagnostic() {
    let marker = directive_for(pt(0.0, 0.0), &[RuleKind::UnusedPin]);
    let diags = run(&one_wire(vec![marker]));
    assert_eq!(dangling_at(&diags, pt(0.0, 0.0)), 1);
    assert_eq!(dangling_at(&diags, pt(12.7, 0.0)), 1);
}

#[test]
fn a_listed_rule_is_suppressed_and_another_rule_at_the_point_still_reported() {
    let rule: EvalFn = Arc::new(|_ctx: &ErcContext| {
        vec![
            Diagnostic::new(RuleKind::UnusedPin, "listed", pt(5.0, 5.0)),
            Diagnostic::new(RuleKind::OrphanLabel, "not listed", pt(5.0, 5.0)),
        ]
    });
    let marker = directive_for(pt(5.0, 5.0), &[RuleKind::UnusedPin]);
    let ctx = ErcContext::from_snapshot(&sheet(Vec::new(), vec![marker]));
    let diags = engine::run_all_with_dsl(&ctx, &[rule]);
    let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(messages, vec!["not listed"]);
}
