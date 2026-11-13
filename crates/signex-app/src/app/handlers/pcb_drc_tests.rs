//! #2307 step 2a - which rule layers a DRC run takes its severities from.

use signex_drc::{DrcSeverities, ViolationKind};
use signex_types::project::{DrcRuleSet, ProjectRules};
use signex_types::violation::Severity;

use super::pcb_drc::drc_severities;
use crate::app::Signex;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};

/// A project with one open sheet and no stored rules; the machine's User
/// Default sets `builtin::clearance` to warning.
fn project_without_rules() -> Signex {
    let dir = std::env::temp_dir().join(format!("signex-drc-seed-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R1", "NET_A", &[]),
    );
    app.document_state.active_path = Some(top);
    app.ui_state.machine_drc_rules = set("builtin::clearance", "warning");
    app
}

/// The clearance severity a DRC run of the active project would use.
fn clearance(app: &Signex) -> Option<Severity> {
    let rules = app
        .document_state
        .active_document_project()
        .and_then(|p| p.data.rules.as_ref());
    drc_severities(rules, &app.ui_state.machine_drc_rules).of(ViolationKind::Clearance)
}

fn stores_rules(app: &Signex) -> bool {
    app.document_state
        .active_document_project()
        .is_some_and(|p| p.data.rules.is_some())
}

/// An ERC-only action that makes the project store rules must not switch
/// DRC away from the machine's DRC rules (#2307 review).
#[test]
fn storing_erc_rules_in_the_project_keeps_the_drc_severities() {
    let mut app = project_without_rules();
    assert!(!stores_rules(&app));
    assert_eq!(clearance(&app), Some(Severity::Warning));
    let _ = app.handle_store_erc_rules_in_project();
    assert!(stores_rules(&app));
    assert_eq!(clearance(&app), Some(Severity::Warning));
}

/// With no machine DRC rules, storing ERC rules writes no `drc` key, so the
/// project file is what it was before #2307.
#[test]
fn storing_erc_rules_without_machine_drc_rules_writes_no_drc_key() {
    let mut app = project_without_rules();
    app.ui_state.machine_drc_rules = DrcRuleSet::default();
    let _ = app.handle_store_erc_rules_in_project();
    let rules = app
        .document_state
        .active_document_project()
        .and_then(|p| p.data.rules.as_ref())
        .expect("stored rules");
    let json = serde_json::to_value(rules).unwrap();
    assert!(json.get("erc").is_some(), "{json}");
    assert!(json.get("drc").is_none(), "{json}");
}

#[test]
fn a_first_erc_waiver_keeps_the_drc_severities() {
    let mut app = project_without_rules();
    assert_eq!(clearance(&app), Some(Severity::Warning));
    let _ = app.handle_erc_net_waiver_set("user::net_check", "NET_A", true);
    assert!(stores_rules(&app));
    assert_eq!(clearance(&app), Some(Severity::Warning));
}

fn set(id: &str, sev: &str) -> DrcRuleSet {
    let mut s = DrcRuleSet::default();
    s.severity.insert(id.into(), sev.into());
    s
}

#[test]
fn no_rules_anywhere_gives_the_builtin_severities() {
    assert_eq!(
        drc_severities(None, &DrcRuleSet::default()),
        DrcSeverities::builtin()
    );
    assert_eq!(
        drc_severities(Some(&ProjectRules::default()), &DrcRuleSet::default()),
        DrcSeverities::builtin()
    );
}

#[test]
fn a_project_without_rules_uses_the_machine_profile_layer() {
    let machine = set("builtin::clearance", "warning");
    let got = drc_severities(None, &machine);
    assert_eq!(got.of(ViolationKind::Clearance), Some(Severity::Warning));
}

#[test]
fn a_project_with_rules_uses_its_own_and_ignores_the_machine() {
    let machine = set("builtin::clearance", "warning");
    let project = ProjectRules {
        drc: set("builtin::short", "info"),
        ..ProjectRules::default()
    };
    let got = drc_severities(Some(&project), &machine);
    assert_eq!(got.of(ViolationKind::Short), Some(Severity::Info));
    assert_eq!(got.of(ViolationKind::Clearance), Some(Severity::Error));
}

/// #2307 step 2b: a run keeps the rule text's messages and adds, after
/// them, the net classes a written rule lowers.
#[test]
fn a_run_says_the_text_messages_then_the_classes_a_rule_lowers() {
    use signex_types::net_class::NetClass;
    use signex_types::pcb::{NetDef, PcbBoard};
    let board = PcbBoard {
        sketch: None,
        uuid: uuid::Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: vec![NetDef {
            number: 1,
            name: "HV_A".into(),
        }],
        footprints: Vec::new(),
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    };
    let mut hv = NetClass::named("HV");
    hv.patterns = vec!["HV_*".into()];
    hv.clearance = 0.5;
    let layers = super::pcb_drc::Layers {
        profile: Some(DrcRuleSet {
            dsl: Some(
                "rule base on clearance require clearance >= 0.15mm then error\nrule bad on via"
                    .into(),
            ),
            ..DrcRuleSet::default()
        }),
        project: None,
    };
    let (report, _) = super::pcb_drc::run_check(&board, &[hv.clone()], &layers).unwrap();
    // A syntax error drops the whole text: nothing is lowered.
    assert_eq!(report.rules.notes.len(), 1, "{:?}", report.rules.notes);
    assert!(report.rules.notes[0].starts_with("DRC DSL (profile): "));

    let layers = super::pcb_drc::Layers {
        profile: Some(DrcRuleSet {
            dsl: Some(
                "rule base on clearance require clearance >= 0.15mm then error\n\
                 rule z on track require width >= 0mm then error"
                    .into(),
            ),
            ..DrcRuleSet::default()
        }),
        project: None,
    };
    let (report, _) = super::pcb_drc::run_check(&board, &[hv], &layers).unwrap();
    assert_eq!(
        report.rules.notes,
        [
            "DRC DSL (profile): line 2, column 34: rule 'z': a limit of 0 checks nothing for this rule's scope",
            "DRC rule 'base' lowers net class 'HV' clearance 0.5 \u{2192} 0.15 mm",
        ]
    );
}
