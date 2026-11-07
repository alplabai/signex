//! Graphite Theme page: three preview cards for the headline themes and a
//! grid of swatch cards for the other built-ins. Choosing a card sends the
//! same `DraftTheme` the old picker sent; nothing is written to disk.

use super::{PrefMsg, SP, TS};
use crate::styles::{self, Radii, chrome_px, ti, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};

/// A headline theme: id, card title, tag on the right, description.
const FEATURED: [(ThemeId, &str, &str, &str); 3] = [
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
    (
        ThemeId::Signex,
        "Altium Dark",
        "Classic",
        "Familiar grey chrome and orange accent from Altium Designer.",
    ),
];

/// Swatch cards per row of the built-in grid.
const SWATCHES_PER_ROW: usize = 4;

/// The Theme heading, the three preview cards and the swatch grid.
/// `active` carries the dialog's own tokens (labels, custom theme card).
pub(super) fn theme_section<'a>(
    draft: ThemeId,
    custom_name: Option<&str>,
    active: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(active);
    let label = |s: &str| {
        text(s.to_uppercase())
            .size(TS.section)
            .style(styles::section_label(active))
    };
    let mut col = column![
        text("Theme").size(TS.heading).color(ti(active.text)),
        text("Colours, density and canvas text. Changes preview live; Apply keeps them.")
            .size(TS.label)
            .color(r.ink3),
        Space::new().height(SP.l),
        label("Application theme"),
    ]
    .spacing(SP.s);

    let mut featured = row![].spacing(SP.l);
    for (id, name, tag, desc) in FEATURED {
        featured = featured.push(preview_card(id, name, tag, desc, draft, active));
    }
    col = col.push(featured);
    col = col.push(Space::new().height(SP.l));
    col = col.push(label("Built-in themes"));

    let others: Vec<ThemeId> = ThemeId::BUILTINS
        .iter()
        .copied()
        .filter(|id| !FEATURED.iter().any(|f| f.0 == *id))
        .collect();
    for chunk in others.chunks(SWATCHES_PER_ROW) {
        let mut line = row![].spacing(SP.m);
        for &id in chunk {
            line = line.push(swatch_card(id, id.label(), draft, theme_tokens(id), active));
        }
        for _ in chunk.len()..SWATCHES_PER_ROW {
            line = line.push(Space::new().width(Length::FillPortion(1)));
        }
        col = col.push(line);
    }
    if let Some(name) = custom_name {
        let line = row![swatch_card(
            ThemeId::Custom,
            name,
            draft,
            active.clone(),
            active
        )]
        .spacing(SP.m);
        col = col.push(line);
    }
    col.into()
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
        });
    let body = column![
        mini_preview(&t),
        row![
            radio,
            text(name.to_string()).size(TS.title).color(ink),
            Space::new().width(Length::Fill),
            text(tag.to_string()).size(TS.caption).color(a.ink3),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center),
        text(desc.to_string()).size(TS.body).color(a.ink3),
    ]
    .spacing(SP.m);
    let raised = a.raised;
    button(container(body).padding(SP.l))
        .padding(0)
        .width(Length::FillPortion(1))
        .on_press(PrefMsg::DraftTheme(id))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(raised)),
            text_color: ink,
            border: card_border(selected, accent, line, Radii::GRAPHITE.card),
            ..button::Style::default()
        })
        .into()
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
    };
    let wire = container(Space::new())
        .width(Length::Fixed(chrome_px(36.0)))
        .height(Length::Fixed(2.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(accent)),
            ..container::Style::default()
        });
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
        });
    container(
        column![
            strip,
            container(row![part(), wire, part()].align_y(Alignment::Center))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        ]
        .spacing(SP.s),
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
    .into()
}

/// One built-in swatch: four colour blocks of `t` over the theme's name.
fn swatch_card<'a>(
    id: ThemeId,
    name: &str,
    draft: ThemeId,
    t: ThemeTokens,
    active: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let selected = id == draft;
    let a = tokens::resolve(active);
    let (accent, line, ink, raised) = (ti(active.accent), a.line_soft, ti(active.text), a.raised);
    let block = |c: Color, w: u16| {
        container(Space::new())
            .width(Length::FillPortion(w))
            .height(Length::Fixed(chrome_px(32.0)))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(c)),
                ..container::Style::default()
            })
    };
    let swatch = container(
        row![
            block(ti(t.bg), 2),
            block(ti(t.panel_bg), 2),
            block(ti(t.paper), 2),
            block(ti(t.accent), 1),
        ]
        .width(Length::Fill),
    )
    .clip(true)
    .style(move |_: &Theme| container::Style {
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    button(
        container(
            column![
                swatch,
                text(name.to_string())
                    .size(TS.body)
                    .color(ink)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(SP.s),
        )
        .padding(SP.m),
    )
    .padding(0)
    .width(Length::FillPortion(1))
    .on_press(PrefMsg::DraftTheme(id))
    .style(move |_: &Theme, _| button::Style {
        background: Some(Background::Color(raised)),
        text_color: ink,
        border: card_border(selected, accent, line, Radii::GRAPHITE.card),
        ..button::Style::default()
    })
    .into()
}
