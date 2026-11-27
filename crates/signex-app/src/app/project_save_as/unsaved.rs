//! The unsaved edits a Save Project As copy takes (Caner, 2026-11-25): each
//! open, edited sheet, board and library document of the project is encoded
//! from memory with the encoder its save uses. Read only: the originals stay
//! unsaved and untouched, and nothing is written here.

use std::path::{Path, PathBuf};

use super::plan::{Listed, listed};
use crate::app::Signex;
use signex_types::project::ProjectData;

/// One edited document as the copy writes it: its path below the project
/// folder and its bytes.
pub(crate) type UnsavedCopy = (PathBuf, Vec<u8>);

/// The folder below the project a listed library's documents live in: its
/// manifest's folder, or the library itself when it is kept as a folder.
fn library_dirs(items: &[Listed]) -> Vec<PathBuf> {
    items
        .iter()
        .filter_map(|item| match item {
            Listed::Library(rel) => Some(rel.parent().unwrap_or(Path::new("")).to_path_buf()),
            Listed::File(_) => None,
        })
        .collect()
}

impl Signex {
    /// The open documents of the project in `dir` (holding `data`) with
    /// unsaved edits that the copy takes: listed sheets and boards, and the
    /// symbol and footprint documents of its project libraries.
    pub(crate) fn unsaved_in_copy(&self, data: &ProjectData, dir: &Path) -> Vec<PathBuf> {
        let Ok(items) = listed(data) else {
            return Vec::new();
        };
        let libraries = library_dirs(&items);
        let docs = &self.document_state;
        let primitives = docs
            .symbol_editors
            .keys()
            .chain(docs.footprint_editors.keys());
        let mut out: Vec<PathBuf> = docs
            .stored_dirty_paths()
            .chain(primitives)
            .filter(|path| docs.is_document_dirty(path))
            .filter(|path| {
                let Ok(rel) = path.strip_prefix(dir) else {
                    return false;
                };
                let primitive = docs.symbol_editors.contains_key(path.as_path())
                    || docs.footprint_editors.contains_key(path.as_path());
                if primitive {
                    libraries.iter().any(|lib| rel.starts_with(lib))
                } else {
                    items.contains(&Listed::File(rel.to_path_buf()))
                }
            })
            .cloned()
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The bytes of each document [`Self::unsaved_in_copy`] lists, encoded as
    /// its save encodes it: a sheet by its engine, a board by its `.snxpcb`
    /// writer, a symbol or footprint document by its container writer (whole,
    /// as the editor holds it). A document opened read-only from a newer
    /// minor is left to its bytes on disk. Refused when one does not encode.
    pub(crate) fn unsaved_copies(
        &self,
        data: &ProjectData,
        dir: &Path,
    ) -> Result<Vec<UnsavedCopy>, String> {
        let mut out = Vec::new();
        for path in self.unsaved_in_copy(data, dir) {
            let Ok(rel) = path.strip_prefix(dir) else {
                continue;
            };
            if let Some(bytes) = self.encode_unsaved(&path)? {
                out.push((rel.to_path_buf(), bytes));
            }
        }
        Ok(out)
    }

    /// The document at `path` encoded from memory; `None` when it is kept as
    /// on disk.
    fn encode_unsaved(&self, path: &Path) -> Result<Option<Vec<u8>>, String> {
        let docs = &self.document_state;
        let failed =
            |e: &dyn std::fmt::Display| format!("could not encode {}: {e}", path.display());
        if let Some(engine) = docs.engines.get(path) {
            if engine.document().is_read_only() {
                return Ok(None);
            }
            let measure = crate::schematic_runtime::text_width::live_extent_measure();
            let text = engine
                .serialize_with_extents(measure)
                .map_err(|e| failed(&e))?;
            return Ok(Some(text.into_bytes()));
        }
        if docs.holds_pcb(path) {
            let Some(board) = docs.pcb_board(path) else {
                return Err(format!(
                    "{} could not be re-read after a History restore; close and reopen the tab",
                    path.display()
                ));
            };
            if board.unknown.newer_minor_source.is_some() {
                return Ok(None);
            }
            let text = signex_types::format::SnxPcb::new(board.clone())
                .write_string()
                .map_err(|e| failed(&e))?;
            return Ok(Some(text.into_bytes()));
        }
        let now = chrono::Utc::now();
        if let Some(editor) = docs.symbol_editors.get(path) {
            // Opened repaired: copied as it is on disk, like a newer-minor
            // sheet; only the person's Save writes the repair (#3129).
            if editor.draft.is_read_only() {
                return Ok(None);
            }
            let mut file = editor.draft.file.clone();
            let changed = file.changed_since(&editor.draft.baseline);
            for symbol in &mut file.symbols {
                if changed.contains(&symbol.uuid) {
                    symbol.updated = now;
                }
            }
            if !changed.is_empty() {
                file.updated = now;
            }
            let text = file.to_toml_string().map_err(|e| failed(&e))?;
            return Ok(Some(text.into_bytes()));
        }
        if let Some(editor) = docs.footprint_editors.get(path) {
            let mut file = editor.file.clone();
            // The canvas pads are authoritative, as in the editor's save.
            let active = editor
                .active_idx
                .min(file.footprints.len().saturating_sub(1));
            if let Some(footprint) = file.footprints.get_mut(active) {
                crate::library::editor::footprint::FootprintEditorState::sync_pads_to_primitive(
                    &editor.state,
                    footprint,
                );
            }
            let changed = file.changed_since(&editor.baseline);
            for footprint in &mut file.footprints {
                if changed.contains(&footprint.uuid) {
                    footprint.updated = now;
                }
            }
            if !changed.is_empty() {
                file.updated = now;
            }
            let text = file.to_toml_string().map_err(|e| failed(&e))?;
            return Ok(Some(text.into_bytes()));
        }
        Ok(None)
    }
}
