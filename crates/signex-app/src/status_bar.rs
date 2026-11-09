//! Bottom status bar — cursor position, grid, snap, layer, zoom, units.

use iced::Element;
use signex_types::coord::Unit;
use signex_types::schematic::{SelectedItem, SelectedKind};
use signex_types::theme::ThemeTokens;

use crate::app::{StatusBarRequest, Tool};
use crate::styles;

mod graphite;
pub use graphite::{
    POPUP_GAP, POPUP_PADDING, POPUP_RIGHT_MARGIN, POPUP_WIDTH, check_ink, panels_button, popup,
    popup_height, popup_row, popup_row_height,
};

/// Height of the status bar: a fixed band from [`styles::ChromeMetrics`].
/// A layout probe in `canvas_region` ties it to the laid-out bar.
pub fn status_bar_height(_tokens: &ThemeTokens) -> f32 {
    styles::ChromeMetrics::DEFAULT.status
}

/// Everything the status bar shows, for the Graphite layout.
pub(crate) struct Readout<'r> {
    pub x: f64,
    pub y: f64,
    pub grid_visible: bool,
    pub snap_enabled: bool,
    pub zoom: f64,
    pub unit: Unit,
    pub tool: String,
    pub grid_size_mm: f32,
    pub selection: Option<String>,
    pub pending_git_commits: usize,
    pub altium_import: Option<&'r str>,
    pub activity: Option<&'r str>,
}

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

#[expect(
    clippy::too_many_arguments,
    reason = "13 arguments: the status bar reads thirteen independent slices of app state and owns none of them"
)]
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
    pending_git_commits: usize,
    altium_import: Option<&str>,
    activity: Option<&str>,
) -> Element<'a, StatusBarRequest> {
    graphite::view(
        &Readout {
            x,
            y,
            grid_visible,
            snap_enabled,
            zoom,
            unit,
            tool: format!("{tool}"),
            grid_size_mm,
            selection: selection_summary(selected),
            pending_git_commits,
            altium_import,
            activity,
        },
        tokens,
    )
}
