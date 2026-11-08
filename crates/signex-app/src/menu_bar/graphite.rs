//! Graphite look of the menu bar: the Signex mark in place of the wordmark,
//! flat root buttons, and dropdowns with the shared menu card metrics (28 px
//! rows, a mono shortcut column, a chevron column, soft separators, a width
//! fitted to the rows). Legacy themes never reach it.

use std::sync::LazyLock;

use iced::widget::{Row, Space, button, container, svg, text};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use iced_aw::style::menu_bar as menu_style;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem, MenuMetrics};

use super::{MenuColors, MenuMessage, model};
use crate::styles::TypeScale;

/// Row metrics of a dropdown: the same card numbers as every context menu.
const M: MenuMetrics = MenuMetrics::GRAPHITE;
/// Width of the leading check-mark column of a toggle row: the width
/// `MenuMetrics::fitted_width` reserves for it, so the card fits its rows.
const MARK_WIDTH: f32 = M.check_size;
/// Glyph of the submenu chevron.
const CHEVRON: &str = "\u{203A}";

/// The Signex mark from `assets/brand/signex-logo.svg`, without the wordmark.
/// Its cyan gradient reads on both the light and the dark bar.
static MARK: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../assets/icons/chrome/signex_mark.svg").as_slice())
});

/// Side of the square mark, in px.
pub(super) const MARK_SIZE: f32 = 20.0;
/// Gap between the mark and the first menu root.
pub(super) const MARK_GAP: f32 = 6.0;
/// Corner radius of a root button and of a dropdown row.
pub(super) const ITEM_RADIUS: f32 = 6.0;
/// Corner radius of the dropdown frame.
const DROPDOWN_RADIUS: f32 = 10.0;
/// Horizontal padding of a root button.
const ROOT_PAD_X: f32 = 8.0;
/// Vertical padding of a root button (26 px tall with a 12 px label).
const ROOT_PAD_Y: f32 = 5.0;
/// Left padding of the bar, before the mark.
const BAR_LEFT_PAD: f32 = 10.0;

/// The mark.
pub(super) fn mark<'a, M: 'a>() -> Element<'a, M> {
    svg(MARK.clone()).width(MARK_SIZE).height(MARK_SIZE).boxed()
}

/// Flat root button: radius 6, no border, padding 0 8; lit while hovered or
/// while its dropdown is open.
pub(super) fn root_btn(label: &str, mc: MenuColors) -> Element<'static, MenuMessage> {
    let (ink, hover_bg) = (mc.text, mc.hover);
    let btn = button(
        text(label.to_owned())
            .size(TypeScale::DEFAULT.dense)
            .color(ink),
    )
    .padding([ROOT_PAD_Y, ROOT_PAD_X])
    .on_press(MenuMessage::NoOp)
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover_bg)),
        text_color: ink,
        border: Border {
            radius: ITEM_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    });
    iced::advanced::Widget::_boxed(btn)
}

/// Bar and dropdown frame: radius-10 dropdown with the floating shadow, a
/// borderless rounded highlight behind the open root.
pub(super) fn bar_style(mc: MenuColors) -> menu_style::Style {
    menu_style::Style {
        bar_background: Background::Color(mc.toolbar_bg),
        bar_border: Border::default(),
        bar_shadow: iced::Shadow::default(),
        menu_background: Background::Color(mc.panel_bg),
        menu_border: Border {
            width: 1.0,
            radius: DROPDOWN_RADIUS.into(),
            color: mc.border,
            ..Border::default()
        },
        menu_shadow: mc.shadow,
        path: Background::Color(mc.hover),
        path_border: Border {
            radius: ITEM_RADIUS.into(),
            ..Border::default()
        },
    }
}

/// Columns every row of one dropdown reserves.
#[derive(Clone, Copy)]
pub(super) struct RowShape {
    /// Some row opens a submenu: every row keeps the chevron column.
    pub chevrons: bool,
}

impl RowShape {
    pub(super) fn of(menu: &model::Menu) -> Self {
        Self {
            chevrons: menu
                .entries
                .iter()
                .any(|e| matches!(e, model::MenuEntry::Submenu(_))),
        }
    }
}

/// Width of `menu`'s dropdown: its rows measured as a context-menu card in
/// `ui_font`, between 200 and 320 px.
pub(super) fn dropdown_width(menu: &model::Menu, ui_font: &str) -> f32 {
    let rows: Vec<DropdownEntry<()>> = menu
        .entries
        .iter()
        .filter_map(|entry| match entry {
            model::MenuEntry::Item(item) => {
                let mut row =
                    DropdownItem::passive(item.label.clone()).checked(item.checked.is_some());
                if let Some(s) = &item.shortcut {
                    row = row.shortcut(s.text.clone());
                }
                Some(DropdownEntry::Item(row))
            }
            model::MenuEntry::Submenu(sub) => Some(DropdownEntry::Item(
                DropdownItem::passive(sub.title.clone()).submenu(),
            )),
            model::MenuEntry::Separator => None,
        })
        .collect();
    M.fitted_width(&rows, f32::MAX, ui_font)
}

