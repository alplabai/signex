//! Graphite start screen: a 232 px navigation column and a main column with
//! the "What are you building?" hero, the specification box (a shell: the AI
//! backend is not built), the bundled sample project and the recent list.
//! Every real control dispatches the same messages as the legacy screen.

use std::path::PathBuf;

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input, tooltip};
use iced::{Alignment, Background, Border, Color, Element, Length, Widget as _};
use signex_types::theme::ThemeTokens;

use super::{Action, recent_label, recent_message, visible_recent};
use crate::app::Message;
use crate::panels::signal::coming_soon;
use crate::styles::{
    self, ChromeMetrics, Radii, TypeScale, is_graphite, ti,
    tokens::{hover_overlay, resolve},
};

const TS: TypeScale = TypeScale::DEFAULT;
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
    label: String,
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
    open_shortcut: &str,
    recent_count: usize,
    libraries: &[super::LoadedLibrary],
) -> Element<'a, Message> {
    let open_label = if open_shortcut.is_empty() {
        "Open…".to_string()
    } else {
        format!("Open…  ({open_shortcut})")
    };
    let top = row![
        button(
            text("New project")
                .size(TS.dense)
                .wrapping(iced::widget::text::Wrapping::None)
        )
        .padding([0.0, 12.0])
        .width(Length::Shrink)
        .height(ChromeMetrics::DEFAULT.field + 2.0)
        .on_press(Action::NewProject.message())
        .style(styles::primary_button(tokens)),
        button(
            text(open_label)
                .size(TS.dense)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::None)
                .align_x(iced::alignment::Horizontal::Center)
        )
        .padding([0.0, 14.0])
        .width(Length::Fill)
        .height(ChromeMetrics::DEFAULT.field + 2.0)
        .on_press(Action::OpenProject.message())
        .style(outline_button(tokens)),
    ]
    .spacing(8);
    let items = column![
        nav_row(tokens, "Home".into(), None, true),
        nav_count_row(tokens, "Projects", Some(recent_count)),
        nav_count_row(tokens, "Libraries", Some(libraries.len())),
        coming_soon(
            nav_count_row(tokens, "Parts search", None),
            tokens,
            tooltip::Position::Right,
        ),
        nav_row(
            tokens,
            "Open Sample Project".into(),
            Some(Action::OpenSampleProject.message()),
            false
        ),
        nav_row(
            tokens,
            "Keyboard Shortcuts (F1)".into(),
            Some(Action::KeyboardShortcuts.message()),
            false
        ),
    ]
    .spacing(2);
    let loaded = libraries_card(tokens, libraries);
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

/// A navigation row with the real count at the right (none when unavailable).
/// Rows that open nothing yet are wrapped by the caller in the stub tooltip.
fn nav_count_row<'a>(
    tokens: &ThemeTokens,
    label: &'static str,
    count: Option<usize>,
) -> Element<'a, Message> {
    let (ink, ink3) = (ti(tokens.text), resolve(tokens).ink3);
    let mut line = row![text(label).size(TS.label).color(ink).boxed()].align_y(Alignment::Center);
    line = line.push(Space::new().width(Length::Fill).boxed());
    if let Some(n) = count {
        line = line.push(text(n.to_string()).size(TS.caption).color(ink3).boxed());
    }
    container(line)
        .padding([6.0, 14.0])
        .width(Length::Fill)
        .boxed()
}

