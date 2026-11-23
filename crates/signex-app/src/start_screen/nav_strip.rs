//! The Start screen navigation collapsed to an icon strip (#2346): what
//! `graphite.rs` shows instead of the full navigation when the main column
//! would be too narrow. Every row keeps its message and shows its label in
//! a tooltip on hover; the loaded-libraries card is left out.

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{Space, column, container, scrollable, svg};
use iced::{Color, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text, tooltip};

use super::graphite::{NAV_ROW_H, nav_chrome, nav_divider, nav_row_style, page_action};
use super::kit::{GLYPH_THEME, icon, outline_button};
use super::{Action, StartPage, StartScreenData};
use crate::app::Message;
use crate::icons;
use crate::styles::{self, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Edge of a row icon.
const ICON: f32 = TS.body * 4.0 / 3.0;
/// Space between an icon and the edge of its row.
const ROW_PAD: f32 = SP.m;
/// Width of the strip: one row plus the strip's own padding on both sides.
pub(super) const STRIP_W: f32 = ICON + 4.0 * ROW_PAD;

/// One strip row: its icon, its label (the tooltip), what it sends.
struct StripRow<'a> {
    icon: svg::Handle,
    label: Element<'a, Message>,
    action: Option<Message>,
    active: bool,
}

fn glyph<'a>(handle: svg::Handle, ink: Color) -> Element<'a, Message> {
    icon(handle, ICON, ink)
}

/// `content` with `tip` shown at its right on hover.
fn with_tip<'a>(
    tokens: &ThemeTokens,
    content: Element<'a, Message>,
    tip: Element<'a, Message>,
) -> Element<'a, Message> {
    let tip = container(tip).padding([SP.xs, SP.m]).style(
        signex_widgets::active_bar_dropdown::tooltip_style(ti(tokens.hover), ti(tokens.border)),
    );
    tooltip(content, tip, tooltip::Position::Right)
        .gap(SP.xs)
        .boxed()
}

/// A row label as the tooltip draws it.
fn tip_label<'a>(tokens: &ThemeTokens, label: Localized) -> Element<'a, Message> {
    text(label).size(TS.dense).color(ti(tokens.text)).boxed()
}

/// A tooltip label with a quieter second line under it.
fn tip_with_note<'a>(
    tokens: &ThemeTokens,
    label: Localized,
    note: Element<'a, Message>,
) -> Element<'a, Message> {
    column![tip_label(tokens, label), note]
        .spacing(SP.xs)
        .boxed()
}

/// The quieter second line of a tooltip.
fn note<'a>(tokens: &ThemeTokens, line: impl Into<Label<'a>>) -> Element<'a, Message> {
    text(line)
        .size(TS.caption)
        .color(resolve(tokens).ink3)
        .boxed()
}

/// A label, with the bound key as its note when there is one.
fn tip_with_key<'a>(tokens: &ThemeTokens, label: Localized, key: &str) -> Element<'a, Message> {
    if key.is_empty() {
        tip_label(tokens, label)
    } else {
        tip_with_note(tokens, label, note(tokens, UserText::new(key)))
    }
}

fn strip_row<'a>(tokens: &ThemeTokens, row: StripRow<'a>) -> Element<'a, Message> {
    let mut b = button(container(glyph(row.icon, ti(tokens.text))).center(Length::Fill))
        .width(Length::Fill)
        .height(NAV_ROW_H)
        .padding(0)
        .style(nav_row_style(tokens, row.active));
    if let Some(message) = row.action {
        b = b.on_press(message);
    }
    with_tip(tokens, b.boxed(), row.label)
}

/// New project and Open…, as square icon buttons in their full-width look.
fn top_buttons<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    open_shortcut: &str,
) -> [Element<'a, Message>; 2] {
    let square = |icon: svg::Handle, ink: Color| {
        button(container(glyph(icon, ink)).center_x(Length::Fill))
            .width(Length::Fill)
            .padding(ROW_PAD)
    };
    let new = square(icons::icon_plus(GLYPH_THEME), resolve(tokens).primary_ink)
        .on_press(Action::NewProject.message())
        .style(styles::primary_button(tokens));
    let open = square(icons::icon_start_folder(GLYPH_THEME), ti(tokens.text))
        .on_press(Action::OpenProject.message())
        .style(outline_button(tokens));
    [
        with_tip(
            tokens,
            new.boxed(),
            tip_label(tokens, t!(i18n, ids::START_NEW_PROJECT)),
        ),
        with_tip(
            tokens,
            open.boxed(),
            tip_with_key(tokens, t!(i18n, ids::START_OPEN), open_shortcut),
        ),
    ]
}

/// The navigation as an icon strip, rows in the full navigation's order and
/// with the full rows' icons.
pub(super) fn view<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    data: &StartScreenData<'a>,
) -> Element<'a, Message> {
    let current = data.state.page;
    let page = |icon: svg::Handle, label: MessageId, page: StartPage| StripRow {
        icon,
        label: tip_label(tokens, t!(i18n, label)),
        action: page_action(current, page),
        active: current == page,
    };
    let [new, open] = top_buttons(tokens, i18n, &data.open_shortcut);
    let parts = StripRow {
        icon: icons::icon_chrome_search(GLYPH_THEME),
        label: tip_with_note(
            tokens,
            t!(i18n, ids::START_PARTS_SEARCH),
            note(tokens, t!(i18n, ids::PANELS_COMING_SOON)),
        ),
        action: None,
        active: false,
    };
    let pages = [
        page(
            icons::icon_start_home(GLYPH_THEME),
            ids::START_HOME,
            StartPage::Home,
        ),
        page(
            icons::icon_start_folder(GLYPH_THEME),
            ids::START_PROJECTS,
            StartPage::Projects,
        ),
        page(
            icons::icon_start_book(GLYPH_THEME),
            ids::START_LIBRARIES,
            StartPage::Libraries,
        ),
        parts,
    ];
    // Keyboard Shortcuts, under the full navigation's divider.
    let shortcuts = StripRow {
        icon: icons::icon_start_key(GLYPH_THEME),
        label: tip_with_key(
            tokens,
            t!(i18n, ids::START_KEYBOARD_SHORTCUTS),
            &data.shortcuts_shortcut,
        ),
        action: Some(Action::KeyboardShortcuts.message()),
        active: false,
    };
    let mut items = column![new, open, Space::new().height(SP.m)].spacing(SP.xs);
    for row in pages {
        items = items.push(strip_row(tokens, row));
    }
    items = items.push(container(nav_divider(tokens)).padding([SP.m, 0.0]).boxed());
    items = items.push(strip_row(tokens, shortcuts));
    // Scrolls in a window too short for every row. The thin bar floats in
    // the right-hand padding, clear of the icons.
    let bar = Scrollbar::new().width(SP.xs).scroller_width(SP.xs);
    let rows = scrollable(container(items).padding([SP.l, ROW_PAD]))
        .direction(Direction::Vertical(bar))
        .height(Length::Fill);
    container(rows)
        .width(STRIP_W)
        .height(Length::Fill)
        .style(nav_chrome(tokens))
        .boxed()
}
