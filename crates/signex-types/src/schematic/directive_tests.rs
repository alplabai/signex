use super::{
    ALL_ERC_RULES, DirectiveKind, Point, SchDirective, diff_pair_base, parse_erc_exclusions,
};
use uuid::Uuid;

fn blanket(exclusions: &[&str]) -> SchDirective {
    SchDirective::Blanket {
        uuid: Uuid::from_u128(1),
        start: Point::new(10.0, 20.0),
        end: Point::new(0.0, 0.0),
        net_class: Some("HighSpeed".to_string()),
        erc_exclusions: exclusions.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn a_region_contains_its_edge_and_interior_whatever_the_corner_order() {
    let b = blanket(&[]);
    assert!(b.region_contains(Point::new(5.0, 5.0)));
    assert!(b.region_contains(Point::new(0.0, 20.0)));
    assert!(!b.region_contains(Point::new(10.1, 5.0)));
    assert_eq!(b.anchor(), Point::new(5.0, 10.0));
}

#[test]
fn a_point_directive_has_no_region() {
    let d = SchDirective::DiffPair {
        uuid: Uuid::from_u128(2),
        position: Point::new(1.0, 1.0),
    };
    assert!(d.region().is_none());
    assert!(!d.region_contains(Point::new(1.0, 1.0)));
    assert_eq!(d.kind(), DirectiveKind::DiffPair);
    assert_eq!(d.kind().as_str(), "diff_pair");
}

#[test]
fn a_blanket_excludes_the_rules_it_names_or_every_rule_with_the_wildcard() {
    let b = blanket(&["builtin::unused_pin"]);
    assert!(b.excludes_rule("builtin::unused_pin"));
    assert!(!b.excludes_rule("builtin::orphan_label"));
    assert!(blanket(&[ALL_ERC_RULES]).excludes_rule("builtin::orphan_label"));
    assert_eq!(b.net_class(), Some("HighSpeed"));
}

#[test]
fn map_points_moves_every_stored_point() {
    let mut b = blanket(&[]);
    b.map_points(|p| Point::new(p.x + 1.0, p.y - 1.0));
    let SchDirective::Blanket { start, end, .. } = b else {
        unreachable!()
    };
    assert_eq!(
        (start, end),
        (Point::new(11.0, 19.0), Point::new(1.0, -1.0))
    );
}

#[test]
fn the_pair_base_strips_one_suffix_and_refuses_a_bare_one() {
    assert_eq!(diff_pair_base("USB_D_P"), Some("USB_D"));
    assert_eq!(diff_pair_base("USB_D_N"), Some("USB_D"));
    assert_eq!(diff_pair_base("USB_D"), None);
    assert_eq!(diff_pair_base("_P"), None);
}

#[test]
fn exclusions_are_split_trimmed_and_kept_once() {
    assert_eq!(
        parse_erc_exclusions(" builtin::a, ,builtin::b,builtin::a "),
        vec!["builtin::a".to_string(), "builtin::b".to_string()]
    );
    assert!(parse_erc_exclusions("").is_empty());
}
