//! What a Rules tab shows that takes work to find: the rows of the stored
//! text, the rule flags and scope counts, the last DRC run's per-rule
//! results and "Why this value?" for the selected rule. Computed in
//! `update` (never in `view`), each part again only when what it was
//! computed from changed.

use std::collections::BTreeMap;
use std::path::PathBuf;

use signex_drc::{DrcRules, Effective, FlaggedRule, RuleResult, UserRule};
use signex_drc_dsl::table::{RuleRow, RuleTable};
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;

use crate::app::state::ProjectId;
use crate::app::{Signex, TabKind};

/// What the rows, flags and counts were computed from.
#[derive(Debug, Clone, PartialEq)]
struct AnalysisKey {
    text: String,
    board: Option<(PathBuf, u64)>,
    classes: Vec<NetClass>,
}

/// What the run results and "Why this value?" were read from, besides
/// [`AnalysisKey`]: the DRC run state and the selected rule.
#[derive(Debug, Clone, PartialEq)]
struct ShownKey {
    drc: (u64, bool),
    selected: Option<String>,
}

/// What the last DRC run of the project's board says, for the Actual,
/// Margin and Violations cells.
#[derive(Debug, Clone)]
pub(crate) enum Checked {
    NoBoard,
    NotRun,
    Ran {
        /// The board changed since.
        stale: bool,
        results: BTreeMap<String, RuleResult>,
        /// The written rules the run applied, by name.
        used: BTreeMap<String, UserRule>,
    },
}

/// One project's rules as its Rules tab shows them.
#[derive(Debug, Clone)]
pub struct RulesAnalysis {
    key: AnalysisKey,
    shown_key: Option<ShownKey>,
    /// The rows of the stored rule text.
    pub table: RuleTable,
    /// The rules resolved against the open board.
    resolved: Option<DrcRules>,
    /// Empty with no open board.
    pub flags: Vec<FlaggedRule>,
    /// Objects each applied rule covers, by rule name.
    pub scope: BTreeMap<String, usize>,
    /// The board was open, so flags and counts are known.
    pub has_board: bool,
    pub(crate) checked: Checked,
    /// The selected rule's sample value and the net it is for.
    pub why: Option<(Effective, String)>,
    /// What each class-matrix cell falls back to without its rule.
    pub(crate) defaults: super::matrix::MatrixDefaults,
}

impl RulesAnalysis {
    /// The flags on rule `name`.
    pub fn flags_of<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a FlaggedRule> + 'a {
        self.flags.iter().filter(move |f| f.rule == name)
    }
}

impl Signex {
    /// Refresh the analysis of the Rules tab in view when what it was
    /// computed from changed. Called once per `update`.
    pub(crate) fn sync_rules_analysis(&mut self) {
        let docs = &self.document_state;
        let Some(TabKind::Rules(id)) = docs.tabs.get(docs.active_tab).map(|t| t.kind.clone())
        else {
            return;
        };
        let Some(key) = self.rules_analysis_key(id) else {
            return;
        };
        let shown_key = self.rules_shown_key(id);
        let kept = self.rules_tab_mut(id).analysis.take();
        let text_moved = kept.as_ref().is_some_and(|kept| kept.key.text != key.text);
        let mut analysis = match kept {
            Some(analysis) if analysis.key == key => analysis,
            _ => self.analyse_rules(id, key),
        };
        // A text view nobody typed in follows the stored text (an agent's
        // edit, an ERC dialog undo); typed text is kept.
        if text_moved
            && let Some(view) = self.rules_tab_mut(id).text.as_mut()
            && !view.edited
        {
            *view = super::RulesText::new(&analysis.key.text);
        }
        if analysis.shown_key.as_ref() != Some(&shown_key) {
            analysis.checked = self.rules_checked(id);
            analysis.why = self.rules_why(id, &analysis, shown_key.selected.as_deref());
            analysis.shown_key = Some(shown_key);
        }
        self.rules_tab_mut(id).analysis = Some(analysis);
    }

