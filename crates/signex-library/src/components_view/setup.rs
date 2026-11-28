//! A user's saved column, sort and group setup for one category
//! (mockup row 52). It starts from the category's
//! configuration ([`CategoryView`]), every change is saved per user, and
//! Reset to category default returns to the configuration.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::component::{ComponentRow, DESIGNATOR_PREFIX_PARAM};
use crate::templates::{CategoryView, ColumnKey, ParameterTemplate, SortKey};

/// One category's setup. Serialisable for the user's preferences, keyed
/// there by category.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewSetup {
    /// Every column the category offers, in display order, part number
    /// first; hidden ones keep their place for when they are shown again.
    pub order: Vec<ColumnKey>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub hidden: BTreeSet<ColumnKey>,
    /// Widths the user dragged, in logical pixels; others use the panel's
    /// default for the column.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub widths: BTreeMap<ColumnKey, f32>,
    #[serde(default)]
    pub sort: Vec<SortKey>,
    #[serde(default)]
    pub group: Vec<SortKey>,
    /// Parameters the Details card lists first.
    #[serde(default)]
    pub pinned: Vec<String>,
}

/// The view a category opens with: All parts without a template, else the
/// template's ([`ParameterTemplate::category_view`]).
pub fn category_view(template: Option<&ParameterTemplate>) -> CategoryView {
    template.map_or_else(CategoryView::all_parts, ParameterTemplate::category_view)
}

/// Every column the category offers, for the column picker: part number,
/// the template's parameter columns, then parameters rows carry that the
/// template does not name (by name), then the other row fields. Without a
/// template (All parts) only the row fields.
pub fn offered_columns<'r>(
    template: Option<&ParameterTemplate>,
    rows: impl IntoIterator<Item = &'r ComponentRow>,
) -> Vec<ColumnKey> {
    let Some(template) = template else {
        return ColumnKey::ROW_FIELDS.to_vec();
    };
    let mut out = vec![ColumnKey::PartNumber];
    out.extend(template.parameter_columns());
    let covered = |name: &str| {
        template.slot(name).is_some()
            || template.range(name).is_some()
            || template.ranges.iter().any(|r| r.holds(name))
    };
    let free: BTreeSet<&str> = rows
        .into_iter()
        .flat_map(|row| row.parameters.keys())
        .map(String::as_str)
        .filter(|name| *name != DESIGNATOR_PREFIX_PARAM && !covered(name))
        .collect();
    out.extend(free.into_iter().map(ColumnKey::param));
    out.extend(ColumnKey::ROW_FIELDS.into_iter().skip(1));
    out
}

impl ViewSetup {
    /// The setup `view` describes over the `offered` columns: its columns
    /// visible in its order, every other offered column hidden after them.
    /// A column, sort or group key the category does not offer is left out.
    pub fn from_view(view: &CategoryView, offered: &[ColumnKey]) -> ViewSetup {
        let mut order = vec![ColumnKey::PartNumber];
        for column in view.columns.iter().chain(offered) {
            if !order.contains(column) {
                order.push(column.clone());
            }
        }
        let hidden = order
            .iter()
            .filter(|c| !view.columns.contains(*c) && **c != ColumnKey::PartNumber)
            .cloned()
            .collect();
        ViewSetup {
            order,
            hidden,
            widths: BTreeMap::new(),
            sort: view.sort.clone(),
            group: view.group.clone(),
            pinned: view.pinned.clone(),
        }
        .fitted(offered)
    }

    /// Visible columns, in order.
    pub fn visible_columns(&self) -> Vec<ColumnKey> {
        self.order
            .iter()
            .filter(|c| !self.hidden.contains(*c))
            .cloned()
            .collect()
    }

    /// Whether this is still the category's configuration over the
    /// `offered` columns: the same visible columns, sort and grouping, and
    /// no dragged width (the Columns picker greys Reset to category
    /// default then). Pins are not part of it.
    pub fn is_category_default(&self, view: &CategoryView, offered: &[ColumnKey]) -> bool {
        let default = ViewSetup::from_view(view, offered);
        self.visible_columns() == default.visible_columns()
            && self.sort == default.sort
            && self.group == default.group
            && self.widths.is_empty()
    }

