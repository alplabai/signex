//! What a `place_sheet_symbol` file brings into the sheet it is placed on
//! (#2199): the walk down its subtree (a loop back to the edited sheet, a
//! sheet that cannot be read, names that reach other sheets), the file being a
//! page or a placement already, and the names it brings in. Everything is
//! judged on what the edit read before it ran (`edit_reads`); sheets are
//! matched by their folded key. Rules: `docs/CLI.md`, "Placing parts and
//! hierarchy".

use std::collections::{HashMap, HashSet};
use std::path::Path;

use signex_engine::Command;
use signex_types::schematic::{LabelType, Point, SchematicSheet};

use crate::app::Signex;
use crate::app::project_sheets::{project_pages, resolve_child_reference_quiet};
use crate::app::sheet_hierarchy::{CaseFold, normalised_key};

/// The walk down a file's child references.
pub(super) struct SubtreeFacts {
    /// A sheet of the subtree places the edited sheet: a loop.
    pub(super) reaches: bool,
    /// `<file> is unreadable (<why>)` for each sheet of it that cannot be read.
    pub(super) unreadable: Vec<String>,
    /// A sheet of it carries a name that can reach other sheets: a global
    /// label or a power port naming a net.
    pub(super) names: bool,
}

/// A global label or a power port naming a net: a name other sheets see.
fn carries_reaching_name(sheet: &SchematicSheet) -> bool {
    sheet
        .labels
        .iter()
        .any(|l| matches!(l.label_type, LabelType::Global | LabelType::Power))
        || sheet
            .symbols
            .iter()
            .any(|s| s.is_power && !s.value.is_empty())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Walk down from `file` over `known` (sheets by folded key); `unreadable`
/// holds why a sheet could not be read, by folded key.
fn walk(
    file: &Path,
    edited: &str,
    fold: CaseFold,
    known: &HashMap<String, &SchematicSheet>,
    unreadable: &HashMap<String, String>,
) -> SubtreeFacts {
    let mut facts = SubtreeFacts {
        reaches: false,
        unreadable: Vec::new(),
        names: false,
    };
    let mut seen = HashSet::new();
    let mut queue = vec![file.to_path_buf()];
    while let Some(path) = queue.pop() {
        let key = normalised_key(&path, fold);
        if !seen.insert(key.clone()) {
            continue;
        }
        if let Some(why) = unreadable.get(&key) {
            facts
                .unreadable
                .push(format!("{} is unreadable ({why})", file_name(&path)));
            continue;
        }
        let Some(sheet) = known.get(&key) else {
            continue;
        };
        facts.names |= carries_reaching_name(sheet);
        let dir = path.parent().unwrap_or_else(|| Path::new(""));
        for child in &sheet.child_sheets {
            let Ok(next) = resolve_child_reference_quiet(dir, &child.filename) else {
                continue;
            };
            facts.reaches |= normalised_key(&next, fold) == edited;
            queue.push(next);
        }
    }
    facts
}

impl Signex {
    /// The walk down `file`'s subtree from the sheet an edit acts on: over
    /// the open sheets and the project as read, or for a loose sheet the
    /// file's subtree as read.
    pub(super) fn file_subtree(
        &self,
        file: &Path,
        sheet: &Path,
        fold: CaseFold,
    ) -> Result<SubtreeFacts, String> {
        let docs = &self.document_state;
        let owned = match docs.document_project(sheet) {
            Some(_) => Some(self.edit_project_set(sheet)?),
            None => None,
        };
        let set = owned.as_ref().or_else(|| self.prefetched_subtree());
        let mut known: HashMap<String, &SchematicSheet> = HashMap::new();
        let mut unreadable: HashMap<String, String> = HashMap::new();
        if let Some(set) = set {
            for (path, content) in &set.sheets {
                known.insert(normalised_key(path, fold), content);
            }
            for (path, why) in &set.unreadable {
                unreadable.insert(normalised_key(path, fold), why.clone());
            }
        }
        for (path, engine) in &docs.engines {
            let key = normalised_key(path, fold);
            unreadable.remove(&key);
            known.insert(key, engine.document());
        }
        let edited = normalised_key(sheet, fold);
        Ok(walk(file, &edited, fold, &known, &unreadable))
    }

    /// A `file` that is a page of the project, or that a sheet symbol on
    /// another sheet of it places already: placing it here too changes its
    /// nets (its local names become this placement's), so it needs
    /// `allow_merge=true`. A loose sheet has neither.
    pub(super) fn refuse_placed_file(&self, path: &Path) -> Result<(), String> {
        let docs = &self.document_state;
        let sheet = docs.edit_path().ok_or("no schematic is open")?;
        let Some(project) = docs.document_project(sheet) else {
            return Ok(());
        };
        let fold = crate::app::state::project_case::fold_of(Some(project));
        let key = normalised_key(path, fold);
        let tail = "placing it again changes its nets; pass allow_merge=true to place it anyway";
        if project_pages(Some(project))
            .iter()
            .any(|page| normalised_key(page, fold) == key)
        {
            return Err(format!("`file` is a page of the project; {tail}"));
        }
        let edited = normalised_key(sheet, fold);
        let set = self.edit_project_set(sheet)?;
        for (parent, content) in &set.sheets {
            if normalised_key(parent, fold) == edited {
                continue;
            }
            let dir = parent.parent().unwrap_or_else(|| Path::new(""));
            let places = content.child_sheets.iter().any(|c| {
                resolve_child_reference_quiet(dir, &c.filename)
                    .is_ok_and(|p| normalised_key(&p, fold) == key)
            });
            if places {
                return Err(format!(
                    "`file` is already placed on {}; {tail}",
                    file_name(parent)
                ));
            }
        }
        Ok(())
    }

    /// The name rule (#2199 comment 6761836527) for the nets a sheet symbol's
    /// file brings into the project: a short or a rename of a project net is
    /// refused unless `allow_merge=true`. A file not there yet brings none. In
    /// a project that cannot be read completely the names on other sheets are
    /// unknown, so a file whose subtree carries a name that reaches other
    /// sheets is refused; one that carries none is placed. A loose sheet is
    /// not judged for names (`docs/CLI.md`).
    pub(super) fn refuse_child_names(
        &self,
        command: &Command,
        path: &Path,
        at: Point,
    ) -> Result<(), String> {
        let docs = &self.document_state;
        let sheet = docs.edit_path().ok_or("no schematic is open")?;
        if docs.document_project(sheet).is_none() {
            return Ok(());
        }
        let set = self.edit_project_set(sheet)?;
        let fold = set.fold;
        let key = normalised_key(path, fold);
        if set.absent.iter().any(|p| normalised_key(p, fold) == key) {
            return Ok(());
        }
        let root = crate::app::project_sheets::resolve_netlist_root_for(docs, sheet);
        let complete = set.unreadable.is_empty()
            && set.absent.is_empty()
            && set.refused.is_empty()
            && set.sheets.contains_key(&root);
        if !complete {
            if !self.file_subtree(path, sheet, fold)?.names {
                return Ok(());
            }
            return Err(format!(
                "the project cannot be read completely ({}), so the names on other sheets are unknown and the file's names reach them; fix the project, or pass allow_merge=true to place it anyway (see docs/CLI.md, \"Why allow_merge\")",
                super::project_names::unreadable_reason(&set)
            ));
        }
        let engine = docs.edit_engine().ok_or("no schematic is open")?;
        let after = Self::sheet_after(engine.document(), std::slice::from_ref(command))?;
        let changes = self
            .project_name_changes_of(&after, false)?
            .unwrap_or_default();
        let Some(change) = super::project_names::needing_merge(&changes) else {
            return Ok(());
        };
        Err(super::project_names::project_change_refusal(
            "sheet symbol",
            at,
            &change.names,
        ))
    }
}
