//! The part table of Assign Footprints: a sunken header with the tick-all
//! box, then one ruled row per listed part. A ticked row takes the soft
//! accent fill, the previewed row an accent edge; a changed footprint shows
//! the old name struck and the new one in accent, with a dot on the row.

use iced::widget::{Column, Row, Space, column, container, mouse_area, pick_list, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::{GraphiteDropdown as _, pick_list_style};
use signex_widgets::{PickListPlaceholder as _, checkbox, rich_text, span, text};

use super::{Choice, SP, TS, af, choices, pill};
use crate::app::Message;
use crate::app::assign_footprints::{
    AssignFootprintsMsg, AssignFootprintsState, FootprintPart, PartAccess,
};
use crate::styles::{PillTone, Radii, ti, tokens};

/// Width of the tick column.
const TICK_W: f32 = TS.body * 13.0 / 6.0;
/// Width of the Designator column.
const DESIGNATOR_W: f32 = TS.body * 8.0;
/// Width of the Value column.
const VALUE_W: f32 = TS.body * 55.0 / 6.0;
/// Width of the Footprint column.
const FOOTPRINT_W: f32 = TS.body * 125.0 / 6.0;
/// Side of the changed-row dot.
const DOT: f32 = TS.body / 2.0;

pub(super) fn table<'a>(
    state: &'a AssignFootprintsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let heading = |id, width: Length| -> Element<'a, Message> {
        container(text(t!(i18n, id)).size(TS.caption).color(r.ink4))
            .width(width)
            .boxed()
    };
    let tick_all = checkbox(state.all_selected())
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.body)
        .on_toggle(|on| af(AssignFootprintsMsg::AllToggled(on)));
    let head = Row::<Element<'a, Message>>::new()
        .push(container(tick_all).width(Length::Fixed(TICK_W)).boxed())
        .push(heading(
            ids::DIALOG_ASSIGN_FOOTPRINTS_COL_DESIGNATOR,
            Length::Fixed(DESIGNATOR_W),
        ))
        .push(heading(
            ids::DIALOG_ASSIGN_FOOTPRINTS_COL_VALUE,
            Length::Fixed(VALUE_W),
        ))
        .push(heading(ids::DIALOG_ASSIGN_FOOTPRINTS_COL_ROW, Length::Fill))
        .push(heading(
            ids::DIALOG_ASSIGN_FOOTPRINTS_COL_FOOTPRINT,
            Length::Fixed(FOOTPRINT_W),
        ))
        .spacing(SP.m)
        .align_y(Alignment::Center);
    let (sunken, rule_color) = (r.sunken, r.line_soft);
    let rule = move || -> Element<'a, Message> {
        container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(rule_color)),
                ..container::Style::default()
            })
            .boxed()
    };
    let visible = state.visible_rows();
    let mut list: Column<Element<'a, Message>> = Column::new().width(Length::Fill);
    if visible.is_empty() {
        list = list.push(
            container(
                text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_FILTERED_EMPTY))
                    .size(TS.body)
                    .color(r.ink4),
            )
            .padding([SP.l, SP.m])
            .boxed(),
        );
    }
    let parts = state.parts();
    for index in visible {
        let Some(part) = parts.get(index) else {
            continue;
        };
        list = list.push(rule());
        list = list.push(row_area(state, index, part, i18n, tokens));
    }
    let head = container(head)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            ..container::Style::default()
        });
    container(column![
        head,
        scrollable(list)
            .height(Length::Fill)
            .style(crate::styles::scrollbar_style(tokens))
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true)
    .style(crate::styles::card(tokens))
    .boxed()
}

/// One row with its fill and edge, clickable to preview and hoverable for
/// the "Tick all from this row" link.
fn row_area<'a>(
    state: &'a AssignFootprintsState,
    index: usize,
    part: &'a FootprintPart,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let fill = state
        .selected
        .contains(&index)
        .then(|| crate::panels::graphite::selection_fill(tokens));
    let edge = (state.preview_row == Some(index)).then_some(ti(tokens.accent));
    let cell = container(part_row(state, index, part, i18n, tokens))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                width: if edge.is_some() { 1.0 } else { 0.0 },
                color: edge.unwrap_or(r.line_soft),
                radius: 0.0.into(),
            },
            ..container::Style::default()
        });
    mouse_area(cell)
        .on_press(af(AssignFootprintsMsg::RowPreviewed(index)))
        .on_enter(af(AssignFootprintsMsg::RowHovered {
            row: index,
            inside: true,
        }))
        .on_exit(af(AssignFootprintsMsg::RowHovered {
            row: index,
            inside: false,
        }))
        .boxed()
}

