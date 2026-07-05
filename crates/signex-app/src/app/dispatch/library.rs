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
use crate::library::messages::{
    BrowserEditMsg, CloseLibraryChoice, EditorMsg, LibraryMessage, ParamKindMsg, PickerMsg,
    PrimitiveEditorMsg, PrimitivePickerMsg, SettingsMsg,
};
use crate::library::state::{
    CloseLibraryConfirmState, ComponentPreviewState, DeleteConfirmState, DocumentOptionsModalState,
    EditRowModalState, EditorAddress, LibraryCreateOptionsState, NewComponentState, PickerState,
    PreviewTab, PrimitivePickerState, PrimitivePickerTarget,
};
use signex_library::{PrimitiveKind, PrimitiveRef, RowId};
// The standalone symbol-editor reducer lives next to its state now
// (issue #98); re-imported so the dispatcher call site is unchanged.
use crate::library::editor::symbol::apply::apply_symbol_primitive_edit;

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

    /// Open `.snxlib` at `path` as a Library Browser tab. Mounts the
    /// library if not already mounted, seeds the browser state, and
    /// pushes (or activates) a `TabKind::LibraryBrowser` tab. Phase 1.
    pub(crate) fn handle_open_library_browser(
        &mut self,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        tracing::info!(
            target: "signex::library",
            path = %path.display(),
            exists = path.exists(),
            already_mounted = self.library.library_at(&path).is_some(),
            "open_library_browser: enter"
        );
        // 1. Mount the library if it isn't already. `open_library` is
        //    idempotent — re-mounting an already-open library is a
        //    no-op.
        if let Err(e) = commands::open_library(&mut self.library, path.clone()) {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "open_library_browser: open_library failed"
            );
        }

        // 2. Seed per-browser state if the path isn't already there.
        // 2b. Default `active_table` to the first table the library
        //     exposes, if any. Compute it through an immutable borrow
        //     before we take the mutable browser-entry.
        let default_table: Option<String> = self.library.library_at(&path).and_then(|lib| {
            let mut names: Vec<&String> = lib.tables.keys().collect();
            names.sort();
            names.first().map(|s| (*s).clone())
        });

        // Hydrate persisted search query for this library (per-path,
        // not global) the first time a browser tab opens this session.
        // Reading the prefs file every open is fine — single-digit
        // milliseconds and only on user gesture.
        let persisted_search = crate::fonts::read_library_browser_searches()
            .remove(&path)
            .unwrap_or_default();

        let entry = self
            .library
            .library_browsers
            .entry(path.clone())
            .or_insert_with(|| {
                let mut s = crate::library::state::LibraryBrowserState::new(path.clone());
                s.search = persisted_search;
                s
            });

        if entry.active_table.is_none() {
            entry.active_table = default_table;
        }

        // 3. Activate an existing tab if one is already open for this
        //    path; otherwise push a fresh tab.
        if let Some(idx) = self.document_state.tabs.iter().position(|t| t.path == path) {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return Task::none();
        }

        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| path.display().to_string());
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);

        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path: path.clone(),
            cached_document: None,
            dirty: false,
            project_id,
            kind: crate::app::TabKind::LibraryBrowser(path),
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        // Library Browser tabs don't drive `active_path` — clear so the
        // canvas pane doesn't render a stale schematic.
        self.document_state.active_path = None;
        self.refresh_panel_ctx();
        Task::none()
    }

    /// Inline "+ Component" — mint a draft row directly into the
    /// browser's active table without opening any modal. The user
    /// fills in `internal_pn`, `manufacturer`, `mpn` etc. via the
    /// grid's inline cell editor; symbol / footprint binding lives in
    /// the Properties panel for the selected row.
    ///
    /// Library is implicit (the browser tab's library), table is the
    /// browser's `active_table` or — when none is selected — the
    /// generic-class default resolved through
    /// `manifest.table_for_class("generic")`. Closes F17 / F18 of the
    /// 2026-05-03 library polish: the New Component modal's library
    /// dropdown was meaningless inside a library tab, and the modal
    /// itself was a step the user didn't want.
    fn handle_browser_add_component(
        &mut self,
        library_path: std::path::PathBuf,
        table: Option<String>,
    ) -> Task<Message> {
        let library_idx = match self
            .library
            .open_libraries
            .iter()
            .position(|lib| lib.root == library_path)
        {
            Some(idx) => idx,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    library = %library_path.display(),
                    "browser: Add Component — library not mounted"
                );
                return Task::none();
            }
        };

        // Resolve target table — explicit arg wins, else fall back to
        // the generic class default.
        let generic = signex_library::ComponentClass::generic();
        let resolved_table = match table {
            Some(t) if !t.trim().is_empty() => t,
            _ => match self
                .library
                .open_libraries
                .get(library_idx)
                .and_then(|lib| self.library.set.get(lib.library_id))
                .map(|adapter| adapter.manifest().table_for_class(generic.as_str()))
            {
                Some(t) => t,
                None => {
                    tracing::warn!(
                        target: "signex::library",
                        library = %library_path.display(),
                        "browser: Add Component — no active table and no class default"
                    );
                    return Task::none();
                }
            },
        };

        // F19 (2026-05-03 library polish, "we had a talk about basic
        // parameters"): infer the row's class from the resolved table
        // name so a row added to the "resistors" table comes in as a
        // resistor (not a generic). Reverse-lookup is cheap: strip the
        // trailing "s" and verify that the candidate class round-trips
        // back to the same table via `manifest.table_for_class`.
        // Falls back to `generic` if no match — this covers user-named
        // tables like "passives" that don't follow the pluralisation
        // convention.
        let class = self
            .library
            .open_libraries
            .get(library_idx)
            .and_then(|lib| self.library.set.get(lib.library_id))
            .and_then(|adapter| {
                let manifest = adapter.manifest();
                resolved_table.strip_suffix('s').and_then(|stem| {
                    if manifest.table_for_class(stem) == resolved_table {
                        Some(signex_library::ComponentClass::new(stem))
                    } else {
                        None
                    }
                })
            })
            .unwrap_or(generic);

        match commands::create_component_row(
            &mut self.library,
            library_idx,
            &resolved_table,
            "", // empty PN — user fills it in via inline cell editor
            class,
            None, // symbol_ref bound later via Properties panel
            None, // footprint_ref bound later
        ) {
            Ok(row_id) => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.active_table = Some(resolved_table.clone());
                    state.selected_row = Some(row_id);
                }
                tracing::info!(
                    target: "signex::library",
                    library = %library_path.display(),
                    table = %resolved_table,
                    row_id = %row_id,
                    "browser: minted draft row inline"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    library = %library_path.display(),
                    table = %resolved_table,
                    error = %e,
                    "browser: inline row create failed"
                );
            }
        }
        Task::none()
    }

    /// Phase 2 — open the delete-row confirm modal. Records
    /// `(table, row_id, internal_pn)` on the browser state so the
    /// modal can render a confident message.
    fn handle_browser_delete_row_request(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let internal_pn = self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .map(|r| r.internal_pn.as_str().to_string())
            .unwrap_or_else(|| format!("row {row_id}"));
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.delete_confirm = Some(DeleteConfirmState {
                table,
                row_id,
                internal_pn,
            });
        }
        Task::none()
    }

    /// Confirm step — actually delete the row through
    /// `adapter.delete_row` and refresh the cache.
    fn handle_browser_delete_row_confirm(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    "browser delete: library not mounted"
                );
                return Task::none();
            }
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    "browser delete: adapter not present in set"
                );
                return Task::none();
            }
        };
        match adapter.delete_row(&table, row_id, "delete row") {
            Ok(_) => {
                tracing::info!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "browser delete: row removed"
                );
                if let Err(e) = self.library.refresh_components(&library_path) {
                    tracing::warn!(
                        target: "signex::library",
                        path = %library_path.display(),
                        error = %e,
                        "browser delete: refresh_components failed"
                    );
                }
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    if state.selected_row == Some(row_id) {
                        state.selected_row = None;
                    }
                    state.delete_confirm = None;
                    // Drop any cached cell-edit buffers for the gone row.
                    state.cell_edit.retain(|(rid, _), _| *rid != row_id);
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    error = %e,
                    "browser delete: delete_row failed"
                );
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.delete_confirm = None;
                }
            }
        }
        Task::none()
    }

    /// Open the Edit Component Details modal for a row. Loads the row
    /// from the library cache and seeds the modal with a working copy.
    fn handle_browser_open_edit_modal(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let row = self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .cloned();
        let Some(row) = row else {
            tracing::warn!(
                target: "signex::library",
                path = %library_path.display(),
                table = %table,
                row = %row_id,
                "browser open edit modal: row not found in cache"
            );
            return Task::none();
        };
        let address = EditorAddress::new(library_path.clone(), table, row_id);
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.edit_modal = Some(EditRowModalState::new(address, row));
        }
        Task::none()
    }

    /// Apply a `BrowserEditMsg` to the active edit modal for `library_path`.
    fn handle_browser_edit_msg(
        &mut self,
        library_path: std::path::PathBuf,
        msg: BrowserEditMsg,
    ) -> Task<Message> {
        // Some variants need to fire follow-up tasks (open picker,
        // close modal). We collect those into `next` and return them
        // after releasing the borrow.
        let mut next: Option<Task<Message>> = None;
        // Save needs a separate path — we read the draft, drop the
        // borrow, run the adapter call, then resume.
        let mut save_request: Option<(EditorAddress, signex_library::ComponentRow)> = None;
        let mut close_modal = false;
        if let Some(state) = self.library.library_browsers.get_mut(&library_path)
            && let Some(modal) = state.edit_modal.as_mut()
        {
            match msg {
                BrowserEditMsg::SetInternalPn(s) => {
                    modal.draft.internal_pn = signex_library::InternalPn::new(s);
                    modal.error = None;
                }
                BrowserEditMsg::SetClass(class) => {
                    modal.draft.class = class;
                    modal.error = None;
                }
                BrowserEditMsg::SetState(state_v) => {
                    modal.draft.state = state_v;
                    modal.error = None;
                }
                BrowserEditMsg::SetDatasheetUrl(s) => {
                    modal.draft.datasheet = signex_library::DatasheetRef::url(s);
                    modal.error = None;
                }
                BrowserEditMsg::SetManufacturer(s) => {
                    modal.draft.primary_mpn.manufacturer = s;
                    modal.error = None;
                }
                BrowserEditMsg::SetMpn(s) => {
                    modal.draft.primary_mpn.mpn = s;
                    modal.error = None;
                }
                BrowserEditMsg::SetParamValue { key, value } => {
                    let entry = modal
                        .param_buf
                        .entry(key)
                        .or_insert_with(|| (String::new(), String::new()));
                    entry.0 = value;
                }
                BrowserEditMsg::SetParamUnit { key, unit } => {
                    let entry = modal
                        .param_buf
                        .entry(key)
                        .or_insert_with(|| (String::new(), String::new()));
                    entry.1 = unit;
                }
                BrowserEditMsg::CommitParam { key } => {
                    if let Some((value, unit)) = modal.param_buf.get(&key).cloned() {
                        let pv = if !unit.trim().is_empty() {
                            // Try parse as f64 first, otherwise store as text.
                            value
                                .parse::<f64>()
                                .ok()
                                .map(|n| signex_library::ParamValue::Measurement {
                                    value: n,
                                    unit: unit.clone(),
                                })
                                .unwrap_or_else(|| {
                                    signex_library::ParamValue::Text(format!("{value} {unit}"))
                                })
                        } else if let Ok(n) = value.parse::<f64>() {
                            signex_library::ParamValue::Number(n)
                        } else if value.eq_ignore_ascii_case("true") {
                            signex_library::ParamValue::Bool(true)
                        } else if value.eq_ignore_ascii_case("false") {
                            signex_library::ParamValue::Bool(false)
                        } else {
                            signex_library::ParamValue::Text(value)
                        };
                        modal.draft.parameters.insert(key, pv);
                    }
                }
                BrowserEditMsg::AddParam => {
                    // Find a unique key like "param_N".
                    let mut idx = modal.draft.parameters.len() + 1;
                    let key = loop {
                        let candidate = format!("param_{idx}");
                        if !modal.draft.parameters.contains_key(&candidate) {
                            break candidate;
                        }
                        idx += 1;
                    };
                    modal
                        .draft
                        .parameters
                        .insert(key.clone(), signex_library::ParamValue::Text(String::new()));
                    modal.param_buf.insert(key, (String::new(), String::new()));
                }
                BrowserEditMsg::DeleteParam { key } => {
                    modal.draft.parameters.remove(&key);
                    modal.param_buf.remove(&key);
                }
                BrowserEditMsg::SetTags(s) => {
                    modal.tags_buf = s;
                    modal.error = None;
                }
                BrowserEditMsg::OpenSymbolPicker => {
                    next = Some(Task::done(Message::Library(
                        LibraryMessage::OpenPrimitivePicker {
                            kind: PrimitiveKind::Symbol,
                            target: PrimitivePickerTarget::EditRowModal(modal.address.clone()),
                        },
                    )));
                }
                BrowserEditMsg::OpenFootprintPicker => {
                    next = Some(Task::done(Message::Library(
                        LibraryMessage::OpenPrimitivePicker {
                            kind: PrimitiveKind::Footprint,
                            target: PrimitivePickerTarget::EditRowModal(modal.address.clone()),
                        },
                    )));
                }
                BrowserEditMsg::Save => {
                    // Flush the tags buffer to `parameters["tags"]`
                    // before snapshotting the draft. Empty buffer drops
                    // the entry so we don't keep a dangling empty
                    // string in the param map.
                    let trimmed = modal.tags_buf.trim();
                    if trimmed.is_empty() {
                        modal.draft.parameters.remove("tags");
                    } else {
                        modal.draft.parameters.insert(
                            "tags".to_string(),
                            signex_library::ParamValue::Text(trimmed.to_string()),
                        );
                    }
                    save_request = Some((modal.address.clone(), modal.draft.clone()));
                }
                BrowserEditMsg::Cancel => {
                    close_modal = true;
                }
            }
        }
        if close_modal && let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.edit_modal = None;
        }
        if let Some((address, mut draft)) = save_request {
            // Refresh content_hash before saving.
            match signex_library::hash_row_content(&draft) {
                Ok(h) => {
                    draft.content_hash = h;
                }
                Err(e) => {
                    if let Some(state) = self.library.library_browsers.get_mut(&library_path)
                        && let Some(modal) = state.edit_modal.as_mut()
                    {
                        modal.error = Some(format!("hash failed: {e}"));
                    }
                    return next.unwrap_or_else(Task::none);
                }
            }
            let library_id = self
                .library
                .library_at(&address.library_path)
                .map(|lib| lib.library_id);
            let result = match library_id.and_then(|id| self.library.set.get(id)) {
                Some(adapter) => adapter.update_row(&address.table, draft, "edit row"),
                None => Err(signex_library::LibraryError::NotFound(
                    address.library_path.display().to_string(),
                )),
            };
            match result {
                Ok(_) => {
                    if let Err(e) = self.library.refresh_components(&address.library_path) {
                        tracing::warn!(
                            target: "signex::library",
                            path = %address.library_path.display(),
                            error = %e,
                            "browser edit: refresh_components failed"
                        );
                    }
                    if let Some(state) =
                        self.library.library_browsers.get_mut(&address.library_path)
                    {
                        state.edit_modal = None;
                    }
                }
                Err(e) => {
                    if let Some(state) = self.library.library_browsers.get_mut(&library_path)
                        && let Some(modal) = state.edit_modal.as_mut()
                    {
                        modal.error = Some(e.to_string());
                    }
                }
            }
        }
        next.unwrap_or_else(Task::none)
    }

    /// Commit a per-cell inline edit to the row. Re-hashes + persists.
    fn handle_browser_cell_commit(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
        column: String,
    ) -> Task<Message> {
        // Drop the buffer eagerly — if the save fails we re-insert below.
        let buf = match self
            .library
            .library_browsers
            .get_mut(&library_path)
            .and_then(|s| s.cell_edit.remove(&(row_id, column.clone())))
        {
            Some(v) => v,
            None => return Task::none(),
        };
        // Read the current row from the cache, mutate, re-hash, save.
        let mut row = match self
            .library
            .library_at(&library_path)
            .and_then(|lib| lib.tables.get(&table))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .cloned()
        {
            Some(r) => r,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "browser cell commit: row not found in cache"
                );
                return Task::none();
            }
        };
        match column.as_str() {
            "internal_pn" => {
                row.internal_pn = signex_library::InternalPn::new(buf.clone());
            }
            "manufacturer" => {
                row.primary_mpn.manufacturer = buf.clone();
            }
            "mpn" => {
                row.primary_mpn.mpn = buf.clone();
            }
            other if other.starts_with("parameters.") => {
                let key = &other["parameters.".len()..];
                // Preserve unit on commit by reading the existing value.
                let new_value = match row.parameters.get(key) {
                    Some(signex_library::ParamValue::Measurement { unit, .. }) => {
                        match buf.parse::<f64>() {
                            Ok(n) => signex_library::ParamValue::Measurement {
                                value: n,
                                unit: unit.clone(),
                            },
                            Err(_) => signex_library::ParamValue::Text(buf.clone()),
                        }
                    }
                    Some(signex_library::ParamValue::Number(_)) => match buf.parse::<f64>() {
                        Ok(n) => signex_library::ParamValue::Number(n),
                        Err(_) => signex_library::ParamValue::Text(buf.clone()),
                    },
                    Some(signex_library::ParamValue::Bool(_)) => {
                        if buf.eq_ignore_ascii_case("true") {
                            signex_library::ParamValue::Bool(true)
                        } else if buf.eq_ignore_ascii_case("false") {
                            signex_library::ParamValue::Bool(false)
                        } else {
                            signex_library::ParamValue::Text(buf.clone())
                        }
                    }
                    _ => signex_library::ParamValue::Text(buf.clone()),
                };
                row.parameters.insert(key.to_string(), new_value);
            }
            _ => {
                tracing::warn!(
                    target: "signex::library",
                    column = %column,
                    "browser cell commit: unknown column"
                );
                return Task::none();
            }
        }
        match signex_library::hash_row_content(&row) {
            Ok(h) => row.content_hash = h,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    error = %e,
                    "browser cell commit: hash failed; reverting buffer"
                );
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.cell_edit.insert((row_id, column), buf);
                }
                return Task::none();
            }
        }
        let library_id = self
            .library
            .library_at(&library_path)
            .map(|lib| lib.library_id);
        let result = match library_id.and_then(|id| self.library.set.get(id)) {
            Some(adapter) => adapter.update_row(&table, row, "edit cell"),
            None => Err(signex_library::LibraryError::NotFound(
                library_path.display().to_string(),
            )),
        };
        if let Err(e) = result {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "browser cell commit: update_row failed"
            );
            if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                state.cell_edit.insert((row_id, column), buf);
            }
            return Task::none();
        }
        if let Err(e) = self.library.refresh_components(&library_path) {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "browser cell commit: refresh_components failed"
            );
        }
        Task::none()
    }

    /// Apply a primitive picker sub-message. Most variants close the
    /// modal once the pick lands.
    fn handle_primitive_picker_msg(&mut self, msg: PrimitivePickerMsg) -> Task<Message> {
        match msg {
            PrimitivePickerMsg::SetFilter(s) => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.filter = s;
                    picker.error = None;
                }
                Task::none()
            }
            PrimitivePickerMsg::Cancel => {
                self.library.primitive_picker = None;
                Task::none()
            }
            PrimitivePickerMsg::Pick(primitive_ref) => self.apply_primitive_pick(primitive_ref),
            PrimitivePickerMsg::Browse => {
                let kind = self
                    .library
                    .primitive_picker
                    .as_ref()
                    .map(|p| p.kind)
                    .unwrap_or(PrimitiveKind::Symbol);
                let (label, ext) = match kind {
                    PrimitiveKind::Symbol => ("Pick Symbol (*.snxsym)", "snxsym"),
                    PrimitiveKind::Footprint => ("Pick Footprint (*.snxfpt)", "snxfpt"),
                    PrimitiveKind::Sim => ("Pick Sim Model (*.snxsim)", "snxsim"),
                    _ => ("Pick Primitive", ""),
                };
                Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_title(label)
                            .add_filter(ext, &[ext])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    |path| {
                        Message::Library(LibraryMessage::PrimitivePicker(
                            PrimitivePickerMsg::BrowseResult(path),
                        ))
                    },
                )
            }
            PrimitivePickerMsg::BrowseResult(None) => Task::none(),
            PrimitivePickerMsg::BrowseResult(Some(path)) => {
                self.handle_primitive_picker_browse_result(path)
            }
        }
    }

    /// A primitive ref has been picked — apply it to the picker's
    /// configured target and close the modal.
    fn apply_primitive_pick(&mut self, primitive_ref: PrimitiveRef) -> Task<Message> {
        let Some(picker) = self.library.primitive_picker.take() else {
            return Task::none();
        };
        match picker.target {
            PrimitivePickerTarget::PreviewRow(address) => {
                self.apply_primitive_pick_to_preview(address, picker.kind, primitive_ref);
            }
            PrimitivePickerTarget::EditRowModal(address) => {
                if let Some(state) = self.library.library_browsers.get_mut(&address.library_path)
                    && let Some(modal) = state.edit_modal.as_mut()
                    && modal.address == address
                {
                    match picker.kind {
                        PrimitiveKind::Symbol => {
                            modal.draft.symbol_ref = primitive_ref;
                        }
                        PrimitiveKind::Footprint => {
                            modal.draft.footprint_ref = Some(primitive_ref);
                        }
                        PrimitiveKind::Sim => {
                            modal.draft.sim_ref = Some(primitive_ref);
                        }
                        _ => {}
                    }
                    modal.error = None;
                }
            }
            PrimitivePickerTarget::NewComponentForm => {
                if let Some(nc) = self.library.new_component.as_mut() {
                    match picker.kind {
                        PrimitiveKind::Symbol => {
                            nc.symbol_ref = Some(primitive_ref);
                        }
                        PrimitiveKind::Footprint => {
                            nc.footprint_ref = Some(primitive_ref);
                        }
                        PrimitiveKind::Sim => { /* nothing today */ }
                        _ => {}
                    }
                    nc.error = None;
                }
            }
            PrimitivePickerTarget::BrowserRow(address) => {
                self.apply_primitive_pick_to_browser_row(address, picker.kind, primitive_ref);
            }
        }
        Task::none()
    }

    /// F15 — Library Browser row binding. Same shape as
    /// `apply_primitive_pick_to_preview` but reads/writes the row
    /// through the cache directly because there's no Component
    /// Preview tab open (the user picked from the inline preview /
    /// Properties area). Updates the row, re-hashes, persists via
    /// `adapter.update_row`, refreshes the cache.
    fn apply_primitive_pick_to_browser_row(
        &mut self,
        address: EditorAddress,
        kind: PrimitiveKind,
        primitive_ref: PrimitiveRef,
    ) {
        // 1. Read the row from the library cache.
        let mut row = match self
            .library
            .library_at(&address.library_path)
            .and_then(|lib| lib.tables.get(&address.table))
            .and_then(|rows| {
                rows.iter()
                    .find(|r| RowId::from_uuid(r.row_id) == address.row_id)
            })
            .cloned()
        {
            Some(r) => r,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    library = %address.library_path.display(),
                    table = %address.table,
                    row_id = %address.row_id,
                    "primitive pick: row not found in cache"
                );
                return;
            }
        };
        // 2. Apply.
        match kind {
            PrimitiveKind::Symbol => row.symbol_ref = primitive_ref,
            PrimitiveKind::Footprint => row.footprint_ref = Some(primitive_ref),
            PrimitiveKind::Sim => row.sim_ref = Some(primitive_ref),
            _ => return,
        }
        // 3. Re-hash.
        match signex_library::hash_row_content(&row) {
            Ok(h) => row.content_hash = h,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    error = %e,
                    "browser-row primitive pick: hash failed"
                );
                return;
            }
        }
        // 4. Persist via adapter.
        let library_id = self
            .library
            .library_at(&address.library_path)
            .map(|lib| lib.library_id);
        let commit_msg = match kind {
            PrimitiveKind::Symbol => "bind symbol",
            PrimitiveKind::Footprint => "bind footprint",
            PrimitiveKind::Sim => "bind sim",
            _ => "bind primitive",
        };
        let result = match library_id.and_then(|id| self.library.set.get(id)) {
            Some(adapter) => adapter.update_row(&address.table, row, commit_msg),
            None => Err(signex_library::LibraryError::NotFound(
                address.library_path.display().to_string(),
            )),
        };
        if let Err(e) = result {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "browser-row primitive pick: update_row failed"
            );
            return;
        }
        // 5. Refresh cache.
        if let Err(e) = self.library.refresh_components(&address.library_path) {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "browser-row primitive pick: refresh_components failed"
            );
        }
    }

    /// Component Preview tab — apply a freshly-picked primitive ref to
    /// the row, resolve through the LibrarySet, save via update_row.
    fn apply_primitive_pick_to_preview(
        &mut self,
        address: EditorAddress,
        kind: PrimitiveKind,
        primitive_ref: PrimitiveRef,
    ) {
        let Some(state) = self.library.editors.get_mut(&address) else {
            return;
        };
        match kind {
            PrimitiveKind::Symbol => {
                state.row.symbol_ref = primitive_ref;
                state.symbol = self.library.set.resolve_symbol(&primitive_ref);
            }
            PrimitiveKind::Footprint => {
                state.row.footprint_ref = Some(primitive_ref);
                state.footprint = self.library.set.resolve_footprint(&primitive_ref);
            }
            PrimitiveKind::Sim => {
                state.row.sim_ref = Some(primitive_ref);
                state.sim = self.library.set.resolve_sim(&primitive_ref);
            }
            _ => return,
        }
        // Refresh content_hash + save.
        let mut row = state.row.clone();
        match signex_library::hash_row_content(&row) {
            Ok(h) => {
                row.content_hash = h;
                state.row.content_hash = h;
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    error = %e,
                    "primitive pick: hash failed"
                );
                return;
            }
        }
        let library_id = self
            .library
            .library_at(&address.library_path)
            .map(|lib| lib.library_id);
        let msg = match kind {
            PrimitiveKind::Symbol => "bind symbol",
            PrimitiveKind::Footprint => "bind footprint",
            PrimitiveKind::Sim => "bind sim",
            _ => "bind primitive",
        };
        let result = match library_id.and_then(|id| self.library.set.get(id)) {
            Some(adapter) => adapter.update_row(&address.table, row, msg),
            None => Err(signex_library::LibraryError::NotFound(
                address.library_path.display().to_string(),
            )),
        };
        if let Err(e) = result {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "primitive pick: update_row failed"
            );
            return;
        }
        if let Err(e) = self.library.refresh_components(&address.library_path) {
            tracing::warn!(
                target: "signex::library",
                error = %e,
                "primitive pick: refresh_components failed"
            );
        }
    }

    /// Filesystem-picked primitive — auto-mount the containing
    /// `.snxlib`, then synthesize a Pick.
    fn handle_primitive_picker_browse_result(&mut self, file: std::path::PathBuf) -> Task<Message> {
        // Locate the containing `.snxlib`. Path layout is
        // `<some>/<lib>.snxlib/<symbols|footprints|sims>/<uuid>.<ext>`.
        let snxlib_dir = file
            .ancestors()
            .find(|p| {
                p.extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("snxlib"))
                    .unwrap_or(false)
            })
            .map(|p| p.to_path_buf());
        let Some(snxlib_dir) = snxlib_dir else {
            if let Some(picker) = self.library.primitive_picker.as_mut() {
                picker.error = Some(
                    "Picked file is not inside a `.snxlib` library. v0.9 only supports primitives bound through libraries."
                        .into(),
                );
            }
            return Task::none();
        };
        // Mount the library if not already.
        if let Err(e) = commands::open_library(&mut self.library, snxlib_dir.clone()) {
            tracing::warn!(
                target: "signex::library",
                path = %snxlib_dir.display(),
                error = %e,
                "browse-pick: open_library failed"
            );
            if let Some(picker) = self.library.primitive_picker.as_mut() {
                picker.error = Some(format!("open library failed: {e}"));
            }
            return Task::none();
        }
        // Resolve library_id + parse uuid from filename.
        let library_id = match self.library.library_at(&snxlib_dir) {
            Some(lib) => lib.library_id,
            None => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.error = Some("Library failed to mount.".into());
                }
                return Task::none();
            }
        };
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let uuid = match uuid::Uuid::parse_str(stem) {
            Ok(u) => u,
            Err(_) => {
                if let Some(picker) = self.library.primitive_picker.as_mut() {
                    picker.error = Some(format!(
                        "Filename `{stem}` is not a UUID — pick a primitive file in `<lib>.snxlib/symbols/`."
                    ));
                }
                return Task::none();
            }
        };
        let primitive_ref = PrimitiveRef::new(library_id, uuid);
        Task::done(Message::Library(LibraryMessage::PrimitivePicker(
            PrimitivePickerMsg::Pick(primitive_ref),
        )))
    }

    /// Spawn the "New Component Library" Save-As dialog for the
    /// project rooted at `project_root`. The dialog defaults to
    /// `<project_dir>/<project>-lib.snxlib` so the common
    /// project-local case is one Enter key, but the user can navigate
    /// to a global directory to create a shared library. On confirm,
    /// the dialog dispatches `CreateLibraryAtPath` which calls
    /// `commands::create_library_at` to do the actual disk + manifest
    /// + git init.
    ///
    /// We deliberately do NOT touch disk here — the previous "instant
    /// create on click" behaviour was confusing because users
    /// couldn't see where it was going to land or override the
    /// default name.
    fn handle_create_library_for_project(
        &mut self,
        project_root: std::path::PathBuf,
    ) -> Task<Message> {
        // Locate the LoadedProject so we can derive the suggested
        // path. The dispatch handler that consumes the dialog result
        // re-resolves the project at apply time so a project unload
        // between dialog spawn + confirm is recoverable.
        let Some(loaded) =
            self.document_state.projects.iter().find(|p| {
                p.path == project_root || p.path.parent() == Some(project_root.as_path())
            })
        else {
            tracing::warn!(
                target: "signex::library",
                path = %project_root.display(),
                "create library: no loaded project matches root"
            );
            return Task::none();
        };

        let stem = loaded
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("project");
        let mut name = format!("{stem}-lib");

        let project_dir = std::path::PathBuf::from(&loaded.data.dir);
        // Pre-disambiguate the default name so the user doesn't get a
        // misleading "<project>-lib" suggestion when that path already
        // exists. Conflicts get a `-2`, `-3`, … suffix matching the
        // pattern `commands::create_library` previously used.
        if project_dir.join(format!("{name}.snxlib")).exists() {
            for n in 2..=99 {
                let candidate = format!("{stem}-lib-{n}");
                if !project_dir.join(format!("{candidate}.snxlib")).exists() {
                    name = candidate;
                    break;
                }
            }
        }
        let suggested_filename = format!("{name}.snxlib");
        let project_path = loaded.path.clone();

        Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_title("New Component Library")
                    .add_filter("Signex Component Library", &["snxlib"])
                    .set_directory(&project_dir)
                    .set_file_name(&suggested_filename)
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            move |picked| match picked {
                Some(lib_path) => Message::Library(LibraryMessage::CreateLibraryAtPath {
                    project_path: project_path.clone(),
                    lib_path,
                }),
                None => Message::Noop,
            },
        )
    }

    /// Resolution of the "Library Options" modal (Stage 11 of
    /// `v0.9-snxlib-as-file-plan.md`). Re-resolves the project (in
    /// case it was unloaded between modal spawn + confirm), then
    /// **registers** a pending library — no disk writes here. The
    /// actual `.snxlib/` directory + manifest + git scaffolding land
    /// at project-save time via
    /// `commands::materialize_pending_library`, called from
    /// `save_active_project_if_dirty`. Closes
    /// `feedback_no_disk_writes_without_user_save.md`'s "wait for
    /// explicit user save" invariant. `use_lfs` carries the modal's
    /// checkbox state — when on, the eventual adapter writes
    /// `.gitattributes` for `*.step` / `*.stp` / `*.wrl` / `*.iges`
    /// and stages it into the initial commit.
    fn handle_create_library_at_path(
        &mut self,
        project_path: std::path::PathBuf,
        lib_path: std::path::PathBuf,
        enable_git: bool,
        use_lfs: bool,
    ) -> Task<Message> {
        let Some(loaded) = self
            .document_state
            .projects
            .iter_mut()
            .find(|p| p.path == project_path)
        else {
            tracing::warn!(
                target: "signex::library",
                path = %project_path.display(),
                "create library: project unloaded between dialog spawn and confirm"
            );
            return Task::none();
        };

        match crate::library::commands::register_pending_library(
            lib_path.clone(),
            enable_git,
            use_lfs,
        ) {
            Ok((library_id, spec)) => {
                loaded.pending_libraries.insert(library_id, spec);
                tracing::info!(
                    target: "signex::library",
                    project = %loaded.path.display(),
                    library = %lib_path.display(),
                    library_id = %library_id,
                    use_lfs,
                    "registered pending library — disk write deferred to project save"
                );
                // Mark the project file dirty so the user is prompted
                // to persist the new library entry in the `.snxprj`.
                let project_path = loaded.path.clone();
                self.document_state.dirty_paths.insert(project_path);
            }
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    project = %loaded.path.display(),
                    library = %lib_path.display(),
                    use_lfs,
                    error = %e,
                    "register_pending_library failed (path validation)"
                );
            }
        }

        self.refresh_panel_ctx();
        Task::none()
    }

    /// Open the Component Preview tab for `(library_path, table, row_id)`.
    /// Re-uses the existing tab if one is already open.
    fn handle_open_component_row(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let address = EditorAddress::new(library_path.clone(), table.clone(), row_id);
        let synthetic_path = address.synthetic_tab_path();

        if let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|t| t.path == synthetic_path)
        {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return Task::none();
        }

        // Pre-load the row from the adapter via `read_row`; if it
        // fails we surface and bail without leaving an empty tab
        // behind.
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => {
                tracing::warn!(
                    target: "signex::library",
                    path = %library_path.display(),
                    "open component row: library not open"
                );
                return Task::none();
            }
        };
        let row_result = self
            .library
            .set
            .get(library_id)
            .ok_or_else(|| {
                signex_library::LibraryError::NotFound(library_path.display().to_string())
            })
            .and_then(|adapter| adapter.read_row(&table, row_id));
        let row = match row_result {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(target: "signex::library", error = %e, "open component row: read_row failed");
                return Task::none();
            }
        };

        let title = row.internal_pn.as_str().to_string();
        let project_id = self
            .document_state
            .project_for_path(&synthetic_path)
            .map(|p| p.id);
        let preview = ComponentPreviewState::from_row(library_path.clone(), table.clone(), row);
        self.library.editors.insert(address.clone(), preview);
        self.park_active_schematic_session();
        self.document_state.tabs.push(crate::app::TabInfo {
            title,
            path: synthetic_path,
            cached_document: None,
            dirty: false,
            project_id,
            kind: crate::app::TabKind::ComponentEditor(crate::app::ComponentEditorTab {
                library_path: address.library_path.clone(),
                table: address.table.clone(),
                row_id: address.row_id,
            }),
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.document_state.active_path = None;
        self.refresh_panel_ctx();
        Task::none()
    }

    fn handle_picker_message(&mut self, msg: PickerMsg) -> Task<Message> {
        let Some(picker) = self.library.picker.as_mut() else {
            return Task::none();
        };
        match msg {
            PickerMsg::FilterChanged(s) => {
                picker.filter = s;
            }
            PickerMsg::SelectComponent(summary) => {
                // `ComponentSummary` carries `row_id` directly in the
                // DBLib model; match against that.
                let path = self
                    .library
                    .open_libraries
                    .iter()
                    .find(|lib| {
                        lib.cached_components
                            .iter()
                            .any(|c| c.row_id == summary.row_id)
                    })
                    .map(|lib| lib.root.clone());
                picker.selected = path.map(|p| (p, summary));
            }
            PickerMsg::PlaceSelected => {
                if let Some((path, summary)) = picker.selected.clone() {
                    tracing::warn!(
                        target: "signex::library",
                        library = %path.display(),
                        internal_pn = %summary.internal_pn.as_str(),
                        "place flow shipped in Phase 2 — picker dismissed"
                    );
                }
                self.library.picker = None;
            }
        }
        Task::none()
    }

    fn handle_library_settings_message(&mut self, msg: SettingsMsg) -> Task<Message> {
        use crate::library::settings::{digikey_oauth, persistence};
        use signex_library::distributor::DistributorAdapter;
        use signex_library::distributors::digikey::{DIGIKEY_AUTH_URL, DIGIKEY_TOKEN_URL};
        use signex_library::distributors::keyring::KeyringStore;
        use signex_library::distributors::mouser::MouserAdapter;

        match msg {
            SettingsMsg::DigiKeyConnect => {
                if self.library.settings.digikey_in_flight {
                    return Task::none();
                }
                // Bump the generation BEFORE spawning so the worker
                // captures the current value. `DigiKeyOAuthResult`
                // discards messages whose generation is stale — i.e.
                // belonged to a cancelled flow that's only now winding
                // down. Without this, Cancel + reconnect lets the
                // first worker's outcome stomp on the second flow's
                // state.
                self.library.settings.digikey_flow_generation = self
                    .library
                    .settings
                    .digikey_flow_generation
                    .wrapping_add(1);
                let generation = self.library.settings.digikey_flow_generation;
                self.library.settings.digikey_in_flight = true;
                self.library.settings.digikey_status = Some("Waiting for browser…".to_string());
                let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                self.library.settings.digikey_cancel = Some(cancel_flag.clone());
                let (client_id, client_secret) = digikey_oauth::read_env_credentials();
                let auth_url = DIGIKEY_AUTH_URL.to_string();
                let token_url = DIGIKEY_TOKEN_URL.to_string();
                return Task::perform(
                    async move {
                        let cancel = digikey_oauth::CancelHandle::from_flag(cancel_flag);
                        tokio::task::spawn_blocking(move || {
                            digikey_oauth::run_blocking(
                                client_id,
                                client_secret,
                                auth_url,
                                token_url,
                                cancel,
                                true,
                            )
                        })
                        .await
                        .unwrap_or(digikey_oauth::Outcome::Failed {
                            reason: "worker thread panicked".into(),
                        })
                    },
                    move |outcome| {
                        let (label, err) = match outcome {
                            digikey_oauth::Outcome::Connected { account_label } => {
                                (Some(account_label), None)
                            }
                            digikey_oauth::Outcome::Failed { reason } => (None, Some(reason)),
                            digikey_oauth::Outcome::Cancelled => (None, None),
                        };
                        Message::Library(LibraryMessage::Settings(
                            SettingsMsg::DigiKeyOAuthResult {
                                generation,
                                connected_label: label,
                                error: err,
                            },
                        ))
                    },
                );
            }
            SettingsMsg::DigiKeyCancel => {
                if let Some(flag) = self.library.settings.digikey_cancel.as_ref() {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                // Bump the generation so any in-flight worker's eventual
                // result is treated as stale by the result handler.
                // Then clear the in-flight flag — the user is now free
                // to start a fresh OAuth attempt without the old
                // worker's outcome leaking into the new flow.
                self.library.settings.digikey_flow_generation = self
                    .library
                    .settings
                    .digikey_flow_generation
                    .wrapping_add(1);
                self.library.settings.digikey_cancel = None;
                self.library.settings.digikey_in_flight = false;
                self.library.settings.digikey_status = Some("Cancelled".to_string());
            }
            SettingsMsg::DigiKeyOAuthResult {
                generation,
                connected_label,
                error,
            } => {
                // Drop stale results from a cancelled flow — see the
                // comment on `DigiKeyConnect` for why.
                if generation != self.library.settings.digikey_flow_generation {
                    return Task::none();
                }
                self.library.settings.digikey_in_flight = false;
                self.library.settings.digikey_cancel = None;
                match (connected_label, error) {
                    (Some(label), _) => {
                        self.library.settings.digikey_account_email = Some(label.clone());
                        self.library.settings.digikey_status =
                            Some(format!("Connected as {label}"));
                    }
                    (_, Some(reason)) => {
                        self.library.settings.digikey_status = Some(format!("Failed: {reason}"));
                    }
                    (None, None) => {
                        self.library.settings.digikey_status = Some("Cancelled".to_string());
                    }
                }
            }
            SettingsMsg::MouserApiKeyChanged(s) => {
                self.library.settings.mouser_api_key_buf = s;
            }
            SettingsMsg::MouserTest => {
                if self.library.settings.mouser_in_flight {
                    return Task::none();
                }
                let key = self.library.settings.mouser_api_key_buf.clone();
                if key.is_empty() {
                    self.library.settings.mouser_status =
                        Some("Cannot test — paste an API key first.".to_string());
                    return Task::none();
                }
                self.library.settings.mouser_in_flight = true;
                self.library.settings.mouser_status = Some("Testing…".to_string());
                let key_for_writeback = key.clone();
                return Task::perform(
                    async move {
                        let key_for_test = key.clone();
                        tokio::task::spawn_blocking(move || {
                            const SENTINEL_MPN: &str = "RC0805FR-0710KL";
                            let adapter = MouserAdapter::with_api_key(
                                "https://api.mouser.com/api/v1/search/keyword",
                                key_for_test,
                                None,
                            );
                            adapter
                                .lookup_by_mpn(SENTINEL_MPN)
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("worker thread panicked: {e}")))
                    },
                    move |result| {
                        let result = match result {
                            Ok(()) => {
                                // MD-17: surface keyring backend
                                // unavailability instead of panicking
                                // — the dialog will tell the user to
                                // install libsecret or run with the
                                // env-var auth flow.
                                match KeyringStore::for_provider("mouser", "default") {
                                    Ok(store) => {
                                        if let Err(e) = store.set_secret(&key_for_writeback) {
                                            Err(format!(
                                                "API key valid, but keyring write failed: {e}"
                                            ))
                                        } else {
                                            Ok(())
                                        }
                                    }
                                    Err(e) => Err(format!(
                                        "API key valid, but OS keychain unavailable: {e}"
                                    )),
                                }
                            }
                            Err(e) => Err(e),
                        };
                        Message::Library(LibraryMessage::Settings(SettingsMsg::MouserTestResult(
                            result,
                        )))
                    },
                );
            }
            SettingsMsg::MouserTestResult(result) => {
                self.library.settings.mouser_in_flight = false;
                self.library.settings.mouser_status = Some(match result {
                    Ok(()) => "\u{2713} Connected & key saved to keyring.".to_string(),
                    Err(e) => format!("Failed: {e}"),
                });
            }
            SettingsMsg::PreferenceUp(src) => {
                let order = &mut self.library.settings.preferred_order;
                if let Some(i) = order.iter().position(|s| *s == src)
                    && i > 0
                {
                    order.swap(i, i - 1);
                    persistence::save_preferred_order(order);
                }
            }
            SettingsMsg::PreferenceDown(src) => {
                let order = &mut self.library.settings.preferred_order;
                if let Some(i) = order.iter().position(|s| *s == src)
                    && i + 1 < order.len()
                {
                    order.swap(i, i + 1);
                    persistence::save_preferred_order(order);
                }
            }
        }
        Task::none()
    }

    /// Component Preview event handler.
    fn handle_editor_event(&mut self, address: EditorAddress, msg: EditorMsg) -> Task<Message> {
        match msg {
            EditorMsg::CloseEditor => {
                let synthetic = address.synthetic_tab_path();
                if let Some(idx) = self
                    .document_state
                    .tabs
                    .iter()
                    .position(|t| t.path == synthetic)
                {
                    return self.close_tab_now(idx);
                }
                self.library.editors.remove(&address);
                return Task::none();
            }
            EditorMsg::SaveDraft | EditorMsg::Commit => {
                self.handle_save_row(&address);
                return Task::none();
            }
            EditorMsg::SelectTab(tab) => {
                self.handle_select_preview_tab(&address, tab);
                return Task::none();
            }
            EditorMsg::OpenWhereUsedTab => {
                if let Some(state) = self.library.editors.get_mut(&address) {
                    state.active_tab = PreviewTab::Preview;
                }
                return Task::none();
            }
            // Submit-for-review is dropped from the Component Preview
            // surface in v0.9-refactor-2 — review workflows happen
            // outside the row context. The variants stay in the message
            // tree for potential future revival.
            EditorMsg::SubmitForReview
            | EditorMsg::SubmitForReviewNotesChanged(_)
            | EditorMsg::SubmitForReviewCancel
            | EditorMsg::SubmitForReviewConfirm
            | EditorMsg::SubmitForReviewResult(_) => {
                tracing::debug!(
                    target: "signex::library",
                    "submit-for-review is not wired in the Component Preview surface"
                );
                return Task::none();
            }
            EditorMsg::DatasheetUploadDialog => {
                let library_path = address.library_path.clone();
                let table = address.table.clone();
                let row_id = address.row_id;
                return Task::perform(
                    async {
                        let picked = rfd::AsyncFileDialog::new()
                            .set_title("Pin datasheet PDF")
                            .add_filter("PDF", &["pdf"])
                            .add_filter("All files", &["*"])
                            .pick_file()
                            .await;
                        match picked {
                            Some(handle) => {
                                let filename = handle.file_name();
                                let bytes = handle.read().await;
                                Some((bytes, filename))
                            }
                            None => None,
                        }
                    },
                    move |result| {
                        Message::Library(LibraryMessage::EditorEvent {
                            library_path: library_path.clone(),
                            table: table.clone(),
                            row_id,
                            msg: EditorMsg::DatasheetUploadResult(result),
                        })
                    },
                );
            }
            _ => {}
        }

        let Some(state) = self.library.editors.get_mut(&address) else {
            return Task::none();
        };
        apply_inline_edit(state, msg);
        Task::none()
    }

    fn handle_select_preview_tab(&mut self, address: &EditorAddress, tab: PreviewTab) {
        let Some(state) = self.library.editors.get_mut(address) else {
            return;
        };
        state.active_tab = tab;

        match tab {
            PreviewTab::Preview => {
                if state.symbol.is_none() {
                    let symbol_ref = state.row.symbol_ref;
                    let resolved = self.library.set.resolve_symbol(&symbol_ref);
                    if let Some(state) = self.library.editors.get_mut(address) {
                        state.symbol = resolved;
                    }
                }
                if let Some(state) = self.library.editors.get_mut(address)
                    && state.footprint.is_none()
                {
                    let resolved = state
                        .row
                        .footprint_ref
                        .as_ref()
                        .and_then(|r| self.library.set.resolve_footprint(r));
                    if let Some(state) = self.library.editors.get_mut(address) {
                        state.footprint = resolved;
                    }
                }
            }
            PreviewTab::Simulation => {
                if state.sim.is_none()
                    && let Some(sim_ref) = state.row.sim_ref.as_ref()
                {
                    let sim_ref = *sim_ref;
                    let resolved = self.library.set.resolve_sim(&sim_ref);
                    if let Some(state) = self.library.editors.get_mut(address) {
                        if let Some(sim) = resolved.as_ref() {
                            state.sim_body = Some(iced::widget::text_editor::Content::with_text(
                                sim.body.as_str(),
                            ));
                        }
                        state.sim = resolved;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_save_row(&mut self, address: &EditorAddress) {
        let Some(state) = self.library.editors.get(address) else {
            return;
        };
        let library_path = state.library_path.clone();
        let table = state.table.clone();
        let mut row = state.row.clone();
        row.updated = chrono::Utc::now();
        if let Err(e) = row.refresh_content_hash() {
            tracing::warn!(target: "signex::library", error = %e, "save row: refresh_content_hash failed");
        }

        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => {
                tracing::warn!(target: "signex::library", "save row: library not open");
                return;
            }
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => {
                tracing::warn!(target: "signex::library", "save row: library not mounted");
                return;
            }
        };
        match adapter.update_row(&table, row.clone(), "edit row (signex-app)") {
            Ok(()) => {
                if let Some(state) = self.library.editors.get_mut(address) {
                    state.row = row;
                    state.dirty = false;
                }
                if let Err(e) = self.library.refresh_components(&library_path) {
                    tracing::warn!(target: "signex::library", error = %e, "post-save refresh failed");
                }
            }
            Err(e) => {
                tracing::warn!(target: "signex::library", error = %e, "update_row failed");
            }
        }
    }

    /// F34 — Save-As dialog confirmed for a new symbol library file
    /// (`.snxsym`). The user picked the location + filename in the
    /// rfd `save_file()` dialog — that click IS the explicit save
    /// action, so we write the empty `SymbolFile` to disk
    /// immediately, register the path on the containing project's
    /// `data.libraries` list (so the tree shows it directly under
    /// Libraries), then open it as a clean primitive editor tab
    /// (dirty=false). Subsequent edits flow through the regular
    /// `Ctrl+S → save_primitive_tab_at` path.
    pub(crate) fn handle_add_library_symbol_file_picked(
        &mut self,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("NewSymbol")
            .to_string();
        let symbol = signex_library::Symbol::empty(stem);
        let file = signex_library::SymbolFile::from_symbol(symbol);
        // v0.18.4 — emit TOML envelope (mirror of v0.18.2 .snxfpt).
        let text = match file.to_toml_string() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "AddLibrarySymbolFilePicked: serialize failed"
                );
                return Task::none();
            }
        };
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(
                target: "signex::library",
                parent = %parent.display(),
                error = %e,
                "AddLibrarySymbolFilePicked: create symbols dir failed"
            );
            return Task::none();
        }
        if let Err(e) = std::fs::write(&path, text.as_bytes()) {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "AddLibrarySymbolFilePicked: write .snxsym failed"
            );
            return Task::none();
        }
        self.register_standalone_library_on_project(&path);
        self.handle_open_primitive(path)
    }

    /// F34 — Footprint counterpart to
    /// [`handle_add_library_symbol_file_picked`]. Writes an empty
    /// `FootprintFile` (TOML+TSV envelope), registers the file as a
    /// project library entry, opens the file as a clean primitive
    /// editor tab.
    pub(crate) fn handle_add_library_footprint_file_picked(
        &mut self,
        path: std::path::PathBuf,
    ) -> Task<Message> {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("NewFootprint")
            .to_string();
        let footprint = signex_library::Footprint::empty(stem);
        // v0.18.4 — emit TOML+TSV envelope.
        let file = signex_library::FootprintFile::from_footprint(footprint);
        let text = match file.to_toml_string() {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "AddLibraryFootprintFilePicked: serialize failed"
                );
                return Task::none();
            }
        };
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(
                target: "signex::library",
                parent = %parent.display(),
                error = %e,
                "AddLibraryFootprintFilePicked: create footprints dir failed"
            );
            return Task::none();
        }
        if let Err(e) = std::fs::write(&path, text.as_bytes()) {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "AddLibraryFootprintFilePicked: write .snxfpt failed"
            );
            return Task::none();
        }
        self.register_standalone_library_on_project(&path);
        self.handle_open_primitive(path)
    }

    /// Find the project containing `path` and push a `LibraryEntry`
    /// for it onto `data.libraries` (project-local relative path when
    /// `path` is inside the project dir, absolute otherwise). Marks
    /// the project dirty + refreshes the panel context so the new
    /// entry shows immediately. No-op when the path is already
    /// registered, or when no loaded project owns the file's parent.
    fn register_standalone_library_on_project(&mut self, path: &std::path::Path) {
        use signex_types::project::{LibraryEntry, LibraryEntryKind};
        // Resolve the target project index first so the mutable borrow
        // of `projects` is short-lived (the active-project fallback
        // chained on iter_mut tripped E0500).
        let target_idx = self
            .document_state
            .projects
            .iter()
            .position(|p| {
                let project_dir = std::path::PathBuf::from(&p.data.dir);
                !project_dir.as_os_str().is_empty() && path.starts_with(&project_dir)
            })
            .or_else(|| {
                self.document_state
                    .active_project
                    .and_then(|id| self.document_state.projects.iter().position(|p| p.id == id))
            });
        let Some(idx) = target_idx else {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                "register_standalone_library: no project to attach to"
            );
            return;
        };
        let Some(loaded) = self.document_state.projects.get_mut(idx) else {
            return;
        };
        let project_dir = std::path::PathBuf::from(&loaded.data.dir);
        let (entry_path, entry_kind) = if !project_dir.as_os_str().is_empty()
            && let Ok(rel) = path.strip_prefix(&project_dir)
        {
            (rel.to_path_buf(), LibraryEntryKind::ProjectLocal)
        } else {
            (path.to_path_buf(), LibraryEntryKind::Shared)
        };
        // Skip if the same path is already on the list.
        if loaded
            .data
            .libraries
            .iter()
            .any(|e| loaded.data.resolve_library_path(e) == path)
        {
            return;
        }
        loaded.data.libraries.push(LibraryEntry {
            path: entry_path,
            kind: entry_kind,
            library_id: None,
        });
        let project_path = loaded.path.clone();
        self.document_state.dirty_paths.insert(project_path);
        self.refresh_panel_ctx();
    }

    /// Open a `.snxsym` or `.snxfpt` as a main-window document tab.
    /// Reads the file from disk, builds the matching editor state,
    /// and pushes a `TabKind::SymbolEditor(path)` /
    /// `FootprintEditor(path)` tab into `DocumentState.tabs`.
    ///
    /// Activates an existing tab when the same path is already open
    /// instead of duplicating; surfaces parse / IO failures via
    /// `tracing::warn` (and silently bails — leaving the tab bar
    /// untouched).
    pub(crate) fn handle_open_primitive(&mut self, path: std::path::PathBuf) -> Task<Message> {
        // Already open? Just activate the existing tab.
        if let Some(idx) = self.document_state.tabs.iter().position(|t| t.path == path) {
            if idx != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = idx;
                self.sync_active_tab();
            }
            return Task::none();
        }

        // Dispatch on extension. `.snxsym` → Symbol; `.snxfpt` →
        // Footprint. Anything else is rejected with a tracing warn so
        // a stray dispatch from the project tree doesn't push a
        // bogus tab.
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ext.as_str() {
            "snxsym" => {
                let bytes = match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: read .snxsym failed",
                        );
                        return Task::none();
                    }
                };
                // v0.18.4 — auto-detect TOML vs legacy JSON.
                let file = match signex_library::SymbolFile::from_bytes(&bytes) {
                    Ok(f) if !f.symbols.is_empty() => f,
                    Ok(_) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            "open primitive: .snxsym contains zero symbols",
                        );
                        return Task::none();
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: parse .snxsym failed",
                        );
                        return Task::none();
                    }
                };

                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| {
                        if !file.display_name.is_empty() {
                            file.display_name.clone()
                        } else {
                            file.symbols[0].name.clone()
                        }
                    });
                let project_id = self.document_state.project_for_path(&path).map(|p| p.id);

                let state = crate::app::SymbolEditorState::new(path.clone(), file);
                self.document_state
                    .symbol_editors
                    .insert(path.clone(), state);

                self.park_active_schematic_session();
                self.document_state.tabs.push(crate::app::TabInfo {
                    title,
                    path: path.clone(),
                    cached_document: None,
                    dirty: false,
                    project_id,
                    kind: crate::app::TabKind::SymbolEditor(path),
                });
                self.document_state.active_tab = self.document_state.tabs.len() - 1;
                // Standalone primitive tabs don't drive `active_path`
                // — clear so the canvas doesn't render a stale schematic.
                self.document_state.active_path = None;
                self.refresh_panel_ctx();
                Task::none()
            }
            "snxfpt" => {
                // v0.13.0 — footprint editor gated off for release.
                // A `.snxfpt` opened from the tree / file dialog must
                // not push an editable FootprintEditor tab. Read-only
                // preview + Pick-Footprint binding of existing files
                // stay available elsewhere. Flip
                // `feature_flags::FOOTPRINT_EDITOR_ENABLED` to re-enable.
                if !crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
                    tracing::info!(
                        target: "signex::library",
                        path = %path.display(),
                        "open primitive: footprint editor disabled (v0.13.0) — ignoring .snxfpt open",
                    );
                    return Task::none();
                }
                let bytes = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: read .snxfpt failed",
                        );
                        return Task::none();
                    }
                };
                // v0.18.4 — parse TOML+TSV envelope and use the first
                // footprint as the editor primitive. Multi-footprint
                // containers are not yet exposed in the editor UI.
                let file = match signex_library::FootprintFile::from_toml_str(&bytes) {
                    Ok(f) if !f.footprints.is_empty() => f,
                    Ok(_) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            "open primitive: .snxfpt contains zero footprints",
                        );
                        return Task::none();
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "signex::library",
                            path = %path.display(),
                            error = %e,
                            "open primitive: parse .snxfpt failed",
                        );
                        return Task::none();
                    }
                };
                // v0.18.6 — keep the FootprintFile envelope around so
                // saves preserve `file_uuid` + any future multi-
                // footprint siblings instead of minting a fresh
                // single-footprint container each time.
                let display_name = file.footprints[0].name.clone();

                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(display_name);
                let project_id = self.document_state.project_for_path(&path).map(|p| p.id);

                // HI-23: seed snap-disabled from the user's persisted
                // global toggle so opening a .snxfpt while snap is off
                // doesn't reset the editor to snap-on.
                let state = crate::app::FootprintEditorState::new(path.clone(), file)
                    .with_global_snap_disabled(!self.ui_state.snap_enabled);
                self.document_state
                    .footprint_editors
                    .insert(path.clone(), state);

                self.park_active_schematic_session();
                self.document_state.tabs.push(crate::app::TabInfo {
                    title,
                    path: path.clone(),
                    cached_document: None,
                    dirty: false,
                    project_id,
                    kind: crate::app::TabKind::FootprintEditor(path),
                });
                self.document_state.active_tab = self.document_state.tabs.len() - 1;
                self.document_state.active_path = None;
                self.refresh_panel_ctx();
                Task::none()
            }
            other => {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    ext = %other,
                    "open primitive: unsupported extension",
                );
                Task::none()
            }
        }
    }

    /// Apply a primitive-editor inner message to the matching tab's
    /// editor state. Path-keyed lookup distinguishes Symbol vs
    /// Footprint; the dispatcher routes to the existing canvas-state
    /// helpers so the standalone tab behaviour matches the in-Component
    /// Editor experience verbatim.
    pub(crate) fn handle_primitive_editor_event(
        &mut self,
        path: std::path::PathBuf,
        msg: PrimitiveEditorMsg,
    ) -> Task<Message> {
        // Save is a sibling of the canvas-mutation messages — route
        // through the standalone save path which writes JSON back to
        // disk and (when applicable) reloads in the LibrarySet. When
        // the file doesn't exist on disk yet (newly-minted in-memory
        // tab from `Add New ▸ Symbol` / `Add New ▸ Footprint`), spawn
        // the Save-As dialog instead so the user picks where it lands
        // — same gate as the top-level `Message::SaveFile` path uses.
        if matches!(msg, PrimitiveEditorMsg::Save) {
            if !path.exists() {
                return crate::app::handlers::document_files::spawn_save_as_for_new_primitive(path);
            }
            self.save_primitive_tab_at(&path);
            return Task::none();
        }

        // Per-library display settings (sheet color, grid, unit)
        // mutate `OpenLibrary.display` rather than the per-tab editor
        // state — every primitive editor opened from the same
        // `.snxlib` shares the same view settings (Altium "Document
        // Options" parity). Run these before the editor-level
        // dispatch so the editor closure doesn't see them.
        match &msg {
            PrimitiveEditorMsg::SymbolSetSheetColor(color) => {
                let color = *color;
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    lib.display.sheet_color = color;
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            PrimitiveEditorMsg::SymbolToggleGrid => {
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    lib.display.grid_visible = !lib.display.grid_visible;
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            PrimitiveEditorMsg::SymbolCycleGridSize => {
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    let sizes = crate::canvas::grid::GRID_SIZES_MM;
                    let current_idx = sizes
                        .iter()
                        .position(|s| (s - lib.display.grid_size_mm).abs() < f32::EPSILON)
                        .unwrap_or(2);
                    let next_idx = (current_idx + 1) % sizes.len();
                    lib.display.grid_size_mm = sizes[next_idx];
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            PrimitiveEditorMsg::SymbolCycleUnit => {
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    use signex_types::coord::Unit;
                    lib.display.unit = match lib.display.unit {
                        Unit::Mm => Unit::Mil,
                        Unit::Mil => Unit::Inch,
                        Unit::Inch => Unit::Micrometer,
                        Unit::Micrometer => Unit::Mm,
                    };
                }
                // Unit only affects the status footer text — no
                // canvas redraw needed, but cache clear is harmless
                // and keeps the message handling shape consistent.
                return Task::none();
            }
            _ => {}
        }

        // Symbol-only mutations.
        if let Some(editor) = self.document_state.symbol_editors.get_mut(&path) {
            apply_symbol_primitive_edit(editor, msg);
            // v0.20 — primitive editor edits (designator/size/etc, and
            // critically `placement_paused`) need the panel context
            // rebuilt so the right-dock view reads the new value next
            // frame. Without this the panel renders against stale
            // `FootprintEditorPanelContext` and TAB-pause-driven UI
            // changes (Pad form vs no Pad form) silently miss.
            self.refresh_panel_ctx();
            return Task::none();
        }

        // v0.26-E — clipboard ops need both `pad_clipboard` and the
        // editor mutable simultaneously, so split-borrow at the call
        // site instead of routing through `apply_footprint_primitive_edit`.
        match &msg {
            PrimitiveEditorMsg::FootprintCopyPad
            | PrimitiveEditorMsg::FootprintCutPad
            | PrimitiveEditorMsg::FootprintPastePad => {
                let crate::app::DocumentState {
                    footprint_editors,
                    pad_clipboard,
                    ..
                } = &mut self.document_state;
                if let Some(editor) = footprint_editors.get_mut(&path) {
                    apply_footprint_clipboard_op(editor, pad_clipboard, &msg);
                }
                self.refresh_panel_ctx();
                return Task::none();
            }
            _ => {}
        }

        // Footprint-only mutations.
        if let Some(editor) = self.document_state.footprint_editors.get_mut(&path) {
            apply_footprint_primitive_edit(editor, msg);
            self.refresh_panel_ctx();
            return Task::none();
        }

        tracing::warn!(
            target: "signex::library",
            path = %path.display(),
            "primitive editor event: no matching tab state",
        );
        Task::none()
    }

    /// Clear the canvas cache for the primitive editor tab keyed by
    /// `path`. Used by the per-library display-settings handlers so
    /// the visible canvas redraws as soon as the user flips bg /
    /// grid / etc.
    fn invalidate_primitive_canvas_cache(&mut self, path: &std::path::Path) {
        if let Some(editor) = self.document_state.symbol_editors.get_mut(path) {
            editor.canvas_cache.clear();
        }
        if let Some(editor) = self.document_state.footprint_editors.get_mut(path) {
            editor.canvas_cache.clear();
        }
    }

    /// Write the primitive at `path` back to disk as JSON, commit
    /// through the matching adapter (when the file lives under a
    /// mounted `.snxlib/`), mark the tab clean, and ask the
    /// `LibrarySet` to reload its cached copy so any open Component
    /// Preview tabs see the new bytes.
    pub(crate) fn save_primitive_tab_at(&mut self, path: &std::path::Path) {
        // Symbol path — write the full multi-symbol container back to
        // disk so other symbols in the same file are preserved.
        if let Some(editor) = self.document_state.symbol_editors.get_mut(path) {
            // Refresh the active symbol's + the file's updated
            // timestamps so downstream consumers can detect the rewrite.
            let now = chrono::Utc::now();
            editor.primitive_mut().updated = now;
            editor.file.updated = now;
            // v0.18.4 — emit TOML envelope (mirror of v0.18.2 .snxfpt).
            let toml_text = match editor.file.to_toml_string() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "save primitive: serialize symbol file failed",
                    );
                    return;
                }
            };
            if let Err(e) = atomic_write(path, toml_text.as_bytes()) {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "save primitive: write .snxsym failed",
                );
                return;
            }
            // Capture the symbol name for the commit message before
            // dropping the editor borrow.
            let sym_name = editor.primitive().name.clone();
            editor.dirty = false;
            // Clear the project-scoped dirty marker if any callers
            // had set it.
            self.document_state.dirty_paths.remove(path);
            // Clear the matching tab's dirty flag too.
            if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
                tab.dirty = false;
            }
            // Commit through the matching adapter so the edit lands
            // in git history. No-op when the file lives outside any
            // mounted library (lone-file edit) or when the adapter
            // has no version control (database backend).
            self.commit_external_change_for(path, &format!("save symbol {sym_name}"));
            // v0.22 Phase 8.4 extension — also commit into the
            // owning project's git repo (if `enable_git` is on for
            // that project). When both library- and project-scope VC
            // are enabled the file picks up two parallel commit
            // histories — library tracks symbol-only churn, project
            // tracks the full project snapshot.
            self.commit_save_to_project_git(path, &format!("Save symbol {sym_name}"));
            // Refresh the matching library's primitive cache so the
            // picker modal picks up the new symbol immediately.
            self.refresh_primitive_cache_for(path);
            // Best-effort LibrarySet reload so Component Preview
            // tabs that already cached the primitive see the new bytes.
            self.reload_primitive_in_library_set(path);
            // v0.14.2 — refresh panel ctx so the project-tree red
            // dirty dot drops on the row (same F10 fix as the
            // footprint branch below + the schematic save handler).
            self.refresh_panel_ctx();
            return;
        }

        // Footprint path.
        if let Some(editor) = self.document_state.footprint_editors.get_mut(path) {
            // Sync the canvas-mirrored pad list back into the
            // primitive before serialising — `state.pads` is
            // authoritative on the editor side; without this, in-
            // editor pad edits wouldn't persist.
            let now = chrono::Utc::now();
            {
                let (state, primitive) = editor.parts_mut();
                crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(
                    state, primitive,
                );
            }
            editor.primitive_mut().updated = now;
            editor.file.updated = now;
            // v0.18.6 — emit the editor's persisted FootprintFile
            // directly. `file_uuid` and any multi-footprint siblings
            // are preserved across saves (mirror of SymbolEditorState).
            let toml_text = match editor.file.to_toml_string() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "save primitive: serialize footprint failed",
                    );
                    return;
                }
            };
            if let Err(e) = atomic_write(path, toml_text.as_bytes()) {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "save primitive: write .snxfpt failed",
                );
                return;
            }
            let fp_name = editor.primitive().name.clone();
            editor.dirty = false;
            self.document_state.dirty_paths.remove(path);
            if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
                tab.dirty = false;
            }
            self.commit_external_change_for(path, &format!("save footprint {fp_name}"));
            // v0.22 Phase 8.4 extension — same as the symbol branch:
            // also commit into the owning project's git repo when
            // its `enable_git` is on. Library + project repos can
            // run in parallel; nested `.snxlib/.git/` is opaque to
            // the project repo at the same path.
            self.commit_save_to_project_git(path, &format!("Save footprint {fp_name}"));
            self.refresh_primitive_cache_for(path);
            self.reload_primitive_in_library_set(path);
            // v0.14.2 — same F10 pattern as the schematic save: the
            // project-tree red dirty dot reads cached
            // `panel_ctx.projects[*].sheets[*].is_dirty`, which only
            // refreshes inside `refresh_panel_ctx`. Without this
            // call the dot lingers on the row even though
            // `dirty_paths` no longer contains the path.
            self.refresh_panel_ctx();
        }
    }

    /// Find the open library whose root contains `path`, then ask its
    /// adapter to stage + commit. Best-effort: silently returns when
    /// no mounted library covers `path` (lone-file edit) or when the
    /// commit itself fails (warning is emitted via tracing). Never
    /// blocks the user — the file write already succeeded.
    fn commit_external_change_for(&self, path: &std::path::Path, message: &str) {
        // Find the open library whose working dir is an ancestor of
        // `path`. `lib.root` is the `.snxlib` *file* path now, so we
        // walk against its parent directory (where `symbols/` and
        // `footprints/` actually live).
        let lib = self
            .library
            .open_libraries
            .iter()
            .find(|lib| lib.root_dir().map(|d| path.starts_with(d)).unwrap_or(false));
        let Some(lib) = lib else {
            return;
        };
        let Some(adapter) = self.library.set.get(lib.library_id) else {
            return;
        };
        if let Err(e) = adapter.commit_external_change(path, message) {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "save primitive: commit_external_change failed (file written; commit deferred)",
            );
        }
    }

    /// Refresh the matching library's per-kind primitive cache so the
    /// picker modal sees the just-saved primitive without waiting
    /// for the next full `refresh_components` round-trip. No-op when
    /// `path` lives outside any mounted library.
    fn refresh_primitive_cache_for(&mut self, path: &std::path::Path) {
        // Same `root_dir()` ancestor walk as
        // `commit_external_change_for` — `lib.root` is the `.snxlib`
        // file, the on-disk children sit under its parent dir.
        let library_id = match self
            .library
            .open_libraries
            .iter()
            .find(|lib| lib.root_dir().map(|d| path.starts_with(d)).unwrap_or(false))
        {
            Some(lib) => lib.library_id,
            None => return,
        };
        // Two-step borrow dance: snapshot the listings through the
        // mounted adapter, then move them onto the OpenLibrary entry.
        let (symbols, footprints, sims) = match self.library.set.get(library_id) {
            Some(adapter) => (
                adapter.list_symbols().unwrap_or_default(),
                adapter.list_footprints().unwrap_or_default(),
                adapter.list_sims().unwrap_or_default(),
            ),
            None => return,
        };
        if let Some(lib) = self
            .library
            .open_libraries
            .iter_mut()
            .find(|lib| lib.library_id == library_id)
        {
            lib.cached_symbols = symbols;
            lib.cached_footprints = footprints;
            lib.cached_sims = sims;
        }
    }

    /// Walk the open libraries to find one whose root contains
    /// `path` (e.g. `…/mylib.snxlib/symbols/foo.snxsym` lives under
    /// `…/mylib.snxlib/`), and ask the matching adapter to reload
    /// the primitive UUID encoded in the file. The adapter's
    /// `reload_primitive` (where supported) repopulates its in-memory
    /// cache so any Component Preview tabs that resolve through
    /// `LibrarySet` see the new bytes on the next render.
    ///
    /// Best-effort — returns silently when the path isn't under a
    /// mounted library or when the adapter has no reload hook.
    fn reload_primitive_in_library_set(&mut self, _path: &std::path::Path) {
        // Stubbed pending the corresponding `LibrarySet::reload_primitive`
        // helper. The standalone editor tab already holds the
        // authoritative copy of the primitive in memory and on-disk
        // round-trips happen here; Component Preview tabs pull
        // through `LibrarySet::resolve_*` on the next view, so the
        // only hole this leaves is a Preview tab that has already
        // resolved + cached its primitive in editor state.
    }
}

