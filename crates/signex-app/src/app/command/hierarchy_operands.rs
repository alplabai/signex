//! The hierarchy edits from operands (#2199, decision comment 6769277106):
//! `place_sheet_symbol`, `place_sheet_entry` and `sync_sheet_pins`. Each is one
//! undoable engine edit of the sheet; none writes or creates a file (a sheet
//! symbol only refers to its child file). Rules and refusals: `docs/CLI.md`,
//! "Placing parts and hierarchy". What their checks read from disk (a child
//! file, a link out of the project) is read off the UI thread before they run
//! (`edit_reads`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::json;
use signex_engine::Command;
use signex_types::schematic::{ChildSheet, LabelType, Point, normalize_port_name};

use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::canvas::{reference_from, resolve_new_sheet_path};
use crate::app::mutation_gateway::CommandOutcome as Applied;
use crate::app::project_sheets::{ProjectSheetSet, resolve_child_reference_quiet};
use crate::app::sheet_hierarchy::{
    CaseFold, SheetHierarchy, SheetNode, SheetNodeKind, normalised_key,
};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// `place_sheet_symbol start=x,y end=x,y name=.. file=.. [allow_merge=..]
/// [document=..]`.
pub(super) fn place_sheet_symbol(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.place_sheet_symbol_from_operands(args))
}

/// `place_sheet_entry sheet=.. at=x,y name=.. [direction=..] [document=..]`.
pub(super) fn place_sheet_entry(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.place_sheet_entry_from_operands(args))
}

/// `sync_sheet_pins sheet=.. [document=..]`.
pub(super) fn sync_sheet_pins(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.sync_sheet_pins_from_operands(args))
}

/// A point operand that must be a coordinate: a pin reference names a pin's
/// tip, which is no corner or border point of a sheet symbol.
fn refuse_pin_ref(args: &CommandArgs, name: &str) -> Result<(), String> {
    match args.operand(name) {
        Some(OperandValue::PinRef { .. }) => Err(format!(
            "`{name}` is a coordinate here, not a pin reference"
        )),
        _ => Ok(()),
    }
}

fn applied(outcome: Applied, reason: Option<String>) -> Result<(), String> {
    match outcome {
        Applied::Changed => Ok(()),
        Applied::Unchanged => Err("nothing was placed".to_string()),
        Applied::Failed => Err(reason.unwrap_or_else(|| "the engine refused the edit".into())),
    }
}

/// The keys of the sheets above any placement of the sheet at `path`.
fn sheets_above(hierarchy: &SheetHierarchy, path: &Path, fold: CaseFold) -> HashSet<String> {
    fn walk(
        node: &SheetNode,
        path: &Path,
        fold: CaseFold,
        chain: &mut Vec<String>,
        out: &mut HashSet<String>,
    ) {
        if node.kind == SheetNodeKind::Present && node.is_at(path, fold) {
            out.extend(chain.iter().cloned());
        }
        chain.push(normalised_key(&node.path, fold));
        for child in &node.children {
            walk(child, path, fold, chain, out);
        }
        chain.pop();
    }
    let mut out = HashSet::new();
    for node in hierarchy.top_level() {
        walk(node, path, fold, &mut Vec::new(), &mut out);
    }
    out
}

/// Whether the sheet at `file`, or one it places, carries a name that can
/// reach other sheets: a global label or a power port naming a net.
fn subtree_names_reach_out(set: &ProjectSheetSet, file: &Path) -> bool {
    let fold = set.fold;
    let mut seen = HashSet::new();
    let mut queue = vec![file.to_path_buf()];
    while let Some(path) = queue.pop() {
        if !seen.insert(normalised_key(&path, fold)) {
            continue;
        }
        let Some(sheet) = set.sheets.get(&path) else {
            continue;
        };
        let named = sheet
            .labels
            .iter()
            .any(|l| matches!(l.label_type, LabelType::Global | LabelType::Power))
            || sheet
                .symbols
                .iter()
                .any(|s| s.is_power && !s.value.is_empty());
        if named {
            return true;
        }
        let dir = path.parent().unwrap_or_else(|| Path::new(""));
        queue.extend(
            sheet
                .child_sheets
                .iter()
                .filter_map(|c| resolve_child_reference_quiet(dir, &c.filename).ok()),
        );
    }
    false
}

