//! Graphite start screen (mockup row 1): a navigation column and a main column
//! showing the selected page. Home has the "What are you building?" hero,
//! the bundled sample project and the project lists (`projects.rs`); the
//! Projects and Libraries pages live in `projects_page.rs` and
//! `libraries_page.rs`. When the main column would be narrower than
//! [`MAIN_MIN_W`], the navigation collapses to the icon strip of
//! `nav_strip.rs` (#2346).

use iced::widget::{Space, column, container, row, scrollable, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, tooltip};

use super::kit::{
    GLYPH_THEME, ICON, Look, Size, action_button, icon, mono, outline_button, pill, section_header,
    weighted,
};
use super::projects::{Rows, home_sections};
use super::{
    Action, StartPage, StartScreenData, StartScreenMsg, libraries_page, message, nav_strip,
    project_sections, projects_page,
};
use crate::app::Message;
use crate::styles::{
    self, ChromeMetrics, LINE_HEIGHT, PillTone, Radii, Spacing, TypeScale, ti,
    tokens::{composite_over, hover_overlay, resolve},
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// The hero title ("What are you building?").
pub(super) const HERO_TITLE_SIZE: f32 = TS.display * 1.4;
/// The line under the hero title.
pub(super) const HERO_BODY_SIZE: f32 = TS.heading;

/// The semibold UI face of the hero title and the section labels.
pub(super) fn semibold() -> iced::Font {
    weighted(iced::font::Weight::Semibold)
}
/// Width of the navigation column (the mock's 232 px, from the body size).
pub(super) const NAV_W: f32 = TS.body * 19.5;
/// Narrowest the area beside the full navigation may get (480 px at the
/// default base); below it the navigation collapses to the icon strip.
pub(super) const MAIN_MIN_W: f32 = TS.body * 40.0;
/// Vertical padding of a full navigation row.
const NAV_ROW_PAD_Y: f32 = 6.0;
/// Horizontal padding of a full navigation row (10 px at the default base).
const NAV_ROW_PAD_X: f32 = SP.m + SP.xs / 2.0;
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
    let line = ti(tokens.border);
    move |_: &iced::Theme| {
        let mut border = Border::default();
        border.right = iced::border::Side::default().width(1.0).color(line);
        container::Style {
            background: Some(Background::Color(chrome)),
            border,
            ..container::Style::default()
        }
    }
}

/// The page a navigation row opens, and what it sends: nothing for the
/// page already on screen.
pub(super) fn page_action(current: StartPage, page: StartPage) -> Option<Message> {
    (current != page).then(|| message(StartScreenMsg::PageSelected(page)))
}

/// One full navigation row: its glyph, its label, an optional right-hand
/// cell (a count, a key, a pill) and what it sends.
struct NavRow<'a> {
    glyph: svg::Handle,
    label: Localized,
    right: Option<Element<'a, Message>>,
    action: Option<Message>,
    active: bool,
    /// Greyed: a row that opens nothing yet.
    planned: bool,
}

fn nav_row<'a>(tokens: &ThemeTokens, row_data: NavRow<'a>) -> Element<'a, Message> {
    let (ink, ink3, ink4) = (ti(tokens.text), resolve(tokens).ink3, resolve(tokens).ink4);
    let mut label = text(row_data.label)
        .size(TS.label)
        .color(if row_data.planned { ink4 } else { ink })
        .wrapping(iced::widget::text::Wrapping::None);
    if row_data.active {
        label = label.font(weighted(iced::font::Weight::Medium));
    }
    let glyph_ink = if row_data.planned { ink4 } else { ink3 };
    let mut line = row![icon(row_data.glyph, ICON, glyph_ink), label.boxed()]
        .spacing(SP.m)
        .align_y(Alignment::Center);
    if let Some(right) = row_data.right {
        line = line
            .push(Space::new().width(Length::Fill).boxed())
            .push(right);
    }
    let mut b = button(line)
        .width(Length::Fill)
        .padding([NAV_ROW_PAD_Y, NAV_ROW_PAD_X])
        .style(nav_row_style(tokens, row_data.active));
    if let Some(message) = row_data.action {
        b = b.on_press(message);
    }
    b.boxed()
}

