//! The Rules tab view (#2398): toolbar, notes, then the table and the
//! selected rule's detail, or the text view. Pure: reads the project, its
//! rule text, the tab state and the last DRC run, and emits
//! `RulesTabMsg`s. Every text is a catalog message or the user's own text.

mod detail;
mod table;
mod text_view;

use std::borrow::Cow;

use iced::widget::{Row, Space, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_drc::{Effective, RuleResult};
use signex_drc_dsl::table::{RuleKind, RuleRow, RuleTable};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::pcb::PcbBoard;
use signex_types::project::DrcRuleSet;
use signex_types::theme::ThemeTokens;
use signex_types::violation::Severity;
use signex_widgets::{button, text, text_input};

pub(super) use super::analysis::Checked;
use super::{RulesDomain, RulesFilter, RulesOrigin, RulesTabMsg, RulesTabState};
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
    pub board: Option<&'a PcbBoard>,
    /// The selected rule's sample value and the net it is for.
    pub why: Option<&'a (Effective, String)>,
    /// The net classes' names, for the scope cell's suggestions.
    pub classes: Vec<String>,
    /// The rule layer whose severity entries outrank a rule's own word.
    pub layer: Option<&'a DrcRuleSet>,
}

impl Frame<'_> {
    /// The rule's results in the last run, by rule id.
    pub fn result(&self, row: &RuleRow) -> Option<RuleResult> {
        match &*self.checked {
            Checked::Ran { results, .. } => {
                Some(results.get(&row.id()).copied().unwrap_or_default())
            }
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
        let body: Element<'_, Message> = match state.text.as_ref() {
            Some(rules_text) => text_view::view(&frame, rules_text),
            None => column![table::view(&frame), detail::view(&frame)]
                .spacing(SP.l)
                .height(Length::Fill)
                .boxed(),
        };
        container(
            column![toolbar(&frame), notes(&frame), body]
                .spacing(SP.m)
                .height(Length::Fill),
        )
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
            board: board.map(|(_, board)| board),
            why: analysis.and_then(|analysis| analysis.why.as_ref()),
            classes,
            layer: self.rules_drc_layer(id),
        }
    }
}

/// A toolbar button: accent when `on`.
fn pick<'a>(
    label: Localized,
    on: bool,
    msg: Message,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let face = button(text(label).size(TS.label))
        .padding([SP.xs, SP.m])
        .on_press(msg);
    if on {
        face.style(crate::styles::accent_button(tokens)).boxed()
    } else {
        face.style(crate::styles::subtle_button(tokens)).boxed()
    }
}

/// The findings of `domain`'s rules in the last run.
fn domain_findings(frame: &Frame, domain: RulesDomain) -> usize {
    frame
        .table
        .rows
        .iter()
        .filter(|row| row.kind.map(RulesDomain::of) == Some(domain))
        .filter_map(|row| frame.result(row))
        .map(|r| r.violations)
        .sum()
}

