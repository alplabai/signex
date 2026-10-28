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

/// One fixed slot of the schematic Active Bar, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BarSlot {
    /// A button whose right-click opens `ActiveBarMenu`.
    Trigger(ActiveBarMenu),
    /// Vertical group separator.
    Separator,
}

impl BarSlot {
    const fn width(self) -> f32 {
        match self {
            Self::Trigger(_) => BTN_SIZE,
            Self::Separator => SEP_W,
        }
    }
}

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

/// Height of the bar: one button row plus the container padding.
pub(crate) const BAR_HEIGHT_PX: f32 = BTN_SIZE + 2.0 * BAR_PADDING;

/// Gap between the top of the canvas overlay layer and the bar.
pub(crate) const BAR_TOP_MARGIN_PX: f32 = 4.0;

/// Extra canvas height under the bar that still counts as the Active
/// Bar zone, so a right-click just below the bar does not start a pan.
pub(crate) const BAR_ZONE_SLACK_PX: f32 = 12.0;

/// Canvas-local height of the Active Bar zone (`pos.y < this`).
pub(crate) const BAR_ZONE_HEIGHT_PX: f32 = BAR_TOP_MARGIN_PX + BAR_HEIGHT_PX + BAR_ZONE_SLACK_PX;

/// Left edge and width of every slot, measured from the bar's left edge.
pub(crate) fn slot_rects() -> [(f32, f32); SCHEMATIC_BAR_SLOTS.len()] {
    let widths = SCHEMATIC_BAR_SLOTS.map(BarSlot::width);
    let (offsets, _) = offsets_for_widths(&widths);
    std::array::from_fn(|i| (offsets[i], widths[i]))
}

/// Left edge of `menu`'s trigger button, from the bar's left edge.
pub(crate) fn trigger_left(menu: ActiveBarMenu) -> f32 {
    let rects = slot_rects();
    SCHEMATIC_BAR_SLOTS
        .iter()
        .zip(rects)
        .find_map(|(slot, (left, _))| (*slot == BarSlot::Trigger(menu)).then_some(left))
        .unwrap_or(BAR_PADDING)
}

/// Menu whose trigger button is drawn under `x` (bar-relative), if any.
/// Gaps, separators and the bar padding hit nothing.
pub(crate) fn menu_at(x: f32) -> Option<ActiveBarMenu> {
    let rects = slot_rects();
    SCHEMATIC_BAR_SLOTS
        .iter()
        .zip(rects)
        .find_map(|(slot, (left, width))| match slot {
            BarSlot::Trigger(menu) if x >= left && x < left + width => Some(*menu),
            _ => None,
        })
}