/// A navigation row's count or key, right-aligned in mono.
fn nav_cell<'a>(tokens: &ThemeTokens, value: Localized) -> Element<'a, Message> {
    text(value)
        .size(TS.caption)
        .font(mono())
        .color(resolve(tokens).ink4)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// New project and Open… at the top of the navigation, stacked full width.
fn nav_buttons<'a>(tokens: &ThemeTokens, i18n: &I18n, open_shortcut: &str) -> Element<'a, Message> {
    let height = ChromeMetrics::DEFAULT.button;
    let primary_ink = resolve(tokens).primary_ink;
    let new_label = row![
        icon(crate::icons::icon_plus(GLYPH_THEME), ICON, primary_ink),
        text(t!(i18n, ids::START_NEW_PROJECT))
            .size(TS.dense)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Center)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    ]
    .spacing(SP.s + SP.xs / 2.0)
    .align_y(Alignment::Center);
    let new = button(container(new_label).center_x(Length::Fill))
        .padding([0.0, SP.l])
        .width(Length::Fill)
        .height(height)
        .on_press(Action::NewProject.message())
        .style(styles::primary_button(tokens));
    let mut open_label = row![
        text(t!(i18n, ids::START_OPEN))
            .size(TS.dense)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed()
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    if !open_shortcut.is_empty() {
        open_label = open_label.push(
            container(
                text(UserText::new(open_shortcut))
                    .size(TS.caption)
                    .font(mono())
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .padding([1.0, SP.s + SP.xs / 2.0])
            .style(styles::key_cap(tokens))
            .boxed(),
        );
    }
    let open = button(container(open_label).center(Length::Fill))
        .padding([0.0, SP.l])
        .width(Length::Fill)
        .height(height)
        .on_press(Action::OpenProject.message())
        .style(outline_button(tokens));
    column![new, open]
        .spacing(SP.s + SP.xs)
        .padding([0.0, SP.xs])
        .boxed()
}

fn nav<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    data: &StartScreenData<'a>,
    listed: usize,
) -> Element<'a, Message> {
    let current = data.state.page;
    let count = |n: usize| Some(nav_cell(tokens, t!(i18n, ids::START_COUNT, "count" => n)));
    let page = |glyph: svg::Handle,
                label: MessageId,
                right: Option<Element<'a, Message>>,
                page: StartPage| NavRow {
        glyph,
        label: t!(i18n, label),
        right,
        action: page_action(current, page),
        active: current == page,
        planned: false,
    };
    let parts = NavRow {
        glyph: crate::icons::icon_chrome_search(GLYPH_THEME),
        label: t!(i18n, ids::START_PARTS_SEARCH),
        right: Some(pill(tokens, t!(i18n, ids::START_SOON), PillTone::Neutral)),
        action: None,
        active: false,
        planned: true,
    };
    let shortcuts = NavRow {
        glyph: crate::icons::icon_start_key(GLYPH_THEME),
        label: t!(i18n, ids::START_KEYBOARD_SHORTCUTS),
        right: (!data.shortcuts_shortcut.is_empty()).then(|| {
            let key = data.shortcuts_shortcut.clone();
            nav_cell(tokens, t!(i18n, ids::START_ROW_KEY, "key" => key))
        }),
        action: Some(Action::KeyboardShortcuts.message()),
        active: false,
        planned: false,
    };
    let items = column![
        nav_row(
            tokens,
            page(
                crate::icons::icon_start_home(GLYPH_THEME),
                ids::START_HOME,
                None,
                StartPage::Home
            )
        ),
        nav_row(
            tokens,
            page(
                crate::icons::icon_start_folder(GLYPH_THEME),
                ids::START_PROJECTS,
                count(listed),
                StartPage::Projects
            )
        ),
        nav_row(
            tokens,
            page(
                crate::icons::icon_start_book(GLYPH_THEME),
                ids::START_LIBRARIES,
                count(data.libraries.len()),
                StartPage::Libraries
            )
        ),
        coming_soon(
            nav_row(tokens, parts),
            tokens,
            i18n,
            tooltip::Position::Right
        ),
        // The mock's divider; Keyboard Shortcuts sits under it.
        container(nav_divider(tokens)).padding([SP.m, SP.s + SP.xs / 2.0]),
        nav_row(tokens, shortcuts),
    ]
    .spacing(SP.xs);
    container(column![
        nav_buttons(tokens, i18n, &data.open_shortcut),
        Space::new().height(SP.m + SP.xs / 2.0),
        items,
        Space::new().height(Length::Fill),
        libraries_card(tokens, i18n, &data.libraries),
    ])
    .padding([SP.l, SP.m])
    .width(NAV_W)
    .height(Length::Fill)
    .style(nav_chrome(tokens))
    .boxed()
}

/// The hairline between the page rows and the other navigation rows.
pub(super) fn nav_divider<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    super::kit::hairline(resolve(tokens).line_soft)
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

