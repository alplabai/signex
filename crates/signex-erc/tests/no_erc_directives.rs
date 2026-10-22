//! A No-ERC directive suppresses every diagnostic located at its point, for
//! built-in and DSL rules alike (issue #349). Suppression is positional only.

use signex_erc::engine::{self, EvalFn};
use signex_erc::{Diagnostic, ErcContext, LocationSpace, RuleId, RuleKind};
use signex_types::format::SnxSchematic;
use signex_types::schematic::{NoConnect, Point, SchematicSheet, Wire};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn sheet(wires: Vec<Wire>, directives: Vec<NoConnect>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::nil(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols: Vec::new(),
        wires,
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: directives,
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
        unknown: Default::default(),
    }
}

fn wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: Uuid::new_v4(),
        start: a,
        end: b,
        stroke_width: 0.0,
    }
}

fn directive(p: Point) -> NoConnect {
    NoConnect {
        uuid: Uuid::new_v4(),
        position: p,
    }
}

/// One wire (0,0)-(10,0): two dangling endpoints.
fn one_wire(directives: Vec<NoConnect>) -> SchematicSheet {
    sheet(vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))], directives)
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
    assert_eq!(dangling_at(&diags, pt(10.0, 0.0)), 1);
}

#[test]
fn directive_one_millimetre_away_leaves_the_diagnostic() {
    let diags = run(&one_wire(vec![directive(pt(1.0, 0.0))]));
    assert_eq!(dangling_at(&diags, pt(0.0, 0.0)), 1);
    assert_eq!(dangling_at(&diags, pt(10.0, 0.0)), 1);
}

#[test]
fn sheet_without_directives_is_unaffected() {
    let diags = run(&one_wire(Vec::new()));
    assert_eq!(dangling_at(&diags, pt(0.0, 0.0)), 1);
    assert_eq!(dangling_at(&diags, pt(10.0, 0.0)), 1);
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

    let dir = std::env::temp_dir().join(format!("signex-349-{}", Uuid::new_v4()));
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
    assert_eq!(dangling_at(&diags, pt(10.0, 0.0)), 1);
}
