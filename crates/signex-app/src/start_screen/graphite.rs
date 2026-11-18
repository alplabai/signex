//! Graphite start screen: a navigation column and a main column with the
//! "What are you building?" hero, the bundled sample project and the project
//! lists (`projects.rs`). When the main column would be narrower than
//! [`MAIN_MIN_W`], the navigation collapses to the icon strip of
//! `nav_strip.rs` (#2346).
//! Every real control dispatches the same messages as the legacy screen.

use std::path::PathBuf;

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, tooltip};

use super::{Action, nav_strip, project_sections};
use crate::app::Message;
use crate::pinned_projects::PinnedProjects;
use crate::styles::{
    self, ChromeMetrics, LINE_HEIGHT, Radii, Spacing, TypeScale, ti,
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
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}
/// Width of the navigation column (the mock's 232 px, from the body size).
pub(super) const NAV_W: f32 = TS.body * 19.5;
/// Narrowest the area beside the full navigation may get (480 px at the
/// default base); below it the navigation collapses to the icon strip.
pub(super) const MAIN_MIN_W: f32 = TS.body * 40.0;
/// Vertical padding of a full navigation row.
const NAV_ROW_PAD_Y: f32 = 6.0;
/// Height of a navigation row: one label line and its padding. The icon
/// strip's rows take the same height.
pub(super) const NAV_ROW_H: f32 = TS.label * LINE_HEIGHT + 2.0 * NAV_ROW_PAD_Y;
/// Widest the main column gets.
const MAIN_MAX_W: f32 = 880.0;
/// Horizontal padding of the main column.
pub(super) const MAIN_PAD_X: f32 = 32.0;

/// Whether the navigation collapses to its icon strip in a start screen
/// `width` wide: the area beside the full navigation would be under
/// [`MAIN_MIN_W`].
pub(super) fn nav_collapsed(width: f32) -> bool {
    width - NAV_W < MAIN_MIN_W
}

/// Width the navigation takes in a start screen `width` wide.
pub(super) fn nav_width(width: f32) -> f32 {
    if nav_collapsed(width) {
        nav_strip::STRIP_W
    } else {
        NAV_W
    }
}

pub(super) fn mono() -> iced::Font {
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
pub(super) fn outline_button(
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

/// Look of a navigation row, full or in the icon strip; `active` marks the
/// current page.
pub(super) fn nav_row_style(
    tokens: &ThemeTokens,
    active: bool,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style + 'static {
    // The current page is a neutral chip, not the copper selection tint.
    let (hover, selected, ink) = (
        hover_overlay(tokens),
        resolve(tokens).sunken,
        ti(tokens.text),
    );
    move |_: &iced::Theme, status| button::Style {
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
    }
}

/// Fill and right-hand rule of the navigation column.
pub(super) fn nav_chrome(
    tokens: &ThemeTokens,
) -> impl Fn(&iced::Theme) -> container::Style + 'static {
    let chrome = ti(tokens.panel_bg);
    let line = rule(ti(tokens.border), true);
    move |theme: &iced::Theme| container::Style {
        background: Some(Background::Color(chrome)),
        ..line(theme)
    }
}

/// A navigation row; `active` marks the current page (not pressable).
fn nav_row<'a>(
    tokens: &ThemeTokens,
    label: Localized,
    action: Option<Message>,
    active: bool,
) -> Element<'a, Message> {
    let mut b = button(text(label).size(TS.label))
        .width(Length::Fill)
        .padding([NAV_ROW_PAD_Y, 14.0])
        .style(nav_row_style(tokens, active));
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
    .style(nav_chrome(tokens))
    .boxed()
}

/// The hairline between the page rows and the other navigation rows.
pub(super) fn nav_divider<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let line = ti(tokens.border);
    container(Space::new().height(styles::HAIRLINE))
        .width(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

/// Wrap a navigation row that opens nothing yet in the "Coming soon" tooltip.
fn coming_soon<'a>(
    content: Element<'a, Message>,
    tokens: &ThemeTokens,
    i18n: &I18n,
    position: tooltip::Position,
) -> Element<'a, Message> {
    let hint = container(
        text(t!(i18n, ids::PANELS_COMING_SOON))
            .size(TS.caption)
            .color(ti(tokens.text)),
    )
    .padding([3, 8])
    .style(signex_widgets::active_bar_dropdown::tooltip_style(
        ti(tokens.hover),
        ti(tokens.border),
    ));
    tooltip(content, hint, position).gap(4).boxed()
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
        .padding([NAV_ROW_PAD_Y, 14.0])
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
pub(super) fn section_header<'a>(tokens: &ThemeTokens, title: Localized) -> Element<'a, Message> {
    text(title)
        .size(TS.section)
        .font(semibold())
        .style(styles::section_label(tokens))
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

/// The Graphite start screen.
pub fn view<'a>(
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    open_shortcut: &str,
    recent: &'a [PathBuf],
    pinned: &'a PinnedProjects,
    libraries: &[super::LoadedLibrary],
) -> Element<'a, Message> {
    let open_shortcut = open_shortcut.to_owned();
    let libraries = libraries.to_vec();
    // The width decides the navigation's form, so the screen is laid out
    // per size.
    iced::widget::responsive(move |size| {
        view_sized(
            tokens,
            i18n,
            &open_shortcut,
            (recent, pinned),
            &libraries,
            size.width,
        )
    })
    .boxed()
}

fn view_sized<'a>(
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    open_shortcut: &str,
    (recent, pinned): (&'a [PathBuf], &'a PinnedProjects),
    libraries: &[super::LoadedLibrary],
    width: f32,
) -> Element<'a, Message> {
    // This iced fork has no max-width; the main column takes the room left
    // beside the navigation, up to `MAIN_MAX_W`.
    let main_w = (width - nav_width(width) - 2.0 * MAIN_PAD_X).clamp(0.0, MAIN_MAX_W);
    let sections = project_sections(recent, pinned);
    let listed = sections.pinned.len() + sections.recent.len();
    let mut main = column![
        text(t!(i18n, ids::START_HERO_TITLE))
            .size(HERO_TITLE_SIZE)
            .font(semibold())
            .color(ti(tokens.text)),
        text(t!(i18n, ids::START_HERO_BODY))
            .size(HERO_BODY_SIZE)
            .color(ti(tokens.text_secondary)),
        Space::new().height(8.0),
        section_header(tokens, t!(i18n, ids::START_REFERENCE_DESIGN)),
        sample_card(tokens, i18n),
        Space::new().height(8.0),
    ];
    for section in super::projects::sections(tokens, i18n, &sections, pinned) {
        main = main.push(section);
    }
    let main = main.spacing(10).width(Length::Fixed(main_w));
    let body = scrollable(
        container(main)
            .padding([40.0, MAIN_PAD_X])
            .center_x(Length::Fill),
    );
    let navigation = if nav_collapsed(width) {
        nav_strip::view(tokens, i18n, open_shortcut)
    } else {
        nav(tokens, i18n, open_shortcut, listed, libraries)
    };
    row![
        navigation,
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(styles::panel_region(tokens)),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}
