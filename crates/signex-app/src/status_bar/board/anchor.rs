//! Where a board popover opens: above the segment that opened it (m164).
//!
//! The Grid read-out's place is fixed (`grid_readout_x`); the layer pill
//! follows the Grid read-out and the Snap pill; the counters sit at the right
//! before the angle, the rule, the zoom and the Panels button. Each width is
//! measured from the texts the bar draws, so the card lines up with its
//! segment in either Graphite theme and at any base size.

use signex_i18n::{I18n, Localized, t};

use super::super::ViewAngleReadout;
use super::super::graphite::{
    BAR_GAP, BAR_PADDING, CLUSTER_GAP, PANELS_ICON, SP, TS, grid_readout_x, mono_font, snap_label,
    zoom_text,
};
use super::counters::{counter_widths, grid_readout_width};
use super::kit::{card_width, ui_font};
use super::{BoardCounter, BoardPopoverKind, BoardReadout};
use crate::dock::measure::text_width_in;
use crate::styles::HAIRLINE;

/// The Selection card's gap from its segment's right edge.
const SELECTION_MARGIN: f32 = 8.0 * super::kit::PX;

/// What the bar of one window shows, as far as the anchors need it.
pub(crate) struct AnchorBar<'a> {
    /// The board Grid read-out's value.
    pub grid_value: Localized,
    pub snap_enabled: bool,
    pub zoom: f64,
    pub view_angle: Option<&'a ViewAngleReadout>,
    pub board: &'a BoardReadout,
    pub window_width: f32,
}

fn ui_width(s: &str, size: f32) -> f32 {
    text_width_in(s, size, ui_font(iced::font::Weight::Normal))
}

fn mono_width(s: &str, size: f32) -> f32 {
    text_width_in(s, size, mono_font())
}

/// Where the active layer pill starts: after the Grid read-out, the Snap
/// pill and the rule between the clusters.
fn layer_pill_x(bar: &AnchorBar<'_>, i18n: &I18n) -> f32 {
    let snap = t!(i18n, snap_label(bar.snap_enabled)).to_string();
    grid_readout_x()
        + grid_readout_width(&bar.grid_value, i18n)
        + CLUSTER_GAP
        + 2.0 * 6.0
        + mono_width(&snap, TS.body)
        + BAR_GAP
        + HAIRLINE
        + BAR_GAP
}

/// Where the counters end: left of the angle, the rule, the zoom and the
/// Panels button, each a bar gap apart.
fn counters_right(bar: &AnchorBar<'_>, i18n: &I18n) -> f32 {
    let panels = 2.0 * 10.0
        + PANELS_ICON
        + SP.s
        + ui_width(
            &t!(i18n, signex_i18n::ids::PANELS_STATUS_BAR_PANELS).to_string(),
            TS.section,
        );
    let zoom = mono_width(&zoom_text(i18n, bar.zoom).to_string(), TS.body);
    let angle = bar.view_angle.map_or(0.0, |angle| {
        2.0 * SP.s
            + ui_width(&angle.caption.to_string(), TS.body)
            + SP.xs
            + mono_width(&angle.value, TS.body)
            + BAR_GAP
    });
    bar.window_width
        - BAR_PADDING.right
        - panels
        - BAR_GAP
        - zoom
        - BAR_GAP
        - HAIRLINE
        - BAR_GAP
        - angle
}

/// Where the counter `counter` starts and ends, if the bar shows it.
fn counter_span(bar: &AnchorBar<'_>, counter: BoardCounter, i18n: &I18n) -> Option<(f32, f32)> {
    let widths = counter_widths(bar.board, i18n);
    let right = counters_right(bar, i18n);
    let mut x = right - widths.iter().map(|(_, w)| w).sum::<f32>();
    for (shown, width) in widths {
        if shown == counter {
            return Some((x, x + width));
        }
        x += width;
    }
    None
}

/// The left edge of the `kind` card, kept inside the window.
pub(crate) fn popover_left(kind: BoardPopoverKind, bar: &AnchorBar<'_>, i18n: &I18n) -> f32 {
    let width = card_width(kind);
    let wanted = match kind {
        BoardPopoverKind::Layer => layer_pill_x(bar, i18n),
        BoardPopoverKind::Grid => grid_readout_x(),
        BoardPopoverKind::Unrouted => counter_span(bar, BoardCounter::Routed, i18n)
            .or_else(|| counter_span(bar, BoardCounter::Unrouted, i18n))
            .map_or(0.0, |(left, _)| left),
        BoardPopoverKind::Drc => {
            counter_span(bar, BoardCounter::Drc, i18n).map_or(0.0, |(left, _)| left)
        }
        // Right-aligned to its segment, so Clear stays in the window.
        BoardPopoverKind::Selection => counter_span(bar, BoardCounter::Selected, i18n)
            .map_or(0.0, |(_, right)| right - SELECTION_MARGIN - width),
    };
    let most = (bar.window_width - width - crate::status_bar::POPUP_RIGHT_MARGIN).max(0.0);
    wanted.clamp(0.0, most)
}
