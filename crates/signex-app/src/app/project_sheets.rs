//! Shared assembly of a project's child sheet-map — the one keyed collection
//! both the ERC run and the netlist build ([`signex_net::build_project_netlist`])
//! read, so they see the same project view (ADR-0002 D8).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::schematic::SchematicSheet;

/// Build the project's child sheet-map keyed by the exact `ChildSheet.filename`
/// string each parent uses to reference a child — the key both
/// [`signex_net::build_project_netlist`] and ERC's `BadHierSheetPin` look
/// children up by.
///
/// Each parent's `ChildSheet.filename` is resolved against that parent's own
/// directory to locate the child in `sheets` (the app's already-loaded
/// `path → sheet` set: live engine snapshots for open tabs, disk parses for the
/// rest). Keying on the reference string — not the bare basename the app used to
/// strip to — is what stops two sheets that share a basename in different
/// directories from silently overwriting each other.
pub(crate) fn project_children_map(
    sheets: &HashMap<PathBuf, SchematicSheet>,
) -> HashMap<String, SchematicSheet> {
    let mut children: HashMap<String, SchematicSheet> = HashMap::new();
    for (parent_path, sheet) in sheets {
        let dir = parent_path.parent().unwrap_or_else(|| Path::new(""));
        for cs in &sheet.child_sheets {
            if children.contains_key(&cs.filename) {
                continue;
            }
            let child_path = dir.join(&cs.filename);
            if let Some(child) = sheets.get(&child_path) {
                children.insert(cs.filename.clone(), child.clone());
            }
        }
    }
    children
}

/// The filename string the root sheet is referenced by — its path relative to
/// `project_dir`, falling back to the bare basename. Passed to
/// [`signex_net::build_project_netlist`] as `root_filename` so a child that
/// re-references the root is reported as a cycle rather than recursed into.
pub(crate) fn root_reference_name(root_path: &Path, project_dir: Option<&Path>) -> Option<String> {
    project_dir
        .and_then(|dir| root_path.strip_prefix(dir).ok())
        .map(Path::to_path_buf)
        .or_else(|| root_path.file_name().map(PathBuf::from))
        .and_then(|p| p.to_str().map(str::to_string))
}

/// A one-line, user-facing message for a cross-sheet stitch issue (ADR-0002 D7,
/// part 3) — shown in the Messages panel alongside other diagnostics.
///
/// Lives here rather than in one consumer because *every* caller of
/// [`signex_net::build_project_netlist`] must surface its issues: the netlist
/// is always produced, so a dropped `MissingChild` means an exported netlist
/// that is quietly missing a whole subtree.
pub(crate) fn stitch_issue_message(issue: &signex_net::StitchIssue) -> String {
    use signex_net::StitchIssue as I;
    match issue {
        I::MissingChild {
            parent_path,
            sheet_name,
            filename,
        } => format!(
            "Netlist: sheet '{sheet_name}' on '{parent_path}' references a child '{filename}' that could not be found"
        ),
        I::SheetCycle {
            parent_path,
            filename,
        } => format!("Netlist: sheet cycle — '{parent_path}' re-enters '{filename}'"),
        I::DuplicateSheetUuid {
            filename_a,
            filename_b,
        } => format!(
            "Netlist: sheets '{filename_a}' and '{filename_b}' share a UUID (copied as a template?)"
        ),
        I::SharedReferenceAcrossInstances {
            filename,
            reference,
        } => format!(
            "Netlist: reference '{reference}' in '{filename}' is shared across sheet instances"
        ),
        I::NameCollision { name } => {
            format!(
                "Netlist: two distinct nets are both named '{name}'; the later one was suffixed"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{ChildSheet, FillType, Point};
    use uuid::Uuid;

    fn child_ref(filename: &str) -> ChildSheet {
        ChildSheet {
            uuid: Uuid::nil(),
            name: String::new(),
            filename: filename.to_string(),
            position: Point::ZERO,
            size: (0.0, 0.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        }
    }

    fn sheet(uuid: u128, children: &[&str]) -> SchematicSheet {
        SchematicSheet {
            uuid: Uuid::from_u128(uuid),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: String::new(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: children.iter().map(|f| child_ref(f)).collect(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: HashMap::new(),
            lib_symbols: HashMap::new(),
        }
    }

    #[test]
    fn same_basename_children_in_different_dirs_do_not_collide() {
        // A root references two children that share a basename but live in
        // different directories, via distinct relative filenames. The old
        // basename key ("power.snxsch" for both) overwrote one; keying by the
        // reference string keeps both distinct.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/root.snxsch"),
            sheet(1, &["a/power.snxsch", "b/power.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/a/power.snxsch"), sheet(0xA, &[]));
        sheets.insert(PathBuf::from("/proj/b/power.snxsch"), sheet(0xB, &[]));

        let children = project_children_map(&sheets);

        assert_eq!(children.len(), 2, "both same-basename children present");
        assert_eq!(children["a/power.snxsch"].uuid, Uuid::from_u128(0xA));
        assert_eq!(children["b/power.snxsch"].uuid, Uuid::from_u128(0xB));
    }

    #[test]
    fn resolves_a_bare_child_reference_against_the_parent_dir() {
        // The common flat case: a root in /proj references "child.snxsch",
        // which lives beside it. Keyed by the bare reference string.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/root.snxsch"),
            sheet(1, &["child.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/child.snxsch"), sheet(2, &[]));

        let children = project_children_map(&sheets);

        assert_eq!(children.len(), 1);
        assert_eq!(children["child.snxsch"].uuid, Uuid::from_u128(2));
    }

    #[test]
    fn unreferenced_and_unloadable_sheets_are_absent() {
        // A reference whose file isn't in the loaded set is skipped (no panic,
        // no phantom entry); a sheet nobody references is not added either.
        let mut sheets = HashMap::new();
        sheets.insert(
            PathBuf::from("/proj/root.snxsch"),
            sheet(1, &["missing.snxsch"]),
        );
        sheets.insert(PathBuf::from("/proj/orphan.snxsch"), sheet(3, &[]));

        let children = project_children_map(&sheets);
        assert!(children.is_empty());
    }

    #[test]
    fn root_reference_name_is_relative_to_project_dir() {
        assert_eq!(
            root_reference_name(Path::new("/proj/sub/root.snxsch"), Some(Path::new("/proj"))),
            Some("sub/root.snxsch".to_string())
        );
        assert_eq!(
            root_reference_name(
                Path::new("/elsewhere/root.snxsch"),
                Some(Path::new("/proj"))
            ),
            Some("root.snxsch".to_string())
        );
    }
}