    /// This setup fitted to what the category offers now (a template may
    /// have changed since it was saved): columns no longer offered leave,
    /// sort and group keys on them too, new ones arrive hidden at the end,
    /// and part number is first and visible.
    pub fn fitted(mut self, offered: &[ColumnKey]) -> ViewSetup {
        let mut order = vec![ColumnKey::PartNumber];
        for column in self.order.iter().chain(offered) {
            if offered.contains(column) && !order.contains(column) {
                if !self.order.contains(column) {
                    self.hidden.insert(column.clone());
                }
                order.push(column.clone());
            }
        }
        self.order = order;
        self.hidden
            .retain(|c| offered.contains(c) && *c != ColumnKey::PartNumber);
        self.widths
            .retain(|c, w| offered.contains(c) && w.is_finite() && *w > 0.0);
        self.sort.retain(|k| offered.contains(&k.column));
        self.group.retain(|k| offered.contains(&k.column));
        dedup_keys(&mut self.sort);
        dedup_keys(&mut self.group);
        self
    }

    /// `self`, an edit of `shown` (the `saved` setup fitted to `offered`),
    /// with what `saved` holds for columns not offered now put back: their
    /// place in the order, hidden flag and width, and the saved sort or
    /// grouping when the edit left that alone. An edit made while a
    /// library is unmounted keeps its columns for when it returns (#3236).
    pub fn keeping_unoffered(
        mut self,
        saved: &ViewSetup,
        shown: &ViewSetup,
        offered: &[ColumnKey],
    ) -> ViewSetup {
        let absent = |c: &ColumnKey| !offered.contains(c) && *c != ColumnKey::PartNumber;
        // Each absent column goes back after the offered column it
        // followed in the saved order.
        let mut after: Option<usize> = None;
        for column in &saved.order {
            if !absent(column) {
                if let Some(at) = self.order.iter().position(|c| c == column) {
                    after = Some(at);
                }
            } else if !self.order.contains(column) {
                let at = after.map_or(1, |i| i + 1).min(self.order.len());
                self.order.insert(at, column.clone());
                after = Some(at);
            }
        }
        self.hidden
            .extend(saved.hidden.iter().filter(|c| absent(*c)).cloned());
        for (column, width) in &saved.widths {
            if absent(column) {
                self.widths.entry(column.clone()).or_insert(*width);
            }
        }
        if self.sort == shown.sort {
            self.sort = saved.sort.clone();
        }
        if self.group == shown.group {
            self.group = saved.group.clone();
        }
        self
    }

    /// The Columns picker's list: visible columns in order, then hidden
    /// ones in order, each with whether it shows.
    pub fn column_choices(&self) -> Vec<(ColumnKey, bool)> {
        let visible = self.visible_columns().into_iter().map(|c| (c, true));
        let hidden = self
            .order
            .iter()
            .filter(|c| self.hidden.contains(*c))
            .map(|c| (c.clone(), false));
        visible.chain(hidden).collect()
    }

    /// Show `column` (if offered) in its place in the order.
    pub fn show_column(&mut self, column: &ColumnKey) {
        self.hidden.remove(column);
    }

    /// Hide `column`; part number cannot be hidden.
    pub fn hide_column(&mut self, column: &ColumnKey) {
        if *column != ColumnKey::PartNumber && self.order.contains(column) {
            self.hidden.insert(column.clone());
        }
    }

    /// Drop `column` before (or `after`) `target`, as a header drag does.
    /// Part number stays first.
    pub fn move_column_to(&mut self, column: &ColumnKey, target: &ColumnKey, after: bool) {
        if column == target || *column == ColumnKey::PartNumber {
            return;
        }
        let Some(from) = self.order.iter().position(|c| c == column) else {
            return;
        };
        let moved = self.order.remove(from);
        let Some(at) = self.order.iter().position(|c| c == target) else {
            self.order.insert(from, moved);
            return;
        };
        let at = (at + usize::from(after)).max(1);
        self.order.insert(at, moved);
    }

