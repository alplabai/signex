//! #2307 step 2a - which rule layers a DRC run takes its severities from.

use signex_drc::{DrcSeverities, ViolationKind};
use signex_types::project::{DrcRuleSet, ProjectRules};
use signex_types::violation::Severity;

use super::pcb_drc::drc_severities;

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
