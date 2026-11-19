//! The Rules tab view (#2398, #2530), top to bottom: the domain bar, the
//! find and filter bar, the constraint set cards, the notes, then the
//! constraint grid with the selected cell's detail strip under it (or the
//! Clearance matrix, or the text view). Pure: reads the project, its rule
//! text, the tab state and the last DRC run, and emits `RulesTabMsg`s.
//! Every text is a catalog message or the user's own text.

mod detail;
mod grid_cell;
mod grid_icons;
mod grid_preview;
mod grid_toolbar;
mod matrix;
mod presets;
mod sets;
mod table;
mod text_view;

use std::borrow::Cow;
use std::collections::BTreeMap;

use iced::widget::{column, container, row};
use iced::{Element, Length, Widget as _};
use signex_drc::{Effective, RuleResult};
use signex_drc_dsl::table::catalog::Domain;
use signex_drc_dsl::table::{RuleRow, RuleTable};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::project::{DrcRuleSet, NamedRuleSet};
use signex_types::theme::ThemeTokens;
use signex_types::violation::Severity;
use signex_widgets::constraint_grid::{ColumnId, GridMetrics, GridStyle};
use signex_widgets::text;

pub(super) use super::analysis::Checked;
use super::grid::{Cells, RulesGrid};
use super::{RulesOrigin, RulesPanel, RulesTabMsg, RulesTabState};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::styles::{Spacing, TypeScale, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub(super) fn up(msg: RulesTabMsg) -> Message {
    Message::Project(ProjectMsg::Rules(msg))
}

/// What one frame of the tab reads, gathered once. The rows, the run
/// results and "Why this value?" come from the analysis `update` keeps.
pub(super) struct Frame<'a> {
    pub id: ProjectId,
    pub i18n: &'a I18n,
    pub tokens: &'a ThemeTokens,
    pub state: &'a RulesTabState,
    pub src: String,
    pub table: Cow<'a, RuleTable>,
    pub origin: RulesOrigin,
    pub checked: Cow<'a, Checked>,
    pub running: bool,
    /// The selected rule's sample value and the net it is for.
    pub why: Option<&'a (Effective, String)>,
    /// The net classes' names, for the matrix.
    pub classes: Vec<String>,
    /// The rule layer whose severity entries outrank a rule's own word.
    pub layer: Option<&'a DrcRuleSet>,
    /// The project's constraint sets.
    pub sets: BTreeMap<String, NamedRuleSet>,
}

impl<'a> Frame<'a> {
    /// The rule's results in the last run, by rule id.
    /// `None` while the run is stale or the rule changed since it ran: an
    /// old count must not speak for the rule as it is now.
    pub fn result(&self, row: &RuleRow) -> Option<RuleResult> {
        match &*self.checked {
            Checked::Ran {
                stale: false,
                results,
                used,
            } if used.get(&row.name) == row.compiled.as_ref() => {
                Some(results.get(&row.id()).copied().unwrap_or_default())
            }
            _ => None,
        }
    }

    /// The per-rule results of the last run, when there was one and the
    /// board has not changed since.
    pub fn results(&self) -> Option<&BTreeMap<String, RuleResult>> {
        match &*self.checked {
            Checked::Ran {
                stale: false,
                results,
                ..
            } => Some(results),
            _ => None,
        }
    }

    /// The layer's severity entry for `row`: `Some(None)` switches it off.
    fn entry(&self, row: &RuleRow) -> Option<Option<Severity>> {
        self.layer
            .and_then(|layer| signex_drc::written_rule_override(layer, &row.name))
    }

    /// Whether `row` applies: on in the text and not switched off by an
    /// entry.
    pub fn is_on(&self, row: &RuleRow) -> bool {
        row.enabled && self.entry(row) != Some(None)
    }

    /// The severity `row`'s findings get: an entry's, else its own word.
    pub fn severity(&self, row: &RuleRow) -> Severity {
        self.entry(row).flatten().unwrap_or(row.severity)
    }

    pub fn grid(&self) -> &'a RulesGrid {
        &self.state.grid
    }

    /// The cells of the grid, resolved from the rule text and the sets.
    pub fn cells(&self) -> Cells<'_> {
        Cells {
            grid: &self.state.grid,
            table: &self.table,
            sets: &self.sets,
        }
    }

    /// The grid's colours and sizes.
    pub fn grid_style(&self) -> GridStyle {
        GridStyle::from_tokens(self.tokens, TS.body)
    }

    pub fn metrics(&self) -> GridMetrics {
        GridMetrics::for_text(TS.body)
    }
}

/// Where column `column` starts, from the grid's left edge, so a find can
/// scroll it into view.
pub(super) fn column_left(grid: &RulesGrid, column: ColumnId) -> f32 {
    table::column_left(grid, column, GridMetrics::for_text(TS.body))
}

