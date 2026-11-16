//! The rules table: a heading row, then one row per rule of the sub-tab
//! that passes the search and the filter. A cell opens for typing on a
//! click; Enter writes it back into its rule.

use iced::widget::{Column, Row, column, container, mouse_area, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_drc::RuleFlag;
use signex_drc_dsl::table::{LimitField, RuleField, RuleKind, RuleRow, scope_text};
use signex_i18n::{MessageId, ids, t};
use signex_types::net_class::glob_matches;
use signex_widgets::{button, checkbox, text, text_input};

use super::{Checked, Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::{RulesDomain, RulesFilter, RulesTabMsg};
use crate::panels::form_look::{checkbox_style, ghost_button};
use crate::styles::tokens;

/// A table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Col {
    On,
    Name,
    Kind,
    Scope,
    Layer,
    Objects,
    Limit(LimitField),
    Severity,
    Actual,
    Margin,
    Violations,
    Flags,
}

impl Col {
    fn of(domain: RulesDomain) -> Vec<Col> {
        let mut cols = vec![Col::On, Col::Name];
        if domain == RulesDomain::Routing {
            cols.push(Col::Kind);
        }
        cols.extend([Col::Scope, Col::Layer]);
        if domain == RulesDomain::Clearance {
            cols.push(Col::Objects);
        }
        for kind in domain.kinds() {
            cols.extend(kind.fields().iter().map(|&f| Col::Limit(f)));
        }
        cols.extend([
            Col::Severity,
            Col::Actual,
            Col::Margin,
            Col::Violations,
            Col::Flags,
        ]);
        cols
    }

    fn heading(self) -> MessageId {
        match self {
            Col::On => ids::RULES_COL_ON,
            Col::Name => ids::RULES_COL_NAME,
            Col::Kind => ids::RULES_COL_KIND,
            Col::Scope => ids::RULES_COL_SCOPE,
            Col::Layer => ids::RULES_COL_LAYER,
            Col::Objects => ids::RULES_COL_OBJECTS,
            Col::Limit(LimitField::MinClearance) => ids::RULES_COL_MIN_CLEARANCE,
            Col::Limit(LimitField::MinWidth) => ids::RULES_COL_MIN_WIDTH,
            Col::Limit(LimitField::MaxWidth) => ids::RULES_COL_MAX_WIDTH,
            Col::Limit(LimitField::MinDiameter) => ids::RULES_COL_MIN_DIAMETER,
            Col::Limit(LimitField::MinDrill) => ids::RULES_COL_MIN_DRILL,
            Col::Severity => ids::RULES_COL_SEVERITY,
            Col::Actual => ids::RULES_COL_ACTUAL,
            Col::Margin => ids::RULES_COL_MARGIN,
            Col::Violations => ids::RULES_COL_VIOLATIONS,
            Col::Flags => ids::RULES_COL_FLAGS,
        }
    }

    fn width(self) -> Length {
        match self {
            Col::On => Length::Fixed(TS.body * 3.0),
            Col::Scope | Col::Flags => Length::FillPortion(4),
            Col::Name => Length::FillPortion(3),
            _ => Length::FillPortion(2),
        }
    }

    /// The rule field the cell edits.
    fn field(self) -> Option<RuleField> {
        match self {
            Col::Name => Some(RuleField::Name),
            Col::Scope => Some(RuleField::Scope),
            Col::Layer => Some(RuleField::Layer),
            Col::Objects => Some(RuleField::Pair),
            Col::Limit(field) => Some(RuleField::Limit(field)),
            Col::Severity => Some(RuleField::Severity),
            _ => None,
        }
    }
}

/// The rows of the sub-tab that pass the search and the filter. A rule
/// whose kind this build does not know shows in every sub-tab.
fn shown<'f>(frame: &'f Frame) -> Vec<&'f RuleRow> {
    let state = frame.state;
    let needle = state.search.trim().to_lowercase();
    frame
        .table
        .rows
        .iter()
        .filter(|row| row.kind.is_none_or(|k| RulesDomain::of(k) == state.domain))
        .filter(|row| {
            needle.is_empty()
                || [
                    row.name.clone(),
                    scope_text(&row.scope),
                    row.layer.clone().unwrap_or_default(),
                    row.message.clone().unwrap_or_default(),
                ]
                .iter()
                .any(|hay| hay.to_lowercase().contains(&needle))
        })
        .filter(|row| match state.filter {
            RulesFilter::All => true,
            RulesFilter::Violations => frame.result(row).is_some_and(|r| r.violations > 0),
            RulesFilter::Flagged => {
                row.text_only || !row.problems.is_empty() || flags_of(frame, row).next().is_some()
            }
            RulesFilter::Disabled => !frame.is_on(row),
        })
        .collect()
}

