//! The dialog's left half: the search field, the Same symbol / Same value
//! chips and the part list, each row tagged with its pin match. A row whose
//! pins differ is greyed and cannot be picked; the tag's tooltip says why.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{column, container, row, scrollable};
use iced::{Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::theme_ext;
use signex_widgets::{button, text};

use crate::library::PickerState;
use crate::library::messages::{LibraryMessage, PickerMsg, PlacedPartMsg};
use crate::library::picker::{empty_card, mono, search_field};
use crate::library::state::{ChangeFilter, ChangeSession, PickerEntry, PinMatch};
use crate::styles::{PillTone, ti};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn view<'a>(
    picker: &'a PickerState,
    session: &'a ChangeSession,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    // Enter in the search field reads the picked row while the list shows it.
    let picked = picker.shown_selection();
    let search = search_field(
        t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_SEARCH),
        &picker.filter,
        |s| LibraryMessage::Picker(PickerMsg::FilterChanged(s)),
        picked.map(|_| LibraryMessage::PlacedPart(PlacedPartMsg::ChangePartPicked)),
        tokens,
    );
    let rule = r.line_soft;
    let top = container(column![search, filter_chips(session, tokens, i18n)].spacing(SP.s))
        .padding([SP.m, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border::default().bottom(iced::border::Side::default().width(1.0).color(rule)),
            ..container::Style::default()
        });
    let mut rows = column![].spacing(0);
    let mut listed = 0usize;
    for entry in picker
        .rows
        .visible()
        .filter(|e| !session.hidden.contains(&e.summary.row_id))
    {
        let is_picked = picked.is_some_and(|(path, summary)| {
            path.as_path() == &*entry.path && summary.row_id == entry.summary.row_id
        });
        rows = rows.push(part_row(entry, session, is_picked, tokens, i18n));
        listed += 1;
    }
    let list: Element<'a, LibraryMessage> = if listed == 0 {
        empty_card(t!(i18n, ids::LIBRARY_PICKER_EMPTY), tokens)
    } else {
        scrollable(rows)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    };
    column![top, list].height(Length::Fill).boxed()
}

/// Same symbol, Same value and In stock; Same value is inert while the
/// placed part's value is not known.
fn filter_chips<'a>(
    session: &ChangeSession,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let style = ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: ti(tokens.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    };
    let metrics = ChipMetrics::from_text_size(TS.body);
    let toggle = |filter| LibraryMessage::PlacedPart(PlacedPartMsg::ChangeFilterToggled(filter));
    row![
        chip(
            t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_SAME_SYMBOL),
            None,
            session.filters.same_symbol,
            style,
            metrics,
            i18n,
            Some(toggle(ChangeFilter::SameSymbol)),
        ),
        chip(
            t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_SAME_VALUE),
            None,
            session.filters.same_value,
            style,
            metrics,
            i18n,
            session
                .placed
                .value
                .is_some()
                .then(|| toggle(ChangeFilter::SameValue)),
        ),
        chip(
            t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_IN_STOCK),
            None,
            session.filters.in_stock,
            style,
            metrics,
            i18n,
            Some(toggle(ChangeFilter::InStock)),
        ),
    ]
    .spacing(SP.s)
    .boxed()
}

/// A value on one line, ended with an ellipsis.
fn one_line<'a>(
    value: &'a str,
    font: Option<iced::Font>,
    size: f32,
    ink: iced::Color,
) -> Element<'a, LibraryMessage> {
    let line = text(UserText::new(value))
        .size(size)
        .color(ink)
        .width(Length::Fill)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End);
    match font {
        Some(font) => line.font(font).boxed(),
        None => line.boxed(),
    }
}

/// One part: its MPN (else its internal PN) in mono over its description,
/// and the pin tag at the right end.
fn part_row<'a>(
    entry: &'a PickerEntry,
    session: &ChangeSession,
    is_picked: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let summary = &entry.summary;
    let r = crate::styles::tokens::resolve(tokens);
    let pins = session.pin_match(summary.row_id);
    let pickable = !matches!(pins, Some(PinMatch::Differs { .. }));
    let ink = if pickable {
        theme_ext::text_primary(tokens)
    } else {
        r.ink4
    };
    let title = if summary.mpn.trim().is_empty() {
        summary.internal_pn.as_str()
    } else {
        summary.mpn.as_str()
    };
    let lines = column![
        one_line(title, Some(mono()), TS.body, ink),
        one_line(summary.description.as_str(), None, TS.caption, r.ink4),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    let mut line = row![lines].spacing(SP.s).align_y(iced::Alignment::Center);
    if let Some(pins) = pins {
        line = line.push(pin_tag(pins, session, tokens, i18n));
    }
    let (tint, hover, rule, bar) = (
        crate::styles::tokens::selection_tint(tokens),
        crate::styles::tokens::hover_overlay(tokens),
        r.line_soft,
        ti(tokens.accent),
    );
    let mut part = button(line)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let background = match (is_picked, status) {
                (true, _) => Some(Background::Color(tint)),
                (false, iced::widget::button::Status::Hovered) => Some(Background::Color(hover)),
                _ => None,
            };
            let mut border =
                Border::default().bottom(iced::border::Side::default().width(1.0).color(rule));
            if is_picked {
                border = border.left(iced::border::Side::default().width(2.0).color(bar));
            }
            iced::widget::button::Style {
                background,
                border,
                text_color: ink,
                ..iced::widget::button::Style::default()
            }
        });
    if pickable {
        part = part.on_press(LibraryMessage::PlacedPart(PlacedPartMsg::ChangeRowClicked(
            summary.row_id,
        )));
    }
    part.boxed()
}

/// "same pins", or "4 pins, not 2" with the reason in its tooltip.
fn pin_tag<'a>(
    pins: PinMatch,
    session: &ChangeSession,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let (words, tone) = match pins {
        PinMatch::Same => (
            t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_PINS_SAME),
            PillTone::Ok,
        ),
        PinMatch::Differs { row, placed } => (
            t!(
                i18n,
                ids::LIBRARY_CHANGE_COMPONENT_PINS_DIFFER,
                "row" => row,
                "placed" => placed
            ),
            PillTone::Warn,
        ),
    };
    let (_, ink) = crate::styles::pill_colors(tokens, tone);
    let tag = container(
        text(words)
            .size(TS.caption)
            .color(ink)
            .wrapping(Wrapping::None),
    )
    .padding([0.0, SP.s])
    .style(crate::styles::pill(tokens, tone))
    .boxed();
    let PinMatch::Differs { row, placed } = pins else {
        return tag;
    };
    let why = container(
        text(t!(
            i18n,
            ids::LIBRARY_CHANGE_COMPONENT_PINS_DIFFER_TIP,
            "row" => row,
            "reference" => session.reference.as_str(),
            "placed" => placed
        ))
        .size(TS.body),
    )
    .padding(SP.s)
    .style(crate::styles::context_menu(tokens));
    signex_widgets::tooltip(tag, why, signex_widgets::tooltip::Position::Top).boxed()
}
