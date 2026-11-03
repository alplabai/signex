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

use super::closed_sheets::ClosedSheetScan;
use super::drift::split_lib_id;
use super::state::OpenLibrary;
use crate::panels::SymbolDeleteWarning;

/// Names listed per group before the rest is counted.
const MAX_LISTED: usize = 6;

/// A Delete waiting for the user's answer.
#[derive(Debug, Clone)]
pub struct PendingSymbolDelete {
    pub path: PathBuf,
    pub symbol: Uuid,
    /// The editor's `history_generation` when the warning was raised; any
    /// edit, undo or redo since then makes the warning stale.
    pub history: u64,
    /// Names the scan this warning started; a result carrying another serial
    /// belongs to a warning that was replaced or closed and is dropped.
    pub serial: u64,
    /// How many rows, open sheets and closed sheets the warning named.
    pub rows: usize,
    pub sheets: usize,
    pub closed: usize,
    /// Warning lines before the closed-sheet scan added its own.
    pub base_lines: usize,
    /// Tables (or whole table lists) that could not be read.
    pub unchecked: usize,
    pub warning: SymbolDeleteWarning,
}

impl PendingSymbolDelete {
    /// The warning still describes `editor`: same container, same history,
    /// and the symbol it names is still at its slot.
    pub fn is_current(&self, editor: &crate::app::SymbolDocument) -> bool {
        editor.path == self.path
            && editor.history_generation == self.history
            && editor
                .file
                .symbols
                .get(self.warning.idx)
                .is_some_and(|s| s.uuid == self.symbol)
    }

