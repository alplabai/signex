//! Bottom status bar: the cursor, grid and snap, the tool and its hint at the
//! left; the selection (on a board, the board counters), zoom and the Panels
//! button at the right.

use iced::{Color, Element};
use signex_i18n::Localized;
use signex_types::coord::Unit;
use signex_types::schematic::{SelectedItem, SelectedKind};
use signex_types::theme::ThemeTokens;

use crate::app::StatusBarRequest;
use crate::styles;

mod board;
mod clusters;
mod graphite;
mod grid_popover;
pub(crate) use board::{
    AnchorBar, BoardPopoverModel, BoardReadout, ConnectionTally, DrcAge, DrcList, DrcTally,
    GridChoice, LayerChoice, LayerDropdownGroup, LayerList, NetRow, SelectedRow, SelectedWhat,
    SelectionList, UnroutedList, board_grid_value, custom_pitch, group_layers,
    popover_left as board_popover_left, popover_view as board_popover_view,
};
pub use board::{
    BoardCounter, BoardPopoverEvent, BoardPopoverKind, BoardStatusPopover, BoardStatusUi,
};
pub use graphite::{
    POPUP_GAP, POPUP_PADDING, POPUP_RIGHT_MARGIN, POPUP_WIDTH, check_ink, grid_readout_x,
    panel_list_foot_height, panel_list_height, panels_button, popup, popup_caption_height,
    popup_caption_padding, popup_row, popup_row_height,
};
pub use grid_popover::{GridPopoverEvent, GridSnapPopover, GridSnapSettings};
pub(crate) use grid_popover::{HOTSPOTS_COMMAND, view as grid_popover_view};

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
    pub selection: Option<Localized>,
    pub pending_git_commits: usize,
    /// The file a running board import reads, shown as "Importing …".
    pub board_import: Option<&'r str>,
    pub activity: Option<&'r Activity>,
    pub surface: SurfaceReadout,
}

/// What the status bar shows of the surface in view beyond the cursor and
/// grid; empty for a surface without a board or a sheet.
#[derive(Debug, Clone, Default)]
pub(crate) struct SurfaceReadout {
    /// The active copper layer of a board in view.
    pub active_layer: Option<LayerReadout>,
    /// The view's turn on screen (#2316).
    pub view_angle: Option<ViewAngleReadout>,
    /// Whether a right-click on the Grid read-out opens the Grid & Snap
    /// popover: on a schematic sheet only.
    pub opens_grid_popover: bool,
    /// The board counters (#2460): on a board tab only. With them the layer
    /// pill and the Grid read-out open popovers, and the Selected counter
    /// stands in for the selection summary.
    pub board: Option<BoardReadout>,
    /// A project is open and this window shows no document (mockup row 2):
    /// the bar keeps the cursor columns, empty, and Panels.
    pub no_document: bool,
}

/// The board's active layer pill: a caption, the name and the colour the
/// board view draws the layer in.
#[derive(Debug, Clone)]
pub(crate) struct LayerReadout {
    pub caption: Localized,
    pub name: Localized,
    pub colour: Color,
    /// A click opens the layer dropdown; otherwise the pill is a label.
    pub opens_list: bool,
}

/// The board view's angle readout (#2316): a click opens a value field.
#[derive(Debug, Clone)]
pub(crate) struct ViewAngleReadout {
    pub caption: Localized,
    /// The angle with its degree sign.
    pub value: String,
    pub tooltip: Localized,
    pub placeholder: Localized,
    /// What the open value field holds; `None` while it is closed.
    pub entry: Option<String>,
}

/// The activity segment: what a gesture or editor says, or an error the
/// editor reported, drawn in the error colour.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Activity {
    pub text: String,
    pub is_error: bool,
}

impl Activity {
    pub(crate) fn note(text: String) -> Self {
        Self {
            text,
            is_error: false,
        }
    }
}

/// Format a one-line breakdown of the active canvas selection.
/// Returns `None` when the selection is empty (status bar omits the segment).
fn selection_summary(selected: &[SelectedItem], i18n: &signex_i18n::I18n) -> Option<Localized> {
    use signex_i18n::ids;
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
    let parts: Vec<String> = [
        (components, ids::PANELS_STATUS_BAR_SELECTED_COMPONENTS),
        (wires, ids::PANELS_STATUS_BAR_SELECTED_WIRES),
        (labels, ids::PANELS_STATUS_BAR_SELECTED_LABELS),
        (shapes, ids::PANELS_STATUS_BAR_SELECTED_SHAPES),
        (other, ids::PANELS_STATUS_BAR_SELECTED_OTHER),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, id)| i18n.message_plain(id, &[("count", n.into())]))
    .collect();
    Some(match parts.as_slice() {
        [kind] => {
            signex_i18n::t!(i18n, ids::PANELS_STATUS_BAR_SELECTION_ONE_KIND, "kind" => kind.as_str())
        }
        _ => signex_i18n::t!(
            i18n,
            ids::PANELS_STATUS_BAR_SELECTION_KINDS,
            "count" => selected.len(),
            "kinds" => parts.join(", ")
        ),
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "15 arguments: the status bar reads fifteen independent slices of app state and owns none of them"
)]
pub(crate) fn view<'a>(
    x: f64,
    y: f64,
    grid_visible: bool,
    snap_enabled: bool,
    zoom: f64,
    unit: Unit,
    tool: String,
    grid_size_mm: f32,
    selected: &[SelectedItem],
    tokens: &ThemeTokens,
    pending_git_commits: usize,
    board_import: Option<&str>,
    activity: Option<&Activity>,
    surface: SurfaceReadout,
    i18n: &signex_i18n::I18n,
) -> Element<'a, StatusBarRequest> {
    clusters::view(
        &Readout {
            x,
            y,
            grid_visible,
            snap_enabled,
            zoom,
            unit,
            tool,
            grid_size_mm,
            selection: selection_summary(selected, i18n),
            pending_git_commits,
            board_import,
            activity,
            surface,
        },
        tokens,
        i18n,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(kinds: &[SelectedKind]) -> Option<String> {
        let selected: Vec<SelectedItem> = kinds
            .iter()
            .map(|kind| SelectedItem::new(uuid::Uuid::now_v7(), *kind))
            .collect();
        selection_summary(&selected, &signex_i18n::I18n::en_us())
            .map(|text| text.into_string().replace(['\u{2068}', '\u{2069}'], ""))
    }

    /// The selection segment reads as it did before the catalog (#2059).
    #[test]
    fn the_selection_summary_counts_each_kind() {
        assert_eq!(summary(&[]), None);
        assert_eq!(
            summary(&[SelectedKind::Symbol]).as_deref(),
            Some("Sel: 1 component")
        );
        assert_eq!(
            summary(&[SelectedKind::Wire, SelectedKind::Bus]).as_deref(),
            Some("Sel: 2 wires")
        );
        assert_eq!(
            summary(&[
                SelectedKind::Symbol,
                SelectedKind::Symbol,
                SelectedKind::Label,
                SelectedKind::Drawing,
                SelectedKind::Junction,
            ])
            .as_deref(),
            Some("Sel: 5 (2 components, 1 label, 1 shape, 1 other)")
        );
    }
}
