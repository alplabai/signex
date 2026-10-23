//! Child-sheet references for ownership checks, resolved from the open
//! projects' own sheet lists rather than only from the loaded engines.
//!
//! A sibling's dirty child sheet whose parent tab was closed has no parent
//! engine to point at it; reading the unloaded parents from disk keeps that
//! child attributable to its project (#741).
//!
//! The disk walk blocks, so it never runs inside `update`: the caller takes a
//! [`SheetRefInputs`] snapshot, resolves it on a worker thread and gets the
//! [`SheetRefs`] back as a message.
//!
//! Closing project A splits A's dirty documents by one strict rule: `mine` is
//! what A's own ownership walk reaches (plus A's `.snxprj`) AND no other open
//! project's walk reaches; every other dirty document associated with A is
//! `kept`. If any open project has a sheet that cannot be read, ownership is
//! unknown and Discard All is off.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::scope::{self, CaseFold, ReachSets, path_key_with};
use super::{DocumentState, ProjectId};
use crate::app::project_sheets::{normalize_clamped, ownership_reference};

/// How many file names a single Messages-panel or dialog line spells out
/// before it falls back to a count.
const MAX_NAMES_LISTED: usize = 5;

/// A sheet that exists but could not be read or parsed, so its children are
/// unknown.
#[derive(Debug, Clone)]
pub struct UnreadableSheet {
    pub(crate) path: PathBuf,
}

/// `a, b, c (+N more)`: at most [`MAX_NAMES_LISTED`] names, then a count, so a
/// line stays short however many files there are.
pub(crate) fn bounded_names(names: &[String]) -> String {
    let shown = names
        .iter()
        .take(MAX_NAMES_LISTED)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    match names.len().saturating_sub(MAX_NAMES_LISTED) {
        0 => shown,
        more => format!("{shown} (+{more} more)"),
    }
}

/// Child references per sheet path, plus the sheets that could not be read.
#[derive(Debug, Clone, Default)]
pub struct SheetRefs {
    pub(crate) refs: HashMap<PathBuf, Vec<String>>,
    pub(crate) unreadable: Vec<UnreadableSheet>,
    /// The open projects this was resolved for. A project opened afterwards
    /// has disk-only children nobody read, so a consumer re-resolves when
    /// [`SheetRefs::is_current_for`] says no.
    pub(crate) resolved_for: Vec<ProjectId>,
}

impl SheetRefs {
    /// True when exactly the projects in `open` were open at resolve time.
    pub(crate) fn is_current_for(&self, open: &[ProjectId]) -> bool {
        let resolved: HashSet<ProjectId> = self.resolved_for.iter().copied().collect();
        let now: HashSet<ProjectId> = open.iter().copied().collect();
        resolved == now
    }

    /// Why Discard All is off, or `None` when every sheet could be read. A
    /// sheet that cannot be read hides its children, so which documents are
    /// the closing project's alone is unknown and nothing may be discarded.
    pub fn discard_blocked_reason(&self) -> Option<String> {
        if self.unreadable.is_empty() {
            return None;
        }
        let names: Vec<String> = self
            .unreadable
            .iter()
            .map(|u| file_label(&u.path))
            .collect();
        Some(format!(
            "Discard All is disabled: {} sheet(s) could not be read ({}), so ownership is unknown",
            names.len(),
            bounded_names(&names),
        ))
    }
}

/// Everything the disk walk needs, owned so it can move to a worker thread.
#[derive(Debug, Clone)]
pub(crate) struct SheetRefInputs {
    /// Child references of every loaded engine. They win over disk, so
    /// unsaved edits are honoured.
    loaded: HashMap<PathBuf, Vec<String>>,
    /// Every sheet each open project lists.
    roots: Vec<PathBuf>,
    /// The projects the roots came from.
    projects: Vec<ProjectId>,
}

impl SheetRefInputs {
    /// Walk the open projects' sheets, reading only the ones no engine holds.
    ///
    /// Every reference is followed (see [`ownership_reference`]): ownership
    /// never refuses an escape, only opening a file does. Each sheet is
    /// visited once per [`path_key_with`] key and `..` is clamped at the
    /// filesystem root, so a `../` loop or chain terminates.
    ///
    /// Blocking file IO: call from a worker thread.
    pub(crate) fn resolve(self, fold: CaseFold) -> SheetRefs {
        let Self {
            loaded,
            roots,
            projects,
        } = self;
        let loaded_by_key = scope::refs_by_key(&loaded, fold);
        let mut from_disk: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut unreadable = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut pending: Vec<PathBuf> = roots.iter().map(|r| normalize_clamped(r)).collect();
        while let Some(path) = pending.pop() {
            let key = path_key_with(&path, fold);
            if !seen.insert(key.clone()) {
                continue;
            }
            let children = match loaded_by_key.get(&key) {
                Some(children) => (*children).clone(),
                None => match read_child_refs(&path) {
                    Ok(children) => {
                        from_disk.insert(path.clone(), children.clone());
                        children
                    }
                    Err(_) => {
                        unreadable.push(UnreadableSheet { path: path.clone() });
                        Vec::new()
                    }
                },
            };
            let dir = path.parent().unwrap_or_else(|| Path::new(""));
            pending.extend(children.iter().filter_map(|c| ownership_reference(dir, c)));
        }
        let mut refs = loaded.clone();
        refs.extend(from_disk);
        SheetRefs {
            refs,
            unreadable,
            resolved_for: projects,
        }
    }
}

