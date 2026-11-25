//! The files one notebook compile reads: the notebook file, the chapter
//! files it includes and any attachment the caller adds, by path inside the
//! project's `notebook/` folder. The crate does no IO: [`NotebookSources::load`]
//! takes the reader.

use std::collections::{BTreeMap, BTreeSet};

use crate::attachment::DesignValues;
use crate::syntax::includes;

/// The files of one notebook, keyed by their `/`-separated path inside the
/// notebook folder (`design.snxnbk`, `chapters/power.snxnbk`,
/// `attachments/ripple.csv`), and the design's part values the stale marks
/// are printed against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotebookSources {
    main: String,
    files: BTreeMap<String, Vec<u8>>,
    design: DesignValues,
}

/// A file [`NotebookSources::load`] could not add.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LoadProblem {
    /// The file that includes it; empty for the notebook file itself.
    pub file: String,
    /// The 1-based line of the include; 0 for the notebook file itself.
    pub line: u32,
    /// The path as written.
    pub path: String,
    pub kind: LoadProblemKind,
}

/// Why a file was not added.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoadProblemKind {
    /// The reader failed; `reason` is its message.
    Unreadable { reason: String },
    /// The path leaves the notebook folder.
    OutsideNotebook,
}

impl NotebookSources {
    /// Sources holding only the notebook file `main` with `text`.
    pub fn new(main: impl Into<String>, text: impl Into<Vec<u8>>) -> Self {
        let main: String = main.into();
        let main = normalize(&main).unwrap_or_default();
        let mut files = BTreeMap::new();
        files.insert(main.clone(), text.into());
        Self {
            main,
            files,
            design: DesignValues::default(),
        }
    }

    /// Read the notebook file `main` and every chapter it includes, at any
    /// depth, each once, with `read` (given the path inside the notebook
    /// folder). What could not be read is listed; the compile then reports
    /// the missing include where it is written.
    pub fn load(
        main: &str,
        mut read: impl FnMut(&str) -> std::io::Result<Vec<u8>>,
    ) -> (Self, Vec<LoadProblem>) {
        let mut problems = Vec::new();
        let Some(main) = normalize(main) else {
            problems.push(LoadProblem {
                file: String::new(),
                line: 0,
                path: main.to_string(),
                kind: LoadProblemKind::OutsideNotebook,
            });
            return (Self::new("", Vec::new()), problems);
        };
        let mut sources = Self {
            main: main.clone(),
            files: BTreeMap::new(),
            design: DesignValues::default(),
        };
        let mut seen = BTreeSet::from([main.clone()]);
        let mut pending = vec![(String::new(), 0, main.clone(), main)];
        while let Some((from, line, written, path)) = pending.pop() {
            let bytes = match read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    problems.push(LoadProblem {
                        file: from,
                        line,
                        path: written,
                        kind: LoadProblemKind::Unreadable {
                            reason: error.to_string(),
                        },
                    });
                    continue;
                }
            };
            let text = String::from_utf8_lossy(&bytes).into_owned();
            sources.files.insert(path.clone(), bytes);
            // Pushed in reverse so the first include is read first.
            for include in includes(&text).into_iter().rev() {
                match resolve(&path, &include.path) {
                    Some(target) if seen.insert(target.clone()) => {
                        pending.push((path.clone(), include.line, include.path, target));
                    }
                    Some(_) => {}
                    None => problems.push(LoadProblem {
                        file: path.clone(),
                        line: include.line,
                        path: include.path,
                        kind: LoadProblemKind::OutsideNotebook,
                    }),
                }
            }
        }
        problems.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
        (sources, problems)
    }

    /// Add or replace the file at `path` (an attachment, a chapter). A path
    /// that leaves the notebook folder is refused with `false`.
    pub fn insert(&mut self, path: &str, bytes: impl Into<Vec<u8>>) -> bool {
        let Some(path) = normalize(path) else {
            return false;
        };
        self.files.insert(path, bytes.into());
        true
    }

    /// Set the design's part values, by designator: an attachment taken
    /// with other values prints its stale mark. None are set by default,
    /// and then nothing prints as stale.
    pub fn set_design_values(&mut self, values: DesignValues) {
        self.design = values;
    }

    /// The part values set with [`Self::set_design_values`].
    pub fn design_values(&self) -> &DesignValues {
        &self.design
    }

    /// The attachments the notebook files name that are not held yet, as
    /// paths inside the notebook folder (`attachments/scope/a.csv`), each
    /// once, sorted: what the caller reads before a compile so the figures
    /// print.
    pub fn missing_attachments(&self) -> Vec<String> {
        let mut missing: Vec<String> = crate::record::record(self)
            .attachments
            .iter()
            .map(crate::attachment::Attachment::path_in_notebook)
            .filter(|path| !self.files.contains_key(path))
            .collect();
        missing.sort();
        missing.dedup();
        missing
    }

    /// The notebook file's path.
    pub fn main(&self) -> &str {
        &self.main
    }

    /// The bytes of the file at `path`, if it is held.
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    /// Every held path, sorted.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }
}

/// `path` written in the file `from` as a path inside the notebook folder:
/// relative to `from`'s folder, or to the folder itself with a leading `/`.
/// `None` when it climbs out.
pub fn resolve(from: &str, path: &str) -> Option<String> {
    if path.starts_with('/') {
        return normalize(path);
    }
    let folder = from.rsplit_once('/').map_or("", |(folder, _)| folder);
    normalize(&format!("{folder}/{path}"))
}

/// `path` with `.` and empty segments dropped and `..` applied; `None` when
/// it climbs out of the folder or is empty.
fn normalize(path: &str) -> Option<String> {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split(['/', '\\']) {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    (!segments.is_empty()).then(|| segments.join("/"))
}
