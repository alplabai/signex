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
    /// The sheets that were open when the scan began and so were skipped.
    pub skipped_open: Vec<PathBuf>,
}

/// Where the disk scan starts, gathered in `update` from memory.
#[derive(Debug, Clone, Default)]
pub struct ScanPlan {
    /// Sheets to read, and to follow down their `child_sheets`: every
    /// declared page and root of the open projects, plus the children of the
    /// open sheets. Sheets that are open are not listed (memory covers them).
    pub seeds: Vec<PathBuf>,
    /// Sheets open as tabs; a walk that reaches one skips it, its content and
    /// children were already counted from memory.
    pub open: Vec<PathBuf>,
    /// Child references that could not be resolved, so nothing was read.
    pub unresolved: Vec<String>,
}

impl ScanPlan {
    /// There is something only the disk can answer.
    pub fn has_work(&self) -> bool {
        !self.seeds.is_empty() || !self.unresolved.is_empty()
    }
}

use crate::app::project_sheets::host_path_key as key;

/// Read every sheet the plan reaches, children included, and collect the
/// ones that place `target`. A sheet visited twice is read once.
pub fn scan_closed_sheets(plan: &ScanPlan, target: SymbolMatch) -> ClosedSheetScan {
    let mut scan = ClosedSheetScan {
        placed: Vec::new(),
        unreadable: plan.unresolved.clone(),
    };
    let open: std::collections::HashSet<String> = plan.open.iter().map(|p| key(p)).collect();
    let mut visited = std::collections::HashSet::new();
    let mut queue: Vec<PathBuf> = plan.seeds.clone();
    while let Some(path) = queue.pop() {
        let k = key(&path);
        if open.contains(&k) || !visited.insert(k) {
            continue;
        }
        let sheet = match crate::app::project_sheets::load_sheet_disk(&path) {
            Ok(Some(sheet)) => sheet,
            // Declared but absent: nothing was read, so nothing is known.
            Ok(None) => {
                scan.unreadable
                    .push(format!("{} (file not found)", file_name(&path)));
                continue;
            }
            Err(why) => {
                scan.unreadable.push(format!("{} {why}", file_name(&path)));
                continue;
            }
        };
        scan.placed.extend(placed_on_sheets(
            std::iter::once((path.as_path(), &sheet)),
            |id| target.matches(id),
        ));
        let dir = path.parent().unwrap_or_else(|| Path::new(""));
        for child in &sheet.child_sheets {
            use crate::app::project_sheets::{
                ChildReferenceRefusal, resolve_child_reference_quiet,
            };
            match resolve_child_reference_quiet(dir, &child.filename) {
                Ok(resolved) => queue.push(resolved),
                Err(ChildReferenceRefusal::Empty) => {}
                Err(ChildReferenceRefusal::Escapes { reference, .. }) => {
                    scan.unreadable.push(format!(
                        "{}: child sheet {reference} is outside the project folder",
                        file_name(&path)
                    ))
                }
            }
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
