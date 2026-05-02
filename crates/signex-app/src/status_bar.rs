//! Bottom status bar — cursor position, grid, snap, layer, zoom, units.

use iced::widget::{button, container, row, space, text};
use iced::{Element, Length};
use signex_types::coord::Unit;
use signex_types::schematic::{SelectedItem, SelectedKind};
use signex_types::theme::ThemeTokens;

use crate::app::{StatusBarRequest, Tool};
use crate::styles;

pub const STATUS_BAR_HORIZONTAL_PADDING: u16 = 8;

/// Format a one-line breakdown of the active canvas selection.
/// Returns `None` when the selection is empty (status bar omits the segment).
fn selection_summary(selected: &[SelectedItem]) -> Option<String> {
    if selected.is_empty() {
        return None;
    }
    let mut components = 0usize;
    let mut wires = 0usize;
    let mut labels = 0usize;
    let mut shapes = 0usize;
    let mut other = 0usize;
    for it in selected {
        match it.kind {
            SelectedKind::Symbol => components += 1,
            SelectedKind::Wire | SelectedKind::Bus | SelectedKind::BusEntry => wires += 1,
            SelectedKind::Label | SelectedKind::SheetPin => labels += 1,
            SelectedKind::Drawing => shapes += 1,
            _ => other += 1,
        }
    }
    let mut parts: Vec<String> = Vec::new();
    let push = |parts: &mut Vec<String>, n: usize, singular: &str, plural: &str| {
        if n > 0 {
            parts.push(format!("{n} {}", if n == 1 { singular } else { plural }));
        }
    };
    push(&mut parts, components, "component", "components");
    push(&mut parts, wires, "wire", "wires");
    push(&mut parts, labels, "label", "labels");
    push(&mut parts, shapes, "shape", "shapes");
    push(&mut parts, other, "other", "other");
    let total = selected.len();
    if parts.len() == 1 {
        Some(format!("Sel: {}", parts.remove(0)))
    } else {
        Some(format!("Sel: {total} ({})", parts.join(", ")))
    }
}

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
    selected: &[SelectedItem],
    tokens: &ThemeTokens,
) -> Element<'a, StatusBarRequest> {
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
    let border_c = styles::ti(tokens.border);
    let text_c = styles::ti(tokens.text);
    let muted_c = styles::ti(tokens.text_secondary);

    let sep = move || text("|").size(10).color(border_c);
    let lbl = move |s: String| text(s).size(11).color(text_c);
    let dim = move |s: &'static str| text(s).size(11).color(muted_c);

    let mut bar = row![
        lbl(coord_text),
        sep(),
        dim("Grid:"),
        button(text(grid_text).size(11).color(text_c))
            .padding([1, 4])
            .style(button::text)
            .on_press(StatusBarRequest::ToggleGrid),
        sep(),
        button(text(snap_label).size(11).color(text_c))
            .padding([1, 4])
            .style(button::text)
            .on_press(StatusBarRequest::ToggleSnap),
        sep(),
        dim("E-Snap"),
        sep(),
        lbl(format!("{tool}")),
    ];
    if let Some(summary) = selection_summary(selected) {
        bar = bar.push(sep());
        bar = bar.push(
            button(text(summary).size(11).color(text_c))
                .padding([1, 4])
                .style(button::text)
                .on_press(StatusBarRequest::OpenPropertiesForSelection),
        );
    }
    let bar = bar
        .push(space::horizontal())
        .push(lbl(format!("{zoom:.0}%")))
        .push(sep())
        .push(
            button(text(format!("{unit}")).size(11).color(text_c))
                .padding([1, 4])
                .style(button::text)
                .on_press(StatusBarRequest::CycleUnit),
        )
        .push(sep())
        .push(
            button(text("Panels").size(11).color(text_c))
                .padding([1, 6])
                .style(button::text)
                .on_press(StatusBarRequest::TogglePanelList),
        )
        .spacing(4)
        .align_y(iced::Alignment::Center);

    container(bar)
        .width(Length::Fill)
        .padding([2, STATUS_BAR_HORIZONTAL_PADDING])
        .style(styles::status_bar(tokens))
        .into()
}