fn flags_of<'f>(frame: &'f Frame, row: &'f RuleRow) -> impl Iterator<Item = &'f RuleFlag> + 'f {
    frame
        .state
        .analysis
        .iter()
        .flat_map(move |a| a.flags_of(&row.name).map(|f| &f.flag))
}

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let cols = Col::of(frame.state.domain);
    let mut head: Row<Element<'a, Message>> = Row::new().spacing(SP.s).padding([SP.xs, SP.m]);
    for &col in &cols {
        head = head.push(
            text(t!(i18n, col.heading()))
                .size(TS.caption)
                .color(tokens::muted_text(tokens))
                .width(col.width())
                .boxed(),
        );
    }
    let rows = shown(frame);
    let mut body: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
    if rows.is_empty() && !frame.table.is_unparsed() {
        body = body.push(
            container(
                text(t!(i18n, ids::RULES_EMPTY))
                    .size(TS.body)
                    .color(tokens::muted_text(tokens)),
            )
            .padding([SP.m, SP.m])
            .boxed(),
        );
    }
    for row in rows {
        body = body.push(row_view(frame, row, &cols));
        let editing_scope = frame
            .state
            .edit
            .as_ref()
            .is_some_and(|e| e.rule == row.name && e.field == RuleField::Scope);
        if editing_scope {
            body = body.push(scope_help(frame));
        }
    }
    column![head, scrollable(body).height(Length::FillPortion(3))]
        .spacing(SP.xs)
        .height(Length::FillPortion(3))
        .boxed()
}

fn row_view<'a>(frame: &Frame<'a>, row: &RuleRow, cols: &[Col]) -> Element<'a, Message> {
    let (id, tokens) = (frame.id, frame.tokens);
    let mut line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.s)
        .padding([SP.xs, SP.m])
        .align_y(Alignment::Center);
    for &col in cols {
        line = line.push(container(cell(frame, row, col)).width(col.width()).boxed());
    }
    let selected = frame.state.selected.as_deref() == Some(row.name.as_str());
    let tint = tokens::selection_tint(tokens);
    let boxed =
        container(line)
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: selected.then_some(iced::Background::Color(tint)),
                ..Default::default()
            });
    mouse_area(boxed)
        .on_press(up(RulesTabMsg::RowSelected {
            project_id: id,
            rule: row.name.clone(),
        }))
        .boxed()
}

fn plain<'a>(frame: &Frame<'a>, value: String, muted: bool) -> Element<'a, Message> {
    let ink = if muted {
        tokens::muted_text(frame.tokens)
    } else {
        crate::styles::ti(frame.tokens.text)
    };
    text(user(value)).size(TS.body).color(ink).boxed()
}

fn cell<'a>(frame: &Frame<'a>, row: &RuleRow, col: Col) -> Element<'a, Message> {
    let (i18n, id) = (frame.i18n, frame.id);
    match col {
        Col::On => {
            let rule = row.name.clone();
            checkbox(frame.is_on(row))
                .size(TS.body)
                .style(checkbox_style)
                .on_toggle(move |on| {
                    up(RulesTabMsg::EnabledToggled {
                        project_id: id,
                        rule: rule.clone(),
                        on,
                    })
                })
                .boxed()
        }
        Col::Kind => {
            let kind = match row.kind {
                Some(RuleKind::Clearance) => ids::RULES_KIND_CLEARANCE,
                Some(RuleKind::Track) => ids::RULES_KIND_TRACK,
                Some(RuleKind::Via) => ids::RULES_KIND_VIA,
                None => ids::RULES_BADGE_TEXT_ONLY,
            };
            text(t!(i18n, kind)).size(TS.body).boxed()
        }
        Col::Actual | Col::Margin | Col::Violations => outcome(frame, row, col),
        Col::Flags => flags(frame, row),
        _ => field_cell(frame, row, col),
    }
}

/// What an editable cell shows when it is not being typed in.
fn shown_value(frame: &Frame, row: &RuleRow, col: Col) -> Option<String> {
    let i18n = frame.i18n;
    let value = match col {
        Col::Name => row.name.clone(),
        Col::Scope if row.scope == signex_drc::NetScope::Board => {
            t!(i18n, ids::RULES_SCOPE_ALL).into_string()
        }
        Col::Scope => scope_text(&row.scope),
        Col::Layer => row
            .layer
            .clone()
            .unwrap_or_else(|| t!(i18n, ids::RULES_LAYER_ALL).into_string()),
        Col::Objects => row
            .pair
            .as_ref()
            .map(|[a, b]| format!("{a}-{b}"))
            .unwrap_or_else(|| t!(i18n, ids::RULES_OBJECTS_ANY).into_string()),
        Col::Limit(field) => {
            if row.kind != Some(field.kind()) {
                return None;
            }
            row.limit(field)
                .map(|l| l.value.clone())
                .unwrap_or_else(|| t!(i18n, ids::RULES_CELL_EMPTY).into_string())
        }
        Col::Severity => {
            let id = match frame.severity(row) {
                signex_types::violation::Severity::Error => ids::RULES_SEVERITY_ERROR,
                signex_types::violation::Severity::Warning => ids::RULES_SEVERITY_WARNING,
                signex_types::violation::Severity::Info => ids::RULES_SEVERITY_INFO,
            };
            t!(i18n, id).into_string()
        }
        _ => return None,
    };
    Some(value)
}

