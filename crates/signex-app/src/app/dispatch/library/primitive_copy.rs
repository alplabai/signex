//! The picker's offer of symbols that are in no library (#1841): list the
//! project's own `.snxsym` files, and copy the chosen symbol into the
//! target row's library before binding it, so a row never references a file
//! outside its library. A symbol of a library that is not open can instead
//! be linked: the library is opened and the row binds its symbol (lib-10).
//! Reads and the copy run on workers; the library write
//! takes its place in the library's write order. The original file is never
//! touched. Every failure goes to the Messages panel and the picker's error
//! line.

use super::*;
use crate::app::file_extension::has_extension;
use crate::library::loose_symbols::{
    LooseRead, LooseSymbol, copy_into_library, read_loose_symbols,
};
use std::path::PathBuf;
use std::sync::Arc;

impl Signex {
    /// Read the project's standalone symbol files for the picker just
    /// opened. Nothing for a picker of another kind.
    pub(super) fn list_loose_symbols_task(&self, kind: PrimitiveKind) -> Task<Message> {
        if kind != PrimitiveKind::Symbol {
            return Task::none();
        }
        read_loose_task(self.loose_symbol_candidates(), false)
    }

    /// The project-listed `.snxsym` files outside every mounted library. A
    /// file under a mounted library's directory is left out even when it is
    /// outside that library's `symbols/` folder, or when several libraries
    /// share the folder: this is the rule `deepest_libraries` applies
    /// everywhere, and `push_loose` hides the same files once it is open.
    pub(super) fn loose_symbol_candidates(&self) -> Vec<PathBuf> {
        self.document_state
            .projects
            .iter()
            .flat_map(|project| {
                project
                    .data
                    .libraries
                    .iter()
                    .map(|entry| project.data.resolve_library_path(entry))
            })
            .filter(|path| has_extension(path, "snxsym"))
            .filter(|path| {
                !self
                    .library
                    .is_in_mounted_library(path, self.identity_fold_for(path))
            })
            .collect()
    }

    /// A file outside every library, picked with Browse.
    pub(super) fn read_browsed_loose_symbol(&self, file: PathBuf) -> Task<Message> {
        read_loose_task(vec![file], true)
    }

    pub(super) fn handle_loose_listed(
        &mut self,
        read: &Arc<LooseRead>,
        from_browse: bool,
    ) -> Task<Message> {
        for problem in &read.problems {
            crate::diagnostics::log_warning(problem);
        }
        // A browsed file of an open library binds that library's symbol, as
        // the directory-form route does; it is never offered as a copy.
        if from_browse
            && let [only] = read.symbols.as_slice()
            && let Some(lib) = self.library.library_holding_with(
                &only.file,
                PrimitiveKind::Symbol,
                self.identity_fold_for(&only.file),
            )
        {
            let reference = signex_library::PrimitiveRef::new(lib.library_id, only.uuid);
            return Task::done(Message::Library(LibraryMessage::PrimitivePicker(
                PrimitivePickerMsg::Pick(reference),
            )));
        }
        let refused = from_browse
            && matches!(read.symbols.as_slice(), [one] if one.library.is_none())
            && self.refuse_copy_into_several_libraries();
        let Some(picker) = self.library.primitive_picker.as_mut() else {
            return Task::none();
        };
        for symbol in &read.symbols {
            if !picker.loose.contains(symbol) {
                picker.loose.push(symbol.clone());
            }
        }
        if from_browse {
            match read.symbols.as_slice() {
                // The refusal already said to pick a symbol from a library.
                [_] if refused => {}
                [one] => picker.copy_confirm = Some(one.clone()),
                [] => {
                    picker.error = Some("That file holds no symbol.".into());
                }
                _ => {
                    picker.error =
                        Some("That file holds several symbols; choose one from the list.".into());
                }
            }
        }
        Task::none()
    }

    pub(super) fn handle_loose_chosen(&mut self, symbol: LooseSymbol) -> Task<Message> {
        if symbol.library.is_none() && self.refuse_copy_into_several_libraries() {
            return Task::none();
        }
        if let Some(picker) = self.library.primitive_picker.as_mut() {
            picker.copy_confirm = Some(symbol);
            picker.opening_library = None;
            picker.error = None;
        }
        Task::none()
    }

