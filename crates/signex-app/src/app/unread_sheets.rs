//! Which sheets of an assembled project were not read, so their contents are
//! unknown: the one answer for the contents-table refresh (#2742) and Smart
//! Paste (#2744). A file that exists but could not be read, a missing file a
//! sheet symbol still places, and a reference the walk refused all count. A
//! missing declared page no symbol places is not part of the design.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::app::project_sheets::{
    ChildReferenceRefusal, ProjectSheetSet, lexically_normalize, resolve_child_reference_quiet,
};
use crate::app::state::scope::path_key_with;

/// The sheets of `set` that were not read, sorted, each once.
pub(crate) fn unread_sheets(set: &ProjectSheetSet) -> Vec<PathBuf> {
    let absent: HashSet<String> = set
        .absent
        .iter()
        .map(|path| path_key_with(path, set.fold))
        .collect();
    let mut out: Vec<PathBuf> = set.unreadable.iter().map(|(p, _)| p.clone()).collect();
    out.extend(
        set.refused
            .iter()
            .map(|child| joined(parent_of(&child.parent), &child.reference)),
    );
    for (path, sheet) in &set.sheets {
        let dir = parent_of(path);
        for child in &sheet.child_sheets {
            match resolve_child_reference_quiet(dir, &child.filename) {
                Ok(file) if absent.contains(&path_key_with(&file, set.fold)) => out.push(file),
                Ok(_) | Err(ChildReferenceRefusal::Empty) => {}
                Err(ChildReferenceRefusal::Escapes { resolved, .. }) => out.push(resolved),
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn parent_of(path: &Path) -> &Path {
    path.parent().unwrap_or_else(|| Path::new(""))
}

fn joined(dir: &Path, reference: &str) -> PathBuf {
    lexically_normalize(&dir.join(reference.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::handlers::menu::export::tests::sheet_with_net;
    use crate::app::project_sheets::RefusedChild;

    fn set_with_child(reference: &str) -> ProjectSheetSet {
        let mut set = ProjectSheetSet::empty();
        set.sheets.insert(
            PathBuf::from("/p/top.snxsch"),
            sheet_with_net("R1", "N", &[reference]),
        );
        set
    }

    #[test]
    fn a_missing_file_a_symbol_places_is_unread() {
        let mut set = set_with_child("gone.snxsch");
        set.absent.push(PathBuf::from("/p/gone.snxsch"));
        assert_eq!(unread_sheets(&set), [PathBuf::from("/p/gone.snxsch")]);
    }

    #[test]
    fn a_missing_page_no_symbol_places_is_not_unread() {
        let mut set = set_with_child("here.snxsch");
        set.absent.push(PathBuf::from("/p/page.snxsch"));
        assert!(unread_sheets(&set).is_empty());
    }

    #[test]
    fn a_refused_reference_is_unread_once() {
        let mut set = set_with_child("../power.snxsch");
        set.refused.push(RefusedChild {
            parent: PathBuf::from("/p/top.snxsch"),
            reference: "../power.snxsch".to_string(),
        });
        assert_eq!(unread_sheets(&set), [PathBuf::from("/power.snxsch")]);
    }

    #[test]
    fn a_failed_path_check_is_unread_without_a_recorded_refusal() {
        let set = set_with_child("../power.snxsch");
        assert_eq!(unread_sheets(&set), [PathBuf::from("/power.snxsch")]);
    }

    #[test]
    fn a_sheet_symbol_with_no_file_names_nothing() {
        let set = set_with_child("   ");
        assert!(unread_sheets(&set).is_empty());
    }
}