/// The names of `list` that `keep` keeps, sorted.
fn names(list: &[String], keep: impl Fn(&String) -> bool) -> Vec<String> {
    let mut kept: Vec<String> = list.iter().filter(|n| keep(n)).cloned().collect();
    kept.sort_unstable();
    kept
}

impl Signex {
    /// The one sheet symbol named `name` on the sheet an edit acts on.
    fn sheet_symbol_named(&self, name: &str) -> Result<&ChildSheet, String> {
        let engine = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        let found: Vec<&ChildSheet> = engine
            .document()
            .child_sheets
            .iter()
            .filter(|c| c.name == name)
            .collect();
        match found.as_slice() {
            [only] => Ok(only),
            [] => Err(format!("no sheet symbol `{name}` on this sheet")),
            many => Err(format!(
                "{} sheet symbols on this sheet are named `{name}`; rename one first",
                many.len()
            )),
        }
    }

    /// The child file `file` names from the sheet's folder, checked without a
    /// read: the project folder, the file's path and its reference. Refused
    /// unless it is a `.snxsch` inside the project folder.
    pub(super) fn child_file_target(
        &self,
        file: &str,
    ) -> Result<(PathBuf, PathBuf, String), String> {
        let docs = &self.document_state;
        let sheet = docs.edit_path().ok_or("no schematic is open")?;
        let parent_dir = sheet.parent().unwrap_or_else(|| Path::new(""));
        let project = docs.document_project(sheet);
        let project_dir: PathBuf =
            project.map_or_else(|| parent_dir.to_path_buf(), |p| p.dir().to_path_buf());
        let fold = crate::app::state::project_case::fold_of(project);
        let path = resolve_new_sheet_path(parent_dir, &project_dir, file, fold)
            .map_err(|why| format!("`file`: {why}"))?;
        let reference = reference_from(parent_dir, &path, fold);
        Ok((project_dir, path, reference))
    }

    /// The child file reference `file`: [`Self::child_file_target`], not
    /// through a link that leaves the project folder (read before the edit
    /// ran), and neither this sheet nor one above it. Nothing is created or
    /// opened.
    fn child_file_reference(&self, file: &str) -> Result<String, String> {
        let (_, path, reference) = self.child_file_target(file)?;
        if self.prefetched_escape(&path)? {
            return Err(
                "`file`: the sheet must be inside the project folder (a linked folder leaves it)"
                    .to_string(),
            );
        }
        let docs = &self.document_state;
        let sheet = docs.edit_path().ok_or("no schematic is open")?;
        let fold = crate::app::state::project_case::fold_of(docs.document_project(sheet));
        let target = normalised_key(&path, fold);
        if target == normalised_key(sheet, fold) {
            return Err("`file` is this sheet: a sheet cannot contain itself".to_string());
        }
        if self.sheets_above_edited(sheet, fold)?.contains(&target) {
            return Err(
                "`file` is an ancestor of this sheet: a sheet cannot contain itself".to_string(),
            );
        }
        if self.file_reaches(&path, sheet, fold)? {
            return Err(
                "`file` places this sheet below it: a sheet cannot contain itself".to_string(),
            );
        }
        Ok(reference)
    }

    /// The keys of the sheets above `sheet`: in its project's hierarchy built
    /// from the project as the edit's checks read it (the closed sheets read
    /// before the edit ran, `edit_reads`), and in the hierarchy a window
    /// already holds for it (a sheet the project does not own yet).
    fn sheets_above_edited(&self, sheet: &Path, fold: CaseFold) -> Result<HashSet<String>, String> {
        let mut above = self
            .hierarchy_describing(sheet)
            .map(|(h, _)| sheets_above(h, sheet, fold))
            .unwrap_or_default();
        if self.document_state.document_project(sheet).is_none() {
            return Ok(above);
        }
        let set = self.edit_project_set(sheet)?;
        let docs = &self.document_state;
        let root = crate::app::project_sheets::resolve_netlist_root_for(docs, sheet);
        let hierarchy = crate::app::sheet_hierarchy::build_sheet_hierarchy(
            &set.sheets,
            &set.unreadable,
            &root,
            &set.pages_outside_the_hierarchy,
            set.fold,
        );
        above.extend(sheets_above(&hierarchy, sheet, fold));
        Ok(above)
    }

