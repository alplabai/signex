//! Bottom status bar — cursor position, grid, snap, layer, zoom, units.

use iced::widget::{button, container, row, space, text};
use iced::{Element, Length};
use signex_types::coord::Unit;

use crate::app::{StatusBarMsg, Tool};
use crate::styles;

#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    x: f64,
    y: f64,
    grid_visible: bool,
    snap_enabled: bool,
    zoom: f64,
    unit: Unit,
    tool: &Tool,
    grid_size_mm: f32,
) -> Element<'a, StatusBarMsg> {
    let coord_text = match unit {
        Unit::Mm => format!("X:{x:.2} Y:{y:.2}"),
        Unit::Mil => format!("X:{:.1} Y:{:.1}", x / 0.0254, y / 0.0254),
        Unit::Inch => format!("X:{:.4} Y:{:.4}", x / 25.4, y / 25.4),
        Unit::Micrometer => format!("X:{:.0} Y:{:.0}", x * 1000.0, y * 1000.0),
    };

    let grid_text = if grid_visible {
        format!("{grid_size_mm:.3}mm")
    } else {
        "OFF".to_string()
    };

    let snap_label = if snap_enabled { "Snap" } else { "Free" };
    let sep = || text("|").size(10).color(styles::BORDER_COLOR);
    let lbl = |s: String| text(s).size(11).color(styles::TEXT_PRIMARY);
    let dim = |s: &'static str| text(s).size(11).color(styles::TEXT_MUTED);

    let bar = row![
        lbl(coord_text),
        sep(),
        dim("Grid:"),
        button(text(grid_text).size(11).color(styles::TEXT_PRIMARY))
            .padding([1, 4])
            .style(button::text)
            .on_press(StatusBarMsg::ToggleGrid),
        sep(),
        button(text(snap_label).size(11).color(styles::TEXT_PRIMARY))
            .padding([1, 4])
            .style(button::text)
            .on_press(StatusBarMsg::ToggleSnap),
        sep(),
        dim("E-Snap"),
        sep(),
        lbl(format!("{tool}")),
        space::horizontal(),
        lbl(format!("{zoom:.0}%")),
        sep(),
        button(text(format!("{unit}")).size(11).color(styles::TEXT_PRIMARY))
            .padding([1, 4])
            .style(button::text)
            .on_press(StatusBarMsg::CycleUnit),
        sep(),
        button(text("Panels").size(11).color(styles::TEXT_PRIMARY))
            .padding([1, 6])
            .style(button::text)
            .on_press(StatusBarMsg::TogglePanelList),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    container(bar)
        .width(Length::Fill)
        .padding([2, 8])
        .style(styles::status_bar)
        .into()
}
