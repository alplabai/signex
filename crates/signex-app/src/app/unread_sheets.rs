//! Which sheets of an assembled project were not read, so their contents are
//! unknown: the one answer for the contents-table refresh (#2742) and Smart
//! Paste (#2744). A file that exists but could not be read, a missing file a
//! sheet symbol still places, and a reference the walk refused all count. A
//! missing declared page no symbol places is not part of the design.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use signex_types::schematic::{SchDrawing, TableSource};

use crate::app::project_sheets::{
    ChildReferenceRefusal, OpenSheets, ProjectSheetSet, lexically_normalize,
    resolve_child_reference_quiet,
};
use crate::app::state::scope::{CaseFold, path_key_with};

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

/// Whether a stored contents link is a file name with no folder.
pub(crate) fn is_bare_link(link: &str) -> bool {
    !link.contains(['/', '\\'])
}

/// The bare contents-table links of the `open` sheets with no file in their
/// table's folder, resolved from it, sorted, each once. Reads the disk, so
/// only the compile job calls it. A check that fails for any reason but "not
/// found" leaves the file out: it may be there, so its row stays (#2742).
pub(crate) fn bare_links_missing_on_disk(open: &OpenSheets, fold: CaseFold) -> Vec<PathBuf> {
    let open_keys: HashSet<String> = open.keys().map(|p| path_key_with(p, fold)).collect();
    let mut out = Vec::new();
    for (path, sheet) in open {
        let dir = parent_of(path);
        let tables = sheet.drawings.iter().filter_map(|drawing| match drawing {
            SchDrawing::Table(t) if matches!(t.source, TableSource::ProjectSheets { .. }) => {
                Some(t)
            }
            _ => None,
        });
        for link in tables.flat_map(|t| &t.links) {
            if link.is_empty() || !is_bare_link(link) {
                continue;
            }
            let file = lexically_normalize(&dir.join(link));
            if !open_keys.contains(&path_key_with(&file, fold)) && is_confirmed_missing(&file) {
                out.push(file);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn is_confirmed_missing(file: &Path) -> bool {
    matches!(
        std::fs::symlink_metadata(file),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    )
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

    /// An open sheet in `dir` with a contents table linking to `links`.
    fn open_with_links(dir: &Path, links: &[&str]) -> OpenSheets {
        use signex_types::schematic::{Point, SchTable, SchematicSheet};
        let mut table = SchTable::new_typed(uuid::Uuid::from_u128(1), Point::new(0.0, 0.0));
        table.source = TableSource::ProjectSheets { per_channel: true };
        table.links = links.iter().map(|link| (*link).to_string()).collect();
        let mut sheet = SchematicSheet::default();
        sheet.drawings.push(SchDrawing::Table(table));
        OpenSheets::from([(dir.join("contents.snxsch"), sheet)])
    }

    #[test]
    fn only_a_bare_link_with_no_file_beside_the_table_is_missing() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("here.snxsch"), "x").expect("seed");
        let open = open_with_links(
            dir.path(),
            &["here.snxsch", "gone.snxsch", "power/gone.snxsch", ""],
        );

        let missing = bare_links_missing_on_disk(&open, CaseFold::default());

        assert_eq!(missing, [dir.path().join("gone.snxsch")]);
    }

    #[test]
    fn an_open_sheet_not_yet_on_disk_is_not_missing() {
        let dir = tempfile::tempdir().expect("dir");
        let mut open = open_with_links(dir.path(), &["new.snxsch"]);
        open.insert(dir.path().join("new.snxsch"), Default::default());

        assert!(bare_links_missing_on_disk(&open, CaseFold::default()).is_empty());
    }

    /// A NUL in the name makes the check itself fail, not "not found".
    #[test]
    fn a_failed_disk_check_counts_as_present() {
        let dir = tempfile::tempdir().expect("dir");
        let open = open_with_links(dir.path(), &["bad\0name.snxsch"]);

        assert!(bare_links_missing_on_disk(&open, CaseFold::default()).is_empty());
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
