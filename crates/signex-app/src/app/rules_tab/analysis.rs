//! The rule flags and scope counts a Rules tab shows, computed in `update`
//! (never in `view`) when the rule text, the board or the net classes
//! changed since the last time.

use std::collections::BTreeMap;
use std::path::PathBuf;

use signex_drc::FlaggedRule;
use signex_types::net_class::NetClass;

use crate::app::state::ProjectId;
use crate::app::{Signex, TabKind};

/// What the flags and counts were computed from.
#[derive(Debug, Clone, PartialEq)]
struct AnalysisKey {
    text: String,
    board: Option<(PathBuf, u64)>,
    classes: Vec<NetClass>,
}

/// Flags and scope counts of one project's rules on its board.
#[derive(Debug, Clone)]
pub struct RulesAnalysis {
    key: AnalysisKey,
    /// Empty with no open board.
    pub flags: Vec<FlaggedRule>,
    /// Objects each applied rule covers, by rule name.
    pub scope: BTreeMap<String, usize>,
    /// The board was open, so flags and counts are known.
    pub has_board: bool,
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
        let current = self
            .document_state
            .project_tabs
            .get(&id)
            .and_then(|tab| tab.rules.analysis.as_ref())
            .is_some_and(|analysis| analysis.key == key);
        if current {
            return;
        }
        let analysis = self.analyse_rules(id, key);
        self.document_state
            .project_tabs
            .entry(id)
            .or_default()
            .rules
            .analysis = Some(analysis);
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

    fn analyse_rules(&self, id: ProjectId, key: AnalysisKey) -> RulesAnalysis {
        let mut analysis = RulesAnalysis {
            key,
            flags: Vec::new(),
            scope: BTreeMap::new(),
            has_board: false,
        };
        let (Some(rules), Some((_, board)), Some(project)) = (
            self.rules_resolved(id),
            self.rules_board(id),
            self.document_state.project_by_id(id),
        ) else {
            return analysis;
        };
        analysis.has_board = true;
        analysis.flags = rules.rule_flags(board, &project.data.net_classes);
        for rule in rules.user_rules() {
            if let Some(items) = rules.scope_items(board, &rule.name) {
                analysis.scope.insert(rule.name.clone(), items.len());
            }
        }
        analysis
    }
}
