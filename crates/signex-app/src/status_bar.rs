//! Bottom status bar — cursor position, grid, snap, layer, zoom, units.

use iced::widget::{button, container, row, text};
use iced::widget::space;
use iced::{Element, Length};
use signex_types::coord::Unit;

use crate::app::{StatusBarMsg, Tool};

/// Render the status bar.
pub fn view<'a>(
    x: f64,
    y: f64,
    grid_visible: bool,
    snap_enabled: bool,
    zoom: f64,
    unit: Unit,
    tool: &Tool,
) -> Element<'a, StatusBarMsg> {
    let coord_text = match unit {
        Unit::Mm => format!("X:{x:.2} Y:{y:.2}"),
        Unit::Mil => format!("X:{:.1} Y:{:.1}", x / 0.0254, y / 0.0254),
        Unit::Inch => format!("X:{:.4} Y:{:.4}", x / 25.4, y / 25.4),
        Unit::Micrometer => format!("X:{:.0}µm Y:{:.0}µm", x * 1000.0, y * 1000.0),
    };

    let grid_label = if grid_visible { "Grid" } else { "Grid OFF" };
    let snap_label = if snap_enabled { "Snap" } else { "Free" };

    let bar = row![
        text(coord_text).size(12),
        text(" | ").size(12),
        button(text(grid_label).size(12))
            .padding([2, 6])
            .style(button::text)
            .on_press(StatusBarMsg::ToggleGrid),
        text(" | ").size(12),
        button(text(snap_label).size(12))
            .padding([2, 6])
            .style(button::text)
            .on_press(StatusBarMsg::ToggleSnap),
        text(" | ").size(12),
        text("E-Snap").size(12),
        text(" | ").size(12),
        text(format!("{tool}")).size(12),
        space::horizontal(),
        text(format!("{zoom:.0}%")).size(12),
        text(" | ").size(12),
        button(text(format!("{unit}")).size(12))
            .padding([2, 6])
            .style(button::text)
            .on_press(StatusBarMsg::CycleUnit),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    container(bar)
        .width(Length::Fill)
        .padding([3, 8])
        .style(container::bordered_box)
        .into()
}