/// The loaded libraries and their component count, from the library state;
/// it opens the Libraries page.
fn libraries_card<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    libraries: &[super::LoadedLibrary],
) -> Element<'a, Message> {
    let (ink, ink4) = (ti(tokens.text), resolve(tokens).ink4);
    let components: usize = libraries.iter().map(|lib| lib.components).sum();
    let summary = t!(
        i18n,
        ids::START_LIBRARIES_SUMMARY,
        "libraries" => libraries.len(),
        "components" => components
    );
    let body = column![
        text(t!(i18n, ids::START_LIBRARIES))
            .size(TS.body)
            .font(weighted(iced::font::Weight::Medium))
            .color(ink),
        text(summary).size(TS.caption).color(ink4),
    ]
    .spacing(SP.xs);
    let (fill, line, strong) = (
        ti(tokens.bg),
        ti(tokens.border),
        resolve(tokens).line_strong,
    );
    button(body)
        .width(Length::Fill)
        .padding([SP.m + SP.xs / 2.0, SP.l])
        .on_press(message(StartScreenMsg::PageSelected(StartPage::Libraries)))
        .style(move |_: &iced::Theme, status| button::Style {
            background: Some(Background::Color(fill)),
            text_color: ink,
            border: Border {
                width: 1.0,
                color: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                    strong
                } else {
                    line
                },
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .boxed()
}

/// The bundled sample project as a real card: its thumbnail, title, line
/// and Open; the card and Open both open it through File ▸ Open Sample
/// Project.
fn sample_card<'a>(
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    data: &StartScreenData<'a>,
) -> Element<'a, Message> {
    let (ink, ink3) = (ti(tokens.text), resolve(tokens).ink3);
    let slot = match &data.state.sample_project {
        Some(path) => data.thumbnails.slot(path, false),
        None => crate::project_thumbnail::ThumbnailSlot::NoPreview,
    };
    let body = column![
        text(t!(i18n, ids::START_SAMPLE_TITLE))
            .size(TS.body)
            .font(semibold())
            .color(ink),
        text(t!(i18n, ids::START_SAMPLE_BODY))
            .size(TS.body)
            .color(ink3),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    let open = action_button(
        tokens,
        (Look::Outline, Size::Regular),
        None,
        t!(i18n, ids::START_OPEN_PROJECT),
        Some(Action::OpenSampleProject.message()),
    );
    let content = row![
        crate::project_thumbnail::tile(tokens, i18n, slot),
        body.boxed(),
        open
    ]
    .spacing(SP.l + SP.xs / 2.0)
    .align_y(Alignment::Center);
    let card = styles::card(tokens);
    let hover = hover_overlay(tokens);
    button(content)
        .width(Length::Fill)
        .padding([SP.l, SP.l + SP.xs / 2.0])
        .on_press(Action::OpenSampleProject.message())
        .style(move |theme: &iced::Theme, status| {
            let base = card(theme);
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: if hovered {
                    Some(Background::Color(composite_over(
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
    data: StartScreenData<'a>,
) -> Element<'a, Message> {
    // The width decides the navigation's form, so the screen is laid out
    // per size.
    iced::widget::responsive(move |size| view_sized(tokens, i18n, &data, size.width)).boxed()
}

/// Home: the hero, the reference design and the project lists.
fn home<'a>(
    rows: Rows<'a>,
    data: &StartScreenData<'a>,
    listed: usize,
) -> Vec<Element<'a, Message>> {
    let (tokens, i18n) = (rows.tokens, rows.i18n);
    let sections = project_sections(data.recent.entries(), data.pinned);
    let mut out = vec![
        text(t!(i18n, ids::START_HERO_TITLE))
            .size(HERO_TITLE_SIZE)
            .font(semibold())
            .color(ti(tokens.text))
            .boxed(),
        text(t!(i18n, ids::START_HERO_BODY))
            .size(HERO_BODY_SIZE)
            .color(resolve(tokens).ink3)
            .boxed(),
        Space::new().height(SP.m).boxed(),
        section_header(tokens, t!(i18n, ids::START_REFERENCE_DESIGN)),
        sample_card(tokens, i18n, data),
        Space::new().height(SP.m).boxed(),
    ];
    out.extend(home_sections(rows, &sections, data.pinned, listed));
    out
}

fn view_sized<'a>(
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
    data: &StartScreenData<'a>,
    width: f32,
) -> Element<'a, Message> {
    // This iced fork has no max-width; the main column takes the room left
    // beside the navigation, up to `MAIN_MAX_W`.
    let main_w = (width - nav_width(width) - 2.0 * MAIN_PAD_X).clamp(0.0, MAIN_MAX_W);
    let rows = Rows {
        tokens,
        i18n,
        recent: data.recent,
        thumbnails: data.thumbnails,
        now: data.now,
    };
    let listed = projects_page::scope_counts(data.recent.entries(), data.pinned).all;
    let (content, spacing) = match data.state.page {
        StartPage::Home => (home(rows, data, listed), SP.m + SP.xs / 2.0),
        StartPage::Projects => (
            projects_page::view(rows, data.pinned, data.state, main_w),
            SP.l,
        ),
        StartPage::Libraries => (
            libraries_page::view(tokens, i18n, &data.libraries, data.state, main_w),
            SP.l,
        ),
    };
    let mut main = column![].spacing(spacing).width(Length::Fixed(main_w));
    for part in content {
        main = main.push(part);
    }
    let body = scrollable(
        container(main)
            .padding([40.0, MAIN_PAD_X])
            .center_x(Length::Fill),
    );
    let navigation = if nav_collapsed(width) {
        nav_strip::view(tokens, i18n, data)
    } else {
        nav(tokens, i18n, data, listed)
    };
    let page_fill = ti(tokens.bg);
    row![
        navigation,
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(page_fill)),
                ..container::Style::default()
            }),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}
