//! Graphite Theme page: one preview card per theme (Graphite Light and
//! Graphite Dark). Choosing a card sends the same `DraftTheme` the old
//! picker sent; nothing is written to disk.

use super::{PrefMsg, SP, TS};
use crate::styles::{self, Radii, chrome_px, ti, tokens};
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::{button, raw, text};

/// A theme card: id, card title, tag on the right, description.
const FEATURED: [(ThemeId, &str, &str, &str); 2] = [
    (
        ThemeId::GraphiteLight,
        "Graphite Light",
        "Default",
        "Warm paper canvas, graphite ink, copper accent.",
    ),
    (
        ThemeId::GraphiteDark,
        "Graphite Dark",
        "Dark",
        "Low-glare charcoal for long routing sessions.",
    ),
];

/// The Theme heading and one preview card per theme.
/// `active` carries the dialog's own tokens (labels, card chrome).
pub(super) fn theme_section<'a>(draft: ThemeId, active: &ThemeTokens) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(active);
    let label = |s: &str| {
        // user-visible: migrate to the #2063 casemap
        // ast-grep-ignore: no-case-in-view
        text(raw(s.to_uppercase()))
            .size(TS.section)
            .style(styles::section_label(active))
            .boxed()
    };
    let mut col = column![
        text(raw("Theme"))
            .size(TS.heading)
            .color(ti(active.text))
            .boxed(),
        text(raw(
            "Colours, density and canvas text. Changes preview live; Apply keeps them."
        ))
        .size(TS.label)
        .color(r.ink3)
        .boxed(),
        Space::new().height(SP.l).boxed(),
        label("Application theme"),
    ]
    .spacing(SP.s);

    let mut featured = row![].spacing(SP.l);
    for (id, name, tag, desc) in FEATURED {
        featured = featured.push(preview_card(id, name, tag, desc, draft, active));
    }
    col = col.push(featured.boxed());
    col.boxed()
}

fn card_border(selected: bool, accent: Color, line: Color, radius: f32) -> Border {
    Border {
        width: if selected { 2.0 } else { 1.0 },
        color: if selected { accent } else { line },
        radius: radius.into(),
        ..Border::default()
    }
}

/// One headline card: a mini schematic in the theme's own colours, a radio
/// mark, the name, a tag and a one-line description.
fn preview_card<'a>(
    id: ThemeId,
    name: &str,
    tag: &str,
    desc: &str,
    draft: ThemeId,
    active: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let selected = id == draft;
    let t = theme_tokens(id);
    let a = tokens::resolve(active);
    let (accent, line, ink) = (ti(active.accent), a.line_soft, ti(active.text));
    let radio = container(Space::new())
        .width(Length::Fixed(TS.label))
        .height(Length::Fixed(TS.label))
        .style(move |_: &Theme| container::Style {
            background: selected.then_some(Background::Color(accent)),
            border: Border {
                width: if selected { 4.0 } else { 1.0 },
                color: if selected { accent } else { line },
                radius: 999.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed();
    let body = column![
        mini_preview(&t),
        row![
            radio,
            text(raw(name.to_string()))
                .size(TS.title)
                .color(ink)
                .boxed(),
            Space::new().width(Length::Fill).boxed(),
            text(raw(tag.to_string()))
                .size(TS.caption)
                .color(a.ink3)
                .boxed(),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed(),
        text(raw(desc.to_string()))
            .size(TS.body)
            .color(a.ink3)
            .boxed(),
    ]
    .spacing(SP.m);
    let raised = a.raised;
    button(container(body.boxed()).padding(SP.l))
        .padding(0)
        .width(Length::FillPortion(1))
        .on_press(PrefMsg::DraftTheme(id))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(raised)),
            text_color: ink,
            border: card_border(selected, accent, line, Radii::GRAPHITE.card),
            ..button::Style::default()
        })
        .boxed()
}

/// Two boxed parts joined by a copper-style wire, under a title strip, drawn
/// in `t`'s paper, ink and accent.
fn mini_preview<'a>(t: &ThemeTokens) -> Element<'a, PrefMsg> {
    let (paper, ink, accent, rule) = (ti(t.paper), ti(t.text), ti(t.accent), ti(t.border));
    let part = move || {
        container(Space::new())
            .width(Length::Fixed(chrome_px(30.0)))
            .height(Length::Fixed(chrome_px(30.0)))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(paper)),
                border: Border {
                    width: 1.0,
                    color: ink,
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    };
    let wire = container(Space::new())
        .width(Length::Fixed(chrome_px(36.0)))
        .height(Length::Fixed(2.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(accent)),
            ..container::Style::default()
        })
        .boxed();
    let strip = container(Space::new())
        .width(Length::Fixed(chrome_px(8.0)))
        .height(Length::Fixed(chrome_px(8.0)))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(ink)),
            border: Border {
                radius: 2.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed();
    container(
        column![
            strip,
            container(
                row![part(), wire, part()]
                    .align_y(Alignment::Center)
                    .boxed()
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .boxed(),
        ]
        .spacing(SP.s)
        .boxed(),
    )
    .padding(SP.m)
    .width(Length::Fill)
    .height(Length::Fixed(chrome_px(96.0)))
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(paper)),
        border: Border {
            width: 1.0,
            color: rule,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}
