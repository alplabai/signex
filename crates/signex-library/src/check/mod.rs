//! Headless library check (#3111): everything Signex needs from a library
//! directory, verified without a window, as a list of [`Problem`]s.
//!
//! [`check_library`] loads the library the way the app does, validates every
//! row as a save would, resolves every symbol, footprint and sim reference
//! (into this library, the `--with` libraries and the libraries the manifest
//! `requires`), checks unique part numbers, row ids and library ids, readable
//! format tokens, STEP attachments, and that every file is in canonical byte
//! form. The codes are stable; `docs/CLI.md` lists them.

mod primitives;
mod rows;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use uuid::Uuid;

use crate::adapters::local_git::LocalGitAdapter;
use crate::library_file::{LibraryFile, SnxlibManifest};

pub use primitives::LibraryIndex;

/// How bad a [`Problem`] is. Only errors fail a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// One finding of [`check_library`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub severity: Severity,
    /// Stable machine key, `<area>.<what>`.
    pub code: &'static str,
    /// The file, relative to the library root with `/`; empty for the
    /// library as a whole.
    pub file: String,
    /// The row id, primitive uuid or table the problem is about.
    pub item: Option<String>,
    pub message: String,
}

impl Problem {
    pub(crate) fn error(
        code: &'static str,
        file: &str,
        item: Option<String>,
        message: String,
    ) -> Self {
        Self {
            severity: Severity::Error,
            code,
            file: file.to_string(),
            item,
            message,
        }
    }

    pub(crate) fn warning(
        code: &'static str,
        file: &str,
        item: Option<String>,
        message: String,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::error(code, file, item, message)
        }
    }
}

/// True when `problems` holds an error.
pub fn has_errors(problems: &[Problem]) -> bool {
    problems.iter().any(|p| p.severity == Severity::Error)
}

/// Check the library at `dir` (its folder or its `.snxlib`), resolving
/// references into the libraries at `with` as well. Problems are sorted by
/// file, item and code.
pub fn check_library(dir: &Path, with: &[PathBuf]) -> Vec<Problem> {
    let mut out = Vec::new();
    let Some(snxlib) = locate_snxlib(dir, &mut out) else {
        return out;
    };
    let snxlib_rel = snxlib
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let adapter = match LocalGitAdapter::open_to_read(&snxlib) {
        Ok(adapter) => adapter,
        Err(e) => {
            out.push(Problem::error(
                "library.unreadable",
                &snxlib_rel,
                None,
                e.to_string(),
            ));
            return out;
        }
    };
    let Some(manifest) = check_snxlib_bytes(&snxlib, &snxlib_rel, &mut out) else {
        return out;
    };
    let own = LibraryIndex::scan(adapter.root(), manifest.library_id, &mut out);
    let others = other_libraries(&manifest, &snxlib_rel, with, &mut out);
    rows::check_rows(&adapter, &snxlib_rel, &own, &others, &mut out);
    out.sort_by(|a, b| (&a.file, &a.item, a.code).cmp(&(&b.file, &b.item, b.code)));
    out
}

fn adapter_library_id(adapter: &LocalGitAdapter) -> Uuid {
    use crate::adapter::LibraryAdapter;
    adapter.library_id()
}

/// The `.snxlib` at `dir`: `dir` itself, or the only one directly in it.
fn locate_snxlib(dir: &Path, out: &mut Vec<Problem>) -> Option<PathBuf> {
    if dir.is_file() {
        return Some(dir.to_path_buf());
    }
    let found: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "snxlib"))
            .collect(),
        Err(e) => {
            out.push(Problem::error(
                "library.not_found",
                "",
                None,
                format!("{}: {e}", dir.display()),
            ));
            return None;
        }
    };
    match <[PathBuf; 1]>::try_from(found) {
        Ok([one]) => Some(one),
        Err(found) => {
            let (code, what) = if found.is_empty() {
                ("library.not_found", "no .snxlib file")
            } else {
                ("library.ambiguous", "more than one .snxlib file")
            };
            out.push(Problem::error(
                code,
                "",
                None,
                format!("{} holds {what}", dir.display()),
            ));
            None
        }
    }
}