/// Atomic write — write `bytes` to `<path>.tmp` then `rename` over
/// `path`. A crash mid-write leaves either the original file intact
/// Re-export of the shared atomic-write helper (HI-6). Lives in
/// `signex-types::atomic_io` so engine, library, and app share one
/// implementation; the function used to be a private duplicate here.
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    signex_types::atomic_io::atomic_write(path, bytes)
}

/// World-space bbox covering the symbol's body + every pin + every
/// graphic. Used by `SymbolFit` so the dispatcher can compute a
/// `Camera::fit_rect` against the active symbol without reaching
/// into the canvas program. Matches the `SymbolCanvas::bbox` shape
/// (pad 5.08 mm around the body, 1.27 mm around every pin) so
/// click-Fit and Home key produce the same viewport.
/// v0.26-E — apply Cut / Copy / Paste against the document-level
/// `pad_clipboard`. Split-borrowed at the call site so both the
/// editor and the clipboard slot are mutable.
///
/// Behaviour:
///  - **Copy**: clones the selected pad into the clipboard. No-op
///    when nothing is selected.
///  - **Cut**: Copy + delete; mirrors into the sketch + invalidates
///    the canvas cache.
///  - **Paste**: places a clone of the clipboard pad at the cursor
///    (or `original.position + (1mm, 1mm)` if cursor is unknown),
///    picks a free designator (max + 1), pre-computes a fresh
///    `sketch_entity_id` so the new pad mirrors into the sketch on
///    its first edit, and selects the new pad post-paste.
pub(crate) fn apply_footprint_clipboard_op(
    editor: &mut crate::app::FootprintEditorState,
    clipboard: &mut Option<crate::library::editor::footprint::state::EditorPad>,
    msg: &PrimitiveEditorMsg,
) {
    use crate::library::editor::footprint::pad_to_sketch;
    use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;

    match msg {
        PrimitiveEditorMsg::FootprintCopyPad => {
            let Some(idx) = editor.state.selected_pad else {
                return;
            };
            let Some(pad) = editor.state.pads.get(idx) else {
                return;
            };
            *clipboard = Some(pad.clone());
        }
        PrimitiveEditorMsg::FootprintCutPad => {
            let Some(idx) = editor.state.selected_pad else {
                return;
            };
            // Snapshot history BEFORE the mutation so undo restores
            // the pad. Mirrors the v0.24 push_history pattern.
            editor.push_history();
            let did_delete = editor.with_parts(|state, primitive| {
                let Some(pad) = state.pads.get(idx).cloned() else {
                    return false;
                };
                *clipboard = Some(pad.clone());
                pad_to_sketch::mirror_delete_pad_from_sketch(&pad, primitive);
                state.delete_pad(idx);
                CanvasState::sync_pads_to_primitive(state, primitive);
                true
            });
            if did_delete {
                editor.canvas_cache.clear();
                editor.dirty = true;
            }
        }
        PrimitiveEditorMsg::FootprintPastePad => {
            let Some(template) = clipboard.clone() else {
                return;
            };
            // Paste position: prefer the cursor; fall back to the
            // template''s original + a tiny diagonal offset so the
            // user sees the new pad rather than overlap.
            let (px, py) = match editor.state.cursor_mm {
                Some((cx, cy)) => (cx, cy),
                None => (template.position_mm.0 + 1.0, template.position_mm.1 + 1.0),
            };
            // Pick a designator: max-existing + 1, falling back to
            // the template''s number when nothing parses.
            let next_num = editor
                .state
                .pads
                .iter()
                .filter_map(|p| p.number.parse::<u64>().ok())
                .max()
                .map(|n| (n + 1).to_string())
                .unwrap_or_else(|| template.number.clone());
            editor.push_history();
            editor.with_parts(|state, primitive| {
                let mut new_pad = template.clone();
                new_pad.position_mm = (px, py);
                new_pad.number = next_num.clone();
                // Reset sketch links so the pad mirrors freshly into
                // the sketch on the next mode switch (avoids two
                // pads sharing an entity id).
                new_pad.sketch_entity_id = None;
                new_pad.corner_entity_ids = None;
                state.pads.push(new_pad);
                let new_idx = state.pads.len() - 1;
                state.selected_pad = Some(new_idx);
                state.recompute_courtyard();
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        _ => {}
    }
}

/// Apply a primitive-editor event to a standalone Footprint editor
/// state. Mirrors the footprint-tab arms of `apply_inline_edit` but
/// against the path-keyed standalone state.
pub(crate) fn apply_footprint_primitive_edit(
    editor: &mut crate::app::FootprintEditorState,
    msg: PrimitiveEditorMsg,
) {
    use crate::library::editor::footprint::layers::FpLayer;
    use crate::library::editor::footprint::state::FootprintEditorState as CanvasState;

    use crate::library::editor::footprint::pad_to_sketch;

    // v0.27 — Role=Pad on a Line is shorthand for "make this loop a
    // pad." Rewrite the message here so the SetRole arm only ever
    // sees Point-targeted Pad role assignments (where it makes
    // sense). Without this, Role=Pad on a Line was a silent no-op
    // — the Properties dropdown read as broken.
    let msg = if let PrimitiveEditorMsg::FootprintSketchSetRole {
        id,
        role: crate::library::messages::RoleTag::Pad,
    } = &msg
    {
        let is_line = editor
            .primitive()
            .sketch
            .as_ref()
            .and_then(|s| s.entities.iter().find(|e| e.id == *id))
            .map(|e| matches!(e.kind, signex_sketch::entity::EntityKind::Line { .. }))
            .unwrap_or(false);
        if is_line {
            editor.state.selected_sketch = Some(*id);
            PrimitiveEditorMsg::FootprintSketchMakePadFromProfile
        } else {
            msg
        }
    } else {
        msg
    };

    /// v0.15 — gate the Pads → Sketch mirror on whether the
    /// footprint already has a sketch (i.e. the user has visited
    /// Sketch mode at least once OR auto-mint has already fired).
    /// Mirroring into a non-existent sketch would create one
    /// silently, which is undesirable for users who only ever work
    /// in Pads mode.
    fn footprint_sketch_is_active(fp: &signex_library::primitive::footprint::Footprint) -> bool {
        match fp.sketch.as_ref() {
            Some(s) => !s.entities.is_empty(),
            None => false,
        }
    }

    // v0.24 Phase 1 (Track B) — capture an undo snapshot ahead of
    // any mutating message. Selection-only / cursor-tracking /
    // tool-state messages are pure UI state and don't need history;
    // everything else gets a snapshot so Ctrl+Z reverses it. The
    // dispatcher is the canonical entry point for footprint
    // mutations, so wrapping here covers every message type
    // uniformly without each arm needing its own push.
    if mutates_footprint_state(&msg) {
        editor.push_history();
    }

    match msg {
        // v0.18.7 — switch the active footprint within the multi-
        // footprint envelope. Resets the canvas pad list off the
        // newly-active primitive, clears selection, refits the
        // camera on the next frame so a different-sized footprint
        // doesn't open at a stale zoom.
        PrimitiveEditorMsg::FootprintSelectActiveIdx(idx) => {
            let last = editor.file.footprints.len().saturating_sub(1);
            let clamped = idx.min(last);
            if clamped == editor.active_idx {
                return;
            }
            editor.active_idx = clamped;
            // Re-derive the canvas-side state from the new active
            // primitive so pads / sketch / courtyard mirror what's
            // on disk for this footprint.
            editor.state =
                crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
                    editor.primitive(),
                );
            editor.canvas_cache.clear();
        }
        // v0.18.7 — append a fresh empty footprint to the envelope
        // and switch onto it. Names the new sibling `Footprint N`
        // where N counts existing siblings + 1; the user can rename
        // via the Properties panel.
        PrimitiveEditorMsg::FootprintAddNewSibling => {
            let next_n = editor.file.footprints.len() + 1;
            let new_fp = signex_library::Footprint::empty(format!("Footprint {next_n}"));
            editor.file.footprints.push(new_fp);
            editor.active_idx = editor.file.footprints.len() - 1;
            editor.state =
                crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
                    editor.primitive(),
                );
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintAddPad { x_mm, y_mm } => {
            // v0.15 — bidirectional Pads → Sketch mirror. The new
            // pad gets a backing sketch Point + PadAttr (when the
            // sketch already has any other backed entity, i.e. the
            // user has been in Sketch mode at least once).
            // v0.18.6 — split-borrow at the top of the arm so
            // `state.pads.get_mut(...)` and `primitive` coexist; both
            // halves originate from disjoint editor fields.
            editor.with_parts(|state, primitive| {
                let idx = state.add_pad_at(x_mm, y_mm);
                if let Some(pad) = state.pads.get_mut(idx) {
                    if footprint_sketch_is_active(primitive) {
                        pad_to_sketch::mirror_add_pad_to_sketch(pad, primitive);
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintAddVia { x_mm, y_mm } => {
            // v0.27 — vias are a small Round plated-through pad. The
            // canonical via geometry is fixed (0.6 mm copper / 0.3 mm
            // drill / Multi-Layer F.Cu+B.Cu+masks) so the user gets a
            // proper via regardless of what `next_pad_defaults` looks
            // like. Bypasses `add_pad_at` (which inherits Pads-mode
            // defaults) and constructs the EditorPad directly.
            use crate::library::editor::footprint::state::EditorPad;
            use signex_library::{LayerId, PadKind, PadShape};
            const VIA_DIAMETER_MM: f64 = 0.6;
            const VIA_DRILL_MM: f64 = 0.3;
            editor.with_parts(|state, primitive| {
                let number = state.next_pad_number();
                let mut pad = EditorPad::new_default(number, (x_mm, y_mm));
                pad.size_mm = (VIA_DIAMETER_MM, VIA_DIAMETER_MM);
                pad.shape = PadShape::Round;
                pad.kind = PadKind::Tht;
                pad.drill_diameter_mm = Some(VIA_DRILL_MM);
                pad.layers = vec![
                    LayerId::new("F.Cu"),
                    LayerId::new("F.Mask"),
                    LayerId::new("B.Cu"),
                    LayerId::new("B.Mask"),
                ];
                state.pads.push(pad);
                let idx = state.pads.len() - 1;
                state.selected_pad = Some(idx);
                if let Some(p) = state.pads.get_mut(idx) {
                    if footprint_sketch_is_active(primitive) {
                        pad_to_sketch::mirror_add_pad_to_sketch(p, primitive);
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        // v0.18.14 — Selection Filter pill toggle from the unified
        // active bar. The panel-side equivalent
        // (`PanelMsg::FpEditorToggleSelectionFilter`) routes through
        // a dedicated handler in `handlers/dock/sch_library`; this
        // arm covers the active-bar dispatch path.
        PrimitiveEditorMsg::FootprintToggleSelectionFilter(kind) => {
            editor.state.selection_filter.toggle(kind);
            editor.canvas_cache.clear();
        }
        // v0.18.15.1 — Place Track 2-click gesture. First click
        // stashes the start in `state.track_first`; second click
        // commits the line to silk_f and chains by re-stashing the
        // second click as the next gesture's start.
        PrimitiveEditorMsg::FootprintTrackClick { x_mm, y_mm } => {
            match editor.state.track_first {
                None => {
                    editor.state.track_first = Some((x_mm, y_mm));
                }
                Some((sx, sy)) => {
                    let primitive = editor.primitive_mut();
                    primitive
                        .silk_f
                        .push(signex_library::primitive::footprint::FpGraphic {
                            kind: signex_library::primitive::footprint::FpGraphicKind::Line {
                                from: [sx, sy],
                                to: [x_mm, y_mm],
                            },
                            stroke_width: 0.15,
                            filled: false,
                        });
                    // Chain — the second click becomes the next
                    // segment's start, matching Altium's stroke-a-
                    // polyline workflow.
                    editor.state.track_first = Some((x_mm, y_mm));
                    editor.dirty = true;
                }
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintTrackCancel => {
            editor.state.track_first = None;
            editor.canvas_cache.clear();
        }
        // v0.18.15.3 — Place Arc 3-click gesture (centre / radius
        // start / sweep end). Idle → Center → Start → commit. After
        // commit the gesture resets to Idle (no chain — arcs
        // typically aren't strung together).
        PrimitiveEditorMsg::FootprintArcClick { x_mm, y_mm } => {
            use crate::library::editor::footprint::state::PlaceArcPending;
            let next = match editor.state.place_arc_pending {
                PlaceArcPending::Idle => PlaceArcPending::Center {
                    center: (x_mm, y_mm),
                },
                PlaceArcPending::Center { center } => PlaceArcPending::Start {
                    center,
                    start: (x_mm, y_mm),
                },
                PlaceArcPending::Start { center, start } => {
                    let (cx, cy) = center;
                    let (sx, sy) = start;
                    let radius = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                    if radius > 1e-6 {
                        let start_deg = (sy - cy).atan2(sx - cx).to_degrees();
                        let end_deg = (y_mm - cy).atan2(x_mm - cx).to_degrees();
                        let primitive = editor.primitive_mut();
                        primitive
                            .silk_f
                            .push(signex_library::primitive::footprint::FpGraphic {
                                kind: signex_library::primitive::footprint::FpGraphicKind::Arc {
                                    center: [cx, cy],
                                    radius,
                                    start_deg,
                                    end_deg,
                                },
                                stroke_width: 0.15,
                                filled: false,
                            });
                        editor.dirty = true;
                    }
                    PlaceArcPending::Idle
                }
            };
            editor.state.place_arc_pending = next;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintArcCancel => {
            editor.state.place_arc_pending =
                crate::library::editor::footprint::state::PlaceArcPending::Idle;
            editor.canvas_cache.clear();
        }
        // v0.18.15.4 — Place Polygon multi-click gesture. Each
        // click appends a vertex; commit happens on tool switch /
        // Esc via `FootprintPolygonCommit`.
        PrimitiveEditorMsg::FootprintPolygonClick { x_mm, y_mm } => {
            editor.state.place_polygon_vertices.push((x_mm, y_mm));
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintPolygonCommit => {
            let verts = std::mem::take(&mut editor.state.place_polygon_vertices);
            // v0.18.17 — emit one `Polygon` FpGraphic (instead of
            // N Lines). `filled` follows the active tool —
            // `PlacePolygon` = stroked outline, `PlaceRegion` =
            // solid fill.
            let filled = matches!(
                editor.state.pads_tool,
                crate::library::editor::footprint::state::PadsTool::PlaceRegion
            );
            if verts.len() >= 3 {
                let vertices: Vec<[f64; 2]> = verts.iter().map(|(x, y)| [*x, *y]).collect();
                let primitive = editor.primitive_mut();
                primitive
                    .silk_f
                    .push(signex_library::primitive::footprint::FpGraphic {
                        kind: signex_library::primitive::footprint::FpGraphicKind::Polygon {
                            vertices,
                        },
                        stroke_width: if filled { 0.0 } else { 0.15 },
                        filled,
                    });
                editor.dirty = true;
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintPolygonCancel => {
            editor.state.place_polygon_vertices.clear();
            editor.canvas_cache.clear();
        }
        // v0.18.18 — silk-front graphic selection. Clears
        // selected_pad symmetrically so the Properties panel
        // doesn't try to render two selection-specific bodies at
        // once.
        PrimitiveEditorMsg::FootprintSelectSilkF(sel) => {
            editor.state.selected_silk_f = sel;
            if sel.is_some() {
                editor.state.selected_pad = None;
            }
            editor.canvas_cache.clear();
        }
        // v0.18.18 — delete the selected silk-front graphic.
        // No-op when nothing is selected. Updates `editor.dirty`
        // and clears the selection state.
        PrimitiveEditorMsg::FootprintDeleteSilkF => {
            if let Some(idx) = editor.state.selected_silk_f {
                let primitive = editor.primitive_mut();
                if idx < primitive.silk_f.len() {
                    primitive.silk_f.remove(idx);
                    editor.dirty = true;
                }
                // HI-25: shared selection-adjustment helper — keep
                // `selected_silk_f` valid against the new vec length
                // instead of clearing unconditionally.
                editor.state.selected_silk_f =
                    crate::library::editor::footprint::state::adjust_selection_after_remove(
                        editor.state.selected_silk_f,
                        idx,
                    );
            }
            editor.canvas_cache.clear();
        }
        // v0.18.15 — Place String tool. Appends a silk-layer text
        // label `FpGraphic { kind: Text { position, content: "TEXT",
        // size: 1.0 }, stroke_width: 0.0 }` to the active footprint's
        // `silk_f`. The user edits the content via the Properties
        // panel later (Properties wiring is queued).
        PrimitiveEditorMsg::FootprintAddText { x_mm, y_mm } => {
            let primitive = editor.primitive_mut();
            primitive
                .silk_f
                .push(signex_library::primitive::footprint::FpGraphic {
                    kind: signex_library::primitive::footprint::FpGraphicKind::Text {
                        position: [x_mm, y_mm],
                        content: "TEXT".to_string(),
                        size: 1.0,
                    },
                    stroke_width: 0.0,
                    filled: false,
                });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        // v0.18.12 — Place Hole tool. Drops a non-plated through
        // hole at the cursor (no copper, drill from `next_pad_defaults`).
        PrimitiveEditorMsg::FootprintAddHole { x_mm, y_mm } => {
            editor.with_parts(|state, primitive| {
                let idx = state.add_hole_at(x_mm, y_mm);
                if let Some(pad) = state.pads.get_mut(idx) {
                    if footprint_sketch_is_active(primitive) {
                        pad_to_sketch::mirror_add_pad_to_sketch(pad, primitive);
                    }
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintMovePad { idx, x_mm, y_mm } => {
            editor.with_parts(|state, primitive| {
                state.move_pad(idx, x_mm, y_mm);
                // v0.15 — mirror the move into the sketch.
                if let Some(pad) = state.pads.get(idx) {
                    pad_to_sketch::mirror_move_pad_in_sketch(pad, primitive);
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintCursorAt { x_mm, y_mm } => {
            editor.state.cursor_mm = Some((x_mm, y_mm));
        }
        PrimitiveEditorMsg::FootprintSelectPad(sel) => {
            editor.state.selected_pad = sel;
            // v0.27 — single-pad select replaces the multi-select
            // extras too. Multi-select uses FootprintSelectPads.
            editor.state.selected_pads_extra.clear();
            // v0.27 — record the click position for Select
            // overlapped / Select next so the dropdown can find
            // the stack at the last-clicked location.
            if sel.is_some() {
                editor.state.last_click_world_mm = editor.state.cursor_mm;
            }
            // v0.18.18 — pad and silk selection are mutually
            // exclusive in the Properties panel; clear the silk
            // selection when a pad is picked.
            if sel.is_some() {
                editor.state.selected_silk_f = None;
            }
            // v0.25 polish — clear verbatim numeric buffers on
            // selection change so a stale "0.1." buffer from one
            // pad doesn't follow the user to the next pad's input.
            editor.state.numeric_buffers.clear();
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSelectPads(mut pads) => {
            // v0.27 — Altium-parity multi-select. Empty list = clear.
            // First entry becomes the primary (drives Properties);
            // rest land in `selected_pads_extra` for highlight only.
            // Dedupe so a sloppy caller passing [3, 3, 7] still gets
            // [3, 7] selected.
            pads.sort_unstable();
            pads.dedup();
            if pads.is_empty() {
                editor.state.selected_pad = None;
                editor.state.selected_pads_extra.clear();
            } else {
                editor.state.selected_pad = Some(pads[0]);
                editor.state.selected_pads_extra = pads[1..].to_vec();
                editor.state.selected_silk_f = None;
                editor.state.last_click_world_mm = editor.state.cursor_mm;
            }
            editor.state.numeric_buffers.clear();
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchSelectMany(ids) => {
            // v0.27 — Sketch-mode multi-select replacement. First
            // entity is primary (drives the inspector + DOF
            // overlay focus); the second slots into the secondary
            // (used by the constraint submenu's "two entities"
            // pairing); the rest land in extras. Empty list
            // deselects everything.
            if ids.is_empty() {
                editor.state.selected_sketch = None;
                editor.state.selected_sketch_secondary = None;
                editor.state.selected_sketch_extra.clear();
            } else {
                editor.state.selected_sketch = Some(ids[0]);
                editor.state.selected_sketch_secondary = ids.get(1).copied();
                editor.state.selected_sketch_extra =
                    ids.iter().skip(2).copied().collect();
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintDeleteSelected => {
            // v0.27 — Delete walks the full multi-select set, not
            // just the primary `selected_pad`. Rubber-band + Ctrl-
            // click extras get the same treatment as the primary so
            // pressing Delete after a rubber-band sweep clears the
            // whole region. Sketch-mode entities use the sketch
            // dispatcher so the solver re-converges without dangling
            // constraints.
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use crate::library::editor::footprint::state::EditorMode;

            let did_delete = editor.with_parts(|state, primitive| {
                let mut any = false;

                // Sketch-mode deletion — primary + secondary + extras.
                if state.mode == EditorMode::Sketch {
                    use std::collections::HashSet;
                    let mut seen: HashSet<signex_sketch::id::SketchEntityId> = HashSet::new();
                    let mut victims: Vec<signex_sketch::id::SketchEntityId> = Vec::new();
                    let mut push_unique =
                        |id: signex_sketch::id::SketchEntityId,
                         vs: &mut Vec<signex_sketch::id::SketchEntityId>,
                         seen: &mut HashSet<_>| {
                            if seen.insert(id) {
                                vs.push(id);
                            }
                        };
                    if let Some(id) = state.selected_sketch.take() {
                        push_unique(id, &mut victims, &mut seen);
                    }
                    if let Some(id) = state.selected_sketch_secondary.take() {
                        push_unique(id, &mut victims, &mut seen);
                    }
                    let extras: Vec<_> = state.selected_sketch_extra.drain(..).collect();
                    for id in extras {
                        push_unique(id, &mut victims, &mut seen);
                    }
                    for id in victims {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::DeleteEntity(id),
                        );
                        any = true;
                    }
                }

                // Pads (always — rubber-band can also select pads).
                let mut pad_victims: Vec<usize> = Vec::new();
                if let Some(idx) = state.selected_pad {
                    pad_victims.push(idx);
                }
                pad_victims.extend(state.selected_pads_extra.iter().copied());
                pad_victims.sort_unstable();
                pad_victims.dedup();
                // Remove highest-index first so earlier indices stay
                // valid through the loop.
                pad_victims.sort_unstable_by(|a, b| b.cmp(a));
                for idx in pad_victims {
                    if let Some(pad) = state.pads.get(idx) {
                        pad_to_sketch::mirror_delete_pad_from_sketch(pad, primitive);
                    }
                    state.delete_pad(idx);
                    any = true;
                }
                state.selected_pads_extra.clear();
                CanvasState::sync_pads_to_primitive(state, primitive);
                any
            });
            if did_delete {
                editor.canvas_cache.clear();
                editor.dirty = true;
            }
        }
        PrimitiveEditorMsg::FootprintToggleLayer(name) => {
            if let Some(layer) = FpLayer::from_standard_name(&name) {
                editor.state.layer_visibility.toggle(layer);
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintToggleAutoFit => {
            editor.state.toggle_auto_fit();
            editor.with_parts(|state, primitive| {
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSetMode(mode) => {
            use crate::library::editor::footprint::state::EditorMode;
            // v0.14.2 — bidirectional sketch ↔ pads foundation.
            // When the user enters Sketch mode for the first time on
            // a footprint that has literal pads but no sketch
            // entities yet, mint a Point + PadAttr for every pad so
            // they're visible / editable in Sketch mode. The bake
            // immediately re-emits identical pads, so the round-trip
            // is identity-preserving.
            let entering_sketch =
                editor.state.mode != EditorMode::Sketch && mode == EditorMode::Sketch;
            if entering_sketch {
                use crate::library::editor::footprint::pad_to_sketch;
                let _ = editor.with_parts(|state, primitive| {
                    pad_to_sketch::auto_mint_for_literal_pads(&mut state.pads, primitive)
                });
            }
            // v0.15 — reset tool state on every mode change so a
            // stale Place Pad / Place Point selection from a prior
            // session in this tab doesn't carry over and cause
            // accidental entity placement on the first click.
            editor.state.pads_tool = crate::library::editor::footprint::state::PadsTool::Select;
            editor.state.active_tool = crate::library::editor::footprint::state::SketchTool::Select;
            editor.state.tool_pending = crate::library::editor::footprint::state::ToolPending::Idle;
            editor.state.mode = mode;
            // Run the dispatcher so the sketch is initialised + solved
            // on first switch into Sketch mode (or no-op otherwise).
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::SetMode(mode));
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchPlacePoint { x_mm, y_mm } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use signex_sketch::entity::{Entity, EntityKind};
            use signex_sketch::id::SketchEntityId;
            use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
            // Ensure the sketch has at least one plane so the entity has
            // somewhere to live.
            let plane_id = match editor.primitive().sketch.as_ref() {
                Some(s) if !s.planes.is_empty() => s.planes[0].id,
                _ => {
                    let pid = PlaneId::new();
                    let sketch = editor
                        .primitive_mut()
                        .sketch
                        .get_or_insert_with(signex_sketch::SketchData::default);
                    sketch.planes.push(Plane {
                        id: pid,
                        kind: PlaneKind::BoardTop,
                    });
                    pid
                }
            };
            let id = SketchEntityId::new();
            let mut entity = Entity::new(id, plane_id, EntityKind::Point { x: x_mm, y: y_mm });
            entity.construction = editor.state.construction_mode;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddEntity(entity));
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchEditParameter { name, expr } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(
                    state,
                    primitive,
                    SketchEdit::EditParameter { name, expr },
                );
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchSetTool(tool) => {
            editor.state.active_tool = tool;
            editor.state.tool_pending = crate::library::editor::footprint::state::ToolPending::Idle;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchToggleConstruction => {
            editor.state.construction_mode = !editor.state.construction_mode;
            // v0.22 Phase A5 — mutual exclusivity with centerline.
            if editor.state.construction_mode {
                editor.state.centerline_mode = false;
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchToggleCenterline => {
            editor.state.centerline_mode = !editor.state.centerline_mode;
            // Mutual exclusivity — enabling centerline clears
            // construction (same Fusion 360 convention as the
            // Linetype submenu where Normal/Construction/Centerline
            // are radio-style).
            if editor.state.centerline_mode {
                editor.state.construction_mode = false;
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintTogglePlacementPause => {
            editor.state.placement_paused = !editor.state.placement_paused;
            editor.canvas_cache.clear();
        }
        // v0.26 — right-click context menu plumbing. State-only
        // mutations; canvas cache is cleared when target adjusts the
        // selection (right-click on a pad selects it Altium-style).
        PrimitiveEditorMsg::FootprintShowContextMenu { x, y, target } => {
            use crate::library::editor::footprint::state::FootprintContextTarget;
            // Close any active dropdown before opening the context
            // menu so two popups never coexist (Altium parity).
            editor.state.active_bar_menu = None;
            // v0.26-B Altium parity — right-click on a pad selects
            // it (so subsequent menu actions like Delete / Properties
            // act on the right-clicked pad) without losing prior
            // selection on bare-canvas right-click.
            match target {
                FootprintContextTarget::Pad(idx) => {
                    if editor.state.selected_pad != Some(idx) {
                        editor.state.selected_pad = Some(idx);
                        editor.state.selected_silk_f = None;
                        editor.canvas_cache.clear();
                    }
                }
                FootprintContextTarget::SilkF(idx) => {
                    if editor.state.selected_silk_f != Some(idx) {
                        editor.state.selected_silk_f = Some(idx);
                        editor.state.selected_pad = None;
                        editor.canvas_cache.clear();
                    }
                }
                FootprintContextTarget::Empty => {}
            }
            editor.state.context_menu = Some(
                crate::library::editor::footprint::state::FootprintContextMenuState {
                    x,
                    y,
                    target,
                    submenu: None,
                },
            );
        }
        PrimitiveEditorMsg::FootprintCloseContextMenu => {
            editor.state.context_menu = None;
        }
        PrimitiveEditorMsg::FootprintContextMenuOpenSubmenu(sm) => {
            if let Some(ref mut menu) = editor.state.context_menu {
                menu.submenu = sm;
            }
        }
        PrimitiveEditorMsg::FootprintFitConsumed => {
            editor.state.fit_pending = false;
        }
        // v0.26-E — clipboard ops intercepted at the call site
        // (apply_footprint_clipboard_op needs split-borrow with
        // document_state.pad_clipboard). The match arm here is
        // unreachable in practice but required for exhaustiveness.
        PrimitiveEditorMsg::FootprintCopyPad
        | PrimitiveEditorMsg::FootprintCutPad
        | PrimitiveEditorMsg::FootprintPastePad => {}
        PrimitiveEditorMsg::FootprintContextMenuAction(action) => {
            use crate::library::editor::footprint::state::FootprintContextAction as Act;
            match action {
                Act::SelectAllPads => {
                    // Existing semantics: SelectAll only meaningful
                    // when there's at least one pad. With multi-
                    // select not yet implemented, "Select All" maps
                    // to selecting the first pad as a stand-in until
                    // the v0.26 multi-pad selection lands. The dock
                    // SelectAll path on the active bar already does
                    // the right thing — defer to it once it grows.
                    if !editor.state.pads.is_empty() {
                        editor.state.selected_pad = Some(0);
                    }
                    editor.state.context_menu = None;
                    editor.canvas_cache.clear();
                }
                Act::DeselectAll => {
                    editor.state.selected_pad = None;
                    editor.state.selected_silk_f = None;
                    editor.state.selected_sketch = None;
                    editor.state.selected_sketch_secondary = None;
                    editor.state.context_menu = None;
                    editor.canvas_cache.clear();
                }
                Act::FitToWindow => {
                    // v0.26-C — arm the one-shot fit signal. The
                    // canvas Program''s next `update` tick has &mut
                    // access to its own State (where `scale` /
                    // `offset` / `did_initial_fit` live) and can
                    // consume the flag; it publishes
                    // `EditorMsg::FootprintFitConsumed` to clear the
                    // request so it doesn''t re-trigger every event.
                    editor.state.fit_pending = true;
                    editor.state.context_menu = None;
                    editor.canvas_cache.clear();
                }
            }
        }
        PrimitiveEditorMsg::FootprintSetPadsTool(tool) => {
            editor.state.pads_tool = tool;
            // v0.18.15.1 — leaving the PlaceTrack tool clears the
            // in-flight gesture so re-entering doesn't start
            // mid-segment from a stale anchor.
            if !matches!(
                tool,
                crate::library::editor::footprint::state::PadsTool::PlaceTrack
            ) {
                editor.state.track_first = None;
            }
            // v0.18.15.3 — same cleanup for Place Arc.
            if !matches!(
                tool,
                crate::library::editor::footprint::state::PadsTool::PlaceArc
            ) {
                editor.state.place_arc_pending =
                    crate::library::editor::footprint::state::PlaceArcPending::Idle;
            }
            // v0.18.15.4/v0.18.17 — leaving Place Polygon /
            // Place Region commits the in-flight vertex stash if
            // it has ≥ 3 vertices, then clears. The `filled` flag
            // follows the OUTGOING tool (we just set
            // editor.state.pads_tool = tool above; check the
            // OLD tool's identity by recording before the swap is
            // unnecessary because PadsTool::PlaceRegion is the
            // only tool that flips filled).
            let was_polygon_or_region = !editor.state.place_polygon_vertices.is_empty();
            if was_polygon_or_region
                && !matches!(
                    tool,
                    crate::library::editor::footprint::state::PadsTool::PlacePolygon
                        | crate::library::editor::footprint::state::PadsTool::PlaceRegion
                )
            {
                let verts = std::mem::take(&mut editor.state.place_polygon_vertices);
                if verts.len() >= 3 {
                    // The dispatcher arm uses
                    // `editor.state.pads_tool` (now equal to the
                    // NEW tool), so `filled` would be wrong. We
                    // can't distinguish whether the user was on
                    // PlacePolygon vs PlaceRegion now — fall back
                    // to `filled: false` and let the user re-fire
                    // PlaceRegion if they wanted fill. Future:
                    // store filled-ness on the in-flight stash
                    // alongside vertices.
                    let vertices: Vec<[f64; 2]> = verts.iter().map(|(x, y)| [*x, *y]).collect();
                    let primitive = editor.primitive_mut();
                    primitive
                        .silk_f
                        .push(signex_library::primitive::footprint::FpGraphic {
                            kind: signex_library::primitive::footprint::FpGraphicKind::Polygon {
                                vertices,
                            },
                            stroke_width: 0.15,
                            filled: false,
                        });
                    editor.dirty = true;
                }
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintToolEscape => {
            // v0.15 — global Esc tool cancel. Resets both Pads and
            // Sketch tool state, mode-agnostic.
            editor.state.pads_tool = crate::library::editor::footprint::state::PadsTool::Select;
            editor.state.active_tool = crate::library::editor::footprint::state::SketchTool::Select;
            editor.state.tool_pending = crate::library::editor::footprint::state::ToolPending::Idle;
            // v0.18.15.1 — Esc also bails out of an in-flight
            // Place Track 2-click gesture.
            editor.state.track_first = None;
            // v0.18.15.3 — and Place Arc.
            editor.state.place_arc_pending =
                crate::library::editor::footprint::state::PlaceArcPending::Idle;
            // v0.18.15.4 — Esc drops the in-flight Polygon stash
            // (no commit; matches Altium's Esc-cancels-tool
            // semantic).
            editor.state.place_polygon_vertices.clear();
            // v0.13 — Esc also dismisses any open active-bar dropdown.
            editor.state.active_bar_menu = None;
            // v0.20 — Esc clears the selected pad / silk graphic too,
            // matching the schematic canvas + Altium PCB Library
            // editor. Without this, Esc only reset the tool but the
            // pad selection persisted, leaving the user staring at
            // pad properties they no longer wanted to edit.
            editor.state.selected_pad = None;
            editor.state.selected_silk_f = None;
            editor.state.placement_paused = false;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintToggleActiveBarMenu(menu) => {
            editor.state.active_bar_menu = match editor.state.active_bar_menu {
                Some(m) if m == menu => None,
                _ => Some(menu),
            };
        }
        PrimitiveEditorMsg::FootprintCloseActiveBarMenu => {
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintActiveBarStub(label) => {
            crate::diagnostics::log_info(format!(
                "Active bar: {label} — coming soon (footprint Altium parity)"
            ));
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintActiveBarToggleSnap(flag) => {
            use crate::panels::SnapOptionFlag;
            let opts = &mut editor.state.snap_options;
            match flag {
                SnapOptionFlag::PointHit => opts.point_hit = !opts.point_hit,
                SnapOptionFlag::HorizontalVertical => {
                    opts.horizontal_vertical = !opts.horizontal_vertical
                }
                SnapOptionFlag::Angle => opts.angle = !opts.angle,
                SnapOptionFlag::Grid => opts.grid = !opts.grid,
                SnapOptionFlag::TrackVertices => {
                    opts.snap_track_vertices = !opts.snap_track_vertices
                }
                SnapOptionFlag::TrackLines => opts.snap_track_lines = !opts.snap_track_lines,
                SnapOptionFlag::ArcCenters => opts.snap_arc_centers = !opts.snap_arc_centers,
                SnapOptionFlag::Intersections => opts.snap_intersections = !opts.snap_intersections,
                SnapOptionFlag::PadCenters => opts.snap_pad_centers = !opts.snap_pad_centers,
                SnapOptionFlag::PadVertices => opts.snap_pad_vertices = !opts.snap_pad_vertices,
                SnapOptionFlag::PadEdges => opts.snap_pad_edges = !opts.snap_pad_edges,
                SnapOptionFlag::ViaCenters => opts.snap_via_centers = !opts.snap_via_centers,
                SnapOptionFlag::Texts => opts.snap_texts = !opts.snap_texts,
                SnapOptionFlag::Regions => opts.snap_regions = !opts.snap_regions,
                SnapOptionFlag::FootprintOrigins => {
                    opts.snap_footprint_origins = !opts.snap_footprint_origins
                }
                SnapOptionFlag::Body3dPoints => {
                    opts.snap_3d_body_points = !opts.snap_3d_body_points
                }
                SnapOptionFlag::SnapToGrids => opts.snap_to_grids = !opts.snap_to_grids,
                SnapOptionFlag::SnapToGuides => opts.snap_to_guides = !opts.snap_to_guides,
                SnapOptionFlag::SnapToAxes => opts.snap_to_axes = !opts.snap_to_axes,
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintActiveBarSetSnappingMode(mode) => {
            editor.state.snapping_mode = mode;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintActiveBarSetSnapSubTab(sub) => {
            editor.state.snap_subtab = sub;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintActiveBarRotateSelection => {
            editor.with_parts(|state, primitive| {
                if let Some(idx) = state.selected_pad
                    && let Some(pad) = state.pads.get_mut(idx)
                {
                    pad.rotation_deg = (pad.rotation_deg + 90.0).rem_euclid(360.0);
                    CanvasState::sync_pads_to_primitive(state, primitive);
                }
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintActiveBarFlipSelection => {
            editor.with_parts(|state, primitive| {
                if let Some(idx) = state.selected_pad
                    && let Some(pad) = state.pads.get_mut(idx)
                {
                    let new_layers: Vec<signex_library::LayerId> = pad
                        .layers
                        .iter()
                        .map(|l| {
                            let s = l.as_str();
                            let flipped = if let Some(rest) = s.strip_prefix("F.") {
                                format!("B.{rest}")
                            } else if let Some(rest) = s.strip_prefix("B.") {
                                format!("F.{rest}")
                            } else {
                                s.to_string()
                            };
                            signex_library::LayerId::new(flipped)
                        })
                        .collect();
                    pad.layers = new_layers;
                    CanvasState::sync_pads_to_primitive(state, primitive);
                }
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintActiveBarAlignSelectionToGrid => {
            editor.with_parts(|state, primitive| {
                let step = state.snap_options.grid_step_mm.max(0.001);
                if let Some(idx) = state.selected_pad
                    && let Some(pad) = state.pads.get_mut(idx)
                {
                    let (x, y) = pad.position_mm;
                    pad.position_mm = ((x / step).round() * step, (y / step).round() * step);
                    // v0.23 — mirror the snap into the sketch so the
                    // construction outline + centre Point follow the
                    // pad. Skipping this left the sketch primitive
                    // stranded at the pre-snap position.
                    let pad_snapshot = pad.clone();
                    pad_to_sketch::mirror_move_pad_in_sketch(&pad_snapshot, primitive);
                    CanvasState::sync_pads_to_primitive(state, primitive);
                }
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintActiveBarMoveOriginToGrid => {
            editor.with_parts(|state, primitive| {
                let step = state.snap_options.grid_step_mm.max(0.001);
                let mut snapshots: Vec<crate::library::editor::footprint::state::EditorPad> =
                    Vec::with_capacity(state.pads.len());
                for pad in state.pads.iter_mut() {
                    let (x, y) = pad.position_mm;
                    pad.position_mm = ((x / step).round() * step, (y / step).round() * step);
                    snapshots.push(pad.clone());
                }
                // v0.23 — mirror every pad's new position into the
                // sketch. Same fix as the single-pad align path.
                for snapshot in &snapshots {
                    pad_to_sketch::mirror_move_pad_in_sketch(snapshot, primitive);
                }
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintActiveBarSelectAll => {
            // v0.27 — Altium-parity: Select All multi-selects every
            // pad in Pads mode; Sketch mode still picks the first
            // entity (sketch-side multi-select is a v0.28 follow-up).
            use crate::library::editor::footprint::state::EditorMode;
            match editor.state.mode {
                EditorMode::Sketch => {
                    if editor.state.selected_sketch.is_none() {
                        editor.state.selected_sketch = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|sk| sk.entities.first().map(|e| e.id));
                    }
                }
                EditorMode::Normal => {
                    if !editor.state.pads.is_empty() {
                        editor.state.selected_pad = Some(0);
                        editor.state.selected_pads_extra =
                            (1..editor.state.pads.len()).collect();
                    }
                }
                EditorMode::View3d => {}
            }
            editor.canvas_cache.clear();
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintActiveBarClearSelection => {
            editor.state.selected_pad = None;
            editor.state.selected_pads_extra.clear();
            editor.state.selected_sketch = None;
            editor.state.selected_sketch_secondary = None;
            editor.state.selected_silk_f = None;
            editor.canvas_cache.clear();
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintActiveBarSetSketchTool(tool) => {
            // Switch to Sketch mode if not already there, then arm the
            // selected sketch tool. Cancels any in-flight gesture.
            use crate::library::editor::footprint::state::{EditorMode, ToolPending};
            if editor.state.mode != EditorMode::Sketch {
                editor.state.mode = EditorMode::Sketch;
            }
            editor.state.active_tool = tool;
            editor.state.tool_pending = ToolPending::Idle;
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSetName(new_name) => {
            // Rename the ACTIVE internal footprint. The .snxfpt
            // envelope holds N footprints; only the user-selected one
            // mutates. Empty names are accepted but treated as
            // "unnamed" for breadcrumb / file display purposes.
            editor.primitive_mut().name = new_name;
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchToolEscape => {
            editor.state.tool_pending = crate::library::editor::footprint::state::ToolPending::Idle;
            // v0.24 Track D — leaving the gesture also drops any
            // numeric buffer the user had been typing. Otherwise a
            // half-typed length would leak across to a freshly-started
            // tool gesture.
            editor.state.placement_input = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchPlacementInputChar(ch) => {
            // v0.24 Track D — append `ch` to `placement_input.buffer`,
            // minting a fresh entry against the active tool's matching
            // `PlacementInputKind` if one isn't already pinned. Drops
            // the keypress silently when the active tool / pending
            // state doesn't accept numeric input.
            use crate::library::editor::footprint::state::{PlacementInput, PlacementInputKind};
            let tool = editor.state.active_tool;
            let pending = editor.state.tool_pending.clone();
            let kind_for_active = PlacementInputKind::from_active_tool(tool, &pending);
            // Resolve the kind: if a buffer already exists, keep its
            // kind so the user can finish typing across a second
            // keypress; otherwise mint one matched to the tool.
            let kind = match editor.state.placement_input.as_ref() {
                Some(existing) => existing.kind,
                None => match kind_for_active {
                    Some(k) => k,
                    None => return, // tool doesn't accept numeric input
                },
            };
            // Validation:
            // - digits always allowed,
            // - one decimal point per buffer,
            // - leading minus only for `ArcSweep` and only at position 0,
            // - everything else dropped.
            let buf_ref = editor
                .state
                .placement_input
                .as_ref()
                .map(|p| p.buffer.as_str())
                .unwrap_or("");
            let accept = if ch.is_ascii_digit() {
                true
            } else if ch == '.' {
                !buf_ref.contains('.')
            } else if ch == '-' {
                kind.allows_negative() && buf_ref.is_empty()
            } else {
                false
            };
            if !accept {
                return;
            }
            // Mint or append.
            let entry = editor
                .state
                .placement_input
                .get_or_insert_with(|| PlacementInput {
                    buffer: String::new(),
                    kind,
                });
            entry.buffer.push(ch);
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchPlacementInputBackspace => {
            // v0.24 Track D — pop one character; clear `placement_input`
            // entirely once the buffer empties so the next typed digit
            // mints a fresh entry against the (possibly different)
            // active tool.
            if let Some(entry) = editor.state.placement_input.as_mut() {
                entry.buffer.pop();
                if entry.buffer.is_empty() {
                    editor.state.placement_input = None;
                }
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintSketchPlacementInputEnter => {
            // v0.24 Track D — Enter is a no-op on state. The buffer
            // stays alive so the next click consumes it. The message
            // is captured at the canvas layer purely so the keypress
            // doesn't fall through to a global shortcut.
        }
        PrimitiveEditorMsg::FootprintSketchPlacementInputEscape => {
            // v0.24 Track D — Esc throws away the buffer immediately;
            // the next click commits at the cursor position with no
            // override. Tool pending state is left intact so the
            // gesture itself isn't cancelled (use right-click / tool
            // Esc for that).
            if editor.state.placement_input.is_some() {
                editor.state.placement_input = None;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintSketchSelect { id, shift } => {
            // None clears both selection slots. Some(id) without
            // shift replaces primary; with shift adds to secondary
            // (or replaces secondary with the new id).
            match (id, shift) {
                (None, _) => {
                    editor.state.selected_sketch = None;
                    editor.state.selected_sketch_secondary = None;
                }
                (Some(new_id), false) => {
                    editor.state.selected_sketch = Some(new_id);
                    editor.state.selected_sketch_secondary = None;
                }
                (Some(new_id), true) => {
                    if editor.state.selected_sketch.is_none() {
                        editor.state.selected_sketch = Some(new_id);
                    } else if editor.state.selected_sketch != Some(new_id) {
                        editor.state.selected_sketch_secondary = Some(new_id);
                    }
                }
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchMovePoint { id, dx, dy } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(
                    state,
                    primitive,
                    SketchEdit::MovePoint { id, dx, dy },
                );
            });
            // v0.16.0.1 fix — when the dragged Point is a pad's
            // centre, also translate that pad's outline-corner Points
            // by the same delta so the construction outline tracks
            // the pad. Without this the corner outline was stranded
            // at the previous centre after a sketch-mode drag.
            let centre_pad_idx = editor
                .state
                .pads
                .iter()
                .position(|p| p.sketch_entity_id == Some(id));
            if let Some(pad_idx) = centre_pad_idx {
                if let Some(corners) = editor.state.pads[pad_idx].corner_entity_ids {
                    for corner_id in corners {
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::MovePoint {
                                    id: corner_id,
                                    dx,
                                    dy,
                                },
                            );
                        });
                    }
                }
                // Keep `EditorPad.position_mm` in sync so a Pads-mode
                // tab switch shows the pad at the new world position.
                editor.state.pads[pad_idx].position_mm.0 += dx;
                editor.state.pads[pad_idx].position_mm.1 += dy;
                editor.with_parts(|state, primitive| {
                    CanvasState::sync_pads_to_primitive(state, primitive);
                });
            }
            // v0.16.1 fix — when the dragged Point is one of any
            // pad's outline-corner Points, recompute that pad's bbox
            // from all 4 corner positions. Update the pad's
            // position_mm + size_mm AND rewrite the centre Point's
            // PadAttr.size_x_expr / size_y_expr so the bake re-emits
            // the pad at the new size. This is the "drag-corner-to-
            // resize-pad" behaviour the user expects when they grab
            // a corner of the construction outline.
            let corner_pad_idx = editor.state.pads.iter().position(|p| {
                p.corner_entity_ids
                    .as_ref()
                    .map(|ids| ids.contains(&id))
                    .unwrap_or(false)
            });
            if let Some(pad_idx) = corner_pad_idx {
                use signex_sketch::entity::EntityKind;
                let Some(corners) = editor.state.pads[pad_idx].corner_entity_ids else {
                    // `position()` above already required `is_some()`; this
                    // arm is unreachable in practice but propagating via
                    // early-let-else avoids the matching `.unwrap()` panic
                    // if a future refactor decouples the two.
                    return;
                };
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                if let Some(sketch) = editor.primitive().sketch.as_ref() {
                    for cid in corners {
                        if let Some(e) = sketch.entities.iter().find(|e| e.id == cid) {
                            if let EntityKind::Point { x, y } = e.kind {
                                if x < min_x {
                                    min_x = x;
                                }
                                if y < min_y {
                                    min_y = y;
                                }
                                if x > max_x {
                                    max_x = x;
                                }
                                if y > max_y {
                                    max_y = y;
                                }
                            }
                        }
                    }
                }
                if min_x.is_finite() && min_y.is_finite() {
                    let new_w = (max_x - min_x).max(0.05);
                    let new_h = (max_y - min_y).max(0.05);
                    let new_cx = (min_x + max_x) / 2.0;
                    let new_cy = (min_y + max_y) / 2.0;
                    let pad = &mut editor.state.pads[pad_idx];
                    let old_centre = pad.position_mm;
                    pad.position_mm = (new_cx, new_cy);
                    pad.size_mm = (new_w, new_h);
                    let centre_id = pad.sketch_entity_id;
                    // v0.18.12.1 bugfix — re-align the OTHER three
                    // corner Points to the new pad bbox. Previously
                    // only the dragged corner moved, leaving the
                    // pad rectangle (drawn at the new bbox) and the
                    // non-dragged corners stranded at their old
                    // positions — visible as the dashed-construction
                    // outline drifting off the rendered pad on
                    // subsequent corner drags.
                    let new_positions: [(f64, f64); 4] = [
                        (max_x, min_y), // ne
                        (max_x, max_y), // se
                        (min_x, max_y), // sw
                        (min_x, min_y), // nw
                    ];
                    for (corner_id, (target_x, target_y)) in
                        corners.iter().zip(new_positions.iter())
                    {
                        // Skip the corner the user just dragged — it's
                        // already at the right position, and emitting
                        // a zero-delta MovePoint would still trip the
                        // solver.
                        if *corner_id == id {
                            continue;
                        }
                        let cur = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == *corner_id))
                            .and_then(|e| {
                                if let signex_sketch::entity::EntityKind::Point { x, y } = e.kind {
                                    Some((x, y))
                                } else {
                                    None
                                }
                            });
                        if let Some((cx, cy)) = cur {
                            let cdx = *target_x - cx;
                            let cdy = *target_y - cy;
                            if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::MovePoint {
                                            id: *corner_id,
                                            dx: cdx,
                                            dy: cdy,
                                        },
                                    );
                                });
                            }
                        }
                    }
                    // Move the centre Point to the new bbox centre +
                    // rewrite the PadAttr size exprs so the bake
                    // emits the new size.
                    if let Some(centre_id) = centre_id {
                        let cdx = new_cx - old_centre.0;
                        let cdy = new_cy - old_centre.1;
                        if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::MovePoint {
                                        id: centre_id,
                                        dx: cdx,
                                        dy: cdy,
                                    },
                                );
                            });
                        }
                        if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                            if let Some(centre) =
                                sketch.entities.iter_mut().find(|e| e.id == centre_id)
                            {
                                if let Some(attr) = centre.pad.as_mut() {
                                    attr.size_x_expr = format!("{:.4}mm", new_w);
                                    attr.size_y_expr = format!("{:.4}mm", new_h);
                                }
                            }
                        }
                    }
                    editor.with_parts(|state, primitive| {
                        CanvasState::sync_pads_to_primitive(state, primitive);
                    });
                }
            }
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchMoveLine { id, dx, dy } => {
            // v0.27 — drag a Line edge by translating both its
            // endpoints in one solver pass. The dispatcher reads
            // the Line's start/end IDs, then emits MovePoint for
            // each. The solver re-runs once after both moves so
            // H/V/Distance constraints converge correctly without
            // the brief mid-tick "one corner moved, the other
            // didn't" state a two-message split would produce.
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use signex_sketch::entity::EntityKind;
            let endpoints = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == id))
                .and_then(|e| match e.kind {
                    EntityKind::Line { start, end } => Some((start, end)),
                    _ => None,
                });
            let Some((start, end)) = endpoints else {
                return;
            };
            // v0.27 — snapshot the line's pre-drag endpoint positions.
            // Used after the translate step to detect which bbox edge
            // the line lay on (so the matching pad can resize). Read
            // BEFORE the MovePoint passes shift these Points.
            let pre_drag_endpoints: Option<((f64, f64), (f64, f64))> = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| {
                    let pos_of = |pid: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
                        s.entities.iter().find(|e| e.id == pid).and_then(|e| {
                            if let EntityKind::Point { x, y } = e.kind {
                                Some((x, y))
                            } else {
                                None
                            }
                        })
                    };
                    pos_of(start).zip(pos_of(end))
                });
            // v0.27 — gather the arc victim set BEFORE running any
            // moves so adjacency lookups read pre-drag positions.
            // Arc centres + the "other" tangent endpoint of any Arc
            // tangent to a moving line endpoint translate by the
            // same `(dx, dy)` as the rigid edge so rounded-rectangle
            // corners stay rigid (constant radius). The line's own
            // endpoints are handled separately below — they may
            // slide along an adjacent edge rather than translating
            // rigidly (Fusion-style "expand toward dragging").
            let mut arc_victims: std::collections::HashSet<signex_sketch::id::SketchEntityId> =
                std::collections::HashSet::new();
            if let Some(s) = editor.primitive().sketch.as_ref() {
                for e in &s.entities {
                    if let EntityKind::Arc {
                        start: a_s,
                        end: a_e,
                        center: a_c,
                        ..
                    } = e.kind
                    {
                        let touches = a_s == start
                            || a_s == end
                            || a_e == start
                            || a_e == end;
                        if touches {
                            arc_victims.insert(a_c);
                            if a_s != start && a_s != end {
                                arc_victims.insert(a_s);
                            }
                            if a_e != start && a_e != end {
                                arc_victims.insert(a_e);
                            }
                        }
                    }
                }
            }
            // v0.27 — per-endpoint slide. If the endpoint connects
            // to exactly one OTHER line (closed polygon vertex),
            // slide the endpoint along that adjacent line so the
            // dragged edge only stretches/shrinks perpendicular and
            // the adjacent edges retain their direction. The pad
            // bbox case still produces the right answer here because
            // a rect pad's edge endpoints connect to perpendicular
            // edges — sliding along a perpendicular line by the
            // perpendicular drag delta is equivalent to translating.
            // When the endpoint has zero or ≥2 other lines (free
            // vertex, arc tangent, T-junction), fall back to rigid
            // translate so the existing pad / arc-corner flows keep
            // working.
            let read_pos = |pid: signex_sketch::id::SketchEntityId| -> Option<(f64, f64)> {
                editor
                    .primitive()
                    .sketch
                    .as_ref()
                    .and_then(|s| s.entities.iter().find(|e| e.id == pid))
                    .and_then(|e| match e.kind {
                        EntityKind::Point { x, y } => Some((x, y)),
                        _ => None,
                    })
            };
            // Find the unique other line connected to `endpoint`
            // (excluding the dragged line itself). Returns the far
            // endpoint of that line — the one we treat as the
            // slide pivot. Returns `None` when 0 or ≥2 other lines
            // meet at this endpoint.
            let find_far = |endpoint: signex_sketch::id::SketchEntityId|
                -> Option<signex_sketch::id::SketchEntityId> {
                let sketch = editor.primitive().sketch.as_ref()?;
                let mut found: Option<signex_sketch::id::SketchEntityId> = None;
                for e in &sketch.entities {
                    if e.id == id {
                        continue;
                    }
                    if let EntityKind::Line { start: ls, end: le } = e.kind {
                        let far = if ls == endpoint {
                            Some(le)
                        } else if le == endpoint {
                            Some(ls)
                        } else {
                            None
                        };
                        if let Some(f) = far {
                            if found.is_some() {
                                return None;
                            }
                            found = Some(f);
                        }
                    }
                }
                found
            };
            // 2D line-line intersection. `p1 + t*d1 = p2 + s*d2`.
            // Returns `None` for parallel / coincident lines.
            let intersect = |p1: (f64, f64),
                             d1: (f64, f64),
                             p2: (f64, f64),
                             d2: (f64, f64)|
             -> Option<(f64, f64)> {
                let det = d2.0 * d1.1 - d1.0 * d2.1;
                if det.abs() < 1e-9 {
                    return None;
                }
                let t = (d2.0 * (p2.1 - p1.1) - d2.1 * (p2.0 - p1.0)) / det;
                Some((p1.0 + t * d1.0, p1.1 + t * d1.1))
            };
            let target_for =
                |endpoint: signex_sketch::id::SketchEntityId, pos: (f64, f64)| -> (f64, f64) {
                    let rigid = (pos.0 + dx, pos.1 + dy);
                    let Some(far_id) = find_far(endpoint) else {
                        return rigid;
                    };
                    let Some(far_pos) = read_pos(far_id) else {
                        return rigid;
                    };
                    let Some((sx_pre, sy_pre)) = read_pos(start) else {
                        return rigid;
                    };
                    let Some((ex_pre, ey_pre)) = read_pos(end) else {
                        return rigid;
                    };
                    let line_d = (ex_pre - sx_pre, ey_pre - sy_pre);
                    let other_d = (pos.0 - far_pos.0, pos.1 - far_pos.1);
                    intersect(rigid, line_d, far_pos, other_d).unwrap_or(rigid)
                };
            let start_pos_opt = read_pos(start);
            let end_pos_opt = read_pos(end);
            if let (Some(start_pos), Some(end_pos)) = (start_pos_opt, end_pos_opt) {
                let start_target = target_for(start, start_pos);
                let end_target = target_for(end, end_pos);
                let start_delta = (start_target.0 - start_pos.0, start_target.1 - start_pos.1);
                let end_delta = (end_target.0 - end_pos.0, end_target.1 - end_pos.1);
                if start_delta.0.abs() > 1e-9 || start_delta.1.abs() > 1e-9 {
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::MovePoint {
                                id: start,
                                dx: start_delta.0,
                                dy: start_delta.1,
                            },
                        );
                    });
                }
                if end_delta.0.abs() > 1e-9 || end_delta.1.abs() > 1e-9 {
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::MovePoint {
                                id: end,
                                dx: end_delta.0,
                                dy: end_delta.1,
                            },
                        );
                    });
                }
            }
            for pid in arc_victims {
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(
                        state,
                        primitive,
                        SketchEdit::MovePoint { id: pid, dx, dy },
                    );
                });
            }
            // v0.27 — propagate the edge drag to the literal pad
            // bbox. Without this the sketch outline visibly resizes
            // but `pad.size_mm` / `pad.position_mm` (and the baked
            // pad rendering) stay frozen — the user sees the line
            // move while the pad copper underneath does nothing.
            //
            // Strategy: classify the line's pre-drag pose against
            // each pad's bbox to identify which side it lies on
            // (top / bottom / left / right). Only axis-aligned lines
            // qualify — diagonal sketch lines are never pad edges
            // for Rect / RoundRect / Oval / Chamfered shapes.
            const EDGE_EPS: f64 = 1e-4;
            if let Some(((sx, sy), (ex, ey))) = pre_drag_endpoints {
                let is_horizontal = (sy - ey).abs() < EDGE_EPS;
                let is_vertical = (sx - ex).abs() < EDGE_EPS;
                let pad_count = editor.state.pads.len();
                for pad_idx in 0..pad_count {
                    let bbox_data = {
                        let pad = &editor.state.pads[pad_idx];
                        if pad.corner_entity_ids.is_none() {
                            continue;
                        }
                        let (xmin, ymin, xmax, ymax) = pad.bbox_mm();
                        // Both endpoints must lie on the same bbox
                        // side; partial overlap (line extends past a
                        // corner) means it's not a pad edge.
                        let in_x = sx >= xmin - EDGE_EPS
                            && sx <= xmax + EDGE_EPS
                            && ex >= xmin - EDGE_EPS
                            && ex <= xmax + EDGE_EPS;
                        let in_y = sy >= ymin - EDGE_EPS
                            && sy <= ymax + EDGE_EPS
                            && ey >= ymin - EDGE_EPS
                            && ey <= ymax + EDGE_EPS;
                        if !in_x || !in_y {
                            continue;
                        }
                        let edge: Option<&str> = if is_horizontal
                            && (sy - ymin).abs() < EDGE_EPS
                        {
                            Some("top")
                        } else if is_horizontal && (sy - ymax).abs() < EDGE_EPS {
                            Some("bottom")
                        } else if is_vertical && (sx - xmin).abs() < EDGE_EPS {
                            Some("left")
                        } else if is_vertical && (sx - xmax).abs() < EDGE_EPS {
                            Some("right")
                        } else {
                            None
                        };
                        let Some(edge) = edge else {
                            continue;
                        };
                        let (new_xmin, new_ymin, new_xmax, new_ymax) = match edge {
                            "top" => (xmin, ymin + dy, xmax, ymax),
                            "bottom" => (xmin, ymin, xmax, ymax + dy),
                            "left" => (xmin + dx, ymin, xmax, ymax),
                            "right" => (xmin, ymin, xmax + dx, ymax),
                            _ => unreachable!(),
                        };
                        // Reject degenerate drags that would collapse
                        // or invert the bbox. The user has to release
                        // and re-grab if they want sub-50µm pads.
                        if new_xmax - new_xmin < 0.05 || new_ymax - new_ymin < 0.05 {
                            continue;
                        }
                        Some((new_xmin, new_ymin, new_xmax, new_ymax))
                    };
                    let Some((new_xmin, new_ymin, new_xmax, new_ymax)) = bbox_data else {
                        continue;
                    };
                    let new_w = new_xmax - new_xmin;
                    let new_h = new_ymax - new_ymin;
                    let new_cx = (new_xmin + new_xmax) / 2.0;
                    let new_cy = (new_ymin + new_ymax) / 2.0;
                    let (corners_arr, centre_id) = {
                        let pad = &editor.state.pads[pad_idx];
                        (
                            pad.corner_entity_ids
                                .expect("checked is_some above"),
                            pad.sketch_entity_id,
                        )
                    };
                    // Rewrite the centre Point's PadAttr size exprs
                    // FIRST so the next solve+bake reads the new
                    // size. solve_and_bake → refresh_pads_from_primitive
                    // overwrites state.pads.size_mm from the bake
                    // output, so any earlier write here gets wiped.
                    // Updating PadAttr ahead of the solve makes the
                    // bake produce the resized pad on its own.
                    if let Some(centre_id) = centre_id
                        && let Some(sketch) = editor.primitive_mut().sketch.as_mut()
                        && let Some(centre) =
                            sketch.entities.iter_mut().find(|e| e.id == centre_id)
                        && let Some(attr) = centre.pad.as_mut()
                    {
                        attr.size_x_expr = format!("{:.4}mm", new_w);
                        attr.size_y_expr = format!("{:.4}mm", new_h);
                    }
                    // Move the centre Point to the new bbox centre.
                    // Each apply_sketch_edit_with_warnings runs the
                    // solver + bake; refresh_pads_from_primitive then
                    // pulls state.pads from `footprint.pads`, so
                    // reading the centre's pre-edit position needs to
                    // happen RIGHT BEFORE this MovePoint emission.
                    if let Some(centre_id) = centre_id {
                        let cur_centre = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == centre_id))
                            .and_then(|e| {
                                if let EntityKind::Point { x, y } = e.kind {
                                    Some((x, y))
                                } else {
                                    None
                                }
                            });
                        if let Some((cur_cx, cur_cy)) = cur_centre {
                            let cdx = new_cx - cur_cx;
                            let cdy = new_cy - cur_cy;
                            if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::MovePoint {
                                            id: centre_id,
                                            dx: cdx,
                                            dy: cdy,
                                        },
                                    );
                                });
                            }
                        }
                    }
                    // Realign the 4 bbox corner Points to match the
                    // resized bbox. For Rect pads the line drag's
                    // victim loop already shifted the affected
                    // corners; for RoundRect / Oval / Chamfered the
                    // bbox corners aren't in `victims` so they need
                    // explicit catch-up here. Order: [ne, se, sw, nw]
                    // — see mint_pad_corner_outline.
                    let target_positions: [(f64, f64); 4] = [
                        (new_xmax, new_ymin), // ne
                        (new_xmax, new_ymax), // se
                        (new_xmin, new_ymax), // sw
                        (new_xmin, new_ymin), // nw
                    ];
                    for (corner_id, (target_x, target_y)) in
                        corners_arr.iter().zip(target_positions.iter())
                    {
                        let cur = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == *corner_id))
                            .and_then(|e| {
                                if let EntityKind::Point { x, y } = e.kind {
                                    Some((x, y))
                                } else {
                                    None
                                }
                            });
                        if let Some((cx, cy)) = cur {
                            let cdx = *target_x - cx;
                            let cdy = *target_y - cy;
                            if cdx.abs() > 1e-9 || cdy.abs() > 1e-9 {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::MovePoint {
                                            id: *corner_id,
                                            dx: cdx,
                                            dy: cdy,
                                        },
                                    );
                                });
                            }
                        }
                    }
                }
            }
            editor.with_parts(|state, primitive| {
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchResizeRoundPad {
            pad_idx,
            diameter_mm,
        } => {
            // v0.27 — round-pad diameter handle drag. Update three
            // sources of truth in lockstep so the on-canvas handle
            // motion, the bake output, and the parameter table stay
            // consistent:
            //   1. `pad.size_mm = (d, d)` — Editor mirror of the bbox.
            //   2. Circle entity radius — sketch-side geometry the
            //      Sketch overlay renders.
            //   3. `diameter_<slug>` parameter expression + the
            //      centre Point's PadAttr size_x_expr / size_y_expr —
            //      the bake reads these to emit the baked pad.
            let d = diameter_mm.max(0.05);
            let centre_id = editor.state.pads.get(pad_idx).and_then(|p| p.sketch_entity_id);
            let diameter_param =
                editor.state.pads.get(pad_idx).and_then(|p| {
                    p.shape_params.get("diameter").cloned()
                });
            if let Some(pad) = editor.state.pads.get_mut(pad_idx) {
                pad.size_mm = (d, d);
            }
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                use signex_sketch::entity::EntityKind;
                if let Some(cid) = centre_id {
                    for entity in sketch.entities.iter_mut() {
                        if let EntityKind::Circle { center, radius } = &mut entity.kind {
                            if *center == cid {
                                *radius = d / 2.0;
                            }
                        }
                        if entity.id == cid {
                            if let Some(attr) = entity.pad.as_mut() {
                                attr.size_x_expr = format!("{:.4}mm", d);
                                attr.size_y_expr = format!("{:.4}mm", d);
                            }
                        }
                    }
                }
                if let Some(name) = diameter_param.as_deref() {
                    sketch
                        .parameters
                        .insert(name.to_string(), format!("{:.4}mm", d));
                }
            }
            editor.with_parts(|state, primitive| {
                CanvasState::sync_pads_to_primitive(state, primitive);
            });
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSetSelectionMode2d(mode) => {
            // v0.27 — active-bar Selection picker rows. The rubber-
            // band release picker reads this on commit so Inside /
            // Touching / Outside semantics apply.
            editor.state.selection_mode_2d = mode;
            editor.state.active_bar_menu = None;
        }
        PrimitiveEditorMsg::FootprintSelectAllOnLayer => {
            // v0.27 — multi-select every pad on the active layer.
            // Active layer = layer of the currently-selected pad,
            // or F.Cu when nothing is selected.
            let layer = editor
                .state
                .selected_pad
                .and_then(|idx| editor.state.pads.get(idx))
                .map(|p| p.primary_layer())
                .unwrap_or(crate::library::editor::footprint::layers::FpLayer::FCu);
            let mut matches: Vec<usize> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .filter_map(|(idx, p)| {
                    if p.primary_layer() == layer {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            if matches.is_empty() {
                editor.state.selected_pad = None;
                editor.state.selected_pads_extra.clear();
            } else {
                editor.state.selected_pad = Some(matches.remove(0));
                editor.state.selected_pads_extra = matches;
            }
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintLassoArm => {
            editor.state.lasso_mode_active = true;
            editor.state.lasso_vertices.clear();
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintLassoAddVertex { x_mm, y_mm } => {
            if editor.state.lasso_mode_active {
                editor.state.lasso_vertices.push((x_mm, y_mm));
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintLassoCancel => {
            editor.state.lasso_mode_active = false;
            editor.state.lasso_vertices.clear();
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintLassoCommit => {
            // v0.27 — close the polygon, multi-select every pad whose
            // centre lies inside (even-odd ray casting). Anything
            // less than three vertices is a degenerate polygon and
            // commits as deselect-all so a stray click doesn't leave
            // the user stuck in lasso mode with no feedback.
            let verts: Vec<(f64, f64)> = std::mem::take(&mut editor.state.lasso_vertices);
            editor.state.lasso_mode_active = false;
            let in_poly = |px: f64, py: f64| -> bool {
                if verts.len() < 3 {
                    return false;
                }
                let mut inside = false;
                let n = verts.len();
                let mut j = n - 1;
                for i in 0..n {
                    let (xi, yi) = verts[i];
                    let (xj, yj) = verts[j];
                    let denom = yj - yi;
                    if denom.abs() < 1e-10 {
                        j = i;
                        continue;
                    }
                    let intersect = ((yi > py) != (yj > py))
                        && (px < (xj - xi) * (py - yi) / denom + xi);
                    if intersect {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            };
            let mut hits: Vec<usize> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .filter_map(|(idx, p)| {
                    if in_poly(p.position_mm.0, p.position_mm.1) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            if hits.is_empty() {
                editor.state.selected_pad = None;
                editor.state.selected_pads_extra.clear();
            } else {
                editor.state.selected_pad = Some(hits.remove(0));
                editor.state.selected_pads_extra = hits;
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintTouchingLineArm => {
            editor.state.touching_line_active = true;
            editor.state.touching_line_first = None;
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintTouchingLineFirst { x_mm, y_mm } => {
            if editor.state.touching_line_active {
                editor.state.touching_line_first = Some((x_mm, y_mm));
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintTouchingLineCancel => {
            editor.state.touching_line_active = false;
            editor.state.touching_line_first = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintTouchingLineCommit { x_mm, y_mm } => {
            // v0.27 — Touching Line: every pad whose bbox is
            // intersected by the segment from `touching_line_first`
            // → (x_mm, y_mm) becomes selected. Liang-Barsky-style
            // segment-vs-AABB clip.
            let Some((sx, sy)) = editor.state.touching_line_first.take() else {
                editor.state.touching_line_active = false;
                editor.canvas_cache.clear();
                return;
            };
            editor.state.touching_line_active = false;
            let dx = x_mm - sx;
            let dy = y_mm - sy;
            let segment_hits_aabb = |xmin: f64, ymin: f64, xmax: f64, ymax: f64| -> bool {
                // Both endpoints inside?
                let inside = |x: f64, y: f64| -> bool {
                    x >= xmin && x <= xmax && y >= ymin && y <= ymax
                };
                if inside(sx, sy) || inside(x_mm, y_mm) {
                    return true;
                }
                // Liang-Barsky parametric clip in [0, 1].
                let mut t_enter = 0.0_f64;
                let mut t_exit = 1.0_f64;
                let p = [-dx, dx, -dy, dy];
                let q = [sx - xmin, xmax - sx, sy - ymin, ymax - sy];
                for i in 0..4 {
                    if p[i].abs() < 1e-12 {
                        if q[i] < 0.0 {
                            return false;
                        }
                    } else {
                        let t = q[i] / p[i];
                        if p[i] < 0.0 {
                            if t > t_exit {
                                return false;
                            }
                            if t > t_enter {
                                t_enter = t;
                            }
                        } else {
                            if t < t_enter {
                                return false;
                            }
                            if t < t_exit {
                                t_exit = t;
                            }
                        }
                    }
                }
                t_enter <= t_exit
            };
            let mut hits: Vec<usize> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .filter_map(|(idx, p)| {
                    let (x0, y0, x1, y1) = p.bbox_mm();
                    if segment_hits_aabb(x0, y0, x1, y1) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            if hits.is_empty() {
                editor.state.selected_pad = None;
                editor.state.selected_pads_extra.clear();
            } else {
                editor.state.selected_pad = Some(hits.remove(0));
                editor.state.selected_pads_extra = hits;
            }
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSelectOverlapped
        | PrimitiveEditorMsg::FootprintSelectNextOverlapped => {
            // v0.27 — Cycle through pads stacked at the most recent
            // click world position. SelectOverlapped goes to the
            // previous pad in z-order; SelectNextOverlapped advances.
            // Without a recorded click position there's no stack to
            // cycle, so the action is a silent no-op.
            let forward = matches!(msg, PrimitiveEditorMsg::FootprintSelectNextOverlapped);
            let Some((wx, wy)) = editor.state.last_click_world_mm else {
                editor.state.active_bar_menu = None;
                return;
            };
            let stack: Vec<usize> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .filter_map(|(idx, p)| if p.contains_mm(wx, wy) { Some(idx) } else { None })
                .collect();
            if stack.is_empty() {
                editor.state.active_bar_menu = None;
                return;
            }
            // Pick next/prev relative to the current primary selection.
            let cur_pos = editor
                .state
                .selected_pad
                .and_then(|s| stack.iter().position(|&i| i == s));
            let next_idx = match cur_pos {
                Some(p) => {
                    if forward {
                        (p + 1) % stack.len()
                    } else {
                        (p + stack.len() - 1) % stack.len()
                    }
                }
                None => 0,
            };
            editor.state.selected_pad = Some(stack[next_idx]);
            editor.state.selected_pads_extra.clear();
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintRecomputeCourtyardOutline => {
            // v0.27 — outline-following courtyard. Pure read-write
            // on the editor state; the new polygon lands on
            // `state.courtyard_outline_mm` and the canvas draws it
            // in preference to the bbox.
            editor.state.recompute_courtyard_outline();
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSelectOffGridPads => {
            // v0.27 — pads whose centre falls between grid steps.
            // The active grid step lives on snap_options; defaults
            // to 1 mm. Tolerance is 1% of the step so pads exactly
            // on the grid (with floating-point noise) don't
            // false-positive.
            let step = editor.state.snap_options.grid_step_mm.max(1e-6);
            let tol = step * 0.01;
            let on_grid = |v: f64| -> bool {
                let r = (v / step).round() * step;
                (v - r).abs() <= tol
            };
            let mut matches: Vec<usize> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .filter_map(|(idx, p)| {
                    let (x, y) = p.position_mm;
                    if !on_grid(x) || !on_grid(y) {
                        Some(idx)
                    } else {
                        None
                    }
                })
                .collect();
            if matches.is_empty() {
                editor.state.selected_pad = None;
                editor.state.selected_pads_extra.clear();
            } else {
                editor.state.selected_pad = Some(matches.remove(0));
                editor.state.selected_pads_extra = matches;
            }
            editor.state.active_bar_menu = None;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchDimensionInput(s) => {
            editor.state.dimension_input = s;
        }
        PrimitiveEditorMsg::FootprintSketchSetRole { id, role } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_role_with_warnings;
            use crate::library::editor::footprint::state::EditorPad;
            use signex_library::primitive::footprint::{
                LayerId, PadKind as LibPadKind, PadShape as LibPadShape,
            };

            // v0.27 — the Role=Pad-on-a-Line case is rewritten to
            // MakePadFromProfile at the top of
            // `apply_footprint_primitive_edit`, so this arm only
            // sees Point-targeted Pad assignments + every other
            // role. PadAttr is Point-only on the schema side, so
            // dispatching to `apply_sketch_role_with_warnings` is
            // always meaningful from here on.
            editor.with_parts(|state, primitive| {
                apply_sketch_role_with_warnings(state, primitive, id, role);
            });

            // Diff `state.pads` against the entity's new role so the
            // canvas's pad list mirrors role assignments. Per-entity
            // diff (rather than full rebuild from `primitive.pads`)
            // preserves `sketch_entity_id` + `corner_entity_ids` on
            // previously auto-minted Pads-mode pads.
            let entity_has_pad = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.entities.iter().find(|e| e.id == id))
                .map(|e| e.pad.is_some())
                .unwrap_or(false);
            let existing_idx = editor
                .state
                .pads
                .iter()
                .position(|p| p.sketch_entity_id == Some(id));
            match (entity_has_pad, existing_idx) {
                (true, None) => {
                    use signex_sketch::entity::EntityKind;
                    let (x, y, number) = editor
                        .primitive()
                        .sketch
                        .as_ref()
                        .and_then(|s| s.entities.iter().find(|e| e.id == id))
                        .map(|e| {
                            let (x, y) = match e.kind {
                                EntityKind::Point { x, y } => (x, y),
                                _ => (0.0, 0.0),
                            };
                            let num = e.pad.as_ref().map(|a| a.number.clone()).unwrap_or_default();
                            (x, y, num)
                        })
                        .unwrap_or((0.0, 0.0, String::new()));
                    editor.state.pads.push(EditorPad {
                        number,
                        position_mm: (x, y),
                        size_mm: (1.0, 1.0),
                        kind: LibPadKind::Smd,
                        shape: LibPadShape::Rect,
                        layers: vec![LayerId::new("Top Layer")],
                        sketch_entity_id: Some(id),
                        corner_entity_ids: None,
                        rotation_deg: 0.0,
                        drill_diameter_mm: None,
                        stack: crate::library::editor::footprint::state::PadStackUi::default(),
                        feature_top: signex_sketch::attr::PadFeature::None,
                        feature_bottom: signex_sketch::attr::PadFeature::None,
                        testpoint: signex_sketch::attr::TestpointFlags::default(),
                        template: String::new(),
                        template_library: String::new(),
                        electrical_type: signex_sketch::attr::ElectricalType::Load,
                        net: String::new(),
                        locked: false,
                        hole_tolerance_plus_mm: None,
                        hole_tolerance_minus_mm: None,
                        hole_rotation_deg: None,
                        copper_offset_x_mm: None,
                        copper_offset_y_mm: None,
                        shape_params: crate::library::editor::footprint::state::ShapeParamMap::new(),
                    });
                }
                (false, Some(idx)) => {
                    editor.state.pads.remove(idx);
                    if editor.state.selected_pad == Some(idx) {
                        editor.state.selected_pad = None;
                    } else if let Some(sel) = editor.state.selected_pad {
                        if sel > idx {
                            editor.state.selected_pad = Some(sel - 1);
                        }
                    }
                }
                _ => {}
            }
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        PrimitiveEditorMsg::FootprintSketchMakePadFromProfile => {
            // v0.22 Phase D4 — convert the closed-loop profile that
            // includes the currently-selected Line into a
            // `PadShape::Custom(SketchProfile)` pad.
            //
            // Walk: start from the selected Line, use
            // `signex_bake::profile::trace_closed_profile` to chase
            // the unique-incident-edge cycle in the sketch. On
            // success, compute the centroid of the traced vertices,
            // mint a centre `Point` there, and attach a `PadAttr`
            // whose `shape` is `Custom(SketchProfile{source: vec![
            // seed_line_id]})`. The bake re-walks the loop on the
            // next solve and emits a `LibPadShape::Custom` polygon.
            //
            // Designator: `next_pad_num` from existing `PadAttr`
            // entities, identical pattern to
            // `apply_sketch_role(.., RoleTag::Pad)` for ordering
            // consistency.
            //
            // Fail modes (silent except for warning push):
            // - No Line selected → "select a Line first".
            // - Line is not part of a closed loop → "loop is open
            //   or branches".
            // - `last_solve` is None (no solve has run yet) → ask
            //   user to interact briefly so a solve fires, then
            //   retry. (Auto-mint paths on entry to Sketch mode
            //   already trigger a solve, so this is rare.)
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use signex_sketch::attr::{
                CustomPadShape, PadAttr, PadKind, PadShape, PadSide,
                PasteAperturePattern,
            };
            use signex_sketch::entity::{Entity, EntityKind};
            use signex_sketch::id::SketchEntityId;
            use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

            // v0.27 — walk the full sketch selection (primary +
            // secondary + extras) for the first Line. The
            // closed-loop walker doesn't care which seed edge it
            // gets — any Line on the loop seeds the trace. Falls
            // back to scanning every sketch Line when nothing
            // suitable is selected, so the action also works on a
            // bare "select-nothing-and-click-Make-Pad" workflow.
            let line_id: SketchEntityId = {
                let sketch_lookup = editor.primitive().sketch.as_ref();
                let kind_of = |id: SketchEntityId| -> Option<EntityKind> {
                    sketch_lookup
                        .and_then(|s| s.entities.iter().find(|e| e.id == id))
                        .map(|e| e.kind.clone())
                };
                let selection: Vec<SketchEntityId> = editor
                    .state
                    .selected_sketch
                    .into_iter()
                    .chain(editor.state.selected_sketch_secondary.into_iter())
                    .chain(editor.state.selected_sketch_extra.iter().copied())
                    .collect();

                // First pass — Line directly in the selection.
                let direct_line = selection
                    .iter()
                    .find(|id| matches!(kind_of(**id), Some(EntityKind::Line { .. })))
                    .copied();
                // Second pass — a selected Point's incident Line.
                let incident_line = selection.iter().find_map(|id| {
                    if matches!(kind_of(*id), Some(EntityKind::Point { .. })) {
                        sketch_lookup.and_then(|s| {
                            s.entities
                                .iter()
                                .find(|e| match e.kind {
                                    EntityKind::Line { start, end } => {
                                        start == *id || end == *id
                                    }
                                    _ => false,
                                })
                                .map(|e| e.id)
                        })
                    } else {
                        None
                    }
                });
                // Third pass — any sketch Line at all.
                let any_line = sketch_lookup.and_then(|s| {
                    s.entities
                        .iter()
                        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
                        .map(|e| e.id)
                });

                match direct_line.or(incident_line).or(any_line) {
                    Some(id) => id,
                    None => {
                        editor.state.solve_warnings.push(
                            "Make Pad from Profile: no Lines in the sketch — draw a closed shape first"
                                .into(),
                        );
                        editor.canvas_cache.clear();
                        return;
                    }
                }
            };

            // Walk the loop to compute the centroid; needs a fresh
            // solve so vertex positions are accurate.
            let solve = match editor.state.last_solve.as_ref() {
                Some(s) => s,
                None => {
                    editor.state.solve_warnings.push(
                        "Make Pad from Profile: no solve has run yet — interact briefly to trigger a solve, then retry"
                            .into(),
                    );
                    editor.canvas_cache.clear();
                    return;
                }
            };
            let sketch_for_walk = match editor.primitive().sketch.as_ref() {
                Some(s) => s,
                None => return,
            };

            let trace =
                signex_bake::profile::trace_closed_profile(sketch_for_walk, solve, line_id);
            let vertices = match trace {
                Ok(v) if v.len() >= 3 => v,
                Ok(_) => {
                    editor.state.solve_warnings.push(
                        "Make Pad from Profile: traced loop has fewer than 3 vertices".into(),
                    );
                    editor.canvas_cache.clear();
                    return;
                }
                Err(e) => {
                    editor.state.solve_warnings.push(format!(
                        "Make Pad from Profile: loop walk failed ({e:?}) — the loop must be closed and non-branching"
                    ));
                    editor.canvas_cache.clear();
                    return;
                }
            };
            // v0.27 — area-weighted centroid + axis-aligned bbox of
            // the closed-loop polygon. The arithmetic mean of vertex
            // positions only matches the geometric centroid for
            // regular polygons; for an arbitrary triangle / outline
            // it lands biased toward whichever side has the most
            // densely-spaced vertices (which is why the user saw
            // the pad mint near a corner instead of inside the
            // shape). The shoelace centroid is the proper EDA
            // answer — pad sits at the geometric middle of its own
            // copper outline.
            let n_v = vertices.len();
            let mut signed_area = 0.0_f64;
            let mut cx_acc = 0.0_f64;
            let mut cy_acc = 0.0_f64;
            for i in 0..n_v {
                let (x0, y0) = (vertices[i][0], vertices[i][1]);
                let (x1, y1) = (
                    vertices[(i + 1) % n_v][0],
                    vertices[(i + 1) % n_v][1],
                );
                let cross = x0 * y1 - x1 * y0;
                signed_area += cross;
                cx_acc += (x0 + x1) * cross;
                cy_acc += (y0 + y1) * cross;
            }
            let area = signed_area * 0.5;
            let (cx, cy) = if area.abs() > 1e-12 {
                let s = 1.0 / (6.0 * area);
                (cx_acc * s, cy_acc * s)
            } else {
                // Degenerate polygon — fall back to mean.
                let n = n_v as f64;
                (
                    vertices.iter().map(|p| p[0]).sum::<f64>() / n,
                    vertices.iter().map(|p| p[1]).sum::<f64>() / n,
                )
            };
            // Axis-aligned bbox — drives `size_x_expr` / `size_y_expr`
            // so the synced `state.pads` row is at least bbox-sized
            // (instead of the default 1mm × 1mm). Polygon-shape
            // rendering on the editor canvas is a follow-up.
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for p in &vertices {
                if p[0] < min_x {
                    min_x = p[0];
                }
                if p[1] < min_y {
                    min_y = p[1];
                }
                if p[0] > max_x {
                    max_x = p[0];
                }
                if p[1] > max_y {
                    max_y = p[1];
                }
            }
            let bbox_w = (max_x - min_x).max(0.05);
            let bbox_h = (max_y - min_y).max(0.05);

            // Plane: reuse the seed Line's plane so the new pad
            // entity ends up on the same one.
            let plane_id = sketch_for_walk
                .entities
                .iter()
                .find(|e| e.id == line_id)
                .map(|e| e.plane)
                .unwrap_or_else(|| {
                    sketch_for_walk
                        .planes
                        .first()
                        .map(|p| p.id)
                        .unwrap_or_else(PlaneId::new)
                });
            // Ensure plane exists (defensive — almost always already
            // in `sketch.planes`).
            let _ = Plane {
                id: plane_id,
                kind: PlaneKind::BoardTop,
            };

            // Next pad designator from existing pad attrs.
            let next_pad_num = sketch_for_walk
                .entities
                .iter()
                .filter_map(|e| e.pad.as_ref())
                .filter_map(|attr| attr.number.parse::<u32>().ok())
                .max()
                .unwrap_or(0)
                + 1;

            let centre_id = SketchEntityId::new();
            let mut centre = Entity::new(
                centre_id,
                plane_id,
                EntityKind::Point { x: cx, y: cy },
            );
            centre.pad = Some(PadAttr {
                number: next_pad_num.to_string(),
                kind: PadKind::Smd,
                side: PadSide::Top,
                shape: PadShape::Custom(CustomPadShape::SketchProfile {
                    source: vec![line_id],
                }),
                size_x_expr: format!("{:.3}mm", bbox_w),
                size_y_expr: format!("{:.3}mm", bbox_h),
                rotation_expr: None,
                offset_x_expr: None,
                offset_y_expr: None,
                drill: None,
                mask_margin_expr: None,
                paste_margin_expr: None,
                paste_apertures: PasteAperturePattern::Single,
                ..PadAttr::default()
            });
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(
                    state,
                    primitive,
                    SketchEdit::AddEntity(centre),
                );
            });
            // v0.27 — pivot the selection onto the new pad's centre
            // Point. Without this, the Role dropdown still reads
            // "Unassigned" because the user's prior selection (the
            // Line we walked) has no PadAttr — the new PadAttr lives
            // on the freshly-minted centre. Clearing extras avoids
            // a confusing "primary is the centre but extras still
            // point at the loop's lines" state right after Make Pad.
            editor.state.selected_sketch = Some(centre_id);
            editor.state.selected_sketch_secondary = None;
            editor.state.selected_sketch_extra.clear();
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchUnlinkCornerRadius { arc_entity_id } => {
            // v0.24 Phase 3 (Track A3) — split a RoundRect pad's
            // shared `corner_r_<slug>` parameter into a per-corner
            // override for the right-clicked Arc.
            //
            // Lookup chain:
            //   1. Walk every EditorPad to find the one whose
            //      `shape_params` contains a `corner_r_<corner>_arc`
            //      key whose value (UUID slug) matches `arc_entity_id`.
            //   2. From that match, derive the corner key
            //      (`corner_r_ne` / `_se` / `_sw` / `_nw`).
            //   3. Mint a fresh parameter `<shared_name>_<corner>`,
            //      copy the current shared expression as its value,
            //      and bind the corner key on `pad.shape_params`.
            //   4. Trigger a `ForceRebuild` so the solver re-runs and
            //      the bake reflects the new parametric link.
            //
            // Defensive: arc not part of any pad → tracing::warn +
            // no-op. Pad has no shared `corner_r` binding (e.g.
            // legacy data) → tracing::warn + no-op.
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;

            let arc_id_str = arc_entity_id.0.simple().to_string();

            // Locate the pad + corner this arc belongs to.
            let pad_corner: Option<(usize, &'static str)> = editor
                .state
                .pads
                .iter()
                .enumerate()
                .find_map(|(idx, pad)| {
                    let arc_keys: [(&str, &str); 4] = [
                        ("corner_r_ne_arc", "corner_r_ne"),
                        ("corner_r_se_arc", "corner_r_se"),
                        ("corner_r_sw_arc", "corner_r_sw"),
                        ("corner_r_nw_arc", "corner_r_nw"),
                    ];
                    for (sidecar_key, corner_key) in arc_keys {
                        if pad.shape_params.get(sidecar_key).map(|s| s.as_str())
                            == Some(arc_id_str.as_str())
                        {
                            return Some((idx, corner_key));
                        }
                    }
                    None
                });

            let Some((pad_idx, corner_key)) = pad_corner else {
                tracing::warn!(
                    target: "signex::v024",
                    "FootprintSketchUnlinkCornerRadius: arc {arc_entity_id:?} doesn't belong \
                     to any pad's shape_params; ignoring"
                );
                return;
            };

            // Already unlinked → no-op (idempotent).
            if editor.state.pads[pad_idx]
                .shape_params
                .contains_key(corner_key)
            {
                tracing::warn!(
                    target: "signex::v024",
                    "FootprintSketchUnlinkCornerRadius: corner {corner_key} on pad {pad_idx} \
                     is already unlinked; ignoring"
                );
                return;
            }

            // Resolve the shared parameter name + current value.
            let shared_name = match editor.state.pads[pad_idx]
                .shape_params
                .get("corner_r")
                .cloned()
            {
                Some(n) => n,
                None => {
                    tracing::warn!(
                        target: "signex::v024",
                        "FootprintSketchUnlinkCornerRadius: pad {pad_idx} has no shared \
                         corner_r binding; ignoring"
                    );
                    return;
                }
            };
            let shared_value = editor
                .primitive()
                .sketch
                .as_ref()
                .and_then(|s| s.parameters.get_raw(&shared_name).map(str::to_string))
                .unwrap_or_default();

            // Mint the per-corner parameter name. Use the corner_key
            // suffix (e.g. `_ne`) appended to the shared name's slug
            // so the per-corner names cluster together in the
            // parameter table for inspection.
            let corner_suffix = corner_key
                .strip_prefix("corner_r_")
                .unwrap_or(corner_key);
            let new_param_name = format!("{shared_name}_{corner_suffix}");

            // Apply the rewrite. push_history is already captured at
            // the top of this dispatcher arm via mutates_footprint_state.
            editor.with_parts(|state, primitive| {
                // Mint the new parameter on the sketch.
                if let Some(sketch) = primitive.sketch.as_mut() {
                    sketch.parameters.insert(new_param_name.clone(), shared_value.clone());
                }
                // Record the per-corner override on the pad.
                if let Some(pad) = state.pads.get_mut(pad_idx) {
                    pad.shape_params
                        .insert(corner_key.to_string(), new_param_name.clone());
                }
                // ForceRebuild → solver re-runs, bake regenerates pad
                // geometry from the (now per-corner-aware) parameters.
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::FootprintSketchAddConstraintForSelection(tag) => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use crate::library::messages::SketchConstraintTag;
            use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
            use signex_sketch::id::ConstraintId;

            let primary = editor.state.selected_sketch;
            let secondary = editor.state.selected_sketch_secondary;
            let dim_target = editor
                .state
                .dimension_input
                .trim()
                .parse::<f64>()
                .ok()
                .map(DimTarget::Literal);

            // Determine selected entity kinds (Point / Line / Arc / Circle)
            // by inspecting the sketch.
            let kind_of = |id: signex_sketch::id::SketchEntityId| -> Option<&'static str> {
                use signex_sketch::entity::EntityKind;
                editor
                    .primitive()
                    .sketch
                    .as_ref()?
                    .entities
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| match e.kind {
                        EntityKind::Point { .. } => "Point",
                        EntityKind::Line { .. } => "Line",
                        EntityKind::Arc { .. } => "Arc",
                        EntityKind::Circle { .. } => "Circle",
                    })
            };
            let p_kind = primary.and_then(kind_of);
            let s_kind = secondary.and_then(kind_of);

            let new_kind: Option<ConstraintKind> = match (tag, p_kind, s_kind, primary, secondary) {
                (SketchConstraintTag::Fixed, Some("Point"), _, Some(p), _) => {
                    Some(ConstraintKind::Fixed { point: p })
                }
                (
                    SketchConstraintTag::Coincident,
                    Some("Point"),
                    Some("Point"),
                    Some(p1),
                    Some(p2),
                ) => Some(ConstraintKind::Coincident { p1, p2 }),
                (
                    SketchConstraintTag::DistancePtPt,
                    Some("Point"),
                    Some("Point"),
                    Some(p1),
                    Some(p2),
                ) => dim_target.map(|t| ConstraintKind::DistancePtPt { p1, p2, target: t }),
                (SketchConstraintTag::Horizontal, Some("Line"), _, Some(l), _) => {
                    Some(ConstraintKind::Horizontal { line: l })
                }
                (SketchConstraintTag::Vertical, Some("Line"), _, Some(l), _) => {
                    Some(ConstraintKind::Vertical { line: l })
                }
                (SketchConstraintTag::Parallel, Some("Line"), Some("Line"), Some(l1), Some(l2)) => {
                    Some(ConstraintKind::Parallel { l1, l2 })
                }
                (
                    SketchConstraintTag::Perpendicular,
                    Some("Line"),
                    Some("Line"),
                    Some(l1),
                    Some(l2),
                ) => Some(ConstraintKind::Perpendicular { l1, l2 }),
                (
                    SketchConstraintTag::EqualLength,
                    Some("Line"),
                    Some("Line"),
                    Some(l1),
                    Some(l2),
                ) => Some(ConstraintKind::EqualLength { l1, l2 }),
                (
                    SketchConstraintTag::PointOnLine,
                    Some("Point"),
                    Some("Line"),
                    Some(p),
                    Some(l),
                ) => Some(ConstraintKind::PointOnLine { point: p, line: l }),
                (
                    SketchConstraintTag::PointOnLine,
                    Some("Line"),
                    Some("Point"),
                    Some(l),
                    Some(p),
                ) => Some(ConstraintKind::PointOnLine { point: p, line: l }),
                (SketchConstraintTag::Midpoint, Some("Point"), Some("Line"), Some(p), Some(l)) => {
                    Some(ConstraintKind::Midpoint { point: p, line: l })
                }
                (SketchConstraintTag::Midpoint, Some("Line"), Some("Point"), Some(l), Some(p)) => {
                    Some(ConstraintKind::Midpoint { point: p, line: l })
                }
                _ => None,
            };

            if let Some(kind) = new_kind {
                let constraint = Constraint {
                    id: ConstraintId::new(),
                    kind,
                };
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(
                        state,
                        primitive,
                        SketchEdit::AddConstraint(constraint),
                    );
                });
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        PrimitiveEditorMsg::FootprintSketchToolClick {
            x_mm,
            y_mm,
            snap_id,
        } => {
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            use crate::library::editor::footprint::state::{
                PlacementInputKind, SketchTool, ToolPending,
            };
            use signex_sketch::entity::{Entity, EntityKind};
            use signex_sketch::id::SketchEntityId;
            use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

            // v0.16.1 — sticky construction flag captured once so each
            // newly-minted entity can be flagged in one place. Pads
            // (PadAttr-carrying centre Points minted via auto_mint /
            // mirror_add) intentionally bypass this; the bake skips
            // construction entities and a construction pad would
            // disappear from the rendered output.
            let construction_mode = editor.state.construction_mode;
            let centerline_mode = editor.state.centerline_mode;
            let mut flag = |mut e: Entity| -> Entity {
                e.construction = construction_mode;
                e.centerline = centerline_mode;
                e
            };

            // Resolve the click into either an existing snap Point or a
            // freshly-minted Point. For multi-click tools (Line / Rect /
            // Circle / Arc), the dispatcher reuses the snap target's ID
            // so closed-loop detection (canvas.rs::draw_filled_closed_
            // loops) continues to recognise cycles by shared endpoint
            // ID. Otherwise it appends a Point at the click position
            // and uses that new ID for the active tool's gesture state.
            //
            // v0.22 Phase A1 — Auto-Coincident inference for the
            // Place-Point tool. A Place-Point click on an existing
            // Point used to be a silent no-op (snap_id was returned
            // but never acted upon). It now mints a fresh Point at
            // the snap target and pins it to the target with a
            // Coincident constraint, so the user gets a Fusion-style
            // "place a point coincident with this one" gesture
            // without having to switch to the Constraint sub-tool.
            // Multi-click tools deliberately keep shared-ID
            // semantics — their endpoint ID is the bake's vertex
            // identity and switching to constraint-merged points
            // would silently break the closed-loop walker.
            let plane_id = match editor.primitive().sketch.as_ref() {
                Some(s) if !s.planes.is_empty() => s.planes[0].id,
                _ => {
                    let pid = PlaneId::new();
                    let sketch = editor
                        .primitive_mut()
                        .sketch
                        .get_or_insert_with(signex_sketch::SketchData::default);
                    sketch.planes.push(Plane {
                        id: pid,
                        kind: PlaneKind::BoardTop,
                    });
                    pid
                }
            };

            // v0.24 Track D — consume `state.placement_input` if it
            // matches the active tool's pending state. The buffer is
            // parsed as `f64` mm (length / radius) or degrees
            // (sweep), translated into an effective click position
            // overriding `x_mm` / `y_mm`, and the snap target is
            // dropped so the typed-length wins over a coincidence
            // hit. Returns the effective `(x, y)` and a flag whose
            // `true` value means the click was geometry-pinned by a
            // numeric input — used to (1) ignore `snap_id` and (2)
            // clear `state.placement_input` after the gesture
            // commits.
            let placement_input_kind = editor.state.placement_input.as_ref().map(|p| p.kind);
            let placement_input_value = editor
                .state
                .placement_input
                .as_ref()
                .and_then(|p| p.buffer.parse::<f64>().ok());
            let resolve_point_xy =
                |id: SketchEntityId, primitive: &signex_library::primitive::footprint::Footprint| -> Option<(f64, f64)> {
                    primitive
                        .sketch
                        .as_ref()
                        .and_then(|s| s.entities.iter().find(|e| e.id == id))
                        .and_then(|e| match e.kind {
                            EntityKind::Point { x, y } => Some((x, y)),
                            _ => None,
                        })
                };
            let (eff_x_mm, eff_y_mm, used_placement_input): (f64, f64, bool) = match (
                placement_input_kind,
                placement_input_value,
                editor.state.active_tool,
                editor.state.tool_pending.clone(),
            ) {
                // Line second click — pin distance from `first` along
                // the cursor azimuth.
                (
                    Some(PlacementInputKind::LineLength),
                    Some(len),
                    SketchTool::Line,
                    ToolPending::LineFirst { first },
                ) if len > 0.0 => {
                    let primitive = editor.primitive();
                    if let Some((fx, fy)) = resolve_point_xy(first, primitive) {
                        let dx = x_mm - fx;
                        let dy = y_mm - fy;
                        let cursor_len = (dx * dx + dy * dy).sqrt();
                        if cursor_len > 1e-9 {
                            let ux = dx / cursor_len;
                            let uy = dy / cursor_len;
                            (fx + len * ux, fy + len * uy, true)
                        } else {
                            // Cursor coincides with the first
                            // endpoint — no azimuth to pin to. Fall
                            // back to the raw click so the user gets
                            // visible feedback that nothing happened.
                            (x_mm, y_mm, false)
                        }
                    } else {
                        (x_mm, y_mm, false)
                    }
                }
                // Circle second click — radius from centre, along
                // the cursor azimuth.
                (
                    Some(PlacementInputKind::CircleRadius),
                    Some(r),
                    SketchTool::Circle,
                    ToolPending::CircleCenter { center },
                ) if r > 0.0 => {
                    let primitive = editor.primitive();
                    if let Some((cx, cy)) = resolve_point_xy(center, primitive) {
                        let dx = x_mm - cx;
                        let dy = y_mm - cy;
                        let cursor_len = (dx * dx + dy * dy).sqrt();
                        if cursor_len > 1e-9 {
                            let ux = dx / cursor_len;
                            let uy = dy / cursor_len;
                            (cx + r * ux, cy + r * uy, true)
                        } else {
                            // Cursor at centre → fall back; the user
                            // can re-position before clicking.
                            (x_mm, y_mm, false)
                        }
                    } else {
                        (x_mm, y_mm, false)
                    }
                }
                // Arc second click — start endpoint at exact radius
                // from centre, along cursor azimuth.
                (
                    Some(PlacementInputKind::ArcRadius),
                    Some(r),
                    SketchTool::Arc,
                    ToolPending::ArcCenter { center },
                ) if r > 0.0 => {
                    let primitive = editor.primitive();
                    if let Some((cx, cy)) = resolve_point_xy(center, primitive) {
                        let dx = x_mm - cx;
                        let dy = y_mm - cy;
                        let cursor_len = (dx * dx + dy * dy).sqrt();
                        if cursor_len > 1e-9 {
                            let ux = dx / cursor_len;
                            let uy = dy / cursor_len;
                            (cx + r * ux, cy + r * uy, true)
                        } else {
                            (x_mm, y_mm, false)
                        }
                    } else {
                        (x_mm, y_mm, false)
                    }
                }
                // Arc third click — sweep from `start` by typed
                // degrees CCW around `center`. Radius is the
                // committed |centre, start| distance.
                (
                    Some(PlacementInputKind::ArcSweep),
                    Some(deg),
                    SketchTool::Arc,
                    ToolPending::ArcStart { center, start },
                ) => {
                    let primitive = editor.primitive();
                    let parts = (
                        resolve_point_xy(center, primitive),
                        resolve_point_xy(start, primitive),
                    );
                    if let (Some((cx, cy)), Some((sx, sy))) = parts {
                        let r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                        if r > 1e-9 {
                            let start_ang = (sy - cy).atan2(sx - cx);
                            let end_ang = start_ang + deg.to_radians();
                            (
                                cx + r * end_ang.cos(),
                                cy + r * end_ang.sin(),
                                true,
                            )
                        } else {
                            (x_mm, y_mm, false)
                        }
                    } else {
                        (x_mm, y_mm, false)
                    }
                }
                _ => (x_mm, y_mm, false),
            };
            // When numeric input pinned the click, ignore the snap
            // hit (the user explicitly asked for a different
            // distance / angle).
            let effective_snap_id = if used_placement_input {
                None
            } else {
                snap_id
            };

            let resolved_id: SketchEntityId = match effective_snap_id {
                Some(target) if matches!(editor.state.active_tool, SketchTool::Point) => {
                    use signex_sketch::constraint::{Constraint, ConstraintKind};
                    use signex_sketch::id::ConstraintId;

                    let new_id = SketchEntityId::new();
                    let entity = flag(Entity::new(
                        new_id,
                        plane_id,
                        EntityKind::Point {
                            x: eff_x_mm,
                            y: eff_y_mm,
                        },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(entity),
                        );
                    });
                    let constraint = Constraint {
                        id: ConstraintId::new(),
                        kind: ConstraintKind::Coincident {
                            p1: new_id,
                            p2: target,
                        },
                    };
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddConstraint(constraint),
                        );
                    });
                    new_id
                }
                Some(id) => id,
                None => {
                    let id = SketchEntityId::new();
                    let entity = flag(Entity::new(
                        id,
                        plane_id,
                        EntityKind::Point {
                            x: eff_x_mm,
                            y: eff_y_mm,
                        },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(entity),
                        );
                    });
                    id
                }
            };

            // v0.23 — RepickPolarCenter intercept. Triggered by the
            // Pattern sub-form's "Re-pick centre" button. The next
            // click on a Point overwrites the array's `center`,
            // independent of the active tool. `resolved_id` is either
            // an existing Point (when snap hit) or a freshly-minted
            // Point at the click location. Skip the tool match below
            // by handling cleanup inline.
            let mut consumed_by_repick = false;
            if let ToolPending::RepickPolarCenter { array_id } = editor.state.tool_pending {
                if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                    if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id) {
                        if let signex_sketch::array::ArrayKind::Polar { center, .. } =
                            &mut array.kind
                        {
                            *center = resolved_id;
                        }
                    }
                }
                editor.with_parts(|state, primitive| {
                    apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
                });
                editor.state.tool_pending = ToolPending::Idle;
                consumed_by_repick = true;
            }

            if consumed_by_repick {
                editor.canvas_cache.clear();
                editor.dirty = true;
                return;
            }

            // Per-tool state machine — advance `tool_pending` and emit
            // the gesture-completing AddEntity when ready.
            match editor.state.active_tool {
                SketchTool::Select | SketchTool::Point => {
                    // Select: ignore (no add). Point: already added above.
                    editor.state.tool_pending = ToolPending::Idle;
                }
                SketchTool::Line => match editor.state.tool_pending {
                    ToolPending::Idle => {
                        editor.state.tool_pending = ToolPending::LineFirst { first: resolved_id };
                    }
                    ToolPending::LineFirst { first } => {
                        let line_id = SketchEntityId::new();
                        let line = flag(Entity::new(
                            line_id,
                            plane_id,
                            EntityKind::Line {
                                start: first,
                                end: resolved_id,
                            },
                        ));
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::AddEntity(line),
                            );
                        });

                        // v0.22 Phase A2 — Auto-Horizontal/Vertical
                        // inference. If the line's slope is within ±5°
                        // of a cardinal axis, add the matching
                        // constraint so the alignment survives a drag.
                        // The cursor-snap engine already pulls the
                        // click onto the axis when within tolerance,
                        // so this just promotes the implicit alignment
                        // to an explicit constraint visible in the
                        // constraint list.
                        {
                            use signex_sketch::constraint::{Constraint, ConstraintKind};
                            use signex_sketch::id::ConstraintId;
                            const AXIS_THRESHOLD_DEG: f64 = 5.0;
                            let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                                editor
                                    .primitive()
                                    .sketch
                                    .as_ref()
                                    .and_then(|s| s.entities.iter().find(|e| e.id == id))
                                    .and_then(|e| match e.kind {
                                        EntityKind::Point { x, y } => Some((x, y)),
                                        _ => None,
                                    })
                            };
                            if let (Some((x0, y0)), Some((x1, y1))) =
                                (pos_of(first), pos_of(resolved_id))
                            {
                                let dx = x1 - x0;
                                let dy = y1 - y0;
                                let len_sq = dx * dx + dy * dy;
                                if len_sq > 1e-12 {
                                    let len = len_sq.sqrt();
                                    let sin_abs = (dy / len).abs();
                                    let cos_abs = (dx / len).abs();
                                    let thresh = AXIS_THRESHOLD_DEG.to_radians().sin();
                                    let kind = if sin_abs < thresh {
                                        Some(ConstraintKind::Horizontal { line: line_id })
                                    } else if cos_abs < thresh {
                                        Some(ConstraintKind::Vertical { line: line_id })
                                    } else {
                                        None
                                    };
                                    if let Some(k) = kind {
                                        let constraint = Constraint {
                                            id: ConstraintId::new(),
                                            kind: k,
                                        };
                                        editor.with_parts(|state, primitive| {
                                            apply_sketch_edit_with_warnings(
                                                state,
                                                primitive,
                                                SketchEdit::AddConstraint(constraint),
                                            );
                                        });
                                    }
                                }
                            }
                        }

                        // v0.16.1 — chain: keep the Line tool active
                        // and use this click's endpoint as the next
                        // segment's anchor. Esc / right-click cancel
                        // back to Select. Matches Fusion 2D sketch.
                        editor.state.tool_pending = ToolPending::LineFirst { first: resolved_id };
                    }
                    _ => {
                        editor.state.tool_pending = ToolPending::LineFirst { first: resolved_id };
                    }
                },
                SketchTool::Circle => match editor.state.tool_pending {
                    ToolPending::Idle => {
                        editor.state.tool_pending = ToolPending::CircleCenter {
                            center: resolved_id,
                        };
                    }
                    ToolPending::CircleCenter { center } => {
                        // Compute radius from centre + edge points.
                        let r = if let (Some(c_pt), Some(e_pt)) = (
                            editor
                                .primitive()
                                .sketch
                                .as_ref()
                                .and_then(|s| s.entities.iter().find(|e| e.id == center))
                                .and_then(|e| match e.kind {
                                    EntityKind::Point { x, y } => Some((x, y)),
                                    _ => None,
                                }),
                            editor
                                .primitive()
                                .sketch
                                .as_ref()
                                .and_then(|s| s.entities.iter().find(|e| e.id == resolved_id))
                                .and_then(|e| match e.kind {
                                    EntityKind::Point { x, y } => Some((x, y)),
                                    _ => None,
                                }),
                        ) {
                            ((e_pt.0 - c_pt.0).powi(2) + (e_pt.1 - c_pt.1).powi(2)).sqrt()
                        } else {
                            1.0
                        };
                        let circle_id = SketchEntityId::new();
                        let circle = flag(Entity::new(
                            circle_id,
                            plane_id,
                            EntityKind::Circle { center, radius: r },
                        ));
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::AddEntity(circle),
                            );
                        });
                        editor.state.tool_pending = ToolPending::Idle;
                    }
                    _ => {
                        editor.state.tool_pending = ToolPending::CircleCenter {
                            center: resolved_id,
                        };
                    }
                },
                SketchTool::RoundedRectangle => match editor.state.tool_pending {
                    ToolPending::Idle => {
                        editor.state.tool_pending =
                            ToolPending::RoundedRectangleFirst { first: resolved_id };
                    }
                    ToolPending::RoundedRectangleFirst { first } => {
                        // v0.16 — commit the rounded rectangle. Read
                        // first/opposite corner positions, derive the
                        // axis-aligned bbox, clamp the corner radius,
                        // and emit 4 arc-centre Points + 8 arc-end /
                        // line-end Points + 4 Lines (axis-aligned,
                        // shortened by the radius) + 4 Arcs (one per
                        // corner, sweep CCW around the centre).
                        let first_pos = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == first))
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            });
                        let opposite_pos = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == resolved_id))
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            });
                        if let (Some((fx, fy)), Some((ox, oy))) = (first_pos, opposite_pos) {
                            let x0 = fx.min(ox);
                            let y0 = fy.min(oy);
                            let x1 = fx.max(ox);
                            let y1 = fy.max(oy);
                            let half_w = (x1 - x0) / 2.0;
                            let half_h = (y1 - y0) / 2.0;
                            // Read corner radius from dimension input;
                            // default 0.5 mm, clamp to [0.05, half_min].
                            let r_input = editor
                                .state
                                .dimension_input
                                .trim()
                                .parse::<f64>()
                                .ok()
                                .unwrap_or(0.5);
                            let r_max = half_w.min(half_h).max(0.05);
                            let r = r_input.clamp(0.05, r_max);

                            let tl_c = SketchEntityId::new();
                            let tr_c = SketchEntityId::new();
                            let br_c = SketchEntityId::new();
                            let bl_c = SketchEntityId::new();
                            let tl_right = SketchEntityId::new();
                            let tr_left = SketchEntityId::new();
                            let tr_top = SketchEntityId::new();
                            let br_top = SketchEntityId::new();
                            let br_right = SketchEntityId::new();
                            let bl_left = SketchEntityId::new();
                            let bl_bot = SketchEntityId::new();
                            let tl_bot = SketchEntityId::new();

                            for (id, x, y) in [
                                (tl_c, x0 + r, y0 + r),
                                (tr_c, x1 - r, y0 + r),
                                (br_c, x1 - r, y1 - r),
                                (bl_c, x0 + r, y1 - r),
                                (tl_right, x0 + r, y0),
                                (tr_left, x1 - r, y0),
                                (tr_top, x1, y0 + r),
                                (br_top, x1, y1 - r),
                                (br_right, x1 - r, y1),
                                (bl_left, x0 + r, y1),
                                (bl_bot, x0, y1 - r),
                                (tl_bot, x0, y0 + r),
                            ] {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(flag(Entity::new(
                                            id,
                                            plane_id,
                                            EntityKind::Point { x, y },
                                        ))),
                                    );
                                });
                            }
                            // Lines: top, right, bottom, left.
                            for (s, e) in [
                                (tl_right, tr_left),
                                (tr_top, br_top),
                                (br_right, bl_left),
                                (bl_bot, tl_bot),
                            ] {
                                let line_id = SketchEntityId::new();
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(flag(Entity::new(
                                            line_id,
                                            plane_id,
                                            EntityKind::Line { start: s, end: e },
                                        ))),
                                    );
                                });
                            }
                            // Arcs: TR, BR, BL, TL — sweep CCW around
                            // each centre so each subtends 90°.
                            for (center, start, end) in [
                                (tr_c, tr_left, tr_top),
                                (br_c, br_top, br_right),
                                (bl_c, bl_left, bl_bot),
                                (tl_c, tl_bot, tl_right),
                            ] {
                                let arc_id = SketchEntityId::new();
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(flag(Entity::new(
                                            arc_id,
                                            plane_id,
                                            EntityKind::Arc {
                                                center,
                                                start,
                                                end,
                                                sweep_ccw: true,
                                            },
                                        ))),
                                    );
                                });
                            }
                        }
                        editor.state.tool_pending = ToolPending::Idle;
                    }
                    _ => {
                        editor.state.tool_pending =
                            ToolPending::RoundedRectangleFirst { first: resolved_id };
                    }
                },
                SketchTool::Rectangle => match editor.state.tool_pending {
                    ToolPending::Idle => {
                        editor.state.tool_pending =
                            ToolPending::RectangleFirst { first: resolved_id };
                    }
                    ToolPending::RectangleFirst { first } => {
                        // v0.15 — commit the rectangle. Resolve the
                        // first corner's world position from the
                        // sketch, then mint 2 new Points (the two
                        // mid-axis corners) and 4 Lines connecting
                        // (first, midA, opposite, midB) into a
                        // closed loop. resolved_id is the opposite
                        // corner the user just clicked.
                        let first_pos = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == first))
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            });
                        let opposite_pos = editor
                            .primitive()
                            .sketch
                            .as_ref()
                            .and_then(|s| s.entities.iter().find(|e| e.id == resolved_id))
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            });
                        if let (Some((x0, y0)), Some((x1, y1))) = (first_pos, opposite_pos) {
                            // Mint the two mid-axis corners.
                            let mid_a_id = SketchEntityId::new();
                            let mid_b_id = SketchEntityId::new();
                            let mid_a = flag(Entity::new(
                                mid_a_id,
                                plane_id,
                                EntityKind::Point { x: x1, y: y0 },
                            ));
                            let mid_b = flag(Entity::new(
                                mid_b_id,
                                plane_id,
                                EntityKind::Point { x: x0, y: y1 },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(mid_a),
                                );
                            });
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(mid_b),
                                );
                            });
                            // Now the 4 lines: first → mid_a →
                            // opposite → mid_b → first.
                            for (s, e) in [
                                (first, mid_a_id),
                                (mid_a_id, resolved_id),
                                (resolved_id, mid_b_id),
                                (mid_b_id, first),
                            ] {
                                let line_id = SketchEntityId::new();
                                let line = flag(Entity::new(
                                    line_id,
                                    plane_id,
                                    EntityKind::Line { start: s, end: e },
                                ));
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(line),
                                    );
                                });
                            }
                        }
                        editor.state.tool_pending = ToolPending::Idle;
                    }
                    _ => {
                        editor.state.tool_pending =
                            ToolPending::RectangleFirst { first: resolved_id };
                    }
                },
                SketchTool::Arc => match editor.state.tool_pending {
                    ToolPending::Idle => {
                        editor.state.tool_pending = ToolPending::ArcCenter {
                            center: resolved_id,
                        };
                    }
                    ToolPending::ArcCenter { center } => {
                        editor.state.tool_pending = ToolPending::ArcStart {
                            center,
                            start: resolved_id,
                        };
                    }
                    ToolPending::ArcStart { center, start } => {
                        let arc_id = SketchEntityId::new();
                        let arc = flag(Entity::new(
                            arc_id,
                            plane_id,
                            EntityKind::Arc {
                                center,
                                start,
                                end: resolved_id,
                                sweep_ccw: true,
                            },
                        ));
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::AddEntity(arc),
                            );
                        });
                        editor.state.tool_pending = ToolPending::Idle;
                    }
                    _ => {
                        editor.state.tool_pending = ToolPending::ArcCenter {
                            center: resolved_id,
                        };
                    }
                },
                SketchTool::Mirror => {
                    // v0.22 Phase B1 + extension — Mirror tool.
                    // Pre-condition: a Line entity must already be
                    // selected via the Select tool; clicks while no
                    // Line is selected are silent no-ops with a
                    // warning surfaced via `solve_warnings`.
                    //
                    // The picked entity's geometry is reflected
                    // across the selected Line and a fresh entity is
                    // minted referencing mirrored copies of every
                    // Point it touches. Each mirrored Point pair
                    // gets a `SymmetricAboutLine` constraint so the
                    // solver maintains symmetry through subsequent
                    // edits (drag the source and the mirror tracks
                    // it parametrically).
                    //
                    // Scope: Points / Lines / Arcs / Circles.
                    // Mirrored Arcs flip `sweep_ccw` because
                    // reflection inverts winding. Mirrored Circles
                    // re-use the source radius (Circle's `radius` is
                    // a literal, not a referenced Point, so it
                    // round-trips unchanged).
                    use signex_sketch::constraint::{Constraint, ConstraintKind};
                    use signex_sketch::id::ConstraintId;

                    let line_id = match editor.state.selected_sketch {
                        Some(id) => id,
                        None => {
                            editor.state.solve_warnings.push(
                                "Mirror: select a Line first (Select tool, click a Line, then click here to mirror)"
                                    .into(),
                            );
                            editor.state.tool_pending = ToolPending::Idle;
                            editor.canvas_cache.clear();
                            return;
                        }
                    };

                    let sketch_ref = match editor.primitive().sketch.as_ref() {
                        Some(s) => s,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    let line_endpoints = sketch_ref
                        .entities
                        .iter()
                        .find(|e| e.id == line_id)
                        .and_then(|e| match e.kind {
                            EntityKind::Line { start, end } => Some((start, end)),
                            _ => None,
                        });
                    let (a_id, b_id) = match line_endpoints {
                        Some(p) => p,
                        None => {
                            editor.state.solve_warnings.push(
                                "Mirror: selection is not a Line — pick a Line entity first"
                                    .into(),
                            );
                            editor.state.tool_pending = ToolPending::Idle;
                            editor.canvas_cache.clear();
                            return;
                        }
                    };

                    let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                        sketch_ref
                            .entities
                            .iter()
                            .find(|e| e.id == id)
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            })
                    };
                    let kind_of = sketch_ref
                        .entities
                        .iter()
                        .find(|e| e.id == resolved_id)
                        .map(|e| e.kind.clone());
                    let kind_of = match kind_of {
                        Some(k) => k,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };

                    let (ax, ay) = match pos_of(a_id) {
                        Some(p) => p,
                        None => return,
                    };
                    let (bx, by) = match pos_of(b_id) {
                        Some(p) => p,
                        None => return,
                    };
                    let vx = bx - ax;
                    let vy = by - ay;
                    let v_dot_v = vx * vx + vy * vy;
                    if v_dot_v <= 1e-12 {
                        editor.state.solve_warnings.push(
                            "Mirror: degenerate Line (zero length)".into(),
                        );
                        editor.state.tool_pending = ToolPending::Idle;
                        editor.canvas_cache.clear();
                        return;
                    }
                    let reflect = |px: f64, py: f64| -> (f64, f64) {
                        let t = ((px - ax) * vx + (py - ay) * vy) / v_dot_v;
                        let foot_x = ax + t * vx;
                        let foot_y = ay + t * vy;
                        (2.0 * foot_x - px, 2.0 * foot_y - py)
                    };

                    // Mirror a Point entity by ID: emits a new Point
                    // at the reflected position and a
                    // SymmetricAboutLine constraint linking source
                    // and mirror. Returns the new Point's ID.
                    // Captured by reference so the closure can be
                    // called repeatedly for chained-Point entities.
                    let mut mint_mirror_point =
                        |editor: &mut crate::app::FootprintEditorState,
                         pt_id: SketchEntityId,
                         pos: (f64, f64)|
                         -> SketchEntityId {
                            let (rx, ry) = reflect(pos.0, pos.1);
                            let new_id = SketchEntityId::new();
                            let new_entity = flag(Entity::new(
                                new_id,
                                plane_id,
                                EntityKind::Point { x: rx, y: ry },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_entity),
                                );
                            });
                            let constraint = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::SymmetricAboutLine {
                                    p1: pt_id,
                                    p2: new_id,
                                    line: line_id,
                                },
                            };
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(constraint),
                                );
                            });
                            new_id
                        };

                    match kind_of {
                        EntityKind::Point { x, y } => {
                            mint_mirror_point(editor, resolved_id, (x, y));
                        }
                        EntityKind::Line { start, end } => {
                            let s_pos = match pos_of(start) {
                                Some(p) => p,
                                None => return,
                            };
                            let e_pos = match pos_of(end) {
                                Some(p) => p,
                                None => return,
                            };
                            let new_start = mint_mirror_point(editor, start, s_pos);
                            let new_end = mint_mirror_point(editor, end, e_pos);
                            let new_line_id = SketchEntityId::new();
                            let new_line = flag(Entity::new(
                                new_line_id,
                                plane_id,
                                EntityKind::Line {
                                    start: new_start,
                                    end: new_end,
                                },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_line),
                                );
                            });
                        }
                        EntityKind::Arc {
                            center,
                            start,
                            end,
                            sweep_ccw,
                        } => {
                            let c_pos = match pos_of(center) {
                                Some(p) => p,
                                None => return,
                            };
                            let s_pos = match pos_of(start) {
                                Some(p) => p,
                                None => return,
                            };
                            let e_pos = match pos_of(end) {
                                Some(p) => p,
                                None => return,
                            };
                            let new_center = mint_mirror_point(editor, center, c_pos);
                            let new_start = mint_mirror_point(editor, start, s_pos);
                            let new_end = mint_mirror_point(editor, end, e_pos);
                            // Reflection inverts winding — flip
                            // sweep_ccw so the mirrored arc traces
                            // the same arc on the other side.
                            let new_arc_id = SketchEntityId::new();
                            let new_arc = flag(Entity::new(
                                new_arc_id,
                                plane_id,
                                EntityKind::Arc {
                                    center: new_center,
                                    start: new_start,
                                    end: new_end,
                                    sweep_ccw: !sweep_ccw,
                                },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_arc),
                                );
                            });
                        }
                        EntityKind::Circle { center, radius } => {
                            let c_pos = match pos_of(center) {
                                Some(p) => p,
                                None => return,
                            };
                            let new_center = mint_mirror_point(editor, center, c_pos);
                            let new_circle_id = SketchEntityId::new();
                            let new_circle = flag(Entity::new(
                                new_circle_id,
                                plane_id,
                                EntityKind::Circle {
                                    center: new_center,
                                    radius,
                                },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_circle),
                                );
                            });
                        }
                    }
                    editor.state.tool_pending = ToolPending::Idle;
                }
                SketchTool::Offset => {
                    // v0.22 Phase B2 — Offset tool. Pre-condition: a
                    // Line / Arc / Circle is in `selected_sketch`. The
                    // click position determines which side of the
                    // source curve the offset lands on. Offset
                    // distance comes from `state.dimension_input`,
                    // default 0.5 mm.
                    //
                    // Lines: emits a parallel Line at perpendicular
                    // distance and adds (Parallel + DistancePtLine)
                    // constraints so the relationship survives source
                    // edits.
                    //
                    // Circles / Arcs: emits a concentric copy that
                    // shares the source's centre Point so the centres
                    // stay locked. The new radius is a literal
                    // (source.radius ± dist) — the schema has no
                    // radius-dimension constraint, so further radius
                    // edits don't auto-propagate; the user can
                    // re-offset or edit the literal directly.
                    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
                    use signex_sketch::id::ConstraintId;

                    let source_id = match editor.state.selected_sketch {
                        Some(id) => id,
                        None => {
                            editor.state.solve_warnings.push(
                                "Offset: select a Line / Arc / Circle first (Select tool, click the curve, then click on the side to offset)"
                                    .into(),
                            );
                            editor.state.tool_pending = ToolPending::Idle;
                            editor.canvas_cache.clear();
                            return;
                        }
                    };
                    // v0.25 polish — prefer placement_input over the
                    // legacy `dimension_input` text field. The
                    // keypress-driven cursor overlay is the
                    // discoverable path; `dimension_input` stays as
                    // the Properties-panel fallback for users who
                    // already have a value there.
                    let dist_from_placement = editor
                        .state
                        .placement_input
                        .as_ref()
                        .filter(|p| p.kind == PlacementInputKind::OffsetDistance)
                        .and_then(|p| p.buffer.parse::<f64>().ok())
                        .filter(|d| d.is_finite() && *d > 1e-9);
                    let dist = dist_from_placement.unwrap_or_else(|| {
                        editor
                            .state
                            .dimension_input
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .filter(|d| d.is_finite() && *d > 1e-9)
                            .unwrap_or(0.5)
                    });
                    // Clear the buffer so the next Offset click
                    // doesn''t accidentally reuse the old value.
                    if dist_from_placement.is_some() {
                        editor.state.placement_input = None;
                    }

                    let sketch_ref = match editor.primitive().sketch.as_ref() {
                        Some(s) => s,
                        None => {
                            editor.state.tool_pending = ToolPending::Idle;
                            return;
                        }
                    };
                    let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                        sketch_ref
                            .entities
                            .iter()
                            .find(|e| e.id == id)
                            .and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            })
                    };
                    let source_kind = sketch_ref
                        .entities
                        .iter()
                        .find(|e| e.id == source_id)
                        .map(|e| e.kind.clone());
                    let source_kind = match source_kind {
                        Some(k) => k,
                        None => {
                            editor.state.solve_warnings.push(
                                "Offset: selection no longer exists in the sketch".into(),
                            );
                            editor.state.tool_pending = ToolPending::Idle;
                            editor.canvas_cache.clear();
                            return;
                        }
                    };

                    match source_kind {
                        EntityKind::Line { start, end } => {
                            let (ax, ay) = match pos_of(start) {
                                Some(p) => p,
                                None => return,
                            };
                            let (bx, by) = match pos_of(end) {
                                Some(p) => p,
                                None => return,
                            };
                            let dx = bx - ax;
                            let dy = by - ay;
                            let len = (dx * dx + dy * dy).sqrt();
                            if len < 1e-9 {
                                editor.state.solve_warnings.push(
                                    "Offset: degenerate Line (zero length)".into(),
                                );
                                editor.state.tool_pending = ToolPending::Idle;
                                editor.canvas_cache.clear();
                                return;
                            }
                            // Perpendicular unit vector. Sign from the
                            // cross of (line direction) × (click −
                            // line start): positive = click is on the
                            // (-dy, dx) side, negative = (dy, -dx)
                            // side.
                            let cx = x_mm - ax;
                            let cy = y_mm - ay;
                            let cross = dx * cy - dy * cx;
                            let sign = if cross >= 0.0 { 1.0 } else { -1.0 };
                            let nx = -dy / len * sign;
                            let ny = dx / len * sign;
                            let off_x = nx * dist;
                            let off_y = ny * dist;

                            let new_a = SketchEntityId::new();
                            let new_b = SketchEntityId::new();
                            let new_line_id = SketchEntityId::new();
                            let a_entity = flag(Entity::new(
                                new_a,
                                plane_id,
                                EntityKind::Point {
                                    x: ax + off_x,
                                    y: ay + off_y,
                                },
                            ));
                            let b_entity = flag(Entity::new(
                                new_b,
                                plane_id,
                                EntityKind::Point {
                                    x: bx + off_x,
                                    y: by + off_y,
                                },
                            ));
                            let new_line = flag(Entity::new(
                                new_line_id,
                                plane_id,
                                EntityKind::Line {
                                    start: new_a,
                                    end: new_b,
                                },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(a_entity),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(b_entity),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_line),
                                );
                            });
                            // Parallel + DistancePtLine on the start
                            // endpoint pins the offset distance
                            // parametrically. The end endpoint is left
                            // free along the offset line direction —
                            // the user can drag it without breaking
                            // the offset relationship.
                            let parallel = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::Parallel {
                                    l1: source_id,
                                    l2: new_line_id,
                                },
                            };
                            let dist_constraint = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::DistancePtLine {
                                    point: new_a,
                                    line: source_id,
                                    target: DimTarget::Literal(dist),
                                },
                            };
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(parallel),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(dist_constraint),
                                );
                            });
                        }
                        EntityKind::Circle { center, radius } => {
                            let (cx, cy) = match pos_of(center) {
                                Some(p) => p,
                                None => return,
                            };
                            // Click distance from centre — inside the
                            // circle = shrink (-dist), outside =
                            // expand (+dist). Clamp to a positive
                            // radius so we don't mint a degenerate
                            // shape.
                            let click_r = ((x_mm - cx).powi(2) + (y_mm - cy).powi(2)).sqrt();
                            let signed = if click_r < radius { -dist } else { dist };
                            let new_radius = (radius + signed).max(1e-6);
                            let new_circle_id = SketchEntityId::new();
                            let new_circle = flag(Entity::new(
                                new_circle_id,
                                plane_id,
                                EntityKind::Circle {
                                    center,
                                    radius: new_radius,
                                },
                            ));
                            // v0.23 — parametric link: mint an anchor
                            // Point on the new circle and pin its
                            // distance to the source circle to
                            // `signed`. Combined with a DistancePtCircle
                            // on the new circle (target=0), this
                            // forces `new_radius = source_radius +
                            // signed` through the solver — so when
                            // the user edits the target via the
                            // Properties panel later, the new
                            // circle's radius follows.
                            let scale = if click_r > 1e-9 {
                                new_radius / click_r
                            } else {
                                1.0
                            };
                            let anchor_id = SketchEntityId::new();
                            let anchor = flag(Entity::new(
                                anchor_id,
                                plane_id,
                                EntityKind::Point {
                                    x: cx + (x_mm - cx) * scale,
                                    y: cy + (y_mm - cy) * scale,
                                },
                            ));
                            let on_new_circle = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::DistancePtCircle {
                                    point: anchor_id,
                                    circle: new_circle_id,
                                    target: DimTarget::Literal(0.0),
                                },
                            };
                            let parametric_offset = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::DistancePtCircle {
                                    point: anchor_id,
                                    circle: source_id,
                                    target: DimTarget::Literal(signed),
                                },
                            };
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(anchor),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_circle),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(on_new_circle),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(parametric_offset),
                                );
                            });
                        }
                        EntityKind::Arc {
                            center,
                            start,
                            end,
                            sweep_ccw,
                        } => {
                            let (cx, cy) = match pos_of(center) {
                                Some(p) => p,
                                None => return,
                            };
                            let (sx, sy) = match pos_of(start) {
                                Some(p) => p,
                                None => return,
                            };
                            let (ex, ey) = match pos_of(end) {
                                Some(p) => p,
                                None => return,
                            };
                            // Source radius from start position;
                            // direction from start angle.
                            let source_r =
                                ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
                            let click_r =
                                ((x_mm - cx).powi(2) + (y_mm - cy).powi(2)).sqrt();
                            let signed = if click_r < source_r { -dist } else { dist };
                            let new_r = (source_r + signed).max(1e-6);
                            let scale = new_r / source_r.max(1e-9);

                            let new_start = SketchEntityId::new();
                            let new_end = SketchEntityId::new();
                            let new_arc_id = SketchEntityId::new();
                            let s_entity = flag(Entity::new(
                                new_start,
                                plane_id,
                                EntityKind::Point {
                                    x: cx + (sx - cx) * scale,
                                    y: cy + (sy - cy) * scale,
                                },
                            ));
                            let e_entity = flag(Entity::new(
                                new_end,
                                plane_id,
                                EntityKind::Point {
                                    x: cx + (ex - cx) * scale,
                                    y: cy + (ey - cy) * scale,
                                },
                            ));
                            let new_arc = flag(Entity::new(
                                new_arc_id,
                                plane_id,
                                EntityKind::Arc {
                                    center,
                                    start: new_start,
                                    end: new_end,
                                    sweep_ccw,
                                },
                            ));
                            // v0.23 — parametric link: pin both new
                            // endpoints to be `signed` away from the
                            // source arc's underlying circle. Since
                            // both arcs share the same `center`, this
                            // forces the new arc's radius to track
                            // source_radius + signed through the
                            // solver. End Point's angle is left free
                            // — the user can drag it without breaking
                            // the parametric distance.
                            let dist_start = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::DistancePtCircle {
                                    point: new_start,
                                    circle: source_id,
                                    target: DimTarget::Literal(signed),
                                },
                            };
                            let dist_end = Constraint {
                                id: ConstraintId::new(),
                                kind: ConstraintKind::DistancePtCircle {
                                    point: new_end,
                                    circle: source_id,
                                    target: DimTarget::Literal(signed),
                                },
                            };
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(s_entity),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(e_entity),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(new_arc),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(dist_start),
                                );
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddConstraint(dist_end),
                                );
                            });
                        }
                        EntityKind::Point { .. } => {
                            editor.state.solve_warnings.push(
                                "Offset: selection is a Point — pick a Line / Arc / Circle"
                                    .into(),
                            );
                        }
                    }
                    editor.state.tool_pending = ToolPending::Idle;
                }
                SketchTool::RectPattern => {
                    // v0.22 Phase B3 — Rectangular Pattern. Click 1
                    // picks the source entity (whatever was clicked,
                    // including a freshly-minted Point if the click
                    // missed everything). Mints a default 2×2 grid
                    // with 5 mm spacing, sequential numbering. User
                    // edits via JSON until a Properties sub-form
                    // lands.
                    use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
                    let array = Array {
                        id: ArrayId::new(),
                        kind: ArrayKind::Grid {
                            source: resolved_id,
                            nx_expr: "2".into(),
                            ny_expr: "2".into(),
                            dx_expr: "5mm".into(),
                            dy_expr: "5mm".into(),
                            depopulation: None,
                        },
                        numbering: NumberingScheme::default(),
                    };
                    let sketch = editor
                        .primitive_mut()
                        .sketch
                        .get_or_insert_with(signex_sketch::SketchData::default);
                    sketch.arrays.push(array);
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::ForceRebuild,
                        );
                    });
                    editor.state.tool_pending = ToolPending::Idle;
                }
                SketchTool::TangentArc => {
                    // v0.24 Track C — Tangent Arc. Two-click chained
                    // arc segment that mints an Arc tangent to the
                    // most recently committed Line whose end Point
                    // matches the first click. The dispatcher also
                    // emits a `TangentLineArc` constraint so the
                    // tangency survives further edits.
                    //
                    // - Click 1: stash the resolved Point as
                    //   `ToolPending::TangentArcFirst { first }`.
                    //   Mirrors the Line tool's first-click flow.
                    // - Click 2: locate a Line whose `end == first`.
                    //   Compute the tangent centre on the line's
                    //   perpendicular bisector through `first` so
                    //   the arc starts off the line tangentially.
                    //   Mint an Arc entity + TangentLineArc
                    //   constraint and chain back to Idle.
                    //
                    // Fallback: when no incident Line is found, the
                    // dispatcher mints a placeholder centre at the
                    // perpendicular bisector of the chord (no
                    // tangency reference) and publishes a warning
                    // via `solve_warnings`. The Arc still appears in
                    // the sketch so the user can constrain it
                    // manually if desired.
                    use signex_sketch::constraint::{Constraint, ConstraintKind};
                    use signex_sketch::id::ConstraintId;

                    match editor.state.tool_pending {
                        ToolPending::TangentArcFirst { first } => {
                            // Look up the first endpoint position +
                            // any Line ending at `first`.
                            let (
                                first_pos,
                                end_pos,
                                incident_line,
                            ): ((f64, f64), (f64, f64), Option<(SketchEntityId, (f64, f64))>) = {
                                let sketch_ref = match editor.primitive().sketch.as_ref() {
                                    Some(s) => s,
                                    None => {
                                        editor.state.tool_pending = ToolPending::Idle;
                                        return;
                                    }
                                };
                                let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                                    sketch_ref
                                        .entities
                                        .iter()
                                        .find(|e| e.id == id)
                                        .and_then(|e| match e.kind {
                                            EntityKind::Point { x, y } => Some((x, y)),
                                            _ => None,
                                        })
                                };
                                let first_p = match pos_of(first) {
                                    Some(p) => p,
                                    None => {
                                        editor.state.tool_pending = ToolPending::Idle;
                                        return;
                                    }
                                };
                                let end_p = match pos_of(resolved_id) {
                                    Some(p) => p,
                                    None => {
                                        editor.state.tool_pending = ToolPending::Idle;
                                        return;
                                    }
                                };
                                // Find a Line whose end matches `first`.
                                // Prefer the most recently authored one
                                // (last in the list) so chained sketches
                                // pick up the immediately preceding
                                // Line, not an unrelated old one.
                                let line = sketch_ref
                                    .entities
                                    .iter()
                                    .rev()
                                    .find_map(|e| match e.kind {
                                        EntityKind::Line { start, end } if end == first => {
                                            pos_of(start).map(|p| (e.id, p))
                                        }
                                        EntityKind::Line { start, end } if start == first => {
                                            pos_of(end).map(|p| (e.id, p))
                                        }
                                        _ => None,
                                    });
                                (first_p, end_p, line)
                            };

                            // Compute the tangent centre.
                            //
                            // With an incident Line, the centre lies
                            // on the line's perpendicular through
                            // `first`. We pick the side of the chord
                            // (`first` → `end_pos`) that lets the arc
                            // reach `end` along that perpendicular,
                            // and place the centre on the
                            // perpendicular bisector of the chord
                            // intersected with the line-perpendicular
                            // through `first`. That intersection is
                            // the unique circle that is tangent to
                            // the line at `first` and passes through
                            // `end_pos`.
                            //
                            // Without an incident Line, fall back to
                            // the chord's perpendicular bisector
                            // midpoint shifted by half-chord —
                            // produces a 90° arc as a sane default.
                            let (cx, cy) = match incident_line {
                                Some((_, line_other_pos)) => {
                                    // Line direction (line_other -> first)
                                    let lx = first_pos.0 - line_other_pos.0;
                                    let ly = first_pos.1 - line_other_pos.1;
                                    let llen_sq = lx * lx + ly * ly;
                                    if llen_sq <= 1e-12 {
                                        // Degenerate; treat as no line.
                                        let mx = (first_pos.0 + end_pos.0) * 0.5;
                                        let my = (first_pos.1 + end_pos.1) * 0.5;
                                        let dx = end_pos.0 - first_pos.0;
                                        let dy = end_pos.1 - first_pos.1;
                                        // Rotate 90° CCW for placeholder.
                                        (mx + (-dy) * 0.5, my + dx * 0.5)
                                    } else {
                                        // Perpendicular to the line at first.
                                        let llen = llen_sq.sqrt();
                                        let nx = -ly / llen;
                                        let ny = lx / llen;
                                        // Centre is on the line through `first`
                                        // along (nx, ny). Solve for the t such
                                        // that |centre - end| = |centre - first|:
                                        //   (first.x + t*nx - end.x)^2
                                        //   + (first.y + t*ny - end.y)^2 = t^2
                                        // Expanding:
                                        //   |first - end|^2
                                        //   + 2*t*((first.x - end.x)*nx + (first.y - end.y)*ny)
                                        //   = 0
                                        // → t = -|first - end|^2 /
                                        //       (2 * ((first - end) · n))
                                        let dx = first_pos.0 - end_pos.0;
                                        let dy = first_pos.1 - end_pos.1;
                                        let denom = 2.0 * (dx * nx + dy * ny);
                                        let chord_sq = dx * dx + dy * dy;
                                        if denom.abs() <= 1e-9 {
                                            // end is on the line — tangent
                                            // circle is undefined (would be
                                            // infinite radius / a straight
                                            // line). Fall back to the chord
                                            // midpoint perpendicular.
                                            let mx = (first_pos.0 + end_pos.0) * 0.5;
                                            let my = (first_pos.1 + end_pos.1) * 0.5;
                                            (mx + nx * 0.5, my + ny * 0.5)
                                        } else {
                                            let t = -chord_sq / denom;
                                            (first_pos.0 + t * nx, first_pos.1 + t * ny)
                                        }
                                    }
                                }
                                None => {
                                    // Placeholder centre — perpendicular
                                    // to the chord at the midpoint, half
                                    // chord length out (gives a 90°
                                    // arc). The user will typically
                                    // re-constrain manually.
                                    editor.state.solve_warnings.push(
                                        "Tangent Arc: no incident line found, placeholder centre"
                                            .into(),
                                    );
                                    let mx = (first_pos.0 + end_pos.0) * 0.5;
                                    let my = (first_pos.1 + end_pos.1) * 0.5;
                                    let dx = end_pos.0 - first_pos.0;
                                    let dy = end_pos.1 - first_pos.1;
                                    // Rotate 90° CCW.
                                    (mx + (-dy) * 0.5, my + dx * 0.5)
                                }
                            };

                            // Mint the centre Point.
                            let centre_id = SketchEntityId::new();
                            let centre = flag(Entity::new(
                                centre_id,
                                plane_id,
                                EntityKind::Point { x: cx, y: cy },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(centre),
                                );
                            });

                            // Mint the Arc entity. Sweep direction
                            // chosen so the arc opens away from the
                            // incident line (when present); without a
                            // line, default CCW.
                            let arc_id = SketchEntityId::new();
                            let sweep_ccw = match incident_line {
                                Some((_, line_other_pos)) => {
                                    // Cross product of (line_other -> first)
                                    // and (first -> end) tells us which
                                    // side of the line `end` is on.
                                    let lx = first_pos.0 - line_other_pos.0;
                                    let ly = first_pos.1 - line_other_pos.1;
                                    let ex = end_pos.0 - first_pos.0;
                                    let ey = end_pos.1 - first_pos.1;
                                    // Cross > 0 → end is to the left of
                                    // the incoming line direction → CCW
                                    // arc opens left.
                                    lx * ey - ly * ex >= 0.0
                                }
                                None => true,
                            };
                            let arc = flag(Entity::new(
                                arc_id,
                                plane_id,
                                EntityKind::Arc {
                                    center: centre_id,
                                    start: first,
                                    end: resolved_id,
                                    sweep_ccw,
                                },
                            ));
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(arc),
                                );
                            });

                            // Add the TangentLineArc constraint when
                            // we have an incident Line.
                            if let Some((line_id, _)) = incident_line {
                                let constraint = Constraint {
                                    id: ConstraintId::new(),
                                    kind: ConstraintKind::TangentLineArc {
                                        line: line_id,
                                        arc: arc_id,
                                    },
                                };
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddConstraint(constraint),
                                    );
                                });
                            }

                            editor.state.tool_pending = ToolPending::Idle;
                        }
                        _ => {
                            // First click — stash the endpoint and
                            // wait for click 2.
                            editor.state.tool_pending =
                                ToolPending::TangentArcFirst { first: resolved_id };
                        }
                    }
                }
                SketchTool::CircularPattern => {
                    // v0.22 Phase B4 — Circular Pattern. Click 1
                    // picks the source entity. The polar array
                    // requires a centre Point — mint a fresh one
                    // 5 mm to the right of the click position so the
                    // array doesn't all stack on the source. Default
                    // count 4, sweep 360°.
                    use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
                    let centre_id = SketchEntityId::new();
                    let centre = flag(Entity::new(
                        centre_id,
                        plane_id,
                        EntityKind::Point {
                            x: x_mm + 5.0,
                            y: y_mm,
                        },
                    ));
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::AddEntity(centre),
                        );
                    });
                    let array = Array {
                        id: ArrayId::new(),
                        kind: ArrayKind::Polar {
                            source: resolved_id,
                            center: centre_id,
                            count_expr: "4".into(),
                            sweep_angle_expr: "360deg".into(),
                            depopulation: None,
                        },
                        numbering: NumberingScheme::default(),
                    };
                    let sketch = editor
                        .primitive_mut()
                        .sketch
                        .get_or_insert_with(signex_sketch::SketchData::default);
                    sketch.arrays.push(array);
                    editor.with_parts(|state, primitive| {
                        apply_sketch_edit_with_warnings(
                            state,
                            primitive,
                            SketchEdit::ForceRebuild,
                        );
                    });
                    editor.state.tool_pending = ToolPending::Idle;
                }
                SketchTool::Fillet => {
                    // v0.27 — EDA Fillet. Two-click gesture:
                    //   click 1: pick the first Line (we hit-test for
                    //     a Line near the click — fall back to a
                    //     warning if none).
                    //   click 2: pick the second Line that shares an
                    //     endpoint with the first. Compute tangent
                    //     points at radius `r` from the shared corner
                    //     along each line, splice in an Arc connecting
                    //     them centred on the angle bisector, and
                    //     shorten both lines to end at the tangent
                    //     points.
                    //
                    // Radius source — `state.placement_input` (kind
                    // FilletRadius) when the user typed one; else
                    // `state.dimension_input`; else 0.5 mm.
                    fn pick_line_at(
                        sketch: &signex_sketch::SketchData,
                        x: f64,
                        y: f64,
                    ) -> Option<SketchEntityId> {
                        const TOL_MM: f64 = 0.30;
                        let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                            sketch.entities.iter().find(|e| e.id == id).and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            })
                        };
                        let mut best: Option<(f64, SketchEntityId)> = None;
                        for e in &sketch.entities {
                            if let EntityKind::Line { start, end } = e.kind {
                                let (Some(a), Some(b)) = (pos_of(start), pos_of(end)) else {
                                    continue;
                                };
                                let dx = b.0 - a.0;
                                let dy = b.1 - a.1;
                                let llen2 = dx * dx + dy * dy;
                                if llen2 <= 1e-12 {
                                    continue;
                                }
                                let t = ((x - a.0) * dx + (y - a.1) * dy) / llen2;
                                let tc = t.clamp(0.0, 1.0);
                                let px = a.0 + tc * dx;
                                let py = a.1 + tc * dy;
                                let d2 = (px - x).powi(2) + (py - y).powi(2);
                                if d2 <= TOL_MM * TOL_MM
                                    && best.as_ref().is_none_or(|(b2, _)| d2 < *b2)
                                {
                                    best = Some((d2, e.id));
                                }
                            }
                        }
                        best.map(|(_, id)| id)
                    }

                    let click_xy = (x_mm, y_mm);
                    let radius_mm = editor
                        .state
                        .placement_input
                        .as_ref()
                        .filter(|p| p.kind == PlacementInputKind::FilletRadius)
                        .and_then(|p| p.buffer.parse::<f64>().ok())
                        .filter(|r| r.is_finite() && *r > 1e-9)
                        .unwrap_or_else(|| {
                            editor
                                .state
                                .dimension_input
                                .trim()
                                .parse::<f64>()
                                .ok()
                                .filter(|r| r.is_finite() && *r > 1e-9)
                                .unwrap_or(0.5)
                        });

                    match editor.state.tool_pending {
                        ToolPending::FilletFirst { line: first_line } => {
                            let sketch_ref = match editor.primitive().sketch.as_ref() {
                                Some(s) => s,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            let second_line = match pick_line_at(sketch_ref, click_xy.0, click_xy.1)
                            {
                                Some(id) if id != first_line => id,
                                _ => {
                                    editor.state.solve_warnings.push(
                                        "Fillet: second click missed a different Line — pick the adjacent line".into(),
                                    );
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            // Resolve the two Lines' endpoints.
                            let line_endpoints = |id: SketchEntityId| -> Option<(SketchEntityId, SketchEntityId)> {
                                sketch_ref.entities.iter().find(|e| e.id == id).and_then(
                                    |e| match e.kind {
                                        EntityKind::Line { start, end } => Some((start, end)),
                                        _ => None,
                                    },
                                )
                            };
                            let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
                                sketch_ref
                                    .entities
                                    .iter()
                                    .find(|e| e.id == id)
                                    .and_then(|e| match e.kind {
                                        EntityKind::Point { x, y } => Some((x, y)),
                                        _ => None,
                                    })
                            };
                            let (a_s, a_e) = match line_endpoints(first_line) {
                                Some(p) => p,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            let (b_s, b_e) = match line_endpoints(second_line) {
                                Some(p) => p,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            // Find the shared corner Point.
                            let corner_id = if a_s == b_s || a_s == b_e {
                                a_s
                            } else if a_e == b_s || a_e == b_e {
                                a_e
                            } else {
                                editor.state.solve_warnings.push(
                                    "Fillet: the two Lines do not share an endpoint — bridge them with a Coincident constraint first".into(),
                                );
                                editor.state.tool_pending = ToolPending::Idle;
                                return;
                            };
                            // Identify the "outer" endpoint of each line.
                            let a_other = if a_s == corner_id { a_e } else { a_s };
                            let b_other = if b_s == corner_id { b_e } else { b_s };
                            let (cx, cy) = match pos_of(corner_id) {
                                Some(p) => p,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            let (ax, ay) = match pos_of(a_other) {
                                Some(p) => p,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            let (bx, by) = match pos_of(b_other) {
                                Some(p) => p,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            // Direction unit vectors away from corner.
                            let dax = ax - cx;
                            let day = ay - cy;
                            let dbx = bx - cx;
                            let dby = by - cy;
                            let alen = (dax * dax + day * day).sqrt();
                            let blen = (dbx * dbx + dby * dby).sqrt();
                            if alen <= 1e-9 || blen <= 1e-9 {
                                editor.state.tool_pending = ToolPending::Idle;
                                return;
                            }
                            let aux = dax / alen;
                            let auy = day / alen;
                            let bux = dbx / blen;
                            let buy = dby / blen;
                            // Half-angle between the two lines via dot product.
                            let cos_theta = (aux * bux + auy * buy).clamp(-1.0, 1.0);
                            let theta = cos_theta.acos();
                            if theta < 1e-3 || (std::f64::consts::PI - theta) < 1e-3 {
                                editor.state.solve_warnings.push(
                                    "Fillet: lines are colinear — nothing to round".into(),
                                );
                                editor.state.tool_pending = ToolPending::Idle;
                                return;
                            }
                            let half = theta * 0.5;
                            // Distance from corner to tangent point along each line.
                            let trim = radius_mm / half.tan();
                            let cap = trim.min(alen * 0.999).min(blen * 0.999);
                            if cap < radius_mm * 0.05 {
                                editor.state.solve_warnings.push(
                                    "Fillet: radius too large for these lines — pick a smaller r".into(),
                                );
                                editor.state.tool_pending = ToolPending::Idle;
                                return;
                            }
                            let r_used = cap * half.tan();
                            let ta_x = cx + aux * cap;
                            let ta_y = cy + auy * cap;
                            let tb_x = cx + bux * cap;
                            let tb_y = cy + buy * cap;
                            // Arc centre — on the angle bisector at
                            // distance r / sin(half) from the corner.
                            let bis_x = (aux + bux).abs() + (auy + buy).abs();
                            let _ = bis_x; // appease borrow checker, no-op
                            let mid_x = aux + bux;
                            let mid_y = auy + buy;
                            let mid_len = (mid_x * mid_x + mid_y * mid_y).sqrt().max(1e-9);
                            let bx_unit = mid_x / mid_len;
                            let by_unit = mid_y / mid_len;
                            let centre_off = r_used / half.sin();
                            let centre_x = cx + bx_unit * centre_off;
                            let centre_y = cy + by_unit * centre_off;
                            // Determine sweep direction — the arc opens
                            // away from the corner; pick CCW if the
                            // cross product (a -> b) is positive.
                            let cross = aux * buy - auy * bux;
                            let sweep_ccw = cross > 0.0;
                            // Mint two new tangent Points + an Arc; replace
                            // the corner endpoint references on the source
                            // Lines with the new tangent Points so the
                            // arc bridges them. We do this by updating the
                            // existing Line entities in-place via the
                            // sketch (no SketchEdit::EditLine variant
                            // exists yet — fall back to delete + re-add).
                            let ta_id = SketchEntityId::new();
                            let tb_id = SketchEntityId::new();
                            let centre_id = SketchEntityId::new();
                            let arc_id = SketchEntityId::new();
                            let entities = vec![
                                flag(Entity::new(
                                    ta_id,
                                    plane_id,
                                    EntityKind::Point { x: ta_x, y: ta_y },
                                )),
                                flag(Entity::new(
                                    tb_id,
                                    plane_id,
                                    EntityKind::Point { x: tb_x, y: tb_y },
                                )),
                                flag(Entity::new(
                                    centre_id,
                                    plane_id,
                                    EntityKind::Point {
                                        x: centre_x,
                                        y: centre_y,
                                    },
                                )),
                                flag(Entity::new(
                                    arc_id,
                                    plane_id,
                                    EntityKind::Arc {
                                        center: centre_id,
                                        start: ta_id,
                                        end: tb_id,
                                        sweep_ccw,
                                    },
                                )),
                            ];
                            for ent in entities {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(ent),
                                    );
                                });
                            }
                            // Rewrite the two source Lines so the corner
                            // endpoint becomes the new tangent point.
                            // No public SketchEdit variant rewrites a
                            // Line's endpoints, so we mutate the schema
                            // directly and trigger a force-rebuild.
                            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                                for e in sketch.entities.iter_mut() {
                                    if e.id == first_line {
                                        if let EntityKind::Line { start, end } = &mut e.kind {
                                            if *start == corner_id {
                                                *start = ta_id;
                                            } else if *end == corner_id {
                                                *end = ta_id;
                                            }
                                        }
                                    }
                                    if e.id == second_line {
                                        if let EntityKind::Line { start, end } = &mut e.kind {
                                            if *start == corner_id {
                                                *start = tb_id;
                                            } else if *end == corner_id {
                                                *end = tb_id;
                                            }
                                        }
                                    }
                                }
                            }
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::ForceRebuild,
                                );
                            });
                            editor.state.tool_pending = ToolPending::Idle;
                        }
                        _ => {
                            // First click — pick the first Line.
                            let sketch_ref = match editor.primitive().sketch.as_ref() {
                                Some(s) => s,
                                None => {
                                    editor.state.tool_pending = ToolPending::Idle;
                                    return;
                                }
                            };
                            match pick_line_at(sketch_ref, click_xy.0, click_xy.1) {
                                Some(id) => {
                                    editor.state.tool_pending = ToolPending::FilletFirst { line: id };
                                }
                                None => {
                                    editor.state.solve_warnings.push(
                                        "Fillet: click missed any Line — try clicking closer to a line stroke".into(),
                                    );
                                    editor.state.tool_pending = ToolPending::Idle;
                                }
                            }
                        }
                    }
                }
                SketchTool::Trim => {
                    // v0.27 — EDA Trim. Single click on a Line: find
                    // its self-intersections with all other Lines,
                    // pick the two intersections that bracket the
                    // click point on the line, split the line into
                    // up-to-three segments, and remove the middle
                    // segment containing the click. If only one
                    // intersection exists, remove the side containing
                    // the click. If no intersection exists, remove
                    // the whole Line (Fusion-style "trim to nothing"
                    // is a useful EDA fallback for stripping a stray
                    // overlap).
                    fn line_xy(
                        sketch: &signex_sketch::SketchData,
                        id: SketchEntityId,
                    ) -> Option<((f64, f64), (f64, f64))> {
                        let pos_of = |pid: SketchEntityId| -> Option<(f64, f64)> {
                            sketch.entities.iter().find(|e| e.id == pid).and_then(|e| match e.kind {
                                EntityKind::Point { x, y } => Some((x, y)),
                                _ => None,
                            })
                        };
                        sketch.entities.iter().find(|e| e.id == id).and_then(|e| match e.kind {
                            EntityKind::Line { start, end } => {
                                Some((pos_of(start)?, pos_of(end)?))
                            }
                            _ => None,
                        })
                    }
                    fn pick_line_at_for_trim(
                        sketch: &signex_sketch::SketchData,
                        x: f64,
                        y: f64,
                    ) -> Option<SketchEntityId> {
                        const TOL_MM: f64 = 0.30;
                        let mut best: Option<(f64, SketchEntityId)> = None;
                        for e in &sketch.entities {
                            if let EntityKind::Line { .. } = e.kind
                                && let Some(((ax, ay), (bx, by))) = line_xy(sketch, e.id)
                            {
                                let dx = bx - ax;
                                let dy = by - ay;
                                let llen2 = dx * dx + dy * dy;
                                if llen2 <= 1e-12 {
                                    continue;
                                }
                                let t = ((x - ax) * dx + (y - ay) * dy) / llen2;
                                let tc = t.clamp(0.0, 1.0);
                                let px = ax + tc * dx;
                                let py = ay + tc * dy;
                                let d2 = (px - x).powi(2) + (py - y).powi(2);
                                if d2 <= TOL_MM * TOL_MM
                                    && best.as_ref().is_none_or(|(b2, _)| d2 < *b2)
                                {
                                    best = Some((d2, e.id));
                                }
                            }
                        }
                        best.map(|(_, id)| id)
                    }

                    let target_line = match editor.primitive().sketch.as_ref() {
                        Some(s) => pick_line_at_for_trim(s, x_mm, y_mm),
                        None => None,
                    };
                    let Some(target_line) = target_line else {
                        editor.state.solve_warnings.push(
                            "Trim: click missed any Line — try clicking closer to a line stroke".into(),
                        );
                        editor.state.tool_pending = ToolPending::Idle;
                        return;
                    };
                    // Compute intersections of `target_line` with every
                    // other Line; collect parametric `t` values.
                    let mut hits: Vec<f64> = Vec::new();
                    if let Some(s) = editor.primitive().sketch.as_ref()
                        && let Some(((ax, ay), (bx, by))) = line_xy(s, target_line)
                    {
                        let dx = bx - ax;
                        let dy = by - ay;
                        let llen2 = dx * dx + dy * dy;
                        if llen2 > 1e-12 {
                            for e in &s.entities {
                                if e.id == target_line {
                                    continue;
                                }
                                if let EntityKind::Line { .. } = e.kind
                                    && let Some(((cx, cy), (ex, ey))) = line_xy(s, e.id)
                                {
                                    let r_x = dx;
                                    let r_y = dy;
                                    let s_x = ex - cx;
                                    let s_y = ey - cy;
                                    let denom = r_x * s_y - r_y * s_x;
                                    if denom.abs() <= 1e-12 {
                                        continue;
                                    }
                                    let qx = cx - ax;
                                    let qy = cy - ay;
                                    let t = (qx * s_y - qy * s_x) / denom;
                                    let u = (qx * r_y - qy * r_x) / denom;
                                    if (1e-6..=1.0 - 1e-6).contains(&t)
                                        && (-1e-6..=1.0 + 1e-6).contains(&u)
                                    {
                                        hits.push(t);
                                    }
                                }
                            }
                        }
                        // Click t-value on target_line.
                        let click_t = if llen2 > 1e-12 {
                            ((x_mm - ax) * dx + (y_mm - ay) * dy) / llen2
                        } else {
                            0.5
                        };
                        // Bracketing the click between the nearest
                        // intersection below and above.
                        let lo = hits.iter().copied().filter(|t| *t < click_t).fold(0.0_f64, f64::max);
                        let hi = hits
                            .iter()
                            .copied()
                            .filter(|t| *t > click_t)
                            .fold(1.0_f64, f64::min);
                        // Three cases: full line (hits empty), half line
                        // (one hit), middle slice (two hits).
                        let trim_full = hits.is_empty();
                        let trim_lo = (lo - 0.0).abs() < 1e-9;
                        let trim_hi = (hi - 1.0).abs() < 1e-9;

                        if trim_full {
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::DeleteEntity(target_line),
                                );
                            });
                        } else if trim_lo && !trim_hi {
                            // Click is before the first intersection —
                            // shorten the line to start at `hi`.
                            let new_start = (ax + dx * hi, ay + dy * hi);
                            // Replace the line's start endpoint with a
                            // new Point at `new_start`.
                            let new_pid = SketchEntityId::new();
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(flag(Entity::new(
                                        new_pid,
                                        plane_id,
                                        EntityKind::Point {
                                            x: new_start.0,
                                            y: new_start.1,
                                        },
                                    ))),
                                );
                            });
                            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                                for e in sketch.entities.iter_mut() {
                                    if e.id == target_line
                                        && let EntityKind::Line { start, .. } = &mut e.kind
                                    {
                                        *start = new_pid;
                                    }
                                }
                            }
                        } else if trim_hi && !trim_lo {
                            // Click is after the last intersection —
                            // shorten the line to end at `lo`.
                            let new_end = (ax + dx * lo, ay + dy * lo);
                            let new_pid = SketchEntityId::new();
                            editor.with_parts(|state, primitive| {
                                apply_sketch_edit_with_warnings(
                                    state,
                                    primitive,
                                    SketchEdit::AddEntity(flag(Entity::new(
                                        new_pid,
                                        plane_id,
                                        EntityKind::Point {
                                            x: new_end.0,
                                            y: new_end.1,
                                        },
                                    ))),
                                );
                            });
                            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                                for e in sketch.entities.iter_mut() {
                                    if e.id == target_line
                                        && let EntityKind::Line { end, .. } = &mut e.kind
                                    {
                                        *end = new_pid;
                                    }
                                }
                            }
                        } else {
                            // Click bracketed by two intersections —
                            // split the line into two: [start..lo] and
                            // [hi..end]. We keep the original entity as
                            // the [start..lo] piece (rewriting its end)
                            // and mint a new Line for [hi..end].
                            let lo_pt = (ax + dx * lo, ay + dy * lo);
                            let hi_pt = (ax + dx * hi, ay + dy * hi);
                            let lo_pid = SketchEntityId::new();
                            let hi_pid = SketchEntityId::new();
                            let new_line_id = SketchEntityId::new();
                            // Capture the original end-point id so the
                            // mint of the second segment is correct.
                            let orig_end = if let Some(sk) = editor.primitive().sketch.as_ref() {
                                sk.entities.iter().find(|e| e.id == target_line).and_then(
                                    |e| match e.kind {
                                        EntityKind::Line { end, .. } => Some(end),
                                        _ => None,
                                    },
                                )
                            } else {
                                None
                            };
                            let Some(orig_end) = orig_end else {
                                editor.state.tool_pending = ToolPending::Idle;
                                return;
                            };
                            for ent in [
                                flag(Entity::new(
                                    lo_pid,
                                    plane_id,
                                    EntityKind::Point {
                                        x: lo_pt.0,
                                        y: lo_pt.1,
                                    },
                                )),
                                flag(Entity::new(
                                    hi_pid,
                                    plane_id,
                                    EntityKind::Point {
                                        x: hi_pt.0,
                                        y: hi_pt.1,
                                    },
                                )),
                                flag(Entity::new(
                                    new_line_id,
                                    plane_id,
                                    EntityKind::Line {
                                        start: hi_pid,
                                        end: orig_end,
                                    },
                                )),
                            ] {
                                editor.with_parts(|state, primitive| {
                                    apply_sketch_edit_with_warnings(
                                        state,
                                        primitive,
                                        SketchEdit::AddEntity(ent),
                                    );
                                });
                            }
                            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                                for e in sketch.entities.iter_mut() {
                                    if e.id == target_line
                                        && let EntityKind::Line { end, .. } = &mut e.kind
                                    {
                                        *end = lo_pid;
                                    }
                                }
                            }
                        }
                        editor.with_parts(|state, primitive| {
                            apply_sketch_edit_with_warnings(
                                state,
                                primitive,
                                SketchEdit::ForceRebuild,
                            );
                        });
                    }
                    editor.state.tool_pending = ToolPending::Idle;
                }
            }
            // v0.24 Track D — buffer is consumed once per click. The
            // user has to type again before the next gesture step,
            // mirroring Fusion. Always clear when the resolve step
            // honoured the buffer; leave alone otherwise so a stray
            // pre-tool-pending keystroke survives until the user
            // either commits or Esc-clears.
            if used_placement_input {
                editor.state.placement_input = None;
            }
            editor.canvas_cache.clear();
            editor.dirty = true;
        }
        // Symbol variants are no-ops on a Footprint editor.
        PrimitiveEditorMsg::SymbolSetTool(_)
        | PrimitiveEditorMsg::SymbolAddPin { .. }
        | PrimitiveEditorMsg::SymbolAddRectangle { .. }
        | PrimitiveEditorMsg::SymbolAddLine { .. }
        | PrimitiveEditorMsg::SymbolAddCircle { .. }
        | PrimitiveEditorMsg::SymbolAddArc { .. }
        | PrimitiveEditorMsg::SymbolAddText { .. }
        | PrimitiveEditorMsg::SymbolSelect(_)
        | PrimitiveEditorMsg::SymbolDeselect
        | PrimitiveEditorMsg::SymbolMoveSelected { .. }
        | PrimitiveEditorMsg::SymbolMoveGraphicHandle { .. }
        | PrimitiveEditorMsg::SymbolDeleteSelected
        | PrimitiveEditorMsg::SymbolSetPinNumber { .. }
        | PrimitiveEditorMsg::SymbolSetPinName { .. }
        | PrimitiveEditorMsg::SymbolPrevPart
        | PrimitiveEditorMsg::SymbolNextPart
        | PrimitiveEditorMsg::SymbolNewPart
        | PrimitiveEditorMsg::SymbolRemovePart
        | PrimitiveEditorMsg::SymbolPan { .. }
        | PrimitiveEditorMsg::SymbolZoom { .. }
        | PrimitiveEditorMsg::SymbolFit
        | PrimitiveEditorMsg::SymbolCursorAt { .. }
        | PrimitiveEditorMsg::SymbolSetSheetColor(_)
        | PrimitiveEditorMsg::SymbolToggleGrid
        | PrimitiveEditorMsg::SymbolCycleGridSize
        | PrimitiveEditorMsg::SymbolCycleUnit
        | PrimitiveEditorMsg::SymbolToggleActiveBarMenu(_)
        | PrimitiveEditorMsg::SymbolCloseActiveBarMenu
        | PrimitiveEditorMsg::SymbolActiveBarStub(_)
        | PrimitiveEditorMsg::SymbolToggleSelectionFilter(_)
        | PrimitiveEditorMsg::Save => {}
    }
}

