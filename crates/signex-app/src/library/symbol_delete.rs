//! Who still uses a library symbol that is about to be deleted (#1845).
//!
//! The SCH Library panel's Delete asks first when a component row binds the
//! symbol or an open sheet has parts placed from it. The scan reads only
//! state already in memory (the cached rows of the open libraries and the
//! open sheets), so it is safe on the `update` path.

use std::path::{Path, PathBuf};

use signex_library::PrimitiveRef;
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::state::OpenLibrary;
use crate::panels::SymbolDeleteWarning;

/// Names listed per group before the rest is counted.
const MAX_LISTED: usize = 6;

/// A Delete waiting for the user's answer.
#[derive(Debug, Clone)]
pub struct PendingSymbolDelete {
    pub path: PathBuf,
    pub symbol: Uuid,
    /// `(undo, redo)` depths of the editor when the warning was raised; an
    /// edit, undo or redo since then makes the warning stale.
    pub history: (usize, usize),
    /// How many rows and open sheets the warning named.
    pub rows: usize,
    pub sheets: usize,
    /// Tables (or whole table lists) that could not be read.
    pub unchecked: usize,
    pub warning: SymbolDeleteWarning,
}

impl PendingSymbolDelete {
    /// The warning still describes `editor`: same container, same history,
    /// and the symbol it names is still at its slot.
    pub fn is_current(&self, editor: &crate::app::SymbolDocument) -> bool {
        editor.path == self.path
            && (editor.undo_snapshots.len(), editor.redo_snapshots.len()) == self.history
            && editor
                .file
                .symbols
                .get(self.warning.idx)
                .is_some_and(|s| s.uuid == self.symbol)
    }
}

/// What the scan found for one symbol.
#[derive(Debug, Default)]
pub struct SymbolUsage {
    /// One line per kind of finding (rows, open sheets, unreadable tables);
    /// empty when nothing uses the symbol and nothing was left unchecked.
    pub lines: Vec<String>,
    pub rows: usize,
    pub sheets: usize,
    /// Unreadable tables plus libraries whose table list failed.
    pub unchecked: usize,
}

/// Rows and open sheets that use `symbol`, plus a line for every table of a
/// mounted library that could not be read: what binds into it is unknown.
pub fn usage_lines<'a>(
    libs: &[OpenLibrary],
    sheets: impl Iterator<Item = (&'a Path, &'a SchematicSheet)>,
    symbol: &PrimitiveRef,
) -> SymbolUsage {
    let mut rows = Vec::new();
    let mut unchecked = Vec::new();
    for lib in libs {
        if lib.dangling.tables_not_listed {
            unchecked.push(format!(
                "Tables of {} could not be listed; its rows were not checked",
                lib.display_name
            ));
        }
        for table in &lib.dangling.unreadable_tables {
            unchecked.push(format!(
                "Table {table} ({}) could not be read; its rows were not checked",
                lib.display_name
            ));
        }
        let mut tables: Vec<_> = lib.tables.iter().collect();
        tables.sort_by(|a, b| a.0.cmp(b.0));
        for (table, list) in tables {
            for row in list.iter().filter(|r| r.binds_symbol(symbol)) {
                rows.push(format!(
                    "{} ({} / {})",
                    row.internal_pn.as_str(),
                    lib.display_name,
                    table
                ));
            }
        }
    }
    let lib_id = super::place::lib_id_of(symbol);
    let placed = placed_on_sheets(sheets, |id| id == lib_id);
    let mut lines = Vec::new();
    if !rows.is_empty() {
        lines.push(format!("Component rows: {}", capped(&rows)));
    }
    if !placed.is_empty() {
        lines.push(format!(
            "Open sheets with placed parts: {}",
            capped(&placed)
        ));
    }
    let unchecked_count = unchecked.len();
    lines.extend(unchecked);
    SymbolUsage {
        lines,
        rows: rows.len(),
        sheets: placed.len(),
        unchecked: unchecked_count,
    }
}

