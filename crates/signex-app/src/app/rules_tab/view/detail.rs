//! The detail strip under the grid, in three columns: the selected cell's
//! drawing; the rule that governs it (its text, name and severity, what
//! the compiler and the flags say, the objects it covers with Jump); and
//! "Which value wins, and why", walking the row, its constraint set, its
//! parents and the default, then the board's own resolution for a net.

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_drc::{RuleFlag, Source, Verdict};
use signex_drc_dsl::table::grid::{self as cells, CellSource, GridScope};
use signex_drc_dsl::table::{RuleField, RuleRow};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::{button, checkbox, text, text_input};

use super::grid_cell::scope_name;
use super::grid_icons::{Icon, icon};
use super::{Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::RulesTabMsg;
use crate::app::rules_tab::grid::{CellShow, RowRef};
use crate::app::rules_tab::json::layer_name;
use crate::app::rules_tab::search::{column_label, group_label};
use crate::panels::form_look::checkbox_style;
use crate::styles::{ti, tokens};

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let strip = row![
        container(super::grid_preview::view(frame)).width(Length::FillPortion(4)),
        container(scrollable(rule(frame)).height(Length::Fill)).width(Length::FillPortion(3)),
        container(scrollable(why(frame)).height(Length::Fill)).width(Length::FillPortion(3)),
    ]
    .spacing(SP.l)
    .height(Length::Fill);
    container(strip)
        .padding(SP.m)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::card(frame.tokens))
        .boxed()
}

pub(super) fn section<'a>(frame: &Frame<'a>, id: MessageId) -> Element<'a, Message> {
    text(t!(frame.i18n, id))
        .size(TS.caption)
        .color(tokens::muted_text(frame.tokens))
        .boxed()
}

/// The rule whose detail is shown.
fn selected_rule<'f>(frame: &'f Frame) -> Option<&'f RuleRow> {
    frame
        .state
        .selected
        .as_deref()
        .and_then(|name| frame.table.row(name))
}

/// The middle column: the governing rule.
fn rule<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let mut body: Column<Element<'a, Message>> = Column::new().spacing(SP.s);
    body = body.push(section(frame, ids::RULES_DETAIL_TEXT));
    let Some(row) = selected_rule(frame) else {
        return body
            .push(
                text(t!(i18n, ids::RULES_DETAIL_NO_RULE))
                    .size(TS.body)
                    .color(tokens::muted_text(tokens))
                    .boxed(),
            )
            .boxed();
    };
    body = body.push(rule_line(frame, row));
    if let Some(source) = super::schematic_source::caption(frame, row) {
        body = body.push(source);
    }
    body = body.push(
        text(user(row.text(&frame.src).to_string()))
            .size(TS.body)
            .font(crate::fonts::iced_font_for_family(
                crate::fonts::CHROME_MONO_FONT,
            ))
            .boxed(),
    );
    if !row.problems.is_empty() {
        body = body.push(section(frame, ids::RULES_DETAIL_PROBLEMS));
        for problem in &row.problems {
            let (line, column) = problem.line_col(&frame.src);
            body = body.push(
                text(t!(
                    i18n,
                    ids::RULES_TEXT_PROBLEM,
                    "line" => line,
                    "column" => column,
                    "message" => problem.message.as_str()
                ))
                .size(TS.body)
                .color(tokens::danger(tokens))
                .boxed(),
            );
        }
    }
    let flags: Vec<String> = frame
        .state
        .analysis
        .iter()
        .flat_map(|a| a.flags_of(&row.name))
        .map(|f| flag_text(frame, &f.flag))
        .collect();
    if !flags.is_empty() {
        body = body.push(section(frame, ids::RULES_DETAIL_FLAGS));
        for flag in flags {
            body = body.push(
                text(user(flag))
                    .size(TS.body)
                    .color(tokens::danger(tokens))
                    .boxed(),
            );
        }
    }
    body = body.push(section(frame, ids::RULES_DETAIL_VIOLATIONS));
    body = body.push(violations(frame, row));
    body.boxed()
}

