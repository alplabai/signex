//! Schematic Active Bar geometry: one slot table that the view, the
//! dropdown anchor and the canvas right-click hit-test all read.
//!
//! The pixel constants live in `signex_widgets::active_bar`
//! (`BTN_SIZE`, `SEP_W`, `ROW_SPACING`, `BAR_PADDING`); the slot order
//! lives in [`SCHEMATIC_BAR_SLOTS`]. `bar_items` in the parent module
//! is checked against that table by a test, so a button added to the
//! view without a table entry (or the reverse) fails CI instead of
//! silently shifting hit rects and dropdown anchors.

use signex_widgets::active_bar::{BAR_PADDING, BTN_SIZE, ROW_SPACING, SEP_W, offsets_for_widths};

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
    const fn width(self) -> f32 {
        match self {
            Self::Trigger(_) => BTN_SIZE,
            Self::Separator => SEP_W,
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

const fn bar_width_px() -> f32 {
    let mut x = 2.0 * BAR_PADDING;
    let mut i = 0;
    while i < SCHEMATIC_BAR_SLOTS.len() {
        if i > 0 {
            x += ROW_SPACING;
        }
        x += SCHEMATIC_BAR_SLOTS[i].width();
        i += 1;
    }
    x
}

/// Active Bar width without the draw-mode pill, derived from the slot
/// table and the widget's layout constants.
pub const BAR_WIDTH_PX: f32 = bar_width_px();

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
pub(crate) fn bar_width_for(tool: Tool) -> f32 {
    let widths: Vec<f32> = bar_slots(tool).into_iter().map(BarSlot::width).collect();
    offsets_for_widths(&widths).1
}

/// Window-space x of the bar's left edge: the bar is centred on the
/// full window width (the overlay layer spans the window, not the canvas).
pub(crate) fn bar_left_in_window(tool: Tool, window_width: f32) -> f32 {
    (window_width - bar_width_for(tool)) / 2.0
}

/// Height of the bar: one button row plus the container padding.
pub(crate) const BAR_HEIGHT_PX: f32 = BTN_SIZE + 2.0 * BAR_PADDING;

/// Gap between the top of the canvas overlay layer and the bar.
pub(crate) const BAR_TOP_MARGIN_PX: f32 = 4.0;

/// Gap between the bar's bottom edge and an open dropdown.
pub(crate) const BAR_DROPDOWN_GAP_PX: f32 = 6.0;

/// Extra canvas height under the bar that still counts as the Active
/// Bar zone, so a right-click just below the bar does not start a pan.
pub(crate) const BAR_ZONE_SLACK_PX: f32 = 12.0;

/// Canvas-local height of the Active Bar zone (`pos.y < this`).
pub(crate) const BAR_ZONE_HEIGHT_PX: f32 = BAR_TOP_MARGIN_PX + BAR_HEIGHT_PX + BAR_ZONE_SLACK_PX;

/// Left edge and width of every slot of the bar drawn for `tool`,
/// measured from the bar's left edge.
pub(crate) fn slot_rects(tool: Tool) -> Vec<(f32, f32)> {
    let widths: Vec<f32> = bar_slots(tool).into_iter().map(BarSlot::width).collect();
    let (offsets, _) = offsets_for_widths(&widths);
    offsets.into_iter().zip(widths).collect()
}

/// Left edge of `menu`'s trigger button, from the bar's left edge.
/// The pill sits after every trigger, so this does not depend on the tool.
pub(crate) fn trigger_left(menu: ActiveBarMenu) -> f32 {
    slot_rects(Tool::Select)
        .into_iter()
        .zip(SCHEMATIC_BAR_SLOTS)
        .find_map(|((left, _), slot)| (slot == BarSlot::Trigger(menu)).then_some(left))
        .unwrap_or(BAR_PADDING)
}

/// Menu whose trigger button is drawn under `x` (bar-relative) while
/// `tool` is active, if any. Gaps, separators, the pill and the bar
/// padding hit nothing.
pub(crate) fn menu_at(tool: Tool, x: f32) -> Option<ActiveBarMenu> {
    bar_slots(tool)
        .into_iter()
        .zip(slot_rects(tool))
        .find_map(|(slot, (left, width))| match slot {
            BarSlot::Trigger(menu) if x >= left && x < left + width => Some(menu),
            _ => None,
        })
}