    fn rules_analysis_key(&self, id: ProjectId) -> Option<AnalysisKey> {
        let (text, _) = self.rules_text_of(id)?;
        let project = self.document_state.project_by_id(id)?;
        let board = self.rules_board(id).map(|(path, _)| {
            let revision = self
                .document_state
                .pcb_revisions
                .get(&path)
                .copied()
                .unwrap_or(0);
            (path, revision)
        });
        Some(AnalysisKey {
            text,
            board,
            classes: project.data.net_classes.clone(),
        })
    }

    fn rules_shown_key(&self, id: ProjectId) -> ShownKey {
        let drc = &self.interaction_state.pcb_drc;
        let running = self
            .rules_board_path(id)
            .is_some_and(|path| drc.is_running_for(&path));
        ShownKey {
            drc: (drc.generation(), running),
            selected: self
                .document_state
                .project_tabs
                .get(&id)
                .and_then(|tab| tab.rules.selected.clone()),
        }
    }

    fn analyse_rules(&self, id: ProjectId, key: AnalysisKey) -> RulesAnalysis {
        let key_classes = key.classes.clone();
        let mut analysis = RulesAnalysis {
            table: signex_drc_dsl::table::table(&key.text),
            key,
            shown_key: None,
            resolved: None,
            flags: Vec::new(),
            scope: BTreeMap::new(),
            has_board: false,
            checked: Checked::NoBoard,
            why: None,
            defaults: super::matrix::class_defaults(&key_classes),
        };
        let (Ok(rules), Some((_, board)), Some(project)) = (
            self.rules_resolved(id),
            self.rules_board(id),
            self.document_state.project_by_id(id),
        ) else {
            return analysis;
        };
        analysis.has_board = true;
        analysis.flags = rules.rule_flags(board, &project.data.net_classes);
        analysis.defaults = super::matrix::board_defaults(&rules, board, &project.data.net_classes);
        for rule in rules.user_rules() {
            if let Some(items) = rules.scope_items(board, &rule.name) {
                analysis.scope.insert(rule.name.clone(), items.len());
            }
        }
        analysis.resolved = Some(rules);
        analysis
    }

    /// What the last DRC run of `id`'s board says.
    pub(crate) fn rules_checked(&self, id: ProjectId) -> Checked {
        let Some((path, _)) = self.rules_board(id) else {
            return Checked::NoBoard;
        };
        let Some(done) = self.interaction_state.pcb_drc.done_for(&path) else {
            return Checked::NotRun;
        };
        let revision = self
            .document_state
            .pcb_revisions
            .get(&path)
            .copied()
            .unwrap_or(0);
        Checked::Ran {
            stale: done.is_stale(revision, self.board_net_classes(&path)),
            results: done.report.rule_results(),
            used: done
                .report
                .rules
                .user_rules()
                .map(|r| (r.name.clone(), r.clone()))
                .collect(),
        }
    }

    /// "Why this value?" for the selected rule `name`.
    fn rules_why(
        &self,
        id: ProjectId,
        analysis: &RulesAnalysis,
        name: Option<&str>,
    ) -> Option<(Effective, String)> {
        let row = analysis.table.row(name?)?;
        let rules = analysis.resolved.as_ref()?;
        let (_, board) = self.rules_board(id)?;
        sample(rules, board, row)
    }
}

/// The value that governs the first net `row` takes in, on the layers and
/// objects it names, for its first limit, and that net's name. `None` for
/// an off or invalid rule or a rule with no net on the board.
fn sample(rules: &DrcRules, board: &PcbBoard, row: &RuleRow) -> Option<(Effective, String)> {
    let compiled = row.compiled.as_ref().filter(|_| row.applied)?;
    let field = row.limits.first()?.field;
    // A class-pair rule (#2398) is resolved with a net of the other class
    // across.
    let (net, other) = rules.first_pair_of(board, &row.name)?;
    let objects = compiled
        .pair
        .map_or([None, None], |[a, b]| [Some(a), Some(b)]);
    let layers = compiled.layers.unwrap_or(u32::MAX);
    let effective = rules.effective_between(field.limit_kind(), net, other, layers, objects);
    let net_name = board
        .nets
        .iter()
        .find(|n| n.number == net)
        .map(|n| n.name.clone())
        .unwrap_or_default();
    Some((effective, net_name))
}