/// A flag on a rule, worded.
fn flag_text(frame: &Frame, flag: &RuleFlag) -> String {
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
        RuleFlag::BelowFabFloor {
            floor,
            from_nm,
            to_nm,
            ..
        } => t!(
            i18n,
            ids::RULES_FLAG_BELOW_FAB,
            "floor" => floor.as_str(),
            "from" => mm(*from_nm),
            "to" => mm(*to_nm)
        ),
    }
    .into_string()
}

/// The rule's On box, its name and severity (each typed in on a click),
/// and Remove.
fn rule_line<'a>(frame: &Frame<'a>, row: &RuleRow) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let rule = row.name.clone();
    let on = checkbox(frame.is_on(row))
        .size(TS.body)
        .style(checkbox_style)
        .on_toggle(move |on| {
            up(RulesTabMsg::EnabledToggled {
                project_id: id,
                rule: rule.clone(),
                on,
            })
        });
    let severity = match frame.severity(row) {
        signex_types::violation::Severity::Error => ids::RULES_SEVERITY_ERROR,
        signex_types::violation::Severity::Warning => ids::RULES_SEVERITY_WARNING,
        signex_types::violation::Severity::Info => ids::RULES_SEVERITY_INFO,
    };
    let remove = button(text(t!(i18n, ids::RULES_REMOVE)).size(TS.caption))
        .padding([0.0, SP.s])
        .style(crate::styles::danger_button(tokens))
        .on_press(up(RulesTabMsg::RuleRemoved {
            project_id: id,
            rule: row.name.clone(),
        }));
    row![
        on,
        field(frame, row, RuleField::Name, row.name.clone()),
        field(
            frame,
            row,
            RuleField::Severity,
            t!(i18n, severity).into_string()
        ),
        Space::new().width(Length::Fill),
        remove,
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// One field of the rule: its value as a button, or the box it is typed
/// in (Enter writes it, Cancel drops it).
fn field<'a>(
    frame: &Frame<'a>,
    row: &RuleRow,
    field: RuleField,
    shown: String,
) -> Element<'a, Message> {
    let (i18n, id) = (frame.i18n, frame.id);
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
            .style(crate::styles::ghost_button(frame.tokens))
            .on_press(up(RulesTabMsg::CellCancelled { project_id: id }));
        return row![input, cancel]
            .spacing(SP.xs)
            .align_y(Alignment::Center)
            .boxed();
    }
    let face = button(text(user(shown)).size(TS.body))
        .padding([0.0, SP.xs])
        .style(crate::styles::ghost_button(frame.tokens));
    if row.text_only {
        return face.boxed();
    }
    face.on_press(up(RulesTabMsg::CellOpened {
        project_id: id,
        rule: row.name.clone(),
        field,
    }))
    .boxed()
}

