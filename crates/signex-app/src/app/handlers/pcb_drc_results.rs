//! The DRC results panel's actions (#2399), one path for the panel, the
//! command palette, the CLI and the agent socket: show a violation (select
//! its objects, centre the view), filter the list, and waive or unwaive a
//! violation. A waiver is board data (`BoardOptions::drc_waivers`), set as
//! one `SetBoardOptions` undo step; the filter is view state.

use std::path::Path;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::BoardCommand;
use signex_i18n::{ids, t};
use signex_types::format::pcb_layer_token;
use signex_types::pcb_layer::PcbLayer;
use signex_types::pcb_options::{BoardOptions, DrcWaiver};

use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};
use crate::panels::{DrcFilter, DrcFilterChoice, DrcPanelMsg, DrcRow, DrcWaiveDraft};

/// What a DRC waiver says: why it is accepted, and who accepted it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DrcWaiverText {
    pub note: String,
    pub author: String,
}

impl From<&str> for DrcWaiverText {
    /// A note with no author (a command that names none).
    fn from(note: &str) -> Self {
        Self {
            note: note.to_string(),
            author: String::new(),
        }
    }
}

impl Signex {
    /// A DRC panel event, on the board in view.
    pub(crate) fn handle_drc_panel(&mut self, msg: DrcPanelMsg) -> Task<Message> {
        let Some(path) = self.active_tab_path() else {
            return Task::none();
        };
        let mut follow = Task::none();
        let refused = match msg {
            DrcPanelMsg::RowPicked(key) => {
                self.ui_state.validation_panels.drc.selected = Some(key.clone());
                if let Err(why) = self.show_drc_violation(&path, &key) {
                    let line = t!(self.ui_state.i18n, ids::DRC_PANEL_SHOW_REFUSED, "why" => why);
                    crate::diagnostics::log_warning(line.into_string());
                }
                None
            }
            DrcPanelMsg::FilterPicked(choice) => {
                let filter = &mut self.interaction_state.pcb_drc.filter;
                match choice {
                    DrcFilterChoice::Rule(rule) => filter.rule = rule,
                    DrcFilterChoice::Layer(layer) => filter.layer = layer,
                    DrcFilterChoice::Severity(severity) => filter.severity = severity,
                }
                None
            }
            DrcPanelMsg::WaivedShownToggled(on) => {
                self.interaction_state.pcb_drc.filter.show_waived = on;
                None
            }
            DrcPanelMsg::RuleOpened(rule) => {
                follow = self.open_drc_rule(&rule);
                None
            }
            DrcPanelMsg::BaseRuleOpened(rule) => {
                follow = self.open_rules_at(rule.domain(), None);
                None
            }
            DrcPanelMsg::MessagesShown => {
                follow = self.show_panel(crate::panels::PanelKind::Messages);
                None
            }
            DrcPanelMsg::WaiveStarted(key) => {
                follow = self.start_drc_waiver(key);
                None
            }
            DrcPanelMsg::ReasonActed(action) => {
                if let Some(draft) = self.interaction_state.pcb_drc.waive_draft.as_mut() {
                    draft.blank &= !action.is_edit();
                    draft.reason.perform(action);
                }
                None
            }
            DrcPanelMsg::AuthorEdited(author) => {
                if let Some(draft) = self.interaction_state.pcb_drc.waive_draft.as_mut() {
                    draft.author = author;
                    draft.blank = false;
                }
                None
            }
            DrcPanelMsg::WaiveConfirmed => self.confirm_drc_waiver(&path),
            DrcPanelMsg::WaiveCancelled => {
                self.interaction_state.pcb_drc.waive_draft = None;
                None
            }
            DrcPanelMsg::Unwaived(key) => self.set_drc_waiver(&path, &key, None).err(),
            view_event @ (DrcPanelMsg::TileToggled(_)
            | DrcPanelMsg::TileCleared
            | DrcPanelMsg::GroupToggled(_)
            | DrcPanelMsg::BaseRulesToggled
            | DrcPanelMsg::RowEntered(_)
            | DrcPanelMsg::RowLeft(_)) => {
                apply_drc_view(&mut self.ui_state.validation_panels.drc, view_event);
                None
            }
        };
        if let Some(why) = refused {
            let line = t!(self.ui_state.i18n, ids::DRC_PANEL_REFUSED, "why" => why);
            crate::diagnostics::log_warning(line.into_string());
        }
        self.refresh_panel_ctx();
        follow
    }

    /// Open the waiver form for `key`: the Author is the last one typed,
    /// else the cached user identity (asked for once, AV-13).
    fn start_drc_waiver(&mut self, key: String) -> Task<Message> {
        let author = self
            .interaction_state
            .pcb_drc
            .waive_draft
            .as_ref()
            .map(|d| d.author.clone())
            .filter(|a| !a.trim().is_empty())
            .unwrap_or_else(|| self.ui_state.validation_panels.identity.author());
        self.interaction_state.pcb_drc.waive_draft = Some(DrcWaiveDraft {
            key,
            author,
            ..DrcWaiveDraft::default()
        });
        self.ask_user_identity()
    }

