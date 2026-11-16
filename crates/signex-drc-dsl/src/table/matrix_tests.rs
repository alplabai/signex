//! #2398 steps 5 and 6 - the class matrix, named rule sets and presets, as
//! edits of the rule text that leave every other rule as typed.

use signex_drc::NetScope;
use signex_types::project::FabTemplate;

use super::matrix::{CellChange, CellKey, cell, cells, set_cell};
use super::presets::{
    PresetMode, RowChangeKind, diff_rows, preset_result, tier_from_id, tier_text,
};
use super::sets::{apply_set, new_set, sync_set};
use super::*;
use crate::compile;

const OTHER: &str = "rule base  on clearance\n  require clearance >= 0.2mm then error\n";

#[test]
fn a_class_pair_compiles_and_is_one_scope_cell() {
    let src = "rule m on clearance when class == \"HV\" and other_class == \"LV\" require clearance >= 1mm then error";
    let got = compile(src);
    assert!(got.diagnostics.is_empty(), "{:?}", got.diagnostics);
    assert_eq!(
        got.rules[0].nets,
        NetScope::ClassPair("HV".into(), "LV".into())
    );
    let row = &table(src).rows[0];
    assert!(!row.text_only);
    assert_eq!(
        scope_text(&row.scope),
        "class == \"HV\" and other_class == \"LV\""
    );
    assert_eq!(parse_scope(&scope_text(&row.scope)), Ok(row.scope.clone()));
}

#[test]
fn other_class_needs_a_class_and_a_clearance() {
    let alone = compile(
        "rule m on clearance when other_class == \"LV\" require clearance >= 1mm then error",
    );
    assert_eq!(alone.rules.len(), 0);
    assert!(alone.diagnostics[0].message.contains("needs a 'class'"));
    let track = compile(
        "rule m on track when class == \"HV\" and other_class == \"LV\" require width >= 1mm then error",
    );
    assert_eq!(track.rules.len(), 0);
    assert!(track.diagnostics[0].message.contains("clearance rule only"));
}

#[test]
fn setting_an_empty_cell_adds_one_rule_and_keeps_the_rest_as_typed() {
    let key = CellKey::new("HV", "LV");
    let (text, change) = set_cell(OTHER, &key, "0.8mm").unwrap();
    assert_eq!(change, CellChange::Added("cc_HV_LV".into()));
    assert!(text.starts_with(OTHER), "{text}");
    assert!(text.contains(
        "rule cc_HV_LV on clearance when class == \"HV\" and other_class == \"LV\" require clearance >= 0.8mm then error"
    ));
    // Either order of the classes is the same cell.
    let rows = table(&text);
    assert_eq!(
        cell(&rows, &CellKey::new("LV", "HV")).map(|r| r.name.as_str()),
        Some("cc_HV_LV")
    );
    assert_eq!(cells(&rows).len(), 1);
    assert_eq!(cells(&rows)[0].value, "0.8mm");

    let (changed, change) = set_cell(&text, &CellKey::new("LV", "HV"), "1mm").unwrap();
    assert_eq!(change, CellChange::Changed("cc_HV_LV".into()));
    assert!(changed.starts_with(OTHER) && changed.contains(">= 1mm"));
    let (same, change) = set_cell(&changed, &key, "1mm").unwrap();
    assert_eq!(
        (same.as_str(), change),
        (changed.as_str(), CellChange::Unchanged)
    );

    let (cleared, change) = set_cell(&changed, &key, "").unwrap();
    assert_eq!(change, CellChange::Removed("cc_HV_LV".into()));
    assert_eq!(cleared, OTHER);
}

#[test]
fn a_sub_grid_cell_is_its_own_rule_narrowed_to_an_object_pair() {
    let main = CellKey::new("HV", "LV");
    let (text, _) = set_cell("", &main, "0.8mm").unwrap();
    let sub = CellKey::new("HV", "LV").with_pair("pad-track").unwrap();
    let (text, change) = set_cell(&text, &sub, "1.2mm").unwrap();
    assert_eq!(change, CellChange::Added("cc_HV_LV_pad_track".into()));
    let rows = table(&text);
    assert_eq!(cells(&rows).len(), 2);
    let track_pad = CellKey::new("LV", "HV").with_pair("track-pad").unwrap();
    assert_eq!(
        cell(&rows, &track_pad).map(|r| r.name.as_str()),
        Some("cc_HV_LV_pad_track")
    );
    assert_eq!(
        cell(&rows, &main).map(|r| r.name.as_str()),
        Some("cc_HV_LV")
    );
    assert!(CellKey::new("a", "b").with_pair("track").is_err());
}

