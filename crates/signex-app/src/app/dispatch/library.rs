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
        if let Err(e) = signex_types::atomic_io::atomic_write(&path, text.as_bytes()) {
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
        if let Err(e) = signex_types::atomic_io::atomic_write(&path, text.as_bytes()) {
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
        msg: PrimitiveEdit,
    ) -> Task<Message> {
        match msg {
            // Save is a sibling of the canvas-mutation messages — route
            // through the standalone save path which writes JSON back to
            // disk and (when applicable) reloads in the LibrarySet. When
            // the file doesn't exist on disk yet (newly-minted in-memory
            // tab from `Add New ▸ Symbol` / `Add New ▸ Footprint`), spawn
            // the Save-As dialog instead so the user picks where it lands
            // — same gate as the top-level `Message::SaveFile` path uses.
            PrimitiveEdit::Save => {
                if !path.exists() {
                    return crate::app::handlers::document_files::spawn_save_as_for_new_primitive(
                        path,
                    );
                }
                self.save_primitive_tab_at(&path);
                Task::none()
            }
            PrimitiveEdit::Symbol(msg) => self.handle_symbol_primitive_edit(path, msg),
            PrimitiveEdit::Footprint(msg) => self.handle_footprint_primitive_edit(path, msg),
        }
    }

    /// Symbol-tab branch of [`Self::handle_primitive_editor_event`].
    /// Per-library display settings (sheet color, grid, unit) mutate the
    /// shared `OpenLibrary.display`; everything else routes to the
    /// standalone symbol editor keyed by `path`.
    fn handle_symbol_primitive_edit(
        &mut self,
        path: std::path::PathBuf,
        msg: SymbolEditorMsg,
    ) -> Task<Message> {
        // Per-library display settings (sheet color, grid, unit)
        // mutate `OpenLibrary.display` rather than the per-tab editor
        // state — every primitive editor opened from the same
        // `.snxlib` shares the same view settings (Altium "Document
        // Options" parity). Run these before the editor-level
        // dispatch so the editor closure doesn't see them.
        match &msg {
            SymbolEditorMsg::SetSheetColor(color) => {
                let color = *color;
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    lib.display.sheet_color = color;
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            SymbolEditorMsg::ToggleGrid => {
                if let Some(lib) = self.library.containing_library_mut(&path) {
                    lib.display.grid_visible = !lib.display.grid_visible;
                }
                self.invalidate_primitive_canvas_cache(&path);
                return Task::none();
            }
            SymbolEditorMsg::CycleGridSize => {
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
            SymbolEditorMsg::CycleUnit => {
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
            crate::library::editor::symbol::updates::apply_symbol_primitive_edit(editor, msg);
            // v0.20 — primitive editor edits (designator/size/etc, and
            // critically `placement_paused`) need the panel context
            // rebuilt so the right-dock view reads the new value next
            // frame. Without this the panel renders against stale
            // `FootprintEditorPanelContext` and TAB-pause-driven UI
            // changes (Pad form vs no Pad form) silently miss.
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

    /// Footprint-tab branch of [`Self::handle_primitive_editor_event`].
    /// Clipboard ops split-borrow `pad_clipboard` alongside the editor;
    /// everything else routes to the standalone footprint editor keyed
    /// by `path`.
    fn handle_footprint_primitive_edit(
        &mut self,
        path: std::path::PathBuf,
        msg: FootprintEditorMsg,
    ) -> Task<Message> {
        // v0.26-E — clipboard ops need both `pad_clipboard` and the
        // editor mutable simultaneously, so split-borrow at the call
        // site instead of routing through `apply_footprint_primitive_edit`.
        match &msg {
            FootprintEditorMsg::CopyPad
            | FootprintEditorMsg::CutPad
            | FootprintEditorMsg::PastePad => {
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
            // Task 6 — capturing a preset writes straight to disk from
            // inside `apply_footprint_primitive_edit` (which only
            // borrows `editor`, not `self`). Re-read it into
            // `interaction_state` here so the very next
            // `refresh_panel_ctx()` call — a few lines down — shows
            // the new chip immediately instead of waiting for a
            // restart.
            let is_capture_preset = matches!(&msg, FootprintEditorMsg::CaptureFilterPreset);
            apply_footprint_primitive_edit(editor, msg);
            if is_capture_preset {
                self.interaction_state.footprint_filter_presets =
                    crate::fonts::read_footprint_filter_presets();
            }
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