    /// Store the form's waiver; a reason and an author are both required
    /// (AV-13). The form stays open until the waiver is stored.
    fn confirm_drc_waiver(&mut self, path: &Path) -> Option<String> {
        let draft = self.interaction_state.pcb_drc.waive_draft.clone()?;
        if draft.reason.is_empty() || draft.author.trim().is_empty() {
            if let Some(open) = self.interaction_state.pcb_drc.waive_draft.as_mut() {
                open.blank = true;
            }
            return None;
        }
        let text = DrcWaiverText {
            note: draft.reason.text(),
            author: draft.author.clone(),
        };
        let refused = self.set_drc_waiver(path, &draft.key, Some(text)).err();
        if refused.is_none() {
            self.interaction_state.pcb_drc.waive_draft = None;
        }
        refused
    }

    /// Open the Rules tab of the board's project on `domain`, with `rule`
    /// (a written rule's name) selected.
    fn open_rules_at(
        &mut self,
        domain: crate::app::RulesDomain,
        rule: Option<String>,
    ) -> Task<Message> {
        let Some(project_id) = self.rules_project_for(None) else {
            crate::diagnostics::log_info("Rules: no project is open");
            return Task::none();
        };
        self.open_rules_tab(project_id);
        let mut tasks =
            vec![
                self.dispatch_rules_tab_message(crate::app::RulesTabMsg::DomainPicked {
                    project_id,
                    domain,
                }),
            ];
        if let Some(rule) = rule {
            tasks.push(
                self.dispatch_rules_tab_message(crate::app::RulesTabMsg::RowSelected {
                    project_id,
                    rule,
                }),
            );
        }
        Task::batch(tasks)
    }

    /// A group's Rule link: the domain of what its rows break, and the rule
    /// itself selected when it is a written one.
    fn open_drc_rule(&mut self, rule: &str) -> Task<Message> {
        let kind = self
            .pcb_drc_panel()
            .and_then(|panel| panel.rows.into_iter().find(|row| row.rule == rule))
            .map(|row| row.kind);
        let domain = kind.map_or(crate::app::RulesDomain::Clearance, rules_domain_of);
        let name = rule
            .strip_prefix(signex_drc_dsl::table::RULE_ID_PREFIX)
            .map(str::to_string);
        self.open_rules_at(domain, name)
    }

    /// Before the first run: which base rules the project's rule text
    /// writes, and the default clearance the run will use (#87).
    pub(crate) fn fill_drc_idle(
        &self,
        panel: &mut crate::panels::DrcPanel,
        pcb: &signex_types::pcb::PcbBoard,
    ) {
        if let Some((text, _)) = self
            .rules_project_for(None)
            .and_then(|id| self.rules_text_of(id))
        {
            let table = signex_drc_dsl::table::table(&text);
            panel.written_base_rules = crate::panels::BaseRule::ALL
                .into_iter()
                .filter(|rule| rule.is_written_in(&table))
                .collect();
        }
        let set = pcb.setup.as_ref().map_or(0.0, |setup| setup.clearance);
        if !set.is_finite() || set <= 0.0 {
            panel.default_clearance_mm = Some(format!("{:.1}", signex_drc::DEFAULT_CLEARANCE_MM));
            panel.written_clearance = panel
                .written_base_rules
                .contains(&crate::panels::BaseRule::Clearance);
        }
    }

    /// Why `board` cannot be acted on from the DRC results: it must be the
    /// board in view, whose last run the panel lists.
    fn drc_results_board(&self, board: &Path) -> Result<(), String> {
        if self.active_tab_path().as_deref() == Some(board) && self.active_pcb().is_some() {
            Ok(())
        } else {
            Err(format!(
                "{} is not the board in view; the DRC results are the board in view's",
                board.display()
            ))
        }
    }

    /// Select the objects of the last run's violation `key` on `board` and
    /// centre the view on it. The answer names what was selected.
    pub(crate) fn show_drc_violation(&mut self, board: &Path, key: &str) -> Result<Value, String> {
        self.drc_results_board(board)?;
        let done = self
            .interaction_state
            .pcb_drc
            .done_for(board)
            .ok_or("the board has no DRC run yet; run `run_drc` first")?;
        let violation = done
            .report
            .violations
            .iter()
            .find(|v| signex_drc::waiver_key(v) == key)
            .ok_or_else(|| format!("the last DRC run has no violation `{key}`"))?;
        let items = violation.items.clone();
        let rule = signex_drc::rule_id(violation).to_string();
        let (x, y) = (
            pcb_geom::nm_to_mm(violation.at.x),
            pcb_geom::nm_to_mm(violation.at.y),
        );
        self.select_pcb_items(&items);
        let canvas = &mut self.interaction_state.pcb_canvas;
        // A queued fit would land afterwards and undo the centring.
        canvas.pending_fit.set(None);
        canvas.center_on_world(x as f32, y as f32);
        canvas.clear_bg_cache();
        canvas.clear_content_cache();
        Ok(json!({
            "board": board.display().to_string(),
            "id": key,
            "rule": rule,
            "items": items.iter().map(signex_drc::findings::item_id).collect::<Vec<_>>(),
            "at_mm": [x, y],
        }))
    }

