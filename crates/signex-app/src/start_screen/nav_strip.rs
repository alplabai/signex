//! The Home screen navigation collapsed to an icon strip (#2346): what
//! `graphite.rs` shows instead of the full navigation when the main column
//! would be too narrow. Every row keeps its message and shows its label in
//! a tooltip on hover; the loaded-libraries card is left out.

use iced::widget::{Space, column, container, svg};
use iced::{Color, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::{Label, button, text, tooltip};

use super::Action;
use super::graphite::{nav_chrome, nav_divider, nav_row_style, outline_button};
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

fn glyph<'a>(icon: svg::Handle, ink: Color) -> Element<'a, Message> {
    svg(icon)
        .width(ICON)
        .height(ICON)
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
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

fn strip_row<'a>(tokens: &ThemeTokens, row: StripRow<'a>) -> Element<'a, Message> {
    let mut b = button(container(glyph(row.icon, ti(tokens.text))).center_x(Length::Fill))
        .width(Length::Fill)
        .padding(ROW_PAD)
        .style(nav_row_style(tokens, row.active));
    if let Some(message) = row.action {
        b = b.on_press(message);
    }
    with_tip(tokens, b.boxed(), row.label)
}

/// New project and Open…, as square icon buttons in their full-width look.
fn top_buttons<'a>(
    tokens: &ThemeTokens,
    theme: ThemeId,
    i18n: &I18n,
    open_shortcut: &str,
) -> [Element<'a, Message>; 2] {
    let square = |icon: svg::Handle, ink: Color| {
        button(container(glyph(icon, ink)).center_x(Length::Fill))
            .width(Length::Fill)
            .padding(ROW_PAD)
    };
    let new = square(icons::icon_plus(theme), resolve(tokens).primary_ink)
        .on_press(Action::NewProject.message())
        .style(styles::primary_button(tokens));
    let open = square(icons::icon_dd_references(theme), ti(tokens.text))
        .on_press(Action::OpenProject.message())
        .style(outline_button(tokens));
    let open_tip = if open_shortcut.is_empty() {
        tip_label(tokens, t!(i18n, ids::START_OPEN))
    } else {
        tip_with_note(
            tokens,
            t!(i18n, ids::START_OPEN),
            note(tokens, UserText::new(open_shortcut)),
        )
    };
    [
        with_tip(
            tokens,
            new.boxed(),
            tip_label(tokens, t!(i18n, ids::START_NEW_PROJECT)),
        ),
        with_tip(tokens, open.boxed(), open_tip),
    ]
}

/// The navigation as an icon strip, rows in the full navigation's order.
/// No row had an icon, so each borrows the closest existing glyph.
pub(super) fn view<'a>(
    tokens: &ThemeTokens,
    theme: ThemeId,
    i18n: &I18n,
    open_shortcut: &str,
) -> Element<'a, Message> {
    let page = |icon: svg::Handle, label: MessageId, active: bool| StripRow {
        icon,
        label: tip_label(tokens, t!(i18n, label)),
        action: None,
        active,
    };
    let [new, open] = top_buttons(tokens, theme, i18n, open_shortcut);
    let parts = StripRow {
        label: tip_with_note(
            tokens,
            t!(i18n, ids::START_PARTS_SEARCH),
            note(tokens, t!(i18n, ids::PANELS_COMING_SOON)),
        ),
        ..page(
            icons::icon_chrome_search(theme),
            ids::START_PARTS_SEARCH,
            false,
        )
    };
    let app_row = |icon: svg::Handle, label: MessageId, action: Action| StripRow {
        action: Some(action.message()),
        ..page(icon, label, false)
    };
    let pages = [
        page(icons::icon_dd_sheet_actions(theme), ids::START_HOME, true),
        page(
            icons::icon_dd_project_options(theme),
            ids::START_PROJECTS,
            false,
        ),
        page(icons::icon_component(theme), ids::START_LIBRARIES, false),
        parts,
    ];
    // The two app-only rows, under the full navigation's divider.
    let app_rows = [
        app_row(
            icons::icon_dd_open_child_sheet(theme),
            ids::START_OPEN_SAMPLE,
            Action::OpenSampleProject,
        ),
        app_row(
            icons::icon_dd_snippets(theme),
            ids::START_KEYBOARD_SHORTCUTS,
            Action::KeyboardShortcuts,
        ),
    ];
    let mut items = column![new, open, Space::new().height(SP.m)].spacing(SP.xs);
    for row in pages {
        items = items.push(strip_row(tokens, row));
    }
    items = items.push(container(nav_divider(tokens)).padding([SP.m, 0.0]));
    for row in app_rows {
        items = items.push(strip_row(tokens, row));
    }
    container(items)
        .padding([SP.l, ROW_PAD])
        .width(STRIP_W)
        .height(Length::Fill)
        .style(nav_chrome(tokens))
        .boxed()
}