/// A closing project's dirty documents, split so none falls through: each
/// one is either in `mine` (offered to Save All / Discard All) or in `kept`
/// (never discarded: closing leaves it open and says so).
#[derive(Debug, Clone, Default)]
pub(crate) struct CloseScan {
    pub(crate) mine: Vec<PathBuf>,
    pub(crate) kept: Vec<PathBuf>,
}

impl CloseScan {
    /// Messages-panel line naming the closing project's kept files, bounded.
    pub(crate) fn warning(&self, project_name: &str) -> Option<String> {
        if self.kept.is_empty() {
            return None;
        }
        let names: Vec<String> = self.kept.iter().map(|p| file_label(p)).collect();
        Some(format!(
            "Close Project '{project_name}': kept {} unsaved document(s) open, not provably this project's alone: {}",
            names.len(),
            bounded_names(&names),
        ))
    }
}

/// File name of `path` for display, the whole path when it has none.
pub(crate) fn file_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl DocumentState {
    /// Snapshot what [`SheetRefInputs::resolve`] needs; cheap, no IO.
    pub(crate) fn sheet_ref_inputs(&self) -> SheetRefInputs {
        SheetRefInputs {
            loaded: self.child_sheet_refs(),
            roots: self
                .projects
                .iter()
                .flat_map(|p| {
                    let dir = p.dir();
                    p.data.sheets.iter().map(move |s| dir.join(&s.filename))
                })
                .collect(),
            projects: self.projects.iter().map(|p| p.id).collect(),
        }
    }

    /// What every open project's ownership walk reaches, for `sheet_refs`.
    pub(crate) fn reach_sets(&self, sheet_refs: &SheetRefs) -> ReachSets {
        ReachSets::new(&self.projects, &sheet_refs.refs, CaseFold::host_default())
    }

    /// Split the dirty documents of `projects[closing]` into [`CloseScan`]'s
    /// two lists.
    ///
    /// A dirty path is associated with the project when it sits in the
    /// project directory, backs one of its tabs, is reached by its walk, or is
    /// its `.snxprj`. It is `mine` only when the project's own walk reaches it
    /// (or it is the project's `.snxprj`) AND no other open project's walk
    /// does; everything else associated is `kept`. Location, tabs and library
    /// mounts are never evidence of ownership.
    pub(crate) fn close_scan(&self, sheet_refs: &SheetRefs, closing: usize) -> CloseScan {
        let Some(project) = self.projects.get(closing) else {
            return CloseScan::default();
        };
        let fold = CaseFold::host_default();
        let reach = self.reach_sets(sheet_refs);
        let project_file = path_key_with(&project.path, fold);
        let mut scan = CloseScan::default();
        for path in &self.dirty_paths {
            let reached = reach.reached_by(closing, path);
            let is_project_file = path_key_with(path, fold) == project_file;
            let backs_tab = self
                .tabs
                .iter()
                .any(|t| t.path == *path && t.project_id == Some(project.id));
            let associated =
                path.starts_with(project.dir()) || backs_tab || reached || is_project_file;
            if !associated {
                continue;
            }
            if (reached || is_project_file) && !reach.reached_by_other(closing, path) {
                scan.mine.push(path.clone());
            } else {
                scan.kept.push(path.clone());
            }
        }
        scan.mine.sort();
        scan.kept.sort();
        scan
    }
}

