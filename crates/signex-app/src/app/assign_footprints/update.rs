//! Opening the dialog, building its table, its edits and Cancel. Apply is in
//! `apply.rs`.

use std::path::Path;
use std::sync::Arc;

use iced::Task;
use signex_types::schematic::{SchematicSheet, Symbol};

use super::AssignFootprintsMsg;
use super::read::{FootprintRead, ReadInputs, listed_parts, read_parts};
use super::state::{
    AssignFootprintsState, AssignProblem, FootprintOffer, FootprintPart, PartAccess,
};
use crate::app::project_sheets;
use crate::app::{Message, Signex};

/// How the sheet a part sits on can be edited here.
#[derive(Clone, Copy)]
enum SheetAccess {
    Open,
    Closed,
    ReadOnly,
}

impl Signex {
    pub(crate) fn dispatch_assign_footprints_message(
        &mut self,
        msg: AssignFootprintsMsg,
    ) -> Task<Message> {
        use AssignFootprintsMsg as M;
        if let M::Requested = msg {
            return self.open_assign_footprints();
        }
        if let M::Read { generation, result } = msg {
            return self.assign_footprints_read(generation, &result);
        }
        if let M::ApplyRequested = msg {
            self.apply_footprint_assignments();
            return Task::none();
        }
        let Some(state) = self.ui_state.assign_footprints.as_mut() else {
            return Task::none();
        };
        match msg {
            M::FootprintPicked { row, binding } => state.pick(row, &binding),
            M::TickedPicked(binding) => state.pick_selected(&binding),
            M::RowToggled(row) => state.toggle(row),
            M::AllToggled(on) => state.select_all(on),
            M::SameRowTicked(row) => state.select_same_row(row),
            M::CancelRequested => self.ui_state.assign_footprints = None,
            M::Requested | M::Read { .. } | M::ApplyRequested => {}
        }
        Task::none()
    }

