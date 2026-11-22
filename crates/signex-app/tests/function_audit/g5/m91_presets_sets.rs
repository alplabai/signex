//! Mockup row 91, the Presets and Rule sets panels: a preset shows what it
//! changes before Apply; a set is limits written once and applied to scopes.

use std::collections::BTreeMap;

use signex_app::app::RulesTabMsg;
use signex_drc_dsl::table::presets::RowChangeKind;
use signex_types::project::NamedRuleSet;

use super::{Rig, SIG_RULE};

const ADVANCED: &str = "tier:advanced";

fn pick(rig: &mut Rig, preset: &str) {
    let project_id = rig.id;
    rig.send(RulesTabMsg::PresetPicked {
        project_id,
        preset: preset.to_string(),
    });
}

/// Fill the set form; its field message takes a type a test cannot name.
fn fill_form(rig: &mut Rig, name: &str, values: &str, severity: &str, scope: &str) {
    let form = &mut rig.tab_mut().set_form;
    form.name = name.to_string();
    form.values = values.to_string();
    form.severity = severity.to_string();
    form.scope = scope.to_string();
}

fn sets(rig: &Rig) -> BTreeMap<String, NamedRuleSet> {
    rig.rules().map(|rules| rules.drc.sets).unwrap_or_default()
}

/// The rule set `set` wrote for its first scope.
fn written_rule(rig: &Rig, set: &str) -> String {
    sets(rig)[set].uses[0].rules[0].clone()
}

#[test]
fn picking_a_tier_previews_its_seven_rules_and_writes_nothing() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    pick(&mut rig, ADVANCED);

    let preview = rig.tab().preview.as_ref().expect("the preview opens");
    assert_eq!(preview.preset, ADVANCED);
    assert_eq!(preview.changes.len(), 7, "the seven fab_* rules");
    assert!(
        preview
            .changes
            .iter()
            .all(|c| c.kind == RowChangeKind::Added && c.name.starts_with("fab_"))
    );
    rig.assert_untouched(SIG_RULE);
    assert_eq!(rig.router(), None);
}

#[test]
fn applying_a_tier_merges_its_minimums_and_router_tool_and_one_undo_restores_both() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    pick(&mut rig, ADVANCED);

    rig.send(RulesTabMsg::PresetApplied { project_id });

    assert_eq!(rig.clearance_of("fab_clearance").as_deref(), Some("0.1mm"));
    assert!(
        rig.table().row("sig").is_some(),
        "a tier merges, keeping rules"
    );
    assert_eq!(rig.router(), Some(1.6), "the Advanced tier's router tool");
    assert!(rig.tab().preview.is_none(), "Apply closes the preview");
    assert!(rig.is_dirty());

    rig.undo();
    rig.assert_untouched(SIG_RULE);
    assert_eq!(rig.router(), None, "the same step restores the router tool");
}

#[test]
fn cancelling_a_preset_preview_writes_nothing() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    pick(&mut rig, ADVANCED);

    rig.send(RulesTabMsg::PresetDismissed { project_id });

    assert!(rig.tab().preview.is_none());
    rig.assert_untouched(SIG_RULE);
    assert_eq!(rig.router(), None);
}

#[test]
fn an_unknown_preset_is_refused_and_opens_no_preview() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    pick(&mut rig, "tier:imaginary");

    assert!(rig.refusal().contains("imaginary"), "{}", rig.refusal());
    assert!(rig.tab().preview.is_none());
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn apply_is_refused_when_the_rules_changed_since_the_preview() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    pick(&mut rig, ADVANCED);
    rig.type_in_all_row("0.25");
    let edited = rig.stored();

    rig.send(RulesTabMsg::PresetApplied { project_id });

    assert!(rig.refusal().contains("changed since"), "{}", rig.refusal());
    assert_eq!(rig.stored(), edited, "the newer rules are kept");
    assert!(rig.table().row("fab_clearance").is_none());
    assert_eq!(rig.router(), None);
}

#[test]
fn closing_the_side_panel_drops_the_preset_preview() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    pick(&mut rig, ADVANCED);

    rig.send(RulesTabMsg::PanelPicked {
        project_id,
        panel: None,
    });

    assert!(rig.tab().panel.is_none());
    assert!(rig.tab().preview.is_none());
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn switching_to_constraint_sets_drops_an_open_preset_preview() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    pick(&mut rig, ADVANCED);

    rig.send(RulesTabMsg::SetsToggled { project_id });

    assert!(rig.tab().panel.is_some(), "the sets panel is shown");
    assert!(
        rig.tab().preview.is_none(),
        "leaving Presets drops its preview, as every other way out does"
    );
}