    /// Whether the file at `file` places `sheet` somewhere below it: the walk
    /// down its child references, over the open sheets and the project as the
    /// edit read it (the file's subtree is read with it, `edit_reads`); for a
    /// loose sheet, over the file's subtree as read.
    fn file_reaches(&self, file: &Path, sheet: &Path, fold: CaseFold) -> Result<bool, String> {
        let docs = &self.document_state;
        let owned = match docs.document_project(sheet) {
            Some(_) => Some(self.edit_project_set(sheet)?),
            None => None,
        };
        let set = owned.as_ref().or_else(|| self.prefetched_subtree());
        let content = |path: &Path| {
            docs.engines
                .get(path)
                .map(signex_engine::Engine::document)
                .or_else(|| set.and_then(|s| s.sheets.get(path)))
        };
        let wanted = normalised_key(sheet, fold);
        let mut seen = HashSet::new();
        let mut queue = vec![file.to_path_buf()];
        while let Some(path) = queue.pop() {
            if !seen.insert(normalised_key(&path, fold)) {
                continue;
            }
            let Some(content) = content(&path) else {
                continue;
            };
            let dir = path.parent().unwrap_or_else(|| Path::new(""));
            for child in &content.child_sheets {
                let Ok(next) = resolve_child_reference_quiet(dir, &child.filename) else {
                    continue;
                };
                if normalised_key(&next, fold) == wanted {
                    return Ok(true);
                }
                queue.push(next);
            }
        }
        Ok(false)
    }

    /// The sheet symbol the operands describe, checked: corners, name and
    /// file, none taken on this sheet.
    fn new_sheet_symbol(&self, args: &CommandArgs) -> Result<ChildSheet, String> {
        refuse_pin_ref(args, "start")?;
        refuse_pin_ref(args, "end")?;
        let (start, end) = (
            self.grid_point(args, "start")?,
            self.grid_point(args, "end")?,
        );
        let name = text(args, "name")?.trim().to_string();
        let filename = self.child_file_reference(&text(args, "file")?)?;
        let sheet = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?
            .document();
        if sheet.child_sheets.iter().any(|c| c.name == name) {
            return Err(format!("a sheet symbol `{name}` is already on this sheet"));
        }
        if let Some(other) = signex_engine::other_sheet_symbol_using_file(
            &sheet.child_sheets,
            uuid::Uuid::nil(),
            &filename,
        ) {
            return Err(format!(
                "sheet symbol `{}` already refers to {filename}; one file is placed once",
                other.name
            ));
        }
        let Some(child_sheet) = signex_engine::child_sheet_from_corners(start, end, &name) else {
            return Err(format!(
                "each edge must be at least {} mm",
                signex_engine::MIN_CHILD_SHEET_EDGE_MM
            ));
        };
        Ok(ChildSheet {
            filename,
            ..child_sheet
        })
    }

