//! A new import's staged preview (#3171): reading its part table and the
//! selected part's drawing on workers, removing its folder when the dialog
//! moves on, and the quit rung that removes it before Signex exits. The
//! engine side is `crate::library_import::staging`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::primitive::PrimitiveRef;
use signex_library::{LibraryAdapter as _, LibraryError, PrimitiveSummary, RowId};

use super::*;
use crate::library::import_dialog::parts::{
    PartDrawing, PartRead, PartView, PartsState, PreviewPart, Supplier,
};
use crate::library::state::ComponentsPanelRowKey;
use crate::library_import::StagedLibrary;
use crate::panels::components_panel::details::ComponentDetails;
use crate::panels::components_panel::details::load::load_details;

/// The parts of the staged library `library_file`, by part number. Reads
/// only; blocking.
fn read_staged_parts(library_file: &Path) -> Result<Vec<PreviewPart>, String> {
    let adapter = LocalGitAdapter::open_to_read(library_file).map_err(|e| e.to_string())?;
    let own = adapter.library_id();
    let names =
        |listed: Result<Vec<PrimitiveSummary>, LibraryError>| -> HashMap<uuid::Uuid, String> {
            listed
                .unwrap_or_default()
                .into_iter()
                .map(|s| (s.uuid, s.name))
                .collect()
        };
    let symbols = names(adapter.list_symbols());
    let footprints = names(adapter.list_footprints());
    let name_in = |map: &HashMap<uuid::Uuid, String>, r: &PrimitiveRef| {
        (r.library_id == own)
            .then(|| map.get(&r.uuid).cloned())
            .flatten()
    };
    let rows = adapter.iter_rows().map_err(|e| e.to_string())?;
    let mut parts: Vec<PreviewPart> = rows
        .into_iter()
        .map(|(table, row)| PreviewPart {
            symbol: name_in(&symbols, &row.symbol_ref),
            footprint: row
                .footprint_ref
                .as_ref()
                .and_then(|r| name_in(&footprints, r)),
            alternates: row
                .alternate_footprints
                .iter()
                .map(|b| name_in(&footprints, &b.footprint).unwrap_or_else(|| short(&b.footprint)))
                .collect(),
            manufacturer: row.primary_mpn.manufacturer.clone(),
            mpn: row.primary_mpn.mpn.clone(),
            suppliers: row
                .supply
                .iter()
                .map(|s| Supplier {
                    distributor: s.distributor.clone(),
                    sku: s.sku.clone(),
                })
                .collect(),
            part_number: row.internal_pn.to_string(),
            row_id: RowId(row.row_id),
            table,
        })
        .collect();
    parts.sort_by(|a, b| {
        a.part_number
            .cmp(&b.part_number)
            .then_with(|| a.table.cmp(&b.table))
    });
    Ok(parts)
}

/// A footprint not found by name: the first eight characters of its id.
fn short(reference: &PrimitiveRef) -> String {
    reference
        .uuid
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect()
}

/// The symbol, footprints and parameters of one part of the staged
/// library, read as the Components panel reads them. Blocking.
fn read_part_drawing(
    library_file: &Path,
    table: &str,
    row_id: RowId,
) -> Result<ComponentDetails, String> {
    let adapter = LocalGitAdapter::open_to_read(library_file).map_err(|e| e.to_string())?;
    let key = ComponentsPanelRowKey {
        library_path: library_file.to_path_buf(),
        table: table.to_string(),
        row_id,
    };
    load_details(&key, &adapter, &Vec::new()).map_err(|e| e.to_string())
}

/// Remove a staged preview's folder on a worker.
pub(crate) fn discard_staged(staged: StagedLibrary) -> Task<Message> {
    Task::future(async move {
        let shown = staged.dir().display().to_string();
        let gone = tokio::task::spawn_blocking(move || staged.discard()).await;
        if !matches!(gone, Ok(true)) {
            log::warn!("Library import: the preview {shown} was not removed");
        }
    })
    .discard()
}

