//! The read-only design-rule check of the board in view (#2222).
//!
//! The check is `signex_drc::run`; it runs in `spawn_blocking` on a copy of
//! the board, so a large board never holds the UI thread, and its report is
//! applied only if no newer run has begun. Nothing here edits the board.

use std::sync::Arc;

use iced::Task;
use signex_drc::{DrcRules, ViolationKind};
use signex_types::project::DrcRuleSet;

use crate::app::{DrcRunResult, Message, PcbMsg, Signex};

/// Violations written to the Messages log after the summary line; the panel
/// lists them all (up to its own cap), the log is for a headless run.
const LOGGED_VIOLATIONS: usize = 20;

impl Signex {
    /// Start a check of the active board. With no PCB in view it only says so.
    pub(crate) fn handle_pcb_drc_requested(&mut self) -> Task<Message> {
        let Some(path) = self.active_tab_path() else {
            crate::diagnostics::log_info("DRC: no PCB is open");
            return Task::none();
        };
        let Some(board) = self.active_pcb().cloned() else {
            crate::diagnostics::log_info("DRC: the active document is not a PCB");
            return Task::none();
        };
        let revision = self
            .document_state
            .pcb_revisions
            .get(&path)
            .copied()
            .unwrap_or(0);
        if self.interaction_state.pcb_drc.is_running_for(&path) {
            // One run per board at a time: its result is what the panel will show.
            crate::diagnostics::log_info("DRC: a check of this board is already running");
            return Task::none();
        }
        let project = self.document_state.active_document_project();
        let classes = project
            .map(|p| p.data.net_classes.clone())
            .unwrap_or_default();
        let (profile, project_layer) = drc_layers(
            project.and_then(|p| p.data.rules.as_ref()),
            &self.ui_state.machine_drc_rules,
        );
        let layers = Layers {
            profile: profile.cloned(),
            project: project_layer.cloned(),
        };
        let generation = self.interaction_state.pcb_drc.begin(path, revision);
        self.refresh_panel_ctx();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || run_check(&board, &classes, &layers))
                    .await
                    .unwrap_or_else(|e| Err(format!("DRC worker failed: {e}")))
            },
            move |result| {
                Message::Pcb(PcbMsg::DrcFinished {
                    generation,
                    result: Arc::new(result),
                })
            },
        )
    }

    /// Apply a finished run, unless a newer one superseded it.
    pub(crate) fn handle_pcb_drc_finished(&mut self, generation: u64, result: &DrcRunResult) {
        match result {
            Err(reason) => {
                if self.interaction_state.pcb_drc.abandon(generation) {
                    crate::diagnostics::log_warning(format!("DRC could not run: {reason}"));
                }
            }
            Ok((report, net_names)) => {
                if self.interaction_state.pcb_drc.finish(
                    generation,
                    report.clone(),
                    net_names.clone(),
                ) {
                    log_report(report, net_names);
                }
            }
        }
        self.refresh_panel_ctx();
    }

    /// What the Messages panel shows of the active board's check; `None`
    /// with no PCB in view.
    pub(crate) fn pcb_drc_panel(&self) -> Option<crate::panels::DrcPanel> {
        let path = self.active_tab_path()?;
        self.active_pcb()?;
        let drc = &self.interaction_state.pcb_drc;
        let mut panel = crate::panels::DrcPanel {
            running: drc.is_running_for(&path),
            ..Default::default()
        };
        let Some(done) = drc.done_for(&path) else {
            return Some(panel);
        };
        let revision = self
            .document_state
            .pcb_revisions
            .get(&path)
            .copied()
            .unwrap_or(0);
        let report = &done.report;
        panel.ran = true;
        panel.stale = done.revision != revision;
        if report.rules.from_default {
            panel.default_clearance_mm =
                Some(format!("{:.1}", pcb_geom::nm_to_mm(report.rules.clearance)));
        }
        panel.written_clearance = report.rules.sets_written_clearance();
        panel.rules_not_applied = report.rules.rules_not_applied;
        panel.rule_text_unparsed = report.rules.texts_not_applied > 0;
        panel.clearance = report.count(ViolationKind::Clearance);
        panel.short = report.count(ViolationKind::Short);
        panel.unconnected = report.count(ViolationKind::Unconnected);
        panel.skipped = report.skipped.len();
        panel.hidden = report
            .violations
            .len()
            .saturating_sub(crate::panels::DRC_MAX_ROWS);
        let name = |net: u32| {
            done.net_names
                .get(&net)
                .cloned()
                .unwrap_or_else(|| format!("#{net}"))
        };
        let mm = |nm: i64| format!("{:.3}", pcb_geom::nm_to_mm(nm));
        panel.rows = report
            .violations
            .iter()
            .enumerate()
            .take(crate::panels::DRC_MAX_ROWS)
            .map(|(index, v)| crate::panels::DrcRow {
                index,
                kind: v.kind,
                a: name(v.nets.0),
                b: name(v.nets.1),
                measured_mm: mm(v.measured_nm),
                required_mm: mm(v.required_nm),
                over_maximum: v.kind == ViolationKind::TrackWidth && v.measured_nm > v.required_nm,
                x_mm: mm(v.at.x),
                y_mm: mm(v.at.y),
                layer: pcb_geom::copper_layers(v.layers).first().copied(),
            })
            .collect();
        Some(panel)
    }

    /// A violation row was clicked: select its items on the canvas.
    pub(crate) fn handle_pcb_drc_violation_picked(&mut self, index: usize) {
        let Some(path) = self.active_tab_path() else {
            return;
        };
        let Some(items) = self
            .interaction_state
            .pcb_drc
            .done_for(&path)
            .and_then(|d| d.report.violations.get(index))
            .map(|v| v.items.clone())
        else {
            return;
        };
        self.select_pcb_items(&items);
    }
}

