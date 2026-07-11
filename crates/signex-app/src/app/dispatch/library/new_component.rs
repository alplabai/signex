//! New Component modal handlers — the draft-row fast path, the modal
//! field setters, and the inline "+ New Table" create form that lives
//! inside the modal's Advanced disclosure.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

impl Signex {
    /// File ▸ Library ▸ New Component… — v0.13 appends a draft row
    /// directly to the active library's first table and focuses it,
    /// falling back to the legacy modal when no library / table exists.
    pub(super) fn handle_new_component(&mut self) -> Task<Message> {
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

    /// Live-edit of the New Component modal's "Internal PN" field.
    pub(super) fn handle_new_component_set_internal_pn(&mut self, s: String) -> Task<Message> {
        if let Some(nc) = self.library.new_component.as_mut() {
            nc.internal_pn = s;
            nc.error = None;
        }
        Task::none()
    }

    /// User picked a target library in the modal.
    pub(super) fn handle_new_component_set_library(&mut self, idx: usize) -> Task<Message> {
        if let Some(nc) = self.library.new_component.as_mut() {
            nc.library_idx = Some(idx);
            nc.error = None;
        }
        Task::none()
    }

    /// User picked a class in the modal pick_list.
    pub(super) fn handle_new_component_set_class(
        &mut self,
        class: signex_library::ComponentClass,
    ) -> Task<Message> {
        // Changing class does NOT overwrite the table pick —
        // that's the user's explicit choice. Class only
        // affects the parameter template.
        if let Some(nc) = self.library.new_component.as_mut() {
            nc.class = class;
            nc.error = None;
        }
        Task::none()
    }

    /// User picked a target table (filename stem) in the modal.
    pub(super) fn handle_new_component_set_table(&mut self, name: String) -> Task<Message> {
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
                    && let Some(cfg) = adapter.manifest().tables().iter().find(|c| c.name == name)
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

    /// Live-edit of the modal's "Category" field.
    pub(super) fn handle_new_component_set_category(&mut self, s: String) -> Task<Message> {
        if let Some(nc) = self.library.new_component.as_mut() {
            nc.category = s;
            nc.error = None;
        }
        Task::none()
    }

    /// Toggle the Advanced ▾ disclosure on the New Component modal.
    pub(super) fn handle_new_component_toggle_advanced(&mut self) -> Task<Message> {
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

    /// User opened the inline create-table form inside the modal.
    pub(super) fn handle_new_component_begin_create_table(&mut self) -> Task<Message> {
        if let Some(nc) = self.library.new_component.as_mut() {
            nc.creating_table = Some(crate::library::state::NewTableDraft::default());
            nc.error = None;
        }
        Task::none()
    }

    /// Live-edit of the new-table name field.
    pub(super) fn handle_new_component_set_new_table_name(
        &mut self,
        name: String,
    ) -> Task<Message> {
        if let Some(nc) = self.library.new_component.as_mut()
            && let Some(draft) = nc.creating_table.as_mut()
        {
            draft.name = name;
            draft.error = None;
        }
        Task::none()
    }

    /// Cancel the inline create-table form without writing anything.
    pub(super) fn handle_new_component_cancel_create_table(&mut self) -> Task<Message> {
        if let Some(nc) = self.library.new_component.as_mut() {
            nc.creating_table = None;
        }
        Task::none()
    }

    /// Confirm — calls `create_empty_table` on the active library's
    /// adapter, refreshes the components cache, switches the modal's
    /// `table` selection to the freshly-minted name.
    pub(super) fn handle_new_component_confirm_create_table(&mut self) -> Task<Message> {
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

    /// Submit the New Component modal — creates the draft row, then
    /// opens a Component Preview tab focused on the new row.
    pub(super) fn handle_new_component_submit(&mut self) -> Task<Message> {
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
                            slot.error = Some("Pick a target table before submitting.".into());
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
}
