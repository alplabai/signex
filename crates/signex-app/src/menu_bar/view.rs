//! Menu-bar view: renders the declarative [`MenuBarModel`] as iced_aw menus
//! plus the Signex wordmark. What the menus contain lives in `model/`.

use super::model::{
    ItemState, Menu as MenuModel, MenuBarModel, MenuEntry, MenuItem, menu_bar_model,
};
use super::*;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

type MenuItemView = Item<'static, MenuMessage, Theme, iced::Renderer>;

// ─── View: Menu Bar ──────────────────────────────────────────

/// The menu bar in `tokens`' theme; `ui_font` is the face its labels draw
/// in, which the Graphite dropdowns are fitted to.
pub fn view(
    tokens: &ThemeTokens,
    ui_font: &str,
    ctx: MenuContext<'_>,
) -> Element<'static, MenuMessage> {
    let mc = MenuColors::from_tokens(tokens);
    let mb = render_menu_bar(&menu_bar_model(&ctx), mc, ui_font);

    iced::advanced::Widget::_boxed(
        row![graphite::mark(), mb]
            .spacing(graphite::MARK_GAP)
            .align_y(iced::Alignment::Center),
    )
}

fn render_menu_bar(
    model: &MenuBarModel,
    mc: MenuColors,
    ui_font: &str,
) -> MenuBar<'static, MenuMessage, Theme, iced::Renderer> {
    MenuBar::new(
        model
            .menus
            .iter()
            .map(|menu| render_root(menu, mc, ui_font))
            .collect(),
    )
    .spacing(1.0)
    .padding([1.0, SP.s])
    .close_on_item_click_global(true)
    .close_on_background_click_global(true)
    // `Backdrop` paints `styling.path` behind the active root while its
    // dropdown is open, so "File / Edit / Place / …" stays visibly lit
    // after the pointer leaves the root and enters the submenu — matches
    // Altium. `FakeHovering` (the default) only affects items inside the
    // dropdown and leaves the root dark.
    .draw_path(DrawPath::Backdrop)
    .style(move |_theme: &Theme, _status| graphite::bar_style(mc))
}

fn render_root(menu: &MenuModel, mc: MenuColors, ui_font: &str) -> MenuItemView {
    Item::with_menu(
        graphite::root_btn(&menu.title, mc),
        render_dropdown(menu, mc, ui_font),
    )
}

fn render_submenu(menu: &MenuModel, mc: MenuColors, ui_font: &str) -> MenuItemView {
    let launcher = graphite::submenu_btn(&menu.title, mc);
    Item::with_menu(launcher, render_dropdown(menu, mc, ui_font))
}

fn render_dropdown(
    menu: &MenuModel,
    mc: MenuColors,
    ui_font: &str,
) -> Menu<'static, MenuMessage, Theme, iced::Renderer> {
    let shape = graphite::RowShape::of(menu);
    let items = menu
        .entries
        .iter()
        .map(|entry| render_entry(entry, mc, shape, ui_font))
        .collect();
    // Fitted to its rows, 28 px rows with no gaps between them, so every
    // hover fill touches the next and the card pads its rows evenly.
    Menu::new(items)
        .width(Length::Fixed(graphite::dropdown_width(menu, ui_font)))
        .offset(4.0)
        .spacing(0.0)
        .padding(graphite::dropdown_padding())
}

fn render_entry(
    entry: &MenuEntry,
    mc: MenuColors,
    shape: graphite::RowShape,
    ui_font: &str,
) -> MenuItemView {
    match entry {
        MenuEntry::Item(item) => render_item(item, mc, shape),
        MenuEntry::Submenu(menu) => render_submenu(menu, mc, ui_font),
        MenuEntry::Separator => Item::new(graphite::separator(mc)),
    }
}

/// A row dispatches only when it has a handler and is enabled; otherwise it
/// renders greyed out with no press handler.
fn render_item(item: &MenuItem, mc: MenuColors, shape: graphite::RowShape) -> MenuItemView {
    let shortcut = item.shortcut.as_ref().map(|s| s.text.clone());
    let row = graphite::item_btn(
        &item.label,
        item.checked,
        shortcut,
        item.dispatches().cloned(),
        mc,
        shape,
    );
    if !matches!(item.state, ItemState::Placeholder) {
        return Item::new(row);
    }
    // A planned feature reads as planned rather than broken (#1251).
    let hint = container(
        text(raw(crate::active_bar::PLANNED_TOOLTIP))
            .size(MENU_SHORTCUT_SIZE)
            .color(mc.text),
    )
    .padding([3, 8])
    .style(signex_widgets::active_bar_dropdown::tooltip_style(
        mc.hover, mc.border,
    ));
    Item::new(iced::advanced::Widget::_boxed(
        signex_widgets::tooltip(row, hint, iced::widget::tooltip::Position::Right).gap(4),
    ))
}
