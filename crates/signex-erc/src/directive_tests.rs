//! #2420: what a Blanket and a Compile Mask do to ERC findings.

use signex_types::schematic::{Label, LabelType, Point, SchDirective, SchematicSheet};
use uuid::Uuid;

use crate::{ErcContext, RuleKind, engine};

const ORPHAN: &str = "builtin::orphan_label";

/// A sheet with one floating net label at (10, 10): an orphan label.
fn orphan_sheet() -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".into(),
        labels: vec![Label::new("LOOSE", Point::new(10.0, 10.0), LabelType::Net)],
        ..Default::default()
    }
}

fn orphans(sheet: &SchematicSheet) -> usize {
    engine::run_all(&ErcContext::from_snapshot(sheet))
        .iter()
        .filter(|d| d.rule_kind == RuleKind::OrphanLabel)
        .count()
}

fn blanket(exclusions: &[&str]) -> SchDirective {
    SchDirective::Blanket {
        uuid: Uuid::from_u128(1),
        start: Point::new(0.0, 0.0),
        end: Point::new(20.0, 20.0),
        net_class: None,
        erc_exclusions: exclusions.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn the_fixture_reports_its_orphan_label() {
    assert_eq!(orphans(&orphan_sheet()), 1);
}

#[test]
fn a_blanket_drops_the_rules_it_excludes_inside_it() {
    let mut sheet = orphan_sheet();
    sheet.directives.push(blanket(&[ORPHAN]));
    assert_eq!(orphans(&sheet), 0);

    let mut every = orphan_sheet();
    every.directives.push(blanket(&["*"]));
    assert_eq!(orphans(&every), 0);
}

#[test]
fn a_blanket_keeps_other_rules_and_findings_outside_it() {
    let mut other_rule = orphan_sheet();
    other_rule
        .directives
        .push(blanket(&["builtin::unused_pin"]));
    assert_eq!(orphans(&other_rule), 1);

    let mut outside = orphan_sheet();
    outside.directives.push(SchDirective::Blanket {
        uuid: Uuid::from_u128(2),
        start: Point::new(50.0, 50.0),
        end: Point::new(60.0, 60.0),
        net_class: None,
        erc_exclusions: vec![ORPHAN.to_string()],
    });
    assert_eq!(orphans(&outside), 1);
}

#[test]
fn a_compile_mask_takes_what_it_covers_out_of_erc() {
    let mut sheet = orphan_sheet();
    sheet.directives.push(SchDirective::CompileMask {
        uuid: Uuid::from_u128(3),
        start: Point::new(0.0, 0.0),
        end: Point::new(20.0, 20.0),
    });
    let ctx = ErcContext::from_snapshot(&sheet);
    assert!(ctx.labels.is_empty(), "the masked label is not projected");
    assert_eq!(orphans(&sheet), 0);
}
