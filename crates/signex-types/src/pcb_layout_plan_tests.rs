use super::*;

fn plan_with(settings: &[(LayoutSetting, &str)]) -> LayoutPlan {
    let mut plan = LayoutPlan::default();
    for (setting, text) in settings {
        plan.set(*setting, text).expect("a setting the plan takes");
    }
    plan
}

#[test]
fn the_stages_run_in_order_and_each_setting_belongs_to_one() {
    let tokens: Vec<&str> = LayoutStage::ALL.iter().map(|s| s.token()).collect();
    assert_eq!(tokens, ["placement", "routing", "pours", "checks"]);
    let of = |stage: LayoutStage| -> Vec<&str> { stage.settings().map(|s| s.operand()).collect() };
    assert_eq!(of(LayoutStage::Placement), ["weights", "rooms", "keep"]);
    assert_eq!(of(LayoutStage::Routing), ["nets", "skip"]);
    assert_eq!(of(LayoutStage::Pours), ["pours"]);
    assert_eq!(of(LayoutStage::Checks), ["rules", "requirements"]);
    assert_eq!(
        LayoutStage::from_token(" routing "),
        Some(LayoutStage::Routing)
    );
    assert_eq!(LayoutStage::from_token("route"), None);
}

#[test]
fn each_setting_reads_its_comma_list_and_empty_text_clears_it() {
    let pour = Uuid::from_u128(7);
    let mut plan = plan_with(&[
        (LayoutSetting::Weights, "edges:5, connection:3"),
        (LayoutSetting::Rooms, "Power, Analog"),
        (LayoutSetting::Keep, "J1"),
        (LayoutSetting::Nets, "class:Power,GND, SDA"),
        (LayoutSetting::Skip, "NC_1"),
        (LayoutSetting::Pours, &pour.to_string()),
        (LayoutSetting::Rules, "builtin::clearance,user::hv_gap"),
        (LayoutSetting::Requirements, "size"),
    ]);
    assert_eq!(
        plan.placement.weights,
        BTreeMap::from([
            (PlacementPriority::Connection, 3.0),
            (PlacementPriority::Edges, 5.0)
        ])
    );
    assert_eq!(plan.placement.rooms, ["Power", "Analog"]);
    assert_eq!(
        plan.routing.nets,
        [
            RouteTarget::Class("Power".into()),
            RouteTarget::Net("GND".into()),
            RouteTarget::Net("SDA".into())
        ]
    );
    assert_eq!(plan.pours.pours, [pour]);
    assert_eq!(plan.check(), Ok(()));

    // The same text again changes nothing; empty text clears.
    assert_eq!(plan.set(LayoutSetting::Rooms, "Power,Analog"), Ok(false));
    assert_eq!(plan.set(LayoutSetting::Rooms, ""), Ok(true));
    assert!(plan.placement.rooms.is_empty());
    assert!(plan.clear(LayoutStage::Routing));
    assert!(plan.stage_is_empty(LayoutStage::Routing));
    assert!(!plan.clear(LayoutStage::Routing), "nothing left to clear");
}

#[test]
fn a_setting_that_does_not_parse_is_refused_and_changes_nothing() {
    let mut plan = plan_with(&[(LayoutSetting::Weights, "thermal:1")]);
    let kept = plan.clone();
    for (setting, text) in [
        (LayoutSetting::Weights, "heat:1"),
        (LayoutSetting::Weights, "thermal"),
        (LayoutSetting::Weights, "thermal:-1"),
        (LayoutSetting::Weights, "thermal:NaN"),
        (LayoutSetting::Weights, "thermal:1,thermal:2"),
        (LayoutSetting::Rooms, "Power,Power"),
        (LayoutSetting::Nets, "class:"),
        (LayoutSetting::Nets, "GND,GND"),
        (LayoutSetting::Pours, "not-a-uuid"),
        (LayoutSetting::Rules, "clearance"),
        (LayoutSetting::Rules, "builtin::"),
    ] {
        assert!(plan.set(setting, text).is_err(), "{text}");
        assert_eq!(plan, kept, "{text}");
    }
}

#[test]
fn a_plan_round_trips_through_its_setting_texts() {
    let plan = plan_with(&[
        (LayoutSetting::Weights, "separation:0.25,thermal:2"),
        (LayoutSetting::Nets, "class:HS,CLK"),
        (LayoutSetting::Requirements, "erc,size"),
    ]);
    let mut again = LayoutPlan::default();
    for setting in LayoutSetting::ALL {
        again.set(setting, &plan.setting_text(setting)).unwrap();
    }
    assert_eq!(again, plan);
    assert!(LayoutPlan::default().is_empty());
    assert!(!plan.is_empty());
}

/// A plan that `set` could not have made (a name with a comma, a weight
/// below 0) fails `check`, which the file reader runs.
#[test]
fn check_refuses_what_set_would_refuse() {
    let mut plan = LayoutPlan::default();
    plan.placement.rooms = vec!["A,B".into()];
    assert!(plan.check().is_err());
    let mut plan = LayoutPlan::default();
    plan.placement
        .weights
        .insert(PlacementPriority::Edges, -2.0);
    assert!(plan.check().is_err());
    let mut plan = LayoutPlan::default();
    plan.checks.rules = vec!["builtin::short".into(), "builtin::short".into()];
    assert!(plan.check().is_err());
}

/// The JSON form the board options carry: a net class as `class:<name>`,
/// the priorities by token, empty stages left out.
#[test]
fn the_plan_serialises_with_tokens_and_without_empty_stages() {
    let plan = plan_with(&[
        (LayoutSetting::Weights, "edges:5"),
        (LayoutSetting::Nets, "class:Power,GND"),
    ]);
    let json = serde_json::to_value(&plan).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "placement": {"weights": {"edges": 5.0}},
            "routing": {"nets": ["class:Power", "GND"]}
        })
    );
    let back: LayoutPlan = serde_json::from_value(json).unwrap();
    assert_eq!(back, plan);
}