/// v0.24 Phase 1 (Track B) — message-kind classifier driving the
/// `push_history` decision in [`apply_footprint_primitive_edit`].
/// Returns `true` for messages that mutate persisted footprint /
/// sketch state (so undo can roll them back), `false` for pure UI
/// state (selection, cursor tracking, tool mode toggles, panel
/// pickers — these don't enter the history because rolling back a
/// "click happened here" doesn't make sense to the user).
///
/// Lean toward `true` when in doubt — extra history entries cost
/// memory but never break correctness; missing entries leave edits
/// unreversable.
fn mutates_footprint_state(msg: &PrimitiveEditorMsg) -> bool {
    use PrimitiveEditorMsg::*;
    match msg {
        // Pure UI state — selection / hover / cursor / tool mode.
        // These don't change persisted geometry and shouldn't enter
        // the history.
        FootprintCursorAt { .. }
        | FootprintSelectPad(_)
        | FootprintSelectSilkF(_)
        | FootprintToggleLayer(_)
        | FootprintSetPadsTool(_)
        | FootprintToolEscape
        | FootprintToggleActiveBarMenu(_)
        | FootprintCloseActiveBarMenu
        | FootprintActiveBarStub(_)
        | FootprintActiveBarToggleSnap(_)
        | FootprintActiveBarSetSnappingMode(_)
        | FootprintActiveBarSetSnapSubTab(_)
        | FootprintActiveBarSelectAll
        | FootprintActiveBarClearSelection
        | FootprintActiveBarSetSketchTool(_)
        | FootprintSetMode(_)
        | FootprintSketchSetTool(_)
        | FootprintSketchToggleConstruction
        | FootprintSketchToggleCenterline
        | FootprintTogglePlacementPause
        | FootprintSketchToolEscape
        // v0.24 Track D — placement-input keypress messages mutate
        // only the transient `placement_input` overlay buffer; they
        // don't touch persisted geometry, so undo doesn't need them.
        | FootprintSketchPlacementInputChar(_)
        | FootprintSketchPlacementInputBackspace
        | FootprintSketchPlacementInputEnter
        | FootprintSketchPlacementInputEscape
        | FootprintSketchSelect { .. }
        | FootprintSketchDimensionInput(_)
        | FootprintToggleSelectionFilter(_)
        | FootprintToggleAutoFit
        | FootprintSelectActiveIdx(_)
        | FootprintShowContextMenu { .. }
        | FootprintCloseContextMenu
        | FootprintContextMenuOpenSubmenu(_)
        | FootprintContextMenuAction(_)
        | FootprintFitConsumed
        // v0.26-E — clipboard ops handle their own push_history at
        // call site, so the snapshot-classifier here returns false
        // (Copy mutates nothing; Cut + Paste already snapshotted).
        | FootprintCopyPad
        | FootprintCutPad
        | FootprintPastePad
        | Save => false,
        // All other variants either add/remove/move geometry,
        // mutate pad attributes, or rebuild the sketch — they all
        // need a history snapshot.
        _ => true,
    }
}