    /// Open the dialog and start reading the project's sheets.
    fn open_assign_footprints(&mut self) -> Task<Message> {
        if self.ui_state.assign_footprints.is_some() {
            return Task::none();
        }
        let ds = &self.document_state;
        let Some(root) = project_sheets::resolve_netlist_root(ds) else {
            crate::diagnostics::log_warning(
                "Assign Footprints: open a schematic first; there are no parts to list.",
            );
            return Task::none();
        };
        let inputs = ReadInputs {
            open: project_sheets::open_sheets_snapshot(ds),
            pages: project_sheets::declared_pages(ds),
            root,
            active: ds.active_path.clone(),
            fold: crate::app::state::project_case::fold_of(ds.active_document_project()),
        };
        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;
        self.ui_state.assign_footprints_reads += 1;
        let generation = self.ui_state.assign_footprints_reads;
        self.ui_state.assign_footprints = Some(AssignFootprintsState::reading(
            generation,
            self.document_state.documents_epoch,
        ));
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || read_parts(&inputs)),
            move |result| {
                Message::from(AssignFootprintsMsg::Read {
                    generation,
                    result: result.map(Arc::new),
                })
            },
        )
    }

    /// The read finished. A read that an opened or closed sheet made stale
    /// is started again; an answer for a closed or newer dialog is dropped.
    fn assign_footprints_read(
        &mut self,
        generation: u64,
        result: &Result<Arc<FootprintRead>, String>,
    ) -> Task<Message> {
        let epoch = self.document_state.documents_epoch;
        let Some(state) = self.ui_state.assign_footprints.as_ref() else {
            return Task::none();
        };
        if state.generation != generation {
            return Task::none();
        }
        if state.epoch != epoch {
            self.ui_state.assign_footprints = None;
            return self.open_assign_footprints();
        }
        let parts = match result {
            Ok(read) => {
                for (path, why) in &read.unreadable {
                    crate::diagnostics::log_warning(format!(
                        "Assign Footprints: sheet '{}' not listed, {why}",
                        path.display()
                    ));
                }
                Ok(self.footprint_parts(read))
            }
            Err(why) => Err(why.clone()),
        };
        let Some(state) = self.ui_state.assign_footprints.as_mut() else {
            return Task::none();
        };
        match parts {
            Ok(parts) => state.show(parts),
            Err(why) => {
                let problem = AssignProblem::ReadFailed(why);
                crate::diagnostics::log_warning(format!(
                    "Assign Footprints: {}",
                    problem.english()
                ));
                state.error = Some(problem);
            }
        }
        Task::none()
    }

    /// Every listed part of the project, sorted by designator: an open
    /// sheet's from its live document, a closed one's from the read.
    fn footprint_parts(&self, read: &FootprintRead) -> Vec<FootprintPart> {
        let project_dir = self
            .document_state
            .active_document_project()
            .map(|p| p.dir().to_path_buf());
        let mut parts = Vec::new();
        for listed in &read.sheets {
            let path = self.resolve_held_spelling(listed.clone());
            let (doc, access): (&SchematicSheet, SheetAccess) =
                match self.document_state.engines.get(&path) {
                    Some(engine) if engine.document().unknown.newer_minor_source.is_some() => {
                        (engine.document(), SheetAccess::ReadOnly)
                    }
                    Some(engine) => (engine.document(), SheetAccess::Open),
                    None => match read.closed.get(listed) {
                        Some(sheet) if sheet.unknown.newer_minor_source.is_some() => {
                            (sheet, SheetAccess::ReadOnly)
                        }
                        Some(sheet) => (sheet, SheetAccess::Closed),
                        None => continue,
                    },
                };
            let label = shown(project_dir.as_deref(), &path);
            for symbol in listed_parts(doc) {
                parts.push(self.footprint_part(&path, &label, symbol, access));
            }
        }
        parts.sort_by(|a, b| {
            signex_types::designator::compare_references(&a.designator, &b.designator)
                .then_with(|| a.sheet.cmp(&b.sheet))
        });
        parts
    }

    /// One part's row: its library row's offers (the Properties list) and
    /// which of them it is built with.
    fn footprint_part(
        &self,
        sheet: &Path,
        sheet_label: &str,
        symbol: &Symbol,
        sheet_access: SheetAccess,
    ) -> FootprintPart {
        let row = self.row_of_placed_symbol(symbol).map(|(_, _, row)| row);
        // `footprint_choices_for` lists the row's footprints first, then at
        // most one extra entry: "(edited)" or "(no longer offered)".
        let offered = row.map_or(0, |row| row.footprint_bindings().len());
        let choices = self.footprint_choices_for(symbol);
        let offers: Vec<FootprintOffer> = choices
            .iter()
            .take(offered)
            .map(|c| FootprintOffer {
                binding: c.binding.clone(),
                label: c.label.clone(),
            })
            .collect();
        let current_binding = choices
            .iter()
            .take(offered)
            .find(|c| c.selected)
            .map(|c| c.binding.clone());
        let access = match sheet_access {
            SheetAccess::ReadOnly => PartAccess::ReadOnly,
            SheetAccess::Closed => PartAccess::SheetClosed,
            SheetAccess::Open if row.is_none() => PartAccess::NoRow,
            SheetAccess::Open if offers.len() < 2 => PartAccess::NoAlternates,
            SheetAccess::Open => PartAccess::Editable,
        };
        FootprintPart {
            sheet: sheet.to_path_buf(),
            sheet_label: sheet_label.to_string(),
            symbol_id: symbol.uuid,
            designator: symbol.reference.clone(),
            value: symbol.value.clone(),
            row_label: row.map(|row| row.internal_pn.as_str().to_string()),
            row_key: row.and(symbol.library_id.zip(symbol.row_id)),
            current: symbol.footprint.clone(),
            current_binding,
            offers,
            access,
        }
    }
}

/// `path` as the user knows it: relative to the project folder when inside it.
fn shown(project_dir: Option<&Path>, path: &Path) -> String {
    project_dir
        .and_then(|dir| path.strip_prefix(dir).ok())
        .unwrap_or(path)
        .display()
        .to_string()
}