    /// The name rule (#2199 comment 6761836527) for the nets a sheet symbol's
    /// file brings into the project: a short or a rename of a project net is
    /// refused unless `allow_merge=true`. A file not there yet brings none. In
    /// a project that cannot be read completely the names on other sheets are
    /// unknown, so a file whose subtree carries a name that reaches other
    /// sheets is refused; one that carries none is placed. A loose sheet is
    /// not judged for names (`docs/CLI.md`).
    fn refuse_child_names(&self, command: &Command, file: &str, at: Point) -> Result<(), String> {
        let docs = &self.document_state;
        let sheet = docs.edit_path().ok_or("no schematic is open")?;
        if docs.document_project(sheet).is_none() {
            return Ok(());
        }
        let (_, path, _) = self.child_file_target(file)?;
        let set = self.edit_project_set(sheet)?;
        let fold = set.fold;
        let key = normalised_key(&path, fold);
        if set.absent.iter().any(|p| normalised_key(p, fold) == key) {
            return Ok(());
        }
        let root = crate::app::project_sheets::resolve_netlist_root_for(docs, sheet);
        let complete = set.unreadable.is_empty()
            && set.absent.is_empty()
            && set.refused.is_empty()
            && set.sheets.contains_key(&root);
        if !complete {
            if !subtree_names_reach_out(&set, &path) {
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

    fn place_sheet_symbol_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        let child_sheet = self.new_sheet_symbol(args)?;
        let (id, at, size) = (child_sheet.uuid, child_sheet.position, child_sheet.size);
        let (name, filename) = (child_sheet.name.clone(), child_sheet.filename.clone());
        let command = Command::PlaceChildSheet { child_sheet };
        self.refuse_contacts(std::slice::from_ref(&command), "sheet symbol")?;
        if !matches!(args.operand("allow_merge"), Some(OperandValue::Bool(true))) {
            self.refuse_child_names(&command, &text(args, "file")?, at)?;
        }
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], false, true);
        applied(outcome, reason)?;
        Ok(CommandOutcome::Answered(json!({
            "id": id.to_string(),
            "name": name,
            "file": filename,
            "at": [at.x, at.y],
            "size": [size.0, size.1],
        })))
    }

    fn place_sheet_entry_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        refuse_pin_ref(args, "at")?;
        let at = self.grid_point(args, "at")?;
        let sheet_name = text(args, "sheet")?;
        let name = text(args, "name")?;
        let direction = text(args, "direction").ok();
        let symbol = self.sheet_symbol_named(&sheet_name)?;
        let Some(wanted) = normalize_port_name(&name) else {
            return Err("`name` is empty".to_string());
        };
        if symbol
            .pins
            .iter()
            .any(|p| normalize_port_name(&p.name) == Some(wanted))
        {
            return Err(format!(
                "sheet `{sheet_name}` already has an entry `{wanted}`"
            ));
        }
        let pin = signex_engine::new_sheet_entry(symbol, at, &name, direction.as_deref());
        if signex_net::pt_key(&pin.position) != signex_net::pt_key(&at) {
            return Err(format!(
                "{},{} is not on the border of sheet `{sheet_name}` (away from its corners); the nearest border point is {},{}",
                at.x, at.y, pin.position.x, pin.position.y
            ));
        }
        if let Some(given) = &direction
            && given.trim().to_ascii_lowercase() != pin.direction
        {
            return Err(format!(
                "`direction` must be input, output, bidirectional, tri_state or passive, not `{given}`"
            ));
        }
        let entry = json!({
            "sheet": sheet_name,
            "name": pin.name,
            "direction": pin.direction,
            "at": [pin.position.x, pin.position.y],
        });
        let command = Command::PlaceSheetPin {
            sheet_id: symbol.uuid,
            pin,
        };
        self.refuse_contacts(std::slice::from_ref(&command), "sheet entry")?;
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], false, true);
        applied(outcome, reason)?;
        Ok(CommandOutcome::Answered(entry))
    }

    fn sync_sheet_pins_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        let sheet_name = text(args, "sheet")?;
        let filename = self.sheet_symbol_named(&sheet_name)?.filename.clone();
        // The child file was read before the edit ran.
        let load = |path: &Path| self.prefetched_sheet(path);
        let sync = self.prepare_child_file_sync(&filename, &load)?;
        let command = sync.command();
        self.refuse_contacts(std::slice::from_ref(&command), "synced entries")?;
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], true, true);
        let before = sync.names_before.clone();
        let after = self.pin_names_for(&filename);
        match outcome {
            Applied::Failed => {
                return Err(reason.unwrap_or_else(|| "the engine refused the sync".into()));
            }
            Applied::Changed | Applied::Unchanged => self.report_file_sync(&sync),
        }
        Ok(CommandOutcome::Answered(json!({
            "sheet": sheet_name,
            "file": filename,
            "added": names(&after, |n| !before.contains(n)),
            "removed": names(&before, |n| !after.contains(n)),
            "kept": names(&after, |n| before.contains(n)),
        })))
    }
}
