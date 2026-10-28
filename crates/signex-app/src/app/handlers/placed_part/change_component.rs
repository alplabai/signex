//! Change Component (#1260), in three steps inside the component picker:
//! pick the replacement part, review what it does to the pins, confirm.
//! The read of the picked row is a `Task`; nothing touches the sheet until
//! the confirm, which is one engine command and so one undo step.

use iced::Task;
use signex_engine::{
    Command, ComponentChangePreview, ComponentReplacement, change_component_preview,
};
use uuid::Uuid;

use super::{PlacedPart, part_name};
use crate::app::contracts::Message;
use crate::app::state::Signex;
use crate::library::PickerState;
use crate::library::messages::{
    LibraryMessage, PlacedPartMsg, PlacedPartTarget, ResolvedPlacement,
};
use crate::library::place::resolve_placement;
use crate::library::state::{ChangePreview, PickerMode};

impl Signex {
    pub(super) fn start_change_component(&mut self, part: PlacedPart) -> Task<Message> {
        let name = part_name(&part.symbol);
        if part.symbol.locked {
            crate::diagnostics::log_warning(format!(
                "Unlock {name} in Properties to change its component: it is locked."
            ));
            return Task::none();
        }
        let elsewhere = self.units_on_other_sheets(&part);
        if !elsewhere.is_empty() {
            crate::diagnostics::log_warning(format!(
                "Change Component works on one sheet; units of {name} are on other sheets: {}",
                elsewhere.join(", ")
            ));
            return Task::none();
        }
        if self.library.open_libraries.is_empty() {
            crate::diagnostics::log_warning(format!(
                "Open a library to change the component of {name}: none is open."
            ));
            return Task::none();
        }
        self.library.picker = Some(PickerState {
            mode: PickerMode::ChangeComponent(part.target),
            ..PickerState::default()
        });
        self.library.sync_view_caches();
        Task::none()
    }

