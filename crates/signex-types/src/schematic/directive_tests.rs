use super::{
    ALL_ERC_RULES, BlanketPour, DirectiveKind, NetRules, Point, SchDirective, diff_pair_base,
    diff_pair_halves, parse_erc_exclusions,
};
use uuid::Uuid;

fn blanket(exclusions: &[&str]) -> SchDirective {
    SchDirective::Blanket {
        uuid: Uuid::from_u128(1),
        start: Point::new(10.0, 20.0),
        end: Point::new(0.0, 0.0),
        net_class: Some("HighSpeed".to_string()),
        erc_exclusions: exclusions.iter().map(|s| s.to_string()).collect(),
        track_width: None,
        pour: BlanketPour::Normal,
        include_edge_nets: false,
    }
}

fn net_parameters() -> SchDirective {
    SchDirective::NetParameters {
        uuid: Uuid::from_u128(3),
        position: Point::new(4.0, 6.0),
        net_class: Some(" POWER_IN ".to_string()),
        diff_pair_class: None,
        rules: NetRules::default(),
        show_on_flag: false,
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
        class: None,
    };
    assert!(d.region().is_none());
    assert!(!d.region_contains(Point::new(1.0, 1.0)));
    assert_eq!(d.kind(), DirectiveKind::DiffPair);
    assert_eq!(d.kind().as_str(), "diff_pair");
}

#[test]
fn net_parameters_is_a_point_directive_anchored_at_its_position() {
    let mut d = net_parameters();
    assert!(d.region().is_none());
    assert!(!d.region_contains(Point::new(4.0, 6.0)));
    assert_eq!(d.anchor(), Point::new(4.0, 6.0));
    assert_eq!(d.kind(), DirectiveKind::NetParameters);
    assert_eq!(d.kind().as_str(), "net_parameters");
    assert_eq!(d.net_class(), Some("POWER_IN"));
    assert!(!d.excludes_rule(ALL_ERC_RULES));
    d.map_points(|p| Point::new(p.x + 1.0, p.y));
    assert_eq!(d.anchor(), Point::new(5.0, 6.0));
    d.set_uuid(Uuid::from_u128(9));
    assert_eq!(d.uuid(), Uuid::from_u128(9));
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
fn pair_halves_match_every_suffix_style_only_when_asked() {
    let halves =
        |net, every| diff_pair_halves(net, every).map(|h| (h.name, h.positive, h.negative));
    let s = |a: &str, b: &str, c: &str| Some((a.to_string(), b.to_string(), c.to_string()));
    assert_eq!(halves("USB_D_N", false), s("USB_D", "USB_D_P", "USB_D_N"));
    assert_eq!(halves("USB_DP", true), s("USB_D", "USB_DP", "USB_DN"));
    assert_eq!(halves("USB_DN", true), s("USB_D", "USB_DP", "USB_DN"));
    assert_eq!(halves("CLK-", true), s("CLK", "CLK+", "CLK-"));
    assert_eq!(halves("USB_DP", false), None);
    assert_eq!(halves("CLK+", false), None);
    assert_eq!(halves("+", true), None);
    assert_eq!(halves("VBUS", true), None);
}

#[test]
fn only_the_fields_added_with_net_parameters_need_format_1_3() {
    assert!(!blanket(&["builtin::unused_pin"]).needs_format_1_3());
    let with = |edit: fn(&mut SchDirective)| {
        let mut b = blanket(&[]);
        edit(&mut b);
        b.needs_format_1_3()
    };
    assert!(with(|b| {
        if let SchDirective::Blanket { track_width, .. } = b {
            *track_width = Some(1.0)
        }
    }));
    assert!(with(|b| if let SchDirective::Blanket { pour, .. } = b {
        *pour = BlanketPour::KeepShort
    }));
    assert!(with(|b| {
        if let SchDirective::Blanket {
            include_edge_nets, ..
        } = b
        {
            *include_edge_nets = true
        }
    }));
    let pair = |class: Option<&str>| SchDirective::DiffPair {
        uuid: Uuid::from_u128(2),
        position: Point::new(0.0, 0.0),
        class: class.map(str::to_string),
    };
    assert!(!pair(None).needs_format_1_3());
    assert!(pair(Some("USB")).needs_format_1_3());
    assert_eq!(pair(Some(" USB ")).diff_pair_class(), Some("USB"));
    assert!(net_parameters().needs_format_1_3());
    let mask = SchDirective::CompileMask {
        uuid: Uuid::from_u128(4),
        start: Point::new(0.0, 0.0),
        end: Point::new(1.0, 1.0),
    };
    assert!(!mask.needs_format_1_3());
}

#[test]
fn net_rules_are_empty_until_one_is_set_and_refuse_a_non_finite_value() {
    let mut rules = NetRules::default();
    assert!(rules.is_empty() && rules.is_finite());
    rules.current_a = Some(2.0);
    assert!(!rules.is_empty() && rules.is_finite());
    rules.width_mm = Some(f64::NAN);
    assert!(!rules.is_finite());
}

#[test]
fn exclusions_are_split_trimmed_and_kept_once() {
    assert_eq!(
        parse_erc_exclusions(" builtin::a, ,builtin::b,builtin::a "),
        vec!["builtin::a".to_string(), "builtin::b".to_string()]
    );
    assert!(parse_erc_exclusions("").is_empty());
}