    /// Fold the closed-sheet scan into the warning, together with the
    /// in-memory usage read again now: rows and open sheets may have changed
    /// while the scan ran. Nothing found and nothing unchecked leaves
    /// `warning.lines` empty: the delete is then safe to run.
    pub fn apply_closed_scan(&mut self, usage: SymbolUsage, scan: &ClosedSheetScan) {
        self.rows = usage.rows;
        self.sheets = usage.sheets;
        self.base_lines = usage.lines.len();
        self.warning.lines = usage.lines;
        if !scan.placed.is_empty() {
            self.warning.lines.push(format!(
                "Closed sheets with placed parts: {}",
                capped(&scan.placed)
            ));
        }
        if !scan.unreadable.is_empty() {
            self.warning.lines.push(format!(
                "Closed sheets not checked: {}",
                capped(&scan.unreadable)
            ));
        }
        self.closed = scan.placed.len();
        self.unchecked = usage.unchecked + scan.unreadable.len();
        self.warning.only_unknown = self.rows == 0 && self.sheets == 0 && self.closed == 0;
        self.warning.checking = false;
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
    let target = (symbol.library_id, symbol.uuid);
    let placed = placed_on_sheets(sheets, |id| split_lib_id(id) == Some(target));
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
pub(super) fn placed_on_sheets<'a>(
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

/// The scan for a container that is not inside a mounted library: its
/// component rows cannot be checked (usage is unknown), but open sheets are
/// still matched by the symbol's uuid, whatever library the part names.
pub fn usage_outside_library<'a>(
    container: &Path,
    sheets: impl Iterator<Item = (&'a Path, &'a SchematicSheet)>,
    symbol: Uuid,
) -> SymbolUsage {
    let placed = placed_on_sheets(sheets, |id| {
        split_lib_id(id).is_some_and(|(_, sym)| sym == symbol)
    });
    let mut lines = vec![format!(
        "{} is not inside a mounted library; its component rows were not checked",
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

    fn sheet_placing(lib_id: &str, reference: &str) -> SchematicSheet {
        let mut sheet = SchematicSheet::default();
        sheet.symbols.push(signex_types::schematic::Symbol {
            lib_id: lib_id.into(),
            reference: reference.into(),
            ..Default::default()
        });
        sheet
    }

    use crate::app::SymbolDocument;
    use signex_library::{Symbol, SymbolFile};

    fn pending_for(editor: &SymbolDocument) -> PendingSymbolDelete {
        PendingSymbolDelete {
            path: editor.path.clone(),
            symbol: editor.file.symbols[0].uuid,
            history: editor.history_generation,
            serial: 1,
            rows: 1,
            sheets: 0,
            closed: 0,
            base_lines: 0,
            unchecked: 0,
            warning: SymbolDeleteWarning {
                idx: 0,
                name: "A".into(),
                only_unknown: false,
                checking: false,
                lines: Vec::new(),
            },
        }
    }

    fn edit(editor: &mut SymbolDocument) {
        assert!(editor.record_edit(None, |e| {
            e.file.symbols[0].name.push('x');
            true
        }));
    }

    fn editor() -> SymbolDocument {
        SymbolDocument::new(
            PathBuf::from("a.snxsym"),
            SymbolFile::from_symbol(Symbol::empty("A")),
        )
    }

    #[test]
    fn undo_then_a_new_edit_leaves_the_warning_stale() {
        // Undo depth is 1 again after undo + edit; only the generation differs.
        let mut ed = editor();
        edit(&mut ed);
        let pending = pending_for(&ed);
        assert!(pending.is_current(&ed));

        assert!(ed.undo());
        assert!(!pending.is_current(&ed), "undo");
        edit(&mut ed);
        assert_eq!(ed.undo_snapshots.len(), 1);
        assert!(!pending.is_current(&ed), "undo then edit");
    }

    #[test]
    fn redo_leaves_the_warning_stale() {
        let mut ed = editor();
        edit(&mut ed);
        assert!(ed.undo());
        let pending = pending_for(&ed);
        assert!(pending.is_current(&ed));

        assert!(ed.redo());
        assert!(!pending.is_current(&ed));
    }

    #[test]
    fn a_reopened_editor_does_not_revive_the_warning() {
        let (mut app, _task) = crate::app::Signex::new();
        let path = PathBuf::from("a.snxsym");
        let file = SymbolFile::from_symbol(Symbol::empty("A"));

        app.open_symbol_editor_tab(path.clone(), file.clone());
        edit(app.document_state.symbol_editors.get_mut(&path).unwrap());
        let pending = pending_for(&app.document_state.symbol_editors[&path]);
        assert!(pending.is_current(&app.document_state.symbol_editors[&path]));

        // Close, reopen and make the same single edit.
        app.document_state.symbol_editors.remove(&path);
        app.document_state.tabs.clear();
        app.open_symbol_editor_tab(path.clone(), file);
        edit(app.document_state.symbol_editors.get_mut(&path).unwrap());
        assert!(!pending.is_current(&app.document_state.symbol_editors[&path]));
    }

    #[test]
    fn an_upper_case_lib_id_on_a_sheet_is_still_named() {
        let lib = library();
        let symbol = PrimitiveRef::new(lib.library_id, Uuid::now_v7());
        let upper = super::super::place::lib_id_of(&symbol).to_uppercase();
        let sheet = sheet_placing(&upper, "U7");
        let path = PathBuf::from("/tmp/a.snxsch");

        let usage = usage_lines(
            std::slice::from_ref(&lib),
            std::iter::once((path.as_path(), &sheet)),
            &symbol,
        );
        assert_eq!(usage.sheets, 1);
        assert!(usage.lines.join("\n").contains("U7"), "{:?}", usage.lines);

        let outside = usage_outside_library(
            Path::new("/tmp/x.snxsym"),
            std::iter::once((path.as_path(), &sheet)),
            symbol.uuid,
        );
        assert_eq!(outside.sheets, 1);
    }

    #[test]
    fn a_closed_scan_replaces_the_checking_line_and_counts_unchecked_sheets() {
        let mut pending = pending_for(&editor());
        pending.rows = 0;
        pending.base_lines = 0;
        pending.warning.only_unknown = true;
        pending.warning.checking = true;
        pending.warning.lines = vec!["Checking closed sheets of the open projects…".into()];
        assert_eq!(
            pending.warning.heading(),
            "Checking whether symbol \"A\" is in use"
        );

        pending.apply_closed_scan(
            SymbolUsage::default(),
            &ClosedSheetScan {
                placed: Vec::new(),
                unreadable: vec!["x.snxsch could not be read".into()],
            },
        );

        assert_eq!(pending.unchecked, 1);
        assert_eq!(
            pending.warning.lines,
            ["Closed sheets not checked: x.snxsch could not be read"]
        );
        assert!(pending.warning.only_unknown && !pending.warning.checking);
        assert_eq!(
            pending.warning.heading(),
            "Symbol \"A\" may still be in use"
        );

        pending.apply_closed_scan(
            SymbolUsage::default(),
            &ClosedSheetScan {
                placed: vec!["y.snxsch: U1".into()],
                unreadable: Vec::new(),
            },
        );
        assert!(!pending.warning.only_unknown);
        assert_eq!(pending.warning.heading(), "Symbol \"A\" is still in use");
    }
}