    /// The picked row's read, off the update thread.
    pub(super) fn resolve_change_part(&mut self) -> Task<Message> {
        let Some(picker) = self.library.picker.as_ref() else {
            return Task::none();
        };
        let (PickerMode::ChangeComponent(target), Some((path, summary))) =
            (&picker.mode, picker.selected.clone())
        else {
            return Task::none();
        };
        let target = target.clone();
        let Some(table) = self.library.table_of_row(&path, summary.row_id) else {
            crate::diagnostics::log_warning(format!(
                "Refresh the library and pick again: {} is no longer in it.",
                summary.internal_pn.as_str()
            ));
            return Task::none();
        };
        let open_libraries: Vec<(Uuid, std::path::PathBuf)> = self
            .library
            .open_libraries
            .iter()
            .map(|l| (l.library_id, l.root.clone()))
            .collect();
        let picked_row = summary.row_id;
        let row_id = signex_library::RowId::from_uuid(picked_row);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    resolve_placement(&path, &table, row_id, &open_libraries)
                })
                .await
                .unwrap_or_else(|e| Err(format!("part read worker failed: {e}")))
            },
            move |result| {
                Message::Library(LibraryMessage::PlacedPart(
                    PlacedPartMsg::ChangePartResolved {
                        target: target.clone(),
                        row_id: picked_row,
                        result: result.map(Box::new),
                    },
                ))
            },
        )
    }

    /// The read finished: judge the replacement against the sheet and show
    /// the pin preview. A read the user has since dismissed is dropped.
    pub(super) fn show_change_preview(
        &mut self,
        target: PlacedPartTarget,
        row_id: Uuid,
        result: Result<Box<ResolvedPlacement>, String>,
    ) -> Task<Message> {
        let still_asked = matches!(
            self.library.picker.as_ref().map(|p| &p.mode),
            Some(PickerMode::ChangeComponent(t)) if *t == target
        );
        let still_picked = self
            .library
            .picker
            .as_ref()
            .and_then(|p| p.selected.as_ref())
            .is_some_and(|(_, summary)| summary.row_id == row_id);
        if !still_asked || !still_picked {
            return Task::none();
        }
        let name = self.target_reference(&target);
        let resolved = match result {
            Ok(resolved) => *resolved,
            Err(detail) => {
                crate::diagnostics::log_warning(format!(
                    "Pick another part to change {name}: {detail}"
                ));
                return Task::none();
            }
        };
        if resolved.is_power {
            crate::diagnostics::log_warning(format!(
                "Pick a component, not a power port, for {name}: {} is a power symbol.",
                resolved.internal_pn
            ));
            return Task::none();
        }
        let Some(doc) = self.target_document(&target) else {
            self.drop_change(&name, "the sheet or part changed");
            return Task::none();
        };
        let preview = change_component_preview(doc, target.symbol_id, &replacement_of(&resolved));
        if let Some(unit) = preview.missing_unit {
            crate::diagnostics::log_warning(format!(
                "Pick a part that has unit {unit}: the new part has no drawing for it, so {name} was not changed."
            ));
            return Task::none();
        }
        self.show_preview(target, name, resolved, preview);
        Task::none()
    }

    pub(super) fn back_out_of_change_preview(&mut self) -> Task<Message> {
        if let Some(picker) = self.library.picker.as_mut()
            && let PickerMode::ChangePreview(preview) = &picker.mode
        {
            picker.mode = PickerMode::ChangeComponent(preview.target.clone());
        }
        Task::none()
    }

    /// Apply the reviewed change as one engine command (one undo step), if
    /// the sheet still gives the preview the user saw; otherwise show the
    /// fresh preview instead of committing something unreviewed.
    pub(super) fn confirm_change_component(&mut self) -> Task<Message> {
        let Some(PickerState {
            mode: PickerMode::ChangePreview(change),
            ..
        }) = self.library.picker.as_ref().cloned()
        else {
            return Task::none();
        };
        let ChangePreview {
            target,
            reference,
            resolved,
            preview: shown,
        } = *change;
        let replacement = replacement_of(&resolved);
        let Some(doc) = self.target_document(&target) else {
            self.drop_change(&reference, "the sheet or part changed meanwhile");
            return Task::none();
        };
        let fresh = change_component_preview(doc, target.symbol_id, &replacement);
        if fresh != shown {
            crate::diagnostics::log_warning(format!(
                "Review the changed pins and confirm again: the sheet changed since the preview of {reference}."
            ));
            self.show_preview(target, reference, resolved, fresh);
            return Task::none();
        }
        if let Some(unit) = fresh.missing_unit {
            self.library.picker = None;
            crate::diagnostics::log_warning(format!(
                "Pick a part that has unit {unit}: the new part has no drawing for it, so {reference} was not changed."
            ));
            return Task::none();
        }
        self.library.picker = None;
        let part = resolved.internal_pn.clone();
        // A refusal (a locked part) is logged by the gateway.
        let changed = self.apply_engine_command(
            Command::ChangeComponent {
                symbol_id: target.symbol_id,
                replacement,
            },
            true,
            true,
        );
        if changed {
            crate::diagnostics::log_info(format!("{reference} now uses {part}."));
            if let Some(line) = lost_connections_line(&reference, &fresh) {
                crate::diagnostics::log_warning(line);
            }
        }
        Task::none()
    }

    fn show_preview(
        &mut self,
        target: PlacedPartTarget,
        reference: String,
        resolved: ResolvedPlacement,
        preview: ComponentChangePreview,
    ) {
        if let Some(picker) = self.library.picker.as_mut() {
            picker.mode = PickerMode::ChangePreview(Box::new(ChangePreview {
                target,
                reference,
                resolved,
                preview,
            }));
        }
    }

    /// Where the other units of the part (same designator, same drawing)
    /// sit on the project's other open sheets: one name per sheet.
    fn units_on_other_sheets(&self, part: &PlacedPart) -> Vec<String> {
        let reference = part.symbol.reference.trim();
        if reference.is_empty() {
            return Vec::new();
        }
        let docs = &self.document_state;
        let owner = docs
            .document_project(&part.target.sheet)
            .map(|p| p.path.clone());
        let mut sheets: Vec<String> = docs
            .engines
            .iter()
            .filter(|(path, _)| **path != part.target.sheet)
            .filter(|(path, _)| {
                owner.is_some() && docs.document_project(path).map(|p| p.path.clone()) == owner
            })
            .filter(|(_, engine)| {
                engine
                    .document()
                    .symbols
                    .iter()
                    .any(|s| s.reference.trim() == reference && s.lib_id == part.symbol.lib_id)
            })
            .map(|(path, _)| {
                path.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            })
            .collect();
        sheets.sort();
        sheets
    }

    /// The sheet of `target`, if it is still the active one on the engine
    /// the change started on and still holds the symbol.
    fn target_document(
        &self,
        target: &PlacedPartTarget,
    ) -> Option<&signex_types::schematic::SchematicSheet> {
        if self.document_state.active_path.as_deref() != Some(target.sheet.as_path()) {
            return None;
        }
        let engine = self.document_state.active_engine()?;
        let doc = engine.document();
        (engine.instance_id() == target.engine_instance
            && doc.symbols.iter().any(|s| s.uuid == target.symbol_id))
        .then_some(doc)
    }

    fn target_reference(&self, target: &PlacedPartTarget) -> String {
        self.document_state
            .engines
            .get(&target.sheet)
            .and_then(|e| {
                e.document()
                    .symbols
                    .iter()
                    .find(|s| s.uuid == target.symbol_id)
            })
            .map_or_else(|| "the part".to_string(), part_name)
    }

    fn drop_change(&mut self, name: &str, why: &str) {
        self.library.picker = None;
        crate::diagnostics::log_warning(format!(
            "Start Change Component again: {why}, so {name} was not changed."
        ));
    }
}

/// The replacement the engine applies for a read part.
fn replacement_of(resolved: &ResolvedPlacement) -> ComponentReplacement {
    ComponentReplacement {
        lib_symbol: resolved.lib_symbol.clone(),
        library_id: Some(resolved.library_id),
        row_id: Some(resolved.row_id),
        library_version: resolved.library_version.clone(),
        value: resolved.value.clone(),
        footprint: resolved.footprint.clone(),
        datasheet: resolved.datasheet.clone(),
        fields: resolved.fields.clone(),
    }
}

/// One Messages line for a change that went ahead with lost, moved or
/// gained connections, within the panel's cut; `None` when it was clean.
fn lost_connections_line(reference: &str, preview: &ComponentChangePreview) -> Option<String> {
    if preview.is_clean() {
        return None;
    }
    let mut parts = Vec::new();
    for (what, pins) in [
        ("dangling", &preview.unmatched_connected),
        ("moved", &preview.moved_connected),
        ("gained", &preview.gained),
    ] {
        if !pins.is_empty() {
            parts.push(format!("{what} {}", pins.join(",")));
        }
    }
    let line = format!("Check the wires of {reference}: {}.", parts.join("; "));
    Some(crate::text_trunc::prefix_within_bytes(&line, 157).to_string())
}
