//! Closed sheets of the open projects, scanned for a symbol that is about to
//! be deleted (#1903). Blocking disk reads: run on a worker, never in `update`.
//!
//! A sheet that cannot be read is reported as unchecked, never as unused: a
//! failed read must not make a delete look safe.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::drift::split_lib_id;
use super::symbol_delete::placed_on_sheets;

/// Which placed parts count as the symbol being deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolMatch {
    /// The symbol of this library (library uuid, symbol uuid).
    InLibrary(Uuid, Uuid),
    /// The container is outside every mounted library: match by symbol uuid.
    AnyLibrary(Uuid),
}

impl SymbolMatch {
    fn matches(self, lib_id: &str) -> bool {
        match (self, split_lib_id(lib_id)) {
            (Self::InLibrary(lib, sym), Some(found)) => found == (lib, sym),
            (Self::AnyLibrary(sym), Some((_, found))) => found == sym,
            (_, None) => false,
        }
    }
}

/// What the disk scan found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedSheetScan {
    /// `sheet.snxsch: U1, U2` for every sheet that places the symbol.
    pub placed: Vec<String>,
    /// `sheet.snxsch (why)` for every sheet that could not be checked.
    pub unreadable: Vec<String>,
}

impl ClosedSheetScan {
    /// The worker died before it finished: nothing was checked.
    pub fn unfinished(reason: &str) -> Self {
        Self {
            placed: Vec::new(),
            unreadable: vec![format!("closed sheets ({reason})")],
        }
    }
}

/// The result as it travels back to `update`; `serial` names the warning that
/// asked for it.
#[derive(Debug, Clone)]
pub struct ClosedSheetsReport {
    pub serial: u64,
    pub scan: ClosedSheetScan,
}

/// Read every sheet in `paths` and collect the ones that place `target`.
pub fn scan_closed_sheets(paths: &[PathBuf], target: SymbolMatch) -> ClosedSheetScan {
    let mut scan = ClosedSheetScan::default();
    for path in paths {
        match crate::app::project_sheets::load_sheet_disk(path) {
            Ok(Some(sheet)) => scan.placed.extend(placed_on_sheets(
                std::iter::once((path.as_path(), &sheet)),
                |id| target.matches(id),
            )),
            // Declared but absent: nothing was read, so nothing is known.
            Ok(None) => scan
                .unreadable
                .push(format!("{} (file not found)", file_name(path))),
            Err(why) => scan.unreadable.push(format!("{} {why}", file_name(path))),
        }
    }
    scan.placed.sort();
    scan.unreadable.sort();
    scan
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests;
