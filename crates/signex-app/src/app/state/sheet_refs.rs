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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::DocumentState;
use super::scope::{self, CaseFold, path_key_with};
use crate::app::project_sheets::{child_reference_in_root, lexically_normalize};

/// A sheet that exists but could not be read or parsed, so its children are
/// unknown.
#[derive(Debug, Clone)]
pub struct UnreadableSheet {
    pub(crate) path: PathBuf,
    pub(crate) reason: String,
}

impl UnreadableSheet {
    /// One-line description for the Messages panel. The panel shortens long
    /// entries, so the file name and the decision come before the directory.
    pub(crate) fn warning(&self) -> String {
        let name = self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let dir = self.path.parent().unwrap_or_else(|| Path::new(""));
        format!(
            "Close Project: keeping the folder of {name} open, the sheet {} ({})",
            self.reason,
            dir.display(),
        )
    }
}

/// Child references per sheet path, plus the sheets that could not be read.
#[derive(Debug, Clone, Default)]
pub struct SheetRefs {
    pub(crate) refs: HashMap<PathBuf, Vec<String>>,
    pub(crate) unreadable: Vec<UnreadableSheet>,
}

/// Everything the disk walk needs, owned so it can move to a worker thread.
#[derive(Debug, Clone)]
pub(crate) struct SheetRefInputs {
    /// Child references of every loaded engine. They win over disk, so
    /// unsaved edits are honoured.
    loaded: HashMap<PathBuf, Vec<String>>,
    /// Every sheet each open project lists.
    roots: Vec<PathBuf>,
}

impl SheetRefInputs {
    /// Walk the open projects' sheets, reading only the ones no engine holds.
    ///
    /// Child references are resolved with the rule `Open Child Sheet` uses
    /// (joined, lexically normalized, rejected when they escape the parent's
    /// directory) and visited once per [`path_key_with`] key, so a `../` loop
    /// folds onto a seen sheet instead of growing the path.
    ///
    /// Blocking file IO: call from a worker thread.
    pub(crate) fn resolve(self, fold: CaseFold) -> SheetRefs {
        let Self { loaded, roots } = self;
        let loaded_by_key: HashMap<String, &Vec<String>> = loaded
            .iter()
            .map(|(path, refs)| (path_key_with(path, fold), refs))
            .collect();
        let mut from_disk: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut unreadable = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut pending: Vec<PathBuf> = roots.iter().map(|r| lexically_normalize(r)).collect();
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
                    Err(reason) => {
                        unreadable.push(UnreadableSheet {
                            path: path.clone(),
                            reason,
                        });
                        Vec::new()
                    }
                },
            };
            let dir = path.parent().unwrap_or_else(|| Path::new(""));
            pending.extend(
                children
                    .iter()
                    .filter_map(|c| child_reference_in_root(dir, c)),
            );
        }
        let mut refs = loaded.clone();
        refs.extend(from_disk);
        SheetRefs { refs, unreadable }
    }
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
        }
    }

    /// Whether `path` belongs to an open project other than
    /// `projects[closing]` (see [`scope::claimed_by_other_project`]).
    pub(crate) fn claimed_by_other_project(
        &self,
        sheet_refs: &SheetRefs,
        closing: usize,
        path: &Path,
    ) -> bool {
        let unreadable: Vec<PathBuf> = sheet_refs
            .unreadable
            .iter()
            .map(|u| u.path.clone())
            .collect();
        scope::claimed_by_other_project(
            &self.projects,
            &sheet_refs.refs,
            &unreadable,
            closing,
            path,
            CaseFold::host_default(),
        )
    }

    /// Whether `projects[closing]` owns `path` (see [`scope::owned_by_project`]).
    pub(crate) fn owned_by_project(
        &self,
        sheet_refs: &SheetRefs,
        closing: usize,
        path: &Path,
    ) -> bool {
        scope::owned_by_project(
            &self.projects,
            &sheet_refs.refs,
            closing,
            path,
            CaseFold::host_default(),
        )
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
    fn a_dotdot_cycle_reads_each_sheet_once_and_reports_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let root = dir.join("x.snxsch");
        write_sheet(&root, &["../d/x.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.clone()],
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
    fn a_reference_that_escapes_its_parent_directory_is_not_followed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let outside = tmp.path().join("outside.snxsch");
        write_sheet(&outside, &[]);
        let root = dir.join("x.snxsch");
        write_sheet(&root, &["../outside.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root],
        };

        let resolved = inputs.resolve(CaseFold::Preserve);

        assert!(!resolved.refs.contains_key(&outside));
    }
}
