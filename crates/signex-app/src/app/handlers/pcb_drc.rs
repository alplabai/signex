//! The read-only design-rule check of the board in view (#2222).
//!
//! The check is `signex_drc::run`; it runs in `spawn_blocking` on a copy of
//! the board, so a large board never holds the UI thread, and its report is
//! applied only if no newer run has begun. Nothing here edits the board.

use std::sync::Arc;

use iced::Task;
use signex_drc::{DrcRules, ViolationKind, findings};
use signex_types::project::DrcRuleSet;

use crate::app::state::DrcInput;
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
        self.start_pcb_drc(path, board)
    }

    /// Check `board`, the board at `path`, whether or not its tab is in
    /// view (the Rules tab runs its project's board, #2398).
    pub(crate) fn start_pcb_drc(
        &mut self,
        path: std::path::PathBuf,
        board: signex_types::pcb::PcbBoard,
    ) -> Task<Message> {
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
        let (loose, classes, layers) = self.drc_inputs(&path);
        let generation = self.interaction_state.pcb_drc.begin(DrcInput {
            path,
            revision,
            net_classes: classes.clone(),
        });
        self.refresh_panel_ctx();
        let libraries = self.open_library_roots();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    run_check_with(&board, &classes, &layers, &libraries)
                })
                .await
                .unwrap_or_else(|e| Err(format!("DRC worker failed: {e}")))
            },
            move |result| {
                Message::Pcb(PcbMsg::DrcFinished {
                    generation,
                    loose,
                    result: Arc::new(result),
                })
            },
        )
    }

    /// What a check of the board at `path` reads: whether the board is in
    /// no open project, its net classes and its rule layers. The board's
    /// own project, as `check_drc` takes it (#2332): a loose board gets this
    /// machine's layer and no net classes, and says so.
    fn drc_inputs(
        &self,
        path: &std::path::Path,
    ) -> (bool, Vec<signex_types::net_class::NetClass>, Layers) {
        let project = self.document_state.board_project(path, None);
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
        (project.is_none(), classes, layers)
    }

    /// The rules a check of `board` (the board at `path`) applies, and its
    /// project's net classes: what Interactive Routing keeps (#2389).
    pub(crate) fn board_design_rules(
        &self,
        path: &std::path::Path,
        board: &signex_types::pcb::PcbBoard,
    ) -> Result<(DrcRules, Vec<signex_types::net_class::NetClass>), String> {
        let (_, classes, layers) = self.drc_inputs(path);
        let rules = compile_rules(board, &classes, &layers)?;
        Ok((rules, classes))
    }

    /// Apply a finished run, unless a newer one superseded it. A `loose`
    /// board's run says first that it was in no open project.
    pub(crate) fn handle_pcb_drc_finished(
        &mut self,
        generation: u64,
        loose: bool,
        result: &DrcRunResult,
    ) {
        match result {
            Err(reason) => {
                if self.interaction_state.pcb_drc.abandon(generation) {
                    crate::diagnostics::log_warning(format!("DRC could not run: {reason}"));
                }
            }
            Ok((report, net_names)) => {
                // The clock is read here, once: the status bar and its DRC
                // popover say how long ago this was (#2460).
                let now = std::time::SystemTime::now();
                if self.interaction_state.pcb_drc.finish(
                    generation,
                    report.clone(),
                    net_names.clone(),
                    now,
                ) {
                    self.ui_state.board_status.now = Some(now);
                    if loose {
                        // As `check_drc` words it in its report's warnings.
                        crate::diagnostics::log_warning(super::LOOSE_BOARD_WARNING);
                    }
                    log_report(report, net_names);
                    self.log_lapsed_drc_waivers();
                }
            }
        }
        self.refresh_panel_ctx();
    }

    /// One Messages line when the last run found waivers whose objects
    /// changed since they were waived (#2307): those findings are back.
    fn log_lapsed_drc_waivers(&self) {
        let Some(done) = self.interaction_state.pcb_drc.last() else {
            return;
        };
        let Some(board) = self.document_state.pcb_board(&done.path) else {
            return;
        };
        let revision = self
            .document_state
            .pcb_revisions
            .get(&done.path)
            .copied()
            .unwrap_or(0);
        if done.is_stale(revision, self.board_net_classes(&done.path)) {
            // Edited during the run: the report is not of the geometry now.
            return;
        }
        let waivers = &board.options.drc_waivers;
        if waivers.is_empty() {
            return;
        }
        let lapsed = done
            .report
            .violations
            .iter()
            .filter(|v| {
                matches!(
                    signex_drc::waiver_state(waivers, v, board),
                    signex_drc::WaiverState::Lapsed(_)
                )
            })
            .count();
        if lapsed > 0 {
            crate::diagnostics::log_warning(format!(
                "DRC: {lapsed} waived finding(s) are back: their objects changed since they were waived"
            ));
        }
    }

    /// What the Messages panel and the DRC results panel (#2399) show of the
    /// active board's check; `None` with no PCB in view.
    pub(crate) fn pcb_drc_panel(&self) -> Option<crate::panels::DrcPanel> {
        let path = self.active_tab_path()?;
        let pcb = self.active_pcb()?;
        Some(self.pcb_drc_panel_for(&path, pcb))
    }

    /// [`Self::pcb_drc_panel`] of the open board at `path`, wherever it is
    /// shown: the DRC popover of an undocked board window reads it (#2460).
    pub(crate) fn pcb_drc_panel_for(
        &self,
        path: &std::path::Path,
        pcb: &signex_types::pcb::PcbBoard,
    ) -> crate::panels::DrcPanel {
        let path = path.to_path_buf();
        let waivers = &pcb.options.drc_waivers;
        let drc = &self.interaction_state.pcb_drc;
        let mut panel = crate::panels::DrcPanel {
            running: drc.is_running_for(&path),
            filter: drc.filter.clone(),
            view: self.ui_state.validation_panels.drc.clone(),
            waive_draft: drc.waive_draft.clone(),
            ..Default::default()
        };
        let Some(done) = drc.done_for(&path) else {
            self.fill_drc_idle(&mut panel, pcb);
            return panel;
        };
        let revision = self
            .document_state
            .pcb_revisions
            .get(&path)
            .copied()
            .unwrap_or(0);
        let report = &done.report;
        panel.ran = true;
        panel.stale = done.is_stale(revision, self.board_net_classes(&path));
        if report.rules.from_default {
            panel.default_clearance_mm =
                Some(format!("{:.1}", pcb_geom::nm_to_mm(report.rules.clearance)));
        }
        panel.checked_clearance_mm = Some(findings::mm(report.rules.clearance).to_string());
        panel.written_clearance = report.rules.sets_written_clearance();
        panel.rules_not_applied = report.rules.rules_not_applied;
        panel.rule_text_unparsed = report.rules.texts_not_applied > 0;
        panel.clearance = report.count(ViolationKind::Clearance);
        panel.short = report.count(ViolationKind::Short);
        panel.unconnected = report.count(ViolationKind::Unconnected);
        panel.keepout = report.count(ViolationKind::Keepout);
        panel.skipped = report.skipped.len();
        panel.skipped_keepouts = report.skipped_keepouts.len();
        panel.hidden = report
            .violations
            .len()
            .saturating_sub(crate::panels::DRC_MAX_ROWS);
        // Net 0 is no net: left empty for the panel to word.
        let name = |net: u32| match net {
            0 => String::new(),
            _ => done
                .net_names
                .get(&net)
                .cloned()
                .unwrap_or_else(|| format!("#{net}")),
        };
        let mm = |nm: i64| format!("{:.3}", pcb_geom::nm_to_mm(nm));
        panel.rows = report
            .violations
            .iter()
            .enumerate()
            .take(crate::panels::DRC_MAX_ROWS)
            .map(|(index, v)| {
                let layers = pcb_geom::copper_layers(v.layers);
                crate::panels::DrcRow {
                    index,
                    kind: v.kind,
                    a: name(v.nets.0),
                    b: name(v.nets.1),
                    measured_mm: mm(v.measured_nm),
                    required_mm: mm(v.required_nm),
                    // A kind with a maximum broke it when over the limit.
                    over_maximum: v.kind.limits().iter().any(|k| k.is_maximum())
                        && v.measured_nm > v.required_nm,
                    x_mm: mm(v.at.x),
                    y_mm: mm(v.at.y),
                    layer: layers.first().copied(),
                    layers,
                    key: signex_drc::waiver_key(v),
                    rule: signex_drc::rule_id(v).to_string(),
                    severity: report.severity_of(v),
                    objects: v.items.len(),
                    // A lapsed waiver leaves the row open (#2307).
                    waiver: match signex_drc::waiver_state(waivers, v, pcb) {
                        signex_drc::WaiverState::Waived(w) => Some(w.note.clone()),
                        _ => None,
                    },
                }
            })
            .collect();
        panel
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
/// No library model is read: a part's 3D body is its stored height.
#[cfg(test)]
pub(super) fn run_check(
    board: &signex_types::pcb::PcbBoard,
    classes: &[signex_types::net_class::NetClass],
    layers: &Layers,
) -> DrcRunResult {
    run_check_with(board, classes, layers, &[])
}

/// [`run_check`], the parts' 3D bodies read from the open `libraries` when
/// a rule measures them (#2939); a model that cannot be read is a note,
/// and that part's stored height is used.
pub(super) fn run_check_with(
    board: &signex_types::pcb::PcbBoard,
    classes: &[signex_types::net_class::NetClass],
    layers: &Layers,
    libraries: &[(uuid::Uuid, std::path::PathBuf)],
) -> DrcRunResult {
    let mut rules = compile_rules(board, classes, layers)?;
    let models = if rules.measures_bodies() {
        let (models, unread) = super::pcb_drc_bodies::body_models(board, libraries);
        rules.notes.extend(unread);
        models
    } else {
        signex_drc::BodyModels::new()
    };
    let report = signex_drc::run_with_bodies(board, &rules, &models);
    let names = board
        .nets
        .iter()
        .map(|n| (n.number, n.name.clone()))
        .collect();
    Ok((report, names))
}

/// The rules a check of `board` applies, the rule text compiled: shared
/// by the check and Interactive Routing (#2389), so a route keeps exactly
/// the clearances the check asks.
pub(super) fn compile_rules(
    board: &signex_types::pcb::PcbBoard,
    classes: &[signex_types::net_class::NetClass],
    layers: &Layers,
) -> Result<DrcRules, String> {
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
    Ok(rules)
}

/// The summary line. Its wording changes only when the run has written
/// rules, so a run without rule text logs what it always did.
fn summary_line(report: &signex_drc::DrcReport) -> String {
    let count = |k| report.count(k);
    let rules = &report.rules;
    let board_mm = findings::mm(rules.clearance);
    let clearance = if rules.sets_written_clearance() {
        format!("minimum clearance {board_mm} mm where no written rule applies")
    } else {
        format!("minimum clearance {board_mm} mm")
    };
    let mut tail = String::new();
    if !report.skipped_keepouts.is_empty() {
        let n = report.skipped_keepouts.len();
        tail.push_str(&format!("; {n} keepout(s) could not be checked"));
    }
    if !report.skipped.is_empty() {
        let n = report.skipped.len();
        tail.push_str(&format!("; {n} item(s) could not be checked"));
    }
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
    // The other base rules (#2435): only the kinds with findings.
    let others: Vec<String> = ViolationKind::ALL
        .into_iter()
        .filter(|&k| k > ViolationKind::ViaDrill)
        .filter_map(|k| {
            let n = report.count(k);
            let name = k
                .rule_id()
                .trim_start_matches("builtin::")
                .replace('_', " ");
            (n > 0).then(|| format!("{n} {name}"))
        })
        .collect();
    if !others.is_empty() {
        crate::diagnostics::log_warning(format!("DRC: {}", others.join(", ")));
    }
}

/// The run's Messages lines. A finding is worded by `signex_drc::findings`,
/// as `check_drc` words it (#2334).
fn log_report(report: &signex_drc::DrcReport, names: &std::collections::BTreeMap<u32, String>) {
    let summary = summary_line(report);
    for entry in &report.rules.severities.unrecognised {
        crate::diagnostics::log_info(format!("DRC rules: ignoring unrecognised entry ({entry})"));
    }
    log_rule_text(report);
    if let Some(note) = findings::default_clearance_note(&report.rules) {
        crate::diagnostics::log_info(note);
    }
    let unapplied = report.rules.rules_not_applied + report.rules.texts_not_applied > 0;
    if report.is_clean() && !unapplied {
        crate::diagnostics::log_info(summary);
        return;
    }
    crate::diagnostics::log_warning(summary);
    for skipped in &report.skipped_keepouts {
        crate::diagnostics::log_warning(findings::skipped_keepout_warning(skipped));
    }
    for skipped in &report.skipped {
        crate::diagnostics::log_warning(findings::skipped_warning(skipped));
    }
    for v in report.violations.iter().take(LOGGED_VIOLATIONS) {
        crate::diagnostics::log_warning(findings::messages_line(v, names));
    }
}