/// The built-in rule severities of the layers [`drc_layers`] picks.
#[cfg(test)]
pub(crate) fn drc_severities(
    project: Option<&signex_types::project::ProjectRules>,
    machine: &DrcRuleSet,
) -> signex_drc::DrcSeverities {
    let (profile, project) = drc_layers(project, machine);
    signex_drc::resolve_severities(profile, project)
}

/// The `(profile, project)` rule layers a DRC run reads, severities and
/// rule text alike, as ERC picks its rules: a project that stores a rule
/// set uses its `rules.drc` alone, so it checks the same on every machine;
/// otherwise the machine's `User Default` profile layer.
pub(crate) fn drc_layers<'a>(
    project: Option<&'a signex_types::project::ProjectRules>,
    machine: &'a DrcRuleSet,
) -> (Option<&'a DrcRuleSet>, Option<&'a DrcRuleSet>) {
    match project {
        Some(rules) => (None, Some(&rules.drc)),
        None => (Some(machine), None),
    }
}

/// The rule layers, owned, for the background run.
pub(super) struct Layers {
    pub(super) profile: Option<DrcRuleSet>,
    pub(super) project: Option<DrcRuleSet>,
}

/// The run itself, off the UI thread: the rule text is compiled here too.
pub(super) fn run_check(
    board: &signex_types::pcb::PcbBoard,
    classes: &[signex_types::net_class::NetClass],
    layers: &Layers,
) -> DrcRunResult {
    let (profile, project) = (layers.profile.as_ref(), layers.project.as_ref());
    let written = signex_drc_dsl::compile_layers(profile, project);
    let mut rules = DrcRules::for_board(board, classes)
        .map_err(|e| format!("the board's clearance rules are not usable: {e}"))?
        .with_user_rules(board, classes, written.rules.iter().cloned());
    rules.severities = signex_drc::resolve_severities_with(profile, project, &written.rules);
    // The text's own messages first, then what the rules lower.
    let lowered = std::mem::take(&mut rules.notes);
    rules.notes = written.messages;
    rules.notes.extend(lowered);
    rules.rules_not_applied = written.not_applied;
    rules.texts_not_applied = written.unparsed;
    let report = signex_drc::run(board, &rules);
    let names = board
        .nets
        .iter()
        .map(|n| (n.number, n.name.clone()))
        .collect();
    Ok((report, names))
}

/// The summary line. Its wording changes only when the run has written
/// rules, so a run without rule text logs what it always did.
fn summary_line(report: &signex_drc::DrcReport) -> String {
    let count = |k| report.count(k);
    let rules = &report.rules;
    let board_mm = pcb_geom::nm_to_mm(rules.clearance);
    let clearance = if rules.sets_written_clearance() {
        format!("minimum clearance {board_mm} mm where no written rule applies")
    } else {
        format!("minimum clearance {board_mm} mm")
    };
    let tail = if report.skipped.is_empty() {
        String::new()
    } else {
        format!("; {} item(s) could not be checked", report.skipped.len())
    };
    format!(
        "DRC: {} clearance, {} short, {} unconnected, {} keepout ({clearance}){tail}",
        count(ViolationKind::Clearance),
        count(ViolationKind::Short),
        count(ViolationKind::Unconnected),
        count(ViolationKind::Keepout),
    )
}