/// The loaded libraries and their component count, from the library state.
fn libraries_card<'a>(
    tokens: &ThemeTokens,
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
    let summary = format!(
        "{} loaded \u{00B7} {} component{}",
        libraries.len(),
        components,
        if components == 1 { "" } else { "s" }
    );
    let mut body = column![
        text("Libraries").size(TS.label).color(ink).boxed(),
        text(summary).size(TS.dense).color(ink2).boxed(),
    ]
    .spacing(2);
    if !names.is_empty() {
        body = body.push(
            text(names)
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

fn section_header<'a>(tokens: &ThemeTokens, title: &str) -> Element<'a, Message> {
    text(title.to_uppercase())
        .size(TS.section)
        .style(styles::section_label(tokens))
        .boxed()
}

/// The specification box: a disabled shell, no sample text.
fn spec_box<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = resolve(tokens);
    let (ink4, ink) = (r.ink4, ti(tokens.text));
    let input: iced::widget::TextInput<'a, Message> = text_input(
        "Describe the board: what it does, its size, layers and budget.",
        "",
    )
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
    let stub = |label: &'a str, kind: u8| -> Element<'a, Message> {
        let b = button(text(label).size(TS.dense))
            .padding([0.0, 12.0])
            .height(ChromeMetrics::DEFAULT.field + 2.0);
        let b: Element<'a, Message> = if kind == 0 {
            b.style(styles::accent_button_rounded(tokens)).boxed()
        } else {
            b.style(styles::ghost_button(tokens)).boxed()
        };
        coming_soon(b, tokens, tooltip::Position::Top)
    };
    let actions = row![
        stub("Attach", 1),
        stub("Import", 1),
        stub("Constraints", 1),
        Space::new().width(Length::Fill).boxed(),
        stub("Generate design", 0),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let mode = |label: &'a str, on: bool| -> Element<'a, Message> {
        let (raised, ink, ink3) = (
            resolve(tokens).raised,
            ti(tokens.text),
            resolve(tokens).ink3,
        );
        let pill = container(
            text(label)
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
        coming_soon(pill.boxed(), tokens, tooltip::Position::Top)
    };
    let sunken = resolve(tokens).sunken;
    let modes = container(row![mode("Generator", true), mode("Assistant", false)].spacing(2))
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
        text("Signal uses your loaded libraries first")
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
fn sample_card<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let (ink, ink2) = (ti(tokens.text), ti(tokens.text_secondary));
    let body = column![
        text("Sample project").size(TS.label).color(ink),
        text("Open the bundled example project and look around.")
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

fn recent_list<'a>(tokens: &ThemeTokens, recent: &'a [PathBuf]) -> Element<'a, Message> {
    let shown = visible_recent(recent);
    if shown.is_empty() {
        return crate::panels::empty_state_card(
            tokens,
            "No recent projects",
            "Projects you open appear here.",
            None,
        );
    }
    let (ink, ink3) = (ti(tokens.text), resolve(tokens).ink3);
    let (hover, line) = (hover_overlay(tokens), resolve(tokens).line_soft);
    let tile_fill = resolve(tokens).sunken;
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
                text(recent_label(path)).size(TS.label).color(ink),
                text(path.display().to_string())
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
    container(list)
        .width(Length::Fill)
        .style(styles::card(tokens))
        .boxed()
}

/// The Graphite start screen. Call only when [`is_graphite`] holds.
pub fn view<'a>(
    tokens: &'a ThemeTokens,
    open_shortcut: &str,
    recent: &'a [PathBuf],
    libraries: &[super::LoadedLibrary],
) -> Element<'a, Message> {
    debug_assert!(is_graphite(tokens));
    let open_shortcut = open_shortcut.to_owned();
    let libraries = libraries.to_vec();
    // This iced fork has no max-width; the main column takes the room left
    // beside the navigation, up to `MAIN_MAX_W`.
    iced::widget::responsive(move |size| {
        let room = (size.width - NAV_W - 2.0 * MAIN_PAD_X).max(0.0);
        view_sized(
            tokens,
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
    open_shortcut: &str,
    recent: &'a [PathBuf],
    libraries: &[super::LoadedLibrary],
    main_w: f32,
) -> Element<'a, Message> {
    let main = column![
        text("What are you building?")
            .size(TS.display * 1.4)
            .color(ti(tokens.text)),
        text("Describe the board. Signal drafts the specification, architecture and parts, and you approve each stage before anything is drawn.")
            .size(TS.title)
            .color(ti(tokens.text_secondary)),
        Space::new().height(8.0),
        spec_box(tokens),
        Space::new().height(8.0),
        section_header(tokens, "Start from a reference design"),
        sample_card(tokens),
        Space::new().height(8.0),
        section_header(tokens, "Recent projects"),
        recent_list(tokens, recent),
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
