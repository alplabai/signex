//! The ERC run behind `check_erc` (#2199, decision
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6798772157): the
//! same job as Run ERC, over the project of a named sheet (a loose sheet on
//! its own), with its inputs captured from memory and its sheets read off the
//! UI thread. It changes no ERC state and says nothing in Messages; what it
//! found comes back as a `signex_types::check` report. See `docs/CLI.md`,
//! "Checks on demand".

use std::path::Path;

use serde_json::json;
use signex_erc::check::{finding, finding_waiver, net_waiver};
use signex_types::check::{CheckReport, WaivedFinding};

use super::erc_job::{ErcRunInputs, run_erc_check_job};
use crate::app::Signex;
use crate::app::project_sheets::{open_sheets_snapshot, project_pages, project_root_sheet_path};
use crate::app::state::LoadedProject;

type RuleOverrides = std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>;
type PinMatrixOverrides = std::collections::HashMap<(u8, u8), signex_erc::Severity>;

/// The id a report names its check by.
pub(crate) const ERC_CHECK: &str = "erc";

impl Signex {
    /// The check of `sheet`'s `project` (`None`: a loose sheet), ready to run
    /// off the UI thread. Captured here from memory: no file is read.
    pub(crate) fn erc_check_job(
        &self,
        sheet: &Path,
        project: Option<&LoadedProject>,
    ) -> impl FnOnce() -> CheckReport + Send + 'static {
        let inputs = self.erc_check_inputs(sheet, project);
        move || erc_check_report(inputs)
    }

    /// What Run ERC would read for this project, with no tab of its own: the
    /// open sheets as they are in memory, every other sheet from disk.
    fn erc_check_inputs(&self, sheet: &Path, project: Option<&LoadedProject>) -> ErcRunInputs {
        let (overrides, pin_matrix_overrides) = self.erc_rule_settings_quiet(project);
        let rules = project.and_then(|p| p.data.rules.as_ref());
        ErcRunInputs {
            open: open_sheets_snapshot(&self.document_state),
            tab_paths: Vec::new(),
            active: None,
            pages: project_pages(project),
            root: Some(
                project
                    .and_then(project_root_sheet_path)
                    .unwrap_or_else(|| sheet.to_path_buf()),
            ),
            project_dir: project.map(|p| p.dir().to_path_buf()),
            overrides,
            rule_set: self.rule_set_identity_of(project),
            config: signex_erc::ErcConfig::with_pin_matrix_overrides(&pin_matrix_overrides),
            net_waivers: rules.map(|r| r.erc_net_waivers.clone()).unwrap_or_default(),
            violation_waivers: rules.map(|r| r.erc_waivers.clone()).unwrap_or_default(),
            fold: crate::app::state::project_case::fold_of(project),
        }
    }

    /// The rule settings Run ERC uses for `project`, said nowhere: the
    /// project's stored rule set, else this machine's preferences.
    fn erc_rule_settings_quiet(
        &self,
        project: Option<&LoadedProject>,
    ) -> (RuleOverrides, PinMatrixOverrides) {
        match project.and_then(|p| p.data.rules.as_ref()) {
            Some(rules) => {
                let resolved = signex_erc::project_rules::resolve(&rules.erc);
                (resolved.severity, resolved.pin_matrix)
            }
            None => (
                self.ui_state.erc_severity_override.clone(),
                self.ui_state.pin_matrix_overrides.clone(),
            ),
        }
    }
}

/// Run the check and put what it found in the shared shape. Blocking.
fn erc_check_report(inputs: ErcRunInputs) -> CheckReport {
    let waivers = inputs.violation_waivers.clone();
    let (outcome, net_waived) = run_erc_check_job(inputs);
    let name = |path: &Path| path.display().to_string();
    let findings = outcome
        .by_path
        .iter()
        .flat_map(|(path, list)| list.iter().map(move |v| finding(v, &name(path))))
        .collect();
    let per_finding = outcome.waived_findings.iter().map(|w| WaivedFinding {
        finding: finding(&w.violation, &name(&w.path)),
        waiver: finding_waiver(&w.waiver),
    });
    let per_net = net_waived
        .iter()
        .map(|(path, v, (rule, net))| WaivedFinding {
            finding: finding(v, &name(path)),
            waiver: net_waiver(rule, net),
        });
    let lapsed = outcome
        .lapsed_waivers
        .iter()
        .map(|(rule, objects)| {
            waivers
                .iter()
                .find(|w| w.same_identity(rule, objects))
                .map_or_else(
                    || json!({"kind": "finding", "rule_id": rule, "objects": objects}),
                    finding_waiver,
                )
        })
        .collect();
    CheckReport::new(
        ERC_CHECK,
        findings,
        per_finding.chain(per_net).collect(),
        lapsed,
        outcome.warnings,
    )
}
