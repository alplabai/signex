//! An ERC run reads the project's stored rule set, not the machine's
//! preferences (#1085).

use std::collections::HashMap;

use signex_erc::{RuleKind, Severity};

use crate::app::handlers::menu::export::tests::app_workspace;

fn project_rules(severity: &[(&str, &str)]) -> signex_types::project::ProjectRules {
    let mut rules = signex_types::project::ProjectRules::default();
    for (id, sev) in severity {
        rules.erc.severity.insert((*id).into(), (*sev).into());
    }
    rules
}

#[test]
fn a_project_rule_set_overrides_the_machine_preferences() {
    let dir = std::env::temp_dir().join(format!("signex-erc-rules-{}", uuid::Uuid::new_v4()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    app.document_state.projects[0].data.rules = Some(project_rules(&[
        ("dangling_wire", "off"),
        ("unused_pin", "error"),
    ]));

    // Two machines with different preferences must resolve the same rules.
    let mut resolved = Vec::new();
    for machine_prefs in [
        HashMap::new(),
        HashMap::from([(RuleKind::DanglingWire, Severity::Error)]),
    ] {
        app.ui_state.erc_severity_override = machine_prefs;
        app.ui_state
            .pin_matrix_overrides
            .insert((0, 0), Severity::Error);
        let (_, inputs) = app.begin_erc_run();
        resolved.push(inputs.overrides);
        assert_eq!(
            inputs.config.pin_matrix.cell(0, 0),
            Severity::Off,
            "machine pin matrix must not leak into a project with rules"
        );
    }
    assert_eq!(resolved[0], resolved[1]);
    assert_eq!(resolved[0][&RuleKind::DanglingWire], Severity::Off);
    assert_eq!(resolved[0][&RuleKind::UnusedPin], Severity::Error);
}

#[test]
fn a_project_without_rules_still_uses_the_machine_preferences() {
    let dir = std::env::temp_dir().join(format!("signex-erc-norules-{}", uuid::Uuid::new_v4()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    assert!(app.document_state.projects[0].data.rules.is_none());
    app.ui_state
        .erc_severity_override
        .insert(RuleKind::DanglingWire, Severity::Off);
    let (_, inputs) = app.begin_erc_run();
    assert_eq!(inputs.overrides[&RuleKind::DanglingWire], Severity::Off);
}

#[test]
fn the_preferences_fallback_note_is_logged_once_per_project_open() {
    let dir = std::env::temp_dir().join(format!("signex-erc-once-{}", uuid::Uuid::new_v4()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    let id = app.document_state.projects[0].id;
    assert!(app.document_state.erc_fallback_noted.is_empty());
    app.begin_erc_run();
    app.begin_erc_run();
    assert!(app.document_state.erc_fallback_noted.contains(&id));
    assert!(!app.note_erc_fallback(id), "second note for the same open");
}