/// The id of the grid's both-ways scrollable.
pub(super) fn grid_scroll_id() -> iced::widget::Id {
    iced::widget::Id::from("rules-constraint-grid".to_string())
}

impl Signex {
    /// The project whose Rules tab the main window shows. A Rules tab is
    /// not undocked (it would take the main window's shortcuts).
    pub(crate) fn rules_tab_in_window(&self, is_main: bool) -> Option<ProjectId> {
        if !is_main {
            return None;
        }
        let docs = &self.document_state;
        docs.tabs
            .get(docs.active_tab)
            .and_then(|tab| tab.kind.as_rules())
    }

    pub(crate) fn view_rules_tab(&self, id: ProjectId) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n: &I18n = &self.ui_state.i18n;
        let state = self
            .document_state
            .project_tabs
            .get(&id)
            .map(|tab| &tab.rules);
        let (Some(state), Some((src, origin))) = (state, self.rules_text_of(id)) else {
            return container(
                text(t!(i18n, ids::RULES_NO_PROJECT))
                    .size(TS.title)
                    .color(tokens::muted_text(tokens)),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .boxed();
        };
        let frame = self.rules_frame(id, i18n, tokens, state, src, origin);
        let domain = state.grid.domain();
        let rules: Element<'_, Message> = match state.text.as_ref() {
            Some(rules_text) => text_view::view(&frame, rules_text),
            None if state.is_matrix_shown() => matrix::view(&frame),
            None if domain == Domain::SignalIntegrity => table::later(&frame),
            None => column![
                container(table::view(&frame)).height(Length::FillPortion(3)),
                container(detail::view(&frame)).height(Length::FillPortion(2)),
            ]
            .spacing(SP.m)
            .height(Length::Fill)
            .boxed(),
        };
        let body: Element<'_, Message> = match state.panel {
            Some(RulesPanel::Presets) => row![
                container(rules).width(Length::FillPortion(3)),
                container(presets::view(&frame, self.preset_profiles()))
                    .width(Length::FillPortion(2)),
            ]
            .spacing(SP.l)
            .height(Length::Fill)
            .boxed(),
            _ => rules,
        };
        let mut page = column![grid_toolbar::domains(&frame)].spacing(SP.s);
        if state.text.is_none() {
            page = page.push(grid_toolbar::find_bar(&frame));
            if state.panel == Some(RulesPanel::Sets) {
                page = page.push(sets::view(&frame));
            }
        }
        page = page.push(notes(&frame)).push(body);
        container(page.height(Length::Fill))
            .padding(SP.l)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .boxed()
    }

    fn rules_frame<'a>(
        &'a self,
        id: ProjectId,
        i18n: &'a I18n,
        tokens: &'a ThemeTokens,
        state: &'a RulesTabState,
        src: String,
        origin: RulesOrigin,
    ) -> Frame<'a> {
        let drc = &self.interaction_state.pcb_drc;
        let board = self.rules_board(id);
        let running = board
            .as_ref()
            .is_some_and(|(path, _)| drc.is_running_for(path));
        // `update` keeps the analysis of the tab in view; before its first
        // `update` the tab reads the same thing directly.
        let analysis = state.analysis.as_ref();
        let table = match analysis {
            Some(analysis) => Cow::Borrowed(&analysis.table),
            None => Cow::Owned(signex_drc_dsl::table::table(&src)),
        };
        let checked = match analysis {
            Some(analysis) => Cow::Borrowed(&analysis.checked),
            None => Cow::Owned(self.rules_checked(id)),
        };
        let classes = self
            .document_state
            .project_by_id(id)
            .map(|p| p.data.net_classes.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default();
        Frame {
            id,
            i18n,
            tokens,
            state,
            table,
            src,
            origin,
            checked,
            running,
            why: analysis.and_then(|analysis| analysis.why.as_ref()),
            classes,
            layer: self.rules_drc_layer(id),
            sets: self.rules_drc_of(id).map(|d| d.sets).unwrap_or_default(),
        }
    }
}

/// The notes over the grid: the info banner while the rules come from
/// this machine's profile, a syntax error, a refused edit.
fn notes<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let mut lines = column![].spacing(SP.xs);
    if frame.origin == RulesOrigin::Machine {
        lines = lines.push(grid_toolbar::info_banner(
            frame,
            t!(i18n, ids::RULES_ORIGIN_MACHINE),
        ));
    }
    if let Some(first) = frame.table.syntax.first() {
        let error = first.render(&frame.src);
        lines = lines.push(
            text(t!(i18n, ids::RULES_SYNTAX_ERROR, "error" => error))
                .size(TS.body)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    if let Some(reason) = frame.state.refused.as_ref() {
        lines = lines.push(
            text(t!(i18n, ids::RULES_REFUSED, "reason" => reason.as_str()))
                .size(TS.body)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    lines.boxed()
}

/// User text, owned: a rule's name, scope or value as written.
pub(super) fn user(text_: impl Into<String>) -> UserText {
    UserText::new(text_.into())
}