fn field_cell<'a>(frame: &Frame<'a>, row: &RuleRow, col: Col) -> Element<'a, Message> {
    let (id, i18n) = (frame.id, frame.i18n);
    let Some(field) = col.field() else {
        return plain(frame, String::new(), true);
    };
    let editing = frame
        .state
        .edit
        .as_ref()
        .filter(|e| e.rule == row.name && e.field == field);
    if let Some(edit) = editing {
        let input = text_input(t!(i18n, ids::RULES_CELL_EMPTY), edit.text.as_str())
            .on_input(move |text| {
                up(RulesTabMsg::CellEdited {
                    project_id: id,
                    text,
                })
            })
            .on_submit(up(RulesTabMsg::CellCommitted { project_id: id }))
            .size(TS.body)
            .padding(SP.xs)
            .width(Length::Fill)
            .style(crate::panels::form_look::text_input_style);
        let cancel = button(text(t!(i18n, ids::RULES_CELL_CANCEL)).size(TS.caption))
            .padding([0.0, SP.xs])
            .style(ghost_button)
            .on_press(up(RulesTabMsg::CellCancelled { project_id: id }));
        return row![input, cancel]
            .spacing(SP.xs)
            .align_y(Alignment::Center)
            .boxed();
    }
    let Some(value) = shown_value(frame, row, col) else {
        return plain(frame, t!(i18n, ids::RULES_CELL_EMPTY).into_string(), true);
    };
    if row.text_only {
        return plain(frame, value, false);
    }
    button(text(user(value)).size(TS.body))
        .padding([0.0, SP.xs])
        .style(ghost_button)
        .on_press(up(RulesTabMsg::CellOpened {
            project_id: id,
            rule: row.name.clone(),
            field,
        }))
        .boxed()
}

/// Actual, Margin and Violations: from the last run, or why they are
/// blank.
fn outcome<'a>(frame: &Frame<'a>, row: &RuleRow, col: Col) -> Element<'a, Message> {
    let i18n = frame.i18n;
    if let Some(blank) = blank_reason(frame, row) {
        let shown = match col {
            Col::Actual => t!(i18n, blank).into_string(),
            _ => t!(i18n, ids::RULES_CELL_EMPTY).into_string(),
        };
        return plain(frame, shown, true);
    }
    let result = frame.result(row).unwrap_or_default();
    let mm = |nm: i64| format!("{:.3}", pcb_geom::nm_to_mm(nm));
    let value = match (col, result.worst) {
        (Col::Violations, _) => result.violations.to_string(),
        (Col::Actual, Some((measured, _))) => mm(measured),
        (Col::Margin, Some(_)) => result.margin_nm().map(mm).unwrap_or_default(),
        (Col::Actual, None) => t!(i18n, ids::RULES_BLANK_PASSES).into_string(),
        _ => t!(i18n, ids::RULES_CELL_EMPTY).into_string(),
    };
    if result.violations > 0 {
        return text(user(value))
            .size(TS.body)
            .color(tokens::danger(frame.tokens))
            .boxed();
    }
    plain(frame, value, result.worst.is_none())
}

/// Why a rule's Actual and Margin are blank; `None` when the last run
/// speaks for it.
pub(super) fn blank_reason(frame: &Frame, row: &RuleRow) -> Option<MessageId> {
    if !frame.is_on(row) {
        return Some(ids::RULES_BLANK_OFF);
    }
    match &*frame.checked {
        Checked::NoBoard => Some(ids::RULES_BLANK_NO_BOARD),
        Checked::NotChecked => Some(ids::RULES_BLANK_NOT_CHECKED),
        Checked::Ran { stale, used, .. } => {
            if *stale || used.get(&row.name) != row.compiled.as_ref() {
                return Some(ids::RULES_BLANK_STALE);
            }
            let scope = frame
                .state
                .analysis
                .as_ref()
                .and_then(|a| a.scope.get(&row.name));
            (scope == Some(&0)).then_some(ids::RULES_BLANK_NO_OBJECTS)
        }
    }
}