/// The rule's findings in the last run, and the objects it covers with
/// Jump (show them on the board).
fn violations<'a>(frame: &Frame<'a>, row: &RuleRow) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let reason = frame.findings_reason(row);
    let findings = frame
        .result(row)
        .map(|r| r.violations)
        .filter(|_| reason.is_none());
    let line = match reason {
        Some(reason) => t!(i18n, reason),
        None => t!(i18n, ids::RULES_DETAIL_FINDINGS, "count" => findings.unwrap_or_default()),
    };
    let ink = if findings.is_some_and(|n| n > 0) {
        tokens::danger(tokens)
    } else {
        tokens::muted_text(tokens)
    };
    let count = frame
        .state
        .analysis
        .as_ref()
        .filter(|a| a.has_board)
        .and_then(|a| a.scope.get(&row.name).copied());
    let Some(count) = count else {
        return column![
            text(line).size(TS.body).color(ink),
            text(t!(i18n, ids::RULES_DETAIL_SCOPE_UNKNOWN))
                .size(TS.body)
                .color(tokens::muted_text(tokens)),
        ]
        .spacing(SP.xs)
        .boxed();
    };
    let jump = button(
        row![
            icon(Icon::Target, TS.body, ti(tokens.accent)),
            text(t!(i18n, ids::RULES_DETAIL_JUMP)).size(TS.label),
        ]
        .spacing(SP.xs)
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .style(crate::styles::outline_button(tokens))
    .on_press_maybe((count > 0).then(|| {
        up(RulesTabMsg::ScopeShown {
            project_id: id,
            rule: row.name.clone(),
        })
    }));
    column![
        text(line).size(TS.body).color(ink),
        row![
            text(t!(i18n, ids::RULES_DETAIL_SCOPE, "count" => count)).size(TS.body),
            jump,
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// "Which value wins, and why" for the selected cell, then the board's
/// resolution for a net the governing rule takes in.
fn why<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let mut out: Column<Element<'a, Message>> =
        column![section(frame, ids::RULES_WHY_WINS)].spacing(SP.xs);
    if let Some(chain) = chain(frame) {
        out = out.push(chain);
    }
    out.push(board_why(frame)).boxed()
}

/// The hierarchy walk for the selected cell: the row (its own value or
/// its constraint set's), each parent, the default; the winner marked.
fn chain<'a>(frame: &Frame<'a>) -> Option<Element<'a, Message>> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let grid = frame.grid();
    let at = grid.view.selected()?;
    let row = grid.row(at.row)?.clone();
    let column = grid.column(at.column)?;
    let field = column.field?;
    let group = grid
        .groups()
        .iter()
        .find(|g| g.columns.iter().any(|c| c.id == column.id))?;
    let show = frame.cells().show(at);
    let (value, how) = match &show {
        CellShow::Value {
            written, source, ..
        } => (written.clone(), how_text(frame, source)),
        _ => (
            t!(i18n, ids::RULES_CELL_EMPTY).into_string(),
            t!(i18n, ids::RULES_WHY_HOW_NOT_SET).into_string(),
        ),
    };
    let mut out: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
    out = out.push(
        text(t!(
            i18n,
            ids::RULES_WHY_CELL,
            "row" => super::table::row_name(frame, &row).as_str().to_string(),
            "group" => t!(i18n, group_label(group.id)).into_string(),
            "column" => t!(i18n, column_label(column.id)).into_string(),
            "value" => value,
            "how" => how
        ))
        .size(TS.body)
        .boxed(),
    );
    let winner = match &show {
        CellShow::Value {
            source: CellSource::Inherited { from, .. },
            ..
        } => Some(from.clone()),
        CellShow::Value {
            source: CellSource::Own { .. } | CellSource::Set { .. },
            ..
        } => row.scope(),
        CellShow::Value {
            source: CellSource::Default,
            ..
        } => Some(GridScope::All),
        _ => None,
    };
    let levels: Vec<GridScope> = row
        .scope()
        .into_iter()
        .chain(grid.parents(at.row).iter().cloned())
        .collect();
    for level in levels {
        let own = cells::resolve(&frame.table, &frame.sets, &level, &[], field);
        let wins = winner.as_ref() == Some(&level);
        let how = match own.as_ref().map(|c| &c.source) {
            Some(CellSource::Set { set, .. }) => {
                t!(i18n, ids::RULES_WHY_LEVEL_SET, "set" => set.as_str())
            }
            Some(CellSource::Default) => t!(i18n, ids::RULES_WHY_LEVEL_DEFAULT),
            Some(_) => t!(i18n, ids::RULES_WHY_LEVEL_OWN),
            None => t!(i18n, ids::RULES_WHY_LEVEL_NOT_SET),
        };
        let value = own
            .map(|c| c.value)
            .unwrap_or_else(|| t!(i18n, ids::RULES_CELL_EMPTY).into_string());
        let id = if wins {
            ids::RULES_WHY_LEVEL_WINS
        } else {
            ids::RULES_WHY_LEVEL
        };
        let line = t!(
            i18n,
            id,
            "row" => scope_name(frame, &level),
            "value" => value,
            "how" => how.into_string()
        );
        let ink = if wins {
            ti(tokens.success)
        } else {
            tokens::muted_text(tokens)
        };
        out = out.push(text(line).size(TS.body).color(ink).boxed());
    }
    let extra = match row {
        RowRef::Scoped(_) => ids::RULES_WHY_EXTRA_SCOPED,
        RowRef::Layer { .. } => ids::RULES_WHY_EXTRA_LAYER,
        _ => ids::RULES_WHY_EXTRA_HIERARCHY,
    };
    out = out.push(
        text(t!(i18n, extra))
            .size(TS.caption)
            .color(tokens::muted_text(tokens))
            .boxed(),
    );
    Some(out.boxed())
}