/// `child_sheets` filenames of the sheet at `path`. A missing file has none;
/// an unreadable or unparsable one returns why.
fn read_child_refs(path: &Path) -> Result<Vec<String>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("could not be read: {e}")),
    };
    signex_types::format::SnxSchematic::parse(&text)
        .map(|parsed| {
            parsed
                .sheet
                .child_sheets
                .into_iter()
                .map(|c| c.filename)
                .collect()
        })
        .map_err(|e| format!("could not be parsed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_sheet() -> signex_types::schematic::SchematicSheet {
        signex_types::schematic::SchematicSheet {
            uuid: uuid::Uuid::nil(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: "A4".to_string(),
            root_sheet_page: "1".to_string(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: HashMap::new(),
            lib_symbols: HashMap::new(),
            unknown: Default::default(),
        }
    }

    fn write_sheet(path: &Path, children: &[&str]) {
        let mut sheet = blank_sheet();
        sheet.child_sheets = children
            .iter()
            .map(|f| signex_types::schematic::ChildSheet {
                uuid: uuid::Uuid::nil(),
                name: String::new(),
                filename: (*f).to_string(),
                position: signex_types::schematic::Point::new(0.0, 0.0),
                size: (0.0, 0.0),
                stroke_width: 0.0,
                fill: signex_types::schematic::FillType::None,
                stroke_color: None,
                fill_color: None,
                fields_autoplaced: false,
                pins: Vec::new(),
                instances: Vec::new(),
            })
            .collect();
        let text = signex_types::format::SnxSchematic::new(sheet)
            .write_string()
            .expect("serialise sheet");
        std::fs::write(path, text).expect("write sheet");
    }

    #[test]
    fn name_lists_are_bounded_to_a_count_plus_the_first_few() {
        let names: Vec<String> = (0..200).map(|n| format!("s{n}.snxsch")).collect();
        let line = bounded_names(&names);
        assert!(line.starts_with("s0.snxsch, s1.snxsch"), "{line}");
        assert!(line.ends_with("(+195 more)"), "{line}");
        assert!(line.len() < 100, "{line}");
        assert_eq!(bounded_names(&names[..2]), "s0.snxsch, s1.snxsch");

        let scan = CloseScan {
            mine: Vec::new(),
            kept: names.iter().map(PathBuf::from).collect(),
        };
        let warning = scan.warning("P").expect("kept files warn");
        assert!(warning.contains("kept 200"), "{warning}");
        assert!(warning.len() < 250, "{warning}");

        let refs = SheetRefs {
            unreadable: names
                .iter()
                .map(|n| UnreadableSheet {
                    path: PathBuf::from(n),
                })
                .collect(),
            ..SheetRefs::default()
        };
        let reason = refs.discard_blocked_reason().expect("blocked");
        assert!(reason.len() < 250, "{reason}");
        assert!(SheetRefs::default().discard_blocked_reason().is_none());
    }

    #[test]
    fn a_dotdot_cycle_reads_each_sheet_once_and_reports_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let root = dir.join("x.snxsch");
        write_sheet(&root, &["../d/x.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.clone()],
            projects: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Preserve);

        assert!(resolved.unreadable.is_empty(), "{:?}", resolved.unreadable);
        assert_eq!(resolved.refs.len(), 1, "{:?}", resolved.refs.keys());
        assert!(resolved.refs.contains_key(&root));
    }

    #[test]
    fn a_case_and_separator_variant_reference_resolves_to_the_loaded_engine() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("root.snxsch");
        write_sheet(&root, &["CHILD.snxsch"]);
        // Loaded under the lowercase spelling, with a grandchild the disk
        // never saw (no such file exists).
        let loaded_child = tmp.path().join("child.snxsch");
        let inputs = SheetRefInputs {
            loaded: HashMap::from([(loaded_child.clone(), vec!["grand.snxsch".to_string()])]),
            roots: vec![root],
            projects: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Fold);

        assert_eq!(
            resolved.refs.get(&loaded_child),
            Some(&vec!["grand.snxsch".to_string()]),
        );
        assert!(
            !resolved.refs.contains_key(&tmp.path().join("CHILD.snxsch")),
            "the variant spelling must be answered by the engine, not re-read from disk",
        );
        assert!(resolved.unreadable.is_empty());
    }

    #[test]
    fn a_reference_that_escapes_its_parent_directory_is_followed_for_ownership() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let outside = tmp.path().join("outside.snxsch");
        let grandchild = tmp.path().join("grand.snxsch");
        write_sheet(&outside, &["grand.snxsch"]);
        write_sheet(&grandchild, &[]);
        let root = dir.join("x.snxsch");
        write_sheet(&root, &["../outside.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root],
            projects: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Preserve);

        assert!(
            resolved.refs.contains_key(&outside),
            "{:?}",
            resolved.refs.keys()
        );
        assert!(
            resolved.refs.contains_key(&grandchild),
            "{:?}",
            resolved.refs.keys()
        );
    }

    #[test]
    fn an_absolute_reference_is_followed_for_ownership() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let outside = tmp.path().join("outside.snxsch");
        write_sheet(&outside, &[]);
        let root = dir.join("x.snxsch");
        write_sheet(&root, &[outside.to_str().expect("utf8 path")]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root],
            projects: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Preserve);

        assert!(
            resolved.refs.contains_key(&outside),
            "{:?}",
            resolved.refs.keys()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_chain_of_parent_references_past_the_root_terminates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let root = dir.join("x.snxsch");
        // `/../../..` clamps to `/`, so this names the sheet itself and the
        // walk folds onto the key it already saw instead of chasing `..`.
        let chained = format!("/../../../..{}", root.display());
        write_sheet(&root, &[chained.as_str(), "../../../../../../../x.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.clone()],
            projects: Vec::new(),
        };

        let resolved = inputs.resolve(CaseFold::Preserve);

        assert!(resolved.unreadable.is_empty(), "{:?}", resolved.unreadable);
        assert!(resolved.refs.contains_key(&root));
        assert!(resolved.refs.len() <= 3, "{:?}", resolved.refs.keys());
    }
}