/// A flag, problem or text-only badge, worded.
pub(super) fn flag_text(frame: &Frame, flag: &RuleFlag) -> String {
    let i18n = frame.i18n;
    let mm = |nm: i64| format!("{}", pcb_geom::nm_to_mm(nm));
    match flag {
        RuleFlag::Shadowed { by } => t!(i18n, ids::RULES_FLAG_SHADOWED, "by" => by.as_str()),
        RuleFlag::Conflict { with } => t!(i18n, ids::RULES_FLAG_CONFLICT, "with" => with.as_str()),
        RuleFlag::LowersClass {
            class,
            from_nm,
            to_nm,
        } => t!(
            i18n,
            ids::RULES_FLAG_LOWERS,
            "class" => class.as_str(),
            "from" => mm(*from_nm),
            "to" => mm(*to_nm)
        ),
        RuleFlag::NoNets => t!(i18n, ids::RULES_FLAG_NO_NETS),
    }
    .into_string()
}

fn flags<'a>(frame: &Frame<'a>, row: &RuleRow) -> Element<'a, Message> {
    let i18n = frame.i18n;
    let mut words: Vec<String> = flags_of(frame, row).map(|f| flag_text(frame, f)).collect();
    if row.text_only {
        words.push(t!(i18n, ids::RULES_BADGE_TEXT_ONLY).into_string());
    }
    if !row.problems.is_empty() {
        let count = row.problems.len();
        words.push(t!(i18n, ids::RULES_BADGE_PROBLEMS, "count" => count).into_string());
    }
    let ink = if words.is_empty() {
        tokens::muted_text(frame.tokens)
    } else {
        tokens::danger(frame.tokens)
    };
    text(user(words.join("; ")))
        .size(TS.caption)
        .color(ink)
        .boxed()
}

/// Under the scope cell being typed in: what it can say, picks from the
/// board's nets and the project's classes, and what is wrong with it.
fn scope_help<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let typed = frame
        .state
        .edit
        .as_ref()
        .map(|e| e.text.clone())
        .unwrap_or_default();
    let mut help = Column::new().spacing(SP.xs).padding([SP.xs, SP.xl]);
    help = help.push(
        text(t!(i18n, ids::RULES_SCOPE_HINT))
            .size(TS.caption)
            .color(tokens::muted_text(tokens))
            .boxed(),
    );
    let nets: Vec<String> = frame
        .board
        .map(|b| b.nets.iter().map(|n| n.name.clone()).collect())
        .unwrap_or_default();
    match signex_drc_dsl::table::parse_scope(&typed) {
        Err(reason) => {
            help = help.push(
                text(user(reason))
                    .size(TS.caption)
                    .color(tokens::danger(tokens))
                    .boxed(),
            );
        }
        Ok(scope) => {
            if let Some(problem) = scope_problem(frame, &scope, &nets) {
                help = help.push(
                    text(problem)
                        .size(TS.caption)
                        .color(tokens::danger(tokens))
                        .boxed(),
                );
            }
        }
    }
    let needle = typed.trim().to_lowercase();
    let candidates = frame
        .classes
        .iter()
        .map(|c| format!("class == \"{c}\""))
        .chain(nets.iter().map(|n| format!("net == \"{n}\"")))
        .filter(|c| needle.is_empty() || c.to_lowercase().contains(&needle))
        .take(8);
    let mut picks: Row<Element<'a, Message>> = Row::new().spacing(SP.xs);
    for candidate in candidates {
        picks = picks.push(
            button(text(user(candidate.clone())).size(TS.caption))
                .padding([0.0, SP.xs])
                .style(crate::styles::outline_button(tokens))
                .on_press(up(RulesTabMsg::CellEdited {
                    project_id: id,
                    text: candidate,
                }))
                .boxed(),
        );
    }
    help.push(picks.boxed()).boxed()
}

/// A scope that parses but takes in no net of the board, worded.
fn scope_problem(
    frame: &Frame,
    scope: &signex_drc::NetScope,
    nets: &[String],
) -> Option<signex_i18n::Localized> {
    let i18n = frame.i18n;
    frame.board?;
    match scope {
        signex_drc::NetScope::Net(net) if !nets.contains(net) => {
            Some(t!(i18n, ids::RULES_SCOPE_NO_NET, "net" => net.as_str()))
        }
        signex_drc::NetScope::Class(class) if !frame.classes.contains(class) => {
            Some(t!(i18n, ids::RULES_SCOPE_NO_CLASS, "class" => class.as_str()))
        }
        signex_drc::NetScope::Pattern(glob) if !nets.iter().any(|n| glob_matches(glob, n)) => {
            Some(t!(i18n, ids::RULES_SCOPE_NO_MATCH, "pattern" => glob.as_str()))
        }
        _ => None,
    }
}