/// Where a cell's value comes from, in a few words.
fn how_text(frame: &Frame, source: &CellSource) -> String {
    let i18n = frame.i18n;
    match source {
        CellSource::Own { .. } => t!(i18n, ids::RULES_WHY_HOW_OWN),
        CellSource::Set { set, .. } => t!(i18n, ids::RULES_WHY_HOW_SET, "set" => set.as_str()),
        CellSource::Inherited { from, .. } => {
            t!(i18n, ids::RULES_WHY_HOW_INHERITED, "row" => scope_name(frame, from))
        }
        CellSource::Default => t!(i18n, ids::RULES_WHY_HOW_DEFAULT),
    }
    .into_string()
}

/// The board's resolution for the first net the governing rule takes in,
/// on the layers it names, for its first limit: the value, where it comes
/// from, and how every rule of that limit fared.
fn board_why<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let mut out: Column<Element<'a, Message>> =
        column![section(frame, ids::RULES_WHY_TITLE)].spacing(SP.xs);
    let Some((effective, net_name)) = frame.why else {
        return out
            .push(
                text(t!(i18n, ids::RULES_WHY_NONE))
                    .size(TS.body)
                    .color(tokens::muted_text(tokens))
                    .boxed(),
            )
            .boxed();
    };
    let mm = |nm: i64| format!("{}", pcb_geom::nm_to_mm(nm));
    let source = match &effective.source {
        Source::Rule(rule) => t!(i18n, ids::RULES_WHY_SOURCE_RULE, "rule" => rule.as_str()),
        Source::NetClass => t!(i18n, ids::RULES_WHY_SOURCE_CLASS),
        Source::BoardSetup => t!(i18n, ids::RULES_WHY_SOURCE_SETUP),
        Source::Unlimited => t!(i18n, ids::RULES_WHY_SOURCE_NONE),
    }
    .into_string();
    let layer = effective
        .layer
        .and_then(layer_name)
        .unwrap_or_else(|| t!(i18n, ids::RULES_LAYER_ALL).into_string());
    let value = effective
        .value_nm
        .map(mm)
        .unwrap_or_else(|| t!(i18n, ids::RULES_CELL_EMPTY).into_string());
    out = out.push(
        text(t!(
            i18n,
            ids::RULES_WHY_FOR,
            "net" => net_name.as_str(),
            "layer" => layer,
            "value" => value,
            "source" => source
        ))
        .size(TS.body)
        .boxed(),
    );
    for candidate in &effective.chain {
        let verdict = match candidate.verdict {
            Verdict::Won => ids::RULES_VERDICT_WON,
            Verdict::LessSpecific => ids::RULES_VERDICT_LESS_SPECIFIC,
            Verdict::LessStrict => ids::RULES_VERDICT_LESS_STRICT,
            Verdict::OtherNets => ids::RULES_VERDICT_OTHER_NETS,
            Verdict::OtherLayers => ids::RULES_VERDICT_OTHER_LAYERS,
            Verdict::OtherObjects => ids::RULES_VERDICT_OTHER_OBJECTS,
        };
        let ink = if candidate.verdict == Verdict::Won {
            ti(tokens.text)
        } else {
            tokens::muted_text(tokens)
        };
        out = out.push(
            text(t!(
                i18n,
                ids::RULES_WHY_CANDIDATE,
                "rule" => candidate.name.as_str(),
                "value" => mm(candidate.value_nm),
                "verdict" => t!(i18n, verdict).into_string()
            ))
            .size(TS.body)
            .color(ink)
            .boxed(),
        );
    }
    out.boxed()
}
