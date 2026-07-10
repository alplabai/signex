//! Library subsystem dispatcher. Routes
//! [`crate::library::LibraryMessage`] to the right side-effecting
//! handler.
//!
//! In the DBLib model the Component view is preview-only.
//! Symbol/Footprint/Sim render read-only here; the standalone
//! `.snxsym` / `.snxfpt` / `.snxsim` document tabs own actual
//! editing. The dispatcher's editor handlers are scoped to the five
//! Component Preview tabs (Preview / Parameters / Supply / Datasheet
//! / Simulation).

use iced::Task;

use super::super::*;
use crate::library::commands;
use crate::library::component_preview::apply_inline_edit;
use crate::library::editor::footprint::updates::{
    apply_footprint_clipboard_op, apply_footprint_primitive_edit,
};
use crate::library::messages::{
    BrowserEditMsg, CloseLibraryChoice, EditorMsg, FootprintEditorMsg, LibraryMessage, PickerMsg,
    PrimitiveEdit, PrimitivePickerMsg, SettingsMsg, SymbolEditorMsg,
};
use crate::library::state::{
    CloseLibraryConfirmState, ComponentPreviewState, DeleteConfirmState, DocumentOptionsModalState,
    EditRowModalState, EditorAddress, LibraryCreateOptionsState, NewComponentState, PickerState,
    PreviewTab, PrimitivePickerState, PrimitivePickerTarget,
};
use signex_library::{PrimitiveKind, PrimitiveRef, RowId};

mod browser;
mod component_preview;
mod editor;
mod primitive_picker;
mod recovery;
mod registration;
mod settings;
mod updates;

use recovery::{
    atomic_write, handle_recovery_broken_binding, handle_recovery_git_missing,
    handle_recovery_library_missing, route_open_error,
};

impl Signex {
    pub(crate) fn dispatch_library_message(&mut self, msg: LibraryMessage) -> Task<Message> {
        match msg {
            LibraryMessage::OpenLibraryDialog => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Open Library (*.snxlib/)")
                        .pick_folder()
                        .await
                        .map(|f| f.path().to_path_buf())
                },
                |path| Message::Library(LibraryMessage::OpenLibraryAt(path)),
            ),
            LibraryMessage::OpenLibraryAt(None) => Task::none(),
            LibraryMessage::OpenLibraryAt(Some(path)) => {
                if let Err(e) = commands::open_library(&mut self.library, path.clone()) {
                    tracing::warn!(target: "signex::library", error = %e, path = %path.display(), "open_library failed");
                    route_open_error(&mut self.library, &path, &e);
                }
                Task::none()
            }
            LibraryMessage::CloseLibrary(path) => {
                // If any Component Preview editors against this library
                // are dirty, divert to the confirm modal so the user
                // can Save All / Discard All / Cancel rather than
                // losing the edits silently. The modal handler
                // (`CloseLibraryConfirm`) finishes the close once the
                // user picks an option.
                let dirty = self.library.dirty_editors_for_library(&path);
                if dirty.is_empty() {
                    self.library.close_library(&path);
                } else {
                    let library_name = self
                        .library
                        .library_at(&path)
                        .map(|lib| lib.display_name.clone())
                        .unwrap_or_else(|| {
                            path.file_name()
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_else(|| path.display().to_string())
                        });
                    self.library.close_library_confirm = Some(CloseLibraryConfirmState {
                        library_path: path,
                        library_name,
                        dirty_editors: dirty,
                    });
                }
                Task::none()
            }
            LibraryMessage::OpenPicker => {
                self.library.picker = Some(PickerState::default());
                Task::none()
            }
            LibraryMessage::ClosePicker => {
                self.library.picker = None;
                Task::none()
            }

