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
    GridChoice, LAYER_DROPDOWN_WIDTH, LayerChoice, LayerDropdownGroup, LayerList, NetRow,
    SelectedRow, SelectedWhat, SelectionList, UnroutedList, board_grid_value, custom_pitch,
    group_layers, popover_left as board_popover_left, popover_view as board_popover_view,
    two_column_card as layer_dropdown_card,
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
    /// Whether the Grid read-out carries the grid glyph and a caret that
    /// opens the grid step list: in a footprint editor only (mockup row 63).
    pub opens_grid_picker: bool,
    /// The board counters (#2460): on a board tab only. With them the layer
    /// pill and the Grid read-out open popovers, and the Selected counter
    /// stands in for the selection summary.
    pub board: Option<BoardReadout>,
    /// A project is open and this window shows no document (mockup row 2):
    /// the bar keeps the cursor columns, empty, and Panels.
    pub no_document: bool,
    /// Skipped libraries still missing (mockup row 138); above zero the bar
    /// shows the Library Missing chip.
    pub missing_libraries: usize,
    /// Image files the sheet's project names and cannot show (#1116, mockup
    /// row 204); above zero the bar counts them in a warning pill.
    pub missing_assets: usize,
    /// The number of the pin a click picked on the part that is the whole
    /// selection (mockup row 50): the selection summary names it.
    pub focused_pin: Option<String>,
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

/// The selection segment: the breakdown of the selection and, while a click
/// has picked a pin of the selected part, that pin (#2998, mockup row 50).
/// `None` when the selection is empty (the status bar omits the segment).
fn selection_summary(
    selected: &[SelectedItem],
    focused_pin: Option<&str>,
    i18n: &signex_i18n::I18n,
) -> Option<Localized> {
    let kinds = selection_kinds(selected, i18n)?;
    Some(match focused_pin {
        Some(pin) => signex_i18n::t!(
            i18n,
            signex_i18n::ids::PANELS_STATUS_BAR_SELECTION_PIN,
            "selection" => kinds.into_string(),
            "pin" => pin
        ),
        None => kinds,
    })
}

/// Format a one-line breakdown of the active canvas selection.
/// Returns `None` when the selection is empty.
fn selection_kinds(selected: &[SelectedItem], i18n: &signex_i18n::I18n) -> Option<Localized> {
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
            selection: selection_summary(selected, surface.focused_pin.as_deref(), i18n),
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
        summary_with_pin(kinds, None)
    }

    fn summary_with_pin(kinds: &[SelectedKind], pin: Option<&str>) -> Option<String> {
        let selected: Vec<SelectedItem> = kinds
            .iter()
            .map(|kind| SelectedItem::new(uuid::Uuid::now_v7(), *kind))
            .collect();
        selection_summary(&selected, pin, &signex_i18n::I18n::en_us())
            .map(|text| text.into_string().replace(['\u{2068}', '\u{2069}'], ""))
    }

    /// #2998, mockup row 50: while a click has picked a pin of the selected
    /// part, the segment names that pin after the breakdown.
    #[test]
    fn a_picked_pin_is_named_after_the_selection() {
        assert_eq!(
            summary_with_pin(&[SelectedKind::Symbol], Some("3")).as_deref(),
            Some("Sel: 1 component · pin 3")
        );
        assert_eq!(
            summary_with_pin(&[SelectedKind::Symbol], Some("A12")).as_deref(),
            Some("Sel: 1 component · pin A12")
        );
        assert_eq!(summary_with_pin(&[], Some("3")), None, "no selection");
    }

    /// The pin segment goes through the catalog: another language reads it
    /// in its own words.
    #[test]
    fn the_picked_pin_text_follows_the_language() {
        let selected = [SelectedItem::new(
            uuid::Uuid::now_v7(),
            SelectedKind::Symbol,
        )];
        let xa = signex_i18n::I18n::new(&signex_i18n::LanguagePref::Locale("en-XA".into()));
        let shown = selection_summary(&selected, Some("3"), &xa)
            .map(|text| text.into_string().replace(['\u{2068}', '\u{2069}'], ""));
        assert_ne!(shown.as_deref(), Some("Sel: 1 component · pin 3"));
        assert!(shown.is_some_and(|s| s.contains('3')));
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