#[test]
fn a_bad_cell_value_is_refused() {
    let key = CellKey::new("HV", "LV");
    assert!(set_cell(OTHER, &key, "wide").is_err());
    assert!(set_cell(OTHER, &CellKey::new("H\"V", "LV"), "1mm").is_err());
}

#[test]
fn a_set_applied_to_a_class_writes_one_rule_per_kind() {
    let set = new_set("min_clearance=0.8mm, min_width=0.3mm", Some("warning")).unwrap();
    assert_eq!(set.limits.len(), 2);
    let (text, set) = apply_set(OTHER, "HV_08", &set, "class == \"HV\"").unwrap();
    assert!(text.starts_with(OTHER), "{text}");
    assert!(text.contains(
        "rule HV_08_HV_clearance on clearance when class == \"HV\" require clearance >= 0.8mm then warning"
    ));
    assert!(text.contains(
        "rule HV_08_HV_track on track when class == \"HV\" require width >= 0.3mm then warning"
    ));
    assert_eq!(set.uses.len(), 1);
    assert_eq!(set.uses[0].scope, "class == \"HV\"");
    assert_eq!(set.uses[0].rules, ["HV_08_HV_clearance", "HV_08_HV_track"]);
    // Applied again: the same rules, no second copy.
    let (again, set) = apply_set(&text, "HV_08", &set, "class == HV").unwrap();
    assert_eq!(again, text);
    assert_eq!(set.uses.len(), 1);
}

#[test]
fn changing_a_set_rewrites_every_rule_it_wrote() {
    let set = new_set("min_clearance=0.8mm, min_width=0.3mm", None).unwrap();
    let (text, set) = apply_set(OTHER, "hv", &set, "class == \"HV\"").unwrap();
    let (text, set) = apply_set(&text, "hv", &set, "net == \"VBUS\"").unwrap();
    let changed = super::sets::redefined(&set, new_set("min_clearance=1mm", None).unwrap());
    let (text, synced) = sync_set(&text, "hv", &changed).unwrap();
    assert!(text.starts_with(OTHER), "{text}");
    assert!(text.contains(
        "rule hv_HV_clearance on clearance when class == \"HV\" require clearance >= 1mm then error"
    ));
    assert!(text.contains("rule hv_VBUS_clearance on clearance when net == \"VBUS\" require clearance >= 1mm then error"));
    assert!(
        !text.contains("on track"),
        "the set no longer limits width: {text}"
    );
    assert_eq!(synced.uses[0].rules, ["hv_HV_clearance"]);
    assert_eq!(synced.uses[1].rules, ["hv_VBUS_clearance"]);
}

#[test]
fn a_set_needs_known_limits_with_units() {
    assert!(new_set("", None).is_err());
    assert!(new_set("min_gap=1mm", None).is_err());
    assert!(new_set("min_clearance=1", None).is_err());
    assert!(new_set("min_clearance=1mm", Some("loud")).is_err());
}

#[test]
fn a_tier_merges_by_name_and_a_profile_replaces() {
    assert_eq!(tier_from_id("advanced"), Some(FabTemplate::Advanced));
    let tier = tier_text(FabTemplate::Standard);
    assert!(
        tier.contains("rule fab_clearance on clearance require clearance >= 0.15mm then error")
    );
    assert!(
        tier.contains(
            "rule fab_via on via require diameter >= 0.5mm and drill >= 0.25mm then error"
        )
    );
    let current = "rule fab_track on track require width >= 0.3mm then error\nrule mine on clearance require clearance >= 0.3mm then error\n";
    let merged = preset_result(current, &tier, PresetMode::Merge).unwrap();
    assert!(merged.contains("rule mine on clearance"), "{merged}");
    let changes = diff_rows(current, &merged);
    let kinds: Vec<(&str, RowChangeKind)> =
        changes.iter().map(|c| (c.name.as_str(), c.kind)).collect();
    assert_eq!(
        kinds,
        [
            ("fab_track", RowChangeKind::Changed),
            ("fab_clearance", RowChangeKind::Added),
            ("fab_via", RowChangeKind::Added),
        ]
    );
    let replaced = preset_result(current, &tier, PresetMode::Replace).unwrap();
    let kinds: Vec<(&str, RowChangeKind)> = diff_rows(current, &replaced)
        .iter()
        .map(|c| (c.name.clone(), c.kind))
        .map(|(n, k)| (if n == "mine" { "mine" } else { "other" }, k))
        .collect();
    assert_eq!(kinds[0], ("mine", RowChangeKind::Removed));
}

#[test]
fn spacing_alone_is_not_a_change() {
    let a = "rule x on via require drill >= 0.2mm then error";
    let b = "rule x  on via\n  require drill >= 0.2mm then error\n";
    assert!(diff_rows(a, b).is_empty());
}