fn toolbar<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens, state, id) = (frame.i18n, frame.tokens, frame.state, frame.id);
    let mut bar: Row<'a, Message> = Row::new().spacing(SP.s).align_y(Alignment::Center);
    let in_table = state.text.is_none();
    for domain in RulesDomain::ALL {
        let label = match domain {
            RulesDomain::Clearance => ids::RULES_DOMAIN_CLEARANCE,
            RulesDomain::Routing => ids::RULES_DOMAIN_ROUTING,
        };
        let count = domain_findings(frame, domain);
        let face = row![
            text(t!(i18n, label)).size(TS.label),
            text(t!(i18n, ids::RULES_DOMAIN_COUNT, "count" => count))
                .size(TS.caption)
                .color(if count > 0 {
                    tokens::danger(tokens)
                } else {
                    tokens::muted_text(tokens)
                }),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center);
        let tab = button(face)
            .padding([SP.xs, SP.m])
            .on_press(up(RulesTabMsg::DomainPicked {
                project_id: id,
                domain,
            }));
        bar = if domain == state.domain && in_table {
            bar.push(tab.style(crate::styles::accent_button(tokens)))
        } else {
            bar.push(tab.style(crate::styles::subtle_button(tokens)))
        };
    }
    bar = bar.push(Space::new().width(SP.l));
    if in_table {
        bar = bar.push(
            text_input(
                t!(i18n, ids::RULES_SEARCH_PLACEHOLDER),
                state.search.as_str(),
            )
            .on_input(move |text| {
                up(RulesTabMsg::SearchEdited {
                    project_id: id,
                    text,
                })
            })
            .size(TS.label)
            .padding(SP.s)
            .width(Length::Fixed(TS.body * 22.0))
            .style(crate::panels::form_look::text_input_style),
        );
        for filter in RulesFilter::ALL {
            let label = match filter {
                RulesFilter::All => ids::RULES_FILTER_ALL,
                RulesFilter::Violations => ids::RULES_FILTER_VIOLATIONS,
                RulesFilter::Flagged => ids::RULES_FILTER_FLAGGED,
                RulesFilter::Disabled => ids::RULES_FILTER_DISABLED,
            };
            let msg = up(RulesTabMsg::FilterPicked {
                project_id: id,
                filter,
            });
            bar = bar.push(pick(t!(i18n, label), filter == state.filter, msg, tokens));
        }
    }
    bar = bar.push(Space::new().width(Length::Fill));
    if in_table && !frame.table.is_unparsed() {
        for &kind in state.domain.kinds() {
            let label = match kind {
                RuleKind::Clearance => ids::RULES_ADD_CLEARANCE,
                RuleKind::Track => ids::RULES_ADD_TRACK,
                RuleKind::Via => ids::RULES_ADD_VIA,
            };
            let msg = up(RulesTabMsg::RuleAdded {
                project_id: id,
                kind,
            });
            bar = bar.push(pick(t!(i18n, label), false, msg, tokens));
        }
    }
    let drc_label = if frame.running {
        ids::RULES_DRC_RUNNING
    } else {
        ids::RULES_RUN_DRC
    };
    let can_run = !frame.running && !matches!(*frame.checked, Checked::NoBoard);
    bar = bar.push(
        button(text(t!(i18n, drc_label)).size(TS.label))
            .padding([SP.xs, SP.m])
            .style(crate::styles::outline_button(tokens))
            .on_press_maybe(can_run.then(|| up(RulesTabMsg::DrcRequested { project_id: id }))),
    );
    let (view_label, to_text) = if in_table {
        (ids::RULES_VIEW_TEXT, true)
    } else {
        (ids::RULES_VIEW_TABLE, false)
    };
    let toggle = up(RulesTabMsg::TextViewToggled {
        project_id: id,
        on: to_text,
    });
    bar = bar.push(pick(t!(i18n, view_label), false, toggle, tokens));
    bar.boxed()
}

/// One line per note: where the rules come from, a syntax error, a
/// refused edit.
fn notes<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let mut lines = column![].spacing(SP.xs);
    if frame.origin == RulesOrigin::Machine {
        lines = lines.push(
            text(t!(i18n, ids::RULES_ORIGIN_MACHINE))
                .size(TS.body)
                .color(tokens::muted_text(tokens)),
        );
    }
    if let Some(first) = frame.table.syntax.first() {
        let error = first.render(&frame.src);
        lines = lines.push(
            text(t!(i18n, ids::RULES_SYNTAX_ERROR, "error" => error))
                .size(TS.body)
                .color(tokens::danger(tokens)),
        );
    }
    if let Some(reason) = frame.state.refused.as_ref() {
        lines = lines.push(
            text(t!(i18n, ids::RULES_REFUSED, "reason" => reason.as_str()))
                .size(TS.body)
                .color(tokens::danger(tokens)),
        );
    }
    lines.boxed()
}

/// User text, owned: a rule's name, scope or value as written.
pub(super) fn user(text_: impl Into<String>) -> UserText {
    UserText::new(text_.into())
}