/// Padding of a dropdown card around its rows.
pub(super) fn dropdown_padding() -> iced::Padding {
    iced::Padding::new(M.panel_pad)
}

/// One dropdown row: the check column of a toggle row, the label, the
/// right-aligned mono shortcut and the chevron column, in a button exactly
/// one 28 px row tall and as wide as the card (the hover fill is the rect
/// that takes the click).
pub(super) fn item_btn(
    label: &str,
    checked: Option<bool>,
    shortcut: Option<String>,
    msg: Option<MenuMessage>,
    mc: MenuColors,
    shape: RowShape,
) -> Element<'static, MenuMessage> {
    let enabled = msg.is_some();
    let ink = if enabled { mc.text } else { mc.text_disabled };
    let mut cells = cells(label, ink, checked);
    if let Some(s) = shortcut {
        cells = cells.push(
            text(s)
                .size(M.shortcut_size)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .color(mc.text_muted)
                .wrapping(text::Wrapping::None)
                .boxed(),
        );
    }
    if shape.chevrons {
        cells = cells.push(chevron_cell("", mc));
    }
    row_button(cells, mc, msg)
}

/// A row that opens a submenu: label and chevron, no shortcut; iced_aw opens
/// the submenu on hover.
pub(super) fn submenu_btn(label: &str, mc: MenuColors) -> Element<'static, MenuMessage> {
    let cells = cells(label, mc.text, None).push(chevron_cell(CHEVRON, mc));
    row_button(cells, mc, Some(MenuMessage::NoOp))
}

/// A soft separator line with the card's margin above and below.
pub(super) fn separator(mc: MenuColors) -> Element<'static, MenuMessage> {
    let line = mc.line_soft;
    container(
        container(Space::new())
            .height(1.0)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(line)),
                ..container::Style::default()
            }),
    )
    .padding([M.separator_margin, 0.0])
    .width(Length::Fill)
    .boxed()
}

/// The leading cells of a row: the check column of a toggle row, the label
/// and the gap that pushes the rest to the right edge.
fn cells(
    label: &str,
    ink: iced::Color,
    checked: Option<bool>,
) -> Row<Element<'static, MenuMessage>> {
    let mut cells: Row<Element<'static, MenuMessage>> = Row::new()
        .spacing(M.row_spacing)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
    if let Some(on) = checked {
        cells = cells.push(
            text(if on { "\u{2713}" } else { "" })
                .size(M.label_size)
                .color(ink)
                .width(Length::Fixed(MARK_WIDTH))
                .boxed(),
        );
    }
    cells
        .push(
            text(label.to_owned())
                .size(M.label_size)
                .color(ink)
                .wrapping(text::Wrapping::None)
                .boxed(),
        )
        .push(Space::new().width(Length::Fill).boxed())
}

fn chevron_cell(glyph: &'static str, mc: MenuColors) -> Element<'static, MenuMessage> {
    text(glyph)
        .size(M.chevron_size)
        .color(mc.text_muted)
        .width(Length::Fixed(M.chevron_width))
        .align_x(iced::alignment::Horizontal::Right)
        .boxed()
}

/// `cells` in a row button that publishes `msg`; without one the row is
/// greyed and takes no hover fill.
fn row_button(
    cells: Row<Element<'static, MenuMessage>>,
    mc: MenuColors,
    msg: Option<MenuMessage>,
) -> Element<'static, MenuMessage> {
    let (hover, ink, enabled) = (mc.hover, mc.text, msg.is_some());
    let btn = button(
        container(cells)
            .padding([0.0, M.row_pad_x])
            .height(M.item_height(false))
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .style(move |_: &Theme, status: button::Status| button::Style {
        background: (enabled
            && matches!(status, button::Status::Hovered | button::Status::Pressed))
        .then_some(Background::Color(hover)),
        text_color: ink,
        border: Border {
            radius: M.row_radius.into(),
            ..Border::default()
        },
        ..button::Style::default()
    });
    match msg {
        Some(m) => btn.on_press(m).boxed(),
        None => btn.boxed(),
    }
}

/// Approximate width of the Graphite menu row: left pad, mark, gap and the
/// root buttons (`PX_PER_CHAR` is a slight overestimate).
pub fn approx_menu_bar_width() -> f32 {
    const PX_PER_CHAR: f32 = 7.5;
    let labels: f32 = model::ROOT_TITLES
        .iter()
        .map(|l| l.chars().count() as f32 * PX_PER_CHAR + 2.0 * ROOT_PAD_X)
        .sum();
    BAR_LEFT_PAD + MARK_SIZE + MARK_GAP + labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_graphite_row_is_wider_than_the_mark_and_every_root() {
        let w = approx_menu_bar_width();
        assert!(w > BAR_LEFT_PAD + MARK_SIZE + MARK_GAP);
        // 52 label chars, 10 roots at 16 px padding.
        assert_eq!(w, 10.0 + 20.0 + 6.0 + 52.0 * 7.5 + 10.0 * 16.0);
    }
}
