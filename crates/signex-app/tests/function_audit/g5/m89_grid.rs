//! Mockup row 89: the Rules tab's constraint grid, its domain bar, find box,
//! filter chips and rule rows.

use signex_app::app::RulesTabMsg;
use signex_drc_dsl::table::catalog::Domain;
use signex_drc_dsl::table::{RuleField, RuleKind};
use signex_widgets::constraint_grid::GridMessage;

use super::{Rig, SIG_RULE, all_row_clearance};

const BOARD_WIDE: &str = "on clearance require clearance >= 0.25mm";

#[test]
fn a_value_typed_in_a_grid_cell_writes_one_rule_marks_dirty_and_one_undo_restores() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    rig.assert_untouched(SIG_RULE);

    rig.type_in_all_row("0.25");

    let text = rig.stored();
    assert!(text.starts_with(SIG_RULE), "the typed rule stays: {text}");
    assert!(text.contains(BOARD_WIDE), "a board-wide rule: {text}");
    assert_eq!(rig.table().rows.len(), 2);
    assert!(rig.is_dirty(), "an edit marks the project dirty");
    assert_eq!(rig.tab().refused, None);

    rig.undo();
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn cancelling_a_grid_cell_edit_writes_nothing() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    rig.grid(GridMessage::CellPressed(all_row_clearance()));
    rig.grid(GridMessage::EditTextChanged("0.25".to_string()));
    rig.grid(GridMessage::EditCancelled);
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn enter_on_an_untouched_grid_cell_writes_nothing() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    rig.grid(GridMessage::CellPressed(all_row_clearance()));
    rig.grid(GridMessage::EditSubmitted);
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn a_grid_cell_refuses_text_a_negative_and_a_comma_decimal_and_says_why() {
    for bad in ["abc", "-0.25", "0,25"] {
        let mut rig = Rig::new(Some(SIG_RULE), &[]);
        rig.type_in_all_row(bad);
        assert!(
            !rig.refusal().is_empty(),
            "'{bad}' is refused with a reason"
        );
        rig.assert_untouched(SIG_RULE);
    }
}

#[test]
fn a_grid_edit_is_refused_while_the_stored_text_has_a_syntax_error() {
    let broken = "rule broken on clearance require clearance >= then error\n";
    let mut rig = Rig::new(Some(broken), &[]);
    rig.type_in_all_row("0.25");
    assert!(rig.refusal().contains("syntax error"), "{}", rig.refusal());
    rig.assert_untouched(broken);
}

#[test]
fn the_cell_reset_drops_the_rows_own_value_and_undo_brings_it_back() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    rig.type_in_all_row("0.25");

    rig.grid(GridMessage::CellReset(all_row_clearance()));
    let text = rig.stored();
    assert!(!text.contains("0.25mm"), "the value is gone: {text}");
    assert!(rig.table().row("sig").is_some(), "other rules stay: {text}");

    rig.undo();
    assert!(rig.stored().contains(BOARD_WIDE), "one undo restores it");
    rig.undo();
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn the_first_edit_of_machine_rules_stores_them_in_the_project_and_undo_unstores_them() {
    let mut rig = Rig::new(None, &[]);
    rig.app.ui_state.machine_drc_rules.dsl = Some(SIG_RULE.to_string());
    assert!(rig.rules().is_none(), "the project stores no rules yet");

    rig.type_in_all_row("0.25");

    let text = rig.stored();
    assert!(text.contains("rule sig "), "the machine's rule: {text}");
    assert!(text.contains(BOARD_WIDE), "the edit: {text}");
    assert!(rig.is_dirty());

    rig.undo();
    assert!(rig.rules().is_none(), "undo stores no rules again");
    assert!(!rig.is_dirty());
}