    /// Swap `column` with its neighbour `delta` places away among its
    /// peers (parameter columns among parameters, row fields among row
    /// fields), as Alt+Up/Down in the Columns picker does. The picker lists
    /// visible columns, then hidden ones, so a visible column swaps only
    /// with visible peers and a hidden one only with hidden peers (#3236).
    pub fn move_column_by(&mut self, column: &ColumnKey, delta: isize) {
        let is_param = column.param_name().is_some();
        let is_hidden = self.hidden.contains(column);
        let peers: Vec<usize> = (0..self.order.len())
            .filter(|&i| {
                let c = &self.order[i];
                *c != ColumnKey::PartNumber
                    && c.param_name().is_some() == is_param
                    && self.hidden.contains(c) == is_hidden
            })
            .collect();
        let Some(i) = peers.iter().position(|&i| self.order[i] == *column) else {
            return;
        };
        let Some(j) = i.checked_add_signed(delta).filter(|j| *j < peers.len()) else {
            return;
        };
        self.order.swap(peers[i], peers[j]);
    }

    /// Set a dragged width; a width that is not a positive number clears it.
    pub fn set_width(&mut self, column: &ColumnKey, width: f32) {
        if width.is_finite() && width > 0.0 {
            self.widths.insert(column.clone(), width);
        } else {
            self.widths.remove(column);
        }
    }

    pub fn reset_width(&mut self, column: &ColumnKey) {
        self.widths.remove(column);
    }

    /// A header click. Plain: sort by this column alone, ascending; on the
    /// only sort key it turns descending, then off. With `nested` (Shift):
    /// add it as the next nested key, ascending; again, descending; again,
    /// out of the sort.
    pub fn click_sort(&mut self, column: &ColumnKey, nested: bool) {
        let at = self.sort.iter().position(|k| k.column == *column);
        match (nested, at) {
            (true, None) => self.sort.push(SortKey::ascending(column.clone())),
            (true, Some(i)) if !self.sort[i].descending => self.sort[i].descending = true,
            (true, Some(i)) => {
                self.sort.remove(i);
            }
            (false, Some(0)) if self.sort.len() == 1 => {
                if self.sort[0].descending {
                    self.sort.clear();
                } else {
                    self.sort[0].descending = true;
                }
            }
            (false, _) => self.sort = vec![SortKey::ascending(column.clone())],
        }
    }

    /// Sort ascending / Sort descending from the header menu: this column
    /// alone.
    pub fn sort_only(&mut self, column: &ColumnKey, descending: bool) {
        self.sort = vec![SortKey {
            column: column.clone(),
            descending,
        }];
    }

    /// Add to sort / Remove from sort from the header menu.
    pub fn toggle_in_sort(&mut self, column: &ColumnKey) {
        toggle_key(&mut self.sort, column);
    }

    /// Group by this / Remove from groups from the header menu: a new
    /// level goes innermost.
    pub fn toggle_group(&mut self, column: &ColumnKey) {
        toggle_key(&mut self.group, column);
    }

    /// Drop `column` on the group bar at level `at` (moving it there when
    /// it is already a level).
    pub fn insert_group(&mut self, column: &ColumnKey, at: usize) {
        let mut entry = SortKey::ascending(column.clone());
        let mut at = at;
        if let Some(i) = self.group.iter().position(|k| k.column == *column) {
            entry = self.group.remove(i);
            if i < at {
                at -= 1;
            }
        }
        let at = at.min(self.group.len());
        self.group.insert(at, entry);
    }

    /// Flip the direction of group level `level`.
    pub fn flip_group(&mut self, level: usize) {
        if let Some(k) = self.group.get_mut(level) {
            k.descending = !k.descending;
        }
    }

    pub fn remove_group(&mut self, level: usize) {
        if level < self.group.len() {
            self.group.remove(level);
        }
    }

    /// Pin a parameter to the top of Details, or unpin it.
    pub fn toggle_pin(&mut self, name: &str) {
        match self.pinned.iter().position(|p| p == name) {
            Some(i) => {
                self.pinned.remove(i);
            }
            None => self.pinned.push(name.to_string()),
        }
    }
}

fn toggle_key(keys: &mut Vec<SortKey>, column: &ColumnKey) {
    match keys.iter().position(|k| k.column == *column) {
        Some(i) => {
            keys.remove(i);
        }
        None => keys.push(SortKey::ascending(column.clone())),
    }
}

/// Keep the first key per column.
fn dedup_keys(keys: &mut Vec<SortKey>) {
    let mut seen: Vec<ColumnKey> = Vec::new();
    keys.retain(|k| {
        if seen.contains(&k.column) {
            false
        } else {
            seen.push(k.column.clone());
            true
        }
    });
}
