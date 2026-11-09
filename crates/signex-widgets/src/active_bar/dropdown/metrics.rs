//! Sizes of a menu card: one [`MenuMetrics`] per look, read by the row
//! renderer, the width fit and the flyout placement, so a drawn row and the
//! place a caller expects it cannot drift apart.

use super::{DropdownEntry, DropdownItem};

/// Family the shortcut column draws in on the Graphite look.
pub const MONO_FAMILY: &str = "JetBrains Mono";

/// Size of a Graphite menu label: the menu bar roots, dropdown rows,
/// flyouts and context menus all read this one token.
pub const MENU_LABEL_SIZE: f32 = 12.0;
/// Size of a Graphite menu shortcut hint, set in [`MONO_FAMILY`] and muted,
/// on every menu surface.
pub const MENU_SHORTCUT_SIZE: f32 = 11.0;

/// Layout numbers of one menu look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuMetrics {
    /// Text size of a row label.
    pub label_size: f32,
    /// Text size of the shortcut column.
    pub shortcut_size: f32,
    /// Whether the shortcut column is drawn in [`MONO_FAMILY`].
    pub mono_shortcut: bool,
    /// Text size of a section header.
    pub header_size: f32,
    /// Text size of the check mark.
    pub check_size: f32,
    /// Text size of the submenu chevron.
    pub chevron_size: f32,
    /// Width of the chevron column, reserved on every row of a menu that has
    /// a submenu row.
    pub chevron_width: f32,
    /// Edge of a row icon, and width of the icon column.
    pub icon_size: f32,
    /// Gap between the cells of a row.
    pub row_spacing: f32,
    /// Smallest gap between a label and the shortcut column.
    pub cluster_gap: f32,
    /// Horizontal padding of a row.
    pub row_pad_x: f32,
    /// Fixed height of an item row; `None` sizes it from its content plus
    /// [`Self::row_pad_y`] above and below.
    pub row_height: Option<f32>,
    /// Vertical padding of a content-sized row.
    pub row_pad_y: f32,
    /// Fixed height of a header row; `None` sizes it from its text.
    pub header_height: Option<f32>,
    /// Vertical padding of a content-sized header.
    pub header_pad_y: f32,
    /// Corner radius of a row's hover fill.
    pub row_radius: f32,
    /// Padding of the card around its rows, every side.
    pub panel_pad: f32,
    /// Space above and below a separator line.
    pub separator_margin: f32,
    /// Narrowest fitted card.
    pub min_width: f32,
    /// Widest fitted card.
    pub max_width: f32,
}

impl MenuMetrics {
    /// The Graphite look: dense 13 px type on 28 px rows, a 16 px
    /// monochrome icon column, a mono shortcut column and a chevron column,
    /// cards fitted between 200 and 320 px.
    pub const GRAPHITE: Self = Self {
        label_size: MENU_LABEL_SIZE,
        shortcut_size: MENU_SHORTCUT_SIZE,
        mono_shortcut: true,
        header_size: 11.0,
        check_size: 13.0,
        chevron_size: 15.0,
        chevron_width: 10.0,
        icon_size: 16.0,
        row_spacing: 8.0,
        cluster_gap: 24.0,
        row_pad_x: 10.0,
        row_height: Some(28.0),
        row_pad_y: 0.0,
        header_height: Some(24.0),
        header_pad_y: 0.0,
        row_radius: 6.0,
        panel_pad: 5.0,
        separator_margin: 4.0,
        min_width: 200.0,
        max_width: 320.0,
    };

    /// Height of one item row; `has_icons` is whether the menu has an icon
    /// column (a content-sized row is then at least icon tall).
    pub fn item_height(&self, has_icons: bool) -> f32 {
        self.row_height.unwrap_or_else(|| {
            let line = line_height(self.label_size);
            let content = if has_icons {
                line.max(self.icon_size)
            } else {
                line
            };
            content + 2.0 * self.row_pad_y
        })
    }

    /// Height of a header row.
    pub fn header_row_height(&self) -> f32 {
        self.header_height
            .unwrap_or_else(|| line_height(self.header_size) + 2.0 * self.header_pad_y)
    }

    /// Height of a separator row: the 1 px line and its margins.
    pub fn separator_height(&self) -> f32 {
        1.0 + 2.0 * self.separator_margin
    }

