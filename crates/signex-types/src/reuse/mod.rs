//! Design snippets and templates (#1283): where they are kept and what they
//! are called. A snippet is a `.snxsch` holding a circuit fragment, placed
//! again as a design block. A sheet template is a `.snxsch` whose frame (size,
//! title block, sheet parameters and its drawings and text) a new sheet starts
//! from. A project template is a project folder whose settings and first
//! sheet's frame a new project starts from. Each is kept in a project folder
//! or in the user's library folder. Rules: `docs/SNIPPETS_AND_TEMPLATES.md`.
//!
//! The listing functions read the disk: run them off the update thread.

mod name;
pub mod template;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde::Serialize;

pub use name::{MAX_NAME_CHARS, NameRefusal, checked_name};

/// Where a snippet or template is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReuseScope {
    /// The folder of the project it was saved from.
    Project,
    /// The user's library folder, shared by every project.
    Library,
}

impl ReuseScope {
    pub const fn token(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Library => "library",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        match token.trim() {
            "project" => Some(Self::Project),
            "library" => Some(Self::Library),
            _ => None,
        }
    }
}

/// What a store entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReuseKind {
    Snippet,
    SheetTemplate,
    ProjectTemplate,
}

/// The extension of a snippet or sheet template file.
pub const SHEET_EXTENSION: &str = "snxsch";

impl ReuseKind {
    /// The folder of this kind under a project or library folder.
    pub fn folder(self, root: &Path) -> PathBuf {
        match self {
            Self::Snippet => root.join("snippets"),
            Self::SheetTemplate => root.join("templates").join("sheets"),
            Self::ProjectTemplate => root.join("templates").join("projects"),
        }
    }

    /// The file (a project template: the folder) of entry `name` under
    /// `root`. `name` is one [`checked_name`] returned.
    pub fn entry_path(self, root: &Path, name: &str) -> PathBuf {
        match self {
            Self::ProjectTemplate => self.folder(root).join(name),
            Self::Snippet | Self::SheetTemplate => {
                self.folder(root).join(format!("{name}.{SHEET_EXTENSION}"))
            }
        }
    }

    /// A project template lives in the library only: a project folder holds
    /// one project.
    pub const fn kept_in(self, scope: ReuseScope) -> bool {
        !matches!((self, scope), (Self::ProjectTemplate, ReuseScope::Project))
    }

    /// The token of a template kind, as `list_templates` names it.
    pub const fn template_token(self) -> Option<&'static str> {
        match self {
            Self::SheetTemplate => Some("sheet"),
            Self::ProjectTemplate => Some("project"),
            Self::Snippet => None,
        }
    }
}

/// One snippet or template on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReuseEntry {
    pub name: String,
    pub kind: ReuseKind,
    pub scope: ReuseScope,
    /// The `.snxsch`, or a project template's folder.
    pub file: PathBuf,
}

/// What a read of some store folders found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReuseListing {
    /// Sorted by kind, scope, then name.
    pub entries: Vec<ReuseEntry>,
    /// Folders or entries that could not be read, each with why.
    pub unreadable: Vec<String>,
}

impl ReuseListing {
    /// The entries of `kind`.
    pub fn of_kind(&self, kind: ReuseKind) -> impl Iterator<Item = &ReuseEntry> {
        self.entries.iter().filter(move |entry| entry.kind == kind)
    }
}

/// Every entry of `kinds` under each `(scope, root)`, sorted. A missing
/// folder holds nothing; a name no [`checked_name`] accepts is skipped, so
/// a stray file cannot name an entry the commands could not address.
/// Blocking.
pub fn list(roots: &[(ReuseScope, PathBuf)], kinds: &[ReuseKind]) -> ReuseListing {
    let mut listing = ReuseListing::default();
    for (scope, root) in roots {
        for kind in kinds.iter().filter(|kind| kind.kept_in(*scope)) {
            list_folder(*scope, *kind, root, &mut listing);
        }
    }
    listing
        .entries
        .sort_by(|a, b| (a.kind, a.scope, &a.name).cmp(&(b.kind, b.scope, &b.name)));
    listing
}

fn list_folder(scope: ReuseScope, kind: ReuseKind, root: &Path, listing: &mut ReuseListing) {
    let dir = kind.folder(root);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            listing
                .unreadable
                .push(format!("{}: {error}", dir.display()));
            return;
        }
    };
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(error) => {
                listing
                    .unreadable
                    .push(format!("{}: {error}", dir.display()));
                continue;
            }
        };
        if let Some(name) = entry_name(kind, &path) {
            listing.entries.push(ReuseEntry {
                name,
                kind,
                scope,
                file: path,
            });
        }
    }
}

/// The entry name `path` stands for in a folder of `kind`, if it is one.
fn entry_name(kind: ReuseKind, path: &Path) -> Option<String> {
    let name = match kind {
        ReuseKind::ProjectTemplate => {
            if !path.is_dir() {
                return None;
            }
            path.file_name()?.to_str()?
        }
        ReuseKind::Snippet | ReuseKind::SheetTemplate => {
            let is_sheet = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(SHEET_EXTENSION));
            if !is_sheet || !path.is_file() {
                return None;
            }
            path.file_stem()?.to_str()?
        }
    };
    checked_name(name).ok().filter(|checked| checked == name)
}

/// The `.snxprj` of the project template folder `folder`: the one there, or
/// why there is none. Blocking.
pub fn project_template_file(folder: &Path) -> Result<PathBuf, String> {
    let entries = std::fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut found = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| format!("{}: {e}", folder.display()))?
            .path();
        let is_project = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("snxprj"));
        if is_project && path.is_file() {
            found.push(path);
        }
    }
    found.sort();
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(format!("{} holds no .snxprj", folder.display())),
        n => Err(format!(
            "{} holds {n} .snxprj files; a project template holds one",
            folder.display()
        )),
    }
}