    /// Waive the last run's violation `key` on `board` with `waiver`'s note
    /// and author, or take its waiver back (`None`): one board undo step.
    /// The report stays current, since a waiver changes nothing the check
    /// reads. A waiver is refused on a stale run: it would fingerprint
    /// geometry the run never checked (#2307).
    pub(crate) fn set_drc_waiver(
        &mut self,
        board: &Path,
        key: &str,
        waiver: Option<DrcWaiverText>,
    ) -> Result<Value, String> {
        let author = waiver
            .as_ref()
            .map(|w| w.author.trim().to_string())
            .unwrap_or_default();
        let note = waiver.map(|w| w.note);
        let revision = self
            .document_state
            .pcb_revisions
            .get(board)
            .copied()
            .unwrap_or(0);
        let stale = self
            .interaction_state
            .pcb_drc
            .done_for(board)
            .is_some_and(|done| done.is_stale(revision, self.board_net_classes(board)));
        if note.is_some() && stale {
            return Err(
                "the board changed since the last DRC run; run DRC again before waiving".into(),
            );
        }
        let violation = self
            .interaction_state
            .pcb_drc
            .done_for(board)
            .and_then(|done| {
                done.report
                    .violations
                    .iter()
                    .find(|v| signex_drc::waiver_key(v) == key)
            });
        let engine = self
            .document_state
            .board_engine(board)
            .ok_or_else(|| format!("{} is not open", board.display()))?;
        let expected_revision = engine.revision();
        let mut options = engine.board().options.clone();
        let had = options.drc_waivers.len();
        options.drc_waivers.retain(|w| w.key != key);
        let note = note.map(|n| n.trim().to_string());
        match &note {
            Some(note) => {
                let v = violation.ok_or_else(|| {
                    format!("the last DRC run of this board has no violation `{key}`")
                })?;
                // Its objects as they are now: a later change lapses it (#2307).
                options.drc_waivers.push(DrcWaiver {
                    key: key.to_string(),
                    rule: signex_drc::rule_id(v).to_string(),
                    note: note.clone(),
                    fingerprint: signex_drc::fingerprint(v, engine.board()),
                    author: author.clone(),
                });
            }
            None if options.drc_waivers.len() == had => {
                return Err(format!("violation `{key}` is not waived"));
            }
            None => {}
        }
        let label = match note {
            Some(_) => t!(self.ui_state.i18n, ids::DRC_PANEL_WAIVE_UNDO),
            None => t!(self.ui_state.i18n, ids::DRC_PANEL_UNWAIVE_UNDO),
        }
        .into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision,
        };
        let from = self.document_state.pcb_revisions.get(board).copied();
        let command = BoardCommand::SetBoardOptions {
            options: Box::new(options),
        };
        self.execute_board_commands(board, vec![command], Some(step))
            .map_err(|e| e.to_string())?;
        let to = self.document_state.pcb_revisions.get(board).copied();
        if let (Some(from), Some(to)) = (from, to) {
            self.interaction_state
                .pcb_drc
                .carry_revision(board, from, to);
        }
        self.refresh_panel_ctx();
        Ok(json!({
            "board": board.display().to_string(),
            "id": key,
            "waived": note.is_some(),
            "note": note,
            "author": note.as_ref().map(|_| author),
        }))
    }

    /// What [`Self::carry_drc_over_waiver_step`] needs from before an undo
    /// or redo of `board`: its revision and options.
    pub(crate) fn drc_waiver_step_start(&self, board: &Path) -> Option<(u64, BoardOptions)> {
        let from = self.document_state.pcb_revisions.get(board).copied()?;
        let options = self
            .document_state
            .board_engine(board)?
            .board()
            .options
            .clone();
        Some((from, options))
    }

    /// An undo or redo of `board` that changed its waivers and nothing else
    /// of its options keeps the DRC results current, as the waiver step
    /// itself does. A waiver step holds that one options command alone.
    pub(crate) fn carry_drc_over_waiver_step(
        &mut self,
        board: &Path,
        before: Option<(u64, BoardOptions)>,
    ) {
        let Some((from, options)) = before else {
            return;
        };
        let Some(now) = self
            .document_state
            .board_engine(board)
            .map(|e| &e.board().options)
        else {
            return;
        };
        let waivers_only = now.drc_waivers != options.drc_waivers
            && BoardOptions {
                drc_waivers: now.drc_waivers.clone(),
                ..options
            } == *now;
        let to = self.document_state.pcb_revisions.get(board).copied();
        if let (true, Some(to)) = (waivers_only, to) {
            self.interaction_state
                .pcb_drc
                .carry_revision(board, from, to);
        }
    }

    /// Set what the DRC results show of the board in view and answer with
    /// the rows shown.
    pub(crate) fn filter_drc_results(
        &mut self,
        board: &Path,
        filter: DrcFilter,
    ) -> Result<Value, String> {
        self.drc_results_board(board)?;
        self.interaction_state.pcb_drc.filter = filter;
        self.refresh_panel_ctx();
        Ok(self.drc_results_json())
    }

    /// The board in view's DRC results as the panel shows them, filtered.
    pub(crate) fn drc_results_json(&self) -> Value {
        let Some(panel) = self.pcb_drc_panel() else {
            return json!({ "ran": false });
        };
        let rows: Vec<Value> = panel
            .rows
            .iter()
            .filter(|row| panel.filter.shows(row))
            .map(row_json)
            .collect();
        json!({
            "ran": panel.ran,
            "running": panel.running,
            "stale": panel.stale,
            "violations": panel.rows.len(),
            "waived": panel.rows.iter().filter(|r| r.waiver.is_some()).count(),
            "rows": rows,
        })
    }
}