/// The `.snxlib` must re-save as its own bytes. Returns its manifest.
fn check_snxlib_bytes(path: &Path, rel: &str, out: &mut Vec<Problem>) -> Option<SnxlibManifest> {
    let read = std::fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|text| {
            LibraryFile::parse_with(&text, &crate::tables::repair_short_component_row)
                .map(|file| (text, file.manifest))
                .map_err(|e| e.to_string())
        });
    let (text, manifest) = match read {
        Ok(read) => read,
        Err(e) => {
            out.push(Problem::error("library.unreadable", rel, None, e));
            return None;
        }
    };
    let resaved = LibraryFile::parse(&text)
        .map_err(|e| e.to_string())
        .and_then(|f| f.write().map_err(|e| e.to_string()));
    match resaved {
        Ok(resaved) if resaved == text => {}
        Ok(resaved) => out.push(Problem::error(
            "snxlib.not_canonical",
            rel,
            None,
            format!(
                "a save would rewrite the file ({})",
                first_difference(&text, &resaved)
            ),
        )),
        Err(e) => out.push(Problem::error(
            "snxlib.not_canonical",
            rel,
            None,
            format!("the file does not re-save: {e}"),
        )),
    }
    Some(manifest)
}

/// The `--with` libraries and the manifest's `requires`, indexed by
/// library id. Reports unreadable and duplicate libraries and required
/// libraries not given.
fn other_libraries(
    manifest: &SnxlibManifest,
    snxlib_rel: &str,
    with: &[PathBuf],
    out: &mut Vec<Problem>,
) -> BTreeMap<Uuid, Other> {
    let own_id = manifest.library_id;
    let section = &manifest.library;
    for bad in section.malformed_requires() {
        out.push(Problem::error(
            "manifest.requires_malformed",
            snxlib_rel,
            None,
            format!("requires entry {bad:?} is not a library id (UUID)"),
        ));
    }
    let required: HashSet<Uuid> = section.required_libraries().into_iter().collect();
    let mut others = BTreeMap::new();
    for path in with {
        let mut ignored = Vec::new();
        let Some(snxlib) = locate_snxlib(path, &mut ignored) else {
            out.push(Problem::error(
                "with.unreadable",
                "",
                None,
                format!(
                    "--with {}: {}",
                    path.display(),
                    ignored.first().map(|p| p.message.as_str()).unwrap_or("")
                ),
            ));
            continue;
        };
        let opened = LocalGitAdapter::open_to_read(&snxlib);
        let other = match opened {
            Ok(other) => other,
            Err(e) => {
                out.push(Problem::error(
                    "with.unreadable",
                    "",
                    None,
                    format!("--with {}: {e}", snxlib.display()),
                ));
                continue;
            }
        };
        let id = adapter_library_id(&other);
        if id == own_id || others.contains_key(&id) {
            out.push(Problem::error(
                "library.duplicate_id",
                "",
                Some(id.to_string()),
                format!(
                    "library id {id} is used by {} and another library of this check",
                    snxlib.display()
                ),
            ));
            continue;
        }
        let index = LibraryIndex::scan(other.root(), id, &mut Vec::new());
        others.insert(
            id,
            Other {
                index: Some(index),
                required: required.contains(&id),
            },
        );
    }
    for id in &required {
        if !others.contains_key(id) {
            others.insert(
                *id,
                Other {
                    index: None,
                    required: true,
                },
            );
            out.push(Problem::error(
                "requires.missing",
                snxlib_rel,
                Some(id.to_string()),
                format!("the manifest requires library {id}; pass it with --with to check references into it"),
            ));
        }
    }
    others
}

/// A library other than the checked one.
pub(crate) struct Other {
    /// `None` for a required library not given with `--with`: references
    /// into it are not checked (`requires.missing` says so once).
    pub index: Option<LibraryIndex>,
    /// Named by the checked library's `requires`.
    pub required: bool,
}

/// `line N: ...` of the first line where `a` and `b` differ.
pub(crate) fn first_difference(a: &str, b: &str) -> String {
    let (mut x, mut y) = (a.lines(), b.lines());
    for line in 1.. {
        match (x.next(), y.next()) {
            (None, None) => return "line endings differ".into(),
            (l, r) if l != r => {
                return format!(
                    "line {line}: file has {:?}, a save writes {:?}",
                    l.unwrap_or(""),
                    r.unwrap_or("")
                );
            }
            _ => {}
        }
    }
    unreachable!("the loop returns")
}
