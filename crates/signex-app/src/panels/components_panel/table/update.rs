//! What each table message does to the model. Pure: the dispatcher turns
//! the returned [`TableEffects`] into tasks (the preference write, the
//! header scroll) and rebuilds the cache afterwards.

use std::collections::HashSet;

use signex_library::components_view::{ViewSetup, group_paths};
use signex_library::templates::ColumnKey;

use super::state::{
    ColumnResize, ComponentsTable, MAX_COLUMN_WIDTH, MIN_COLUMN_WIDTH, TablePopover,
};
use crate::library::messages::{ComponentsTableMsg, HeaderAction, RangeBound, RangeFilterMsg};

/// What the dispatcher owes after a message.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableEffects {
    /// The saved setups or the category changed: write them to
    /// `prefs.json`, off the UI thread.
    pub save_prefs: bool,
    /// The body scrolled sideways: move the header strip with it.
    pub header_x: Option<f32>,
    /// The body is to scroll back to its top (a new category, Collapse
    /// all).
    pub body_to_top: bool,
}

impl ComponentsTable {
    /// Apply one message.
    pub fn apply(&mut self, msg: ComponentsTableMsg) -> TableEffects {
        let mut fx = TableEffects::default();
        match msg {
            ComponentsTableMsg::CategoryPicked(key) => {
                if self.category != key {
                    self.category = key;
                    self.popover = None;
                    self.resize = None;
                    fx.body_to_top = true;
                    // The last category is remembered (Caner, 2026-11-27).
                    fx.save_prefs = true;
                }
            }
            ComponentsTableMsg::HeaderPressed(column) => {
                // A resize whose release was lost outside the table ends
                // here instead of sorting.
                if self.resize.is_some() {
                    return self.apply(ComponentsTableMsg::ResizeEnded);
                }
                let nested = self.shift_held;
                self.edit_setup(&mut fx, |s| s.click_sort(&column, nested));
            }
            ComponentsTableMsg::HeaderRightClicked(column) => {
                self.popover = Some(TablePopover::HeaderMenu(column));
            }
            ComponentsTableMsg::HeaderMenuPicked(action) => self.header_menu(action, &mut fx),
            ComponentsTableMsg::ResizeStarted(column) => {
                let width = self.width_of(&column);
                self.resize = Some(ColumnResize {
                    column,
                    start_width: width,
                    start_x: None,
                    width,
                });
            }
            ComponentsTableMsg::ResizeMoved(x) => {
                if let Some(r) = self.resize.as_mut() {
                    let start = *r.start_x.get_or_insert(x);
                    r.width = (r.start_width + x - start).clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
                }
            }
            ComponentsTableMsg::ResizeEnded => {
                if let Some(r) = self.resize.take()
                    && r.width != r.start_width
                {
                    self.edit_setup(&mut fx, |s| s.set_width(&r.column, r.width));
                }
            }
            ComponentsTableMsg::GroupToggled(path) => {
                let open = self.open_groups.entry(self.category.clone()).or_default();
                if !open.remove(&path) {
                    open.insert(path);
                }
                self.open_generation += 1;
            }
            ComponentsTableMsg::ExpandAllPressed => {
                let all: HashSet<_> = group_paths(&self.cache.nodes).into_iter().collect();
                self.open_groups.insert(self.category.clone(), all);
                self.open_generation += 1;
            }
            ComponentsTableMsg::CollapseAllPressed => {
                self.open_groups.remove(&self.category);
                self.open_generation += 1;
                fx.body_to_top = true;
            }
            ComponentsTableMsg::GroupMenuToggled => self.toggle_popover(TablePopover::GroupMenu),
            ComponentsTableMsg::GroupAdded(column) => {
                self.popover = None;
                self.edit_setup(&mut fx, |s| {
                    let innermost = s.group.len();
                    s.insert_group(&column, innermost);
                });
            }
            ComponentsTableMsg::GroupFlipped(level) => {
                self.edit_setup(&mut fx, |s| s.flip_group(level));
            }
            ComponentsTableMsg::GroupRemoved(level) => {
                self.edit_setup(&mut fx, |s| s.remove_group(level));
            }
            ComponentsTableMsg::ColumnsToggled => {
                self.column_search.clear();
                self.toggle_popover(TablePopover::Columns);
            }
            ComponentsTableMsg::ColumnShown { column, shown } => {
                self.edit_setup(&mut fx, |s| {
                    if shown {
                        s.show_column(&column);
                    } else {
                        s.hide_column(&column);
                    }
                });
            }
            ComponentsTableMsg::ColumnMoved { column, later } => {
                let delta = if later { 1 } else { -1 };
                self.edit_setup(&mut fx, |s| s.move_column_by(&column, delta));
            }
            ComponentsTableMsg::ColumnSearchChanged(text) => self.column_search = text,
            ComponentsTableMsg::ResetPressed => {
                // Back to the category's configuration, which then follows
                // later template edits. Pins are not part of the setup's
                // default and stay.
                let key = self.category.pref_key().to_owned();
                if let Some(saved) = self.setups.remove(&key) {
                    if !saved.pinned.is_empty() && saved.pinned != self.cache.default_view.pinned {
                        let mut kept =
                            ViewSetup::from_view(&self.cache.default_view, &self.cache.offered);
                        kept.pinned = saved.pinned;
                        self.setups.insert(key, kept);
                    }
                    fx.save_prefs = true;
                }
            }
            ComponentsTableMsg::FilterToggled => self.toggle_popover(TablePopover::Filter),
            ComponentsTableMsg::InStockToggled(on) => self.filters_mut().in_stock = on,
            ComponentsTableMsg::RangeFilter(msg) => self.range_filter(msg),
            ComponentsTableMsg::BoundPrefixPicked { key, high, prefix } => {
                let range = &mut self.filters_mut().range;
                range.set_any_prefix(&key, high, prefix.is_none());
                if let Some(prefix) = prefix {
                    let bound = if high {
                        RangeBound::High
                    } else {
                        RangeBound::Low
                    };
                    range.apply(RangeFilterMsg::PrefixPicked { key, bound, prefix });
                }
            }
            ComponentsTableMsg::RangeTargetPicked { key, on } => {
                self.filters_mut().range.set_target(&key, on);
            }
            ComponentsTableMsg::FiltersCleared => {
                self.filters.remove(&self.category);
            }
            ComponentsTableMsg::PopoverDismissed => self.popover = None,
            ComponentsTableMsg::ListOnlyPicked(list_only) => self.list_only = list_only,
            ComponentsTableMsg::Scrolled { x, y, viewport_h } => {
                if x != self.scroll.x {
                    fx.header_x = Some(x);
                }
                self.scroll.x = x;
                self.scroll.y = y;
                self.scroll.viewport_h = viewport_h;
            }
            ComponentsTableMsg::PinToggled(name) => self.toggle_pin(&name, &mut fx),
            ComponentsTableMsg::ShowAllParametersToggled => {
                self.show_all_parameters = !self.show_all_parameters;
            }
        }
        fx
    }