#[test]
fn the_constraint_sets_button_shows_and_hides_the_panel() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    assert!(rig.tab().panel.is_none());

    rig.send(RulesTabMsg::SetsToggled { project_id });
    assert!(rig.tab().panel.is_some());
    rig.send(RulesTabMsg::SetsToggled { project_id });
    assert!(rig.tab().panel.is_none());
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn create_set_stores_its_limits_clears_the_name_and_undo_removes_it() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    fill_form(&mut rig, "HV_08", "min_clearance=0.8mm", "error", "");

    rig.send(RulesTabMsg::SetCreated { project_id });

    let stored = sets(&rig);
    assert_eq!(stored["HV_08"].limits["min_clearance"], "0.8mm");
    assert!(stored["HV_08"].uses.is_empty(), "applied nowhere yet");
    assert_eq!(rig.stored(), SIG_RULE, "a new set writes no rule");
    assert_eq!(
        rig.tab().set_form.name,
        "",
        "the form is ready for the next"
    );
    assert!(rig.is_dirty());

    rig.undo();
    assert!(sets(&rig).is_empty());
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn create_set_refuses_a_bad_name_bad_limits_and_a_bad_severity() {
    let cases = [
        ("2bad", "min_clearance=0.8mm", "error"),
        ("HV_08", "min_clearance=abc", "error"),
        ("HV_08", "", "error"),
        ("HV_08", "no_such_limit=0.8mm", "error"),
        ("HV_08", "min_clearance=0.8mm", "fatal"),
    ];
    for (name, values, severity) in cases {
        let mut rig = Rig::new(Some(SIG_RULE), &[]);
        let project_id = rig.id;
        fill_form(&mut rig, name, values, severity, "");

        rig.send(RulesTabMsg::SetCreated { project_id });

        assert!(!rig.refusal().is_empty(), "{name} / {values} / {severity}");
        assert!(sets(&rig).is_empty());
        assert_eq!(rig.tab().set_form.name, name, "the form keeps the name");
        rig.assert_untouched(SIG_RULE);
    }
}

#[test]
fn a_set_name_already_taken_is_refused() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    fill_form(&mut rig, "HV_08", "min_clearance=0.8mm", "", "");
    rig.send(RulesTabMsg::SetCreated { project_id });

    fill_form(&mut rig, "HV_08", "min_clearance=0.9mm", "", "");
    rig.send(RulesTabMsg::SetCreated { project_id });

    assert!(
        rig.refusal().contains("already exists"),
        "{}",
        rig.refusal()
    );
    assert_eq!(sets(&rig)["HV_08"].limits["min_clearance"], "0.8mm");
}

#[test]
fn applying_a_set_writes_its_rule_and_redefining_rewrites_it_with_undo_for_each() {
    let mut rig = Rig::new(Some(SIG_RULE), &["HV"]);
    let project_id = rig.id;
    fill_form(
        &mut rig,
        "HV_08",
        "min_clearance=0.8mm",
        "error",
        "class == \"HV\"",
    );
    rig.send(RulesTabMsg::SetCreated { project_id });

    rig.send(RulesTabMsg::SetApplied {
        project_id,
        set: "HV_08".to_string(),
    });
    let rule = written_rule(&rig, "HV_08");
    assert_eq!(rig.clearance_of(&rule).as_deref(), Some("0.8mm"));
    assert_eq!(sets(&rig)["HV_08"].uses[0].scope, "class == \"HV\"");

    rig.tab_mut().set_form.values = "min_clearance=0.9mm".to_string();
    rig.send(RulesTabMsg::SetRedefined {
        project_id,
        set: "HV_08".to_string(),
    });
    assert_eq!(rig.clearance_of(&rule).as_deref(), Some("0.9mm"));

    rig.undo();
    assert_eq!(rig.clearance_of(&rule).as_deref(), Some("0.8mm"));
    rig.undo();
    assert_eq!(rig.clearance_of(&rule), None, "the rule goes with its use");
    rig.undo();
    assert!(sets(&rig).is_empty());
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn applying_a_set_to_a_bad_scope_or_a_missing_set_is_refused() {
    let mut rig = Rig::new(Some(SIG_RULE), &["HV"]);
    let project_id = rig.id;
    fill_form(&mut rig, "HV_08", "min_clearance=0.8mm", "", "nonsense");
    rig.send(RulesTabMsg::SetCreated { project_id });
    let text = rig.stored();

    rig.send(RulesTabMsg::SetApplied {
        project_id,
        set: "HV_08".to_string(),
    });
    assert!(!rig.refusal().is_empty(), "a scope that does not read");
    assert!(sets(&rig)["HV_08"].uses.is_empty());

    rig.send(RulesTabMsg::SetApplied {
        project_id,
        set: "NOT_A_SET".to_string(),
    });
    assert!(rig.refusal().contains("NOT_A_SET"), "{}", rig.refusal());
    assert_eq!(rig.stored(), text);
}

#[test]
fn redefining_a_set_with_bad_limits_is_refused_and_keeps_its_rule() {
    let mut rig = Rig::new(Some(SIG_RULE), &["HV"]);
    let project_id = rig.id;
    fill_form(
        &mut rig,
        "HV_08",
        "min_clearance=0.8mm",
        "",
        "class == \"HV\"",
    );
    rig.send(RulesTabMsg::SetCreated { project_id });
    rig.send(RulesTabMsg::SetApplied {
        project_id,
        set: "HV_08".to_string(),
    });
    let text = rig.stored();

    rig.tab_mut().set_form.values = "min_clearance=wide".to_string();
    rig.send(RulesTabMsg::SetRedefined {
        project_id,
        set: "HV_08".to_string(),
    });

    assert!(!rig.refusal().is_empty());
    assert_eq!(rig.stored(), text);
    assert_eq!(sets(&rig)["HV_08"].limits["min_clearance"], "0.8mm");
}
