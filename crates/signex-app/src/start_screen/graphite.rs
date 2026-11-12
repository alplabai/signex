//! Graphite start screen: a 232 px navigation column and a main column with
//! the "What are you building?" hero, the specification box (a shell: the AI
//! backend is not built), the bundled sample project and the recent list.
//! Every real control dispatches the same messages as the legacy screen.

use std::path::PathBuf;

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, text_input, tooltip};

use super::{Action, recent_label, recent_message, visible_recent};
use crate::app::Message;
use crate::panels::signal::coming_soon;
use crate::styles::{
    self, ChromeMetrics, Radii, Spacing, TypeScale, ti,
    tokens::{hover_overlay, resolve},
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// The hero title ("What are you building?").
pub(super) const HERO_TITLE_SIZE: f32 = TS.display * 1.4;
/// The line under the hero title.
pub(super) const HERO_BODY_SIZE: f32 = TS.heading;

/// The semibold UI face of the hero title and the section labels.
pub(super) fn semibold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..iced::Font::DEFAULT
    }
}
/// Width of the navigation column (the mock's 232 px).
const NAV_W: f32 = 232.0;
/// Widest the main column gets.
const MAIN_MAX_W: f32 = 880.0;
/// Horizontal padding of the main column.
const MAIN_PAD_X: f32 = 32.0;

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

fn rule(line: Color, right: bool) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| {
        let side = iced::border::Side::default().width(1.0).color(line);
        let mut border = Border::default();
        if right {
            border.right = side;
        } else {
            border.bottom = side;
        }
        container::Style {
            border,
            ..container::Style::default()
        }
    }
}

/// Quiet bordered button ("Open…"): raised fill, hairline border.
fn outline_button(
    tokens: &ThemeTokens,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style + 'static {
    let (raised, hover, line, ink) = (
        resolve(tokens).raised,
        hover_overlay(tokens),
        ti(tokens.border),
        ti(tokens.text),
    );
    move |_, status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered | button::Status::Pressed => {
                styles::tokens::composite_over(hover, raised)
            }
            _ => raised,
        })),
        text_color: ink,
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// A navigation row; `active` marks the current page (not pressable).
fn nav_row<'a>(
    tokens: &ThemeTokens,
    label: Localized,
    action: Option<Message>,
    active: bool,
) -> Element<'a, Message> {
    // The current page is a neutral chip, not the copper selection tint.
    let (hover, selected, ink) = (
        hover_overlay(tokens),
        resolve(tokens).sunken,
        ti(tokens.text),
    );
    let mut b = button(text(label).size(TS.label))
        .width(Length::Fill)
        .padding([6.0, 14.0])
        .style(move |_: &iced::Theme, status| button::Style {
            background: if active {
                Some(Background::Color(selected))
            } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                Some(Background::Color(hover))
            } else {
                None
            },
            text_color: ink,
            border: Border {
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    if let Some(message) = action {
        b = b.on_press(message);
    }
    b.boxed()
}

fn nav<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    open_shortcut: &str,
    recent_count: usize,
    libraries: &[super::LoadedLibrary],
) -> Element<'a, Message> {
    let cap_fill = resolve(tokens).sunken;
    let cap_ink = resolve(tokens).ink3;
    let open_label: Element<'a, Message> = {
        let label = text(t!(i18n, ids::START_OPEN))
            .size(TS.dense)
            .wrapping(iced::widget::text::Wrapping::None);
        let mut line = row![label.boxed()].spacing(8).align_y(Alignment::Center);
        if !open_shortcut.is_empty() {
            line = line.push(
                container(
                    text(UserText::new(open_shortcut))
                        .size(TS.caption)
                        .font(mono())
                        .color(cap_ink)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .padding([1.0, 6.0])
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(Background::Color(cap_fill)),
                    border: Border {
                        radius: Radii::GRAPHITE.small.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed(),
            );
        }
        container(line)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .boxed()
    };
    let top = row![
        button(
            text(t!(i18n, ids::START_NEW_PROJECT))
                .size(TS.dense)
                .height(Length::Fill)
                .align_y(iced::alignment::Vertical::Center)
                .wrapping(iced::widget::text::Wrapping::None)
        )
        .padding([0.0, 12.0])
        .width(Length::Shrink)
        .height(ChromeMetrics::DEFAULT.field + 2.0)
        .on_press(Action::NewProject.message())
        .style(styles::primary_button(tokens)),
        button(open_label)
            .padding([0.0, 14.0])
            .width(Length::Fill)
            .height(ChromeMetrics::DEFAULT.field + 2.0)
            .on_press(Action::OpenProject.message())
            .style(outline_button(tokens)),
    ]
    .spacing(8);
    let items = column![
        nav_row(tokens, t!(i18n, ids::START_HOME), None, true),
        nav_count_row(tokens, i18n, ids::START_PROJECTS, Some(recent_count)),
        nav_count_row(tokens, i18n, ids::START_LIBRARIES, Some(libraries.len())),
        coming_soon(
            nav_count_row(tokens, i18n, ids::START_PARTS_SEARCH, None),
            tokens,
            i18n,
            tooltip::Position::Right,
        ),
        // The mock's divider; the two app-only rows sit under it.
        container(nav_divider(tokens)).padding([SP.m, SP.l]),
        nav_row(
            tokens,
            t!(i18n, ids::START_OPEN_SAMPLE),
            Some(Action::OpenSampleProject.message()),
            false
        ),
        nav_row(
            tokens,
            t!(i18n, ids::START_KEYBOARD_SHORTCUTS),
            Some(Action::KeyboardShortcuts.message()),
            false
        ),
    ]
    .spacing(2);
    let loaded = libraries_card(tokens, i18n, libraries);
    container(column![
        top,
        Space::new().height(10.0),
        items,
        Space::new().height(Length::Fill),
        loaded,
    ])
    .padding([14.0, 10.0])
    .width(NAV_W)
    .height(Length::Fill)
    .style({
        let chrome = ti(tokens.panel_bg);
        let line = rule(ti(tokens.border), true);
        move |theme: &iced::Theme| container::Style {
            background: Some(Background::Color(chrome)),
            ..line(theme)
        }
    })
    .boxed()
}

