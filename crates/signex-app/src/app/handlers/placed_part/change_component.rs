//! Change Component (#1260), in three steps inside the component picker:
//! pick the replacement part, review what it does to the pins, confirm.
//! The read of the picked row is a `Task`; nothing touches the sheet until
//! the confirm, which is one engine command and so one undo step.

use iced::Task;
use signex_engine::{
    Command, ComponentChangePreview, ComponentReplacement, change_component_preview,
    placed_unit_ids,
};
use signex_types::schematic::{Symbol, normalized_lib_id};
use uuid::Uuid;

use super::{PlacedPart, part_name};
use crate::app::contracts::Message;
use crate::app::project_sheets::{project_pages, resolve_child_reference_quiet};
use crate::app::state::Signex;
use crate::app::state::scope::path_key_with;
use crate::library::PickerState;
use crate::library::messages::{
    LibraryMessage, PlacedPartMsg, PlacedPartTarget, ResolvedPlacement,
};
use crate::library::place::resolve_placement;
use crate::library::state::{ChangePreview, PickerMode};

impl Signex {
    pub(super) fn start_change_component(&mut self, part: PlacedPart) -> Task<Message> {
        let name = part_name(&part.symbol);
        if let Some(advice) = self.change_blocker(&part.target, &part.symbol) {
            crate::diagnostics::log_warning(advice);
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
        let blocker = doc
            .symbols
            .iter()
            .find(|s| s.uuid == target.symbol_id)
            .and_then(|symbol| self.change_blocker(&target, symbol));
        if let Some(advice) = blocker {
            self.library.picker = None;
            crate::diagnostics::log_warning(advice);
            return Task::none();
        }
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

    /// Why the part cannot be changed as a whole right now, as an advice
    /// line: a unit of it is locked, units of it are on other sheets, or a
    /// sheet that could hold units is not open so that cannot be told.
    ///
    /// Nothing here reads a file. A part that can have several units (its
    /// drawing has a unit above 1, or the placed unit is) needs the whole
    /// reachable hierarchy open: the target sheet, the owning project's
    /// declared pages and, repeating over every open sheet, the child sheets
    /// they reference. A single-unit part cannot have siblings (a duplicate
    /// designator is another part, as in `group_into_parts`), and an
    /// unannotated one is a part of its own.
    pub(super) fn change_blocker(
        &self,
        target: &PlacedPartTarget,
        symbol: &Symbol,
    ) -> Option<String> {
        let name = part_name(symbol);
        let docs = &self.document_state;
        let doc = self.target_document(target)?;
        let units = placed_unit_ids(doc, target.symbol_id);
        if units
            .iter()
            .any(|id| doc.symbols.iter().any(|s| s.uuid == *id && s.locked))
        {
            return Some(format!(
                "Unlock {name} in Properties to change its component: a unit of it is locked."
            ));
        }
        let reference = symbol.reference.trim();
        let may_have_units = symbol.unit > 1
            || doc.lib_symbols.get(&symbol.lib_id).is_none_or(|lib| {
                lib.pins.iter().any(|p| p.unit > 1) || lib.graphics.iter().any(|g| g.unit > 1)
            });
        if reference.is_empty() || reference.ends_with('?') || !may_have_units {
            return None;
        }
        let placed_units: Vec<u32> = doc
            .symbols
            .iter()
            .filter(|s| units.contains(&s.uuid))
            .map(|s| s.unit)
            .collect();
        let project = docs.document_project(&target.sheet);
        let fold = crate::app::state::project_case::fold_of(project);
        let key = |p: &std::path::Path| path_key_with(p, fold);
        let engines: std::collections::HashMap<
            String,
            (&std::path::PathBuf, &signex_engine::Engine),
        > = docs.engines.iter().map(|(p, e)| (key(p), (p, e))).collect();
        let owner = project.map(|p| p.path.clone());
        let mut reached = vec![target.sheet.clone()];
        reached.extend(project_pages(project));
        reached.extend(
            docs.engines
                .keys()
                .filter(|path| {
                    owner.is_some() && docs.document_project(path).map(|p| p.path.clone()) == owner
                })
                .cloned(),
        );
        let mut seen: std::collections::HashSet<String> = reached.iter().map(|p| key(p)).collect();
        reached.retain({
            let mut first = std::collections::HashSet::new();
            move |p| first.insert(key(p))
        });
        let mut next = 0;
        while next < reached.len() {
            let path = reached[next].clone();
            next += 1;
            let Some((_, engine)) = engines.get(&key(&path)) else {
                continue;
            };
            let dir = path.parent().unwrap_or_else(|| std::path::Path::new(""));
            for child in &engine.document().child_sheets {
                // The stitcher's own resolution; a reference it would not
                // follow (blank, or escaping the sheet's folder) is not a
                // sheet of this hierarchy.
                let Ok(child_path) = resolve_child_reference_quiet(dir, &child.filename) else {
                    continue;
                };
                if seen.insert(key(&child_path)) {
                    reached.push(child_path);
                }
            }
        }
        let file = |path: &std::path::Path| {
            path.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        };
        let (mut also_on, mut not_open) = (Vec::new(), Vec::new());
        for path in reached.iter().filter(|p| key(p) != key(&target.sheet)) {
            match engines.get(&key(path)) {
                Some((_, engine)) => {
                    let holds_a_unit = engine.document().symbols.iter().any(|s| {
                        s.reference.trim() == reference
                            && normalized_lib_id(&s.lib_id) == normalized_lib_id(&symbol.lib_id)
                            && !placed_units.contains(&s.unit)
                    });
                    if holds_a_unit {
                        also_on.push(file(path));
                    }
                }
                None => not_open.push(file(path)),
            }
        }
        if !also_on.is_empty() {
            return Some(format!(
                "Change Component works on one sheet; units of {name} are also on: {}",
                also_on.join(", ")
            ));
        }
        (!not_open.is_empty()).then(|| {
            format!(
                "Open every sheet of the project to change {name}: not open: {}",
                not_open.join(", ")
            )
        })
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
        // As a fresh placement leaves it; the row owns the footprint.
        footprint: String::new(),
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
