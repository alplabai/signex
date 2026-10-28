//! Menu-bar view: renders the declarative [`MenuBarModel`] as iced_aw menus
//! plus the Signex wordmark. What the menus contain lives in `model/`.

use super::model::{Menu as MenuModel, MenuBarModel, MenuEntry, MenuItem, menu_bar_model};
use super::*;

type MenuItemView = Item<'static, MenuMessage, Theme, iced::Renderer>;

// ─── View: Menu Bar ──────────────────────────────────────────

pub fn view(tokens: &ThemeTokens, ctx: MenuContext<'_>) -> Element<'static, MenuMessage> {
    let mc = MenuColors::from_tokens(tokens);
    let mb = render_menu_bar(&menu_bar_model(&ctx), mc);

    // Wordmark — white on dark themes, near-black on light themes. Picked
    // by toolbar background luminance so custom themes also resolve
    // correctly. The asset is a PNG pre-rasterised at 1×/2×/3× the
    // on-screen 96×31 logical size; `wordmark_tier` picks the one that
    // matches the window's scale factor so text edges stay crisp at
    // 100 %, 125 %, 150 %, 200 %, 300 % Windows scaling. Filter method
    // is Linear so fractional in-between scales (e.g. 1.5×) downsample
    // cleanly from the 2× asset rather than hard-pixelating.
    let dark = is_dark_surface(tokens.toolbar_bg);
    let handle = match (dark, wordmark_tier(ctx.scale_factor)) {
        (true, 1) => (*BRAND_WORDMARK_WHITE_1X).clone(),
        (true, 2) => (*BRAND_WORDMARK_WHITE_2X).clone(),
        (true, _) => (*BRAND_WORDMARK_WHITE_3X).clone(),
        (false, 1) => (*BRAND_WORDMARK_BLACK_1X).clone(),
        (false, 2) => (*BRAND_WORDMARK_BLACK_2X).clone(),
        (false, _) => (*BRAND_WORDMARK_BLACK_3X).clone(),
    };
    let wordmark = image(handle)
        .width(WORDMARK_LOGICAL_W)
        .height(WORDMARK_LOGICAL_H)
        .filter_method(image::FilterMethod::Linear);

    // Just the wordmark + menu roots. The caller decides how to wrap this
    // (plain strip on secondary windows, draggable chrome with window
    // controls on the borderless main window).
    row![wordmark, mb]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .into()
}

fn render_menu_bar(
    model: &MenuBarModel,
    mc: MenuColors,
) -> MenuBar<'static, MenuMessage, Theme, iced::Renderer> {
    MenuBar::new(
        model
            .menus
            .iter()
            .map(|menu| render_root(menu, mc))
            .collect(),
    )
    .spacing(1.0)
    .padding([1, 4])
    .close_on_item_click_global(true)
    .close_on_background_click_global(true)
    // `Backdrop` paints `styling.path` behind the active root while its
    // dropdown is open, so "File / Edit / Place / …" stays visibly lit
    // after the pointer leaves the root and enters the submenu — matches
    // Altium. `FakeHovering` (the default) only affects items inside the
    // dropdown and leaves the root dark.
    .draw_path(DrawPath::Backdrop)
    .style(move |_theme: &Theme, _status| bar_style(mc))
}

fn bar_style(mc: MenuColors) -> menu_style::Style {
    menu_style::Style {
        bar_background: Background::Color(mc.toolbar_bg),
        bar_border: Border::default(),
        bar_shadow: iced::Shadow::default(),
        menu_background: Background::Color(mc.panel_bg),
        menu_border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: mc.border,
        },
        menu_shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(2.0, 4.0),
            blur_radius: 8.0,
        },
        path: Background::Color(mc.hover),
        path_border: Border {
            width: 1.0,
            radius: 2.0.into(),
            color: mc.border,
        },
    }
}

fn render_root(menu: &MenuModel, mc: MenuColors) -> MenuItemView {
    Item::with_menu(root_btn(&menu.title, mc), render_dropdown(menu, mc))
}

fn render_submenu(menu: &MenuModel, mc: MenuColors) -> MenuItemView {
    Item::with_menu(submenu_item_btn(&menu.title, mc), render_dropdown(menu, mc))
}

fn render_dropdown(
    menu: &MenuModel,
    mc: MenuColors,
) -> Menu<'static, MenuMessage, Theme, iced::Renderer> {
    let items = menu
        .entries
        .iter()
        .map(|entry| render_entry(entry, mc))
        .collect();
    Menu::new(items)
        .max_width(DROPDOWN_WIDTH)
        // Sit a couple of pixels below the bar — offset(0) overlaps
        // the bar's bottom row because the dropdown's 1px border
        // paints on the same pixel as the bar's baseline.
        .offset(2.0)
        .spacing(2.0)
        // iced_aw paints the dropdown's background quad at
        // `items.x - padding.left` (see `pad_rectangle` in
        // `iced_aw/src/widget/menu/menu_tree.rs`). Any positive
        // left-padding here drags the visible dropdown LEFT of the
        // root button's highlight box. Zero on the left keeps the
        // dropdown's left border flush with the root's layout
        // bounds, matching Altium's alignment.
        .padding(iced::Padding {
            top: 5.0,
            right: 5.0,
            bottom: 5.0,
            left: 0.0,
        })
}

fn render_entry(entry: &MenuEntry, mc: MenuColors) -> MenuItemView {
    match entry {
        MenuEntry::Item(item) => render_item(item, mc),
        MenuEntry::Submenu(menu) => render_submenu(menu, mc),
        MenuEntry::Separator => separator(mc),
    }
}

/// A row dispatches only when it has a handler and is enabled; otherwise it
/// renders greyed out with no press handler.
fn render_item(item: &MenuItem, mc: MenuColors) -> MenuItemView {
    let shortcut = item.shortcut.as_ref().map(|s| s.text.clone());
    Item::new(menu_item_btn(
        &item.label,
        shortcut,
        item.dispatches().cloned(),
        mc,
    ))
}
