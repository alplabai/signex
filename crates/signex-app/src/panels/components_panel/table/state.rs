//! The Components panel table's model: the category shown, the user's
//! saved setups, open groups, range filters and the open popover. The
//! sorted, grouped and filtered rows live in [`TableCache`], rebuilt in
//! `update` only (docs/architecture/library-scale-bench.md).

use std::collections::{BTreeMap, HashMap, HashSet};

use signex_library::components_view::{GroupPath, ViewSetup};
use signex_library::templates::ColumnKey;

use super::cache::TableCache;
use crate::library::range_filter::RangeFilters;

/// A category of the picker: every part, or one component class.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CategoryKey {
    #[default]
    AllParts,
    /// The parts placed from the panel this session, most recent first
    /// (Caner, 2026-11-27).
    Recent,
    Class(String),
}

impl CategoryKey {
    /// The key a saved setup and the last category are stored under in
    /// `prefs.json`. `*` and `@recent` are not class names, so neither
    /// meets a class.
    pub fn pref_key(&self) -> &str {
        match self {
            CategoryKey::AllParts => "*",
            CategoryKey::Recent => "@recent",
            CategoryKey::Class(class) => class,
        }
    }

    /// The category a [`Self::pref_key`] names.
    pub fn from_pref_key(key: &str) -> CategoryKey {
        match key {
            "*" => CategoryKey::AllParts,
            "@recent" => CategoryKey::Recent,
            class => CategoryKey::Class(class.to_owned()),
        }
    }

    pub fn class(&self) -> Option<&str> {
        match self {
            CategoryKey::AllParts | CategoryKey::Recent => None,
            CategoryKey::Class(class) => Some(class),
        }
    }
}

/// The filters of one category: its range filters and In stock only.
/// Session-scoped; only column, sort and group setups are saved
/// (mockup row 52).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableFilters {
    pub range: RangeFilters,
    pub in_stock: bool,
}

impl TableFilters {
    /// How many filters are active, for the Filter button's badge.
    pub fn active_count(&self) -> usize {
        self.range.active().len() + usize::from(self.in_stock)
    }
}

/// The popover open over the table, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TablePopover {
    Columns,
    Filter,
    /// The right-clicked column's header menu.
    HeaderMenu(ColumnKey),
    /// The group bar's "+ Group" column menu.
    GroupMenu,
}

/// A column being resized: its width when the drag started, the pointer x
/// of the first move, and the width now.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnResize {
    pub column: ColumnKey,
    pub start_width: f32,
    pub start_x: Option<f32>,
    pub width: f32,
}

/// Narrowest and widest a dragged column gets, in logical pixels.
pub const MIN_COLUMN_WIDTH: f32 = 44.0;
pub const MAX_COLUMN_WIDTH: f32 = 420.0;

/// The table body's scroll position, as its scrollable last reported it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TableScroll {
    pub x: f32,
    pub y: f32,
    /// `0.0` until the first report.
    pub viewport_h: f32,
}

/// The table's model.
#[derive(Debug, Clone, Default)]
pub struct ComponentsTable {
    pub category: CategoryKey,
    /// The user's setups, by [`CategoryKey::pref_key`]; a category without
    /// one opens from its configuration (mockup row 52).
    pub setups: BTreeMap<String, ViewSetup>,
    /// Open groups per category, by path; every group starts closed.
    pub open_groups: HashMap<CategoryKey, HashSet<GroupPath>>,
    /// Bumped whenever `open_groups` changes, so the visible lines are
    /// rebuilt only then.
    pub open_generation: u64,
    pub filters: HashMap<CategoryKey, TableFilters>,
    pub popover: Option<TablePopover>,
    /// The Columns picker's search text.
    pub column_search: String,
    /// Shift is held: a header click adds a nested sort key.
    pub shift_held: bool,
    pub resize: Option<ColumnResize>,
    pub scroll: TableScroll,
    /// The view switch: the list without the details pane.
    pub list_only: bool,
    /// Details lists every parameter, not only the pinned ones.
    pub show_all_parameters: bool,
    pub cache: TableCache,
}

impl ComponentsTable {
    /// A table starting from what `prefs.json` holds: the setups and the
    /// category shown last.
    pub fn from_prefs(views: crate::fonts::ComponentsViews) -> Self {
        Self {
            category: views
                .category
                .as_deref()
                .map_or_else(CategoryKey::default, CategoryKey::from_pref_key),
            setups: views.setups,
            ..Self::default()
        }
    }

    /// What `prefs.json` keeps of the table.
    pub fn prefs(&self) -> crate::fonts::ComponentsViews {
        crate::fonts::ComponentsViews {
            category: Some(self.category.pref_key().to_owned()),
            setups: self.setups.clone(),
        }
    }

    pub fn category_is_all(&self) -> bool {
        self.category == CategoryKey::AllParts
    }

    /// The filters of the category shown.
    pub fn filters(&self) -> Option<&TableFilters> {
        self.filters.get(&self.category)
    }

    pub(super) fn filters_mut(&mut self) -> &mut TableFilters {
        self.filters.entry(self.category.clone()).or_default()
    }

    /// Whether the group at `path` of the category shown is open.
    pub fn is_open(&self, path: &GroupPath) -> bool {
        self.open_groups
            .get(&self.category)
            .is_some_and(|open| open.contains(path))
    }

    /// The setup the category shown opens with now: the user's, fitted to
    /// the columns it offers, else its configuration.
    pub(super) fn effective_setup(&self) -> ViewSetup {
        let cache = &self.cache;
        match self.setups.get(self.category.pref_key()) {
            Some(saved) => saved.clone().fitted(&cache.offered),
            None => ViewSetup::from_view(&cache.default_view, &cache.offered),
        }
    }

    /// The width column `column` draws at: a drag in progress, the saved
    /// width, else the column's default.
    pub fn width_of(&self, column: &ColumnKey) -> f32 {
        if let Some(resize) = self.resize.as_ref().filter(|r| r.column == *column) {
            return resize.width;
        }
        self.cache
            .setup
            .widths
            .get(column)
            .copied()
            .unwrap_or_else(|| super::columns::default_width(column, self.cache.template.as_ref()))
    }
}