/// Apply inline-edit messages directly to a Component Preview state.
/// Tab switching, save, and async-bounce variants are handled before
/// reaching here — this is the catch-all for in-place row mutations
/// (parameters / supply / datasheet / pin-map / simulation).
pub(crate) fn apply_inline_edit(state: &mut ComponentPreviewState, msg: EditorMsg) {
    match msg {
        EditorMsg::SelectTab(tab) => state.active_tab = tab,
        // Component-level setters
        EditorMsg::SetLifecycle(s) => {
            state.row.state = s;
            state.dirty = true;
        }
        // Datasheet
        EditorMsg::DatasheetSetMode(mode) => {
            use crate::library::editor::datasheet_picker::DatasheetMode;
            match mode {
                DatasheetMode::Url => match &state.row.datasheet {
                    signex_library::DatasheetRef::Url { .. } => {}
                    _ => {
                        state.row.datasheet = signex_library::DatasheetRef::default();
                        state.dirty = true;
                    }
                },
                DatasheetMode::PinnedPdf => match &state.row.datasheet {
                    signex_library::DatasheetRef::HashPinned { .. } => {}
                    _ => {
                        state.row.datasheet = signex_library::DatasheetRef::HashPinned {
                            hash: String::new(),
                            filename: String::new(),
                        };
                        state.dirty = true;
                    }
                },
            }
        }
        EditorMsg::DatasheetSetUrl(s) => {
            let trimmed = s.trim();
            state.row.datasheet = if trimmed.is_empty() {
                signex_library::DatasheetRef::default()
            } else {
                signex_library::DatasheetRef::url(trimmed)
            };
            state.dirty = true;
        }
        EditorMsg::DatasheetUploadResult(payload) => {
            if let Some((bytes, filename)) = payload {
                use sha2::Digest;
                let mut hasher = sha2::Sha256::new();
                hasher.update(&bytes);
                let hash = format!("{:x}", hasher.finalize());
                state.row.datasheet = signex_library::DatasheetRef::hash_pinned(hash, filename);
                state.dirty = true;
            }
        }
        // Pin Map
        EditorMsg::PinMapAutoMatchByNumber | EditorMsg::PinMapClearOverrides => {
            state.row.pin_map_overrides.clear();
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
            state.dirty = true;
        }
        EditorMsg::PinMapAutoMatchByName => {
            tracing::warn!(
                target: "signex::library",
                "Pin Map: Auto-Match by Name is stubbed; awaiting heuristic implementation"
            );
        }
        EditorMsg::PinMapOpenOverrideEdit(pin) => {
            let seed = state
                .row
                .pin_map_overrides
                .iter()
                .find(|o| o.symbol_pin_number == pin)
                .map(|o| o.footprint_pad_number.clone())
                .unwrap_or_default();
            state.pin_map_state.expanded_row = Some(pin);
            state.pin_map_state.override_buf = seed;
        }
        EditorMsg::PinMapOverrideBufChanged { pin, value } => {
            if state.pin_map_state.expanded_row.as_deref() == Some(pin.as_str()) {
                state.pin_map_state.override_buf = value;
            }
        }
        EditorMsg::PinMapAddOverride { pin, pad } => {
            let trimmed = pad.trim();
            if trimmed.is_empty() {
                state
                    .row
                    .pin_map_overrides
                    .retain(|o| o.symbol_pin_number != pin);
            } else if let Some(existing) = state
                .row
                .pin_map_overrides
                .iter_mut()
                .find(|o| o.symbol_pin_number == pin)
            {
                existing.footprint_pad_number = trimmed.to_string();
            } else {
                state
                    .row
                    .pin_map_overrides
                    .push(signex_library::PinPadOverride::new(pin, trimmed));
            }
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
            state.dirty = true;
        }
        EditorMsg::PinMapCancelOverrideEdit => {
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
        }
        EditorMsg::PinMapRemoveOverride { pin } => {
            state
                .row
                .pin_map_overrides
                .retain(|o| o.symbol_pin_number != pin);
            state.pin_map_state.expanded_row = None;
            state.pin_map_state.override_buf.clear();
            state.dirty = true;
        }
        // Supply — primary
        EditorMsg::SupplyPrimarySetManufacturer(s) => {
            state.row.primary_mpn.manufacturer = s;
            state.dirty = true;
        }
        EditorMsg::SupplyPrimarySetMpn(s) => {
            state.row.primary_mpn.mpn = s;
            state.dirty = true;
        }
        EditorMsg::SupplyPrimarySetStatus(s) => {
            state.row.primary_mpn.status = s;
            state.dirty = true;
        }
        EditorMsg::SupplyPrimarySetNotes(s) => {
            state.row.primary_mpn.notes = if s.trim().is_empty() { None } else { Some(s) };
            state.dirty = true;
        }
        // Supply — alternates
        EditorMsg::SupplyAlternateAdd => {
            let mut alt = signex_library::ManufacturerPart::draft("", "");
            alt.status = signex_library::AlternateStatus::Approved;
            state.row.alternates.push(alt);
            state.dirty = true;
        }
        EditorMsg::SupplyAlternateSetManufacturer { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.manufacturer = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetMpn { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.mpn = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetStatus { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.status = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateSetNotes { idx, value } => {
            if let Some(alt) = state.row.alternates.get_mut(idx) {
                alt.notes = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
                state.dirty = true;
            }
        }
        EditorMsg::SupplyAlternateRemove { idx } => {
            if idx < state.row.alternates.len() {
                state.row.alternates.remove(idx);
                state.dirty = true;
            }
        }
        // Supply — listings
        EditorMsg::SupplyListingAdd => {
            state.row.supply.push(signex_library::DistributorListing {
                distributor: String::new(),
                sku: String::new(),
                url: None,
                moq: None,
            });
            state.dirty = true;
        }
        EditorMsg::SupplyListingSetDistributor { idx, value } => {
            if let Some(listing) = state.row.supply.get_mut(idx) {
                listing.distributor =
                    crate::library::editor::supply::distributor_source_to_string(value);
                state.dirty = true;
            }
        }
        EditorMsg::SupplyListingSetSku { idx, value } => {
            if let Some(listing) = state.row.supply.get_mut(idx) {
                listing.sku = value;
                state.dirty = true;
            }
        }
        EditorMsg::SupplyListingSetUrl { idx, value } => {
            if let Some(listing) = state.row.supply.get_mut(idx) {
                listing.url = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
                state.dirty = true;
            }
        }
        EditorMsg::SupplyListingRemove { idx } => {
            if idx < state.row.supply.len() {
                state.row.supply.remove(idx);
                state.dirty = true;
            }
        }
        // Parameters
        EditorMsg::ParamSetText { name, value } => {
            if !name.is_empty() {
                state
                    .row
                    .parameters
                    .insert(name.clone(), signex_library::ParamValue::Text(value));
                state.dirty = true;
            }
        }
        EditorMsg::ParamSetNumberBuf { name, buf } => {
            state.params_edit_buf.insert(name, buf);
        }
        EditorMsg::ParamCommitNumber { name } => {
            if let Some(buf) = state.params_edit_buf.get(&name).cloned() {
                let trimmed = buf.trim();
                if let Ok(v) = trimmed.parse::<f64>() {
                    state
                        .row
                        .parameters
                        .insert(name, signex_library::ParamValue::Number(v));
                    state.dirty = true;
                }
            }
        }
        EditorMsg::ParamSetMeasurementBuf { name, buf } => {
            state.params_edit_buf.insert(name, buf);
        }
        EditorMsg::ParamCommitMeasurement { name, unit } => {
            if let Some(buf) = state.params_edit_buf.get(&name).cloned() {
                let trimmed = buf.trim();
                if let Ok(v) = trimmed.parse::<f64>() {
                    state.row.parameters.insert(
                        name,
                        signex_library::ParamValue::Measurement { value: v, unit },
                    );
                    state.dirty = true;
                }
            }
        }
        EditorMsg::ParamSetBool { name, value } => {
            state
                .row
                .parameters
                .insert(name, signex_library::ParamValue::Bool(value));
            state.dirty = true;
        }
        EditorMsg::ParamRemove { name } => {
            state.row.parameters.remove(&name);
            state.dirty = true;
        }
        EditorMsg::ParamAddCustom { name, kind } => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return;
            }
            let value = match kind {
                ParamKindMsg::Text => signex_library::ParamValue::Text(String::new()),
                ParamKindMsg::Number => signex_library::ParamValue::Number(0.0),
                ParamKindMsg::Bool => signex_library::ParamValue::Bool(false),
                ParamKindMsg::Measurement(unit) => {
                    signex_library::ParamValue::Measurement { value: 0.0, unit }
                }
            };
            state.row.parameters.insert(trimmed.to_string(), value);
            state.dirty = true;
        }
        // Sim
        EditorMsg::SimSetEnabled(enabled) => {
            if enabled {
                if state.row.sim_ref.is_none() {
                    let sim = signex_library::SimModel {
                        uuid: uuid::Uuid::now_v7(),
                        name: state.row.internal_pn.as_str().to_string(),
                        kind: signex_library::SimKind::Spice3,
                        body: String::new(),
                        default_node_map: std::collections::BTreeMap::new(),
                        // Stage 14: every primitive carries its own
                        // semver string + released flag. Defaults match
                        // the serde defaults so reads of pre-Stage-14
                        // `.snxsim` files work.
                        version: "0.0.1".into(),
                        released: false,
                        created: chrono::Utc::now(),
                        updated: chrono::Utc::now(),
                    };
                    state.row.sim_ref = Some(signex_library::PrimitiveRef::new(
                        state.row.symbol_ref.library_id,
                        sim.uuid,
                    ));
                    state.sim_body = Some(iced::widget::text_editor::Content::new());
                    state.sim = Some(sim);
                }
            } else {
                state.row.sim_ref = None;
                state.sim = None;
                state.sim_body = None;
            }
            state.dirty = true;
        }
        EditorMsg::SimSetKind(kind) => {
            if let Some(sim) = state.sim.as_mut() {
                sim.kind = kind;
                sim.updated = chrono::Utc::now();
                state.dirty = true;
            }
        }
        EditorMsg::SimSetName(name) => {
            if let Some(sim) = state.sim.as_mut() {
                sim.name = name;
                sim.updated = chrono::Utc::now();
                state.dirty = true;
            }
        }
        EditorMsg::SimBodyAction(action) => {
            if let Some(content) = state.sim_body.as_mut() {
                content.perform(action);
                if let Some(sim) = state.sim.as_mut() {
                    sim.body = content.text();
                    sim.updated = chrono::Utc::now();
                }
                state.dirty = true;
            }
        }
        EditorMsg::SimSetPinNode { pin_number, value } => {
            if let Some(sim) = state.sim.as_mut() {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    sim.default_node_map.remove(&pin_number);
                } else {
                    sim.default_node_map.insert(pin_number, trimmed.to_string());
                }
                sim.updated = chrono::Utc::now();
                state.dirty = true;
            }
        }
        // Variants below are kept around for the standalone primitive
        // editors (`.snxsym` / `.snxfpt` document tabs); they're
        // never fired through the Component Preview surface but stay
        // defined to keep the message tree backwards-compatible.
        EditorMsg::CloseEditor
        | EditorMsg::SaveDraft
        | EditorMsg::Commit
        | EditorMsg::SubmitForReview
        | EditorMsg::SubmitForReviewNotesChanged(_)
        | EditorMsg::SubmitForReviewCancel
        | EditorMsg::SubmitForReviewConfirm
        | EditorMsg::SubmitForReviewResult(_)
        | EditorMsg::OpenWhereUsedTab
        | EditorMsg::DatasheetUploadDialog
        | EditorMsg::SymbolPickAiPdf
        | EditorMsg::SymbolPickedAiPdf(_)
        | EditorMsg::SymbolSetTool(_)
        | EditorMsg::SymbolAddPin { .. }
        | EditorMsg::SymbolSelect(_)
        | EditorMsg::SymbolDeselect
        | EditorMsg::SymbolMoveSelected { .. }
        | EditorMsg::SymbolDeleteSelected
        | EditorMsg::SymbolSetField { .. }
        | EditorMsg::SymbolSetPinNumber { .. }
        | EditorMsg::SymbolSetPinName { .. }
        | EditorMsg::SymbolApplyAiPreview
        | EditorMsg::SymbolDismissAiPreview
        | EditorMsg::SaveSymbol(_, _)
        | EditorMsg::FootprintAddPad { .. }
        | EditorMsg::FootprintAddHole { .. }
        | EditorMsg::FootprintAddText { .. }
        | EditorMsg::FootprintTrackClick { .. }
        | EditorMsg::FootprintTrackCancel
        | EditorMsg::FootprintArcClick { .. }
        | EditorMsg::FootprintArcCancel
        | EditorMsg::FootprintPolygonClick { .. }
        | EditorMsg::FootprintPolygonCommit
        | EditorMsg::FootprintPolygonCancel
        | EditorMsg::FootprintSelectSilkF(_)
        | EditorMsg::FootprintDeleteSilkF
        | EditorMsg::FootprintSketchPlacePoint { .. }
        | EditorMsg::FootprintSketchToolClick { .. }
        | EditorMsg::FootprintSketchToolEscape
        | EditorMsg::FootprintSketchPlacementInputChar(_)
        | EditorMsg::FootprintSketchPlacementInputBackspace
        | EditorMsg::FootprintSketchPlacementInputEnter
        | EditorMsg::FootprintSketchPlacementInputEscape
        | EditorMsg::FootprintSketchSelect { .. }
        | EditorMsg::FootprintSketchMovePoint { .. }
        | EditorMsg::FootprintSketchMoveLine { .. }
        | EditorMsg::FootprintSketchResizeRoundPad { .. }
        | EditorMsg::FootprintSetSelectionMode2d(_)
        | EditorMsg::FootprintSelectAllOnLayer
        | EditorMsg::FootprintAddVia { .. }
        | EditorMsg::FootprintSelectOffGridPads
        | EditorMsg::FootprintRecomputeCourtyardOutline
        | EditorMsg::FootprintLassoArm
        | EditorMsg::FootprintLassoAddVertex { .. }
        | EditorMsg::FootprintLassoCommit
        | EditorMsg::FootprintLassoCancel
        | EditorMsg::FootprintTouchingLineArm
        | EditorMsg::FootprintTouchingLineFirst { .. }
        | EditorMsg::FootprintTouchingLineCommit { .. }
        | EditorMsg::FootprintTouchingLineCancel
        | EditorMsg::FootprintSelectOverlapped
        | EditorMsg::FootprintSelectNextOverlapped
        | EditorMsg::FootprintMovePad { .. }
        | EditorMsg::FootprintCursorAt { .. }
        | EditorMsg::FootprintSelectPad(_)
        | EditorMsg::FootprintSelectPads(_)
        | EditorMsg::FootprintSketchSelectMany(_)
        | EditorMsg::FootprintDeleteSelected
        | EditorMsg::FootprintToggleLayer(_)
        | EditorMsg::FootprintToggleAutoFit
        | EditorMsg::FootprintSetPadsTool(_)
        | EditorMsg::FootprintSketchSetTool(_)
        | EditorMsg::FootprintSketchToggleConstruction
        | EditorMsg::FootprintSketchToggleCenterline
        | EditorMsg::FootprintTogglePlacementPause
        | EditorMsg::FootprintShowContextMenu { .. }
        | EditorMsg::FootprintCloseContextMenu
        | EditorMsg::FootprintContextMenuOpenSubmenu(_)
        | EditorMsg::FootprintContextMenuAction(_)
        | EditorMsg::FootprintFitConsumed
        | EditorMsg::FootprintCopyPad
        | EditorMsg::FootprintCutPad
        | EditorMsg::FootprintPastePad
        | EditorMsg::FootprintActiveBarRotateSelection
        | EditorMsg::FootprintActiveBarFlipSelection
        | EditorMsg::FootprintSketchSetRole { .. }
        | EditorMsg::SaveFootprint(_, _)
        | EditorMsg::SetBodyHeight(_)
        | EditorMsg::SetBodyOffsetZ(_)
        | EditorMsg::SetBodyTopColor(_)
        | EditorMsg::SetBodySideColor(_)
        | EditorMsg::SetBodyShape(_)
        | EditorMsg::StepAttachDialog
        | EditorMsg::StepAttachResult(_)
        | EditorMsg::StepAttachRemove
        | EditorMsg::SaveSim(_, _) => {}
    }
}

