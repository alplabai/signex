//! The selected rule's detail: its text, what the compiler and the flags
//! say about it, the objects it applies to, and "Why this value?" for a
//! net it takes in, by the check's own resolution.

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Element, Length};
use signex_drc::{Source, Verdict};
use signex_drc_dsl::table::RuleRow;
use signex_i18n::{MessageId, ids, t};
use signex_widgets::{button, text};

use super::table::flag_text;
use super::{Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::RulesTabMsg;
use crate::app::rules_tab::json::layer_name;
use crate::styles::tokens;

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let Some(row) = frame
        .state
        .selected
        .as_deref()
        .and_then(|name| frame.table.row(name))
    else {
        return Space::new().height(Length::Shrink).into();
    };
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let heading = text(user(row.name.clone()))
        .size(TS.title)
        .color(crate::styles::ti(tokens.text));
    let remove = button(text(t!(i18n, ids::RULES_REMOVE)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::danger_button(tokens))
        .on_press(up(RulesTabMsg::RuleRemoved {
            project_id: id,
            rule: row.name.clone(),
        }));
    let mut body: Column<'a, Message> = Column::new().spacing(SP.s);
    body = body.push(
        row![heading, Space::new().width(Length::Fill), remove]
            .spacing(SP.m)
            .align_y(Alignment::Center),
    );
    body = body.push(section(frame, ids::RULES_DETAIL_TEXT));
    body = body.push(
        text(user(row.text(&frame.src).to_string()))
            .size(TS.body)
            .font(crate::fonts::iced_font_for_family(
                crate::fonts::CHROME_MONO_FONT,
            )),
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
                .color(tokens::danger(tokens)),
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
            body = body.push(text(user(flag)).size(TS.body).color(tokens::danger(tokens)));
        }
    }
    body = body.push(scope(frame, row));
    body = body.push(why(frame, row));
    container(scrollable(body).height(Length::Fill))
        .padding(SP.m)
        .width(Length::Fill)
        .height(Length::FillPortion(2))
        .style(crate::styles::card(tokens))
        .into()
}

fn section<'a>(frame: &Frame<'a>, id: MessageId) -> Element<'a, Message> {
    text(t!(frame.i18n, id))
        .size(TS.caption)
        .color(tokens::muted_text(frame.tokens))
        .into()
}

/// How many objects the rule applies to, and a button that shows them.
fn scope<'a>(frame: &Frame<'a>, row: &RuleRow) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let count = frame
        .state
        .analysis
        .as_ref()
        .filter(|a| a.has_board)
        .and_then(|a| a.scope.get(&row.name).copied());
    let Some(count) = count else {
        return text(t!(i18n, ids::RULES_DETAIL_SCOPE_UNKNOWN))
            .size(TS.body)
            .color(tokens::muted_text(tokens))
            .into();
    };
    let show = button(text(t!(i18n, ids::RULES_DETAIL_SHOW)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::outline_button(tokens))
        .on_press_maybe((count > 0).then(|| {
            up(RulesTabMsg::ScopeShown {
                project_id: id,
                rule: row.name.clone(),
            })
        }));
    row![
        text(t!(i18n, ids::RULES_DETAIL_SCOPE, "count" => count)).size(TS.body),
        show,
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .into()
}

/// "Why this value?" for the first net the rule takes in, on the layers
/// it names, for its first limit: the value, where it comes from, and how
/// every rule of that limit fared.
fn why<'a>(frame: &Frame<'a>, row: &RuleRow) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let mut out: Column<'a, Message> = column![section(frame, ids::RULES_WHY_TITLE)].spacing(SP.xs);
    let found = sample(frame, row);
    let Some((effective, net_name)) = found else {
        return out
            .push(
                text(t!(i18n, ids::RULES_WHY_NONE))
                    .size(TS.body)
                    .color(tokens::muted_text(tokens)),
            )
            .into();
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
            "net" => net_name,
            "layer" => layer,
            "value" => value,
            "source" => source
        ))
        .size(TS.body),
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
            crate::styles::ti(tokens.text)
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
            .color(ink),
        );
    }
    out.into()
}

/// The value that governs the first net `row` takes in, on the layers and
/// objects it names, for its first limit, and that net's name. `None` for
/// an off or invalid rule, a rule with no net on the board, or no board.
fn sample(frame: &Frame, row: &RuleRow) -> Option<(signex_drc::Effective, String)> {
    let rules = frame.resolved.as_ref()?;
    let board = frame.board?;
    let compiled = row.compiled.as_ref().filter(|_| row.applied)?;
    let field = row.limits.first()?.field;
    let net = rules.first_net_of(board, &row.name)?;
    let objects = compiled
        .pair
        .map_or([None, None], |[a, b]| [Some(a), Some(b)]);
    let layers = compiled.layers.unwrap_or(u32::MAX);
    let effective = rules.effective(field.limit_kind(), net, layers, objects);
    let net_name = board
        .nets
        .iter()
        .find(|n| n.number == net)
        .map(|n| n.name.clone())
        .unwrap_or_default();
    Some((effective, net_name))
}