/// A tile, group heading, base-rule heading or row hover of the DRC panel:
/// view state only (#88).
fn apply_drc_view(view: &mut crate::panels::DrcView, msg: DrcPanelMsg) {
    match msg {
        DrcPanelMsg::TileToggled(category) => {
            view.category = (view.category != Some(category)).then_some(category);
        }
        DrcPanelMsg::TileCleared => view.category = None,
        DrcPanelMsg::GroupToggled(rule) => {
            if !view.collapsed.remove(&rule) {
                view.collapsed.insert(rule);
            }
        }
        DrcPanelMsg::BaseRulesToggled => view.base_rules_closed = !view.base_rules_closed,
        DrcPanelMsg::RowEntered(key) => view.hovered = Some(key),
        DrcPanelMsg::RowLeft(key) => {
            if view.hovered.as_deref() == Some(key.as_str()) {
                view.hovered = None;
            }
        }
        // Not view state: `handle_drc_panel` routes these itself.
        DrcPanelMsg::RowPicked(_)
        | DrcPanelMsg::FilterPicked(_)
        | DrcPanelMsg::WaivedShownToggled(_)
        | DrcPanelMsg::RuleOpened(_)
        | DrcPanelMsg::BaseRuleOpened(_)
        | DrcPanelMsg::MessagesShown
        | DrcPanelMsg::WaiveStarted(_)
        | DrcPanelMsg::ReasonActed(_)
        | DrcPanelMsg::AuthorEdited(_)
        | DrcPanelMsg::WaiveConfirmed
        | DrcPanelMsg::WaiveCancelled
        | DrcPanelMsg::Unwaived(_) => {}
    }
}

/// The Rules tab domain that holds the rule a violation kind breaks.
fn rules_domain_of(kind: signex_drc::ViolationKind) -> crate::app::RulesDomain {
    use crate::app::RulesDomain as D;
    use signex_drc::ViolationKind as K;
    match kind {
        K::Clearance | K::Short | K::Unconnected | K::Keepout | K::EdgeClearance => D::Clearance,
        K::TrackWidth | K::ViaDiameter | K::ViaDrill | K::ViaInPad => D::Routing,
        K::HoleSize | K::AnnularRing | K::HoleToHole => D::Holes,
        K::MaskSliver | K::MaskExpansion | K::PasteExpansion => D::Mask,
        K::SilkToPad | K::SilkToSilk | K::CourtyardOverlap => D::Assembly,
        // Keep parts inside is set on the room, not in the Rules tab (#2457);
        // Assembly holds the nearest placement rules.
        K::PartOutsideRoom => D::Assembly,
    }
}

/// One shown row: its key, rule, severity, nets, place, layers and waiver.
fn row_json(row: &DrcRow) -> Value {
    let layers: Vec<String> = row
        .layers
        .iter()
        .map(|layer| pcb_layer_token(PcbLayer::Copper(*layer)))
        .collect();
    json!({
        "id": row.key,
        "rule": row.rule,
        "severity": row.severity,
        "nets": [row.a, row.b],
        "measured_mm": row.measured_mm,
        "required_mm": row.required_mm,
        "at_mm": [row.x_mm, row.y_mm],
        "layers": layers,
        "objects": row.objects,
        "waiver": row.waiver,
    })
}
