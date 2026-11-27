//! The compare step (mockup row 129): Placed now and Replace with, the
//! Now / After table with the Keep now boxes, the connection warnings, what
//! is kept and Apply to.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Space, column, container, row, scrollable};
use iced::{Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{checkbox, radio, text};

use crate::library::messages::{LibraryMessage, PlacedPartMsg};
use crate::library::picker::mono;
use crate::library::state::{
    ChangePreview, ChangeScope, ChangeSession, CompareField, CompareKind, CompareRow,
};
use crate::styles::ti;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn view<'a>(
    change: &'a ChangePreview,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    let session = &change.session;
    let body = column![
        cards(session, change, tokens, i18n),
        heading(ids::LIBRARY_CHANGE_COMPONENT_PARAMETERS, tokens, i18n),
        table(change, tokens, i18n),
        warnings(change, tokens, i18n),
        heading(ids::LIBRARY_CHANGE_COMPONENT_KEPT, tokens, i18n),
        kept(session, change, tokens, i18n),
        heading(ids::LIBRARY_CHANGE_COMPONENT_APPLY_TO, tokens, i18n),
        scope(session, change, i18n),
    ]
    .spacing(SP.s)
    .padding([SP.m, SP.l]);
    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// A section heading (`.fgroup`): caption size, the muted section ink.
fn heading<'a>(id: MessageId, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, LibraryMessage> {
    let words = t!(i18n, id).into_string();
    text(UserText::new(i18n.upper_for_display(&words)))
        .size(TS.caption)
        .color(crate::styles::tokens::resolve(tokens).ink3)
        .boxed()
}

/// One card: a caption over a mono part number and a caption line.
fn card<'a>(
    caption: Localized,
    part: &'a str,
    detail: &'a str,
    accent: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let (edge, fill, caption_ink) = if accent {
        (ti(tokens.accent), Some(r.accent_soft), r.accent_soft_ink)
    } else {
        (ti(tokens.border), None, r.ink4)
    };
    let lines = column![
        text(caption).size(TS.caption).color(caption_ink),
        text(UserText::new(part))
            .size(TS.body)
            .font(mono())
            .color(theme_ext::text_primary(tokens))
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End),
        text(UserText::new(detail))
            .size(TS.caption)
            .color(r.ink4)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End),
    ]
    .spacing(SP.xs);
    container(lines)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                width: 1.0,
                color: edge,
                radius: crate::styles::Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Placed now › Replace with.
fn cards<'a>(
    session: &'a ChangeSession,
    change: &'a ChangePreview,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let new_part = change
        .resolved
        .fields
        .iter()
        .find(|(key, value)| key == "MPN" && !value.trim().is_empty())
        .map_or(change.resolved.internal_pn.as_str(), |(_, value)| {
            value.as_str()
        });
    let r = crate::styles::tokens::resolve(tokens);
    row![
        card(
            t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_PLACED_NOW),
            &session.placed.part,
            &session.placed.footprint,
            false,
            tokens,
        ),
        signex_widgets::chevron::chevron(false, TS.body, r.ink4),
        card(
            t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_REPLACE_WITH),
            new_part,
            &change.resolved.footprint,
            true,
            tokens,
        ),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// One column's width; the head and the rows share them.
fn cell_width(column: usize) -> Length {
    match column {
        0 => Length::FillPortion(3),
        1 | 2 => Length::FillPortion(4),
        _ => Length::FillPortion(2),
    }
}

fn cell<'a>(content: Element<'a, LibraryMessage>, column: usize) -> Element<'a, LibraryMessage> {
    container(content)
        .padding([SP.xs, SP.s])
        .width(cell_width(column))
        .clip(true)
        .boxed()
}

fn value_text<'a>(value: &'a str, ink: iced::Color) -> Element<'a, LibraryMessage> {
    text(UserText::new(value))
        .size(TS.body)
        .font(mono())
        .color(ink)
        .width(Length::Fill)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End)
        .boxed()
}

/// The Now / After table. A replaced row is tinted; a user parameter gets
/// its Keep now box; library data the new part replaces gets an inert,
/// empty box (it cannot be kept, DECISIONS 2026-11-19).
fn table<'a>(
    change: &'a ChangePreview,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let mut head = row![].spacing(0);
    for (column, id) in [
        ids::LIBRARY_CHANGE_COMPONENT_COLUMN_PARAMETER,
        ids::LIBRARY_CHANGE_COMPONENT_COLUMN_NOW,
        ids::LIBRARY_CHANGE_COMPONENT_COLUMN_AFTER,
        ids::LIBRARY_CHANGE_COMPONENT_COLUMN_KEEP,
    ]
    .into_iter()
    .enumerate()
    {
        head = head.push(cell(
            text(t!(i18n, id)).size(TS.caption).color(r.ink3).boxed(),
            column,
        ));
    }
    let mut lines = column![head].spacing(0);
    for line in &change.rows {
        lines = lines.push(table_row(line, change, tokens, i18n));
    }
    let edge = ti(tokens.border);
    container(lines)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color: edge,
                radius: crate::styles::Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The catalog name of a part-data row, or the field's own name.