    /// Height `entry` lays out to, `None` for a `Custom` row (its own widget).
    pub fn entry_height<M: Clone>(&self, entry: &DropdownEntry<M>, has_icons: bool) -> Option<f32> {
        match entry {
            DropdownEntry::Item(_) => Some(self.item_height(has_icons)),
            DropdownEntry::Header(_) => Some(self.header_row_height()),
            DropdownEntry::Separator => Some(self.separator_height()),
            DropdownEntry::Custom(_) => None,
        }
    }

    /// Height of the card for `entries`, `None` when a `Custom` row has no
    /// known height.
    pub fn card_height<M: Clone>(&self, entries: &[DropdownEntry<M>]) -> Option<f32> {
        let has_icons = has_icons(entries);
        entries
            .iter()
            .map(|e| self.entry_height(e, has_icons))
            .sum::<Option<f32>>()
            .map(|rows| rows + 2.0 * self.panel_pad)
    }

    /// Top of the open submenu's launcher row (a submenu row marked active),
    /// measured from the top of the card; `None` when no launcher is open or
    /// a `Custom` row above it has no known height.
    pub fn active_submenu_top<M: Clone>(&self, entries: &[DropdownEntry<M>]) -> Option<f32> {
        let has_icons = has_icons(entries);
        let mut y = self.panel_pad;
        for entry in entries {
            if let DropdownEntry::Item(item) = entry
                && item.submenu
                && item.active
            {
                return Some(y);
            }
            y += self.entry_height(entry, has_icons)?;
        }
        None
    }

    /// Width a card of `entries` needs, clamped to this look's fitted range
    /// and to `max_w`. It mirrors the row layout: panel padding, row
    /// padding, the icon column when a row has an icon, the label, the gap
    /// before the shortcut column, the shortcut, the check and the chevron
    /// column. Labels are measured in `ui_family`, the face the menu draws
    /// in, and shortcuts in the face their column draws in. `Custom` rows
    /// are not measured; a caller with custom rows sizes for them itself.
    pub fn fitted_width<M: Clone>(
        &self,
        entries: &[DropdownEntry<M>],
        max_w: f32,
        ui_family: &str,
    ) -> f32 {
        let family = signex_types::font::nonblank_family(ui_family);
        let shortcut_family = if self.mono_shortcut {
            Some(MONO_FAMILY)
        } else {
            family
        };
        let advance = signex_types::font::estimate_text_width_px;
        let has_icons = has_icons(entries);
        let has_chevrons = has_chevrons(entries);
        let widest_row = entries
            .iter()
            .filter_map(|entry| match entry {
                DropdownEntry::Item(item) => {
                    let mut w = advance(&item.label, self.label_size, family);
                    if has_icons {
                        w += self.icon_size + self.row_spacing;
                    }
                    w += self.row_spacing + self.cluster_gap;
                    if let Some(s) = &item.shortcut {
                        w += self.row_spacing + advance(s, self.shortcut_size, shortcut_family);
                    }
                    if item.checked {
                        w += self.row_spacing + self.check_size;
                    }
                    if has_chevrons {
                        w += self.row_spacing + self.chevron_width;
                    }
                    Some(w)
                }
                DropdownEntry::Header(label) => Some(advance(label, self.header_size, family)),
                _ => None,
            })
            .fold(0.0, f32::max);
        let upper = max_w.min(self.max_width);
        (widest_row + 2.0 * self.row_pad_x + 2.0 * self.panel_pad)
            .ceil()
            .max(self.min_width)
            .min(upper)
    }
}

/// iced's default line for a text of `size` px (the labels set no line height).
fn line_height(size: f32) -> f32 {
    iced::widget::text::LineHeight::default()
        .to_absolute(iced::Pixels(size))
        .0
}

/// Whether any item row of `entries` carries an icon: the menu then has an
/// icon column.
pub(super) fn has_icons<M: Clone>(entries: &[DropdownEntry<M>]) -> bool {
    entries
        .iter()
        .any(|entry| matches!(entry, DropdownEntry::Item(item) if item.icon.is_some()))
}

/// Whether any item row of `entries` opens a submenu: the menu then has a
/// chevron column.
pub(super) fn has_chevrons<M: Clone>(entries: &[DropdownEntry<M>]) -> bool {
    entries.iter().any(|entry| {
        matches!(
            entry,
            DropdownEntry::Item(DropdownItem { submenu: true, .. })
        )
    })
}
