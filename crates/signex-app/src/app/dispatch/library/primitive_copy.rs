//! The picker's offer of symbols that are in no library (#1841): list the
//! project's own `.snxsym` files, and copy the chosen symbol into the
//! target row's library before binding it, so a row never references a file
//! outside its library. Reads and the copy run on workers; the library write
//! takes its place in the library's write order. The original file is never
//! touched. Every failure goes to the Messages panel and the picker's error
//! line.

use super::*;
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
        let files: Vec<PathBuf> = self
            .document_state
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
            .filter(|path| self.library.containing_library(path).is_none())
            .collect();
        read_loose_task(files, false)
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
        if let Some(picker) = self.library.primitive_picker.as_mut() {
            picker.copy_confirm = Some(symbol);
            picker.error = None;
        }
        Task::none()
    }

    pub(super) fn handle_copy_declined(&mut self) -> Task<Message> {
        if let Some(picker) = self.library.primitive_picker.as_mut() {
            picker.copy_confirm = None;
        }
        Task::none()
    }

    /// Copy the confirmed symbol into the target row's library on a worker.
    pub(super) fn handle_copy_confirmed(&mut self) -> Task<Message> {
        let Some(picker) = self.library.primitive_picker.as_ref() else {
            return Task::none();
        };
        let pick = crate::library::state::CopyTarget {
            kind: picker.kind,
            target: picker.target.clone(),
        };
        let (Some(symbol), Some(library_path)) = (
            picker.copy_confirm.clone(),
            picker
                .target
                .library_path()
                .map(std::path::Path::to_path_buf),
        ) else {
            return Task::none();
        };
        if picker.copying || self.refuse_read_only_write(&library_path) {
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
        refresh.chain(Task::done(Message::Library(
            LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Pick(reference)),
        )))
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

fn has_extension(path: &std::path::Path, ext: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
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