fn field_label<'a>(field: &'a CompareField, i18n: &I18n) -> signex_widgets::Label<'a> {
    let id = match field {
        CompareField::Value => ids::LIBRARY_CHANGE_COMPONENT_VALUE,
        CompareField::Footprint => ids::LIBRARY_CHANGE_COMPONENT_FOOTPRINT,
        CompareField::Datasheet => ids::LIBRARY_CHANGE_COMPONENT_DATASHEET,
        CompareField::Named(name) => return UserText::new(name.as_str()).into(),
    };
    t!(i18n, id).into()
}

fn table_row<'a>(
    line: &'a CompareRow,
    change: &'a ChangePreview,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let ink = theme_ext::text_primary(tokens);
    let mut name = row![
        text(field_label(&line.field, i18n))
            .size(TS.body)
            .color(ink)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
    ]
    .spacing(SP.xs);
    if line.kind == CompareKind::User {
        name = name.push(
            text(t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_USER))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }
    let after: Element<'a, LibraryMessage> = if line.kind == CompareKind::User {
        text(t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_NOT_IN_PART))
            .size(TS.body)
            .color(r.ink4)
            .boxed()
    } else {
        value_text(&line.after, ink)
    };
    let keep: Element<'a, LibraryMessage> = match line.kind {
        CompareKind::Same => text(t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_SAME))
            .size(TS.body)
            .color(r.ink4)
            .boxed(),
        CompareKind::Replaced => checkbox(false)
            .size(TS.body)
            .style(crate::panels::form_look::checkbox_style)
            .boxed(),
        CompareKind::User => match &line.field {
            CompareField::Named(field) => checkbox(!change.dropped.contains(field))
                .size(TS.body)
                .style(crate::panels::form_look::checkbox_style)
                .on_toggle(move |_| {
                    LibraryMessage::PlacedPart(PlacedPartMsg::ChangeKeepToggled(field.clone()))
                })
                .boxed(),
            CompareField::Value | CompareField::Footprint | CompareField::Datasheet => {
                Space::new().boxed()
            }
        },
    };
    let tint = (line.kind == CompareKind::Replaced).then_some(r.warn_soft);
    let rule = r.line_soft;
    container(
        row![
            cell(name.boxed(), 0),
            cell(value_text(&line.now, ink), 1),
            cell(after, 2),
            cell(keep, 3),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: tint.map(Background::Color),
        border: Border::default().top(iced::border::Side::default().width(1.0).color(rule)),
        ..container::Style::default()
    })
    .boxed()
}

/// The connection warnings of the pin preview, in the warning ink; the data
/// the swap replaces is in the table above.
fn warnings<'a>(
    change: &ChangePreview,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let warning = theme_ext::warning_color(tokens);
    let preview = &change.preview;
    let mut said: Vec<Localized> = Vec::new();
    if let Some(unit) = preview.missing_unit {
        said.push(t!(i18n, ids::LIBRARY_CHANGE_COMPONENT_WARN_UNIT, "unit" => unit));
    }
    for (pins, id) in [
        (
            &preview.unmatched_connected,
            ids::LIBRARY_CHANGE_COMPONENT_WARN_DANGLING,
        ),
        (
            &preview.moved_connected,
            ids::LIBRARY_CHANGE_COMPONENT_WARN_MOVED,
        ),
        (&preview.gained, ids::LIBRARY_CHANGE_COMPONENT_WARN_GAINED),
    ] {
        if !pins.is_empty() {
            said.push(t!(i18n, id, "pins" => pins.join(", ")));
        }
    }
    let mut lines = column![].spacing(SP.xs);
    for line in said {
        lines = lines.push(text(line).size(TS.body).color(warning).boxed());
    }
    lines.boxed()
}

/// What stays: the designator, the placement and the pins matched.
fn kept<'a>(
    session: &ChangeSession,
    change: &ChangePreview,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mapping = &change.preview.mapping;
    let matched = mapping.pairs.len();
    text(t!(
        i18n,
        ids::LIBRARY_CHANGE_COMPONENT_KEPT_SUMMARY,
        "reference" => session.reference.as_str(),
        "matched" => matched,
        "total" => matched + mapping.unmatched.len()
    ))
    .size(TS.caption)
    .color(crate::styles::tokens::resolve(tokens).ink3)
    .boxed()
}

/// The Apply to choice: only this part. The channel and same-MPN scopes of
/// mockup row 155 wait on a maintainer decision.
fn scope<'a>(
    session: &ChangeSession,
    change: &ChangePreview,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let option = |label: Localized, value: ChangeScope| -> Element<'a, LibraryMessage> {
        radio(label, value, Some(change.scope), |scope| {
            LibraryMessage::PlacedPart(PlacedPartMsg::ChangeScopePicked(scope))
        })
        .size(TS.heading)
        .spacing(SP.s)
        .text_size(TS.body)
        .boxed()
    };
    column![option(
        t!(
            i18n,
            ids::LIBRARY_CHANGE_COMPONENT_SCOPE_THIS,
            "reference" => session.reference.as_str()
        ),
        ChangeScope::ThisPart,
    )]
    .spacing(SP.xs)
    .boxed()
}