/// The hairline between the page rows and the other navigation rows.
fn nav_divider<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let line = ti(tokens.border);
    container(Space::new().height(styles::HAIRLINE))
        .width(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

/// A navigation row with the real count at the right (none when unavailable).
/// Rows that open nothing yet are wrapped by the caller in the stub tooltip.
fn nav_count_row<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    label: MessageId,
    count: Option<usize>,
) -> Element<'a, Message> {
    let (ink, ink3) = (ti(tokens.text), resolve(tokens).ink3);
    let mut line =
        row![text(t!(i18n, label)).size(TS.label).color(ink).boxed()].align_y(Alignment::Center);
    line = line.push(Space::new().width(Length::Fill).boxed());
    if let Some(n) = count {
        line = line.push(
            text(t!(i18n, ids::START_COUNT, "count" => n))
                .size(TS.caption)
                .color(ink3)
                .boxed(),
        );
    }
    container(line)
        .padding([6.0, 14.0])
        .width(Length::Fill)
        .boxed()
}

/// The loaded libraries and their component count, from the library state.
fn libraries_card<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    libraries: &[super::LoadedLibrary],
) -> Element<'a, Message> {
    let (ink, ink2, ink3) = (
        ti(tokens.text),
        ti(tokens.text_secondary),
        resolve(tokens).ink3,
    );
    let components: usize = libraries.iter().map(|(_, n)| n).sum();
    let names = libraries
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>()
        .join(" \u{00B7} ");
    let summary = t!(
        i18n,
        ids::START_LIBRARIES_SUMMARY,
        "libraries" => libraries.len(),
        "components" => components
    );
    let mut body = column![
        text(t!(i18n, ids::START_LIBRARIES))
            .size(TS.label)
            .color(ink)
            .boxed(),
        text(summary).size(TS.dense).color(ink2).boxed(),
    ]
    .spacing(2);
    if !names.is_empty() {
        body = body.push(
            text(signex_i18n::UserText::new(names))
                .size(TS.caption)
                .font(mono())
                .color(ink3)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    }
    container(body)
        .padding(12)
        .width(Length::Fill)
        .clip(true)
        .style(styles::card(tokens))
        .boxed()
}

/// A section heading; the catalog writes it in capitals. Semibold as in the
/// mock; iced has no letter spacing, so the mock's tracking is not drawn.
fn section_header<'a>(tokens: &ThemeTokens, title: Localized) -> Element<'a, Message> {
    text(title)
        .size(TS.section)
        .font(semibold())
        .style(styles::section_label(tokens))
        .boxed()
}