/// One entry per open sheet that has parts whose `lib_id` satisfies
/// `is_symbol`: the sheet file and the references placed from it.
fn placed_on_sheets<'a>(
    sheets: impl Iterator<Item = (&'a Path, &'a SchematicSheet)>,
    is_symbol: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut placed = Vec::new();
    for (path, sheet) in sheets {
        let refs: Vec<&str> = sheet
            .symbols
            .iter()
            .filter(|s| is_symbol(&s.lib_id))
            .map(|s| s.reference.as_str())
            .collect();
        if !refs.is_empty() {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            placed.push(format!("{name}: {}", refs.join(", ")));
        }
    }
    placed.sort();
    placed
}

/// The scan for a container whose library is unknown: it is outside every
/// mounted library, or (`shared_folder`) several share its folder. Its
/// component rows cannot be checked (usage is unknown), but open sheets are
/// still matched by the symbol's uuid, whatever library the part names.
pub fn usage_outside_library<'a>(
    container: &Path,
    sheets: impl Iterator<Item = (&'a Path, &'a SchematicSheet)>,
    symbol: Uuid,
    shared_folder: bool,
) -> SymbolUsage {
    let suffix = format!(":{symbol}");
    let placed = placed_on_sheets(sheets, |id| id.ends_with(&suffix));
    let why = if shared_folder {
        "its library cannot be told apart (several libraries share its folder)"
    } else {
        "it is not inside a mounted library"
    };
    let mut lines = vec![format!(
        "{}: {why}; its component rows were not checked",
        container.display()
    )];
    if !placed.is_empty() {
        lines.push(format!(
            "Open sheets with placed parts: {}",
            capped(&placed)
        ));
    }
    SymbolUsage {
        lines,
        rows: 0,
        sheets: placed.len(),
        unchecked: 1,
    }
}

fn capped(items: &[String]) -> String {
    let mut text = items
        .iter()
        .take(MAX_LISTED)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if items.len() > MAX_LISTED {
        text.push_str(&format!("; and {} more", items.len() - MAX_LISTED));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shared_folder_is_not_called_unmounted() {
        let file = Path::new("/w/symbols/s.snxsym");
        let lines = |shared| {
            usage_outside_library(file, std::iter::empty(), Uuid::nil(), shared)
                .lines
                .join("\n")
        };
        assert!(lines(false).contains("is not inside a mounted library"));
        let shared = lines(true);
        assert!(
            shared.contains("several libraries share its folder"),
            "{shared}"
        );
        assert!(!shared.contains("not inside a mounted library"), "{shared}");
    }

    use crate::library::state::LibraryDisplaySettings;
    use crate::library::test_adapters::{FailingListingAdapter, FailingTableListAdapter};
    use std::collections::HashMap;

    fn library() -> OpenLibrary {
        OpenLibrary {
            root: PathBuf::from("/tmp/a.snxlib"),
            display_name: "Lib".into(),
            library_id: Uuid::now_v7(),
            tables: HashMap::new(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing: Default::default(),
            own_edit_stamp: None,
            display: LibraryDisplaySettings::default(),
        }
    }

    fn scan(lib: &OpenLibrary) -> SymbolUsage {
        let symbol = PrimitiveRef::new(lib.library_id, Uuid::now_v7());
        usage_lines(std::slice::from_ref(lib), std::iter::empty(), &symbol)
    }

    #[test]
    fn a_library_whose_table_list_failed_is_unchecked_until_it_reloads() {
        let mut lib = library();
        assert!(
            lib.reload_tables(&FailingTableListAdapter::new(lib.library_id))
                .is_err()
        );

        let usage = scan(&lib);
        assert_eq!(usage.unchecked, 1);
        assert!(
            usage.lines[0].contains("could not be listed"),
            "{:?}",
            usage.lines
        );

        lib.reload_tables(&FailingListingAdapter::new(lib.library_id))
            .unwrap();
        assert!(scan(&lib).lines.is_empty(), "a good load clears the flag");
    }
}
