//! #2311: a fab tier picked in project setup seeds exactly its `fab_*`
//! rules; picking again replaces only those.

use std::collections::BTreeMap;

use signex_types::project::{DrcRuleSet, FabTemplate};

use super::presets::with_tier;
use super::table;

fn rules_of(drc: &DrcRuleSet) -> Vec<String> {
    let text = drc.dsl.clone().unwrap_or_default();
    let rows = table(&text);
    assert!(!rows.is_unparsed(), "{text}");
    rows.rows.iter().map(|r| r.text(&text).to_string()).collect()
}

#[test]
fn each_tier_seeds_its_exact_fab_rule_set() {
    let expected = |c: [&str; 10]| {
        vec![
            format!("rule fab_clearance on clearance require clearance >= {} then error", c[0]),
            format!("rule fab_track on track require width >= {} then error", c[1]),
            format!(
                "rule fab_via on via require diameter >= {} and drill >= {} then error",
                c[2], c[3]
            ),
            format!(
                "rule fab_hole on hole require diameter >= {} and annular >= {} and spacing >= {} then error",
                c[4], c[5], c[6]
            ),
            format!("rule fab_edge on edge require clearance >= {} then error", c[7]),
            format!("rule fab_mask on mask require sliver >= {} then warning", c[8]),
            format!("rule fab_silk on silk require pad_clearance >= {} then warning", c[9]),
        ]
    };
    let cases = [
        (
            FabTemplate::Standard,
            ["0.15mm", "0.15mm", "0.5mm", "0.25mm", "0.25mm", "0.125mm", "0.5mm", "0.3mm", "0.1mm", "0.15mm"],
        ),
        (
            FabTemplate::Advanced,
            ["0.1mm", "0.1mm", "0.4mm", "0.2mm", "0.2mm", "0.1mm", "0.4mm", "0.25mm", "0.075mm", "0.1mm"],
        ),
        (
            FabTemplate::Hdi,
            ["0.075mm", "0.075mm", "0.3mm", "0.15mm", "0.1mm", "0.075mm", "0.3mm", "0.2mm", "0.06mm", "0.075mm"],
        ),
    ];
    for (tier, values) in cases {
        let seeded = with_tier(&DrcRuleSet::default(), tier).unwrap();
        let rules: Vec<String> = rules_of(&seeded)
            .iter()
            .map(|r| r.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect();
        assert_eq!(rules, expected(values), "{tier:?}");
    }
}

#[test]
fn re_choosing_a_tier_replaces_only_its_fab_rules() {
    let mine = "rule mine on track require width >= 0.3mm then error\n\
                rule fab_custom on clearance require clearance >= 0.5mm then error\n";
    let mut severity = BTreeMap::new();
    severity.insert("user::mine".to_string(), "warning".to_string());
    severity.insert("user::fab_track".to_string(), "off".to_string());
    let start = DrcRuleSet {
        dsl: Some(mine.to_string()),
        severity,
        ..DrcRuleSet::default()
    };
    let standard = with_tier(&start, FabTemplate::Standard).unwrap();
    let hdi = with_tier(&standard, FabTemplate::Hdi).unwrap();
    let text = hdi.dsl.clone().unwrap();
    assert!(text.starts_with(mine), "the other rules stay as typed: {text}");
    assert_eq!(rules_of(&hdi).len(), 9, "{text}");
    assert!(text.contains("rule fab_track on track require width >= 0.075mm then error"));
    assert!(!text.contains("width >= 0.15mm"), "no Standard rule is left: {text}");
    // The tier's own `then` governs its rules; the user's entry stays.
    assert_eq!(hdi.severity.get("user::mine").map(String::as_str), Some("warning"));
    assert!(!hdi.severity.contains_key("user::fab_track"));
    // Picking the same tier again changes nothing.
    assert_eq!(with_tier(&hdi, FabTemplate::Hdi).unwrap(), hdi);
}

#[test]
fn a_rule_text_with_an_error_is_not_merged_into() {
    let broken = DrcRuleSet {
        dsl: Some("rule oops on".to_string()),
        ..DrcRuleSet::default()
    };
    assert!(with_tier(&broken, FabTemplate::Standard).is_err());
}

#[test]
fn a_tier_is_named_by_one_token_everywhere() {
    for tier in FabTemplate::ALL {
        assert_eq!(FabTemplate::from_token(tier.token()), Some(tier));
        assert_eq!(super::presets::tier_id(tier), tier.token());
    }
}
