//! Object List panel (#1259): every object of the sheet in view or of its
//! project, filtered by text and kind. A click selects the object on the
//! canvas, a double click zooms to it; rows selected on the canvas carry an
//! accent edge. A pure view over the rows the panel context holds; the rows
//! are built in `app/handlers/object_list.rs` only when the documents, the
//! scope or the picks change.

mod chips;
pub mod model;

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod view_tests;

use iced::widget::{Column, Space, column, container, mouse_area, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_widgets::text;

pub use model::{
    KindChip, ObjectKind, ObjectListMsg, ObjectListState, ObjectListStatus, ObjectListView,
    ObjectRef, ObjectRow, ObjectScope,
};
use model::{ObjectNoun, RowName, RowValue, ValueWord};

use super::{PanelContext, PanelMsg};
use crate::styles::{Radii, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Height of one object row.
const ROW_H: f32 = TS.body * 2.0;
/// Width of the kind letter column.
const KIND_W: f32 = TS.body * 1.8;
/// Width of the Sheet column.
const SHEET_W: f32 = TS.body * 5.5;
/// Width of the accent edge of a selected row.
const EDGE_W: f32 = TS.body / 6.0;

fn msg(m: ObjectListMsg) -> PanelMsg {
    PanelMsg::ObjectList(m)
}

/// Render the panel.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let view = &ctx.object_list;
    if view.status == ObjectListStatus::NoSchematic {
        let card = crate::panels::empty_state::empty_state_card(
            &ctx.tokens,
            t!(i18n, ids::PANELS_KIND_OBJECT_LIST),
            t!(i18n, ids::PANELS_OBJECT_LIST_NO_SCHEMATIC),
            None,
        );
        return container(card).padding(SP.l).width(Length::Fill).boxed();
    }
    let muted = resolve(&ctx.tokens).ink3;
    let body: Element<'a, PanelMsg> = if view.status == ObjectListStatus::Reading {
        note(t!(i18n, ids::PANELS_OBJECT_LIST_READING), muted)
    } else {
        table(ctx, view)
    };
    let footer = t!(
        i18n,
        ids::PANELS_OBJECT_LIST_FOOTER,
        "total" => view.counts.total(),
        "selected" => view.selected_count
    );
    let footer = container(text(footer).size(TS.caption).color(muted))
        .padding([SP.m, SP.l])
        .width(Length::Fill);
    column![chips::top(ctx, view), body, rule(ctx), footer]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

fn note<'a>(line: Localized, color: iced::Color) -> Element<'a, PanelMsg> {
    container(text(line).size(TS.caption).color(color))
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// A one-pixel soft rule across the panel.
fn rule<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let color = resolve(&ctx.tokens).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// The heading row, the drawn rows, then "… N more" and the hint.
fn table<'a>(ctx: &'a PanelContext, view: &'a ObjectListView) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = resolve(&ctx.tokens).ink3;
    let head = |id: Option<MessageId>, w: Length| -> Element<'a, PanelMsg> {
        let label: Element<'a, PanelMsg> = match id {
            Some(id) => text(t!(i18n, id)).size(TS.caption).color(muted).boxed(),
            None => Space::new().boxed(),
        };
        container(label)
            .width(w)
            .height(ROW_H)
            .align_y(iced::alignment::Vertical::Center)
            .boxed()
    };
    let mut list: Column<Element<'a, PanelMsg>> = column![
        row![
            head(None, Length::Fixed(KIND_W)),
            head(
                Some(ids::PANELS_OBJECT_LIST_COLUMN_NAME),
                Length::FillPortion(2)
            ),
            head(
                Some(ids::PANELS_OBJECT_LIST_COLUMN_VALUE),
                Length::FillPortion(3)
            ),
            head(
                Some(ids::PANELS_OBJECT_LIST_COLUMN_SHEET),
                Length::Fixed(SHEET_W)
            ),
        ]
        .spacing(SP.s)
        .padding([0.0, SP.l]),
        rule(ctx),
    ]
    .width(Length::Fill);
    if view.reading {
        list = list.push(note(t!(i18n, ids::PANELS_OBJECT_LIST_READING), muted));
    }
    if view.rows.is_empty() {
        list = list.push(note(t!(i18n, ids::PANELS_OBJECT_LIST_EMPTY), muted));
    }
    for r in view.rows.iter().take(view.row_limit) {
        list = list.push(object_row(ctx, view, r));
    }
    let more = view.rows.len().saturating_sub(view.row_limit);
    if more > 0 {
        let label = t!(i18n, ids::PANELS_OBJECT_LIST_MORE_ROWS, "count" => more);
        list = list.push(
            signex_widgets::button(text(label).size(TS.caption).color(muted))
                .width(Length::Fill)
                .padding([SP.xs, SP.l])
                .on_press(msg(ObjectListMsg::MoreRowsShown))
                .style(crate::styles::menu_item(&ctx.tokens))
                .boxed(),
        );
    }
    if !view.rows.is_empty() {
        list = list.push(note(t!(i18n, ids::PANELS_OBJECT_LIST_HINT), muted));
    }
    scrollable(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::form_look::scrollbar_style)
        .boxed()
}