/// One part: tick box, designator (with the changed dot), value, library
/// row (with the hover link), and its footprint list or why it has none.
fn part_row<'a>(
    state: &'a AssignFootprintsState,
    index: usize,
    part: &'a FootprintPart,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let editable = part.access == PartAccess::Editable;
    let ink = if editable { ti(tokens.text) } else { r.ink3 };
    let mut tick = checkbox(state.selected.contains(&index))
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.body);
    if editable {
        tick = tick.on_toggle(move |_| af(AssignFootprintsMsg::RowToggled(index)));
    }
    let mut designator = Row::<Element<'a, Message>>::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .width(Length::Fixed(DESIGNATOR_W));
    if state.is_changed(index) {
        let accent = ti(tokens.accent);
        designator = designator.push(
            container(Space::new().width(DOT).height(DOT))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(accent)),
                    border: Border {
                        radius: Radii::GRAPHITE.pill.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed(),
        );
    }
    designator = designator.push(
        text(UserText::new(part.designator.clone()))
            .size(TS.body)
            .color(ink)
            .font(super::super::graphite::mono())
            .boxed(),
    );
    let value = text(UserText::new(part.value.clone()))
        .size(TS.body)
        .color(ink)
        .width(Length::Fixed(VALUE_W));
    Row::<Element<'a, Message>>::new()
        .push(container(tick).width(Length::Fixed(TICK_W)).boxed())
        .push(designator.boxed())
        .push(value.boxed())
        .push(row_cell(state, index, part, i18n, tokens))
        .push(footprint_cell(state, index, part, i18n, tokens))
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
}

/// The library row, with "Tick all from this row" at its end while the row
/// is hovered or previewed.
fn row_cell<'a>(
    state: &'a AssignFootprintsState,
    index: usize,
    part: &'a FootprintPart,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let label: signex_widgets::Label<'a> = match &part.row_label {
        Some(label) => UserText::new(label.clone()).into(),
        None => t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_NO_ROW).into(),
    };
    let mut cell = Row::<Element<'a, Message>>::new()
        .push(
            text(label)
                .size(TS.body)
                .color(r.ink3)
                .font(super::super::graphite::mono())
                .boxed(),
        )
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .width(Length::Fill);
    let pointed = state.hovered == Some(index) || state.preview_row == Some(index);
    if pointed && part.access == PartAccess::Editable && part.row_key.is_some() {
        cell = cell.push(
            signex_widgets::button(
                text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_SAME_ROW))
                    .size(TS.caption)
                    .color(r.accent_text),
            )
            .padding(0)
            .on_press(af(AssignFootprintsMsg::SameRowTicked(index)))
            .style(crate::styles::ghost_button(tokens))
            .boxed(),
        );
    }
    cell.boxed()
}

/// The footprint list of an editable part (a change shows the old name
/// struck before it, the list in accent), or the footprint as text with the
/// reason pill.
fn footprint_cell<'a>(
    state: &'a AssignFootprintsState,
    index: usize,
    part: &'a FootprintPart,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let mono = super::super::graphite::mono();
    let mut cell = Row::<Element<'a, Message>>::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .width(Length::Fixed(FOOTPRINT_W));
    if let Some(reason) = part.access.reason() {
        cell = cell
            .push(
                text(UserText::new(part.current.clone()))
                    .size(TS.body)
                    .color(r.ink4)
                    .font(mono)
                    .boxed(),
            )
            .push(pill(t!(i18n, reason), PillTone::Neutral, tokens));
        return cell.boxed();
    }
    let changed = state.is_changed(index);
    if changed {
        let faint = r.ink4;
        let old = span::<()>(UserText::new(part.current.clone()))
            .strikethrough(true)
            .color(faint)
            .font(mono)
            .size(TS.caption);
        cell = cell.push(rich_text::<(), Message, Theme>([old]).boxed());
    }
    let options = choices(&part.offers);
    let shown = state.shown_binding(index);
    let current = options
        .iter()
        .find(|c| Some(c.binding.as_str()) == shown)
        .cloned();
    let accent = r.accent_text;
    let list = pick_list(current, options, |c: &Choice| c.label.clone())
        .graphite()
        .style(move |theme: &Theme, status| {
            let mut style = pick_list_style(theme, status);
            if changed {
                style.text_color = accent;
            }
            style
        })
        .font(mono)
        .placeholder_label(UserText::new(part.current.clone()))
        .on_select(move |c: Choice| {
            af(AssignFootprintsMsg::FootprintPicked {
                row: index,
                binding: c.binding,
            })
        })
        .width(Length::Fill);
    cell.push(list.boxed()).boxed()
}