            // ── New Component flow ───────────────────────────────────
            // v0.13 — No modal. Append a draft row directly to the
            // active library's first table, then OpenComponentRow so
            // the Library Browser tab focuses the new row. The user
            // fills in the PN inline in the table and picks the
            // symbol / footprint via the Properties panel.
            //
            // When no library is open OR no table exists, fall back
            // to the legacy modal path so the user gets a clear
            // picker UI (the modal handles "no library" + table
            // selection edge cases).
            LibraryMessage::NewComponent => {
                if self.library.open_libraries.is_empty() {
                    // No library — fall back to the legacy modal
                    // path; the modal explains the empty-library
                    // recovery flow.
                    self.library.new_component = Some(NewComponentState {
                        library_idx: None,
                        ..NewComponentState::default()
                    });
                    return Task::none();
                }
                let library_idx = 0usize;
                let library = &self.library.open_libraries[library_idx];
                let library_path = library.root.clone();
                let library_id = library.library_id;
                let class = signex_library::ComponentClass::default();
                let table = self
                    .library
                    .set
                    .get(library_id)
                    .map(|adapter| adapter.manifest().table_for_class(class.as_str()));
                let Some(table) = table else {
                    // Library has no tables registered — fall back
                    // to modal so the user can pick / create one.
                    self.library.new_component = Some(NewComponentState {
                        library_idx: Some(library_idx),
                        ..NewComponentState::default()
                    });
                    return Task::none();
                };
                match commands::create_component_row(
                    &mut self.library,
                    library_idx,
                    &table,
                    "",
                    class,
                    None,
                    None,
                ) {
                    Ok(row_id) => {
                        return Task::done(Message::Library(LibraryMessage::OpenComponentRow {
                            library_path,
                            table,
                            row_id,
                        }));
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            error = %e,
                            "NewComponent: append-row failed; falling back to modal",
                        );
                        self.library.new_component = Some(NewComponentState {
                            library_idx: Some(library_idx),
                            error: Some(e.to_string()),
                            ..NewComponentState::default()
                        });
                    }
                }
                Task::none()
            }
            LibraryMessage::CloseNewComponent => {
                self.library.new_component = None;
                Task::none()
            }
            LibraryMessage::NewComponentSetInternalPn(s) => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.internal_pn = s;
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSetLibrary(idx) => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.library_idx = Some(idx);
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSetClass(class) => {
                // Changing class does NOT overwrite the table pick —
                // that's the user's explicit choice. Class only
                // affects the parameter template.
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.class = class;
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSetTable(name) => {
                // User picked a target table. If exactly one class is
                // associated with this table in the manifest, surface
                // that as the auto-class so the form fills out
                // sensibly. Otherwise the user keeps editing the class
                // independently.
                if let Some(nc) = self.library.new_component.as_mut() {
                    if !name.is_empty() {
                        nc.table = Some(name.clone());
                        // Try to autoselect the matching class from the
                        // manifest (`[[tables]]` override). Only triggers
                        // when the user picked a manifest-declared table.
                        if let Some(library_idx) = nc.library_idx
                            && let Some(lib) = self.library.open_libraries.get(library_idx)
                            && let Some(adapter) = self.library.set.get(lib.library_id)
                            && let Some(cfg) =
                                adapter.manifest().tables().iter().find(|c| c.name == name)
                            && let Some(first) = cfg.classes.first()
                        {
                            nc.class = signex_library::ComponentClass::new(first);
                        }
                    } else {
                        nc.table = None;
                    }
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSetCategory(s) => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.category = s;
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserBeginAddTable { library_path } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.adding_table = Some(crate::library::state::NewTableDraft::default());
                }
                Task::none()
            }
            LibraryMessage::BrowserSetNewTableName {
                library_path,
                value,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path)
                    && let Some(draft) = state.adding_table.as_mut()
                {
                    draft.name = value;
                    draft.error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserCancelAddTable { library_path } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.adding_table = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserDeleteTable {
                library_path,
                table,
            } => {
                let library_id = match self.library.library_at(&library_path) {
                    Some(lib) => lib.library_id,
                    None => return Task::none(),
                };
                let adapter = match self.library.set.get(library_id) {
                    Some(a) => a,
                    None => return Task::none(),
                };
                if let Err(error) =
                    adapter.delete_empty_table(&table, &format!("delete empty table {table}"))
                {
                    if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                        s.delete_error = Some(error.to_string());
                    }
                    return Task::none();
                }
                if let Err(e) = self.library.refresh_components(&library_path) {
                    tracing::warn!(
                        target: "signex::library",
                        path = %library_path.display(),
                        error = %e,
                        "refresh after delete table failed"
                    );
                }
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.delete_error = None;
                    if s.active_table.as_deref() == Some(table.as_str()) {
                        s.active_table = None;
                    }
                }
                Task::none()
            }
            LibraryMessage::BrowserDismissDeleteError { library_path } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.delete_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserBeginRenameTable {
                library_path,
                table,
            } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.renaming_table = Some((table.clone(), table));
                    s.rename_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserSetRenameName {
                library_path,
                value,
            } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                    && let Some((_, buf)) = s.renaming_table.as_mut()
                {
                    *buf = value;
                    s.rename_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserCancelRenameTable { library_path } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.renaming_table = None;
                    s.rename_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserBeginAddClass { library_path } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.adding_class = Some(crate::library::state::NewClassDraft::default());
                    s.class_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserSetNewClassKey {
                library_path,
                value,
            } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                    && let Some(d) = s.adding_class.as_mut()
                {
                    d.key = value;
                    d.error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserSetNewClassLabel {
                library_path,
                value,
            } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                    && let Some(d) = s.adding_class.as_mut()
                {
                    d.label = value;
                    d.error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserCancelAddClass { library_path } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.adding_class = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserConfirmAddClass { library_path } => {
                let Some(state) = self.library.library_browsers.get(&library_path).cloned() else {
                    return Task::none();
                };
                let Some(draft) = state.adding_class.clone() else {
                    return Task::none();
                };
                let key = draft.key.trim().to_string();
                let label = draft.label.trim().to_string();
                if key.is_empty() || label.is_empty() {
                    if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                        && let Some(d) = s.adding_class.as_mut()
                    {
                        d.error = Some("Both key and label are required.".into());
                    }
                    return Task::none();
                }
                let library_id = match self.library.library_at(&library_path) {
                    Some(lib) => lib.library_id,
                    None => return Task::none(),
                };
                let adapter = match self.library.set.get(library_id) {
                    Some(a) => a,
                    None => return Task::none(),
                };
                // Atomic add — `add_library_class` does the
                // duplicate-key check + push inside one
                // `mutate_library_file` borrow on LocalGitAdapter
                // (and the trait default falls back to the legacy
                // two-step path for adapters without single-borrow
                // support).
                if let Err(error) = adapter.add_library_class(
                    signex_library::ClassEntry {
                        key: key.clone(),
                        label,
                    },
                    &format!("add class {key}"),
                ) {
                    if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                        && let Some(d) = s.adding_class.as_mut()
                    {
                        d.error = Some(error.to_string());
                    }
                    return Task::none();
                }
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.adding_class = None;
                    s.class_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserDeleteClass { library_path, key } => {
                let library_id = match self.library.library_at(&library_path) {
                    Some(lib) => lib.library_id,
                    None => return Task::none(),
                };
                let adapter = match self.library.set.get(library_id) {
                    Some(a) => a,
                    None => return Task::none(),
                };
                if let Err(error) =
                    adapter.remove_library_class(&key, &format!("delete class {key}"))
                {
                    if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                        s.class_error = Some(error.to_string());
                    }
                }
                Task::none()
            }
            LibraryMessage::BrowserBeginRenameClass { library_path, key } => {
                let library_id = match self.library.library_at(&library_path) {
                    Some(lib) => lib.library_id,
                    None => return Task::none(),
                };
                let label = self
                    .library
                    .set
                    .get(library_id)
                    .and_then(|adapter| {
                        adapter.library_classes().into_iter().find(|c| c.key == key)
                    })
                    .map(|c| c.label)
                    .unwrap_or_default();
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.renaming_class = Some((key.clone(), key, label));
                    s.class_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserSetRenameClassKey {
                library_path,
                value,
            } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                    && let Some((_, k, _)) = s.renaming_class.as_mut()
                {
                    *k = value;
                    s.class_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserSetRenameClassLabel {
                library_path,
                value,
            } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                    && let Some((_, _, l)) = s.renaming_class.as_mut()
                {
                    *l = value;
                    s.class_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserCancelRenameClass { library_path } => {
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.renaming_class = None;
                    s.class_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserConfirmRenameClass { library_path } => {
                let Some(state) = self.library.library_browsers.get(&library_path).cloned() else {
                    return Task::none();
                };
                let Some((orig, new_key, new_label)) = state.renaming_class.clone() else {
                    return Task::none();
                };
                let new_key = new_key.trim().to_string();
                let new_label = new_label.trim().to_string();
                if new_key.is_empty() || new_label.is_empty() {
                    if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                        s.class_error = Some("Both key and label are required.".into());
                    }
                    return Task::none();
                }
                let library_id = match self.library.library_at(&library_path) {
                    Some(lib) => lib.library_id,
                    None => return Task::none(),
                };
                let adapter = match self.library.set.get(library_id) {
                    Some(a) => a,
                    None => return Task::none(),
                };
                // Atomic rename — `rename_library_class` does the
                // existence check + duplicate check + replace inside
                // a single `mutate_library_file` borrow on the
                // LocalGitAdapter (and falls back to the trait
                // default's two-step on adapters without
                // single-borrow support).
                if let Err(error) = adapter.rename_library_class(
                    &orig,
                    signex_library::ClassEntry {
                        key: new_key.clone(),
                        label: new_label,
                    },
                    &format!("rename class {orig} → {new_key}"),
                ) {
                    if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                        s.class_error = Some(error.to_string());
                    }
                    return Task::none();
                }
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.renaming_class = None;
                    s.class_error = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserConfirmRenameTable { library_path } => {
                let Some(state) = self.library.library_browsers.get(&library_path).cloned() else {
                    return Task::none();
                };
                let Some((old_name, new_buf)) = state.renaming_table.clone() else {
                    return Task::none();
                };
                let new_trimmed = new_buf.trim().to_string();
                if new_trimmed == old_name {
                    if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                        s.renaming_table = None;
                    }
                    return Task::none();
                }
                let library_id = match self.library.library_at(&library_path) {
                    Some(lib) => lib.library_id,
                    None => return Task::none(),
                };
                let adapter = match self.library.set.get(library_id) {
                    Some(a) => a,
                    None => return Task::none(),
                };
                if let Err(error) = adapter.rename_table(
                    &old_name,
                    &new_trimmed,
                    &format!("rename table {old_name} → {new_trimmed}"),
                ) {
                    if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                        s.rename_error = Some(error.to_string());
                    }
                    return Task::none();
                }
                if let Err(e) = self.library.refresh_components(&library_path) {
                    tracing::warn!(
                        target: "signex::library",
                        path = %library_path.display(),
                        error = %e,
                        "refresh after rename table failed"
                    );
                }
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.renaming_table = None;
                    s.rename_error = None;
                    if s.active_table.as_deref() == Some(old_name.as_str()) {
                        s.active_table = Some(new_trimmed);
                    }
                }
                Task::none()
            }
            LibraryMessage::BrowserConfirmAddTable { library_path } => {
                let Some(state) = self.library.library_browsers.get(&library_path).cloned() else {
                    return Task::none();
                };
                let Some(draft) = state.adding_table.as_ref().cloned() else {
                    return Task::none();
                };
                let trimmed = draft.name.trim().to_string();
                if trimmed.is_empty() {
                    if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                        && let Some(d) = s.adding_table.as_mut()
                    {
                        d.error = Some("Table name cannot be empty.".into());
                    }
                    return Task::none();
                }
                let library_id = match self.library.library_at(&library_path) {
                    Some(lib) => lib.library_id,
                    None => return Task::none(),
                };
                let adapter = match self.library.set.get(library_id) {
                    Some(a) => a,
                    None => return Task::none(),
                };
                if let Err(error) =
                    adapter.create_empty_table(&trimmed, &format!("create empty table {trimmed}"))
                {
                    if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                        && let Some(d) = s.adding_table.as_mut()
                    {
                        d.error = Some(error.to_string());
                    }
                    return Task::none();
                }
                if let Err(e) = self.library.refresh_components(&library_path) {
                    tracing::warn!(
                        target: "signex::library",
                        path = %library_path.display(),
                        error = %e,
                        "refresh after add table failed"
                    );
                }
                if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                    s.adding_table = None;
                    s.active_table = Some(trimmed);
                }
                Task::none()
            }
            LibraryMessage::NewComponentToggleAdvanced => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.advanced_open = !nc.advanced_open;
                    if !nc.advanced_open {
                        // Closing the disclosure also clears any
                        // in-flight + New Table form so the user
                        // doesn't reopen Advanced and find a stale
                        // half-typed name.
                        nc.creating_table = None;
                    }
                }
                Task::none()
            }
            LibraryMessage::NewComponentBeginCreateTable => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.creating_table = Some(crate::library::state::NewTableDraft::default());
                    nc.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSetNewTableName(name) => {
                if let Some(nc) = self.library.new_component.as_mut()
                    && let Some(draft) = nc.creating_table.as_mut()
                {
                    draft.name = name;
                    draft.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentCancelCreateTable => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    nc.creating_table = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentConfirmCreateTable => {
                let Some(nc) = self.library.new_component.as_ref() else {
                    return Task::none();
                };
                let Some(draft) = nc.creating_table.as_ref().cloned() else {
                    return Task::none();
                };
                let trimmed = draft.name.trim().to_string();
                if trimmed.is_empty() {
                    if let Some(slot) = self.library.new_component.as_mut()
                        && let Some(d) = slot.creating_table.as_mut()
                    {
                        d.error = Some("Table name cannot be empty.".into());
                    }
                    return Task::none();
                }
                let Some(library_idx) = nc.library_idx else {
                    if let Some(slot) = self.library.new_component.as_mut()
                        && let Some(d) = slot.creating_table.as_mut()
                    {
                        d.error = Some("Pick a library first.".into());
                    }
                    return Task::none();
                };
                let lib = match self.library.open_libraries.get(library_idx) {
                    Some(lib) => lib,
                    None => return Task::none(),
                };
                let library_id = lib.library_id;
                let lib_path = lib.root.clone();
                let adapter = match self.library.set.get(library_id) {
                    Some(a) => a,
                    None => return Task::none(),
                };
                if let Err(error) =
                    adapter.create_empty_table(&trimmed, &format!("create empty table {trimmed}"))
                {
                    if let Some(slot) = self.library.new_component.as_mut()
                        && let Some(d) = slot.creating_table.as_mut()
                    {
                        d.error = Some(error.to_string());
                    }
                    return Task::none();
                }
                if let Err(e) = self.library.refresh_components(&lib_path) {
                    tracing::warn!(
                        target: "signex::library",
                        path = %lib_path.display(),
                        error = %e,
                        "refresh after create_empty_table failed"
                    );
                }
                if let Some(slot) = self.library.new_component.as_mut() {
                    slot.creating_table = None;
                    slot.table = Some(trimmed);
                    slot.error = None;
                }
                Task::none()
            }
            LibraryMessage::NewComponentSubmit => {
                let Some(nc) = self.library.new_component.as_ref().cloned() else {
                    return Task::none();
                };
                let library_idx = match nc.library_idx {
                    Some(i) => i,
                    None => {
                        if let Some(slot) = self.library.new_component.as_mut() {
                            slot.error = Some("Pick a target library before submitting.".into());
                        }
                        return Task::none();
                    }
                };
                // Target table — modal pick takes precedence. When
                // the manifest declared no `[[tables]]` overrides the
                // modal still surfaces a default-pluralised slot;
                // fall back to `Manifest::table_for_class` if the
                // user submitted with an unset pick (ghost case when
                // the modal opens with neither a pre-pick nor a
                // user-selected table).
                let library_path = match self.library.open_libraries.get(library_idx) {
                    Some(lib) => lib.root.clone(),
                    None => {
                        if let Some(slot) = self.library.new_component.as_mut() {
                            slot.error = Some("Selected library is no longer open.".into());
                        }
                        return Task::none();
                    }
                };
                let table = match nc.table.clone() {
                    Some(t) => t,
                    None => {
                        let resolved = self
                            .library
                            .open_libraries
                            .get(library_idx)
                            .and_then(|lib| self.library.set.get(lib.library_id))
                            .map(|adapter| adapter.manifest().table_for_class(nc.class.as_str()));
                        match resolved {
                            Some(t) => t,
                            None => {
                                if let Some(slot) = self.library.new_component.as_mut() {
                                    slot.error =
                                        Some("Pick a target table before submitting.".into());
                                }
                                return Task::none();
                            }
                        }
                    }
                };
                match commands::create_component_row(
                    &mut self.library,
                    library_idx,
                    &table,
                    &nc.internal_pn,
                    nc.class.clone(),
                    nc.symbol_ref,
                    nc.footprint_ref,
                ) {
                    Ok(row_id) => {
                        self.library.new_component = None;
                        return Task::done(Message::Library(LibraryMessage::OpenComponentRow {
                            library_path,
                            table,
                            row_id,
                        }));
                    }
                    Err(e) => {
                        if let Some(slot) = self.library.new_component.as_mut() {
                            slot.error = Some(e.to_string());
                        }
                    }
                }
                Task::none()
            }
            LibraryMessage::NewComponentSubmit => {
                // WS-8 will replace `commands::create_component` with the
                // row-based `create_component_row`. Until that lands, the
                // dispatcher logs the request and bails.
                tracing::warn!(
                    target: "signex::library",
                    "NewComponentSubmit: row-based create flow ships in WS-8"
                );
                self.library.new_component = None;
                Task::none()
            }

            LibraryMessage::ToggleLibraryTreeNode(idx) => {
                if let Some(slot) = self.library.expanded.get_mut(idx) {
                    *slot = !*slot;
                }
                Task::none()
            }
            LibraryMessage::OpenComponentRow {
                library_path,
                table,
                row_id,
            } => self.handle_open_component_row(library_path, table, row_id),
            LibraryMessage::OpenPrimitiveEditor { path } => {
                tracing::info!(
                    target: "signex::library",
                    path = %path.display(),
                    "OpenPrimitiveEditor — standalone document tab opens in WS-7"
                );
                Task::none()
            }
            LibraryMessage::EditorEvent {
                library_path,
                table,
                row_id,
                msg,
            } => self.handle_editor_event(EditorAddress::new(library_path, table, row_id), msg),
            LibraryMessage::Picker(msg) => self.handle_picker_message(msg),
            LibraryMessage::Settings(msg) => self.handle_library_settings_message(msg),
            LibraryMessage::JumpToUseSite(site) => {
                commands::jump_to_use_site(&site);
                Task::none()
            }
            LibraryMessage::Noop => Task::none(),

            LibraryMessage::ConfirmCloseLibrary {
                library_path,
                dirty_editors,
            } => {
                // Direct opener for the modal — used when callers
                // already know the dirty list (e.g. a future
                // workspace-close batch op). For the user-driven
                // close path, `CloseLibrary` is the entry point and
                // it diverts here automatically.
                let library_name = self
                    .library
                    .library_at(&library_path)
                    .map(|lib| lib.display_name.clone())
                    .unwrap_or_else(|| {
                        library_path
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| library_path.display().to_string())
                    });
                self.library.close_library_confirm = Some(CloseLibraryConfirmState {
                    library_path,
                    library_name,
                    dirty_editors,
                });
                Task::none()
            }
            LibraryMessage::CloseLibraryConfirm(choice) => {
                let Some(confirm) = self.library.close_library_confirm.take() else {
                    return Task::none();
                };
                match choice {
                    CloseLibraryChoice::Cancel => {
                        // No state change — user kept the library open.
                    }
                    CloseLibraryChoice::DiscardAll => {
                        // Drop every dirty editor and proceed with the close.
                        // `close_library` retains-not by `library_path`, so
                        // this happens automatically as part of the close.
                        self.library.close_library(&confirm.library_path);
                    }
                    CloseLibraryChoice::SaveAll => {
                        // Persist every dirty editor's row through the
                        // adapter (`handle_save_row` already runs the
                        // hash + commit cycle), then close the
                        // library. Failures are logged; we still
                        // proceed with the close so the user isn't
                        // trapped (the rows stay on disk in their
                        // last good state).
                        for address in &confirm.dirty_editors {
                            self.handle_save_row(address);
                        }
                        self.library.close_library(&confirm.library_path);
                    }
                }
                Task::none()
            }
            LibraryMessage::PlaceLibraryComponent {
                library_path,
                table,
                row_id,
            } => self.handle_place_library_component(library_path, table, row_id),
            LibraryMessage::CreateLibraryAt(project_root) => {
                self.handle_create_library_for_project(project_root)
            }
            LibraryMessage::CreateLibraryAtPath {
                project_path,
                lib_path,
            } => {
                // Stage 11 of `v0.9-snxlib-as-file-plan.md`: pop the
                // "Library Options" modal here instead of creating
                // immediately. The modal lets the user opt into Git
                // LFS for binary 3D models before any disk +
                // `git init` runs. Confirming dispatches
                // `LibraryCreateOptionsConfirm` which calls into
                // `handle_create_library_at_path`.
                self.library.create_options = Some(LibraryCreateOptionsState {
                    project_path,
                    lib_path,
                    enable_git: false,
                    use_lfs: false,
                });
                Task::none()
            }
            LibraryMessage::LibraryCreateOptionsToggleLfs => {
                if let Some(state) = self.library.create_options.as_mut() {
                    state.use_lfs = !state.use_lfs;
                }
                Task::none()
            }
            LibraryMessage::LibraryCreateOptionsToggleGit => {
                if let Some(state) = self.library.create_options.as_mut() {
                    state.enable_git = !state.enable_git;
                    // LFS is meaningless without git — keep the two
                    // toggles consistent so the user doesn't end up
                    // with LFS-on-no-git which the adapter would
                    // silently drop anyway.
                    if !state.enable_git {
                        state.use_lfs = false;
                    }
                }
                Task::none()
            }
            LibraryMessage::LibraryCreateOptionsCancel => {
                self.library.create_options = None;
                Task::none()
            }
            LibraryMessage::LibraryCreateOptionsConfirm => {
                if let Some(state) = self.library.create_options.take() {
                    self.handle_create_library_at_path(
                        state.project_path,
                        state.lib_path,
                        state.enable_git,
                        state.use_lfs,
                    )
                } else {
                    Task::none()
                }
            }
            LibraryMessage::AddLibrarySymbolFilePicked(path) => {
                self.handle_add_library_symbol_file_picked(path)
            }
            LibraryMessage::AddLibraryFootprintFilePicked(path) => {
                self.handle_add_library_footprint_file_picked(path)
            }
            LibraryMessage::ComponentPreviewOpened {
                path,
                table,
                row_id,
            } => {
                tracing::debug!(
                    target: "signex::library",
                    path = %path.display(),
                    table = %table,
                    row_id = %row_id,
                    "ComponentPreviewOpened — Component Preview tab opened"
                );
                Task::none()
            }
            LibraryMessage::PrimitiveEditorEvent { path, msg } => {
                self.handle_primitive_editor_event(path, msg)
            }
            // ── Library Browser tab ──────────────────────────────────
            LibraryMessage::OpenLibraryBrowser(path) => self.handle_open_library_browser(path),
            LibraryMessage::BrowserSelectTable {
                library_path,
                table,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.active_table = Some(table);
                    state.selected_row = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserSearchChanged {
                library_path,
                value,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.search = value.clone();
                }
                // Write-through so reopening this library next session
                // restores the same filter — UX_IMPROVEMENTS §1.1.
                // Per-path scoping prevents two libraries from
                // sharing the same search term.
                crate::fonts::write_library_browser_search(&library_path, &value);
                Task::none()
            }
            LibraryMessage::BrowserSortColumn {
                library_path,
                column_key,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.toggle_sort(column_key);
                }
                Task::none()
            }
            LibraryMessage::BrowserSelectRow {
                library_path,
                table,
                row_id,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    // Switch active table when the click lands on a row
                    // in a different table — keeps the preview pane and
                    // selection coherent.
                    state.active_table = Some(table);
                    state.selected_row = Some(row_id);
                }
                Task::none()
            }
            LibraryMessage::BrowserAddComponent {
                library_path,
                table,
            } => self.handle_browser_add_component(library_path, table),
            LibraryMessage::BrowserDeleteRowRequest {
                library_path,
                table,
                row_id,
            } => self.handle_browser_delete_row_request(library_path, table, row_id),
            LibraryMessage::BrowserDeleteRowConfirm {
                library_path,
                table,
                row_id,
            } => self.handle_browser_delete_row_confirm(library_path, table, row_id),
            LibraryMessage::BrowserDeleteRowCancel { library_path } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.delete_confirm = None;
                }
                Task::none()
            }
            LibraryMessage::OpenPrimitivePicker { kind, target } => {
                self.library.primitive_picker = Some(PrimitivePickerState {
                    kind,
                    target,
                    filter: String::new(),
                    error: None,
                });
                Task::none()
            }
            LibraryMessage::PrimitivePicker(msg) => self.handle_primitive_picker_msg(msg),
            LibraryMessage::BrowserOpenEditModal {
                library_path,
                table,
                row_id,
            } => self.handle_browser_open_edit_modal(library_path, table, row_id),
            LibraryMessage::BrowserEdit { library_path, msg } => {
                self.handle_browser_edit_msg(library_path, msg)
            }
            LibraryMessage::BrowserCellEdit {
                library_path,
                row_id,
                column,
                value,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.cell_edit.insert((row_id, column), value);
                }
                Task::none()
            }
            LibraryMessage::BrowserCellCommit {
                library_path,
                table,
                row_id,
                column,
            } => self.handle_browser_cell_commit(library_path, table, row_id, column),
            LibraryMessage::BrowserCellCancel {
                library_path,
                row_id,
                column,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.cell_edit.remove(&(row_id, column));
                }
                Task::none()
            }
            LibraryMessage::BrowserSetLifecycleFilter {
                library_path,
                filter,
            } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.lifecycle_filter = filter;
                    // Drop the row selection so we never end up with a
                    // selected row that the new filter has just hidden
                    // — the side preview pane would otherwise render
                    // a row the user can no longer see in the grid.
                    state.selected_row = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserClassFilterClicked { library_path, key } => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.class_filter = match state.class_filter.as_deref() {
                        Some(current) if current == key => None,
                        _ => Some(key.clone()),
                    };
                    // Reset selected_row in case the previously-selected row
                    // is filtered out.
                    state.selected_row = None;
                }
                Task::none()
            }
            LibraryMessage::BrowserRefreshPricing {
                library_path,
                table,
                row_id,
            } => {
                // Stage 18 stub — the real adapter dispatch lands once
                // `signex_library::DistributorAdapter::refresh_pricing`
                // gets a row-binding loop. For now we log so the wiring
                // path is observable when the user clicks the menu item.
                tracing::info!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "TODO: distributor refresh wiring (BrowserRefreshPricing)"
                );
                Task::none()
            }
            LibraryMessage::LibraryRefreshAllPricing(library_path) => {
                let count = self
                    .library
                    .library_at(&library_path)
                    .map(|lib| lib.total_rows())
                    .unwrap_or(0);
                tracing::info!(
                    target: "signex::library",
                    path = %library_path.display(),
                    rows = count,
                    "TODO: distributor refresh wiring (LibraryRefreshAllPricing)"
                );
                Task::none()
            }
            // ── Document Options modal (Tools ▸ Document Options) ──
            LibraryMessage::OpenDocumentOptions { library_path } => {
                if let Some(lib) = self.library.library_at(&library_path) {
                    self.library.document_options = Some(DocumentOptionsModalState {
                        library_path: lib.root.clone(),
                        library_name: lib.display_name.clone(),
                        draft: lib.display,
                    });
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsSetSheetColor(c) => {
                if let Some(s) = self.library.document_options.as_mut() {
                    s.draft.sheet_color = c;
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsToggleGrid => {
                if let Some(s) = self.library.document_options.as_mut() {
                    s.draft.grid_visible = !s.draft.grid_visible;
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsCycleGridSize => {
                if let Some(s) = self.library.document_options.as_mut() {
                    let sizes = crate::canvas::grid::GRID_SIZES_MM;
                    let i = sizes
                        .iter()
                        .position(|sz| (sz - s.draft.grid_size_mm).abs() < f32::EPSILON)
                        .unwrap_or(2);
                    s.draft.grid_size_mm = sizes[(i + 1) % sizes.len()];
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsCycleUnit => {
                use signex_types::coord::Unit;
                if let Some(s) = self.library.document_options.as_mut() {
                    s.draft.unit = match s.draft.unit {
                        Unit::Mm => Unit::Mil,
                        Unit::Mil => Unit::Inch,
                        Unit::Inch => Unit::Micrometer,
                        Unit::Micrometer => Unit::Mm,
                    };
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsApply => {
                if let Some(s) = self.library.document_options.take()
                    && let Some(lib) = self.library.containing_library_mut(&s.library_path)
                {
                    lib.display = s.draft;
                }
                // Clear every primitive editor's canvas cache so the
                // new sheet color / grid paints immediately. Cheap.
                for editor in self.document_state.symbol_editors.values_mut() {
                    editor.canvas_cache.clear();
                }
                for editor in self.document_state.footprint_editors.values_mut() {
                    editor.canvas_cache.clear();
                }
                Task::none()
            }
            LibraryMessage::DocumentOptionsCancel => {
                self.library.document_options = None;
                Task::none()
            }

            // Recovery dialogs (Stage 10).
            LibraryMessage::RecoveryLibraryMissing(choice) => {
                handle_recovery_library_missing(self, choice)
            }
            LibraryMessage::RecoveryLibraryMissingLocateResult(picked) => {
                self.library.recovery = None;
                if let Some(new_path) = picked {
                    return Task::done(Message::Library(LibraryMessage::OpenLibraryAt(Some(
                        new_path,
                    ))));
                }
                Task::none()
            }
            LibraryMessage::RecoveryGitMissing(choice) => handle_recovery_git_missing(self, choice),
            LibraryMessage::RecoveryBrokenBinding(choice) => {
                handle_recovery_broken_binding(self, choice)
            }

            // ── Library Updates Available modal (Stage 16) ─────────
            LibraryMessage::LibraryUpdatesToggleSelection(symbol_uuid) => {
                if let Some(state) = self.library.library_updates.as_mut() {
                    state.toggle(symbol_uuid);
                }
                Task::none()
            }
            LibraryMessage::LibraryUpdatesApply => {
                self.handle_library_updates_apply();
                Task::none()
            }
            LibraryMessage::LibraryUpdatesSkipAll => {
                if let Some(state) = self.library.library_updates.take() {
                    self.library
                        .skipped_updates_for
                        .insert(state.schematic_path);
                }
                Task::none()
            }
            LibraryMessage::LibraryUpdatesCancel => {
                self.library.library_updates = None;
                Task::none()
            }

            // ── Components Panel (Stage 9) ────────────────────────────
            LibraryMessage::ComponentsPanelToggleSection(src) => {
                use crate::library::state::ComponentsMountSource;
                let p = &mut self.library.components_panel;
                match src {
                    ComponentsMountSource::Project => p.collapsed_project = !p.collapsed_project,
                    ComponentsMountSource::Installed => {
                        p.collapsed_installed = !p.collapsed_installed
                    }
                    ComponentsMountSource::Global => p.collapsed_global = !p.collapsed_global,
                }
                Task::none()
            }
            LibraryMessage::ComponentsPanelSetFilter(value) => {
                self.library.components_panel.filter = value;
                Task::none()
            }
            LibraryMessage::ComponentsPanelAddLibrary(source) => {
                // Open `*.snxlib` directory picker — landing message is
                // `ComponentsPanelAddLibraryAt` so the dispatcher knows
                // which source bucket the result belongs to.
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Add Library (*.snxlib)")
                            .pick_folder()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    move |path| {
                        Message::Library(LibraryMessage::ComponentsPanelAddLibraryAt {
                            source,
                            path,
                        })
                    },
                )
            }
            LibraryMessage::ComponentsPanelAddLibraryAt { source, path } => {
                use crate::library::state::ComponentsMountSource;
                let Some(path) = path else {
                    return Task::none();
                };
                // Mount the library — same idempotent path the legacy
                // File ▸ Library ▸ Open Library… flow uses.
                if let Err(e) = commands::open_library(&mut self.library, path.clone()) {
                    tracing::warn!(
                        target: "signex::library",
                        error = %e,
                        path = %path.display(),
                        "components-panel add-library failed"
                    );
                    return Task::none();
                }
                match source {
                    ComponentsMountSource::Installed => {
                        if !self.library.installed_libraries.contains(&path) {
                            self.library.installed_libraries.push(path);
                        }
                    }
                    ComponentsMountSource::Global => {
                        match crate::panels::components_panel::global_prefs::add_path(path.clone())
                        {
                            Ok(updated) => {
                                self.library.global_libraries = updated;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    target: "signex::library",
                                    error = %e,
                                    "global_libraries.toml save failed"
                                );
                            }
                        }
                    }
                    ComponentsMountSource::Project => {
                        // Project section's "+ Add Library…" button is
                        // not rendered in Stage 9 (project libs come
                        // from `.snxprj`), but the dispatcher still
                        // handles the variant for future wiring.
                        tracing::info!(
                            target: "signex::library",
                            path = %path.display(),
                            "TODO: add-library to active project (ComponentsMountSource::Project)"
                        );
                    }
                }
                Task::none()
            }
            LibraryMessage::ComponentsPanelPromoteToGlobal(path) => {
                if let Some(idx) = self
                    .library
                    .installed_libraries
                    .iter()
                    .position(|p| p == &path)
                {
                    self.library.installed_libraries.remove(idx);
                    match crate::panels::components_panel::global_prefs::add_path(path.clone()) {
                        Ok(updated) => self.library.global_libraries = updated,
                        Err(e) => {
                            tracing::warn!(
                                target: "signex::library",
                                error = %e,
                                "promote-to-global save failed"
                            );
                        }
                    }
                }
                Task::none()
            }
            LibraryMessage::ComponentsPanelManageGlobal => {
                tracing::info!(
                    target: "signex::library",
                    "TODO: open Global Libraries management dialog"
                );
                Task::none()
            }
            LibraryMessage::ComponentsPanelAddToProject { library_path } => {
                tracing::info!(
                    target: "signex::library",
                    path = %library_path.display(),
                    "TODO: add library to active project's Project.libraries"
                );
                Task::none()
            }
            LibraryMessage::ComponentsPanelPlace {
                library_path,
                table,
                row_id,
            } => {
                // Stage 9 stub — the full ghost-component drag is polish
                // work. Dispatch through the existing place handler so
                // the row at least lands on the canvas via the picker
                // path until ghost-drag ships.
                Task::done(Message::Library(LibraryMessage::PlaceLibraryComponent {
                    library_path,
                    table,
                    row_id,
                }))
            }
        }
    }

}
