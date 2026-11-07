//! Graphite look of the menu bar: the SVG mark in place of the wordmark,
//! flat root buttons and the dropdown frame. Legacy themes never reach it.

use std::sync::LazyLock;

use iced::widget::{button, svg, text};
use iced::{Background, Border, Element, Theme};
use iced_aw::style::menu_bar as menu_style;

use super::{MenuColors, MenuMessage, model};
use crate::styles::TypeScale;

/// Mark on a light bar (dark tile) and on a dark bar (light tile).
static MARK_ON_LIGHT: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        include_bytes!("../../assets/icons/chrome/signex_mark_on_light.svg").as_slice(),
    )
});
static MARK_ON_DARK: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        include_bytes!("../../assets/icons/chrome/signex_mark_on_dark.svg").as_slice(),
    )
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

/// The mark, inverted on a dark bar.
pub(super) fn mark<'a, M: 'a>(dark_bar: bool) -> Element<'a, M> {
    let handle = if dark_bar {
        MARK_ON_DARK.clone()
    } else {
        MARK_ON_LIGHT.clone()
    };
    svg(handle).width(MARK_SIZE).height(MARK_SIZE).into()
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
        // 38 label chars, 8 roots at 16 px padding.
        assert_eq!(w, 10.0 + 20.0 + 6.0 + 38.0 * 7.5 + 8.0 * 16.0);
    }
}