    /// "Link without copying" was chosen (lib-10): mount the `.snxlib` the
    /// symbol belongs to through the usual mount request, then bind the row
    /// to that library's symbol. The question stays up as "Opening library…"
    /// until the mount lands ([`Self::picker_link_after_mount`]). A library
    /// that is already open binds at once.
    pub(super) fn handle_open_library_confirmed(&mut self) -> Task<Message> {
        let Some(picker) = self.library.primitive_picker.as_ref() else {
            return Task::none();
        };
        let Some(symbol) = picker.copy_confirm.clone().filter(|s| s.library.is_some()) else {
            return Task::none();
        };
        if picker.opening_library.is_some() || picker.copying {
            return Task::none();
        }
        let Some(library) = symbol.library.clone() else {
            return Task::none();
        };
        let mounted = self.library.library_at(&library).is_some();
        let Some(picker) = self.library.primitive_picker.as_mut() else {
            return Task::none();
        };
        picker.error = None;
        if !mounted {
            picker.opening_library = Some(library.clone());
            return self.mount_added_libraries(vec![library]);
        }
        let pick = self.link_listed_symbol(&library, &symbol);
        if let Some(picker) = self.library.primitive_picker.as_mut() {
            picker.copy_confirm = None;
        }
        // Requested although already open: a mount request cancels a close
        // still waiting on the library's writes (#1728), so the library the
        // row now binds stays open. It also rescans it, as project open does.
        Task::batch([self.mount_added_libraries(vec![library]), pick])
    }

    /// The pick that binds the row to `symbol` in the mounted library at
    /// `path`, only when that library lists the symbol's uuid. Membership
    /// comes from the directory the file sits in; the library may still have
    /// skipped the file (it did not parse, or its uuid clashes), and a row
    /// bound to a uuid the library does not hold would show Broken binding
    /// from then on. Then the picker's error line says so and nothing binds.
    fn link_listed_symbol(
        &mut self,
        path: &std::path::Path,
        symbol: &LooseSymbol,
    ) -> Task<Message> {
        let Some(lib) = self.library.library_at(path) else {
            return Task::none();
        };
        if lib.cached_symbols.iter().any(|s| s.uuid == symbol.uuid) {
            return pick_task(signex_library::PrimitiveRef::new(
                lib.library_id,
                symbol.uuid,
            ));
        }
        let reason = signex_i18n::t!(
            &self.ui_state.i18n,
            signex_i18n::ids::LIBRARY_PRIMITIVE_PICKER_NOT_LISTED,
            "library" => lib.display_name.clone(),
            "name" => symbol.name.clone()
        )
        .into_string();
        crate::diagnostics::log_warning(reason.clone());
        if let Some(picker) = self.library.primitive_picker.as_mut() {
            picker.error = Some(reason);
        }
        Task::none()
    }

    /// The link a just-mounted library completes: when `path` is the library
    /// the picker is opening for "Link without copying", the pick of that
    /// library's symbol through the usual [`PrimitivePickerMsg::Pick`]. Call
    /// it before [`Self::picker_mount_settled`], which drops the question.
    pub(super) fn picker_link_after_mount(&mut self, path: &std::path::Path) -> Task<Message> {
        let Some(picker) = self.library.primitive_picker.as_ref() else {
            return Task::none();
        };
        if picker.opening_library.as_deref() != Some(path) {
            return Task::none();
        }
        let Some(symbol) = picker
            .copy_confirm
            .clone()
            .filter(|s| s.library.as_deref() == Some(path))
        else {
            return Task::none();
        };
        self.link_listed_symbol(path, &symbol)
    }

    /// A mount finished: if it is the library the picker is opening, end the
    /// "Opening…" state. A failure goes to the picker's error line; a success
    /// is noted in the Messages panel, since the library is open for this
    /// session only and is not in the project's library list.
    pub(crate) fn picker_mount_settled(&mut self, path: &std::path::Path, failure: Option<String>) {
        let Some(picker) = self.library.primitive_picker.as_mut() else {
            return;
        };
        if picker.opening_library.as_deref() != Some(path) {
            return;
        }
        picker.opening_library = None;
        // Only the question about this library; a newer pick keeps its own.
        if picker
            .copy_confirm
            .as_ref()
            .is_some_and(|s| s.library.as_deref() == Some(path))
        {
            picker.copy_confirm = None;
        }
        let name = path
            .file_stem()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        match failure {
            Some(why) => picker.error = Some(format!("Could not open library {name}: {why}")),
            None => crate::diagnostics::log_info(format!(
                "Library {name} is open for this session. It is not in the project's library \
                 list; add it to the project to keep it."
            )),
        }
    }

    /// A copy goes into the one library of the rows the pick is for; the
    /// in-use symbol Delete's Re-bind all can list rows of several (#2572).
    /// Then a symbol from outside every library is refused, with a message
    /// saying to pick one from a library. Returns whether it refused.
    fn refuse_copy_into_several_libraries(&mut self) -> bool {
        let several = self
            .library
            .primitive_picker
            .as_ref()
            .is_some_and(|p| !p.target.rows().is_empty() && p.target.library_path().is_none());
        if !several {
            return false;
        }
        let why = self.ui_state.i18n.message_plain(
            signex_i18n::ids::LIBRARY_PRIMITIVE_PICKER_SEVERAL_LIBRARIES,
            &[],
        );
        if let Some(picker) = self.library.primitive_picker.as_mut() {
            picker.copy_confirm = None;
            picker.error = Some(why);
        }
        true
    }

