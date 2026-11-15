//! What the schematic says about a board component (#2367), read through its
//! link from the sheets and libraries already open; nothing is read from
//! disk here. A part whose sheet is not open is `NotRead`.

use std::path::{Path, PathBuf};

use signex_types::pcb::{Footprint, SymbolLink};
use signex_types::schematic::Symbol;

use crate::app::Signex;
use crate::app::project_sheets::project_root_sheet_path;
use crate::panels::PartIdentity;

impl Signex {
    /// What the schematic says about `footprint` on the board at `board`.
    pub(super) fn part_identity(&self, board: &Path, footprint: &Footprint) -> PartIdentity {
        let Some(link) = footprint.schematic_link.as_ref() else {
            return PartIdentity::NotLinked;
        };
        let Some(symbol) = self.linked_symbol(board, link) else {
            return PartIdentity::NotRead;
        };
        let library = symbol.library_id.and_then(|id| {
            self.library
                .open_libraries
                .iter()
                .find(|l| l.library_id == id)
        });
        let row = library.zip(symbol.row_id).and_then(|(library, row_id)| {
            library
                .tables
                .values()
                .flatten()
                .find(|row| row.row_id == row_id)
        });
        let lib_id = symbol.lib_id.trim();
        let design_item_id = row
            .map(|row| row.internal_pn.0.clone())
            .or_else(|| (!lib_id.is_empty()).then(|| lib_id.to_string()));
        PartIdentity::Read {
            designator: symbol.designator_in(&link.sheet).trim().to_string(),
            design_item_id,
            source: library.map(|l| l.display_name.clone()),
        }
    }

    /// The first unit of the part `link` names, from an open sheet of the
    /// board's project: the sheets the project hierarchy places at the link's
    /// sheet path first, else (no hierarchy yet) any open sheet of the
    /// project, in path order.
    fn linked_symbol(&self, board: &Path, link: &SymbolLink) -> Option<&Symbol> {
        let docs = &self.document_state;
        let project = docs
            .tabs
            .iter()
            .find(|tab| tab.path == board)
            .and_then(|tab| tab.project_id)
            .and_then(|id| docs.projects.iter().find(|p| p.id == id))
            .or_else(|| docs.document_project(board))?;
        let unit_on = |path: &Path| {
            docs.engines.get(path).and_then(|engine| {
                engine
                    .document()
                    .symbols
                    .iter()
                    .find(|s| link.units.contains(&s.uuid))
            })
        };
        let placed: Vec<PathBuf> = project_root_sheet_path(project)
            .and_then(|root| self.hierarchy_describing(&root))
            .map(|(hierarchy, _)| {
                hierarchy
                    .paths_at(&link.sheet)
                    .into_iter()
                    .map(Path::to_path_buf)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(symbol) = placed.iter().find_map(|path| unit_on(path)) {
            return Some(symbol);
        }
        let mut owned: Vec<&PathBuf> = docs
            .engines
            .keys()
            .filter(|path| docs.document_project(path).map(|p| p.id) == Some(project.id))
            .collect();
        owned.sort();
        owned.into_iter().find_map(|path| unit_on(path))
    }
}
