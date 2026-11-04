//! Closed sheets of the open projects, scanned for a symbol that is about to
//! be deleted (#1903). Blocking disk reads: run on a worker, never in `update`.
//!
//! A sheet that cannot be read is reported as unchecked, never as unused: a
//! failed read must not make a delete look safe.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::app::CaseFold;

use crate::app::project_sheets::assemble_from_seeds;

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
    /// Keys of the sheets the walk reached (read or not).
    pub visited: Vec<String>,
    /// The worker died: nothing was read, `unreadable` says so.
    pub unfinished: bool,
    /// How each sheet that was read looked just before it was read; stated
    /// again before an auto-delete to catch a write after the read.
    pub stamps: Vec<(PathBuf, Option<FileStamp>)>,
}

/// A file's modified time and length: cheap to compare, enough to see a write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    modified: Option<std::time::SystemTime>,
    len: u64,
}

impl FileStamp {
    fn of(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        Some(Self {
            modified: meta.modified().ok(),
            len: meta.len(),
        })
    }
}

/// Sheets whose file differs now from when the scan read it. Blocking.
pub fn changed_since_read(stamps: &[(PathBuf, Option<FileStamp>)]) -> Vec<String> {
    stamps
        .iter()
        .filter(|(path, then)| FileStamp::of(path) != *then)
        .map(|(path, _)| format!("{} changed after it was read", file_name(path)))
        .collect()
}

/// The result of [`changed_since_read`], as it travels back to `update`.
#[derive(Debug, Clone)]
pub struct ClosedSheetsVerdict {
    pub serial: u64,
    pub changed: Vec<String>,
}

impl ClosedSheetScan {
    /// The worker died before it finished: nothing was checked.
    pub fn unfinished(reason: &str) -> Self {
        Self {
            placed: Vec::new(),
            unreadable: vec![format!("closed sheets ({reason})")],
            visited: Vec::new(),
            unfinished: true,
            stamps: Vec::new(),
        }
    }
}

/// The result as it travels back to `update`; `serial` names the warning that
/// asked for it.
#[derive(Debug, Clone)]
pub struct ClosedSheetsReport {
    pub serial: u64,
    pub scan: ClosedSheetScan,
    /// What the scan was asked to read; the open sheets in it were skipped.
    pub plan: ScanPlan,
}

/// Where the disk scan starts, gathered in `update` from memory.
#[derive(Debug, Clone, Default)]
pub struct ScanPlan {
    /// Sheets to read, and to follow down their `child_sheets`: every
    /// declared page and root of the open projects, plus the children of the
    /// open sheets. Sheets that are open are not listed (memory covers them).
    pub seeds: Vec<(PathBuf, CaseFold)>,
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

use crate::app::project_sheets::fold_key as key;

/// Read every sheet the plan reaches, children included, and collect the
/// ones that place `target`. A sheet reached twice is listed once. Each sheet
/// is keyed under the key rules of the project that reached it.
///
/// The hierarchy walk is the project assembler's (`assemble_from_seeds`); this
/// only turns what it could not load into the unchecked list.
pub fn scan_closed_sheets(plan: &ScanPlan, target: SymbolMatch) -> ClosedSheetScan {
    let mut scan = ClosedSheetScan {
        placed: Vec::new(),
        unreadable: plan.unresolved.clone(),
        visited: Vec::new(),
        unfinished: false,
        stamps: Vec::new(),
    };
    let mut folds: Vec<CaseFold> = Vec::new();
    for (_, fold) in &plan.seeds {
        if !folds.contains(fold) {
            folds.push(*fold);
        }
    }
    for fold in folds {
        let seeds: Vec<PathBuf> = plan
            .seeds
            .iter()
            .filter(|(_, f)| *f == fold)
            .map(|(p, _)| p.clone())
            .collect();
        // The assembler asks for each path just before it reads it, which is
        // the moment the stamp must describe.
        let stamped = RefCell::new(HashMap::new());
        let set = assemble_from_seeds(
            &|path| {
                stamped
                    .borrow_mut()
                    .insert(path.to_path_buf(), FileStamp::of(path));
                None
            },
            &seeds,
            &plan.open,
            fold,
        );
        let mut stamped = stamped.into_inner();
        for (path, sheet) in &set.sheets {
            scan.placed.extend(placed_on_sheets(
                std::iter::once((path.as_path(), sheet)),
                |id| target.matches(id),
            ));
            scan.visited.push(key(path, fold));
            if let Some(stamp) = stamped.remove(path) {
                scan.stamps.push((path.clone(), stamp));
            }
        }
        for path in &set.absent {
            scan.visited.push(key(path, fold));
            // Declared but absent: nothing was read, so nothing is known.
            scan.unreadable
                .push(format!("{} (file not found)", file_name(path)));
        }
        for (path, why) in &set.unreadable {
            scan.visited.push(key(path, fold));
            scan.unreadable.push(format!("{} {why}", file_name(path)));
        }
        for refused in &set.refused {
            scan.unreadable.push(format!(
                "{}: child sheet {} is outside its parent sheet's folder",
                file_name(&refused.parent),
                refused.reference
            ));
        }
    }
    // Projects with different key rules can reach the same sheet, once per
    // rule; each sheet is listed once.
    scan.placed.sort();
    scan.placed.dedup();
    scan.unreadable.sort();
    scan.unreadable.dedup();
    scan.visited.sort();
    scan.visited.dedup();
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
