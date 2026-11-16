//! The Clearance sub-tab's class-to-class matrix (#2398): one row and one
//! column per net class, one clearance per cell. A written cell shows its
//! value; an empty one shows, greyed, what the pair needs without it. A
//! click types in the cell and opens its object sub-grid below.

use iced::widget::{Column, Row, column, row, scrollable};
use iced::{Alignment, Element, Length};
use signex_drc_dsl::table::matrix::{self, CellKey, OBJECT_PAIRS};
use signex_i18n::{ids, t};
use signex_widgets::{button, text, text_input};

use super::{Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::RulesTabMsg;
use crate::app::rules_tab::matrix::default_key;
use crate::panels::form_look::ghost_button;
use crate::styles::tokens;

fn cell_width() -> Length {
    Length::Fixed(TS.body * 8.0)
}

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let mut body: Column<'a, Message> = Column::new().spacing(SP.xs);
    body = body.push(
        text(t!(i18n, ids::RULES_MATRIX_HINT))
            .size(TS.caption)
            .color(tokens::muted_text(tokens)),
    );
    if frame.classes.is_empty() {
        body = body.push(
            text(t!(i18n, ids::RULES_MATRIX_NO_CLASSES))
                .size(TS.body)
                .color(tokens::muted_text(tokens)),
        );
        return body.height(Length::FillPortion(3)).into();
    }
    let mut head: Row<'a, Message> = Row::new()
        .spacing(SP.s)
        .push(text(user(String::new())).width(cell_width()));
    for class in &frame.classes {
        head = head.push(
            text(user(class.clone()))
                .size(TS.caption)
                .color(tokens::muted_text(tokens))
                .width(cell_width()),
        );
    }
    let mut grid: Column<'a, Message> = Column::new().spacing(SP.xs).push(head);
    for a in &frame.classes {
        let mut line: Row<'a, Message> = Row::new()
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .push(text(user(a.clone())).size(TS.body).width(cell_width()));
        for b in &frame.classes {
            line = line.push(cell(frame, CellKey::new(a.clone(), b.clone())));
        }
        grid = grid.push(line);
    }
    body = body.push(scrollable(grid).height(Length::FillPortion(2)));
    if let Some((a, b)) = frame.state.matrix_open.clone() {
        body = body.push(sub_grid(frame, a, b));
    }
    body.height(Length::FillPortion(3)).into()
}

/// One cell: typed in, written, or its fallback greyed.
fn cell<'a>(frame: &Frame<'a>, key: CellKey) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let editing = frame
        .state
        .matrix_edit
        .as_ref()
        .filter(|edit| edit.key.same_as(&key));
    if let Some(edit) = editing {
        let input = text_input(t!(i18n, ids::RULES_CELL_EMPTY), edit.text.as_str())
            .on_input(move |text| {
                up(RulesTabMsg::MatrixCellEdited {
                    project_id: id,
                    text,
                })
            })
            .on_submit(up(RulesTabMsg::MatrixCellCommitted { project_id: id }))
            .size(TS.body)
            .padding(SP.xs)
            .width(Length::Fill)
            .style(crate::panels::form_look::text_input_style);
        let cancel = button(text(t!(i18n, ids::RULES_CELL_CANCEL)).size(TS.caption))
            .padding([0.0, SP.xs])
            .style(ghost_button)
            .on_press(up(RulesTabMsg::MatrixCellCancelled { project_id: id }));
        return row![input, cancel]
            .spacing(SP.xs)
            .align_y(Alignment::Center)
            .width(cell_width())
            .into();
    }
    let written = matrix::cell(&frame.table, &key)
        .and_then(|row| row.limit(signex_drc_dsl::table::LimitField::MinClearance))
        .map(|limit| limit.value.clone());
    let face: Element<'a, Message> = match written {
        Some(value) => text(user(value)).size(TS.body).into(),
        None => text(fallback(frame, &key))
            .size(TS.body)
            .color(tokens::muted_text(tokens))
            .into(),
    };
    button(face)
        .padding([0.0, SP.xs])
        .width(cell_width())
        .style(ghost_button)
        .on_press(up(RulesTabMsg::MatrixCellOpened {
            project_id: id,
            key,
        }))
        .into()
}

/// What an empty cell falls back to, worded.
fn fallback(frame: &Frame, key: &CellKey) -> signex_i18n::Localized {
    let i18n = frame.i18n;
    let nm = frame
        .state
        .analysis
        .as_ref()
        .and_then(|analysis| analysis.defaults.get(&default_key(key)).copied());
    match nm {
        Some(nm) => {
            let value = crate::app::rules_tab::json::mm(nm).to_string();
            t!(i18n, ids::RULES_MATRIX_DEFAULT, "value" => value)
        }
        None => t!(i18n, ids::RULES_MATRIX_NO_DEFAULT),
    }
}

/// The object sub-grid of classes `a` and `b`: the whole cell, then one
/// cell per object pair, each with Clear when written.
fn sub_grid<'a>(frame: &Frame<'a>, a: String, b: String) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let title = t!(
        i18n,
        ids::RULES_MATRIX_OBJECTS_TITLE,
        "a" => a.as_str(),
        "b" => b.as_str()
    );
    let close = button(text(t!(i18n, ids::RULES_MATRIX_COLLAPSE)).size(TS.caption))
        .padding([0.0, SP.xs])
        .style(ghost_button)
        .on_press(up(RulesTabMsg::MatrixCollapsed { project_id: id }));
    let mut lines: Column<'a, Message> = Column::new().spacing(SP.xs).push(
        row![text(title).size(TS.label), close]
            .spacing(SP.m)
            .align_y(Alignment::Center),
    );
    let whole = CellKey::new(a.clone(), b.clone());
    let mut keys = vec![(t!(i18n, ids::RULES_MATRIX_ALL_OBJECTS).into_string(), whole)];
    for [p, q] in OBJECT_PAIRS {
        let mut key = CellKey::new(a.clone(), b.clone());
        key.pair = Some([p.to_string(), q.to_string()]);
        keys.push((format!("{p}-{q}"), key));
    }
    for (label, key) in keys {
        let written = matrix::cell(&frame.table, &key).is_some();
        let mut line: Row<'a, Message> = Row::new()
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .push(
                text(user(label))
                    .size(TS.body)
                    .color(tokens::muted_text(tokens))
                    .width(cell_width()),
            )
            .push(cell(frame, key.clone()));
        if written {
            line = line.push(
                button(text(t!(i18n, ids::RULES_MATRIX_CLEAR)).size(TS.caption))
                    .padding([0.0, SP.xs])
                    .style(crate::styles::outline_button(tokens))
                    .on_press(up(RulesTabMsg::MatrixCellCleared {
                        project_id: id,
                        key,
                    })),
            );
        }
        lines = lines.push(line);
    }
    column![lines].padding([SP.s, SP.m]).into()
}