// ─────────────────────────────────────────────────────────────────────
// Stage 10 — recovery dialog plumbing
// ─────────────────────────────────────────────────────────────────────
//
// `LocalGitAdapter::open` returns a few recoverable error shapes that
// shouldn't drop on the floor as a bare `tracing::warn!`. The user
// either wants to point Signex at a moved file, accept that history
// is gone, or remove the library from the project entirely. The
// recovery module owns the modal layer; this section owns the
// classification + per-choice action.

use crate::library::recovery::{
    BrokenBindingChoice, GitMissingChoice, LibraryMissingChoice, RecoveryDialog,
};
use crate::library::state::LibraryState;
use signex_library::{LibraryError, LocalGitAdapter};

/// Classify a `LocalGitAdapter::open` error and, if recoverable,
/// stash the matching `RecoveryDialog` on `LibraryState::recovery`.
/// Unrecoverable errors are left alone — the caller's `tracing::warn!`
/// is the only surface.
///
/// String-matches the error message produced by `LocalGitAdapter::open`
/// because the underlying `LibraryError` enum doesn't carry structured
/// "missing-snxlib" / "missing-git" variants in v0.9. This is the
/// lower-effort path called out in `v0.9-snxlib-as-file-plan.md` §2
/// Stage H — adding `LibraryError::MissingGitRepo` /
/// `LibraryError::MissingSnxlibFile` variants is a clean follow-up
/// once the rest of v0.9 settles.
pub(crate) fn route_open_error(
    state: &mut LibraryState,
    path: &std::path::Path,
    err: &LibraryError,
) {
    // Don't clobber an already-open recovery dialog; the user resolves
    // them sequentially.
    if state.recovery.is_some() {
        return;
    }
    let dialog = match err {
        LibraryError::NotFound(msg) if msg.contains("no .snxlib") => {
            Some(RecoveryDialog::LibraryMissing {
                path: path.to_path_buf(),
            })
        }
        LibraryError::Backend(msg) if msg.starts_with("git open") => {
            // v0.9: no remote field on the manifest yet. Stage 13+ will
            // populate this from `[users.<remote>]` so the
            // "Restore from remote" button activates.
            Some(RecoveryDialog::GitMissing {
                path: path.to_path_buf(),
                remote: None,
            })
        }
        _ => None,
    };
    if let Some(d) = dialog {
        state.recovery = Some(d);
    }
}