impl Signex {
    /// The staged library's part table, read on a worker once the preview
    /// lands; nothing when there is none to read.
    pub(crate) fn read_preview_parts(&mut self) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return Task::none();
        };
        let Some(staged) = d.staged.as_deref() else {
            return Task::none();
        };
        if d.parts.is_some() {
            return Task::none();
        }
        d.parts = Some(PartsState::Loading);
        let (dir, file) = (staged.dir().to_path_buf(), staged.library_file.clone());
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_staged_parts(&file))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
            },
            move |parts| dialog_message(ImportDialogMsg::PartsRead { dir, parts }),
        )
    }

    /// The part table was read; a read of a preview the dialog no longer
    /// holds is dropped.
    pub(super) fn preview_parts_read(
        &mut self,
        dir: PathBuf,
        parts: Result<Vec<PreviewPart>, String>,
    ) -> Task<Message> {
        if let Some(d) = self.library.import_dialog.as_deref_mut()
            && d.staged.as_deref().is_some_and(|s| s.dir() == dir)
        {
            d.parts = Some(match parts {
                Ok(parts) => PartsState::Ready(parts),
                Err(why) => PartsState::Failed(why),
            });
        }
        Task::none()
    }

    /// A part was selected: its drawing is read on a worker.
    pub(super) fn preview_part_selected(&mut self, index: usize) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return Task::none();
        };
        let Some(staged) = d.staged.as_deref() else {
            return Task::none();
        };
        let Some(part) = d.preview_parts().get(index) else {
            return Task::none();
        };
        let (table, row_id) = (part.table.clone(), part.row_id);
        let (dir, file) = (staged.dir().to_path_buf(), staged.library_file.clone());
        d.part_view = Some(PartView {
            index,
            read: PartRead::Loading,
            footprint: None,
        });
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_part_drawing(&file, &table, row_id))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
            },
            move |read| {
                let read = read.map(|details| PartDrawing(Arc::new(details)));
                dialog_message(ImportDialogMsg::PartDrawingRead { dir, index, read })
            },
        )
    }

    /// A part's drawing was read; only the selected part's, of the preview
    /// the dialog still holds, counts.
    pub(super) fn preview_part_drawing_read(
        &mut self,
        dir: PathBuf,
        index: usize,
        read: Result<PartDrawing, String>,
    ) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return Task::none();
        };
        if !d.staged.as_deref().is_some_and(|s| s.dir() == dir) {
            return Task::none();
        }
        if let Some(view) = d.part_view.as_mut()
            && view.index == index
        {
            view.read = match read {
                Ok(drawing) => PartRead::Ready(drawing),
                Err(why) => PartRead::Failed(why),
            };
        }
        Task::none()
    }

    /// The dialog's staged preview goes: its reads are forgotten and its
    /// folder removed on a worker.
    pub(super) fn take_staged_preview(&mut self) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return Task::none();
        };
        d.clear_drawn_preview();
        d.move_problem = None;
        d.staged
            .take()
            .map_or_else(Task::none, |staged| discard_staged(*staged))
    }

    /// Quit path, after the import run's wait: the dialog's staged preview
    /// and dry-run folder are removed before `next` runs. Placed after the
    /// run's wait, so an Import moving the preview into place has finished
    /// (a moved preview leaves nothing to remove).
    pub(crate) fn discard_import_preview_then(&mut self, next: Task<Message>) -> Task<Message> {
        let Some(d) = self.library.import_dialog.as_deref_mut() else {
            return next;
        };
        let staged = d.staged.take();
        let dry_run = d.preview_dir.take();
        if staged.is_none() && dry_run.is_none() {
            return next;
        }
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    if let Some(staged) = staged
                        && !staged.discard()
                    {
                        log::warn!(
                            "Quit: the library preview {} was not removed",
                            staged.dir().display()
                        );
                    }
                    if let Some(dir) = dry_run {
                        super::remove_dry_run_dir_now(&dir);
                    }
                })
                .await
            },
            |_| Message::noop(),
        )
        .chain(next)
    }
}
