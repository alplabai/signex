//! #2656: directives that give one net different classes are reported.

use signex_types::schematic::{Point, SchDirective, SelectedKind};
use uuid::Uuid;

use crate::context::ErcContext;
use crate::fixtures::{class_conflict_sheet, net_blanket, net_parameters};
use crate::{RuleKind, Severity, engine};

fn found(ctx: &ErcContext) -> Vec<crate::Diagnostic> {
    engine::run_all(ctx)
        .into_iter()
        .filter(|d| d.rule_kind == RuleKind::NetClassConflict)
        .collect()
}

#[test]
fn a_net_parameters_class_over_a_blanket_class_is_one_warning_at_the_blanket() {
    let hits = found(&ErcContext::from_snapshot(&class_conflict_sheet()));
    assert_eq!(hits.len(), 1, "{hits:?}");
    let d = &hits[0];
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.rule_id.as_str(), "builtin::net_class_conflict");
    assert_eq!(
        d.message,
        "Net SW takes class POWER from a Net Parameters directive; \
         class SIGNAL of this Net Blanket is overridden"
    );
    // At the overridden Blanket's centre, the winner as the peer.
    assert_eq!(d.location, Point::new(5.0, 0.0));
    let primary = d.primary.expect("primary");
    assert_eq!(
        (primary.uuid, primary.kind),
        (Uuid::from_u128(11), SelectedKind::Directive)
    );
    assert_eq!(d.peer.map(|p| p.uuid), Some(Uuid::from_u128(12)));
    assert_eq!(d.tag.as_deref(), Some("net:SW"));
}

#[test]
fn two_net_parameters_name_the_later_one() {
    let mut sheet = class_conflict_sheet();
    sheet.directives.retain(|d| d.region().is_none());
    sheet
        .directives
        .push(net_parameters(13, Point::new(8.0, 0.0), "SIGNAL"));
    let hits = found(&ErcContext::from_snapshot(&sheet));
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].location, Point::new(8.0, 0.0));
    assert_eq!(hits[0].primary.map(|p| p.uuid), Some(Uuid::from_u128(13)));
    assert_eq!(hits[0].peer.map(|p| p.uuid), Some(Uuid::from_u128(12)));
}

#[test]
fn directives_that_agree_are_not_reported() {
    let mut sheet = class_conflict_sheet();
    sheet.directives[1] = net_parameters(12, Point::new(5.0, 0.0), "SIGNAL");
    assert!(found(&ErcContext::from_snapshot(&sheet)).is_empty());
}

#[test]
fn a_single_class_directive_builds_no_conflicts() {
    let mut sheet = class_conflict_sheet();
    sheet.directives.truncate(1);
    let ctx = ErcContext::from_snapshot(&sheet);
    assert!(ctx.class_conflicts.is_empty());
    assert!(found(&ctx).is_empty());
}

/// The overridden Blanket can drop the finding like any other rule inside it.
#[test]
fn a_blanket_that_excludes_the_rule_silences_it() {
    let mut sheet = class_conflict_sheet();
    if let SchDirective::Blanket { erc_exclusions, .. } = &mut sheet.directives[0] {
        erc_exclusions.push("builtin::net_class_conflict".into());
    }
    assert!(found(&ErcContext::from_snapshot(&sheet)).is_empty());
}

/// A net two Blankets catch is reported once per net, kept apart by its tag.
#[test]
fn one_blanket_overridden_on_two_nets_gives_two_findings() {
    use signex_types::schematic::{Label, LabelType, Wire};
    let mut sheet = class_conflict_sheet();
    sheet.directives.truncate(1);
    sheet.wires.push(Wire {
        uuid: Uuid::from_u128(2),
        start: Point::new(0.0, 0.5),
        end: Point::new(10.0, 0.5),
        ..Default::default()
    });
    sheet
        .labels
        .push(Label::new("OTHER", Point::new(0.0, 0.5), LabelType::Net));
    crate::fixtures::class_conflict_pin(&mut sheet, "R3", Point::new(0.0, 0.5));
    crate::fixtures::class_conflict_pin(&mut sheet, "R4", Point::new(10.0, 0.5));
    sheet.directives.push(net_blanket(
        14,
        Point::new(-2.0, -2.0),
        Point::new(12.0, 2.0),
        "SLOW",
    ));
    let hits = found(&ErcContext::from_snapshot(&sheet));
    let tags: Vec<&str> = hits.iter().filter_map(|d| d.tag.as_deref()).collect();
    assert_eq!(tags.len(), 2, "{hits:?}");
    assert!(
        tags.contains(&"net:SW") && tags.contains(&"net:OTHER"),
        "{tags:?}"
    );
    assert!(
        hits.iter()
            .all(|d| d.primary.map(|p| p.uuid) == Some(Uuid::from_u128(14)))
    );
}

#[test]
fn the_rule_has_a_fixture_and_its_severity() {
    let ctx = crate::fixtures::context_for(RuleKind::NetClassConflict);
    assert!(!found(&ctx).is_empty());
    assert_eq!(
        RuleKind::NetClassConflict.default_severity(),
        Severity::Warning
    );
}