#[test]
fn each_domain_of_the_bar_shows_its_grid_and_writes_nothing() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    for domain in Domain::ALL {
        let project_id = rig.id;
        rig.send(RulesTabMsg::DomainPicked { project_id, domain });
        assert_eq!(rig.tab().grid.domain(), domain);
    }
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn typing_creepage_in_find_jumps_to_its_domain_and_enter_steps_the_matches() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    rig.send(RulesTabMsg::DomainPicked {
        project_id,
        domain: Domain::Physical,
    });

    rig.grid(GridMessage::ColumnQueryChanged("creepage".to_string()));
    assert_eq!(rig.tab().grid.domain(), Domain::Clearance);
    let hits = rig.tab().find.hits.len();
    assert!(hits >= 1, "creepage is a constraint");
    assert_eq!(rig.tab().find.at, 0);

    rig.grid(GridMessage::ColumnQuerySubmitted);
    assert_eq!(rig.tab().find.at, 1 % hits, "Enter steps, wrapping");
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn find_does_not_jump_on_two_letters_or_on_a_word_nothing_matches() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    rig.send(RulesTabMsg::DomainPicked {
        project_id,
        domain: Domain::Physical,
    });
    for query in ["cr", "zzzzqq"] {
        rig.grid(GridMessage::ColumnQueryChanged(query.to_string()));
        assert_eq!(rig.tab().grid.domain(), Domain::Physical, "{query}");
    }
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn the_inherited_and_overridden_chips_toggle_and_write_nothing() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    assert!(!rig.tab().hide_inherited && !rig.tab().overridden_only);

    rig.send(RulesTabMsg::InheritedToggled { project_id });
    rig.send(RulesTabMsg::OverriddenOnlyToggled { project_id });
    assert!(rig.tab().hide_inherited && rig.tab().overridden_only);

    rig.send(RulesTabMsg::InheritedToggled { project_id });
    rig.send(RulesTabMsg::OverriddenOnlyToggled { project_id });
    assert!(!rig.tab().hide_inherited && !rig.tab().overridden_only);
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn add_writes_a_new_selected_rule_and_undo_removes_it() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    rig.send(RulesTabMsg::RuleAdded {
        project_id,
        kind: RuleKind::Track,
    });

    assert_eq!(rig.table().rows.len(), 2);
    let selected = rig
        .tab()
        .selected
        .clone()
        .expect("the new rule is selected");
    assert!(rig.table().row(&selected).is_some(), "{selected}");
    assert!(rig.is_dirty());

    rig.undo();
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn removing_a_rule_clears_its_selection_and_undo_restores_it() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    rig.send(RulesTabMsg::CellOpened {
        project_id,
        rule: "sig".to_string(),
        field: RuleField::Name,
    });
    rig.send(RulesTabMsg::CellCancelled { project_id });
    assert_eq!(rig.tab().selected.as_deref(), Some("sig"));

    rig.send(RulesTabMsg::RuleRemoved {
        project_id,
        rule: "sig".to_string(),
    });
    assert!(rig.table().row("sig").is_none());
    assert_eq!(rig.tab().selected, None);
    assert!(rig.is_dirty());

    rig.undo();
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn removing_a_rule_that_does_not_exist_is_refused() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    rig.send(RulesTabMsg::RuleRemoved {
        project_id,
        rule: "nothing_here".to_string(),
    });
    assert!(rig.refusal().contains("nothing_here"), "{}", rig.refusal());
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn switching_a_rule_off_and_on_rewrites_it_and_undo_steps_back() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    let toggled = |on| RulesTabMsg::EnabledToggled {
        project_id,
        rule: "sig".to_string(),
        on,
    };

    rig.send(toggled(false));
    assert!(!rig.table().row("sig").expect("the rule").enabled);
    assert!(rig.is_dirty());

    rig.send(toggled(true));
    assert!(rig.table().row("sig").expect("the rule").enabled);

    rig.undo();
    assert!(!rig.table().row("sig").expect("the rule").enabled);
    rig.undo();
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn renaming_a_rule_in_its_cell_follows_the_selection_and_undo_restores_the_name() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    rig.send(RulesTabMsg::CellOpened {
        project_id,
        rule: "sig".to_string(),
        field: RuleField::Name,
    });
    let seeded = rig.tab().edit.as_ref().map(|e| e.text.clone());
    assert_eq!(
        seeded.as_deref(),
        Some("sig"),
        "the cell opens on its value"
    );

    rig.send(RulesTabMsg::CellEdited {
        project_id,
        text: "signal_gap".to_string(),
    });
    rig.send(RulesTabMsg::CellCommitted { project_id });

    assert!(rig.table().row("signal_gap").is_some());
    assert!(rig.table().row("sig").is_none());
    assert_eq!(rig.tab().edit, None, "a commit closes the cell");
    assert_eq!(rig.tab().selected.as_deref(), Some("signal_gap"));

    rig.undo();
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn a_bad_rule_name_is_refused_and_the_cell_stays_open_until_cancelled() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    rig.send(RulesTabMsg::CellOpened {
        project_id,
        rule: "sig".to_string(),
        field: RuleField::Name,
    });
    rig.send(RulesTabMsg::CellEdited {
        project_id,
        text: "2 bad".to_string(),
    });
    rig.send(RulesTabMsg::CellCommitted { project_id });

    assert!(!rig.refusal().is_empty());
    assert!(
        rig.tab().edit.is_some(),
        "a refused edit keeps the cell open"
    );
    rig.assert_untouched(SIG_RULE);

    rig.send(RulesTabMsg::CellCancelled { project_id });
    assert_eq!(rig.tab().edit, None);
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn a_rule_severity_edit_writes_the_word_and_an_unknown_word_is_refused() {
    let mut rig = Rig::new(Some(SIG_RULE), &[]);
    let project_id = rig.id;
    rig.send(RulesTabMsg::CellOpened {
        project_id,
        rule: "sig".to_string(),
        field: RuleField::Severity,
    });
    rig.send(RulesTabMsg::CellEdited {
        project_id,
        text: "fatal".to_string(),
    });
    rig.send(RulesTabMsg::CellCommitted { project_id });
    assert!(!rig.refusal().is_empty(), "fatal is no severity");
    rig.assert_untouched(SIG_RULE);

    rig.send(RulesTabMsg::CellEdited {
        project_id,
        text: "warning".to_string(),
    });
    rig.send(RulesTabMsg::CellCommitted { project_id });
    assert!(rig.stored().contains("then warning"), "{}", rig.stored());
    assert_eq!(rig.tab().refused, None, "a good edit clears the refusal");

    rig.undo();
    rig.assert_untouched(SIG_RULE);
}