    /// Change the category's setup and mark it saved per user (decision 1).
    fn edit_setup(&mut self, fx: &mut TableEffects, edit: impl FnOnce(&mut ViewSetup)) {
        let mut setup = self.cache.setup.clone();
        edit(&mut setup);
        if setup == self.cache.setup {
            return;
        }
        self.cache.setup = setup.clone();
        self.setups
            .insert(self.category.pref_key().to_owned(), setup);
        fx.save_prefs = true;
    }

    fn header_menu(&mut self, action: HeaderAction, fx: &mut TableEffects) {
        let Some(TablePopover::HeaderMenu(column)) = self.popover.take() else {
            return;
        };
        match action {
            HeaderAction::SortAscending => self.edit_setup(fx, |s| s.sort_only(&column, false)),
            HeaderAction::SortDescending => self.edit_setup(fx, |s| s.sort_only(&column, true)),
            HeaderAction::ToggleSort => self.edit_setup(fx, |s| s.toggle_in_sort(&column)),
            HeaderAction::ToggleGroup => self.edit_setup(fx, |s| s.toggle_group(&column)),
            HeaderAction::Hide => self.edit_setup(fx, |s| s.hide_column(&column)),
            HeaderAction::ResetWidth => self.edit_setup(fx, |s| s.reset_width(&column)),
            HeaderAction::OpenColumns => {
                self.column_search.clear();
                self.popover = Some(TablePopover::Columns);
            }
        }
    }

    fn toggle_popover(&mut self, which: TablePopover) {
        self.popover = if self.popover.as_ref() == Some(&which) {
            None
        } else {
            Some(which)
        };
    }

    fn range_filter(&mut self, msg: RangeFilterMsg) {
        let picked = match &msg {
            RangeFilterMsg::ColumnPicked { key, .. } => Some(key.clone()),
            _ => None,
        };
        let is_new = picked
            .as_ref()
            .is_some_and(|key| self.filters().is_none_or(|f| f.range.get(key).is_none()));
        let min_to_max = picked.as_ref().is_some_and(|key| self.cache.is_range(key));
        let filters = self.filters_mut();
        filters.range.apply(msg);
        if let (Some(key), true) = (picked, is_new) {
            filters.range.start_panel_filter(&key, min_to_max);
        }
    }

    /// Pin or unpin `name` for the selected part's category.
    fn toggle_pin(&mut self, name: &str, fx: &mut TableEffects) {
        let details = &self.cache.details;
        if details.key.is_none() {
            return;
        }
        let key = details.category.pref_key().to_owned();
        let mut setup = match self.setups.get(&key) {
            Some(saved) => saved.clone(),
            None => ViewSetup::from_view(&details.view, &details.offered),
        };
        setup.toggle_pin(name);
        if details.category == self.category {
            self.cache.setup.pinned = setup.pinned.clone();
        }
        self.setups.insert(key, setup);
        fx.save_prefs = true;
    }
}

/// The columns "+ Group" offers: groupable ones not yet a level.
pub fn group_choices(setup: &ViewSetup) -> Vec<ColumnKey> {
    setup
        .order
        .iter()
        .filter(|c| super::columns::is_groupable(c))
        .filter(|c| !setup.group.iter().any(|k| k.column == **c))
        .cloned()
        .collect()
}
