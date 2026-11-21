//! State marks of a tree row: the open / unsaved dot on its right and the
//! edge left of the active row.

use iced::widget::{Row, Space, container};
use iced::{Background, Color, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;

use super::{TreeMsg, TreeStyle};
use crate::theme_ext;

/// Side of the "file is open" dot.
const OPEN_DOT_SZ: f32 = 6.0;
/// Side of the "file has unsaved changes" dot.
const DIRTY_DOT_SZ: f32 = 6.0;
/// Width of the slot left of every row of a tree that marks its active
/// row; the edge sits at its left, the rest is the gap to the row fill.
pub(super) const EDGE_SLOT: f32 = 4.0;
/// Width of the active row's edge.
const EDGE_WIDTH: f32 = 2.0;
/// Space the edge leaves above and below itself.
const EDGE_INSET_Y: f32 = 4.0;

/// The dot on the right of an open row: the theme's error colour when the
/// file has unsaved changes, else `open_ink`. Only one fits a narrow panel,
/// and unsaved implies open.
pub(super) fn state_dot(
    dirty: bool,
    open_ink: Color,
    tokens: &ThemeTokens,
) -> Element<'static, TreeMsg> {
    let (color, sz) = if dirty {
        (theme_ext::error_color(tokens), DIRTY_DOT_SZ)
    } else {
        (open_ink, OPEN_DOT_SZ)
    };
    container(Space::new().width(sz).height(sz))
        .width(sz)
        .height(sz)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            border: iced::Border {
                radius: (sz / 2.0).into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// `row` behind the edge slot of a tree that marks its active row: the
/// edge on the active row, empty space on the others. Other trees get `row`
/// unchanged.
pub(super) fn with_edge(
    row: Element<'static, TreeMsg>,
    active: bool,
    style: Option<&TreeStyle>,
) -> Element<'static, TreeMsg> {
    let Some((color, height)) = style.and_then(|s| s.active_edge.map(|c| (c, s.row_height))) else {
        return row;
    };
    let bar = container(Space::new())
        .width(EDGE_WIDTH)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: active.then_some(Background::Color(color)),
            border: iced::Border {
                radius: (EDGE_WIDTH / 2.0).into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
    let slot = container(bar)
        .width(EDGE_SLOT)
        .height(height)
        .padding([EDGE_INSET_Y, 0.0]);
    Row::new()
        .push(slot.boxed())
        .push(row)
        .width(Length::Fill)
        .boxed()
}
