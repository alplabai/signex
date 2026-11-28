//! The Components panel table (mockup row 52): one category at a time, its parameter columns, nested sort and
//! grouping, range filters and the Details parameter pins.
//!
//! * [`state`]: the model, saved setups and session filters.
//! * [`cache`]: rows, sort, groups and lines, rebuilt in `update` only.
//! * [`update`]: what each [`ComponentsTableMsg`] does.
//! * the views: [`view::toolbar`], [`view::table_block`] and the Details
//!   parameter lines ([`detail_lines`], [`details_extras`]).
//!
//! The domain (cell values, ordering, grouping, setups, filters) is
//! `signex_library::components_view` and `signex_library::range_filter`;
//! this module only keeps state and draws. See
//! docs/architecture/components-panel.md.
//!
//! [`ComponentsTableMsg`]: crate::library::messages::ComponentsTableMsg

mod body;
pub mod cache;
mod category;
pub(crate) mod cells;
pub(crate) mod columns;
mod details;
mod group_bar;
mod header;
mod popovers;
mod state;
mod templates;
mod update;
mod view;

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod kept_tests;
#[cfg(test)]
mod templates_tests;
#[cfg(test)]
mod test_fixture;
#[cfg(test)]
mod update_tests;
#[cfg(test)]
mod view_tests;

pub(crate) use body::{BODY_SCROLL, drawn_rows};
pub use cache::{TableCache, TableLine, TableRow};
pub(crate) use cache::{TableSource, sync};
pub use category::CategoryEntry;
pub(crate) use details::{DetailLine, detail_lines, details_extras};
pub(crate) use header::HEADER_SCROLL;
pub use state::{CategoryKey, ComponentsTable, TableFilters, TablePopover};
pub use update::TableEffects;
pub(crate) use update::ends_column_resize;
pub(crate) use view::{table_block, toolbar};
