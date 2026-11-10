//! Schematic Active Bar geometry: one slot table that the view, the
//! dropdown anchor and the canvas right-click hit-test all read.
//!
//! The pixel numbers live in `signex_widgets::active_bar::Metrics`; the slot
//! order lives in [`SCHEMATIC_BAR_SLOTS`]. `bar_items` in the parent module
//! is checked against that table by a test, so a button added to the
//! view without a table entry (or the reverse) fails CI instead of
//! silently shifting hit rects and dropdown anchors.

use signex_widgets::active_bar::{Metrics, offsets_for_widths};

use super::ActiveBarMenu;
use crate::app::Tool;

/// One fixed slot of the schematic Active Bar, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarSlot {
    /// A button whose right-click opens `ActiveBarMenu`.
    Trigger(ActiveBarMenu),
    /// Vertical group separator.
    Separator,
    /// Draw-mode pill (90 / 45 / Any), shown while Wire or Bus is active.
    DrawModePill,
}

impl BarSlot {
    /// Width under `m`: the number the widget draws and the hit-test reads.
    pub(crate) fn width(self, m: &Metrics) -> f32 {
        match self {
            Self::Trigger(_) => m.btn,
            Self::Separator => m.sep_w,
            Self::DrawModePill => DRAW_MODE_PILL_WIDTH_PX,
        }
    }
}

/// Declared width of the draw-mode pill; fits the widest label
/// ("90°" / "45°" / "Any") at size 12 plus the 7 px side padding.
pub(crate) const DRAW_MODE_PILL_WIDTH_PX: f32 = 42.0;

/// [Filter][Move] | [Select][Align] | [Wire][Power] | [Harness][Sheet][Port][Dir] | [Text][Shapes][NetColor]
///
/// "Move" opens the `Select` menu and "Select" opens `SelectMode`; the
/// menu ids predate the button names.
pub(crate) const SCHEMATIC_BAR_SLOTS: [BarSlot; 17] = [
    BarSlot::Trigger(ActiveBarMenu::Filter),
    BarSlot::Trigger(ActiveBarMenu::Select),
    BarSlot::Separator,
    BarSlot::Trigger(ActiveBarMenu::SelectMode),
    BarSlot::Trigger(ActiveBarMenu::Align),
    BarSlot::Separator,
    BarSlot::Trigger(ActiveBarMenu::Wiring),
    BarSlot::Trigger(ActiveBarMenu::Power),
    BarSlot::Separator,
    BarSlot::Trigger(ActiveBarMenu::Harness),
    BarSlot::Trigger(ActiveBarMenu::SheetSymbol),
    BarSlot::Trigger(ActiveBarMenu::Port),
    BarSlot::Trigger(ActiveBarMenu::Directives),
    BarSlot::Separator,
    BarSlot::Trigger(ActiveBarMenu::TextTools),
    BarSlot::Trigger(ActiveBarMenu::Shapes),
    BarSlot::Trigger(ActiveBarMenu::NetColor),
];

/// The draw-mode pill (and its leading separator) is drawn for these tools.
pub(crate) fn shows_draw_mode_pill(tool: Tool) -> bool {
    matches!(tool, Tool::Wire | Tool::Bus)
}

/// Every slot of the bar drawn while `tool` is active, left to right.
pub(crate) fn bar_slots(tool: Tool) -> Vec<BarSlot> {
    let mut slots = SCHEMATIC_BAR_SLOTS.to_vec();
    if shows_draw_mode_pill(tool) {
        slots.push(BarSlot::Separator);
        slots.push(BarSlot::DrawModePill);
    }
    slots
}

/// Total drawn width of the bar while `tool` is active. The single
/// width the view, the right-click hit-test and the dropdown anchor
/// all centre with.
pub(crate) fn bar_width_for(tool: Tool, m: &Metrics) -> f32 {
    let widths: Vec<f32> = bar_slots(tool).into_iter().map(|s| s.width(m)).collect();
    offsets_for_widths(&widths, m).1
}

/// Window-space x of the bar's left edge: the bar is centred on the
/// full window width (the overlay layer spans the window, not the canvas).
pub(crate) fn bar_left_in_window(tool: Tool, window_width: f32, m: &Metrics) -> f32 {
    (window_width - bar_width_for(tool, m)) / 2.0
}

/// Bar height under `m`: one button row plus the container padding
/// (Graphite 36).
pub(crate) fn bar_height_for(m: &Metrics) -> f32 {
    m.bar_height()
}

/// Gap between the top of the canvas overlay layer and the bar: the bar
/// floats 10 px below the tab strip.
const BAR_TOP_MARGIN_PX: f32 = 10.0;

/// Top margin of the bar. The bar view, the dropdown anchor and the canvas
/// hit-test all read this, so they cannot disagree.
pub(crate) fn bar_top_margin() -> f32 {
    BAR_TOP_MARGIN_PX
}

/// Extra drop of the symbol editor's floating bar so it sits below the
/// editor's own toolbar strip (one text line, button padding, strip padding
/// and the column gap) instead of covering its centre controls.
pub(crate) fn symbol_editor_bar_drop(_tokens: &signex_types::theme::ThemeTokens) -> f32 {
    let ts = crate::styles::TypeScale::DEFAULT;
    let sp = crate::styles::Spacing::DEFAULT;
    ts.body * crate::styles::LINE_HEIGHT + 2.0 * sp.s + 2.0 * sp.m + sp.s
}

/// Canvas-local height of the Active Bar zone under `m` (`pos.y < this`).
pub(crate) fn bar_zone_height_for(m: &Metrics) -> f32 {
    BAR_TOP_MARGIN_PX + bar_height_for(m) + BAR_ZONE_SLACK_PX
}

/// Gap between the bar's bottom edge and an open dropdown.
pub(crate) const BAR_DROPDOWN_GAP_PX: f32 = 6.0;

/// Extra canvas height under the bar that still counts as the Active
/// Bar zone, so a right-click just below the bar does not start a pan.
pub(crate) const BAR_ZONE_SLACK_PX: f32 = 12.0;

/// Left edge and width of every slot of the bar drawn for `tool`,
/// measured from the bar's left edge.
pub(crate) fn slot_rects(tool: Tool, m: &Metrics) -> Vec<(f32, f32)> {
    let widths: Vec<f32> = bar_slots(tool).into_iter().map(|s| s.width(m)).collect();
    let (offsets, _) = offsets_for_widths(&widths, m);
    offsets.into_iter().zip(widths).collect()
}

/// Left edge of `menu`'s trigger button, from the bar's left edge.
/// The pill sits after every trigger, so this does not depend on the tool.
pub(crate) fn trigger_left(menu: ActiveBarMenu, m: &Metrics) -> f32 {
    slot_rects(Tool::Select, m)
        .into_iter()
        .zip(SCHEMATIC_BAR_SLOTS)
        .find_map(|((left, _), slot)| (slot == BarSlot::Trigger(menu)).then_some(left))
        .unwrap_or(m.padding)
}

/// Menu whose trigger button is drawn under `x` (bar-relative) while
/// `tool` is active, if any. Gaps, separators, the pill and the bar
/// padding hit nothing.
pub(crate) fn menu_at(tool: Tool, x: f32, m: &Metrics) -> Option<ActiveBarMenu> {
    bar_slots(tool)
        .into_iter()
        .zip(slot_rects(tool, m))
        .find_map(|(slot, (left, width))| match slot {
            BarSlot::Trigger(menu) if x >= left && x < left + width => Some(menu),
            _ => None,
        })
}