/// Handle the user's choice from the *Library missing* recovery dialog.
fn handle_recovery_library_missing(
    app: &mut Signex,
    choice: LibraryMissingChoice,
) -> Task<Message> {
    match choice {
        LibraryMissingChoice::Cancel => {
            app.library.recovery = None;
            Task::none()
        }
        LibraryMissingChoice::Locate => Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Locate Library (*.snxlib)")
                    .add_filter("Signex Library", &["snxlib"])
                    .pick_file()
                    .await
                    .map(|f| f.path().to_path_buf())
            },
            |path| Message::Library(LibraryMessage::RecoveryLibraryMissingLocateResult(path)),
        ),
        LibraryMissingChoice::RemoveFromProject => {
            let missing = match app.library.recovery.as_ref() {
                Some(RecoveryDialog::LibraryMissing { path }) => path.clone(),
                _ => {
                    app.library.recovery = None;
                    return Task::none();
                }
            };
            for project in app.document_state.projects.iter_mut() {
                // Compute resolved paths up-front so the closure can
                // borrow only the indices vector, not project.data
                // (which retain's closure also tries to read).
                let resolved: Vec<std::path::PathBuf> = project
                    .data
                    .libraries
                    .iter()
                    .map(|e| project.data.resolve_library_path(e))
                    .collect();
                let mut idx = 0usize;
                project.data.libraries.retain(|_| {
                    let keep = resolved[idx] != missing;
                    idx += 1;
                    keep
                });
            }
            app.library.recovery = None;
            Task::none()
        }
    }
}