/// What the run made of the rule text, before the summary. What was not
/// applied gets its own short lines, so the Messages length cap never cuts
/// it off a long summary.
fn log_rule_text(report: &signex_drc::DrcReport) {
    for note in &report.rules.notes {
        crate::diagnostics::log_warning(note.clone());
    }
    for remark in &report.rules.remarks {
        crate::diagnostics::log_info(remark.clone());
    }
    if report.rules.texts_not_applied > 0 {
        crate::diagnostics::log_warning("DRC rules: the rule text was not applied (syntax error)");
    }
    if report.rules.rules_not_applied > 0 {
        crate::diagnostics::log_warning(format!(
            "DRC rules: {} written rule(s) not applied",
            report.rules.rules_not_applied
        ));
    }
    let written = report.rules.user_rules().count();
    if written > 0 {
        crate::diagnostics::log_info(format!("DRC DSL: applying {written} rule(s)"));
    }
    let sizes = [
        ViolationKind::TrackWidth,
        ViolationKind::ViaDiameter,
        ViolationKind::ViaDrill,
    ]
    .map(|k| report.count(k));
    if sizes.iter().any(|&n| n > 0) {
        crate::diagnostics::log_warning(format!(
            "DRC: {} track width, {} via diameter, {} via drill",
            sizes[0], sizes[1], sizes[2]
        ));
    }
}

fn log_report(report: &signex_drc::DrcReport, names: &std::collections::BTreeMap<u32, String>) {
    let summary = summary_line(report);
    for entry in &report.rules.severities.unrecognised {
        crate::diagnostics::log_info(format!("DRC rules: ignoring unrecognised entry ({entry})"));
    }
    log_rule_text(report);
    if report.rules.from_default {
        let mm = pcb_geom::nm_to_mm(report.rules.clearance);
        crate::diagnostics::log_info(if report.rules.sets_written_clearance() {
            format!(
                "DRC: No clearance rule set on this board; {mm} mm applies where no written rule does. Set it in Board Setup."
            )
        } else {
            format!(
                "DRC: No clearance rule set on this board; checked with {mm} mm. Set it in Board Setup."
            )
        });
    }
    let unapplied = report.rules.rules_not_applied + report.rules.texts_not_applied > 0;
    if report.violations.is_empty() && report.skipped.is_empty() && !unapplied {
        crate::diagnostics::log_info(summary);
        return;
    }
    crate::diagnostics::log_warning(summary);
    let name = |net: u32| {
        names
            .get(&net)
            .cloned()
            .unwrap_or_else(|| format!("net {net}"))
    };
    for v in report.violations.iter().take(LOGGED_VIOLATIONS) {
        let what = match v.kind {
            ViolationKind::Clearance => format!(
                "clearance {} mm, needs {} mm",
                pcb_geom::nm_to_mm(v.measured_nm),
                pcb_geom::nm_to_mm(v.required_nm)
            ),
            ViolationKind::Short => "short".to_string(),
            ViolationKind::Unconnected => format!(
                "link of {} mm not routed",
                pcb_geom::nm_to_mm(v.measured_nm)
            ),
            ViolationKind::Keepout => "inside a keepout that forbids it".to_string(),
            ViolationKind::TrackWidth => size_text("track width", v),
            ViolationKind::ViaDiameter => size_text("via diameter", v),
            ViolationKind::ViaDrill => size_text("via drill", v),
        };
        let rule = v
            .rule
            .as_ref()
            .map_or_else(String::new, |id| format!(" ({id})"));
        crate::diagnostics::log_warning(format!(
            "DRC: {what}{rule}, {} / {} at ({}, {}) mm",
            name(v.nets.0),
            name(v.nets.1),
            pcb_geom::nm_to_mm(v.at.x),
            pcb_geom::nm_to_mm(v.at.y)
        ));
    }
}

/// `track width 0.1 mm, needs at least 0.15 mm` (or `at most`).
fn size_text(what: &str, v: &signex_drc::Violation) -> String {
    let bound = if v.measured_nm < v.required_nm {
        "at least"
    } else {
        "at most"
    };
    format!(
        "{what} {} mm, needs {bound} {} mm",
        pcb_geom::nm_to_mm(v.measured_nm),
        pcb_geom::nm_to_mm(v.required_nm)
    )
}