/// One object: kind letter, name, value, sheet. Selected on the canvas, it
/// has the accent-soft fill and an accent edge.
fn object_row<'a>(
    ctx: &'a PanelContext,
    view: &ObjectListView,
    r: &'a ObjectRow,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let resolved = resolve(tokens);
    let (ink, muted) = (ti(tokens.text), resolved.ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let selected = view.is_selected(r);
    let mark = container(
        text(t!(i18n, kind_letter(r.kind)))
            .size(TS.caption)
            .font(mono)
            .color(muted),
    )
    .width(Length::Fixed(KIND_W))
    .align_x(iced::alignment::Horizontal::Center)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(resolved.sunken)),
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let name: Element<'a, PanelMsg> = match &r.name {
        RowName::Text(s) => text(UserText::new(s.as_str()))
            .size(TS.body)
            .font(mono)
            .color(ink)
            .boxed(),
        RowName::Noun(noun) => text(t!(i18n, noun_id(*noun)))
            .size(TS.body)
            .font(mono)
            .color(ink)
            .boxed(),
    };
    let value = text(value_label(i18n, &r.value, ctx.unit))
        .size(TS.body)
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None);
    let sheet = text(UserText::new(r.sheet.as_str()))
        .size(TS.caption)
        .color(muted)
        .wrapping(iced::widget::text::Wrapping::None);
    let cells = row![
        mark,
        container(name).width(Length::FillPortion(2)).clip(true),
        container(value).width(Length::FillPortion(3)).clip(true),
        container(sheet).width(Length::Fixed(SHEET_W)).clip(true),
    ]
    .spacing(SP.s)
    .height(ROW_H)
    .align_y(Alignment::Center);
    let edge = if selected {
        crate::styles::tokens::accent(tokens)
    } else {
        iced::Color::TRANSPARENT
    };
    let fill = selected.then_some(resolved.accent_soft);
    let line = row![
        container(Space::new())
            .width(Length::Fixed(EDGE_W))
            .height(ROW_H)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(edge)),
                ..container::Style::default()
            }),
        container(cells)
            .width(Length::Fill)
            .padding([0.0, SP.l - EDGE_W]),
    ]
    .width(Length::Fill);
    let body = container(line)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            ..container::Style::default()
        });
    // A container, not a button: a button would swallow the press the
    // double click is read from.
    mouse_area(body)
        .on_press(msg(ObjectListMsg::RowClicked(r.target.clone())))
        .on_double_click(msg(ObjectListMsg::RowDoubleClicked(r.target.clone())))
        .interaction(iced::mouse::Interaction::Pointer)
        .boxed()
}

/// The Value / text column of a row, written in the user's unit.
fn value_label(
    i18n: &I18n,
    value: &RowValue,
    unit: signex_types::coord::Unit,
) -> signex_widgets::Label<'static> {
    match value {
        RowValue::Empty => UserText::new(String::new()).into(),
        RowValue::Text(s) => UserText::new(s.clone()).into(),
        RowValue::Word(word) => t!(i18n, word_id(*word)).into(),
        RowValue::Length { net, mm } => {
            let length = signex_types::measure::format_length_mm(*mm, unit);
            match net {
                Some(net) => t!(
                    i18n,
                    ids::PANELS_OBJECT_LIST_VALUE_WIRE,
                    "net" => net.as_str(),
                    "length" => length.as_str()
                )
                .into(),
                None => UserText::new(length).into(),
            }
        }
    }
}

pub(super) fn kind_letter(kind: ObjectKind) -> MessageId {
    match kind {
        ObjectKind::Part => ids::PANELS_OBJECT_LIST_LETTER_PART,
        ObjectKind::Label => ids::PANELS_OBJECT_LIST_LETTER_LABEL,
        ObjectKind::Wire => ids::PANELS_OBJECT_LIST_LETTER_WIRE,
        ObjectKind::PowerPort => ids::PANELS_OBJECT_LIST_LETTER_POWER_PORT,
        ObjectKind::Port => ids::PANELS_OBJECT_LIST_LETTER_PORT,
        ObjectKind::SheetSymbol => ids::PANELS_OBJECT_LIST_LETTER_SHEET_SYMBOL,
        ObjectKind::SheetEntry => ids::PANELS_OBJECT_LIST_LETTER_SHEET_ENTRY,
        ObjectKind::Text => ids::PANELS_OBJECT_LIST_LETTER_TEXT,
        ObjectKind::Junction => ids::PANELS_OBJECT_LIST_LETTER_JUNCTION,
        ObjectKind::Bus => ids::PANELS_OBJECT_LIST_LETTER_BUS,
        ObjectKind::Drawing => ids::PANELS_OBJECT_LIST_LETTER_DRAWING,
        ObjectKind::Directive => ids::PANELS_OBJECT_LIST_LETTER_DIRECTIVE,
    }
}

fn noun_id(noun: ObjectNoun) -> MessageId {
    match noun {
        ObjectNoun::Wire => ids::PANELS_OBJECT_LIST_NOUN_WIRE,
        ObjectNoun::Text => ids::PANELS_OBJECT_LIST_NOUN_TEXT,
        ObjectNoun::Junction => ids::PANELS_OBJECT_LIST_NOUN_JUNCTION,
        ObjectNoun::Bus => ids::PANELS_OBJECT_LIST_NOUN_BUS,
        ObjectNoun::Drawing => ids::PANELS_OBJECT_LIST_NOUN_DRAWING,
        ObjectNoun::Blanket => ids::PANELS_OBJECT_LIST_NOUN_BLANKET,
        ObjectNoun::CompileMask => ids::PANELS_OBJECT_LIST_NOUN_COMPILE_MASK,
        ObjectNoun::DiffPair => ids::PANELS_OBJECT_LIST_NOUN_DIFF_PAIR,
    }
}

fn word_id(word: ValueWord) -> MessageId {
    match word {
        ValueWord::NetLabel => ids::PANELS_OBJECT_LIST_VALUE_NET_LABEL,
        ValueWord::PowerPort => ids::PANELS_OBJECT_LIST_VALUE_POWER_PORT,
        ValueWord::Port => ids::PANELS_OBJECT_LIST_VALUE_PORT,
        ValueWord::SheetEntry => ids::PANELS_OBJECT_LIST_VALUE_SHEET_ENTRY,
    }
}