/// The specification box: a disabled shell, no sample text.
fn spec_box<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, Message> {
    let r = resolve(tokens);
    let (ink4, ink) = (r.ink4, ti(tokens.text));
    let input: iced::widget::TextInput<'a, Message> =
        text_input(t!(i18n, ids::START_SPEC_PLACEHOLDER), "")
            .padding(iced::Padding::ZERO)
            .size(TS.label)
            .width(Length::Fill)
            .style(move |_: &iced::Theme, _| text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                placeholder: ink4,
                value: ink,
                selection: Color { a: 0.4, ..ink },
            });
    let stub = |label: MessageId, kind: u8| -> Element<'a, Message> {
        let b = button(
            text(t!(i18n, label))
                .size(TS.dense)
                .height(Length::Fill)
                .align_y(iced::alignment::Vertical::Center),
        )
        .padding([0.0, 12.0])
        .height(ChromeMetrics::DEFAULT.field + 2.0);
        let b: Element<'a, Message> = if kind == 0 {
            b.style(styles::accent_button_rounded(tokens)).boxed()
        } else {
            b.style(outline_button(tokens)).boxed()
        };
        coming_soon(b, tokens, i18n, tooltip::Position::Top)
    };
    let actions = row![
        stub(ids::START_SPEC_ATTACH, 1),
        stub(ids::START_SPEC_IMPORT, 1),
        stub(ids::START_SPEC_CONSTRAINTS, 1),
        Space::new().width(Length::Fill).boxed(),
        stub(ids::START_SPEC_GENERATE, 0),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let mode = |label: MessageId, on: bool| -> Element<'a, Message> {
        let (raised, ink, ink3) = (
            resolve(tokens).raised,
            ti(tokens.text),
            resolve(tokens).ink3,
        );
        let pill =
            container(
                text(t!(i18n, label))
                    .size(TS.dense)
                    .color(if on { ink } else { ink3 }),
            )
            .padding([4.0, 14.0])
            .style(move |_: &iced::Theme| container::Style {
                background: on.then_some(Background::Color(raised)),
                border: Border {
                    radius: Radii::GRAPHITE.control.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });
        coming_soon(pill.boxed(), tokens, i18n, tooltip::Position::Top)
    };
    let sunken = resolve(tokens).sunken;
    let modes = container(
        row![
            mode(ids::START_SPEC_GENERATOR, true),
            mode(ids::START_SPEC_ASSISTANT, false)
        ]
        .spacing(2),
    )
    .padding(2)
    .style(move |_: &iced::Theme| container::Style {
        background: Some(Background::Color(sunken)),
        border: Border {
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let head = row![
        modes,
        Space::new().width(Length::Fill),
        text(t!(i18n, ids::START_SPEC_LIBRARIES_FIRST))
            .size(TS.dense)
            .color(resolve(tokens).ink3),
    ]
    .align_y(Alignment::Center);
    container(
        column![
            container(head).padding([10.0, 12.0]),
            container(input).padding([6.0, 18.0]).height(96.0),
            container(actions)
                .padding([10.0, 12.0])
                .width(Length::Fill)
                .style(styles::resize_handle_edge(tokens, false)),
        ]
        .spacing(0),
    )
    .width(Length::Fill)
    .style(styles::card(tokens))
    .boxed()
}

/// The bundled sample project as a real card: it opens through File ▸ Open
/// Sample Project.
fn sample_card<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, Message> {
    let (ink, ink2) = (ti(tokens.text), ti(tokens.text_secondary));
    let body = column![
        text(t!(i18n, ids::START_SAMPLE_TITLE))
            .size(TS.label)
            .color(ink),
        text(t!(i18n, ids::START_SAMPLE_BODY))
            .size(TS.dense)
            .color(ink2),
    ]
    .spacing(4);
    let card = styles::card(tokens);
    let hover = hover_overlay(tokens);
    button(body)
        .width(Length::Fill)
        .padding(14)
        .on_press(Action::OpenSampleProject.message())
        .style(move |theme: &iced::Theme, status| {
            let base = card(theme);
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: if hovered {
                    Some(Background::Color(styles::tokens::composite_over(
                        hover,
                        match base.background {
                            Some(Background::Color(c)) => c,
                            _ => Color::TRANSPARENT,
                        },
                    )))
                } else {
                    base.background
                },
                text_color: ink,
                border: base.border,
                ..button::Style::default()
            }
        })
        .boxed()
}

fn recent_list<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    recent: &'a [PathBuf],
) -> Element<'a, Message> {
    let shown = visible_recent(recent);
    if shown.is_empty() {
        return crate::panels::empty_state_card(
            tokens,
            t!(i18n, ids::START_NO_RECENT_TITLE),
            t!(i18n, ids::START_NO_RECENT_BODY),
            None,
        );
    }
    let (ink, ink3) = (ti(tokens.text), resolve(tokens).ink3);
    let (hover, line) = (hover_overlay(tokens), resolve(tokens).line_soft);
    let tile_fill = resolve(tokens).sunken;
    let mut separators = 0;
    let mut list = column![].width(Length::Fill);
    for path in shown {
        // No cheap real thumbnail exists, so the mock's tile is drawn empty.
        let tile =
            container(Space::new())
                .width(52.0)
                .height(40.0)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(Background::Color(tile_fill)),
                    border: Border {
                        radius: Radii::GRAPHITE.small.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                });
        let entry = row![
            tile,
            column![
                text(UserText::new(recent_label(path)))
                    .size(TS.label)
                    .color(ink),
                text(signex_i18n::UserText::new(path.display().to_string()))
                    .size(TS.caption)
                    .font(mono())
                    .color(ink3),
            ]
            .spacing(2),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        list = list.push(
            button(entry)
                .width(Length::Fill)
                .padding([10.0, 14.0])
                .on_press(recent_message(path))
                .style(move |_: &iced::Theme, status| button::Style {
                    background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                        .then_some(Background::Color(hover)),
                    text_color: ink,
                    border: Border {
                        radius: Radii::GRAPHITE.small.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                })
                .boxed(),
        );
        separators += 1;
        if separators < shown.len() {
            list = list.push(
                container(Space::new().height(1.0))
                    .width(Length::Fill)
                    .style(move |_: &iced::Theme| container::Style {
                        background: Some(Background::Color(line)),
                        ..container::Style::default()
                    })
                    .boxed(),
            );
        }
    }
    container(list)
        .width(Length::Fill)
        .padding(1)
        .style(styles::card(tokens))
        .boxed()
}

/// The Graphite start screen.
pub fn view<'a>(
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    open_shortcut: &str,
    recent: &'a [PathBuf],
    libraries: &[super::LoadedLibrary],
) -> Element<'a, Message> {
    let open_shortcut = open_shortcut.to_owned();
    let libraries = libraries.to_vec();
    // This iced fork has no max-width; the main column takes the room left
    // beside the navigation, up to `MAIN_MAX_W`.
    iced::widget::responsive(move |size| {
        let room = (size.width - NAV_W - 2.0 * MAIN_PAD_X).max(0.0);
        view_sized(
            tokens,
            i18n,
            &open_shortcut,
            recent,
            &libraries,
            room.min(MAIN_MAX_W),
        )
    })
    .boxed()
}

