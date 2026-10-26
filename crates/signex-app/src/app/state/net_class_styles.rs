//! The resolved net-class style of every drawn wire and junction.
//!
//! The class table lives in the `.snxprj`; a net's members come from the
//! sheet's netlist. The canvas needs only the join of the two, per element,
//! and reads it from here rather than from the project, so `view` never
//! touches project data. [`NetClassStyleCache::refresh`] rebuilds a sheet's
//! entry only when its engine revision or the class table changed; it runs
//! after an update, never inside `view`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::net_class::{NetClass, NetClassStyle};
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

/// One sheet the cache should hold a resolution for.
pub struct SheetClassInput<'a> {
    pub path: &'a Path,
    /// The engine revision of the sheet; changes on every document edit.
    pub revision: u64,
    pub sheet: &'a SchematicSheet,
    /// The class table of the project that owns the sheet (empty for none).
    pub classes: &'a [NetClass],
}

struct SheetEntry {
    revision: u64,
    classes: Vec<NetClass>,
    styles: HashMap<Uuid, NetClassStyle>,
}

#[derive(Default)]
pub struct NetClassStyleCache {
    sheets: HashMap<PathBuf, SheetEntry>,
    merged: HashMap<Uuid, NetClassStyle>,
}

impl NetClassStyleCache {
    /// Element uuid to class style, over every cached sheet. Element uuids
    /// are unique across sheets, so one flat map serves every window.
    #[must_use]
    pub fn styles(&self) -> &HashMap<Uuid, NetClassStyle> {
        &self.merged
    }

    /// `true` when no element carries a class style, so a caller with no
    /// class table anywhere can skip gathering inputs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.merged.is_empty()
    }

    /// Bring the cache in line with `inputs`. Returns `true` when the merged
    /// map changed, so the caller can drop its drawn-content caches.
    pub fn refresh<'a>(&mut self, inputs: impl IntoIterator<Item = SheetClassInput<'a>>) -> bool {
        let mut seen: Vec<PathBuf> = Vec::new();
        let mut dirty = false;
        for input in inputs {
            seen.push(input.path.to_path_buf());
            let current = self.sheets.get(input.path).is_some_and(|entry| {
                entry.revision == input.revision && entry.classes == input.classes
            });
            if current {
                continue;
            }
            let styles = if input.classes.is_empty() {
                HashMap::new()
            } else {
                let netlist = signex_net::build_netlist(input.sheet);
                signex_net::element_class_styles(&netlist, input.classes)
            };
            let changed_styles = self
                .sheets
                .get(input.path)
                .is_none_or(|entry| entry.styles != styles);
            dirty |= changed_styles;
            self.sheets.insert(
                input.path.to_path_buf(),
                SheetEntry {
                    revision: input.revision,
                    classes: input.classes.to_vec(),
                    styles,
                },
            );
        }
        let before = self.sheets.len();
        self.sheets.retain(|path, _| seen.contains(path));
        if self.sheets.len() != before {
            dirty = true;
        }
        if dirty {
            self.merged = self
                .sheets
                .values()
                .flat_map(|entry| entry.styles.iter().map(|(id, style)| (*id, *style)))
                .collect();
        }
        dirty
    }
}

#[cfg(test)]
mod tests;