/// Handle the user's choice from the *Git missing* recovery dialog.
fn handle_recovery_git_missing(app: &mut Signex, choice: GitMissingChoice) -> Task<Message> {
    match choice {
        GitMissingChoice::Cancel | GitMissingChoice::Skip => {
            app.library.recovery = None;
            Task::none()
        }
        GitMissingChoice::ReInit => {
            let path = match app.library.recovery.as_ref() {
                Some(RecoveryDialog::GitMissing { path, .. }) => path.clone(),
                _ => {
                    app.library.recovery = None;
                    return Task::none();
                }
            };
            app.library.recovery = None;
            match LocalGitAdapter::recover_init(&path) {
                Ok(_) => Task::done(Message::Library(LibraryMessage::OpenLibraryAt(Some(path)))),
                Err(e) => {
                    tracing::warn!(
                        target: "signex::library",
                        path = %path.display(),
                        error = %e,
                        "git recover-init failed"
                    );
                    Task::none()
                }
            }
        }
        GitMissingChoice::RestoreFromRemote => {
            // v0.9 leaves this disabled — the manifest doesn't carry a
            // remote yet. Treat as Cancel.
            app.library.recovery = None;
            Task::none()
        }
    }
}

/// Handle the user's choice from the *Broken primitive binding* dialog.
///
/// v0.9 stub: the dispatch path that detects broken bindings hasn't
/// landed yet (Stage 12+ wires the row-load checks). The handler
/// therefore only knows how to close the dialog; the actual rebind /
/// remove-row flows queue behind the detection plumbing. The dialog
/// surface itself ships now so the overlay layer is in place.
fn handle_recovery_broken_binding(app: &mut Signex, _choice: BrokenBindingChoice) -> Task<Message> {
    app.library.recovery = None;
    Task::none()
}