    pub(super) fn handle_copy_declined(&mut self) -> Task<Message> {
        if let Some(picker) = self.library.primitive_picker.as_mut() {
            picker.copy_confirm = None;
            // Also "Cancel" of an open in progress: the mount lands unannounced.
            picker.opening_library = None;
        }
        Task::none()
    }

    /// Copy the confirmed symbol into the target row's library on a worker.
    pub(super) fn handle_copy_confirmed(&mut self) -> Task<Message> {
        if self.refuse_copy_into_several_libraries() {
            return Task::none();
        }
        let Some(picker) = self.library.primitive_picker.as_ref() else {
            return Task::none();
        };
        let pick = crate::library::state::CopyTarget {
            kind: picker.kind,
            target: picker.target.clone(),
        };
        // A symbol of a library that is not open copies like a loose one
        // (lib-10): the copy reads its file, and a uuid the row's library
        // already holds gets a fresh one.
        let (Some(symbol), Some(library_path)) = (
            picker.copy_confirm.clone(),
            picker
                .target
                .library_path()
                .map(std::path::Path::to_path_buf),
        ) else {
            return Task::none();
        };
        // Not while the link is opening the other library.
        if picker.copying
            || picker.opening_library.is_some()
            || self.refuse_read_only_write(&library_path)
        {
            return Task::none();
        }
        let adapter = self
            .library
            .library_at(&library_path)
            .and_then(|lib| self.library.set.get_shared(lib.library_id));
        let Some(adapter) = adapter else {
            return self.copy_failed(format!(
                "Open the library {} to copy a symbol into it.",
                library_path.display()
            ));
        };
        if let Some(picker) = self.library.primitive_picker.as_mut() {
            picker.copying = true;
        }
        let turn = self.library.writes.turn(&library_path);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    turn.wait();
                    let result = copy_into_library(adapter.as_ref(), &symbol.file, symbol.uuid);
                    crate::library::held_back::report_held_back(
                        &adapter.manifest().library.name,
                        adapter.as_ref(),
                    );
                    result
                })
                .await
                .unwrap_or_else(|e| Err(format!("the copy task failed: {e}")))
            },
            move |result| {
                Message::Library(LibraryMessage::PrimitivePicker(
                    PrimitivePickerMsg::CopyFinished { pick, result },
                ))
            },
        )
    }

    /// The worker is done: refresh the library the copy went into, then
    /// bind the copy exactly as a pick of it from the list would, provided
    /// the pick it was made for is still the open one. A picker cancelled or
    /// reopened for another row meanwhile gets nothing bound.
    pub(super) fn handle_copy_finished(
        &mut self,
        pick: crate::library::state::CopyTarget,
        result: Result<signex_library::PrimitiveRef, String>,
    ) -> Task<Message> {
        let still_open = self
            .library
            .primitive_picker
            .as_ref()
            .is_some_and(|picker| pick.is_open_in(picker));
        let reference = match result {
            Ok(reference) => reference,
            Err(why) if still_open => return self.copy_failed(why),
            Err(why) => {
                crate::diagnostics::log_warning(&why);
                return Task::none();
            }
        };
        let library = pick.target.library_path().and_then(|path| {
            self.library.library_at(path).map(|lib| {
                (
                    lib.display_name.clone(),
                    lib.root_dir().map(std::path::Path::to_path_buf),
                )
            })
        });
        let (name, dir) = library.unzip();
        let refresh = dir
            .flatten()
            .map_or_else(Task::none, |dir| self.refresh_primitive_cache_for(&dir));
        if !still_open {
            crate::diagnostics::log_warning(format!(
                "The symbol was copied into library {}, but the picker it was for is closed, \
                 so it was not bound. Pick it from the library to bind it.",
                name.unwrap_or_default()
            ));
            return refresh;
        }
        refresh.chain(pick_task(reference))
    }

    fn copy_failed(&mut self, why: String) -> Task<Message> {
        crate::diagnostics::log_warning(&why);
        if let Some(picker) = self.library.primitive_picker.as_mut() {
            picker.copying = false;
            picker.copy_confirm = None;
            picker.error = Some(why);
        }
        Task::none()
    }
}

/// Bind `reference` exactly as a pick of it from the list would.
fn pick_task(reference: signex_library::PrimitiveRef) -> Task<Message> {
    Task::done(Message::Library(LibraryMessage::PrimitivePicker(
        PrimitivePickerMsg::Pick(reference),
    )))
}

fn read_loose_task(files: Vec<PathBuf>, from_browse: bool) -> Task<Message> {
    if files.is_empty() {
        return Task::none();
    }
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || read_loose_symbols(&files))
                .await
                .unwrap_or_else(|e| LooseRead {
                    symbols: Vec::new(),
                    problems: vec![format!("reading symbol files failed: {e}")],
                })
        },
        move |read| {
            Message::Library(LibraryMessage::PrimitivePicker(
                PrimitivePickerMsg::LooseListed {
                    read: Arc::new(read),
                    from_browse,
                },
            ))
        },
    )
}