fn view_sized<'a>(
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    open_shortcut: &str,
    recent: &'a [PathBuf],
    libraries: &[super::LoadedLibrary],
    main_w: f32,
) -> Element<'a, Message> {
    let main = column![
        text(t!(i18n, ids::START_HERO_TITLE))
            .size(HERO_TITLE_SIZE)
            .font(semibold())
            .color(ti(tokens.text)),
        text(t!(i18n, ids::START_HERO_BODY))
            .size(HERO_BODY_SIZE)
            .color(ti(tokens.text_secondary)),
        Space::new().height(8.0),
        spec_box(tokens, i18n),
        Space::new().height(8.0),
        section_header(tokens, t!(i18n, ids::START_REFERENCE_DESIGN)),
        sample_card(tokens, i18n),
        Space::new().height(8.0),
        section_header(tokens, t!(i18n, ids::START_RECENT_PROJECTS)),
        recent_list(tokens, i18n, recent),
    ]
    .spacing(10)
    .width(Length::Fixed(main_w));
    let body = scrollable(
        container(main)
            .padding([40.0, MAIN_PAD_X])
            .center_x(Length::Fill),
    );
    row![
        nav(
            tokens,
            i18n,
            open_shortcut,
            visible_recent(recent).len(),
            libraries,
        ),
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::panel_region(tokens)),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}