// ─────────────────────────────────────────────────────────────────────
// Stage 16 — Library Updates Available scan + apply (§3.5)
// ─────────────────────────────────────────────────────────────────────

impl Signex {
    /// Scan the schematic at `path` for placed Symbols whose
    /// `library_version` drifted from the source row's current
    /// version. Splits into two control paths:
    ///
    /// * **Personal** workflow mode — auto-applies every drift to the
    ///   schematic engine silently and dirty-marks the path.
    /// * **Team** workflow mode — populates
    ///   `LibraryState::library_updates` with the entries so
    ///   `view_main_for` opens the modal on the next tick.
    ///
    /// Adapter / library-mount failures degrade to a single
    /// `tracing::warn` line per affected library and skip those
    /// entries — a missing library doesn't abort the schematic open.
    /// Symbols without a `library_id` (Standard-imported, hand-built)
    /// are skipped silently.
    pub(crate) fn scan_library_updates_for_open_schematic(
        &mut self,
        schematic_path: std::path::PathBuf,
    ) {
        use signex_library::WorkflowMode;
        // Snapshot the placed-Symbol identity tuples first so we can
        // mutate the engine in the apply loop below without holding
        // the engine borrow across the library-set lookups.
        let symbol_refs: Vec<(uuid::Uuid, String, uuid::Uuid, RowId, String)> =
            match self.document_state.engines.get(&schematic_path) {
                Some(engine) => engine
                    .document()
                    .symbols
                    .iter()
                    .filter_map(|s| {
                        let library_id = s.library_id?;
                        let row_uuid = s.row_id?;
                        Some((
                            s.uuid,
                            s.reference.clone(),
                            library_id,
                            RowId::from_uuid(row_uuid),
                            s.library_version.clone(),
                        ))
                    })
                    .collect(),
                None => return,
            };

        if symbol_refs.is_empty() {
            return;
        }

        // Group entries by Team-mode source library — Personal-mode
        // libraries auto-apply silently and short-circuit the modal.
        let mut team_entries: Vec<crate::library::updates_dialog::LibraryUpdateEntry> = Vec::new();
        let mut personal_apply: Vec<(uuid::Uuid, String)> = Vec::new();

        // Adapter access lives on the LibrarySet; keep the borrow
        // alive only inside this closure-scope so the apply mutation
        // below can take a different `&mut self.document_state`.
        for (symbol_uuid, ref_des, library_id, row_id, current_version) in &symbol_refs {
            // Resolve the library entry (display_name + path) +
            // adapter handle.
            let Some(open_lib) = self
                .library
                .open_libraries
                .iter()
                .find(|lib| lib.library_id == *library_id)
            else {
                tracing::warn!(
                    target: "signex::library",
                    library_id = %library_id,
                    symbol = %symbol_uuid,
                    "library_updates_scan: library not mounted; skipping drift check"
                );
                continue;
            };
            let library_path = open_lib.root.clone();
            let library_name = open_lib.display_name.clone();

            let Some(adapter) = self.library.set.get(*library_id) else {
                tracing::warn!(
                    target: "signex::library",
                    library_id = %library_id,
                    "library_updates_scan: adapter missing on LibrarySet; skipping"
                );
                continue;
            };

            let mode = adapter.manifest().workflow.mode;

            // Locate the row inside its table — we don't know the
            // table name on the schematic side, so iterate the
            // adapter's tables and look for `row_id`.
            let Ok(table_names) = adapter.list_tables() else {
                continue;
            };
            let mut found: Option<(String, signex_library::ComponentRow)> = None;
            for name in &table_names {
                if let Ok(row) = adapter.read_row(name, *row_id) {
                    found = Some((name.clone(), row));
                    break;
                }
            }
            let Some((_table, row)) = found else {
                tracing::warn!(
                    target: "signex::library",
                    library_id = %library_id,
                    row_id = %row_id,
                    "library_updates_scan: row not found in any table; skipping"
                );
                continue;
            };

            if row.version == *current_version {
                continue; // No drift.
            }

            match mode {
                WorkflowMode::Personal => {
                    personal_apply.push((*symbol_uuid, row.version.clone()));
                }
                WorkflowMode::Team | WorkflowMode::Enterprise => {
                    let bump_kind = crate::library::updates_dialog::classify_bump(
                        current_version,
                        &row.version,
                    );
                    team_entries.push(crate::library::updates_dialog::LibraryUpdateEntry {
                        symbol_uuid: *symbol_uuid,
                        ref_des: ref_des.clone(),
                        library_id: *library_id,
                        library_name: library_name.clone(),
                        row_id: *row_id,
                        library_path: library_path.clone(),
                        current_version: current_version.clone(),
                        latest_version: row.version.clone(),
                        bump_kind,
                        selected: false,
                    });
                }
            }
        }

        // Apply the Personal-mode auto-updates first (silent path).
        if !personal_apply.is_empty()
            && let Some(engine) = self.document_state.engines.get_mut(&schematic_path)
        {
            let mut document = engine.document().clone();
            let mut applied = 0usize;
            for symbol in &mut document.symbols {
                if let Some((_, latest)) =
                    personal_apply.iter().find(|(uuid, _)| uuid == &symbol.uuid)
                {
                    symbol.library_version = latest.clone();
                    applied += 1;
                }
            }
            engine.set_document(document);
            self.document_state
                .dirty_paths
                .insert(schematic_path.clone());
            tracing::info!(
                target: "signex::library",
                schematic = %schematic_path.display(),
                count = applied,
                "library_updates_scan: auto-applied {} update(s) under Personal mode",
                applied
            );
        }

        // Surface the Team-mode entries via the modal state.
        if !team_entries.is_empty() {
            let state = crate::library::updates_dialog::LibraryUpdatesState::new(
                schematic_path.clone(),
                team_entries,
            );
            self.library.library_updates = Some(state);
        } else {
            // Re-scan with no drift — clear any persistent indicator
            // tied to this schematic.
            self.library.skipped_updates_for.remove(&schematic_path);
        }
    }

    /// Apply the user's selected updates from the Library Updates
    /// modal to the schematic engine. Drops the modal state on
    /// success; on apply, dirty-marks the schematic and clears its
    /// "skipped" indicator (the user committed an update, the path is
    /// no longer ambiguously skipped).
    pub(crate) fn handle_library_updates_apply(&mut self) {
        let Some(state) = self.library.library_updates.take() else {
            return;
        };
        let schematic_path = state.schematic_path.clone();
        let updates: Vec<(uuid::Uuid, String)> = state
            .entries
            .iter()
            .filter(|e| e.selected)
            .map(|e| (e.symbol_uuid, e.latest_version.clone()))
            .collect();
        if updates.is_empty() {
            // User clicked Apply with nothing selected — record as
            // skipped so the indicator stays visible.
            self.library.skipped_updates_for.insert(schematic_path);
            return;
        }
        if let Some(engine) = self.document_state.engines.get_mut(&schematic_path) {
            let mut document = engine.document().clone();
            for symbol in &mut document.symbols {
                if let Some((_, latest)) = updates.iter().find(|(uuid, _)| uuid == &symbol.uuid) {
                    symbol.library_version = latest.clone();
                }
            }
            engine.set_document(document);
            self.document_state
                .dirty_paths
                .insert(schematic_path.clone());
        }
        // If some entries were left unchecked, treat the schematic as
        // "still has skipped drift" so the status bar keeps its
        // indicator. Equal counts (everyone selected) clear the flag.
        let any_unchecked = state.entries.iter().any(|e| !e.selected);
        if any_unchecked {
            self.library.skipped_updates_for.insert(schematic_path);
        } else {
            self.library.skipped_updates_for.remove(&schematic_path);
        }
    }
}
